use rayon::prelude::*;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use std::time::Instant;

use dorfromantik_remake::env::{Action, GraphObservation};
use dorfromantik_remake::nn::HexGNNModel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableGraphObservation {
    pub node_positions: Vec<(i32, i32)>,
    pub node_features_flat: Vec<f32>,
    pub edge_index: Vec<(usize, usize)>,
    pub valid_actions: Vec<Action>,
    pub action_features_flat: Vec<f32>,
}

impl SerializableGraphObservation {
    pub fn to_graph_observation(&self) -> GraphObservation {
        let n_nodes = self.node_positions.len();
        let mut node_features = Vec::with_capacity(n_nodes);
        for i in 0..n_nodes {
            let start = i * 70;
            let end = start + 70;
            let mut arr = [0.0f32; 70];
            arr.copy_from_slice(&self.node_features_flat[start..end]);
            node_features.push(arr);
        }

        let n_actions = self.valid_actions.len();
        let mut action_features = Vec::with_capacity(n_actions);
        for i in 0..n_actions {
            let start = i * 16;
            let end = start + 16;
            let mut arr = [0.0f32; 16];
            arr.copy_from_slice(&self.action_features_flat[start..end]);
            action_features.push(arr);
        }

        GraphObservation {
            node_positions: self.node_positions.clone(),
            node_features,
            edge_index: self.edge_index.clone(),
            valid_actions: self.valid_actions.clone(),
            action_features,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealScoreSample {
    pub obs: SerializableGraphObservation,
    pub real_score: f32,
    pub remaining_tiles: usize,
    pub placed_count: usize,
}

fn main() {
    let dataset_path = "data/real_score_dataset_1m.bin";
    if !Path::new(dataset_path).exists() {
        eprintln!("❌ Chưa tìm thấy file dataset: {}", dataset_path);
        eprintln!("👉 Hãy chạy lệnh sinh dataset trước: cargo run --release --bin gen_dataset_real_score");
        return;
    }

    println!("============================================================");
    println!(">>> HUẤN LUYỆN NNUE SUPERVISED TRÊN ĐIỂM SỐ THẬT (1M SAMPLES) <<<");
    println!(" - Đang nạp dataset từ: {}...", dataset_path);
    println!("============================================================\n");

    let file = File::open(dataset_path).unwrap();
    let mut reader = BufReader::new(file);
    let dataset: Vec<RealScoreSample> = bincode::deserialize_from(&mut reader).unwrap();
    let n_samples = dataset.len();
    println!("✅ Đã nạp thành công {} samples độc nhất (100% Unique)!", n_samples);

    let mut model = HexGNNModel::new();
    let lr = 0.0005;
    let epochs = 10;
    let batch_size = 1024;
    let num_batches = n_samples / batch_size;

    println!(" - Số Epochs: {}", epochs);
    println!(" - Batch Size: {} ({} batches / epoch)", batch_size, num_batches);
    println!(" - Learning Rate: {}\n", lr);

    let mut indices: Vec<usize> = (0..n_samples).collect();
    let mut rng = rand::thread_rng();

    for epoch in 1..=epochs {
        let start_time = Instant::now();
        indices.shuffle(&mut rng);

        let mut total_loss = 0.0f32;
        let mut total_error = 0.0f32;

        for b in 0..num_batches {
            let start_idx = b * batch_size;
            let end_idx = start_idx + batch_size;
            let batch_indices = &indices[start_idx..end_idx];

            let chunk_size = 64;
            let (mut batch_grads, (batch_loss, batch_err)) = batch_indices
                .par_chunks(chunk_size)
                .map(|chunk| {
                    let mut chunk_grad = HexGNNModel::new_zero();
                    let mut chunk_loss = 0.0f32;
                    let mut chunk_err = 0.0f32;

                    for &idx in chunk {
                        let sample = &dataset[idx];
                        let obs = sample.obs.to_graph_observation();
                        // Chuẩn hóa điểm số thật: scale về [0, 100] (chia cho 100.0)
                        let target_v = sample.real_score / 100.0;

                        let (_, pred_val) = model.forward(&obs.node_positions, &obs.node_features, &obs.edge_index, &obs.valid_actions, &obs.action_features);
                        
                        let delta = pred_val - target_v;
                        // Smooth L1 (Huber Loss)
                        let abs_d = delta.abs();
                        let loss = if abs_d <= 1.0 {
                            0.5 * delta * delta
                        } else {
                            abs_d - 0.5
                        };

                        chunk_loss += loss;
                        chunk_err += abs_d * 100.0;

                        let dummy_pi = vec![0.0f32; obs.valid_actions.len()];
                        model.backward_accumulate_alphazero(
                            &obs.node_positions,
                            &obs.node_features,
                            &obs.edge_index,
                            &obs.valid_actions,
                            &obs.action_features,
                            &dummy_pi,
                            target_v,
                            &mut chunk_grad,
                        );
                    }

                    (chunk_grad, (chunk_loss, chunk_err))
                })
                .reduce(
                    || (HexGNNModel::new_zero(), (0.0f32, 0.0f32)),
                    |(mut g_acc, (l_acc, e_acc)), (g, (l, e))| {
                        g_acc.add_assign(&g);
                        (g_acc, (l_acc + l, e_acc + e))
                    },
                );

            batch_grads.scale_assign(1.0 / batch_size as f32);
            batch_grads.clip_grad_norm(5.0);
            model.update_weights_adam(&batch_grads, lr);
            total_loss += batch_loss;
            total_error += batch_err;
        }

        let avg_loss = total_loss / (num_batches * batch_size) as f32;
        let avg_err = total_error / (num_batches * batch_size) as f32;
        let dur = start_time.elapsed();

        println!(
            "Epoch [{:>2}/{:>2}] | Huber Loss: {:.4} | Sai số TB: ±{:.1} pts | Thời gian: {:.2}s",
            epoch, epochs, avg_loss, avg_err, dur.as_secs_f32()
        );
    }

    let model_out = "models/nnue_real_score_model.bin";
    fs::create_dir_all("models").unwrap();
    model.save_to_file(model_out).unwrap();
    println!("\n🎉 ĐÃ HUẤN LUYỆN XONG! Model NNUE lưu tại: {}", model_out);
}

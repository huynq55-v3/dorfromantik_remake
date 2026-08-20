use rayon::prelude::*;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use std::time::Instant;

use dorfromantik_remake::env::{Action, GraphObservation};
use dorfromantik_remake::nn::{HexGNNModel, HIDDEN_DIM, NODE_FEAT_DIM};

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

/// Dạng đồ thị rút gọn chỉ phục vụ Value Head
pub struct ValueGraph {
    pub node_features: Vec<[f32; 70]>,
    pub edge_index: Vec<(usize, usize)>,
    pub target_val: f32,
}

/// CSR format cho Graph Adjacency giúp CPU quét bộ nhớ cực nhanh
struct CsrGraph {
    pub offsets: Vec<usize>,
    pub targets: Vec<usize>,
}

impl CsrGraph {
    pub fn from_edges(n_nodes: usize, edges: &[(usize, usize)]) -> Self {
        let mut counts = vec![0usize; n_nodes];
        for &(u, _) in edges {
            if u < n_nodes {
                counts[u] += 1;
            }
        }
        let mut offsets = Vec::with_capacity(n_nodes + 1);
        offsets.push(0);
        for c in &counts {
            offsets.push(offsets.last().unwrap() + c);
        }
        let mut targets = vec![0usize; edges.len()];
        let mut current_pos = offsets.clone();
        for &(u, v) in edges {
            if u < n_nodes {
                let p = current_pos[u];
                targets[p] = v;
                current_pos[u] += 1;
            }
        }
        Self { offsets, targets }
    }
}

/// Forward & Backward siêu tốc sử dụng SGEMM BLAS (matrixmultiply)
pub fn train_value_batch_blas(model: &HexGNNModel, batch: &[&ValueGraph]) -> (HexGNNModel, f32) {
    let b_count = batch.len();
    let mut total_nodes = 0usize;
    let mut node_offsets = Vec::with_capacity(b_count);

    for g in batch {
        node_offsets.push(total_nodes);
        total_nodes += g.node_features.len();
    }

    if total_nodes == 0 {
        return (HexGNNModel::new_zero(), 0.0);
    }

    let mut x_flat = vec![0.0f32; total_nodes * NODE_FEAT_DIM];
    let mut combined_edges = Vec::new();

    for (i, g) in batch.iter().enumerate() {
        let offset = node_offsets[i];
        let n_nodes = g.node_features.len();
        for u in 0..n_nodes {
            x_flat[(offset + u) * NODE_FEAT_DIM..(offset + u + 1) * NODE_FEAT_DIM]
                .copy_from_slice(&g.node_features[u]);
        }
        for &(u, v) in &g.edge_index {
            combined_edges.push((offset + u, offset + v));
        }
    }

    let csr = CsrGraph::from_edges(total_nodes, &combined_edges);
    let n_layers = model.layers.len();
    let mut h_pres = Vec::with_capacity(n_layers);
    let mut h_vals = Vec::with_capacity(n_layers);
    let mut neighs = Vec::with_capacity(n_layers);
    let mut h_curr = x_flat.clone();
    let mut curr_dim = NODE_FEAT_DIM;

    for (li, layer) in model.layers.iter().enumerate() {
        let has_residual = li > 0;
        let out_dim = HIDDEN_DIM;

        // Neighbor aggregation CSR
        let mut neigh = vec![0.0f32; total_nodes * curr_dim];
        for u in 0..total_nodes {
            let start = csr.offsets[u];
            let end = csr.offsets[u + 1];
            let count = end - start;
            if count > 0 {
                let inv = 1.0 / count as f32;
                let u_off = u * curr_dim;
                for idx in start..end {
                    let v = csr.targets[idx];
                    let v_off = v * curr_dim;
                    for d in 0..curr_dim {
                        neigh[u_off + d] += h_curr[v_off + d];
                    }
                }
                for d in 0..curr_dim {
                    neigh[u_off + d] *= inv;
                }
            }
        }

        // SGEMM Forward: Y_s = X * W_s^T + b_s, Y_n = Neigh * W_n^T + b_n
        let y_s = layer.w_self.forward(&h_curr, total_nodes * curr_dim);
        let y_n = layer.w_neigh.forward(&neigh, total_nodes * curr_dim);

        let mut h_pre = vec![0.0f32; total_nodes * out_dim];
        let mut h_val = vec![0.0f32; total_nodes * out_dim];

        for i in 0..total_nodes * out_dim {
            let sum = y_s[i] + y_n[i];
            h_pre[i] = sum;
            let relu = if sum > 0.0 { sum } else { 0.0 };
            h_val[i] = if has_residual { relu + h_curr[i] } else { relu };
        }

        neighs.push(neigh);
        h_pres.push(h_pre);
        h_vals.push(h_val.clone());
        h_curr = h_val;
        curr_dim = out_dim;
    }

    let h_final = h_vals.last().unwrap();

    // Value Head Forward
    let mut mean_h_batch = vec![0.0f32; b_count * HIDDEN_DIM];
    for (i, g) in batch.iter().enumerate() {
        let offset = node_offsets[i];
        let n_nodes = g.node_features.len();
        if n_nodes > 0 {
            let inv_n = 1.0 / n_nodes as f32;
            for u in 0..n_nodes {
                for d in 0..HIDDEN_DIM {
                    mean_h_batch[i * HIDDEN_DIM + d] += h_final[(offset + u) * HIDDEN_DIM + d] * inv_n;
                }
            }
        }
    }

    let val_hidden = model.w_val1.forward(&mean_h_batch, b_count * HIDDEN_DIM);
    let mut val_relu = vec![0.0f32; b_count * HIDDEN_DIM];
    for i in 0..b_count * HIDDEN_DIM {
        val_relu[i] = if val_hidden[i] > 0.0 { val_hidden[i] } else { 0.0 };
    }
    let all_values = model.w_val2.forward(&val_relu, b_count * HIDDEN_DIM);

    // Value Huber Loss
    let mut total_val_loss = 0.0f32;
    let mut val_grads = vec![0.0f32; b_count];

    for (i, g) in batch.iter().enumerate() {
        let pred_val = all_values[i];
        let val_err = pred_val - g.target_val;
        let sample_val_loss = if val_err.abs() <= 1.0 {
            0.5 * val_err * val_err
        } else {
            val_err.abs() - 0.5
        };
        total_val_loss += sample_val_loss;
        val_grads[i] = val_err.clamp(-1.0, 1.0);
    }

    let mut grads = HexGNNModel::new_zero();

    // 1. Backprop qua Value Head (SGEMM)
    let mut d_val_relu = vec![0.0f32; b_count * HIDDEN_DIM];
    for i in 0..b_count {
        let v_g = val_grads[i];
        grads.w_val2.bias[0] += v_g;
        for d in 0..HIDDEN_DIM {
            grads.w_val2.weight[d] += v_g * val_relu[i * HIDDEN_DIM + d];
            d_val_relu[i * HIDDEN_DIM + d] = v_g * model.w_val2.weight[d];
        }
    }

    let mut d_val_hidden = vec![0.0f32; b_count * HIDDEN_DIM];
    for i in 0..b_count * HIDDEN_DIM {
        d_val_hidden[i] = if val_hidden[i] > 0.0 { d_val_relu[i] } else { 0.0 };
    }

    // d_mean_h = d_val_hidden * W_val1 [b_count x HIDDEN_DIM] * [HIDDEN_DIM x HIDDEN_DIM]
    let mut d_mean_h = vec![0.0f32; b_count * HIDDEN_DIM];
    unsafe {
        // grad_W_val1 = d_val_hidden^T * mean_h_batch
        matrixmultiply::sgemm(
            HIDDEN_DIM, b_count, HIDDEN_DIM,
            1.0,
            d_val_hidden.as_ptr(), 1, HIDDEN_DIM as isize,
            mean_h_batch.as_ptr(), HIDDEN_DIM as isize, 1,
            1.0,
            grads.w_val1.weight.as_mut_ptr(), HIDDEN_DIM as isize, 1,
        );
        // d_mean_h = d_val_hidden * W_val1
        matrixmultiply::sgemm(
            b_count, HIDDEN_DIM, HIDDEN_DIM,
            1.0,
            d_val_hidden.as_ptr(), HIDDEN_DIM as isize, 1,
            model.w_val1.weight.as_ptr(), HIDDEN_DIM as isize, 1,
            0.0,
            d_mean_h.as_mut_ptr(), HIDDEN_DIM as isize, 1,
        );
    }
    for i in 0..b_count {
        for d in 0..HIDDEN_DIM {
            grads.w_val1.bias[d] += d_val_hidden[i * HIDDEN_DIM + d];
        }
    }

    let mut d_h_final = vec![0.0f32; total_nodes * HIDDEN_DIM];
    for (i, g) in batch.iter().enumerate() {
        let offset = node_offsets[i];
        let n_nodes = g.node_features.len();
        if n_nodes > 0 {
            let inv_n = 1.0 / n_nodes as f32;
            for u in 0..n_nodes {
                for d in 0..HIDDEN_DIM {
                    d_h_final[(offset + u) * HIDDEN_DIM + d] += d_mean_h[i * HIDDEN_DIM + d] * inv_n;
                }
            }
        }
    }

    // 2. Backprop qua các tầng GNN với SGEMM
    let mut d_h_curr = d_h_final;
    for li in (0..n_layers).rev() {
        let has_residual = li > 0;
        let in_dim = if li == 0 { NODE_FEAT_DIM } else { HIDDEN_DIM };
        let out_dim = HIDDEN_DIM;
        let layer = &model.layers[li];
        let grad_layer = &mut grads.layers[li];

        let mut d_relu = vec![0.0f32; total_nodes * out_dim];
        for i in 0..total_nodes * out_dim {
            d_relu[i] = if h_pres[li][i] > 0.0 { d_h_curr[i] } else { 0.0 };
        }

        let h_in = if has_residual { &h_vals[li - 1] } else { &x_flat };

        // Bias grads
        for u in 0..total_nodes {
            for o in 0..out_dim {
                let g = d_relu[u * out_dim + o];
                grad_layer.w_self.bias[o] += g;
                grad_layer.w_neigh.bias[o] += g;
            }
        }

        // SGEMM Matrix Multiplication Gradients:
        // 1. grad_W_self += d_relu^T * h_in  ([out_dim x total_nodes] * [total_nodes x in_dim] -> [out_dim x in_dim])
        // 2. grad_W_neigh += d_relu^T * neigh ([out_dim x total_nodes] * [total_nodes x in_dim] -> [out_dim x in_dim])
        unsafe {
            matrixmultiply::sgemm(
                out_dim, total_nodes, in_dim,
                1.0,
                d_relu.as_ptr(), 1, out_dim as isize,
                h_in.as_ptr(), in_dim as isize, 1,
                1.0,
                grad_layer.w_self.weight.as_mut_ptr(), in_dim as isize, 1,
            );
            matrixmultiply::sgemm(
                out_dim, total_nodes, in_dim,
                1.0,
                d_relu.as_ptr(), 1, out_dim as isize,
                neighs[li].as_ptr(), in_dim as isize, 1,
                1.0,
                grad_layer.w_neigh.weight.as_mut_ptr(), in_dim as isize, 1,
            );
        }

        if has_residual {
            let mut d_h_prev = d_h_curr;
            let mut d_neigh = vec![0.0f32; total_nodes * out_dim];

            // 3. d_h_prev += d_relu * W_self ([total_nodes x out_dim] * [out_dim x in_dim] -> [total_nodes x in_dim])
            // 4. d_neigh += d_relu * W_neigh ([total_nodes x out_dim] * [out_dim x in_dim] -> [total_nodes x in_dim])
            unsafe {
                matrixmultiply::sgemm(
                    total_nodes, out_dim, in_dim,
                    1.0,
                    d_relu.as_ptr(), out_dim as isize, 1,
                    layer.w_self.weight.as_ptr(), in_dim as isize, 1,
                    1.0,
                    d_h_prev.as_mut_ptr(), in_dim as isize, 1,
                );
                matrixmultiply::sgemm(
                    total_nodes, out_dim, in_dim,
                    1.0,
                    d_relu.as_ptr(), out_dim as isize, 1,
                    layer.w_neigh.weight.as_ptr(), in_dim as isize, 1,
                    0.0,
                    d_neigh.as_mut_ptr(), in_dim as isize, 1,
                );
            }

            // Scatter back to neighbor nodes via CSR
            for u in 0..total_nodes {
                let start = csr.offsets[u];
                let end = csr.offsets[u + 1];
                let count = end - start;
                if count > 0 {
                    let inv = 1.0 / count as f32;
                    let u_off = u * out_dim;
                    for idx in start..end {
                        let v = csr.targets[idx];
                        let v_off = v * out_dim;
                        for d in 0..out_dim {
                            d_h_prev[v_off + d] += d_neigh[u_off + d] * inv;
                        }
                    }
                }
            }
            d_h_curr = d_h_prev;
        }
    }

    (grads, total_val_loss)
}

fn main() {
    let dataset_path = "data/real_score_dataset_1m.bin";
    if !Path::new(dataset_path).exists() {
        eprintln!("❌ Chưa tìm thấy file dataset: {}", dataset_path);
        eprintln!("👉 Hãy chạy lệnh sinh dataset trước: cargo run --release --bin gen_dataset_real_score");
        return;
    }

    println!("============================================================");
    println!(">>> HUẤN LUYỆN GNN SGEMM BLAS (SIÊU TỐC ĐỘ 2000+ MẪU/S) <<<");
    println!(" - Đang nạp dataset từ: {}...", dataset_path);
    println!("============================================================\n");

    let file = File::open(dataset_path).unwrap();
    let mut reader = BufReader::new(file);
    let dataset: Vec<RealScoreSample> = bincode::deserialize_from(&mut reader).unwrap();
    let n_samples = dataset.len();
    println!("✅ Đã nạp thành công {} samples độc nhất (100% Unique)!", n_samples);

    println!("[Chuyển Đổi] Tối ưu hóa cấu trúc Pure Value Graph...");
    let value_graphs: Vec<ValueGraph> = dataset
        .into_par_iter()
        .map(|s| {
            let obs = s.obs.to_graph_observation();
            ValueGraph {
                node_features: obs.node_features,
                edge_index: obs.edge_index,
                target_val: s.real_score / 100.0,
            }
        })
        .collect();

    let mut model = HexGNNModel::new();
    let lr = 0.0005;
    let epochs = 5;
    let batch_size = 1024;
    let num_batches = n_samples / batch_size;

    println!(" - Số Epochs: {}", epochs);
    println!(" - Batch Size: {} ({} batches / epoch)", batch_size, num_batches);
    println!(" - Tối ưu: MatrixMultiply SGEMM (AVX2 FMA Full BLAS Acceleration)\n");

    let mut indices: Vec<usize> = (0..n_samples).collect();
    let mut rng = rand::thread_rng();

    for epoch in 1..=epochs {
        let start_time = Instant::now();
        indices.shuffle(&mut rng);

        let mut total_val_loss = 0.0f32;

        for b in 0..num_batches {
            let start_idx = b * batch_size;
            let end_idx = start_idx + batch_size;
            let batch_indices = &indices[start_idx..end_idx];

            let chunk_size = 128; // Tăng chunk_size để SGEMM ma trận lớn hơn, tận dụng tối đa AVX2
            let (mut batch_grads, batch_val_loss) = batch_indices
                .par_chunks(chunk_size)
                .map(|chunk| {
                    let chunk_samples: Vec<&ValueGraph> = chunk.iter().map(|&idx| &value_graphs[idx]).collect();
                    train_value_batch_blas(&model, &chunk_samples)
                })
                .reduce(
                    || (HexGNNModel::new_zero(), 0.0f32),
                    |(mut g_acc, val_acc), (g, val)| {
                        g_acc.add_assign(&g);
                        (g_acc, val_acc + val)
                    },
                );

            batch_grads.scale_assign(1.0 / batch_size as f32);
            batch_grads.clip_grad_norm(5.0);
            model.update_weights_adam(&batch_grads, lr);
            total_val_loss += batch_val_loss;

            if (b + 1) % 10 == 0 || (b + 1) == num_batches {
                use std::io::Write;
                let elapsed = start_time.elapsed().as_secs_f32();
                let cur_loss = total_val_loss / ((b + 1) * batch_size) as f32;
                let speed = ((b + 1) * batch_size) as f32 / elapsed.max(0.001);
                print!(
                    "\r⏳ [Epoch {:>2}/{:>2}] Batch {:>3}/{} ({:>3.0}%) | Value Loss: {:.4} | Tốc độ: {:>6.0} mẫu/s | {:.1}s",
                    epoch, epochs, b + 1, num_batches, ((b + 1) as f32 / num_batches as f32) * 100.0, cur_loss, speed, elapsed
                );
                let _ = std::io::stdout().flush();
            }
        }

        let avg_loss = total_val_loss / (num_batches * batch_size) as f32;
        let dur = start_time.elapsed();
        let speed = (num_batches * batch_size) as f32 / dur.as_secs_f32();

        println!(
            "\n✅ [Epoch {:>2}/{:>2} XONG] | Value Huber Loss: {:.4} | Tốc độ TB: {:>6.0} mẫu/s | Tổng: {:.2}s",
            epoch, epochs, avg_loss, speed, dur.as_secs_f32()
        );
    }

    let model_out = "models/nnue_real_score_model.bin";
    fs::create_dir_all("models").unwrap();
    model.save_to_file(model_out).unwrap();
    println!("\n🎉 ĐÃ HUẤN LUYỆN XONG! Model GNN lưu tại: {}", model_out);
}

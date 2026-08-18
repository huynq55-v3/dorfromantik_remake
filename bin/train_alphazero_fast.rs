use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use rayon::prelude::*;
use dorfromantik_remake::alphazero::{
    AlphaZeroPipeline, AlphaZeroTrainerConfig, GameMatchRecord, MaxScoreStateRecord,
};
use dorfromantik_remake::gpu_engine::GpuEngine;
use dorfromantik_remake::gpu_nn::GpuNNExecutor;
use dorfromantik_remake::mcts::MCTSConfig;
use dorfromantik_remake::nn::HexGNNModel;

fn load_monthly_game_config() -> (i32, usize, usize) {
    let mut seed = -2093096630;
    let mut initial_stack = 10;
    let mut tile_limit = 100;

    if let Ok(content) = fs::read_to_string("monthly_game_info.txt") {
        for line in content.lines() {
            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                let val = val.trim();
                match key {
                    "REAL_TILE_SEED" => {
                        if let Ok(v) = val.parse::<i32>() {
                            seed = v;
                        }
                    }
                    "ACTIVE_TileStackHeight" => {
                        if let Ok(v) = val.parse::<usize>() {
                            initial_stack = v;
                        }
                    }
                    "ACTIVE_TileLimit" => {
                        if let Ok(v) = val.parse::<usize>() {
                            tile_limit = v;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    (seed, initial_stack, tile_limit)
}

fn find_latest_iter_model(model_dir: &str) -> Option<(usize, String)> {
    let mut max_iter = 0usize;
    let mut best_path: Option<String> = None;

    let entries = fs::read_dir(model_dir).ok()?;
    for entry in entries.flatten() {
        let filename = entry.file_name().to_string_lossy().to_string();
        if let Some(rest) = filename.strip_prefix("alphazero_iter_") {
            if let Some(num_str) = rest.strip_suffix(".bin") {
                if let Ok(n) = num_str.parse::<usize>() {
                    if n > max_iter {
                        max_iter = n;
                        best_path = Some(entry.path().to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    best_path.map(|p| (max_iter, p))
}

/// Huấn luyện Gradient với Disjoint Batched Backpropagation cực nhanh
fn fast_train_step(pipeline: &mut AlphaZeroPipeline) -> (f32, f32, f32) {
    let buf_len = pipeline.replay_buffer.len();
    let warmup_threshold = (pipeline.replay_buffer.capacity as f32 * 0.20) as usize;
    if buf_len < warmup_threshold {
        println!(
            "[Train Fast] Warm-up: buffer {}/{} sample (cần ≥ {}) — chưa train, tiếp tục self-play tích lũy.",
            buf_len, pipeline.replay_buffer.capacity, warmup_threshold
        );
        return (0.0, 0.0, 0.0);
    }

    let target_samples = (pipeline.last_new_samples * 3).max(25_000);
    let m = target_samples.min(buf_len);
    let num_batches = (m / pipeline.config.batch_size).max(1);
    let total_epochs = pipeline.config.train_epochs_per_iter;
    println!(
        "[Train Fast] Bắt đầu: {} epochs × {} batches (batch_size={}) | train trên {} samples (buffer {}/{}) với Disjoint Batched Backprop...",
        total_epochs, num_batches, pipeline.config.batch_size, m, buf_len, pipeline.replay_buffer.capacity
    );

    let mut total_policy_loss = 0.0f32;
    let mut total_value_loss = 0.0f32;
    let mut step_count = 0;

    // Kích thước sub-batch cho mỗi luồng tính toán ma trận gộp
    let chunk_size = 64; 

    for epoch in 0..total_epochs {
        let epoch_indices = pipeline.replay_buffer.sample_prioritized_unique_indices(m);
        let epoch_batches = if epoch_indices.is_empty() {
            0
        } else {
            (epoch_indices.len() / pipeline.config.batch_size)
                .min(num_batches)
                .max(1)
        };

        use std::io::Write;
        print!("[Train Fast Epoch {}/{}] ", epoch + 1, total_epochs);
        let _ = std::io::stdout().flush();

        for batch in 0..epoch_batches {
            let start = batch * pipeline.config.batch_size;
            let end = ((batch + 1) * pipeline.config.batch_size).min(epoch_indices.len());
            let indices = epoch_indices[start..end].to_vec();
            if indices.is_empty() {
                continue;
            }

            let model_ref = &pipeline.model;
            let val_coeff = pipeline.config.value_loss_coeff;
            let buffer_ref = &pipeline.replay_buffer.buffer;

            // Chia batch_size thành các chunk để các luồng Rayon chạy Disjoint Graph Backprop theo khối
            let chunks: Vec<Vec<usize>> = indices.chunks(chunk_size).map(|c| c.to_vec()).collect();

            let (mb_grads, (mb_pi_loss, mb_val_loss)) = chunks
                .into_par_iter()
                .map(|chunk_indices| {
                    let chunk_samples: Vec<&dorfromantik_remake::alphazero::AlphaZeroSample> =
                        chunk_indices.iter().map(|&idx| &buffer_ref[idx]).collect();

                    let mut chunk_grad = HexGNNModel::new_zero();
                    let (pi_l, val_l) = model_ref.backward_accumulate_batch(
                        &chunk_samples,
                        val_coeff,
                        &mut chunk_grad,
                    );
                    (chunk_grad, (pi_l, val_l))
                })
                .reduce(
                    || (HexGNNModel::new_zero(), (0.0f32, 0.0f32)),
                    |(mut g1, (pi1, v1)), (g2, (pi2, v2))| {
                        g1.add_assign(&g2);
                        (g1, (pi1 + pi2, v1 + v2))
                    },
                );

            let mb_len = pipeline.config.batch_size as f32;
            let mut scaled_grads = mb_grads;
            scaled_grads.scale_assign(1.0 / mb_len);
            scaled_grads.clip_grad_norm(1.0);

            pipeline.model.update_weights_adam(&scaled_grads, pipeline.config.lr);

            total_policy_loss += mb_pi_loss / mb_len;
            total_value_loss += mb_val_loss / mb_len;
            step_count += 1;

            if (batch + 1) % 4 == 0 || (batch + 1) == epoch_batches {
                print!("{}/{} ", batch + 1, epoch_batches);
                let _ = std::io::stdout().flush();
            }
        }
        println!();
    }

    if step_count > 0 {
        let avg_pi = total_policy_loss / step_count as f32;
        let avg_val = total_value_loss / step_count as f32;
        (avg_pi + avg_val * pipeline.config.value_loss_coeff, avg_pi, avg_val)
    } else {
        (0.0, 0.0, 0.0)
    }
}

fn main() {
    println!("============================================================");
    println!("=== DORFROMANTIK ALPHAZERO HIGH-PERFORMANCE TRAINER ===");
    println!("=== (GPU Inference Self-Play + Batched Graph Backprop) ===");
    println!("============================================================");

    let gpu_engine = GpuEngine::new();
    if let Some(ref g) = gpu_engine {
        println!("[GPU Engine] Đã phát hiện và khởi tạo GPU: {}", g.device_name);
    } else {
        println!("[GPU Engine] Không phát hiện GPU phù hợp, sử dụng CPU fallback!");
    }

    let (target_seed, initial_stack, tile_limit) = load_monthly_game_config();
    let parallel_envs = 512;

    let args: Vec<String> = std::env::args().collect();
    let n_simulations = if args.len() > 1 {
        args[1].parse::<usize>().unwrap_or(400)
    } else {
        400
    };

    let replay_buffer_capacity = args
        .get(2)
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&v| v > 0);

    let train_epochs = args
        .get(3)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(5);

    let iter_max = args
        .get(4)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(usize::MAX);

    let explore_by_entropy = args
        .get(5)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1)
        != 0;

    let lr = 0.0003;

    let config = AlphaZeroTrainerConfig {
        lr,
        gamma: 0.995,
        value_loss_coeff: 0.5,
        batch_size: 1024,
        train_epochs_per_iter: train_epochs,
        mcts_config: MCTSConfig {
            c_puct: 1.5,
            gamma: 0.995,
            n_simulations,
            dirichlet_alpha: 0.3,
            dirichlet_eps: 0.25,
            explore_by_entropy,
            temp_high: 1.0,
            temp_low: 0.2,
        },
        num_parallel_envs: parallel_envs,
        target_seed,
        initial_stack,
        tile_limit,
        replay_buffer_capacity,
    };

    let model_dir = "models";
    fs::create_dir_all(model_dir).unwrap();
    let latest_model_path = format!("{}/alphazero_latest.bin", model_dir);
    let best_model_path = format!("{}/alphazero_best.bin", model_dir);
    let buffer_path = format!("{}/alphazero_buffer.bin", model_dir);
    let best_game_path = format!("{}/best_game_record.json", model_dir);
    let max_score_states_path = format!("{}/max_score_states.json", model_dir);

    let mut pipeline = AlphaZeroPipeline::new(config.clone());
    let mut start_iter = 0;
    let mut all_time_best_match_score = 0;

    if let Some((latest_iter, ckpt_path)) = find_latest_iter_model(model_dir) {
        start_iter = latest_iter;
        println!(
            "[Checkpoint] Tìm thấy checkpoint iteration cao nhất #{}: `{}`",
            latest_iter, ckpt_path
        );
        if let Ok(loaded_model) = HexGNNModel::load_from_file(&ckpt_path) {
            println!(
                "[Checkpoint] SUCCESS: Đã nạp thành công model HexGNN (Step count = {})!",
                loaded_model.step_count
            );
            pipeline.model = loaded_model;
        }
    } else if Path::new(&latest_model_path).exists() {
        if let Ok(loaded_model) = HexGNNModel::load_from_file(&latest_model_path) {
            pipeline.model = loaded_model;
        }
    }

    if Path::new(&best_game_path).exists() {
        if let Ok(content) = fs::read_to_string(&best_game_path) {
            if let Ok(record) = serde_json::from_str::<GameMatchRecord>(&content) {
                all_time_best_match_score = record.total_score;
                println!(
                    "[Match Record] Đã khôi phục kỷ lục tốt nhất mọi thời đại: {} điểm (Placed {} tiles)",
                    all_time_best_match_score, record.total_placed
                );
            }
        }
    }

    let human_states_path = format!("{}/human_expert_states.json", model_dir);
    let mut combined_states: Vec<MaxScoreStateRecord> = Vec::new();

    if Path::new(&max_score_states_path).exists() {
        if let Ok(content) = fs::read_to_string(&max_score_states_path) {
            if let Ok(states) = serde_json::from_str::<Vec<MaxScoreStateRecord>>(&content) {
                combined_states.extend(states);
            }
        }
    }
    if Path::new(&human_states_path).exists() {
        if let Ok(content) = fs::read_to_string(&human_states_path) {
            if let Ok(states) = serde_json::from_str::<Vec<MaxScoreStateRecord>>(&content) {
                combined_states.extend(states);
            }
        }
    }

    if !combined_states.is_empty() {
        combined_states.sort_unstable_by(|a, b| b.q_value.partial_cmp(&a.q_value).unwrap_or(std::cmp::Ordering::Equal));
        if combined_states.len() > 2000 {
            combined_states.truncate(2000);
        }
        pipeline.max_score_states = combined_states;
        println!(
            "[MaxScoreStates] Đã nạp và gộp top {} states xuất phát cho 50% envs (AI + Human).",
            pipeline.max_score_states.len()
        );
    }

    if Path::new(&buffer_path).exists() {
        println!("[Replay Buffer] Đang nạp buffer từ `{}`...", buffer_path);
        let buf_start = Instant::now();
        match pipeline.replay_buffer.load_from_file(&buffer_path) {
            Ok(_) => {
                println!(
                    "[Replay Buffer] SUCCESS: Đã nạp {} samples trong {:.2}s!",
                    pipeline.replay_buffer.len(),
                    buf_start.elapsed().as_secs_f32()
                );
            }
            Err(e) => {
                println!("[Replay Buffer] CẢNH BÁO: Lỗi đọc buffer ({:?}), bắt đầu buffer trống.", e);
            }
        }
    }

    let gpu_executor = gpu_engine.as_ref().map(|g| {
        println!("[GPU NN] Khởi tạo GpuNNExecutor với persistent weight buffers...");
        let exec = GpuNNExecutor::new(
            Arc::clone(&g.device),
            Arc::clone(&g.queue),
            &pipeline.model,
        );
        println!("[GPU NN] Đã upload 32 weights matrices lên GPU VRAM!");
        exec
    });

    let mut iteration = start_iter;

    loop {
        iteration += 1;
        if iteration > iter_max {
            println!(
                "[Done] Đã huấn luyện xong tới iteration max ({}), kết thúc chương trình.",
                iter_max
            );
            break;
        }
        let iter_start = Instant::now();
        println!(
            "--- [Fast Iteration #{}] (Buffer: {}/{} samples) ---",
            iteration,
            pipeline.replay_buffer.len(),
            pipeline.replay_buffer.capacity
        );

        if let Some(ref exec) = gpu_executor {
            exec.sync_weights(&pipeline.model);
        }

        if Path::new(&human_states_path).exists() {
            if let Ok(content) = fs::read_to_string(&human_states_path) {
                if let Ok(human_states) = serde_json::from_str::<Vec<MaxScoreStateRecord>>(&content) {
                    for h_st in human_states {
                        pipeline.add_high_q_state(h_st.q_value, h_st.remaining_tiles, &h_st.moves);
                    }
                }
            }
        }

        let refresh_start = Instant::now();
        let n_refreshed = pipeline.refresh_max_score_state_q_values(gpu_executor.as_ref(), 0);
        let refresh_dur = refresh_start.elapsed();
        println!(
            "[Refresh Q] Cập nhật Q-value {}/{} states qua Model Value Head trong {:.2}s",
            n_refreshed,
            pipeline.max_score_states.len(),
            refresh_dur.as_secs_f32()
        );

        let self_play_start = Instant::now();
        let (avg_score, max_score, avg_placed, best_self_play_record) =
            pipeline.collect_self_play_data_batch(gpu_executor.as_ref());
        let self_play_dur = self_play_start.elapsed();

        println!(
            "[Self-Play GPU] Score TB: {:.1} | Max Score: {} | Placed TB: {} | Thời gian: {:.2}s",
            avg_score,
            max_score,
            avg_placed,
            self_play_dur.as_secs_f32()
        );

        if let Some(record) = best_self_play_record {
            if record.total_score > all_time_best_match_score {
                all_time_best_match_score = record.total_score;
                println!(
                    "🔥 KHÔI PHỤC/TẠO KỶ LỤC MỚI TRONG SELF-PLAY: {} ĐIỂM (Placed {} tiles)! Đang lưu json & best model...",
                    all_time_best_match_score, record.total_placed
                );
                if let Ok(json_str) = serde_json::to_string_pretty(&record) {
                    let _ = fs::write(&best_game_path, json_str);
                }
                let _ = pipeline.model.save_to_file(&best_model_path);
            }
        }

        let train_start = Instant::now();
        let (total_loss, pi_loss, val_loss) = fast_train_step(&mut pipeline);
        let train_dur = train_start.elapsed();

        println!(
            "[Train Fast] Policy Loss: {:.4} | Value Loss: {:.4} | Total Loss: {:.4} | Thời gian: {:.2}s",
            pi_loss,
            val_loss,
            total_loss,
            train_dur.as_secs_f32()
        );

        if let Err(e) = pipeline.model.save_to_file(&latest_model_path) {
            println!("[Save Error] Không thể lưu latest model: {:?}", e);
        }

        let iter_model_path = format!("{}/alphazero_iter_{}.bin", model_dir, iteration);
        if let Err(e) = pipeline.model.save_to_file(&iter_model_path) {
            println!("[Save Error] Không thể lưu iter model: {:?}", e);
        }

        let save_buf_start = Instant::now();
        if let Err(e) = pipeline.replay_buffer.save_to_file(&buffer_path) {
            println!("[Save Error] Không thể lưu replay buffer: {:?}", e);
        } else {
            println!(
                "[Replay Buffer] Đã lưu {} samples vào `{}` trong {:.2}s",
                pipeline.replay_buffer.len(),
                buffer_path,
                save_buf_start.elapsed().as_secs_f32()
            );
        }

        if let Ok(json_str) = serde_json::to_string_pretty(&pipeline.max_score_states) {
            let _ = fs::write(&max_score_states_path, json_str);
        }

        println!(
            "[Fast Iteration #{}] Hoàn thành trong {:.2}s\n",
            iteration,
            iter_start.elapsed().as_secs_f32()
        );
    }
}

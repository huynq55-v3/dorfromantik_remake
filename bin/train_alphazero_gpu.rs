use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
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

/// Quét thư mục models để tìm file checkpoint `alphazero_iter_<n>.bin` có số iteration lớn nhất.
/// Trả về (iteration, đường dẫn file) nếu có, ngược lại None.
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

fn main() {
    println!("============================================================");
    println!("=== DORFROMANTIK ALPHAZERO GPU TRAINER (Intel Iris Xe / Vulkan) ===");
    println!("============================================================");

    // 1. Khởi tạo GPU Context (giữ lại device/queue để dùng sau)
    let gpu_engine = GpuEngine::new();
    if let Some(ref g) = gpu_engine {
        println!("[GPU Engine] Đã phát hiện và khởi tạo GPU: {}", g.device_name);
    } else {
        println!("[GPU Engine] Không phát hiện GPU phù hợp, sử dụng CPU fallback!");
    }

    let (target_seed, initial_stack, tile_limit) = load_monthly_game_config();
    let parallel_envs = 512;

    // Đọc số simulations từ tham số dòng lệnh nếu có (mặc định 400)
    let args: Vec<String> = std::env::args().collect();
    let n_simulations = if args.len() > 1 {
        args[1].parse::<usize>().unwrap_or(400)
    } else {
        400
    };

    // Đọc dung lượng replay buffer từ tham số dòng lệnh thứ 2 (optional).
    // None => tự động tính theo công thức mặc định (envs * tile_limit * 5, tối thiểu 250k).
    // Giá trị truyền vào phải > 0; nếu không hợp lệ sẽ tự fallback về None.
    let replay_buffer_capacity = args
        .get(2)
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&c| c > 0);

    // Đọc iteration tối đa (iter_max) từ tham số dòng lệnh thứ 3 (optional).
    // Mặc định usize::MAX => chạy gần như không giới hạn (không được truyền iter_max).
    let iter_max = args
        .get(3)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(usize::MAX);

    // Đọc số train epochs mỗi iteration từ tham số dòng lệnh thứ 4 (optional).
    // Mặc định 4. Giá trị phải > 0; nếu không hợp lệ sẽ fallback về 4.
    let train_epochs = args
        .get(4)
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&e| e > 0)
        .unwrap_or(4);

    // Cờ bật quyết định exploration theo entropy TRONG TỪNG TURN (tự tin → explore, bối rối → exploit).
    // Mặc định 1 (BẬT). Truyền 0 để tắt.
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
    let meta_path = format!("{}/alphazero_meta.txt", model_dir);
    let best_game_path = format!("{}/best_game_record.json", model_dir);
    let max_score_states_path = format!("{}/max_score_states.json", model_dir);

    let mut pipeline = AlphaZeroPipeline::new(config.clone());
    let mut start_iter = 0;
    let mut all_time_best_match_score = 0;

    // 2. Tìm checkpoint iteration lớn nhất hiện có (alphazero_iter_<n>.bin).
    // Nếu có, nạp model đó và TIẾP TỤC từ iteration kế tiếp (n+1).
    let mut checkpoint_path: Option<String> = None;
    if let Some((iter, path)) = find_latest_iter_model(model_dir) {
        start_iter = iter;
        checkpoint_path = Some(path);
        println!(
            "[Checkpoint] Tìm thấy model iteration lớn nhất: `{}` (iteration = {})",
            checkpoint_path.as_deref().unwrap_or(""),
            iter
        );
    }

    // Nạp checkpoint model hiện tại nếu có (Tái sử dụng weights cũ, không train lại từ đầu!)
    if let Some(ref ckpt) = checkpoint_path {
        println!("[Checkpoint] Đang nạp model từ `{}`...", ckpt);
        match HexGNNModel::load_from_file(ckpt) {
            Ok(loaded_model) => {
                println!(
                    "[Checkpoint] SUCCESS: Đã nạp thành công model HexGNN (Step count = {})!",
                    loaded_model.step_count
                );
                pipeline.model = loaded_model;
            }
            Err(e) => {
                println!("[Checkpoint] CẢNH BÁO: Lỗi đọc model cũ ({:?}), tạo model mới.", e);
            }
        }
    } else if Path::new(&latest_model_path).exists() {
        println!("[Checkpoint] Đang nạp model cũ từ `{}`...", latest_model_path);
        if let Ok(loaded_model) = HexGNNModel::load_from_file(&latest_model_path) {
            println!(
                "[Checkpoint] SUCCESS: Đã nạp thành công model (Step count = {})!",
                loaded_model.step_count
            );
            pipeline.model = loaded_model;
        }
    } else if Path::new(&best_model_path).exists() {
        println!("[Checkpoint] Đang nạp best model từ `{}`...", best_model_path);
        if let Ok(loaded_model) = HexGNNModel::load_from_file(&best_model_path) {
            println!(
                "[Checkpoint] SUCCESS: Đã nạp thành công best model (Step count = {})!",
                loaded_model.step_count
            );
            pipeline.model = loaded_model;
        }
    }

    // 3. Khôi phục kỷ lục ván chơi tốt nhất (Best Match Record)
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

    // 3.5. Khôi phục và gộp danh sách max-score states (AI: max 2000, Human: max 1000 -> lấy top 2000)
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
                println!("[HumanExpertStates] Đã tìm thấy {} states do con người chơi từ `{}`.", states.len(), human_states_path);
                combined_states.extend(states);
            }
        }
    }

    if !combined_states.is_empty() {
        // Sắp xếp giảm dần theo Q-value và lấy top 2000
        combined_states.sort_unstable_by(|a, b| b.q_value.partial_cmp(&a.q_value).unwrap_or(std::cmp::Ordering::Equal));
        if combined_states.len() > 2000 {
            combined_states.truncate(2000);
        }
        pipeline.max_score_states = combined_states;
        println!(
            "[MaxScoreStates] Đã nạp và gộp top {} states xuất phát cho 80% envs (AI + Human).",
            pipeline.max_score_states.len()
        );
    }

    // 4. Khôi phục Replay Buffer nếu có
    if Path::new(&buffer_path).exists() {
        println!("[Buffer] Đang nạp Replay Buffer từ `{}`...", buffer_path);
        match pipeline.replay_buffer.load_from_file(&buffer_path) {
            Ok(count) => {
                println!("[Buffer] SUCCESS: Đã nạp {} samples từ file buffer!", count);
                let merged = pipeline.replay_buffer.merge_symmetric_actions();
                if merged > 0 {
                    println!("[Buffer] Đã gộp và chuẩn hóa {} actions đẳng cấu (đối xứng xoay) trong buffer cũ!", merged);
                }
                pipeline.replay_buffer.migrate_action_features();
            }
            Err(e) => {
                println!("[Buffer] CẢNH BÁO: Không nạp được buffer cũ ({:?})", e);
            }
        }
    }

    // 5. Khôi phục Metadata (Iteration, All-time Best Match Score)
    if Path::new(&meta_path).exists() {
        if let Ok(content) = fs::read_to_string(&meta_path) {
            for line in content.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    match k.trim() {
                        // Khi có file alphazero_iter_<n>.bin thì ưu tiên số đó (đã gán ở trên).
                        // Meta chỉ giúp khi chưa dùng file iteration (fallback alphazero_latest/best).
                        "iteration" if start_iter == 0 => start_iter = v.trim().parse::<usize>().unwrap_or(0),
                        "all_time_best_match_score" if all_time_best_match_score == 0 => {
                            all_time_best_match_score = v.trim().parse::<usize>().unwrap_or(0)
                        }
                        _ => {}
                    }
                }
            }
            println!(
                "[Meta] Tiếp tục từ Iteration #{}, Kỷ Lục Hiện Tại = {} điểm",
                start_iter + 1,
                all_time_best_match_score
            );
        }
    }

    println!("\n>>> CẤU HÌNH GPU HUẤN LUYỆN ALPHAZERO <<<");
    println!(" - Seed Mục Tiêu: {}", target_seed);
    println!(" - Tile Stack / Limit: {} / {}", initial_stack, tile_limit);
    println!(" - Số Môi Trường Song Song (Envs): {}", parallel_envs);
    println!(" - Số MCTS Simulations / Turn: {}", n_simulations);
    println!(" - Batch Size: {}", config.batch_size);
    println!(" - Train Epochs / Iter: {}", config.train_epochs_per_iter);
    let buffer_capacity = config.replay_buffer_capacity.unwrap_or(200_000);
    println!(" - Replay Buffer Capacity: {} samples", buffer_capacity);
    println!(" - Warm-up (train sau ≥ 20%): ≥ {} samples", ((buffer_capacity as f32) * 0.20) as usize);
    println!(" - Dirichlet Alpha: {} | Dirichlet Eps: {}", config.mcts_config.dirichlet_alpha, config.mcts_config.dirichlet_eps);
    println!(" - Learning Rate: {}", config.lr);
    println!(" - Exploration theo entropy từng turn: {}", if config.mcts_config.explore_by_entropy { "BẬT" } else { "TẮT" });
    if iter_max != usize::MAX {
        println!(" - Iteration Max: {}", iter_max);
    }
    println!(" - Bắt đầu từ Iteration #{} (tiếp tục từ checkpoint)", start_iter + 1);
    println!("============================================================\n");

    // 6. Tạo GPU NN Executor SAU KHI load checkpoint (dùng đúng weights đã train)
    let gpu_executor = gpu_engine.as_ref().map(|g| {
        println!("[GPU NN] Khởi tạo GpuNNExecutor với persistent weight buffers...");
        let exec = GpuNNExecutor::new(
            Arc::clone(&g.device),
            Arc::clone(&g.queue),
            &pipeline.model,
        );
        println!("[GPU NN] Đã upload {} weights matrices lên GPU VRAM!", 32);
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
            "--- [Iteration #{}] (Buffer: {}/{} samples) ---",
            iteration,
            pipeline.replay_buffer.len(),
            pipeline.replay_buffer.capacity
        );

        // A. (deprecated) EvalQueue không còn dùng trong batch mode mới
        // let eval_queue = GpuEvalQueue::new(pipeline.model.clone(), parallel_envs * 4, 100);

        // Sync weights vào GPU executor trước khi tự chơi
        if let Some(ref exec) = gpu_executor {
            exec.sync_weights(&pipeline.model);
        }

        // Tự động load và gộp human_expert_states.json vào pool trước khi Refresh Q
        if Path::new(&human_states_path).exists() {
            if let Ok(content) = fs::read_to_string(&human_states_path) {
                if let Ok(human_states) = serde_json::from_str::<Vec<MaxScoreStateRecord>>(&content) {
                    for h_st in human_states {
                        pipeline.add_high_q_state(h_st.q_value, h_st.remaining_tiles, &h_st.moves);
                    }
                }
            }
        }

        // A.5. Refresh Q-value của toàn bộ max-score states bằng Model Value Head (V_model) trực tiếp
        let refresh_start = Instant::now();
        let n_refreshed = pipeline.refresh_max_score_state_q_values(gpu_executor.as_ref(), 0);
        let refresh_dur = refresh_start.elapsed();
        println!(
            "[Refresh Q] Cập nhật Q-value {}/{} states qua Model Value Head trong {:.2}s",
            n_refreshed,
            pipeline.max_score_states.len(),
            refresh_dur.as_secs_f32()
        );

        // B. Tự chơi (Vectorized Batch MCTS + GPU Neural Network Inference)
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

        // Kiểm tra xem ván tự chơi có phá kỷ lục match score hay không
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

        // C. Train model với Adam Optimizer
        let train_start = Instant::now();
        let (total_loss, pi_loss, val_loss) = pipeline.train_step();
        let train_dur = train_start.elapsed();

        println!(
            "[Train GPU] Policy Loss: {:.4} | Value Loss: {:.4} | Total Loss: {:.4} | Thời gian: {:.2}s",
            pi_loss,
            val_loss,
            total_loss,
            train_dur.as_secs_f32()
        );

        // D. Lưu checkpoint model & buffer định kỳ
        if let Err(e) = pipeline.model.save_to_file(&latest_model_path) {
            println!("[Save Error] Không thể lưu latest model: {:?}", e);
        }

        // Lưu model theo iteration: alphazero_iter_<n>.bin
        let iter_model_path = format!("{}/alphazero_iter_{}.bin", model_dir, iteration);
        if let Err(e) = pipeline.model.save_to_file(&iter_model_path) {
            println!("[Save Error] Không thể lưu model iteration {}: {:?}", iteration, e);
        } else {
            println!("[Checkpoint] Đã lưu model iteration {} vào `{}`", iteration, iter_model_path);
        }

        if let Err(e) = pipeline.replay_buffer.save_to_file(&buffer_path) {
            println!("[Save Error] Không thể lưu replay buffer: {:?}", e);
        }

        // Lưu danh sách max-score states (80% envs khởi động lại từ vị thế tốt ở iter sau)
        if let Ok(json_str) = serde_json::to_string_pretty(&pipeline.max_score_states) {
            if let Err(e) = fs::write(&max_score_states_path, json_str) {
                println!("[Save Error] Không thể lưu max_score_states: {:?}", e);
            }
        }

        // Ghi metadata file (iteration, kỷ lục)
        let meta_content = format!(
            "iteration={}\nall_time_best_match_score={}\n",
            iteration, all_time_best_match_score
        );
        let _ = fs::write(&meta_path, meta_content);

        let total_iter_dur = iter_start.elapsed();
        println!(
            "[Iteration #{}] Hoàn thành trong {:.2}s\n",
            iteration,
            total_iter_dur.as_secs_f32()
        );
    }
}

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use dorfromantik_remake::alphazero::{
    evaluate_alphazero_agent_gpu, AlphaZeroPipeline, AlphaZeroTrainerConfig, GameMatchRecord,
};
use dorfromantik_remake::gpu_engine::{GpuEngine, GpuEvalQueue};
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
    let parallel_envs = 16;

    // Đọc số simulations từ tham số dòng lệnh nếu có (mặc định 400)
    let args: Vec<String> = std::env::args().collect();
    let n_simulations = if args.len() > 1 {
        args[1].parse::<usize>().unwrap_or(400)
    } else {
        400
    };

    let lr = 0.0003;

    let config = AlphaZeroTrainerConfig {
        lr,
        gamma: 0.99,
        value_loss_coeff: 0.5,
        batch_size: 128,
        train_epochs_per_iter: 4,
        mcts_config: MCTSConfig {
            c_puct: 1.5,
            gamma: 0.99,
            n_simulations,
            dirichlet_alpha: 0.3,
            dirichlet_eps: 0.25,
        },
        temp_threshold_moves: 12,
        num_parallel_envs: parallel_envs,
        target_seed,
        initial_stack,
        tile_limit,
    };

    let model_dir = "models";
    fs::create_dir_all(model_dir).unwrap();
    let latest_model_path = format!("{}/alphazero_latest.bin", model_dir);
    let best_model_path = format!("{}/alphazero_best.bin", model_dir);
    let buffer_path = format!("{}/alphazero_buffer.bin", model_dir);
    let meta_path = format!("{}/alphazero_meta.txt", model_dir);
    let best_game_path = format!("{}/best_game_record.json", model_dir);

    let mut pipeline = AlphaZeroPipeline::new(config.clone());
    let mut start_iter = 0;
    let mut best_eval_score = 0;
    let mut all_time_best_match_score = 0;

    // 2. Nạp checkpoint model hiện tại nếu có (Tái sử dụng weights cũ, không train lại từ đầu!)
    if Path::new(&latest_model_path).exists() {
        println!("[Checkpoint] Đang nạp model cũ từ `{}`...", latest_model_path);
        match HexGNNModel::load_from_file(&latest_model_path) {
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

    // 4. Khôi phục Replay Buffer nếu có
    if Path::new(&buffer_path).exists() {
        println!("[Buffer] Đang nạp Replay Buffer từ `{}`...", buffer_path);
        match pipeline.replay_buffer.load_from_file(&buffer_path) {
            Ok(count) => {
                println!("[Buffer] SUCCESS: Đã nạp {} samples từ file buffer!", count);
            }
            Err(e) => {
                println!("[Buffer] CẢNH BÁO: Không nạp được buffer cũ ({:?})", e);
            }
        }
    }

    // 5. Khôi phục Metadata (Iteration, Best Eval Score)
    if Path::new(&meta_path).exists() {
        if let Ok(content) = fs::read_to_string(&meta_path) {
            for line in content.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    match k.trim() {
                        "iteration" => start_iter = v.trim().parse::<usize>().unwrap_or(0),
                        "best_eval_score" => best_eval_score = v.trim().parse::<usize>().unwrap_or(0),
                        _ => {}
                    }
                }
            }
            println!(
                "[Meta] Tiếp tục từ Iteration #{}, Best Eval Score = {}",
                start_iter + 1,
                best_eval_score
            );
        }
    }

    println!("\n>>> CẤU HÌNH GPU HUẤN LUYỆN ALPHAZERO <<<");
    println!(" - Seed Mục Tiêu: {}", target_seed);
    println!(" - Tile Stack / Limit: {} / {}", initial_stack, tile_limit);
    println!(" - Số Môi Trường Song Song (Envs): {}", parallel_envs);
    println!(" - Số MCTS Simulations / Turn: {}", n_simulations);
    println!(" - Batch Size: {}", config.batch_size);
    println!(" - Learning Rate: {}", config.lr);
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
                    "🔥 KHÔI PHỤC/TẠO KỶ LỤC MỚI TRONG SELF-PLAY: {} ĐIỂM (Placed {} tiles)! Đang lưu json...",
                    all_time_best_match_score, record.total_placed
                );
                if let Ok(json_str) = serde_json::to_string_pretty(&record) {
                    let _ = fs::write(&best_game_path, json_str);
                }
            }
        }

        // C. Train model với Adam Optimizer
        let train_start = Instant::now();
        let (pi_loss, val_loss, total_loss) = pipeline.train_step();
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

        if let Err(e) = pipeline.replay_buffer.save_to_file(&buffer_path) {
            println!("[Save Error] Không thể lưu replay buffer: {:?}", e);
        }

        // E. Đánh giá (Evaluation GPU) mỗi 5 iterations
        if iteration % 5 == 0 {
            let eval_start = Instant::now();
            println!("[Eval GPU] Đang chạy đánh giá Greedy Agent trên target seed {}...", target_seed);
            // Tạo eval_queue tạm thời chỉ cho lần eval này
            let eval_queue = dorfromantik_remake::gpu_engine::GpuEvalQueue::new(
                pipeline.model.clone(), parallel_envs * 4, 100,
            );
            let (eval_score, eval_placed, eval_record) = evaluate_alphazero_agent_gpu(
                target_seed,
                initial_stack,
                tile_limit,
                &eval_queue.tx,
                &config.mcts_config,
            );
            let eval_dur = eval_start.elapsed();


            println!(
                "[Eval Result] Score: {} | Placed: {} tiles | Thời gian: {:.2}s",
                eval_score,
                eval_placed,
                eval_dur.as_secs_f32()
            );

            if eval_score > best_eval_score {
                println!(
                    "🏆 KỶ LỤC DỰ ĐOÁN MỚI (Best Eval Score): {} -> {}! Đang lưu best model...",
                    best_eval_score, eval_score
                );
                best_eval_score = eval_score;
                let _ = pipeline.model.save_to_file(&best_model_path);
            }

            if eval_score > all_time_best_match_score {
                all_time_best_match_score = eval_score;
                println!(
                    "🔥 KỶ LỤC MỚI TRONG EVALUATION: {} ĐIỂM! Đang lưu json...",
                    all_time_best_match_score
                );
                if let Ok(json_str) = serde_json::to_string_pretty(&eval_record) {
                    let _ = fs::write(&best_game_path, json_str);
                }
            }

            // Ghi metadata file
            let meta_content = format!(
                "iteration={}\nbest_eval_score={}\nall_time_best_match_score={}\n",
                iteration, best_eval_score, all_time_best_match_score
            );
            let _ = fs::write(&meta_path, meta_content);
        }

        let total_iter_dur = iter_start.elapsed();
        println!(
            "[Iteration #{}] Hoàn thành trong {:.2}s\n",
            iteration,
            total_iter_dur.as_secs_f32()
        );
    }
}

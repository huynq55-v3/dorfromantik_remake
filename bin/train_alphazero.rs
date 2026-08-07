use std::fs;
use std::path::Path;
use std::time::Instant;
use dorfromantik_remake::alphazero::{
    evaluate_alphazero_agent, AlphaZeroPipeline, AlphaZeroTrainerConfig, GameMatchRecord,
};
use dorfromantik_remake::mcts::MCTSConfig;

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
    println!("=== DORFROMANTIK ALPHAZERO (MCTS + GNN) CONTINUOUS TRAIN ===");
    println!("============================================================");

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

    // Tự động kiểm tra và khôi phục kỷ lục ván chơi tốt nhất (Best Match Record)
    if Path::new(&best_game_path).exists() {
        if let Ok(content) = fs::read_to_string(&best_game_path) {
            if let Ok(record) = serde_json::from_str::<GameMatchRecord>(&content) {
                all_time_best_match_score = record.total_score;
                println!(
                    ">>> [LOADED BEST MATCH] Record: {} pts | Placed: {} tiles | Seed: {}",
                    record.total_score, record.total_placed, record.seed
                );
            }
        }
    }

    // Tự động kiểm tra và khôi phục từ Checkpoint Model cũ nếu có
    if Path::new(&latest_model_path).exists() {
        print!("Loading checkpoint from {} ... ", latest_model_path);
        if pipeline.load_checkpoint(&latest_model_path, &buffer_path).is_ok() {
            if let Ok(meta_content) = fs::read_to_string(&meta_path) {
                let parts: Vec<&str> = meta_content.trim().split(',').collect();
                if parts.len() >= 2 {
                    start_iter = parts[0].parse().unwrap_or(0);
                    best_eval_score = parts[1].parse().unwrap_or(0);
                }
            }
            println!("SUCCESS!");
            println!(
                ">>> RESUMED from Iter {:04} | Buffer: {:>5} samples | Best Eval Score: {}",
                start_iter,
                pipeline.buffer_len(),
                best_eval_score
            );
        } else {
            println!("FAILED. Starting from scratch.");
        }
    }

    println!("Target Seed        : {}", target_seed);
    println!("Initial Stack      : {} tiles", initial_stack);
    println!("Tile Limit / Game  : {}", tile_limit);
    println!("MCTS Simulations   : {}", n_simulations);
    println!("Parallel Envs      : {} threads", parallel_envs);
    println!("Learning Rate      : {}", lr);
    println!("Replay Buffer Cap  : 50,000 samples");
    // Đọc và hiển thị quest probability multiplier để xác nhận đã load đúng từ file
    {
        let qpm = dorfromantik_remake::generator::TileGenerator::new(target_seed).global_quest_probability_multiplier;
        println!("Quest Prob Mult    : {:.2}x  (from monthly_game_info.txt)", qpm);
    }
    println!("All-time Match Max : {} pts", all_time_best_match_score);
    println!("Training Mode      : Continuous Infinite Loop (Ctrl+C to stop anytime)");
    println!("------------------------------------------------------------");

    // let server_state = dorfromantik_remake::server::SharedTrainingState::new();
    // let _server_thread = dorfromantik_remake::server::start_server(3030, server_state.clone());

    // // Khởi tạo trạng thái ban đầu cho server
    // {
    //     let mut s = server_state.status.lock().unwrap();
    //     s.iter = start_iter;
    //     s.buffer_len = pipeline.buffer_len();
    //     s.eval_score = best_eval_score;
    //     s.all_time_best_score = all_time_best_match_score;
    //     s.n_simulations = n_simulations;
    // }
    // *server_state.landscape.lock().unwrap() = dorfromantik_remake::server::compute_landscape_points(&pipeline.replay_buffer, 250);

    for iter in (start_iter + 1).. {
        // Kiểm tra xem người dùng có bấm PAUSE từ Web UI không
        // while server_state.is_paused.load(std::sync::atomic::Ordering::Relaxed) {
        //     std::thread::sleep(std::time::Duration::from_millis(500));
        // }

        let t_start = Instant::now();

        // 1. Data Generation via MCTS Self-Play
        let t_gen_start = Instant::now();
        let (self_play_avg_score, self_play_max_score, self_play_avg_placed, sp_best_match) =
            pipeline.collect_self_play_data();
        let t_gen = t_gen_start.elapsed();

        // 2. Training on Replay Buffer (Mini-batches with Adam Optimizer)
        let t_train_start = Instant::now();
        let (loss, pi_loss, val_loss) = pipeline.train_step();
        let t_train = t_train_start.elapsed();

        // 3. Evaluation on Target Seed
        let (eval_score, eval_placed, eval_match) =
            evaluate_alphazero_agent(target_seed, initial_stack, tile_limit, &pipeline.model, &config.mcts_config);

        let t_total = t_start.elapsed();

        // Kiểm tra xem có ván đấu nào (Self-Play hoặc Eval) phá vỡ kỷ lục mọi thời đại không
        let mut new_match_record_saved = false;
        if let Some(sp_record) = sp_best_match {
            if sp_record.total_score > all_time_best_match_score {
                all_time_best_match_score = sp_record.total_score;
                if let Ok(json_str) = serde_json::to_string_pretty(&sp_record) {
                    let _ = fs::write(&best_game_path, json_str);
                    new_match_record_saved = true;
                }
            }
        }
        if eval_match.total_score > all_time_best_match_score {
            all_time_best_match_score = eval_match.total_score;
            if let Ok(json_str) = serde_json::to_string_pretty(&eval_match) {
                let _ = fs::write(&best_game_path, json_str);
                new_match_record_saved = true;
            }
        }

        let is_best = eval_score > best_eval_score;
        if is_best {
            best_eval_score = eval_score;
            let _ = pipeline.model.save_to_file(&best_model_path);
        }

        // Tự động lưu Checkpoint Model + Replay Buffer + Metadata sau MỖI iteration
        let _ = pipeline.save_checkpoint(&latest_model_path, &buffer_path);
        let _ = fs::write(&meta_path, format!("{},{}", iter, best_eval_score));

        // Cập nhật trạng thái thời gian thực và tọa độ Landscape lên Web Server
        // let landscape_data = dorfromantik_remake::server::compute_landscape_points(&pipeline.replay_buffer, 250);
        // {
        //     let mut s = server_state.status.lock().unwrap();
        //     s.iter = iter;
        //     s.buffer_len = pipeline.buffer_len();
        //     s.sp_time_sec = t_gen.as_secs_f32();
        //     s.train_time_sec = t_train.as_secs_f32();
        //     s.total_time_sec = t_total.as_secs_f32();
        //     s.total_loss = loss;
        //     s.policy_loss = pi_loss;
        //     s.value_loss = val_loss;
        //     s.sp_avg_score = self_play_avg_score;
        //     s.sp_max_score = self_play_max_score;
        //     s.sp_avg_placed = self_play_avg_placed;
        //     s.eval_score = eval_score;
        //     s.eval_placed = eval_placed;
        //     s.all_time_best_score = all_time_best_match_score;
        //     s.n_simulations = n_simulations;
        // }
        // *server_state.landscape.lock().unwrap() = landscape_data;

        let flag = if is_best && new_match_record_saved {
            " [BEST EVAL & NEW MATCH RECORD!]"
        } else if new_match_record_saved {
            " [NEW ALL-TIME MATCH RECORD!]"
        } else if is_best {
            " [BEST EVAL SAVED]"
        } else {
            " [SAVED]"
        };

        println!(
            "Iter {:04} | Buf:{:>5}/50k | SP: {:>5.1?} | Tr: {:>5.1?} | Tot: {:>5.1?} | Loss: {:>6.4} (π:{:>5.3}, V:{:>5.3}) | SP Avg/Max: {:>4.0}/{:>4} (P:{:>2}) | Eval: {:>4} (P:{:>2}){}",
            iter,
            pipeline.buffer_len(),
            t_gen,
            t_train,
            t_total,
            loss,
            pi_loss,
            val_loss,
            self_play_avg_score,
            self_play_max_score,
            self_play_avg_placed,
            eval_score,
            eval_placed,
            flag
        );
    }
}

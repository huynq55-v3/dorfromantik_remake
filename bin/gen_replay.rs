use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use dorfromantik_remake::alphazero::{GameMoveRecord, GameMatchRecord};
use dorfromantik_remake::env::DorfromantikEnv;
use dorfromantik_remake::gpu_engine::GpuEngine;
use dorfromantik_remake::gpu_nn::GpuNNExecutor;
use dorfromantik_remake::mcts::{MCTSSearch, MCTSConfig};
use dorfromantik_remake::nn::HexGNNModel;

/// Đọc cấu hình monthly (seed, stack height, tile limit) từ monthly_game_info.txt
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
    // arg1 = số simulations (mặc định 400)
    // arg2 = số envs song song (mặc định 16)
    // arg3 = model path (mặc định: models/alphazero_best.bin nếu có, fallback alphazero_latest.bin)
    let args: Vec<String> = std::env::args().collect();
    let n_simulations = if args.len() > 1 {
        args[1].parse::<usize>().unwrap_or(400)
    } else {
        400
    };
    let n_envs = if args.len() > 2 {
        args[2].parse::<usize>().unwrap_or(16)
    } else {
        16
    };

    let model_path = if args.len() > 3 {
        args[3].clone()
    } else if Path::new("models/alphazero_best.bin").exists() {
        "models/alphazero_best.bin".to_string()
    } else {
        "models/alphazero_latest.bin".to_string()
    };

    if !Path::new(&model_path).exists() {
        eprintln!("Lỗi: không tìm thấy model `{}`", model_path);
        std::process::exit(1);
    }

    let model = match HexGNNModel::load_from_file(&model_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Lỗi đọc model: {:?}", e);
            std::process::exit(1);
        }
    };
    println!("Đã nạp model từ `{}` (step_count = {})", model_path, model.step_count);

    // Khởi tạo GPU context + NN executor (CPU fallback nếu không có GPU)
    let gpu_engine = GpuEngine::new();
    let gpu_executor = gpu_engine.as_ref().map(|g| {
        println!("[GPU] {}", g.device_name);
        GpuNNExecutor::new(
            Arc::clone(&g.device),
            Arc::clone(&g.queue),
            &model,
        )
    });

    let (seed, initial_stack, tile_limit) = load_monthly_game_config();
    println!(
        "Chạy Double-Buffered Batch MCTS (sims={}, parallel_envs={}) | Seed={} | Stack={} | TileLimit={} ...",
        n_simulations, n_envs, seed, initial_stack, tile_limit
    );

    let mcts_config = MCTSConfig {
        c_puct: 1.5,
        gamma: 0.995,
        n_simulations,
        dirichlet_alpha: 0.3,
        dirichlet_eps: 0.25,
        explore_by_entropy: true,
        temp_high: 1.0,
        temp_low: 0.2,
    };
    let mcts = MCTSSearch::new(mcts_config.clone());

    let mut envs: Vec<DorfromantikEnv> = (0..n_envs)
        .map(|_| DorfromantikEnv::new(seed, initial_stack, tile_limit))
        .collect();
    let mut move_records: Vec<Vec<GameMoveRecord>> = vec![Vec::new(); n_envs];
    let mut active = vec![true; n_envs];
    let mut turn_counter = 0usize;
    let t0 = Instant::now();

    while active.iter().any(|&a| a) {
        let active_indices: Vec<usize> = (0..n_envs).filter(|&i| active[i]).collect();
        if active_indices.is_empty() {
            break;
        }
        turn_counter += 1;

        let batch_results = mcts.search_batch_indexed(
            &envs,
            &active_indices,
            &model,
            gpu_executor.as_ref(),
            true, // Bật Dirichlet & Entropy giống Self-Play
            1.0,
        );

        for (k, &idx) in active_indices.iter().enumerate() {
            let (_, _, chosen_action, _, obs) = &batch_results[k];

            if obs.valid_actions.is_empty() {
                active[idx] = false;
                continue;
            }

            let prev_score = envs[idx].score_manager.total_score;
            let res = envs[idx].step(*chosen_action);
            let score_gained = envs[idx].score_manager.total_score.saturating_sub(prev_score);

            let step = move_records[idx].len();
            move_records[idx].push(GameMoveRecord {
                step,
                q: chosen_action.q,
                r: chosen_action.r,
                rotation: chosen_action.rotation,
                score_gained,
                total_score: envs[idx].score_manager.total_score,
                remaining_tiles: envs[idx].score_manager.remaining_tiles,
            });

            if res.done || envs[idx].is_game_over() {
                active[idx] = false;
            }
        }

        print!(".");
        if turn_counter % 20 == 0 {
            print!(" [Turn {} | Active: {}/{}]\n", turn_counter, active.iter().filter(|&&a| a).count(), n_envs);
        }
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
    println!();

    // Tìm ván đạt điểm cao nhất trong số n_envs
    let mut best_idx = 0usize;
    let mut best_score = 0usize;
    for (i, env) in envs.iter().enumerate() {
        let sc = env.score_manager.total_score;
        println!("  Env #{} -> Score: {} | Placed: {} tiles", i + 1, sc, env.placed_count);
        if sc >= best_score {
            best_score = sc;
            best_idx = i;
        }
    }

    let best_env = &envs[best_idx];
    let record = GameMatchRecord {
        seed,
        total_score: best_env.score_manager.total_score,
        total_placed: best_env.placed_count,
        is_eval: true,
        moves: move_records[best_idx].clone(),
    };

    println!(
        "\n🏆 VÁN ĐẤU XUẤT SẮC NHẤT (Env #{}): Score={} | Placed={} | Thời gian: {:.2}s",
        best_idx + 1,
        record.total_score,
        record.total_placed,
        t0.elapsed().as_secs_f32()
    );

    let output_path = "test_replay.json";
    match serde_json::to_string_pretty(&record) {
        Ok(json) => {
            fs::write(output_path, json).expect("Không ghi được test_replay.json");
            println!("Đã lưu replay vào `{}`", output_path);
        }
        Err(e) => {
            eprintln!("Lỗi serialize replay: {:?}", e);
            std::process::exit(1);
        }
    }
}

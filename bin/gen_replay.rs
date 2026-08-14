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
    // arg2 = batch_size lá cho mỗi vòng GPU eval (mặc định 128)
    // arg3 = model path (mặc định: models/alphazero_best.bin nếu có, fallback alphazero_latest.bin)
    let args: Vec<String> = std::env::args().collect();
    let n_simulations = if args.len() > 1 {
        args[1].parse::<usize>().unwrap_or(400)
    } else {
        400
    };
    let batch_size = if args.len() > 2 {
        args[2].parse::<usize>().unwrap_or(128)
    } else {
        128
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
        "Chạy MCTS virtual-loss batch (sims={}, batch={}) | Seed={} | Stack={} | TileLimit={} ...",
        n_simulations, batch_size, seed, initial_stack, tile_limit
    );

    let mcts_config = MCTSConfig {
        c_puct: 1.5,
        gamma: 0.995,
        n_simulations,
        dirichlet_alpha: 0.3,
        dirichlet_eps: 0.25,
        explore_by_entropy: false,
        temp_high: 1.0,
        temp_low: 0.2,
    };
    let mcts = MCTSSearch::new(mcts_config.clone());

    let mut env = DorfromantikEnv::new(seed, initial_stack, tile_limit);
    let mut move_records = Vec::new();
    let mut move_count = 0usize;
    let t0 = Instant::now();

    while !env.is_game_over() {
        let obs = env.extract_graph_observation();
        if obs.valid_actions.is_empty() {
            break;
        }

        // Temperature schedule:
        // - Ở 15 nước đầu: T = 0.2 (mềm dẻo, tránh kẹt hướng đi chết người)
        // - Sau nước 15: T = 0.0 (Greedy thuần túy khai thác tối đa điểm)
        let temp = if move_count < 15 { 0.2 } else { 0.0 };

        let turn_start = Instant::now();
        let (_, _, chosen_action, _) = mcts.search_virtual_loss_batch(
            &env,
            &model,
            gpu_executor.as_ref(),
            false,   // greedy eval, không dirichlet noise
            temp,
            batch_size,
        );
        let turn_ms = turn_start.elapsed().as_millis();

        let prev_score = env.score_manager.total_score;
        let res = env.step(chosen_action);
        let score_gained = env.score_manager.total_score.saturating_sub(prev_score);

        move_records.push(GameMoveRecord {
            step: move_count,
            q: chosen_action.q,
            r: chosen_action.r,
            rotation: chosen_action.rotation,
            score_gained,
            total_score: env.score_manager.total_score,
            remaining_tiles: env.score_manager.remaining_tiles,
        });
        move_count += 1;

        println!("  [Move {}] q={} r={} rot={} score+{} (Total: {}) t={}ms", move_count, chosen_action.q, chosen_action.r, chosen_action.rotation, score_gained, env.score_manager.total_score, turn_ms);

        if res.done {
            break;
        }
    }

    let record = GameMatchRecord {
        seed,
        total_score: env.score_manager.total_score,
        total_placed: env.placed_count,
        is_eval: true,
        moves: move_records,
    };

    println!(
        "Kết quả: Score={} | Placed={} | Moves={} | Thời gian: {:.2}s",
        record.total_score,
        record.total_placed,
        record.moves.len(),
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

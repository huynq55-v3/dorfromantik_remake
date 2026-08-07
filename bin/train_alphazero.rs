use std::fs;
use std::path::Path;
use std::time::Instant;
use dorfromantik_remake::alphazero::{AlphaZeroPipeline, AlphaZeroTrainerConfig, evaluate_alphazero_agent};
use dorfromantik_remake::mcts::MCTSConfig;

fn main() {
    println!("============================================================");
    println!("=== DORFROMANTIK ALPHAZERO (MCTS + GNN) CONTINUOUS TRAIN ===");
    println!("============================================================");

    let target_seed = -2093096630;
    let tile_limit = 100;
    let parallel_envs = 16;
    
    // Đọc số simulations từ tham số dòng lệnh nếu có (mặc định 200)
    let args: Vec<String> = std::env::args().collect();
    let n_simulations = if args.len() > 1 {
        args[1].parse::<usize>().unwrap_or(200)
    } else {
        200
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
        tile_limit,
    };

    let model_dir = "models";
    fs::create_dir_all(model_dir).unwrap();
    let latest_model_path = format!("{}/alphazero_latest.bin", model_dir);
    let best_model_path = format!("{}/alphazero_best.bin", model_dir);
    let buffer_path = format!("{}/alphazero_buffer.bin", model_dir);
    let meta_path = format!("{}/alphazero_meta.txt", model_dir);

    let mut pipeline = AlphaZeroPipeline::new(config.clone());
    let mut start_iter = 0;
    let mut best_eval_score = 0;

    // Tự động kiểm tra và khôi phục từ Checkpoint cũ nếu có
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
            println!(">>> RESUMED from Iter {:04} | Buffer: {:>5} samples | Best Score: {}", start_iter, pipeline.buffer_len(), best_eval_score);
        } else {
            println!("FAILED. Starting from scratch.");
        }
    }

    println!("Target Seed        : {}", target_seed);
    println!("Tile Limit / Game  : {}", tile_limit);
    println!("MCTS Simulations   : {}", n_simulations);
    println!("Parallel Envs      : {} threads", parallel_envs);
    println!("Learning Rate      : {}", lr);
    println!("Replay Buffer Cap  : 50,000 samples");
    println!("Training Mode      : Continuous Infinite Loop (Ctrl+C to stop anytime)");
    println!("------------------------------------------------------------");

    for iter in (start_iter + 1).. {
        let t_start = Instant::now();

        // 1. Data Generation via MCTS Self-Play
        let t_gen_start = Instant::now();
        let (self_play_avg_score, self_play_max_score, self_play_avg_placed) = pipeline.collect_self_play_data();
        let t_gen = t_gen_start.elapsed();

        // 2. Training on Replay Buffer (Mini-batches with Adam Optimizer)
        let t_train_start = Instant::now();
        let (loss, pi_loss, val_loss) = pipeline.train_step();
        let t_train = t_train_start.elapsed();

        // 3. Evaluation on Target Seed
        let (eval_score, eval_placed) = evaluate_alphazero_agent(
            target_seed,
            tile_limit,
            &pipeline.model,
            &config.mcts_config,
        );

        let t_total = t_start.elapsed();

        let is_best = eval_score > best_eval_score;
        if is_best {
            best_eval_score = eval_score;
            let _ = pipeline.model.save_to_file(&best_model_path);
        }

        // Tự động lưu Checkpoint Model + Replay Buffer + Metadata sau MỖI iteration
        let _ = pipeline.save_checkpoint(&latest_model_path, &buffer_path);
        let _ = fs::write(&meta_path, format!("{},{}", iter, best_eval_score));

        let flag = if is_best {
            " [BEST SAVED]"
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

use dorfromantik_remake::ppo::PPOAgent;
use std::time::Instant;

fn main() {
    let target_seed = -2093096630;
    let tile_limit = 100;
    let lr = 0.001;
    let num_iterations = 2000;
    let num_envs_parallel = 32;   // 32 worker threads CPU
    let steps_per_env = 50;       // 1,600 transitions per rollout iteration

    println!("============================================================");
    println!("=== DORFROMANTIK PURE RL (GNN + PPO + ADAM) CPU PIPELINE ===");
    println!("============================================================");
    println!("Target Seed        : {}", target_seed);
    println!("Tile Limit / Game  : {}", tile_limit);
    println!("Parallel CPU Envs  : {} threads", num_envs_parallel);
    println!("Learning Rate      : {}", lr);
    println!("Total Iterations   : {}", num_iterations);
    println!("------------------------------------------------------------\n");

    let mut agent = PPOAgent::new(target_seed, tile_limit, lr);

    let start_overall = Instant::now();
    let mut best_eval_score = 0.0f64;
    let mut best_max_score = 0;

    for iter in 1..=num_iterations {
        let iter_start = Instant::now();

        // 1. DATA GENERATION PIPELINE: Collect Rollout Batch in Parallel on CPU
        let batch = agent.collect_rollout_parallel(num_envs_parallel, steps_per_env);
        let gen_time = iter_start.elapsed();

        // 2. TRAINING PIPELINE: PPO Policy & Value Loss Optimization with Adam
        let train_start = Instant::now();
        let loss = agent.train_step(&batch);
        let train_time = train_start.elapsed();

        // 3. EVALUATION PIPELINE: Deterministic Evaluation on Target Seed
        let (avg_score, max_score, avg_placed) = agent.evaluate(10);
        let iter_total_time = iter_start.elapsed();

        let mut saved_flag = "";
        if avg_score > best_eval_score || max_score > best_max_score {
            best_eval_score = avg_score;
            if max_score > best_max_score {
                best_max_score = max_score;
            }
            saved_flag = " [BEST SAVED]";
        }

        println!(
            "Iter {:03}/{} | DataGen: {:.2?} | Train: {:.2?} | Total: {:.2?} | Loss: {:.4} | Eval Avg Score: {:.1} | Eval Max Score: {} | Avg Placed: {}{}",
            iter, num_iterations, gen_time, train_time, iter_total_time, loss, avg_score, max_score, avg_placed, saved_flag
        );
    }

    let overall_time = start_overall.elapsed();

    println!("\n============================================================");
    println!("=== TRAINING PIPELINE COMPLETED SUCCESSFULLY ===");
    println!("Total Time Elapsed : {:.3?} seconds", overall_time);
    println!("Best Avg Score     : {:.2}", best_eval_score);
    println!("Best Max Score     : {}", best_max_score);
    println!("============================================================");
}

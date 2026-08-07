use dorfromantik_remake::env::DorfromantikEnv;
use rayon::prelude::*;
use std::time::Instant;

fn main() {
    let target_seed = -2093096630;
    let initial_stack = 10;
    let tile_limit = 100;
    let num_games = 10_000;

    println!("=== DORFROMANTIK RL SIMULATION BENCHMARK ===");
    println!("Target Seed: {}", target_seed);
    println!("Tile Limit: {}", tile_limit);
    println!("Simulating {} full games in parallel using Rayon...", num_games);

    let start_time = Instant::now();

    // Run 10,000 games in parallel across CPU threads
    let results: Vec<(usize, i32, usize)> = (0..num_games)
        .into_par_iter()
        .map(|_game_id| {
            let mut env = DorfromantikEnv::new(target_seed, initial_stack, tile_limit);
            let mut steps = 0;

            loop {
                let valid_actions = env.get_valid_actions();
                if valid_actions.is_empty() {
                    break;
                }

                // Simple policy: pick a valid action
                let action = valid_actions[steps % valid_actions.len()];
                let res = env.step(action);

                steps += 1;
                if res.done {
                    break;
                }
            }

            (steps, env.score_manager.total_score as i32, env.placed_count)
        })
        .collect();

    let duration = start_time.elapsed();
    let total_steps: usize = results.iter().map(|(steps, _, _)| steps).sum();
    let max_score = results.iter().map(|(_, score, _)| *score).max().unwrap_or(0);
    let avg_score: f64 = results.iter().map(|(_, score, _)| *score as f64).sum::<f64>() / num_games as f64;
    let steps_per_sec = total_steps as f64 / duration.as_secs_f64();
    let games_per_sec = num_games as f64 / duration.as_secs_f64();

    println!("--- BENCHMARK RESULTS ---");
    println!("Total Games Simulated : {}", num_games);
    println!("Total Steps Completed : {}", total_steps);
    println!("Time Elapsed          : {:.3?} seconds", duration);
    println!("Simulation Speed      : {:.2} steps/sec ({:.2} games/sec)", steps_per_sec, games_per_sec);
    println!("Max Score Achieved    : {}", max_score);
    println!("Average Score         : {:.2}", avg_score);

    // Verify Graph Observation Extraction on 1 game
    let env = DorfromantikEnv::new(target_seed, initial_stack, tile_limit);
    let obs = env.extract_graph_observation();
    println!("\n--- GNN GRAPH OBSERVATION VERIFICATION ---");
    println!("Total Graph Nodes (Placed + Candidates): {}", obs.node_positions.len());
    println!("Node Feature Dimension per Node       : {}", obs.node_features.first().map_or(0, |f| f.len()));
    println!("Total Edge Connections                : {}", obs.edge_index.len());
    println!("Valid Action Space Count              : {}", obs.valid_actions.len());
}

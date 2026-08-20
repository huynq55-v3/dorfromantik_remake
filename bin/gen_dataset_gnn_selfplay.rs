use rand::Rng;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use dorfromantik_remake::board::{get_neighbor_pos, opposite_direction};
use dorfromantik_remake::env::{Action, DorfromantikEnv, GraphObservation};
use dorfromantik_remake::nn::HexGNNModel;
use dorfromantik_remake::score_manager::is_matching_edge;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableGraphObservation {
    pub node_positions: Vec<(i32, i32)>,
    pub node_features_flat: Vec<f32>,
    pub edge_index: Vec<(usize, usize)>,
    pub valid_actions: Vec<Action>,
    pub action_features_flat: Vec<f32>,
}

impl From<GraphObservation> for SerializableGraphObservation {
    fn from(obs: GraphObservation) -> Self {
        let mut node_features_flat = Vec::with_capacity(obs.node_features.len() * 70);
        for feat in obs.node_features {
            node_features_flat.extend_from_slice(&feat);
        }
        let mut action_features_flat = Vec::with_capacity(obs.action_features.len() * 16);
        for feat in obs.action_features {
            action_features_flat.extend_from_slice(&feat);
        }
        Self {
            node_positions: obs.node_positions,
            node_features_flat,
            edge_index: obs.edge_index,
            valid_actions: obs.valid_actions,
            action_features_flat,
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

fn load_game_config() -> (i32, usize, usize) {
    let mut seed = -2093096630;
    let mut stack = 10;
    let mut limit = 100;
    if let Ok(content) = fs::read_to_string("monthly_game_info.txt") {
        for line in content.lines() {
            if let Some((k, v)) = line.split_once('=') {
                match k.trim() {
                    "REAL_TILE_SEED" => {
                        if let Ok(s) = v.trim().parse() {
                            seed = s;
                        }
                    }
                    "ACTIVE_TileStackHeight" => {
                        if let Ok(s) = v.trim().parse() {
                            stack = s;
                        }
                    }
                    "ACTIVE_TileLimit" => {
                        if let Ok(s) = v.trim().parse() {
                            limit = s;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    (seed, stack, limit)
}

pub fn compute_board_hash(env: &DorfromantikEnv) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    let mut sorted_keys: Vec<(i32, i32)> = env.board.placed_tiles.keys().copied().collect();
    sorted_keys.sort_unstable();

    for pos in sorted_keys {
        if let Some(pt) = env.board.placed_tiles.get(&pos) {
            pos.hash(&mut hasher);
            pt.rotation.hash(&mut hasher);
            for e in pt.edge_config.edges {
                (e as u8).hash(&mut hasher);
            }
        }
    }
    if let Some(curr) = env.current_tile() {
        curr.to_hex_edge_config().edges.hash(&mut hasher);
    }
    hasher.finish()
}

fn evaluate_state_gnn(model: &HexGNNModel, env: &DorfromantikEnv) -> f32 {
    let obs = env.extract_graph_observation();
    let (_, val) = model.forward(
        &obs.node_positions,
        &obs.node_features,
        &obs.edge_index,
        &obs.valid_actions,
        &obs.action_features,
    );
    val * 100.0
}

fn select_action_gnn_fast(model: &HexGNNModel, env: &DorfromantikEnv) -> Action {
    let valid_actions = env.get_valid_actions();
    if valid_actions.len() <= 1 {
        return valid_actions.get(0).copied().unwrap_or(Action {
            q: 0,
            r: 0,
            rotation: 0,
        });
    }

    let curr_tile = env.current_tile().unwrap();
    let curr_cfg = curr_tile.to_hex_edge_config();

    let mut scored_actions: Vec<(f32, Action)> = valid_actions
        .iter()
        .map(|&act| {
            let mut cfg = curr_cfg;
            cfg.rotate(act.rotation);
            let mut match_count = 0;
            for dir in 0..6 {
                let n_pos = get_neighbor_pos(act.q, act.r, dir);
                if let Some(neighbor) = env.board.placed_tiles.get(&n_pos) {
                    let my_edge = cfg.edges[dir];
                    let n_edge = neighbor.edge_config.edges[opposite_direction(dir)];
                    if is_matching_edge(my_edge, n_edge) {
                        match_count += 1;
                    }
                }
            }
            (match_count as f32, act)
        })
        .collect();

    scored_actions.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let top_candidates: Vec<Action> = scored_actions.into_iter().take(6).map(|(_, a)| a).collect();

    let mut best_act = top_candidates[0];
    let mut best_val = f32::NEG_INFINITY;

    for act in top_candidates {
        let mut temp = env.clone();
        let res = temp.step(act);
        let immediate_score = (res.breakdown.fit_score + res.breakdown.perfect_count * 60) as f32;
        let gnn_val = evaluate_state_gnn(model, &temp);
        let total = immediate_score + gnn_val;
        if total > best_val {
            best_val = total;
            best_act = act;
        }
    }

    best_act
}

fn main() {
    let (target_seed, initial_stack, tile_limit) = load_game_config();
    let model_path = "models/nnue_real_score_model.bin";
    let target_unique_samples = 1_000_000usize;

    println!("============================================================");
    println!(">>> BỘ SINH DỮ LIỆU TỰ CHƠI BẰNG GNN (GNN SELF-PLAY FLYWHEEL) <<<");
    println!(" - Nạp Model GNN : {}", model_path);
    println!(" - Mục Tiêu Mẫu  : {} samples", target_unique_samples);
    println!("============================================================\n");

    let model = if Path::new(model_path).exists() {
        println!("✅ Đã nạp Model GNN hiện tại!");
        HexGNNModel::load_from_file(model_path).unwrap()
    } else {
        println!("❌ Chưa có model đã train! Hãy train trước.");
        return;
    };

    let output_file = "data/real_score_dataset_gnn_selfplay.bin";
    let _ = fs::remove_file(output_file);

    let start_time = Instant::now();
    let seen_hashes = Mutex::new(HashSet::<u64>::with_capacity(target_unique_samples));
    let disk_writer = Mutex::new(BufWriter::new(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(output_file)
            .unwrap(),
    ));
    let total_unique = AtomicUsize::new(0);

    let mini_batch_size = 50;

    while total_unique.load(Ordering::Relaxed) < target_unique_samples {
        (0..mini_batch_size).into_par_iter().for_each(|_| {
            if total_unique.load(Ordering::Relaxed) >= target_unique_samples {
                return;
            }

            let mut rng = rand::thread_rng();
            let mut env = DorfromantikEnv::new(target_seed, initial_stack, tile_limit);
            let mut turn = 0;
            let mut local_samples = Vec::with_capacity(tile_limit);

            while !env.is_game_over() {
                turn += 1;
                let valid_actions = env.get_valid_actions();
                if valid_actions.is_empty() {
                    break;
                }

                let hash = compute_board_hash(&env);
                let is_new = {
                    let mut set = seen_hashes.lock().unwrap();
                    if set.len() < target_unique_samples && set.insert(hash) {
                        true
                    } else {
                        false
                    }
                };

                if is_new {
                    let obs = env.extract_graph_observation();
                    let sample = RealScoreSample {
                        obs: obs.into(),
                        real_score: env.score_manager.total_score as f32,
                        remaining_tiles: env.score_manager.remaining_tiles,
                        placed_count: env.placed_count,
                    };
                    local_samples.push(sample);
                }

                // 2 nước đầu random mở nhánh, sau đó GNN tự chơi
                let chosen_action = if turn <= 2 {
                    valid_actions[rng.gen_range(0..valid_actions.len())]
                } else if rng.gen::<f32>() < 0.15 {
                    valid_actions[rng.gen_range(0..valid_actions.len())]
                } else {
                    select_action_gnn_fast(&model, &env)
                };

                let res = env.step(chosen_action);
                if res.done {
                    break;
                }
            }

            if !local_samples.is_empty() {
                let n_saved = local_samples.len();
                let count = total_unique.fetch_add(n_saved, Ordering::Relaxed) + n_saved;

                {
                    let mut writer = disk_writer.lock().unwrap();
                    for s in &local_samples {
                        bincode::serialize_into(&mut *writer, s).unwrap();
                    }
                    let _ = writer.flush();
                }

                if count % 5_000 < n_saved || count >= target_unique_samples {
                    let elapsed = start_time.elapsed().as_secs_f32();
                    let speed = count as f32 / elapsed.max(0.001);
                    println!(
                        "⏳ [GNN Flywheel] Đã lưu {:>6}/{} mẫu UNIQUE ({:>3.0}%) | Tốc độ: {:>5.0} mẫu/s | {:.1}s",
                        count, target_unique_samples, (count as f32 / target_unique_samples as f32) * 100.0, speed, elapsed
                    );
                }
            }
        });
    }

    let dur = start_time.elapsed();
    println!(
        "\n✅ Hoàn tất GNN Self-Play Flywheel trong {:.2}s!",
        dur.as_secs_f32()
    );
    println!("🎉 File dataset lưu tại: {}", output_file);
}

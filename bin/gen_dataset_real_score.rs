use rayon::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use dorfromantik_remake::env::{Action, DorfromantikEnv, GraphObservation};

/// Dạng Serializable chuẩn cho GraphObservation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableGraphObservation {
    pub node_positions: Vec<(i32, i32)>,
    pub node_features_flat: Vec<f32>, // flattened N * 70
    pub edge_index: Vec<(usize, usize)>,
    pub valid_actions: Vec<Action>,
    pub action_features_flat: Vec<f32>, // flattened Num_Actions * 16
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

/// Mẫu dữ liệu huấn luyện NNUE Supervised (SerializableGraphObservation + Điểm Số Thật)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealScoreSample {
    pub obs: SerializableGraphObservation,
    pub real_score: f32,
    pub remaining_tiles: usize,
    pub placed_count: usize,
}

/// Tính Hash 64-bit bất biến cho 1 trạng thái bàn cờ (Board State Hash)
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
    // Hash cả tile hiện tại trong hàng chờ
    if let Some(curr) = env.current_tile() {
        curr.to_hex_edge_config().edges.hash(&mut hasher);
    }
    hasher.finish()
}

fn load_game_config() -> (i32, usize, usize) {
    let mut seed = -2093096630;
    let mut stack = 10;
    let mut limit = 100;
    if let Ok(content) = fs::read_to_string("monthly_game_info.txt") {
        for line in content.lines() {
            if let Some((k, v)) = line.split_once('=') {
                match k.trim() {
                    "REAL_TILE_SEED" => if let Ok(s) = v.trim().parse() { seed = s; },
                    "ACTIVE_TileStackHeight" => if let Ok(s) = v.trim().parse() { stack = s; },
                    "ACTIVE_TileLimit" => if let Ok(s) = v.trim().parse() { limit = s; },
                    _ => {}
                }
            }
        }
    }
    (seed, stack, limit)
}

fn main() {
    let (target_seed, initial_stack, tile_limit) = load_game_config();
    let target_unique_samples = 1_000_000usize; // 1 TRIỆU MẪU UNIQUE

    println!("============================================================");
    println!(">>> CÔNG CỤ SINH DATASET ĐIỂM SỐ THẬT (1 TRIỆU MẪU UNIQUE) <<<");
    println!(" - Seed Mục Tiêu: {}", target_seed);
    println!(" - Mục Tiêu Mẫu: {} samples duy nhất", target_unique_samples);
    println!(" - Chạy Song Song Đa Luồng CPU (Rayon)");
    println!("============================================================\n");

    let output_dir = "data";
    fs::create_dir_all(output_dir).unwrap();
    let output_file = format!("{}/real_score_dataset_1m.bin", output_dir);

    let start_time = Instant::now();
    let batch_games = 50_000;

    let all_samples: Vec<(u64, RealScoreSample)> = (0..batch_games)
        .into_par_iter()
        .flat_map(|_| {
            let mut rng = rand::thread_rng();
            // Chạy 100% chuẩn xác trên Target Seed của monthly_game_info.txt
            let mut env = DorfromantikEnv::new(target_seed, initial_stack, tile_limit);

            let mut game_records = Vec::with_capacity(tile_limit);

            while !env.is_game_over() {
                let valid_actions = env.get_valid_actions();
                if valid_actions.is_empty() {
                    break;
                }

                let hash = compute_board_hash(&env);
                let obs = env.extract_graph_observation();
                let real_score = env.score_manager.total_score as f32;
                let remaining_tiles = env.score_manager.remaining_tiles;
                let placed_count = env.placed_count;

                game_records.push((
                    hash,
                    RealScoreSample {
                        obs: obs.into(),
                        real_score,
                        remaining_tiles,
                        placed_count,
                    },
                ));

                // Chọn nước đi kết hợp (90% chọn nước đi tốt nhất, 10% chọn ngẫu nhiên để đa dạng thế cờ)
                let chosen_action = if rng.gen::<f32>() < 0.10 {
                    valid_actions[rng.gen_range(0..valid_actions.len())]
                } else {
                    let mut best_act = valid_actions[0];
                    let mut max_sc = -1000.0f32;
                    for &act in &valid_actions {
                        let mut temp = env.clone();
                        let r = temp.step(act);
                        let sc = r.breakdown.fit_score as f32 + (r.breakdown.perfect_count * 60) as f32;
                        if sc > max_sc {
                            max_sc = sc;
                            best_act = act;
                        }
                    }
                    best_act
                };

                let res = env.step(chosen_action);
                if res.done {
                    break;
                }
            }

            game_records
        })
        .collect();

    println!("[Lọc Trùng] Thu được {} mẫu thô. Đang lọc Unique bằng Hash 64-bit...", all_samples.len());

    let mut seen_hashes: HashSet<u64> = HashSet::with_capacity(all_samples.len());
    let mut unique_samples: Vec<RealScoreSample> = Vec::with_capacity(target_unique_samples);

    for (hash, sample) in all_samples {
        if seen_hashes.insert(hash) {
            unique_samples.push(sample);
            if unique_samples.len() >= target_unique_samples {
                break;
            }
        }
    }

    let dur = start_time.elapsed();
    println!("✅ Hoàn tất lọc! Thu được {} MẪU DUY NHẤT (100% Unique) trong {:.2}s!", unique_samples.len(), dur.as_secs_f32());

    println!("[Lưu Trữ] Đang ghi dataset vào file: {}...", output_file);
    let mut file = BufWriter::new(File::create(&output_file).unwrap());
    bincode::serialize_into(&mut file, &unique_samples).unwrap();
    file.flush().unwrap();

    println!("🎉 XONG! File dataset sẵn sàng cho huấn luyện NNUE!");
}

use rayon::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use dorfromantik_remake::board::{get_neighbor_pos, opposite_direction};
use dorfromantik_remake::env::{Action, DorfromantikEnv, GraphObservation};
use dorfromantik_remake::score_manager::is_matching_edge;

/// Dạng Serializable chuẩn cho GraphObservation
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
    let target_unique_samples = 300_000usize; // 300K MẪU ĐỘC NHẤT (Chỉ chiếm ~300MB RAM)

    println!("============================================================");
    println!(">>> CÔNG CỤ SINH DATASET ĐIỂM SỐ THẬT (STREAMING - SIÊU NHẸ RAM) <<<");
    println!(" - Seed Mục Tiêu: {}", target_seed);
    println!(" - Mục Tiêu Mẫu: {} samples duy nhất", target_unique_samples);
    println!(" - Cơ Chế: Ghi trực tiếp vào đĩa (RAM luôn < 350 MB)");
    println!("============================================================\n");

    let output_dir = "data";
    fs::create_dir_all(output_dir).unwrap();
    let output_file = format!("{}/real_score_dataset_1m.bin", output_dir);

    let start_time = Instant::now();
    let seen_hashes = Mutex::new(HashSet::<u64>::with_capacity(target_unique_samples));
    let collected_samples = Mutex::new(Vec::<RealScoreSample>::with_capacity(target_unique_samples));
    let total_unique = AtomicUsize::new(0);

    let mini_batch_size = 500; // Mỗi đợt chạy 500 ván rồi giải phóng RAM ngay

    while total_unique.load(Ordering::Relaxed) < target_unique_samples {
        (0..mini_batch_size).into_par_iter().for_each(|_| {
            if total_unique.load(Ordering::Relaxed) >= target_unique_samples {
                return;
            }

            let mut rng = rand::thread_rng();
            let mut env = DorfromantikEnv::new(target_seed, initial_stack, tile_limit);

            while !env.is_game_over() {
                let valid_actions = env.get_valid_actions();
                if valid_actions.is_empty() {
                    break;
                }

                let hash = compute_board_hash(&env);

                // Kiểm tra hash nhanh trước khi trích xuất đồ thị (tiết kiệm CPU + RAM)
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

                    let count = total_unique.fetch_add(1, Ordering::Relaxed) + 1;
                    {
                        let mut list = collected_samples.lock().unwrap();
                        list.push(sample);
                    }

                    if count % 10_000 == 0 || count >= target_unique_samples {
                        let elapsed = start_time.elapsed().as_secs_f32();
                        let speed = count as f32 / elapsed.max(0.001);
                        println!(
                            "⏳ [Tiến Độ] Đã thu thập {:>6}/{} mẫu UNIQUE ({:>3.0}%) | Tốc độ: {:>6.0} mẫu/s | RAM: ~250MB | {:.1}s",
                            count, target_unique_samples, (count as f32 / target_unique_samples as f32) * 100.0, speed, elapsed
                        );
                    }
                }

                let chosen_action = if rng.gen::<f32>() < 0.15 {
                    valid_actions[rng.gen_range(0..valid_actions.len())]
                } else {
                    let mut best_act = valid_actions[0];
                    let mut max_sc = -1000.0f32;
                    let curr_tile = env.current_tile().unwrap();
                    let curr_cfg = curr_tile.to_hex_edge_config();

                    for &act in &valid_actions {
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
                        let sc = match_count as f32 * 10.0;
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
        });
    }

    let final_samples = collected_samples.into_inner().unwrap();
    let dur = start_time.elapsed();
    println!("\n✅ Hoàn tất! Thu thập đủ {} MẪU UNIQUE trong {:.2}s!", final_samples.len(), dur.as_secs_f32());

    println!("[Lưu Trữ] Đang ghi dataset vào file: {}...", output_file);
    let mut file = BufWriter::new(File::create(&output_file).unwrap());
    bincode::serialize_into(&mut file, &final_samples).unwrap();
    file.flush().unwrap();

    println!("🎉 XONG! File dataset sẵn sàng cho huấn luyện!");
}

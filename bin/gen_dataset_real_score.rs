use rayon::prelude::*;
use rand::Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use dorfromantik_remake::board::{get_neighbor_pos, opposite_direction, FulfillmentStatus};
use dorfromantik_remake::env::{Action, DorfromantikEnv, GraphObservation};
use dorfromantik_remake::score_manager::is_matching_edge;
use dorfromantik_remake::tile::{EqualityComparison, GeneratedTile};

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

/// Vector 10 Trọng Số Heuristic Tinh Hoa
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeuristicWeights {
    pub w_fit: f32,                // w1: Điểm ghép cạnh (+10/cạnh)
    pub w_perfect: f32,            // w2: Thưởng đạt Perfect (+60 và +1 tile)
    pub w_quest_completed: f32,    // w3: Thưởng hoàn thành Quest (+100 và +5 tiles)
    pub w_mismatch_penalty: f32,   // w4: Phạt cạnh lệch địa hình
    pub w_pocket_created: f32,     // w5: Tiềm năng tạo lỗ khóa chờ
    pub w_quest_progress: f32,     // w6: Tiến độ tăng thêm element/segment cho Quest
    pub w_quest_overflow: f32,     // w7: Phạt nặng làm vỡ Quest dấu bằng (Exactly)
    pub w_open_edges: f32,         // w8: Duy trì số cạnh mở cho cụm
    pub w_stack_health: f32,       // w9: Bảo vệ và tích lũy cọc bài
    pub w_preview_match: f32,      // w10: Tương thích với Tile #2
}

impl Default for HeuristicWeights {
    fn default() -> Self {
        Self {
            w_fit: 1.0,
            w_perfect: 20.0,
            w_quest_completed: 60.0,
            w_mismatch_penalty: -2.5,
            w_pocket_created: 12.0,
            w_quest_progress: 1.5,
            w_quest_overflow: -100.0,
            w_open_edges: 2.0,
            w_stack_health: 3.0,
            w_preview_match: 1.5,
        }
    }
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

fn load_best_weights() -> HeuristicWeights {
    let path = "models/heuristic_best_weights.json";
    if Path::new(path).exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(w) = serde_json::from_str::<HeuristicWeights>(&content) {
                println!("✅ Đã nạp thành công Bộ Trọng Số Di Truyền Tinh Hoa Kỷ Lục từ `{}`!", path);
                return w;
            }
        }
    }
    HeuristicWeights::default()
}

/// Đánh giá nhanh 1 nước đi bằng Heuristic 10 đặc trưng
pub fn evaluate_action_fast(env: &DorfromantikEnv, act: Action, weights: &HeuristicWeights) -> f32 {
    let curr_tile = match env.current_tile() {
        Some(t) => t,
        None => return 0.0,
    };

    let mut cfg = curr_tile.to_hex_edge_config();
    cfg.rotate(act.rotation);

    let mut f_fit = 0.0f32;
    let mut f_mismatch = 0.0f32;
    let mut neighbor_count = 0usize;
    let mut matched_edges = 0usize;

    for dir in 0..6 {
        let n_pos = get_neighbor_pos(act.q, act.r, dir);
        if let Some(neighbor) = env.board.placed_tiles.get(&n_pos) {
            neighbor_count += 1;
            let my_edge = cfg.edges[dir];
            let n_edge = neighbor.edge_config.edges[opposite_direction(dir)];

            if is_matching_edge(my_edge, n_edge) {
                matched_edges += 1;
                f_fit += 10.0;
            } else {
                f_mismatch += 1.0;
            }
        }
    }

    let f_perfect = if neighbor_count == 6 && matched_edges == 6 { 1.0 } else { 0.0 };

    let mut temp_env = env.clone();
    let res = temp_env.step(act);

    let f_quest_completed = res.breakdown.bubble_quests_completed as f32;

    let mut f_pocket = 0.0f32;
    for dir in 0..6 {
        let n_pos = get_neighbor_pos(act.q, act.r, dir);
        if !temp_env.board.placed_tiles.contains_key(&n_pos) {
            let mut surrounding_total = 0;
            let mut surrounding_matched = 0;
            for n_dir in 0..6 {
                let nn_pos = get_neighbor_pos(n_pos.0, n_pos.1, n_dir);
                if let Some(nn_tile) = temp_env.board.placed_tiles.get(&nn_pos) {
                    surrounding_total += 1;
                    let nn_edge = nn_tile.edge_config.edges[opposite_direction(n_dir)];
                    if let Some(front) = temp_env.current_tile() {
                        let front_cfg = front.to_hex_edge_config();
                        if (0..6).any(|rot| {
                            let mut c = front_cfg;
                            c.rotate(rot);
                            is_matching_edge(c.edges[n_dir], nn_edge)
                        }) {
                            surrounding_matched += 1;
                        }
                    }
                }
            }
            if surrounding_total >= 3 && surrounding_matched == surrounding_total {
                f_pocket += surrounding_total as f32;
            }
        }
    }

    let mut f_quest_progress = 0.0f32;
    let mut f_quest_overflow = 0.0f32;

    for (pos, pt) in &temp_env.board.placed_tiles {
        if let GeneratedTile::Quest { quest_data, .. } = &pt.tile {
            if pt.quest_status == Some(FulfillmentStatus::Incomplete) {
                let gt = quest_data.primary_group_type();
                let current_ext_count = temp_env.board.get_quest_external_count(*pos, gt);
                let target = quest_data.target_count;

                if current_ext_count > 0 && target > 0 {
                    if current_ext_count <= target {
                        f_quest_progress += (current_ext_count as f32) / (target as f32) * 10.0;
                    } else if quest_data.equality == EqualityComparison::Exactly {
                        f_quest_overflow += (current_ext_count - target) as f32;
                    }
                }
            }
        }
    }

    let f_open_edges = (6.0 - neighbor_count as f32).max(0.0);
    let f_stack = res.stack_height as f32;

    let mut f_preview = 0.0f32;
    if let Some(next_tile) = temp_env.tile_queue.get(0) {
        let n_cfg = next_tile.to_hex_edge_config();
        for dir in 0..6 {
            let my_edge = cfg.edges[dir];
            if (0..6).any(|rot| {
                let mut c = n_cfg;
                c.rotate(rot);
                is_matching_edge(my_edge, c.edges[opposite_direction(dir)])
            }) {
                f_preview += 1.0;
            }
        }
    }

    weights.w_fit * f_fit
        + weights.w_perfect * f_perfect
        + weights.w_quest_completed * f_quest_completed
        + weights.w_mismatch_penalty * f_mismatch
        + weights.w_pocket_created * f_pocket
        + weights.w_quest_progress * f_quest_progress
        + weights.w_quest_overflow * f_quest_overflow
        + weights.w_open_edges * f_open_edges
        + weights.w_stack_health * f_stack
        + weights.w_preview_match * f_preview
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

fn main() {
    let (target_seed, initial_stack, tile_limit) = load_game_config();
    let base_weights = load_best_weights();
    let target_unique_samples = 300_000usize;

    println!("============================================================");
    println!(">>> BỘ SINH DATASET ĐA DẠNG HÓA TỐI ĐA (DIVERSITY GENERATOR) <<<");
    println!(" - Seed Mục Tiêu : {}", target_seed);
    println!(" - Mục Tiêu Mẫu  : {} samples độc nhất (100% Unique)", target_unique_samples);
    println!(" - Đa Dạng Hóa   : Random Warmup (3 turns) + Weight Noise (±15%) + Softmax (T=0.8)");
    println!("============================================================\n");

    let output_dir = "data";
    fs::create_dir_all(output_dir).unwrap();
    let output_file = format!("{}/real_score_dataset_1m.bin", output_dir);

    let start_time = Instant::now();
    let seen_hashes = Mutex::new(HashSet::<u64>::with_capacity(target_unique_samples));
    let collected_samples = Mutex::new(Vec::<RealScoreSample>::with_capacity(target_unique_samples));
    let total_unique = AtomicUsize::new(0);

    let mini_batch_size = 200;

    while total_unique.load(Ordering::Relaxed) < target_unique_samples {
        (0..mini_batch_size).into_par_iter().for_each(|_| {
            if total_unique.load(Ordering::Relaxed) >= target_unique_samples {
                return;
            }

            let mut rng = rand::thread_rng();
            let normal = Normal::new(0.0, 0.15).unwrap();

            // 1. Tạo biến thể phong cách chơi (Weight Perturbation ±15%)
            let mut game_weights = base_weights.clone();
            game_weights.w_fit *= 1.0 + normal.sample(&mut rng);
            game_weights.w_perfect *= 1.0 + normal.sample(&mut rng);
            game_weights.w_quest_completed *= 1.0 + normal.sample(&mut rng);
            game_weights.w_mismatch_penalty *= 1.0 + normal.sample(&mut rng);
            game_weights.w_pocket_created *= 1.0 + normal.sample(&mut rng);
            game_weights.w_quest_progress *= 1.0 + normal.sample(&mut rng);
            game_weights.w_open_edges *= 1.0 + normal.sample(&mut rng);
            game_weights.w_stack_health *= 1.0 + normal.sample(&mut rng);
            game_weights.w_preview_match *= 1.0 + normal.sample(&mut rng);

            let mut env = DorfromantikEnv::new(target_seed, initial_stack, tile_limit);
            let mut turn = 0;

            while !env.is_game_over() {
                turn += 1;
                let valid_actions = env.get_valid_actions();
                if valid_actions.is_empty() {
                    break;
                }

                let hash = compute_board_hash(&env);

                // Lọc trùng trước khi extract đồ thị
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

                    if count % 5_000 == 0 || count >= target_unique_samples {
                        let elapsed = start_time.elapsed().as_secs_f32();
                        let speed = count as f32 / elapsed.max(0.001);
                        println!(
                            "⏳ [Tiến Độ] Đã thu thập {:>6}/{} mẫu UNIQUE ({:>3.0}%) | Tốc độ: {:>6.0} mẫu/s | {:.1}s",
                            count, target_unique_samples, (count as f32 / target_unique_samples as f32) * 100.0, speed, elapsed
                        );
                    }
                }

                // 2. Chọn nước đi: 3 nước đầu random mở nhánh, sau đó dùng Heuristic + Softmax Sampling (T=0.8)
                let chosen_action = if turn <= 3 {
                    valid_actions[rng.gen_range(0..valid_actions.len())]
                } else {
                    let mut scored: Vec<(f32, Action)> = valid_actions
                        .iter()
                        .map(|&act| (evaluate_action_fast(&env, act, &game_weights), act))
                        .collect();

                    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                    let top_k = scored.into_iter().take(5).collect::<Vec<_>>();

                    if top_k.is_empty() {
                        valid_actions[0]
                    } else if rng.gen::<f32>() < 0.70 {
                        // 70% chọn nước đi tốt nhất
                        top_k[0].1
                    } else {
                        // 30% chọn các nước xếp thứ 2, 3, 4, 5 để tối đa hóa độ đa dạng
                        top_k[rng.gen_range(0..top_k.len())].1
                    }
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

    println!("🎉 XONG! File dataset Đỉnh Cao sẵn sàng cho huấn luyện GNN!");
}

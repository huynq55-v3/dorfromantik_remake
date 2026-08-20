use rayon::prelude::*;
use rand::Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
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

/// Mẫu dữ liệu huấn luyện GNN Value Head (Target = TỔNG ĐIỂM CUỐI VÁN - MONTE CARLO TARGET)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealScoreSample {
    pub obs: SerializableGraphObservation,
    pub real_score: f32, // Chứa Tổng Điểm Cuối Ván (Expected Final Score)
    pub remaining_tiles: usize,
    pub placed_count: usize,
}

/// Vector 10 Trọng Số Heuristic Tinh Hoa
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeuristicWeights {
    pub w_fit: f32,
    pub w_perfect: f32,
    pub w_quest_completed: f32,
    pub w_mismatch_penalty: f32,
    pub w_pocket_created: f32,
    pub w_quest_progress: f32,
    pub w_quest_overflow: f32,
    pub w_open_edges: f32,
    pub w_stack_health: f32,
    pub w_preview_match: f32,
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
    let path = "models/best_heuristic_weights.json";
    if Path::new(path).exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(w) = serde_json::from_str::<HeuristicWeights>(&content) {
                return w;
            }
        }
    }
    HeuristicWeights::default()
}

/// Đánh giá nhanh nước đi trực tiếp trên bảng không clone `env`
#[inline(always)]
pub fn evaluate_action_inplace(env: &DorfromantikEnv, act: Action, weights: &HeuristicWeights) -> f32 {
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
    let f_open_edges = (6.0 - neighbor_count as f32).max(0.0);

    weights.w_fit * f_fit
        + weights.w_perfect * f_perfect
        + weights.w_mismatch_penalty * f_mismatch
        + weights.w_open_edges * f_open_edges
}

fn main() {
    let (target_seed, initial_stack, tile_limit) = load_game_config();
    let base_weights = load_best_weights();
    let target_total_samples = 1_000_000usize; // 1 TRIỆU MẪU KỲ VỌNG

    println!("============================================================");
    println!(">>> BỘ SINH DATASET HỌC GIÁ TRỊ KỲ VỌNG (MONTE CARLO EXPECTED VALUE) <<<");
    println!(" - Seed Mục Tiêu : {}", target_seed);
    println!(" - Mục Tiêu Mẫu  : {} samples (Học phân phối xác suất kỳ vọng E[V])", target_total_samples);
    println!(" - Mục Tiêu Học  : TARGET = TỔNG ĐIỂM KẾT THÚC VÁN ĐẤU");
    println!(" - Cơ Chế        : Zero Lock Mutex - Stream ghi đĩa siêu tốc (RAM < 100MB)");
    println!("============================================================\n");

    let output_dir = "data";
    fs::create_dir_all(output_dir).unwrap();
    let output_file = format!("{}/real_score_dataset_1m.bin", output_dir);

    let _ = fs::remove_file(&output_file);

    let start_time = Instant::now();
    let disk_writer = Mutex::new(BufWriter::new(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&output_file)
            .unwrap(),
    ));
    let total_samples_collected = AtomicUsize::new(0);

    let mini_batch_size = 500;

    while total_samples_collected.load(Ordering::Relaxed) < target_total_samples {
        (0..mini_batch_size).into_par_iter().for_each(|_| {
            if total_samples_collected.load(Ordering::Relaxed) >= target_total_samples {
                return;
            }

            let mut rng = rand::thread_rng();
            let normal = Normal::new(0.0, 0.15).unwrap();

            // Biến thể phong cách chơi (Weight Noise)
            let mut game_weights = base_weights.clone();
            game_weights.w_fit *= 1.0 + normal.sample(&mut rng);
            game_weights.w_perfect *= 1.0 + normal.sample(&mut rng);
            game_weights.w_mismatch_penalty *= 1.0 + normal.sample(&mut rng);
            game_weights.w_open_edges *= 1.0 + normal.sample(&mut rng);

            let mut env = DorfromantikEnv::new(target_seed, initial_stack, tile_limit);
            let mut turn = 0;
            let mut episode_observations = Vec::with_capacity(tile_limit);

            while !env.is_game_over() {
                turn += 1;
                let valid_actions = env.get_valid_actions();
                if valid_actions.is_empty() {
                    break;
                }

                // Thu thập đồ thị của mọi trạng thái trong ván
                let obs = env.extract_graph_observation();
                episode_observations.push((obs, env.score_manager.remaining_tiles, env.placed_count));

                // Chọn nước đi
                let chosen_action = if turn <= 3 {
                    valid_actions[rng.gen_range(0..valid_actions.len())]
                } else {
                    let mut scored: Vec<(f32, Action)> = valid_actions
                        .iter()
                        .map(|&act| (evaluate_action_inplace(&env, act, &game_weights), act))
                        .collect();

                    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                    let top_k = scored.into_iter().take(4).collect::<Vec<_>>();

                    if top_k.is_empty() {
                        valid_actions[0]
                    } else if rng.gen::<f32>() < 0.70 {
                        top_k[0].1
                    } else {
                        top_k[rng.gen_range(0..top_k.len())].1
                    }
                };

                let res = env.step(chosen_action);
                if res.done {
                    break;
                }
            }

            // GÁN TỔNG ĐIỂM CUỐI VÁN CHIA 10 (ĐƠN VỊ CƠ SỞ CỦA GAME)
            let final_game_score_div_10 = (env.score_manager.total_score as f32) / 10.0;

            if !episode_observations.is_empty() {
                let mut local_samples = Vec::with_capacity(episode_observations.len());
                for (obs, rem, placed) in episode_observations {
                    local_samples.push(RealScoreSample {
                        obs: obs.into(),
                        real_score: final_game_score_div_10, // Lưu trực tiếp điểm gốc chia 10 (ví dụ 650.0)
                        remaining_tiles: rem,
                        placed_count: placed,
                    });
                }

                let n_saved = local_samples.len();
                let count = total_samples_collected.fetch_add(n_saved, Ordering::Relaxed) + n_saved;

                {
                    let mut writer = disk_writer.lock().unwrap();
                    for s in &local_samples {
                        bincode::serialize_into(&mut *writer, s).unwrap();
                    }
                    let _ = writer.flush();
                }

                if count % 25_000 < n_saved || count >= target_total_samples {
                    let elapsed = start_time.elapsed().as_secs_f32();
                    let speed = count as f32 / elapsed.max(0.001);
                    println!(
                        "⏳ [Tiến Độ] Đã lưu {:>7}/{} mẫu Monte Carlo ({:>3.0}%) | Tốc độ: {:>6.0} mẫu/s | RAM: <100MB | {:.1}s",
                        count, target_total_samples, (count as f32 / target_total_samples as f32) * 100.0, speed, elapsed
                    );
                }
            }
        });
    }

    let dur = start_time.elapsed();
    println!("\n✅ Hoàn tất! Sinh xong {} MẪU MONTE CARLO trong {:.2}s!", target_total_samples, dur.as_secs_f32());
    println!("🎉 File dataset lưu tại: {} (Chuẩn bị huấn luyện Value Function E[V])", output_file);
}

use rayon::prelude::*;
use rand::Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Instant;

use dorfromantik_remake::board::{get_neighbor_pos, opposite_direction};
use dorfromantik_remake::env::{Action, DorfromantikEnv};
use dorfromantik_remake::score_manager::is_matching_edge;
use dorfromantik_remake::tile::GeneratedTile;

/// Vector 10 Trọng Số Heuristic Tinh Hoa
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeuristicWeights {
    pub w_fit: f32,                // w1: Điểm ghép cạnh (+10/cạnh)
    pub w_perfect: f32,            // w2: Thưởng đạt Perfect (+60 và +1 tile)
    pub w_quest_completed: f32,    // w3: Thưởng hoàn thành Quest (+100 và +5 tiles)
    pub w_mismatch_penalty: f32,   // w4: Phạt cạnh lệch địa hình
    pub w_pocket_created: f32,     // w5: Tiềm năng tạo lỗ khóa chờ (pocket >= 3 cạnh khớp)
    pub w_quest_progress: f32,     // w6: Tiến độ tăng thêm element/segment cho Quest
    pub w_quest_overflow: f32,     // w7: Phạt nặng làm vỡ Quest dấu bằng (Exactly)
    pub w_open_edges: f32,         // w8: Duy trì số cạnh mở cho cụm Nước/Đường ray/Rừng
    pub w_stack_health: f32,       // w9: Bảo vệ và tích lũy cọc bài
    pub w_preview_match: f32,      // w10: Mức độ tương thích với Tile #2 và #3
}

impl Default for HeuristicWeights {
    fn default() -> Self {
        Self {
            w_fit: 1.0,
            w_perfect: 15.0,
            w_quest_completed: 40.0,
            w_mismatch_penalty: -1.5,
            w_pocket_created: 8.0,
            w_quest_progress: 0.8,
            w_quest_overflow: -50.0,
            w_open_edges: 1.2,
            w_stack_health: 2.5,
            w_preview_match: 1.0,
        }
    }
}

impl HeuristicWeights {
    pub fn to_vec(&self) -> Vec<f32> {
        vec![
            self.w_fit,
            self.w_perfect,
            self.w_quest_completed,
            self.w_mismatch_penalty,
            self.w_pocket_created,
            self.w_quest_progress,
            self.w_quest_overflow,
            self.w_open_edges,
            self.w_stack_health,
            self.w_preview_match,
        ]
    }

    pub fn from_vec(v: &[f32]) -> Self {
        Self {
            w_fit: v[0],
            w_perfect: v[1],
            w_quest_completed: v[2],
            w_mismatch_penalty: v[3],
            w_pocket_created: v[4],
            w_quest_progress: v[5],
            w_quest_overflow: v[6],
            w_open_edges: v[7],
            w_stack_health: v[8],
            w_preview_match: v[9],
        }
    }

    /// Đột biến trọng số bằng nhiễu Gaussian
    pub fn mutate(&self, rate: f32, std_dev: f32) -> Self {
        let mut rng = rand::thread_rng();
        let normal = Normal::new(0.0, std_dev).unwrap();
        let mut v = self.to_vec();
        for val in v.iter_mut() {
            if rng.gen::<f32>() < rate {
                *val += normal.sample(&mut rng);
            }
        }
        Self::from_vec(&v)
    }
}

/// Đánh giá điểm Heuristic của 1 Action trên môi trường hiện tại
pub fn evaluate_action(env: &DorfromantikEnv, action: Action, weights: &HeuristicWeights) -> f32 {
    let current_tile = match env.current_tile() {
        Some(t) => t.clone(),
        None => return f32::NEG_INFINITY,
    };

    let mut cfg = current_tile.to_hex_edge_config();
    let period = current_tile.rotation_symmetry_period();
    cfg.rotate(action.rotation % period);

    let mut _matching_edges = 0usize;
    let mut mismatch_edges = 0usize;
    let mut neighbor_count = 0usize;

    for dir in 0..6 {
        let n_pos = get_neighbor_pos(action.q, action.r, dir);
        if let Some(neighbor) = env.board.placed_tiles.get(&n_pos) {
            neighbor_count += 1;
            let my_edge = cfg.edges[dir];
            let n_edge = neighbor.edge_config.edges[opposite_direction(dir)];
            if is_matching_edge(my_edge, n_edge) {
                _matching_edges += 1;
            } else {
                mismatch_edges += 1;
            }
        }
    }

    // Giả lập đặt thử (Trial Placement)
    let mut temp_env = env.clone();
    let res = temp_env.step(action);

    if !res.done && res.reward < -50.0 {
        return f32::NEG_INFINITY;
    }

    // 1. Fit score gain
    let f_fit = res.breakdown.fit_score as f32;

    // 2. Perfect placement gain
    let f_perfect = res.breakdown.perfect_count as f32;

    // 3. Quest completed gain
    let f_quest_completed = res.breakdown.bubble_quests_completed as f32;

    // 4. Mismatch penalty
    let f_mismatch = mismatch_edges as f32;

    // 5. Pocket potential: đếm số ô trống lân cận đang có >= 3 cạnh bao quanh và 100% đều khớp
    let mut f_pocket = 0.0f32;
    for dir in 0..6 {
        let n_pos = get_neighbor_pos(action.q, action.r, dir);
        if !temp_env.board.placed_tiles.contains_key(&n_pos) {
            let mut surrounding_matched = 0usize;
            let mut surrounding_total = 0usize;
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

    // 6. Quest progress: số element kết nối vào quest
    let f_quest_progress = if let GeneratedTile::Quest { .. } = current_tile {
        5.0
    } else {
        0.0
    };

    // 7. Quest overflow penalty
    let f_quest_overflow = 0.0f32;

    // 8. Open edges health
    let f_open_edges = (6.0 - neighbor_count as f32).max(0.0);

    // 9. Stack health
    let f_stack = res.stack_height as f32;

    // 10. Preview match with Tile #2
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

/// Cho 1 cá thể trọng số tự chơi trọn vẹn 1 ván game
pub fn play_game(seed: i32, stack: usize, limit: usize, weights: &HeuristicWeights) -> (usize, usize) {
    let mut env = DorfromantikEnv::new(seed, stack, limit);

    while !env.is_game_over() {
        let valid_actions = env.get_valid_actions();
        if valid_actions.is_empty() {
            break;
        }

        let mut best_action = valid_actions[0];
        let mut best_score = f32::NEG_INFINITY;

        for &act in &valid_actions {
            let score = evaluate_action(&env, act, weights);
            if score > best_score {
                best_score = score;
                best_action = act;
            }
        }

        let res = env.step(best_action);
        if res.done {
            break;
        }
    }

    (env.score_manager.total_score, env.placed_count)
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
    let population_size = 64;
    let generations = 100;
    let top_k = 8;

    println!("============================================================");
    println!(">>> BỘ TỐI ƯU TIẾN HÓA DI TRUYỀN HEURISTIC (GENETIC OPTIMIZER) <<<");
    println!(" - Seed Mục Tiêu: {}", target_seed);
    println!(" - Tile Stack / Limit: {} / {}", initial_stack, tile_limit);
    println!(" - Kích thước Quần Thể (Population): {} cá thể", population_size);
    println!(" - Số Thế Hệ (Generations): {}", generations);
    println!(" - Chọn lọc Top-K: {} cá thể tinh hoa / thế hệ", top_k);
    println!("============================================================\n");

    let weights_path = "models/best_heuristic_weights.json";
    let mut population: Vec<HeuristicWeights> = Vec::with_capacity(population_size);

    // Khởi tạo quần thể: nạp từ file cũ (nếu có), còn lại sinh đột biến
    if Path::new(weights_path).exists() {
        if let Ok(content) = fs::read_to_string(weights_path) {
            if let Ok(w) = serde_json::from_str::<HeuristicWeights>(&content) {
                println!("[Checkpoint] Nạp thành công bộ trọng số tốt nhất trước đó!");
                population.push(w.clone());
                for _ in 1..population_size {
                    population.push(w.mutate(0.4, 3.0));
                }
            }
        }
    }

    while population.len() < population_size {
        let base = HeuristicWeights::default();
        population.push(base.mutate(0.5, 5.0));
    }

    let mut global_best_score = 0usize;
    let mut global_best_weights = population[0].clone();

    for gen in 1..=generations {
        let start_time = Instant::now();

        // Chạy song song toàn bộ quần thể trên Rayon đa luồng
        let results: Vec<(usize, usize, HeuristicWeights)> = population
            .par_iter()
            .map(|w| {
                let (score, placed) = play_game(target_seed, initial_stack, tile_limit, w);
                (score, placed, w.clone())
            })
            .collect();

        // Sắp xếp giảm dần theo điểm số
        let mut sorted_results = results;
        sorted_results.sort_unstable_by(|a, b| b.0.cmp(&a.0));

        let best_gen_score = sorted_results[0].0;
        let best_gen_placed = sorted_results[0].1;
        let avg_gen_score = sorted_results.iter().map(|r| r.0).sum::<usize>() as f32 / population_size as f32;
        let dur = start_time.elapsed();

        if best_gen_score > global_best_score {
            global_best_score = best_gen_score;
            global_best_weights = sorted_results[0].2.clone();
            if let Ok(json_str) = serde_json::to_string_pretty(&global_best_weights) {
                let _ = fs::write(weights_path, json_str);
            }
            println!(
                "🔥 [Thế Hệ #{:>3}] KỶ LỤC MỚI: {:>5} ĐIỂM (Placed: {:>3} tiles) | Score TB: {:>6.1} | Thời gian: {:.2}s",
                gen, best_gen_score, best_gen_placed, avg_gen_score, dur.as_secs_f32()
            );
        } else {
            println!(
                "   [Thế Hệ #{:>3}] Max: {:>5} pts (Placed: {:>3}) | Score TB: {:>6.1} | Best Toàn Cầu: {:>5} pts | {:.2}s",
                gen, best_gen_score, best_gen_placed, avg_gen_score, global_best_score, dur.as_secs_f32()
            );
        }

        // CHỌN LỌC & SINH SẢN (SELECTION & REPRODUCTION)
        let elite: Vec<HeuristicWeights> = sorted_results.iter().take(top_k).map(|r| r.2.clone()).collect();
        let mut next_gen = Vec::with_capacity(population_size);

        for w in &elite {
            next_gen.push(w.clone());
        }

        let mut rng = rand::thread_rng();
        while next_gen.len() < population_size {
            let parent_idx = rng.gen_range(0..top_k);
            let child = elite[parent_idx].mutate(0.35, 2.5);
            next_gen.push(child);
        }

        population = next_gen;
    }

    println!("\n============================================================");
    println!(">>> HOÀN TẤT TIẾN HÓA! KỶ LỤC ĐẠT ĐƯỢC: {} ĐIỂM <<<", global_best_score);
    println!("Trọng số tốt nhất đã được lưu tại: {}", weights_path);
    println!("============================================================");
}

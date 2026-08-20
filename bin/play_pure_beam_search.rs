use rayon::prelude::*;
use std::fs;
use std::time::Instant;

use dorfromantik_remake::board::{get_neighbor_pos, opposite_direction};
use dorfromantik_remake::env::{Action, DorfromantikEnv};
use dorfromantik_remake::score_manager::is_matching_edge;

fn load_monthly_game_config() -> (i32, usize, usize) {
    let mut seed = -2093096630;
    let mut initial_stack = 10;
    let mut tile_limit = 100;

    if let Ok(content) = fs::read_to_string("monthly_game_info.txt") {
        for line in content.lines() {
            if let Some((key, val)) = line.split_once('=') {
                match key.trim() {
                    "REAL_TILE_SEED" => if let Ok(v) = val.trim().parse() { seed = v; },
                    "ACTIVE_TileStackHeight" => if let Ok(v) = val.trim().parse() { initial_stack = v; },
                    "ACTIVE_TileLimit" => if let Ok(v) = val.trim().parse() { tile_limit = v; },
                    _ => {}
                }
            }
        }
    }
    (seed, initial_stack, tile_limit)
}

/// Đánh giá nhanh tiềm năng của 1 nước đi bằng Điểm Thật + Sức Khỏe Cọc Bài
#[inline(always)]
fn evaluate_candidate_fast(env: &DorfromantikEnv, act: Action) -> f32 {
    let curr_tile = match env.current_tile() {
        Some(t) => t,
        None => return 0.0,
    };
    let mut cfg = curr_tile.to_hex_edge_config();
    cfg.rotate(act.rotation);

    let mut fit_score = 0.0f32;
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
                fit_score += 10.0;
            }
        }
    }

    let perfect_bonus = if neighbor_count == 6 && matched_edges == 6 { 60.0 } else { 0.0 };
    fit_score + perfect_bonus
}

/// Thuật toán PURE BEAM SEARCH (Brute-Force Đa Nhánh Sâu 3 Bước Dựa Hoàn Toàn Trên Điểm Thật Của Env)
pub fn select_best_action_pure_beam(
    env: &DorfromantikEnv,
    beam_width_step1: usize,
    beam_width_step2: usize,
    beam_width_step3: usize,
) -> Action {
    let valid_actions = env.get_valid_actions();
    if valid_actions.len() <= 1 {
        return valid_actions.get(0).copied().unwrap_or(Action { q: 0, r: 0, rotation: 0 });
    }

    // LỌC TOP ỨNG VIÊN TẦNG 1
    let mut candidates_step1: Vec<(f32, Action)> = valid_actions
        .iter()
        .map(|&act| (evaluate_candidate_fast(env, act), act))
        .collect();
    candidates_step1.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let top_candidates_step1: Vec<Action> = candidates_step1.into_iter().take(beam_width_step1).map(|(_, a)| a).collect();

    // DUYỆT TẦNG 1 SONG SONG TRÊN RAYON
    let evaluated_branches: Vec<(f32, Action)> = top_candidates_step1
        .par_iter()
        .map(|&act1| {
            let mut env1 = env.clone();
            let _ = env1.step(act1);

            if env1.is_game_over() {
                // Điểm thật thu được tại env1 + thưởng cọc bài
                let final_score = env1.score_manager.total_score as f32 + (env1.score_manager.remaining_tiles * 50) as f32;
                return (final_score, act1);
            }

            let valid_actions2 = env1.get_valid_actions();
            if valid_actions2.is_empty() {
                let score = env1.score_manager.total_score as f32 + (env1.score_manager.remaining_tiles * 50) as f32;
                return (score, act1);
            }

            // LỌC TOP ỨNG VIÊN TẦNG 2
            let mut candidates_step2: Vec<(f32, Action)> = valid_actions2
                .iter()
                .map(|&act| (evaluate_candidate_fast(&env1, act), act))
                .collect();
            candidates_step2.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            let top_candidates_step2: Vec<Action> = candidates_step2.into_iter().take(beam_width_step2).map(|(_, a)| a).collect();

            let mut max_branch_score = f32::NEG_INFINITY;

            for act2 in top_candidates_step2 {
                let mut env2 = env1.clone();
                let _ = env2.step(act2);

                if env2.is_game_over() {
                    let score = env2.score_manager.total_score as f32 + (env2.score_manager.remaining_tiles * 50) as f32;
                    if score > max_branch_score {
                        max_branch_score = score;
                    }
                    continue;
                }

                let valid_actions3 = env2.get_valid_actions();
                if valid_actions3.is_empty() {
                    let score = env2.score_manager.total_score as f32 + (env2.score_manager.remaining_tiles * 50) as f32;
                    if score > max_branch_score {
                        max_branch_score = score;
                    }
                    continue;
                }

                // LỌC TOP ỨNG VIÊN TẦNG 3
                let mut candidates_step3: Vec<(f32, Action)> = valid_actions3
                    .iter()
                    .map(|&act| (evaluate_candidate_fast(&env2, act), act))
                    .collect();
                candidates_step3.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                let top_candidates_step3: Vec<Action> = candidates_step3.into_iter().take(beam_width_step3).map(|(_, a)| a).collect();

                for act3 in top_candidates_step3 {
                    let mut env3 = env2.clone();
                    let _ = env3.step(act3);

                    // ĐIỂM THẬT CỦA ENV TẠI TẦNG 3: Tổng điểm thật + Cọc bài còn lại (Mỗi tile = 50 điểm tiềm năng)
                    let real_eval = env3.score_manager.total_score as f32
                        + (env3.score_manager.remaining_tiles as f32 * 60.0);

                    if real_eval > max_branch_score {
                        max_branch_score = real_eval;
                    }
                }
            }

            (max_branch_score, act1)
        })
        .collect();

    evaluated_branches
        .into_iter()
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, a)| a)
        .unwrap_or(valid_actions[0])
}

fn main() {
    let (target_seed, initial_stack, tile_limit) = load_monthly_game_config();

    println!("============================================================");
    println!(">>> PURE BEAM SEARCH (BRUTE-FORCE 3 TẦNG ĐIỂM THẬT ENV) <<<");
    println!(" - Seed Mục Tiêu : {}", target_seed);
    println!(" - Tile Limit    : {} tiles", tile_limit);
    println!(" - Không Dùng NN : Dựa 100% trên Simulator Ground-Truth");
    println!(" - Beam Width    : Tầng 1 (16) x Tầng 2 (8) x Tầng 3 (4) = 512 nhánh/turn");
    println!("============================================================\n");

    let mut env = DorfromantikEnv::new(target_seed, initial_stack, tile_limit);
    let start_time = Instant::now();

    println!("🎮 BẮT ĐẦU VÁN ĐẤU PURE BEAM SEARCH...\n");

    let mut turn = 0;
    while !env.is_game_over() {
        turn += 1;
        let step_start = Instant::now();
        let old_score = env.score_manager.total_score;

        // Chạy Pure Beam Search 3 Tầng: 16 x 8 x 4 = 512 nhánh giả lập song song
        let best_act = select_best_action_pure_beam(&env, 16, 8, 4);
        let res = env.step(best_act);

        let earned = env.score_manager.total_score.saturating_sub(old_score);

        println!(
            "Turn {:>3} | Placed: {:>3}/{} | Action: ({:>2}, {:>2}, rot:{}) | Fit: {:>2} | +{:>3} pts | Tổng Điểm: {:>5} | Cọc: {:>2} | {:.2}s",
            turn,
            env.placed_count,
            tile_limit,
            best_act.q,
            best_act.r,
            best_act.rotation,
            res.breakdown.fit_score,
            earned,
            env.score_manager.total_score,
            env.score_manager.remaining_tiles,
            step_start.elapsed().as_secs_f32()
        );

        if res.done {
            break;
        }
    }

    let dur = start_time.elapsed();
    println!("\n============================================================");
    println!("🏆 KẾT QUẢ VÁN ĐẤU HOÀN TẤT:");
    println!(" - Tổng Điểm Đạt Được : {} ĐIỂM", env.score_manager.total_score);
    println!(" - Số Tiles Đã Đặt    : {} / {}", env.placed_count, tile_limit);
    println!(" - Cọc Bài Còn Lại    : {} tiles", env.score_manager.remaining_tiles);
    println!(" - Tổng Thời Gian     : {:.2}s", dur.as_secs_f32());
    println!("============================================================");
}

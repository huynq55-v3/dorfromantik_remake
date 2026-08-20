use rayon::prelude::*;
use std::env as std_env;
use std::fs;
use std::sync::atomic::{AtomicI64, Ordering};
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

/// Đánh giá nhanh tiềm năng của 1 nước đi bằng Điểm Ghép Cạnh + Thưởng Perfect
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

/// Tính Cận Trên (Upper-Bound) điểm số tối đa có thể đạt thêm trong k bước tới
#[inline(always)]
fn max_possible_gain(remaining_depth: usize) -> f32 {
    // Cận trên toán học an toàn tuyệt đối (Admissible Bound):
    // 1 tile tối đa: 60 fit + 60 perfect + 2 quests (200) + 11 tiles thưởng (660) = ~980 điểm
    remaining_depth as f32 * 980.0
}

/// Thuật toán PURE BEAM SEARCH + BRANCH & BOUND PRUNING (Cắt Tỉa Cận Trên Siêu Tốc)
pub fn select_best_action_branch_bound(
    env: &DorfromantikEnv,
    beam_width_step1: usize,
    beam_width_step2: usize,
    beam_width_step3: usize,
    beam_width_step4: usize,
    depth: usize,
) -> Action {
    let valid_actions = env.get_valid_actions();
    if valid_actions.len() <= 1 {
        return valid_actions.get(0).copied().unwrap_or(Action { q: 0, r: 0, rotation: 0 });
    }

    // TẦNG 1: Lọc ứng viên và sắp xếp giảm dần theo chất lượng
    let mut candidates_step1: Vec<(f32, Action)> = valid_actions
        .iter()
        .map(|&act| (evaluate_candidate_fast(env, act), act))
        .collect();
    candidates_step1.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let top_candidates_step1: Vec<Action> = if beam_width_step1 == 0 || beam_width_step1 >= candidates_step1.len() {
        candidates_step1.into_iter().map(|(_, a)| a).collect()
    } else {
        candidates_step1.into_iter().take(beam_width_step1).map(|(_, a)| a).collect()
    };

    // Alpha chung được chia sẻ giữa tất cả các luồng để Cắt Tỉa Sớm (Global Alpha Pruning)
    let global_best_alpha = AtomicI64::new(i64::MIN);

    let evaluated_branches: Vec<(f32, Action)> = top_candidates_step1
        .par_iter()
        .map(|&act1| {
            let mut env1 = env.clone();
            let _ = env1.step(act1);

            let score1 = env1.score_manager.total_score as f32 + (env1.score_manager.remaining_tiles * 60) as f32;

            if env1.is_game_over() || depth == 1 {
                let s_int = (score1 * 100.0) as i64;
                global_best_alpha.fetch_max(s_int, Ordering::Relaxed);
                return (score1, act1);
            }

            // BRANCH & BOUND CHECK TẦNG 1:
            let upper_bound1 = score1 + max_possible_gain(depth - 1);
            let current_best = global_best_alpha.load(Ordering::Relaxed) as f32 / 100.0;
            if upper_bound1 <= current_best {
                // Nhánh này dù may mắn nhất cũng không thể thắng kỷ lục hiện tại => CẮT TỈA!
                return (f32::NEG_INFINITY, act1);
            }

            let valid_actions2 = env1.get_valid_actions();
            if valid_actions2.is_empty() {
                let s_int = (score1 * 100.0) as i64;
                global_best_alpha.fetch_max(s_int, Ordering::Relaxed);
                return (score1, act1);
            }

            // TẦNG 2: Sắp xếp giảm dần để sớm cập nhật Alpha cao nhất
            let mut candidates_step2: Vec<(f32, Action)> = valid_actions2
                .iter()
                .map(|&act| (evaluate_candidate_fast(&env1, act), act))
                .collect();
            candidates_step2.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

            let top_candidates_step2: Vec<Action> = if beam_width_step2 == 0 || beam_width_step2 >= candidates_step2.len() {
                candidates_step2.into_iter().map(|(_, a)| a).collect()
            } else {
                candidates_step2.into_iter().take(beam_width_step2).map(|(_, a)| a).collect()
            };

            let mut max_branch_score = f32::NEG_INFINITY;

            for act2 in top_candidates_step2 {
                let mut env2 = env1.clone();
                let _ = env2.step(act2);

                let score2 = env2.score_manager.total_score as f32 + (env2.score_manager.remaining_tiles * 60) as f32;

                if env2.is_game_over() || depth == 2 {
                    if score2 > max_branch_score {
                        max_branch_score = score2;
                        let s_int = (score2 * 100.0) as i64;
                        global_best_alpha.fetch_max(s_int, Ordering::Relaxed);
                    }
                    continue;
                }

                // BRANCH & BOUND CHECK TẦNG 2:
                let upper_bound2 = score2 + max_possible_gain(depth - 2);
                let cur_alpha = global_best_alpha.load(Ordering::Relaxed) as f32 / 100.0;
                if upper_bound2 <= cur_alpha {
                    continue; // Cắt tỉa nhánh con của Tầng 2
                }

                let valid_actions3 = env2.get_valid_actions();
                if valid_actions3.is_empty() {
                    if score2 > max_branch_score {
                        max_branch_score = score2;
                        let s_int = (score2 * 100.0) as i64;
                        global_best_alpha.fetch_max(s_int, Ordering::Relaxed);
                    }
                    continue;
                }

                // TẦNG 3
                let mut candidates_step3: Vec<(f32, Action)> = valid_actions3
                    .iter()
                    .map(|&act| (evaluate_candidate_fast(&env2, act), act))
                    .collect();
                candidates_step3.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

                let top_candidates_step3: Vec<Action> = if beam_width_step3 == 0 || beam_width_step3 >= candidates_step3.len() {
                    candidates_step3.into_iter().map(|(_, a)| a).collect()
                } else {
                    candidates_step3.into_iter().take(beam_width_step3).map(|(_, a)| a).collect()
                };

                for act3 in top_candidates_step3 {
                    let mut env3 = env2.clone();
                    let _ = env3.step(act3);

                    let score3 = env3.score_manager.total_score as f32 + (env3.score_manager.remaining_tiles * 60) as f32;

                    if env3.is_game_over() || depth == 3 {
                        if score3 > max_branch_score {
                            max_branch_score = score3;
                            let s_int = (score3 * 100.0) as i64;
                            global_best_alpha.fetch_max(s_int, Ordering::Relaxed);
                        }
                        continue;
                    }

                    // BRANCH & BOUND CHECK TẦNG 3:
                    let upper_bound3 = score3 + max_possible_gain(depth - 3);
                    let cur_alpha3 = global_best_alpha.load(Ordering::Relaxed) as f32 / 100.0;
                    if upper_bound3 <= cur_alpha3 {
                        continue; // Cắt tỉa nhánh con của Tầng 3
                    }

                    let valid_actions4 = env3.get_valid_actions();
                    if valid_actions4.is_empty() {
                        if score3 > max_branch_score {
                            max_branch_score = score3;
                            let s_int = (score3 * 100.0) as i64;
                            global_best_alpha.fetch_max(s_int, Ordering::Relaxed);
                        }
                        continue;
                    }

                    // TẦNG 4
                    let mut candidates_step4: Vec<(f32, Action)> = valid_actions4
                        .iter()
                        .map(|&act| (evaluate_candidate_fast(&env3, act), act))
                        .collect();
                    candidates_step4.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

                    let top_candidates_step4: Vec<Action> = if beam_width_step4 == 0 || beam_width_step4 >= candidates_step4.len() {
                        candidates_step4.into_iter().map(|(_, a)| a).collect()
                    } else {
                        candidates_step4.into_iter().take(beam_width_step4).map(|(_, a)| a).collect()
                    };

                    for act4 in top_candidates_step4 {
                        let mut env4 = env3.clone();
                        let _ = env4.step(act4);

                        let score4 = env4.score_manager.total_score as f32 + (env4.score_manager.remaining_tiles * 60) as f32;
                        if score4 > max_branch_score {
                            max_branch_score = score4;
                            let s_int = (score4 * 100.0) as i64;
                            global_best_alpha.fetch_max(s_int, Ordering::Relaxed);
                        }
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
    let args: Vec<String> = std_env::args().collect();

    let depth: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(3);
    let w1: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(32);
    let w2: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(16);
    let w3: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(8);
    let w4: usize = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(4);

    let total_branches = match depth {
        2 => w1 * w2,
        3 => w1 * w2 * w3,
        4 => w1 * w2 * w3 * w4,
        _ => w1 * w2 * w3,
    };

    println!("============================================================");
    println!(">>> PURE BEAM SEARCH + BRANCH & BOUND ALPHA PRUNING <<<");
    println!(" - Seed Mục Tiêu : {}", target_seed);
    println!(" - Tile Limit    : {} tiles", tile_limit);
    println!(" - Độ Sâu (Depth): {} Tầng", depth);
    println!(" - Cấu Hình Nhánh: W1({}) x W2({}) x W3({}) x W4({})", w1, w2, w3, w4);
    println!(" - Tối Ưu        : Cắt tỉa cận trên (Alpha Upper-Bound Pruning) đa luồng");
    if w1 == 0 && w2 == 0 && w3 == 0 {
        println!(" - Chế Độ        : ⚡ BRUTE-FORCE TOÀN DIỆN 100% CÓ CẮT TỈA ⚡");
    } else {
        println!(" - Cực Đại Nhánh : ~{} nhánh giả lập / Turn", total_branches);
    }
    println!("============================================================\n");

    let mut env = DorfromantikEnv::new(target_seed, initial_stack, tile_limit);
    let start_time = Instant::now();

    println!("🎮 BẮT ĐẦU VÁN ĐẤU PURE SEARCH (BRANCH & BOUND)...\n");

    let mut turn = 0;
    while !env.is_game_over() {
        turn += 1;
        let step_start = Instant::now();
        let old_score = env.score_manager.total_score;

        let best_act = select_best_action_branch_bound(&env, w1, w2, w3, w4, depth);
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
    println!("============================================================\n");
}

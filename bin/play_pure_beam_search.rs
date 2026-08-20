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

/// Đánh giá nhanh tiềm năng của 1 nước đi bằng Điểm Ghép Cạnh + Thưởng Perfect (để sắp xếp thứ tự duyệt)
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

/// Hàm Đệ Quy DFS Vô Hạn Tầng (Arbitrary Depth DFS with Branch & Bound Alpha Pruning)
fn dfs_search(
    env: &DorfromantikEnv,
    current_depth: usize,
    max_depth: usize,
    beam_widths: &[usize],
    max_gain_per_step: f32,
    global_best_alpha: &AtomicI64,
) -> f32 {
    if current_depth >= max_depth || env.is_game_over() {
        let score = env.score_manager.total_score as f32;
        let s_int = (score * 100.0) as i64;
        global_best_alpha.fetch_max(s_int, Ordering::Relaxed);
        return score;
    }

    let remaining_depth = max_depth - current_depth;
    let current_score = env.score_manager.total_score as f32;

    // BRANCH & BOUND CUTOFF:
    if max_gain_per_step > 0.0 {
        let upper_bound = current_score + (remaining_depth as f32) * max_gain_per_step;
        let cur_alpha = global_best_alpha.load(Ordering::Relaxed) as f32 / 100.0;
        if upper_bound <= cur_alpha {
            return f32::NEG_INFINITY; // Cắt tỉa lập tức!
        }
    }

    let valid_actions = env.get_valid_actions();
    if valid_actions.is_empty() {
        let score = env.score_manager.total_score as f32;
        let s_int = (score * 100.0) as i64;
        global_best_alpha.fetch_max(s_int, Ordering::Relaxed);
        return score;
    }

    // Sắp xếp các nước đi giảm dần theo tiềm năng ghép cạnh để sớm đạt Alpha cao
    let mut scored_actions: Vec<(f32, Action)> = valid_actions
        .into_iter()
        .map(|act| (evaluate_candidate_fast(env, act), act))
        .collect();
    scored_actions.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let beam_limit = beam_widths.get(current_depth).copied().unwrap_or(0);
    let selected_actions: Vec<Action> = if beam_limit == 0 || beam_limit >= scored_actions.len() {
        scored_actions.into_iter().map(|(_, a)| a).collect()
    } else {
        scored_actions.into_iter().take(beam_limit).map(|(_, a)| a).collect()
    };

    let mut best_score = f32::NEG_INFINITY;

    for act in selected_actions {
        let mut next_env = env.clone();
        let _ = next_env.step(act);

        let score = dfs_search(
            &next_env,
            current_depth + 1,
            max_depth,
            beam_widths,
            max_gain_per_step,
            global_best_alpha,
        );

        if score > best_score {
            best_score = score;
        }
    }

    best_score
}

/// Thuật toán tìm nước đi tối ưu tổng quát cho MỌI ĐỘ SÂU (1 -> N tầng)
pub fn select_best_action_recursive_bb(
    env: &DorfromantikEnv,
    max_gain_per_step: f32,
    max_depth: usize,
    beam_widths: &[usize],
) -> Action {
    let valid_actions = env.get_valid_actions();
    if valid_actions.len() <= 1 {
        return valid_actions.get(0).copied().unwrap_or(Action { q: 0, r: 0, rotation: 0 });
    }

    // Sắp xếp tầng 1
    let mut scored_actions: Vec<(f32, Action)> = valid_actions
        .into_iter()
        .map(|act| (evaluate_candidate_fast(env, act), act))
        .collect();
    scored_actions.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let beam1_limit = beam_widths.get(0).copied().unwrap_or(0);
    let top_actions: Vec<Action> = if beam1_limit == 0 || beam1_limit >= scored_actions.len() {
        scored_actions.into_iter().map(|(_, a)| a).collect()
    } else {
        scored_actions.into_iter().take(beam1_limit).map(|(_, a)| a).collect()
    };

    let global_best_alpha = AtomicI64::new(i64::MIN);

    // Duyệt song song tầng 1 trên toàn bộ CPU Cores (Rayon)
    let evaluated: Vec<(f32, Action)> = top_actions
        .par_iter()
        .map(|&act1| {
            let mut next_env = env.clone();
            let _ = next_env.step(act1);

            let score = dfs_search(
                &next_env,
                1,
                max_depth,
                beam_widths,
                max_gain_per_step,
                &global_best_alpha,
            );

            (score, act1)
        })
        .collect();

    evaluated
        .into_iter()
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, a)| a)
        .unwrap_or(top_actions[0])
}

fn main() {
    let (target_seed, initial_stack, tile_limit) = load_monthly_game_config();
    let args: Vec<String> = std_env::args().collect();

    // Tham số 1: Max Gain Cắt Tỉa (Mặc định 320.0, 0 = tắt cắt tỉa)
    let max_gain_per_step: f32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(320.0);
    // Tham số 2: Độ Sâu Tùy Ý (Depth 1 -> 100 tầng)
    let max_depth: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3);

    // Các tham số tiếp theo: Độ rộng nhánh từng tầng W1, W2, W3, ..., W_N
    let mut beam_widths = Vec::new();
    for i in 3..args.len() {
        if let Ok(w) = args[i].parse::<usize>() {
            beam_widths.push(w);
        }
    }
    // Nếu truyền thiếu độ rộng cho các tầng sâu, tự động điền 0 (100% tất cả nước)
    while beam_widths.len() < max_depth {
        beam_widths.push(0);
    }

    let config_str = beam_widths
        .iter()
        .enumerate()
        .map(|(i, &w)| format!("W{}({})", i + 1, w))
        .collect::<Vec<_>>()
        .join(" x ");

    println!("============================================================");
    println!(">>> PURE TOTAL SCORE SEARCH (ĐỆ QUY VÔ HẠN TẦNG + BRANCH & BOUND) <<<");
    println!(" - Seed Mục Tiêu : {}", target_seed);
    println!(" - Tile Limit    : {} tiles", tile_limit);
    println!(" - Tham Số Cận   : Max Gain = {:.1} pts / step", max_gain_per_step);
    println!(" - Độ Sâu (Depth): {} Tầng", max_depth);
    println!(" - Cấu Hình Nhánh: {}", config_str);
    println!(" - Tiêu Chí So   : 100% Total Score của Game");
    println!("============================================================\n");

    let mut env = DorfromantikEnv::new(target_seed, initial_stack, tile_limit);
    let start_time = Instant::now();

    println!("🎮 BẮT ĐẦU VÁN ĐẤU...\n");

    let mut turn = 0;
    while !env.is_game_over() {
        turn += 1;
        let step_start = Instant::now();
        let old_score = env.score_manager.total_score;

        let best_act = select_best_action_recursive_bb(&env, max_gain_per_step, max_depth, &beam_widths);
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

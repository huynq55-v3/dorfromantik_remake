use rayon::prelude::*;
use std::fs;
use std::path::Path;
use std::time::Instant;

use dorfromantik_remake::board::{get_neighbor_pos, opposite_direction};
use dorfromantik_remake::env::{Action, DorfromantikEnv};
use dorfromantik_remake::nn::HexGNNModel;
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

/// Đánh giá thế cờ bằng mạng GNN Value Head thuần
fn evaluate_state_gnn(model: &HexGNNModel, env: &DorfromantikEnv) -> f32 {
    let obs = env.extract_graph_observation();
    let (_, val) = model.forward(
        &obs.node_positions,
        &obs.node_features,
        &obs.edge_index,
        &obs.valid_actions,
        &obs.action_features,
    );
    // Scale ngược lại về điểm thật (x 100.0)
    val * 100.0
}

/// Chọn nước đi tốt nhất bằng 2-Step Beam Search được hướng dẫn bởi mạng GNN Value Model
fn select_best_action_gnn_beam(model: &HexGNNModel, env: &DorfromantikEnv, beam_width: usize) -> Action {
    let valid_actions = env.get_valid_actions();
    if valid_actions.len() <= 1 {
        return valid_actions[0];
    }

    // BƯỚC 1: Lọc nhanh top ứng viên tiềm năng nhất ở Step 1
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

    // Sắp xếp giảm dần theo số cạnh khớp
    scored_actions.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let top_candidates: Vec<Action> = scored_actions.into_iter().take(beam_width).map(|(_, a)| a).collect();

    // BƯỚC 2: Chạy song song Rayon đánh giá sâu 2 bước bằng mạng GNN Value Head
    let evaluated: Vec<(f32, Action)> = top_candidates
        .par_iter()
        .map(|&act1| {
            let mut env1 = env.clone();
            let res1 = env1.step(act1);
            let score1 = (res1.breakdown.fit_score + res1.breakdown.perfect_count * 60) as f32;

            if env1.is_game_over() {
                return (score1, act1);
            }

            let valid_actions2 = env1.get_valid_actions();
            if valid_actions2.is_empty() {
                let gnn_val = evaluate_state_gnn(model, &env1);
                return (score1 + gnn_val, act1);
            }

            // Ở step 2, tìm nước đi tiếp theo có GNN Value Head đánh giá cao nhất
            let mut max_step2_eval = f32::NEG_INFINITY;
            for &act2 in &valid_actions2 {
                let mut env2 = env1.clone();
                let res2 = env2.step(act2);
                let immediate_score = (res2.breakdown.fit_score + res2.breakdown.perfect_count * 60) as f32;
                let gnn_val = evaluate_state_gnn(model, &env2);

                let total_eval = score1 + immediate_score + gnn_val;
                if total_eval > max_step2_eval {
                    max_step2_eval = total_eval;
                }
            }

            (max_step2_eval, act1)
        })
        .collect();

    evaluated
        .into_iter()
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, a)| a)
        .unwrap_or(valid_actions[0])
}

fn main() {
    let (target_seed, initial_stack, tile_limit) = load_monthly_game_config();
    let model_path = "models/nnue_real_score_model.bin";

    println!("============================================================");
    println!(">>> CHƠI THỬ NGHIỆM VỚI MẠNG GNN VALUE HEAD + BEAM SEARCH <<<");
    println!(" - Seed Mục Tiêu: {}", target_seed);
    println!(" - Tile Limit: {} tiles", tile_limit);
    println!(" - Nạp Model: {}", model_path);
    println!("============================================================\n");

    let model = if Path::new(model_path).exists() {
        println!("✅ Đã nạp thành công Model GNN!");
        HexGNNModel::load_from_file(model_path).unwrap()
    } else {
        println!("⚠️ Chưa tìm thấy model đã train, sử dụng Model ngẫu nhiên.");
        HexGNNModel::new()
    };

    let mut env = DorfromantikEnv::new(target_seed, initial_stack, tile_limit);
    let start_time = Instant::now();
    let beam_width = 12; // Khám phá 12 nhánh tốt nhất mỗi lượt

    println!("🎮 BẮT ĐẦU VÁN CHƠI TỰ ĐỘNG...\n");

    let mut turn = 0;
    while !env.is_game_over() {
        turn += 1;
        let step_start = Instant::now();
        let old_score = env.score_manager.total_score;

        // AI GNN Beam Search chọn nước đi
        let best_act = select_best_action_gnn_beam(&model, &env, beam_width);
        let res = env.step(best_act);

        let gnn_val = evaluate_state_gnn(&model, &env);
        let earned = env.score_manager.total_score.saturating_sub(old_score);

        println!(
            "Turn {:>3} | Placed: {:>3}/{} | Action: ({:>2}, {:>2}, rot:{}) | Fit: {:>2} | +{:>3} pts | Tổng Điểm: {:>5} | GNN Eval: {:>6.1} | {:.2}s",
            turn,
            env.placed_count,
            tile_limit,
            best_act.q,
            best_act.r,
            best_act.rotation,
            res.breakdown.fit_score,
            earned,
            env.score_manager.total_score,
            gnn_val,
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

use rayon::prelude::*;
use std::env as std_env;
use std::fs;
use std::path::Path;
use std::time::Instant;

use dorfromantik_remake::env::{Action, DorfromantikEnv};
use dorfromantik_remake::nn::HexGNNModel;

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

/// Đánh giá trạng thái đồ thị trực tiếp bằng mạng GNN Value Head
fn evaluate_state_gnn(model: &HexGNNModel, env: &DorfromantikEnv) -> f32 {
    let obs = env.extract_graph_observation();
    let (_, val) = model.forward(
        &obs.node_positions,
        &obs.node_features,
        &obs.edge_index,
        &obs.valid_actions,
        &obs.action_features,
    );
    val * 1000.0 // Scaled về điểm thực chuẩn xác (/1000.0 lúc train)
}

/// Chọn nước đi tối ưu bằng GNN Value Head:
/// - Nếu beam_limit == 0: ĐÁNH GIÁ 100% TẤT CẢ CÁC NƯỚC ĐI HỢP LỆ (Không lọc thô, để GNN tự nhìn nhận toàn bộ!)
/// - Nếu depth == 1: 1-Step Direct GNN Evaluation
/// - Nếu depth == 2: 2-Step Lookahead GNN Evaluation
pub fn select_best_action_gnn_beam(
    env: &DorfromantikEnv,
    model: &HexGNNModel,
    depth: usize,
    beam_limit: usize,
) -> (Action, f32) {
    let valid_actions = env.get_valid_actions();
    if valid_actions.len() <= 1 {
        let act = valid_actions.get(0).copied().unwrap_or(Action { q: 0, r: 0, rotation: 0 });
        return (act, 0.0);
    }

    let actions_to_evaluate: Vec<Action> = if beam_limit == 0 || beam_limit >= valid_actions.len() {
        valid_actions.clone() // 100% TẤT CẢ CÁC NƯỚC ĐI!
    } else {
        valid_actions.iter().copied().take(beam_limit).collect()
    };

    // Duyệt song song trên Rayon để nạp vào GNN
    let evaluated: Vec<(f32, Action)> = actions_to_evaluate
        .par_iter()
        .map(|&act1| {
            let mut env1 = env.clone();
            let _ = env1.step(act1);

            if env1.is_game_over() || depth == 1 {
                let gnn_val = evaluate_state_gnn(model, &env1);
                return (gnn_val, act1);
            }

            // Nếu depth == 2: Xem tiếp nước đi tối ưu của lượt sau
            let valid_actions2 = env1.get_valid_actions();
            if valid_actions2.is_empty() {
                let gnn_val = evaluate_state_gnn(model, &env1);
                return (gnn_val, act1);
            }

            let mut max_step2_eval = f32::NEG_INFINITY;
            for &act2 in &valid_actions2 {
                let mut env2 = env1.clone();
                let _ = env2.step(act2);
                let gnn_val2 = evaluate_state_gnn(model, &env2);

                if gnn_val2 > max_step2_eval {
                    max_step2_eval = gnn_val2;
                }
            }

            (max_step2_eval, act1)
        })
        .collect();

    evaluated
        .into_iter()
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((0.0, valid_actions[0]))
        .swap_tuple()
}

trait SwapTuple {
    fn swap_tuple(self) -> (Action, f32);
}
impl SwapTuple for (f32, Action) {
    fn swap_tuple(self) -> (Action, f32) {
        (self.1, self.0)
    }
}

fn main() {
    let (target_seed, initial_stack, tile_limit) = load_monthly_game_config();
    let model_path = "models/nnue_real_score_model.bin";
    let args: Vec<String> = std_env::args().collect();

    let depth: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let beam_limit: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0); // 0 = 100% tất cả các nước

    println!("============================================================");
    println!(">>> CHƠI THỬ TỰ ĐỘNG VỚI GNN VALUE TIÊN TRI TƯƠNG LAI <<<");
    println!(" - Seed Mục Tiêu : {}", target_seed);
    println!(" - Tile Limit    : {} tiles", tile_limit);
    println!(" - Độ Sâu (Depth): {} Tầng Lookahead", depth);
    println!(" - Nước Đi Xét   : {}", if beam_limit == 0 { "100% TẤT CẢ CÁC NƯỚC HỢP LỆ (Không lọc thô)" } else { "Top giới hạn" });
    println!(" - Nạp Model     : {}", model_path);
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

    println!("🎮 BẮT ĐẦU VÁN CHƠI TỰ ĐỘNG...\n");

    let mut turn = 0;
    while !env.is_game_over() {
        turn += 1;
        let step_start = Instant::now();
        let old_score = env.score_manager.total_score;

        let (best_act, gnn_eval) = select_best_action_gnn_beam(&env, &model, depth, beam_limit);
        let res = env.step(best_act);

        let earned = env.score_manager.total_score.saturating_sub(old_score);

        println!(
            "Turn {:>3} | Placed: {:>3}/{} | Action: ({:>2}, {:>2}, rot:{}) | Fit: {:>2} | +{:>3} pts | Tổng Điểm: {:>5} | Cọc: {:>2} | GNN Tiên Tri: {:>6.1} | {:.2}s",
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
            gnn_eval,
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

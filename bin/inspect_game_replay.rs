use std::fs;
use serde::Deserialize;
use dorfromantik_remake::env::{DorfromantikEnv, Action};
use dorfromantik_remake::game_config::GroupType;

#[derive(Debug, Deserialize)]
struct ReplayMove {
    step: usize,
    q: i32,
    r: i32,
    rotation: usize,
    score_gained: usize,
    total_score: usize,
    remaining_tiles: usize,
}

#[derive(Debug, Deserialize)]
struct GameRecord {
    seed: i32,
    total_score: usize,
    total_placed: usize,
    is_eval: bool,
    moves: Vec<ReplayMove>,
}

fn main() {
    let file_path = "temp_replay.json";
    let content = fs::read_to_string(file_path).expect("Cannot read temp_replay.json");
    let record: GameRecord = serde_json::from_str(&content).expect("Cannot parse temp_replay.json");

    println!("=========================================================================================");
    println!("               BÁO CÁO PHÂN TÍCH CHUYÊN SÂU REPLAY VÁN ĐẤU (4,860 ĐIỂM)");
    println!("=========================================================================================");
    println!(">>> Seed: {} | Tổng Điểm: {} | Số Bước: {} | Chế độ: {}", 
        record.seed, record.total_score, record.moves.len(), if record.is_eval { "Eval" } else { "Self-Play" });

    let mut env = DorfromantikEnv::new(record.seed, 10, 100);

    let mut total_matching_edges = 0;
    let mut total_mismatching_edges = 0;
    let mut step_perfects = Vec::new();
    let mut quest_completion_steps = Vec::new();
    let mut zero_or_low_score_steps = Vec::new();
    let mut score_milestones = Vec::new(); // (step, score, remaining)

    for (step_i, m) in record.moves.iter().enumerate() {
        let prev_score = env.score_manager.total_score;
        let prev_perfects = env.score_manager.perfect_count;

        let act = Action { q: m.q, r: m.r, rotation: m.rotation };
        let _step_res = env.step(act);

        let current_score = env.score_manager.total_score;
        let current_tiles = env.score_manager.remaining_tiles;
        let current_perfects = env.score_manager.perfect_count;
        let score_diff = current_score.saturating_sub(prev_score);
        let perfect_diff = current_perfects.saturating_sub(prev_perfects);

        if perfect_diff > 0 {
            step_perfects.push((step_i + 1, m.q, m.r, perfect_diff, score_diff));
        }

        if score_diff >= 100 {
            quest_completion_steps.push((step_i + 1, m.q, m.r, score_diff, current_score, current_tiles));
        }

        if score_diff <= 10 {
            zero_or_low_score_steps.push((step_i + 1, m.q, m.r, score_diff));
        }

        if step_i == 24 || step_i == 49 || step_i == 74 || step_i == 99 {
            score_milestones.push((step_i + 1, current_score, current_tiles));
        }
    }

    println!("\n1. 📈 TIẾN TRÌNH ĐIỂM SỐ & DỰ TRỮ CỌC BÀI QUA 4 CHẶNG:");
    println!("  {:<15} | {:<20} | {:<25}", "Chặng (Turns)", "Điểm Tích Lũy", "Cọc Bài Còn Lại");
    println!("  {:-<15}-|-{:-<20}-|-{:-<25}", "", "", "");
    for &(turn, sc, rem) in &score_milestones {
        println!("  Turn {:<10} | {:<20} | {:<25} tiles", turn, sc, rem);
    }

    println!("\n2. 💎 PHÂN TÍCH NGUỒN ĐIỂM & ĐỘ CHÍNH XÁC HÌNH HỌC:");
    println!("  - Tổng số lần ăn PERFECT được ScoreManager ghi nhận: {} lần", env.score_manager.perfect_count);
    for &(st, q, r, cnt, sc) in &step_perfects {
        println!("      • Turn {:<2} tại ({:>3}, {:>3}) -> Ăn {} Perfect (+{} điểm, hồi {} tile)", 
            st, q, r, cnt, cnt * 60, cnt);
    }

    // Quest Analysis at Final State
    println!("\n3. 🎯 PHÂN TÍCH TỶ LỆ HOÀN THÀNH QUEST (QUEST COMPLETION BREAKDOWN):");
    let mut total_quests = 0;
    let mut success_quests = 0;
    let mut failed_quests = 0;
    let mut incomplete_quests = 0;

    let mut quest_by_type: std::collections::HashMap<GroupType, (usize, usize, usize, usize)> = std::collections::HashMap::new(); // (total, success, failed, incomplete)
    let mut quest_by_eq: std::collections::HashMap<&'static str, (usize, usize, usize)> = std::collections::HashMap::new(); // (total, success, failed)

    for (&pos, pt) in &env.board.placed_tiles {
        if let dorfromantik_remake::tile::GeneratedTile::Quest { quest_data, .. } = &pt.tile {
            total_quests += 1;
            let gt = quest_data.primary_group_type();
            let eq_str = match quest_data.equality {
                dorfromantik_remake::tile::EqualityComparison::MoreThan => "MoreThan (>=)",
                dorfromantik_remake::tile::EqualityComparison::Exactly => "Exactly (==)",
            };

            let entry_type = quest_by_type.entry(gt).or_insert((0, 0, 0, 0));
            entry_type.0 += 1;

            let entry_eq = quest_by_eq.entry(eq_str).or_insert((0, 0, 0));
            entry_eq.0 += 1;

            match pt.quest_status {
                Some(dorfromantik_remake::board::FulfillmentStatus::Success) => {
                    success_quests += 1;
                    entry_type.1 += 1;
                    entry_eq.1 += 1;
                }
                Some(dorfromantik_remake::board::FulfillmentStatus::Failed) => {
                    failed_quests += 1;
                    entry_type.2 += 1;
                    entry_eq.2 += 1;
                }
                _ => {
                    incomplete_quests += 1;
                    entry_type.3 += 1;
                }
            }
        }
    }

    let success_rate = (success_quests as f32 / total_quests.max(1) as f32) * 100.0;
    println!("  - Tổng số Quest xuất hiện trong ván:   {} Quests", total_quests);
    println!("  - Số Quest HOÀN THÀNH THÀNH CÔNG (✅):   {}/{} ({:.1}%)", success_quests, total_quests, success_rate);
    println!("  - Số Quest BỊ THẤT BẠI/HỎNG (❌):       {}/{} ({:.1}%)", failed_quests, total_quests, (failed_quests as f32 / total_quests.max(1) as f32) * 100.0);
    println!("  - Số Quest ĐANG DANG DỞ (Incomplete):  {}/{} ({:.1}%)", incomplete_quests, total_quests, (incomplete_quests as f32 / total_quests.max(1) as f32) * 100.0);

    println!("\n  [Chi tiết theo loại địa hình]:");
    for (gt, (tot, succ, fail, inc)) in &quest_by_type {
        let sr = (*succ as f32 / *tot as f32) * 100.0;
        println!("    • {:<12}: {:>2} Quests | ✅ Hoàn thành: {:>2} ({:>5.1}%) | ❌ Hỏng: {:>2} | ⏳ Dở dang: {:>2}", 
            format!("{:?}", gt), tot, succ, sr, fail, inc);
    }

    println!("\n  [Chi tiết theo loại điều kiện]:");
    for (eq, (tot, succ, fail)) in &quest_by_eq {
        let sr = (*succ as f32 / *tot as f32) * 100.0;
        println!("    • {:<15}: {:>2} Quests | ✅ Hoàn thành: {:>2} ({:>5.1}%) | ❌ Hỏng: {:>2}", 
            eq, tot, succ, sr, fail);
    }

    println!("\n  [Danh sách 16 lượt nổ Quest lớn >= 100 điểm]:");
    for &(st, q, r, diff, total, rem) in &quest_completion_steps {
        println!("      • Turn {:<2} tại ({:>3}, {:>3}) -> +{:<3} điểm (Tổng: {:<4}, Cọc bài: {:<2})", 
            st, q, r, diff, total, rem);
    }

    // Terrain Groups Analysis at Final State
    println!("\n4. 🗺️ PHÂN TÍCH CẤU TRÚC QUẦN THỂ ĐỊA HÌNH KẾT THÚC (FINAL MAP STATE):");
    let mut water_groups = Vec::new();
    let mut forest_groups = Vec::new();
    let mut village_groups = Vec::new();
    let mut agri_groups = Vec::new();

    for (_gid, group) in &env.board.groups {
        match group.group_type {
            GroupType::Water => water_groups.push(group.total_element_count),
            GroupType::Forest => forest_groups.push(group.total_element_count),
            GroupType::Village => village_groups.push(group.total_element_count),
            GroupType::Agriculture => agri_groups.push(group.total_element_count),
            _ => {}
        }
    }
    water_groups.sort_unstable_by(|a, b| b.cmp(a));
    forest_groups.sort_unstable_by(|a, b| b.cmp(a));
    village_groups.sort_unstable_by(|a, b| b.cmp(a));
    agri_groups.sort_unstable_by(|a, b| b.cmp(a));

    println!("  - Quần thể Sông (Water): {} cụm | Lớn nhất: {:?} đoạn", water_groups.len(), water_groups.iter().take(5).collect::<Vec<_>>());
    println!("  - Quần thể Rừng (Forest): {} cụm | Lớn nhất: {:?} cây", forest_groups.len(), forest_groups.iter().take(5).collect::<Vec<_>>());
    println!("  - Quần thể Làng (Village): {} cụm | Lớn nhất: {:?} nhà", village_groups.len(), village_groups.iter().take(5).collect::<Vec<_>>());
    println!("  - Quần thể Lúa (Agriculture): {} cụm | Lớn nhất: {:?} ruộng", agri_groups.len(), agri_groups.iter().take(5).collect::<Vec<_>>());

    // Neighbor Count Spectrum at End of Game
    let mut neighbor_dist = [0usize; 7]; // 0..6
    let mut six_neighbor_tiles = Vec::new();
    for (&pos, _pt) in &env.board.placed_tiles {
        let (matches, closed) = env.score_manager.count_matching_edges(&env.board, pos.0, pos.1);
        neighbor_dist[closed] += 1;
        if closed == 6 {
            six_neighbor_tiles.push((pos, matches));
        }
    }

    println!("\n5. 🔍 TẠI SAO 0 LẦN PERFECT? - SOI CẤU TRÚC HÌNH HỌC BÀN CỜ:");
    println!("  - Phân bố số ô bao quanh của 100 tiles trên bàn cờ:");
    for (closed_cnt, &tile_cnt) in neighbor_dist.iter().enumerate() {
        println!("      • {} hàng xóm bao quanh: {:>2} tiles", closed_cnt, tile_cnt);
    }
    println!("  - Số ô ĐÃ ĐƯỢC BAO VẦY 6 HƯỚNG: {} ô", six_neighbor_tiles.len());
    for &(pos, matches) in &six_neighbor_tiles {
        println!("      • Ô tại ({}, {}) bị vây kín 6 hướng nhưng chỉ khớp {}/6 cạnh!", pos.0, pos.1, matches);
    }

    println!("\n6. ⚠️ PHÂN TÍCH CÁC NƯỚC ĐI LÃNG PHÍ / HIỆU QUẢ THẤP (<= 10 ĐIỂM):");
    println!("  - Số nước đi chỉ được <= 10 điểm: {}/100 nước ({:.1}%)", 
        zero_or_low_score_steps.len(), zero_or_low_score_steps.len() as f32);
    if let Some(&(st, q, r, _)) = zero_or_low_score_steps.iter().find(|s| s.3 == 0) {
        println!("  - CẢNH BÁO: Turn {} tại ({}, {}) bị 0 điểm (hoàn toàn không match cạnh nào)!", st + 1, q, r);
    }
    println!("=========================================================================================");
}

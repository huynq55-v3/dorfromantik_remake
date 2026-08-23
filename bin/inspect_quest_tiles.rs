use std::collections::{HashMap, VecDeque};
use std::env;
use std::fs;
use dorfromantik_remake::board::Board;
use dorfromantik_remake::game_config::GroupType;
use dorfromantik_remake::generator::TileGenerator;
use dorfromantik_remake::quest_manager::{initialize_active_quest_tile, QuestManager};
use dorfromantik_remake::tile::{BaseTile, EqualityComparison, GeneratedTile};

#[derive(Debug, Clone)]
pub struct MonthlyGameInfo {
    pub real_tile_seed: i32,
    pub config_string: String,
    pub village_probability: f32,
    pub forest_probability: f32,
    pub agriculture_probability: f32,
    pub water_probability: f32,
    pub train_track_probability: f32,
    pub tile_stack_height: usize,
    pub tile_limit: usize,
    pub density: f32,
    pub quest_probability: f32,
    pub quest_difficulty: f32,
    pub flag_quest_probability: f32,
    pub world_border_radius: i32,
}

impl MonthlyGameInfo {
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Self {
        let mut info = Self {
            real_tile_seed: -2093096630,
            config_string: "0820260cxQFZ1rZgBb".into(),
            village_probability: 125.0,
            forest_probability: 125.0,
            agriculture_probability: 125.0,
            water_probability: 1000.0,
            train_track_probability: 0.0,
            tile_stack_height: 10,
            tile_limit: 100,
            density: 1.4,
            quest_probability: 1.0,
            quest_difficulty: 2.0,
            flag_quest_probability: 0.3,
            world_border_radius: -1,
        };

        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('=') {
                    continue;
                }
                if let Some((key, val)) = trimmed.split_once('=') {
                    let key = key.trim();
                    let val = val.trim();
                    match key {
                        "REAL_TILE_SEED" => {
                            if let Ok(v) = val.parse::<i32>() {
                                info.real_tile_seed = v;
                            }
                        }
                        "CONFIG_STRING" => {
                            info.config_string = val.to_string();
                        }
                        "ACTIVE_VillageProbability" => {
                            if let Ok(v) = val.parse::<f32>() {
                                info.village_probability = v;
                            }
                        }
                        "ACTIVE_ForestProbability" => {
                            if let Ok(v) = val.parse::<f32>() {
                                info.forest_probability = v;
                            }
                        }
                        "ACTIVE_AgricultureProbability" => {
                            if let Ok(v) = val.parse::<f32>() {
                                info.agriculture_probability = v;
                            }
                        }
                        "ACTIVE_WaterProbability" => {
                            if let Ok(v) = val.parse::<f32>() {
                                info.water_probability = v;
                            }
                        }
                        "ACTIVE_TrainTrackProbability" => {
                            if let Ok(v) = val.parse::<f32>() {
                                info.train_track_probability = v;
                            }
                        }
                        "ACTIVE_TileStackHeight" => {
                            if let Ok(v) = val.parse::<usize>() {
                                info.tile_stack_height = v;
                            }
                        }
                        "ACTIVE_TileLimit" => {
                            if let Ok(v) = val.parse::<usize>() {
                                info.tile_limit = v;
                            }
                        }
                        "ACTIVE_Density" => {
                            if let Ok(v) = val.parse::<f32>() {
                                info.density = v;
                            }
                        }
                        "ACTIVE_QuestProbability" => {
                            if let Ok(v) = val.parse::<f32>() {
                                info.quest_probability = v;
                            }
                        }
                        "ACTIVE_QuestDifficulty" => {
                            if let Ok(v) = val.parse::<f32>() {
                                info.quest_difficulty = v;
                            }
                        }
                        "ACTIVE_FlagQuestProbability" => {
                            if let Ok(v) = val.parse::<f32>() {
                                info.flag_quest_probability = v;
                            }
                        }
                        "ACTIVE_WorldBorderRadius" => {
                            if let Ok(v) = val.parse::<i32>() {
                                info.world_border_radius = v;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        info
    }
}

fn group_type_emoji(gt: GroupType) -> &'static str {
    match gt {
        GroupType::Water => "💧 Water",
        GroupType::Forest => "🌲 Forest",
        GroupType::Agriculture => "🌾 Agriculture",
        GroupType::Village => "🏡 Village",
        GroupType::TrainTracks => "🚂 TrainTracks",
    }
}

fn group_type_badge(gt: GroupType) -> &'static str {
    match gt {
        GroupType::Water => "WATER",
        GroupType::Forest => "FOREST",
        GroupType::Agriculture => "AGRI",
        GroupType::Village => "VILLAGE",
        GroupType::TrainTracks => "TRAIN",
    }
}

fn main() {
    let monthly_info = MonthlyGameInfo::from_file("monthly_game_info.txt");

    let args: Vec<String> = env::args().collect();
    let seed = if args.len() > 1 {
        args[1].parse::<i32>().unwrap_or(monthly_info.real_tile_seed)
    } else {
        monthly_info.real_tile_seed
    };

    let total_count = if args.len() > 2 {
        args[2].parse::<usize>().unwrap_or(monthly_info.tile_limit)
    } else {
        monthly_info.tile_limit
    };

    println!("========================================================================================================");
    println!("             📜 BÁO CÁO PHÂN TÍCH TOÀN BỘ {} QUEST TILES ĐẦU TIÊN (GENERATOR SIMULATION)", total_count);
    println!("========================================================================================================");
    println!("📌 [CẤU HÌNH TRÍCH XUẤT TỪ monthly_game_info.txt]:");
    println!("   • REAL_TILE_SEED              : {}", seed);
    println!("   • CONFIG_STRING               : {}", monthly_info.config_string);
    println!("   • ACTIVE_TileLimit            : {}", monthly_info.tile_limit);
    println!("   • ACTIVE_TileStackHeight      : {}", monthly_info.tile_stack_height);
    println!("   • ACTIVE_QuestDifficulty      : {}", monthly_info.quest_difficulty);
    println!("   • ACTIVE_Density              : {}", monthly_info.density);
    println!("   • ACTIVE_QuestProbability     : {}", monthly_info.quest_probability);
    println!("   • ACTIVE_FlagQuestProbability : {}", monthly_info.flag_quest_probability);
    println!("   • ACTIVE_WorldBorderRadius    : {}", monthly_info.world_border_radius);
    println!("   • Probabilities               : Water={}, Forest={}, Agri={}, Village={}, Train={}",
        monthly_info.water_probability, monthly_info.forest_probability,
        monthly_info.agriculture_probability, monthly_info.village_probability,
        monthly_info.train_track_probability
    );

    // Khởi tạo giống 100% bin/simulator.rs
    let mut generator = TileGenerator::from_file(seed, "monthly_game_info.txt");
    let mut quest_manager = QuestManager::from_file("monthly_game_info.txt");
    let mut game_board = Board::new();

    println!("\n📌 [THÔNG SỐ GENERATOR KHỞI TẠO]:");
    println!("   • Tile Seed Increment Step    : {}", generator.tile_seed_increment_step);
    println!("   • At Least 2 Empty Edges Turns: {}", generator.at_least_two_empty_edges_for_x_turns);
    println!("========================================================================================================");

    // 1. Đặt tile xuất phát ban đầu ở tọa độ (0, 0) giống simulator.rs
    let initial_tile = GeneratedTile::Normal {
        base_tile: BaseTile::new(0, seed, "Initial Center Plain Tile"),
        segments: Vec::new(),
    };
    game_board.place_tile(0, 0, initial_tile, 0);

    // 2. Thiết lập Tile Queue buffer (3 preview tiles ban đầu giống simulator.rs)
    let mut tile_queue: VecDeque<GeneratedTile> = VecDeque::new();
    for _ in 0..3 {
        let active_count = quest_manager.pop_next_active_quest_count();
        // Ép overwrite_quest_prob = Some(1.0) để đảm bảo 100% là Quest Tile
        let t = generator.generate_tile(None, active_count, Some(1.0), quest_manager.level);
        tile_queue.push_back(t);
    }

    let mut quest_tiles_records = Vec::new();
    let mut prefab_counts: HashMap<String, usize> = HashMap::new();
    let mut group_counts: HashMap<GroupType, usize> = HashMap::new();
    let mut equality_counts: HashMap<&'static str, usize> = HashMap::new();
    let mut segment_counts: HashMap<String, usize> = HashMap::new();

    println!("\n{:<5} | {:<8} | {:<9} | {:<7} | {:<7} | {:<7} | {:<14} | {:<16} | {:<35}",
        "#", "Filter", "Group", "Cond", "Target", "Bubble", "Segments", "Quest Seed", "Prefab Name");
    println!("{:-<5}-|-{:-<8}-|-{:-<9}-|-{:-<7}-|-{:-<7}-|-{:-<7}-|-{:-<14}-|-{:-<16}-|-{:-<35}",
        "", "", "", "", "", "", "", "", "");

    for i in 1..=total_count {
        let is_early_filter = i <= generator.at_least_two_empty_edges_for_x_turns as usize;
        let filter_str = if is_early_filter { ">=2Empty" } else { "None" };

        // Lấy active tile ở đầu cọc bài (Front of Queue)
        let mut active_tile = tile_queue.pop_front().unwrap();

        // Kích hoạt / đăng ký QuestWatcher và TargetValue cho tile trên cùng cọc bài
        if let GeneratedTile::Quest { ref mut quest_data, .. } = active_tile {
            if quest_data.quest_id.is_none() {
                let qid = quest_manager.add_quest(&quest_data.quest_type);
                quest_data.quest_id = Some(qid);
            }
        }
        initialize_active_quest_tile(&mut active_tile, &game_board, &mut quest_manager);

        // Sinh tile tiếp theo vào cuối queue (giữ buffer 3 tiles) nếu chưa vượt quá tổng số cần sinh
        if generator.generated_tile_count < total_count as i32 {
            let active_count = quest_manager.pop_next_active_quest_count();
            let next_gen = generator.generate_tile(None, active_count, Some(1.0), quest_manager.level);
            tile_queue.push_back(next_gen);
        }

        if let GeneratedTile::Quest { ref quest_data, .. } = active_tile {
            let gt = quest_data.primary_group_type();
            let eq_str = match quest_data.equality {
                EqualityComparison::MoreThan => "+ (>=)",
                EqualityComparison::Exactly => "= (==)",
            };
            let eq_short = match quest_data.equality {
                EqualityComparison::MoreThan => "+",
                EqualityComparison::Exactly => "=",
            };

            let seg_str = quest_data.config_string();
            let prefab = quest_data.quest_type.clone();
            let remaining = quest_data.remaining_display_value();
            let target = quest_data.target_count;
            let quest_seed = quest_data.seed;

            *prefab_counts.entry(prefab.clone()).or_insert(0) += 1;
            *group_counts.entry(gt).or_insert(0) += 1;
            *equality_counts.entry(eq_str).or_insert(0) += 1;
            *segment_counts.entry(seg_str.clone()).or_insert(0) += 1;

            println!("{:<5} | {:<8} | {:<9} | {:<7} | {:<7} | {:<7} | {:<14} | {:<16} | {:<35}",
                format!("#{}", i),
                filter_str,
                group_type_badge(gt),
                format!("{} {}", eq_short, target),
                target,
                format!("💬 {}", remaining),
                seg_str,
                quest_seed,
                prefab
            );

            quest_tiles_records.push((i, filter_str, gt, quest_data.clone(), target, remaining));
        }
    }

    println!("========================================================================================================");
    println!("\n📊 1. PHÂN BỐ THEO NHÓM ĐỊA HÌNH (GROUP TYPE DISTRIBUTION):");
    let mut sorted_groups: Vec<_> = group_counts.into_iter().collect();
    sorted_groups.sort_by(|a, b| b.1.cmp(&a.1));
    for (gt, count) in sorted_groups {
        let pct = (count as f32 / total_count as f32) * 100.0;
        println!("  • {:<16}: {:>3}/{} tiles ({:>5.1}%)", group_type_emoji(gt), count, total_count, pct);
    }

    println!("\n🎯 2. PHÂN BỐ THEO LOẠI ĐIỀU KIỆN NHIỆM VỤ (EQUALITY CONDITION):");
    for (eq, count) in &equality_counts {
        let pct = (*count as f32 / total_count as f32) * 100.0;
        println!("  • {:<16}: {:>3}/{} tiles ({:>5.1}%)", eq, count, total_count, pct);
    }

    println!("\n🧩 3. DANH SÁCH TOÀN BỘ {} PREFAB QUEST KHÁ DĨ XUẤT HIỆN TRONG {} TILES:", prefab_counts.len(), total_count);
    let mut sorted_prefabs: Vec<_> = prefab_counts.into_iter().collect();
    sorted_prefabs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for (idx, (prefab, count)) in sorted_prefabs.iter().enumerate() {
        let pct = (*count as f32 / total_count as f32) * 100.0;
        println!("  {:>2}. {:<45} | Xuất hiện: {:>3} lần ({:>5.1}%)", idx + 1, prefab, count, pct);
    }

    println!("\n🗺️ 4. PHÂN BỐ CẤU TRÚC ĐỊA HÌNH SEGMENTS CỦA QUEST TILES:");
    let mut sorted_segs: Vec<_> = segment_counts.into_iter().collect();
    sorted_segs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for (idx, (seg, count)) in sorted_segs.iter().enumerate() {
        let pct = (*count as f32 / total_count as f32) * 100.0;
        println!("  {:>2}. Cấu hình '{:<15}' | Xuất hiện: {:>3} lần ({:>5.1}%)", idx + 1, seg, count, pct);
    }

    println!("\n🔍 5. ĐẶC ĐIỂM 5 TILES ĐẦU TIÊN (ÁP DỤNG BỘ LỌC AtLeastTwoEmptyEdges - occupiedEdges < 5):");
    for (i, filter, gt, qdata, target, rem) in quest_tiles_records.iter().take(5) {
        println!("  • Tile #{}: Filter='{}' | Nhóm={:?} | Prefab='{}' | Cấu hình='{}' | Điều kiện: {:?} {} (Bóng nhiệm vụ: {})",
            i, filter, gt, qdata.quest_type, qdata.config_string(), qdata.equality, target, rem);
    }

    println!("========================================================================================================\n");
}

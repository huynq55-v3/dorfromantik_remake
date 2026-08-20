use crate::game_config::{GroupType, QuestConfigurations, SegmentPresetConfigurations, SegmentType, TilePresetConfigurations};
use crate::tile::{BaseTile, GeneratedTile, QuestTileData, SegmentData};
use crate::unity_random::UnityRandom;
use std::sync::LazyLock;

/// Cache tĩnh — khởi tạo DUY NHẤT 1 lần cho toàn bộ chương trình.
/// Loại bỏ hoàn toàn fs::read_to_string() và heap allocation trong hot path MCTS.
static TILE_PRESETS: LazyLock<TilePresetConfigurations> = LazyLock::new(|| {
    TilePresetConfigurations::default()
});

static SEG_PRESETS: LazyLock<SegmentPresetConfigurations> = LazyLock::new(|| {
    SegmentPresetConfigurations::from_file("monthly_game_info.txt")
});

static QUEST_CONFIGS: LazyLock<QuestConfigurations> = LazyLock::new(|| {
    QuestConfigurations::from_file("monthly_game_info.txt")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileGenFilter {
    None,
    AtLeastTwoEmptyEdges,
}

#[derive(Debug, Clone)]
pub struct TileGenerator {
    pub tile_generation_seed: i32,
    pub generated_tile_count: i32,
    pub generated_quest_count: i32,
    pub tile_seed_increment_step: i32,
    pub at_least_two_empty_edges_for_x_turns: i32,
    pub base_tile_counter: usize,
    pub global_quest_probability_multiplier: f32,
    // SpecialTileSpawner (Train Station reward)
    pub last_rewarded_score: usize,
    pub last_rewarded_step: i32,
    pub score_treshold: usize,
    pub increase_amount: usize,
    pub increase_treshold: bool,
}

impl TileGenerator {
    pub fn new(seed: i32) -> Self {
        Self::from_file(seed, "monthly_game_info.txt")
    }

    pub fn from_file<P: AsRef<std::path::Path>>(seed: i32, path: P) -> Self {
        // Tính toán tile_seed_increment_step ngẫu nhiên từ Unity InitState LCG formula
        let mut rng = UnityRandom::init_state(seed);
        let tile_seed_increment_step = rng.range_i32(-100000, 100000);

        // Đọc ACTIVE_QuestProbability từ file cấu hình (mặc định = 1.0 nếu không tìm thấy)
        let mut global_quest_probability_multiplier = 1.0_f32;
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                if let Some((key, val)) = line.split_once('=') {
                    if key.trim() == "ACTIVE_QuestProbability" {
                        if let Ok(v) = val.trim().parse::<f32>() {
                            global_quest_probability_multiplier = v;
                        }
                    }
                }
            }
        }

        Self {
            tile_generation_seed: seed,
            generated_tile_count: 0,
            generated_quest_count: 0,
            tile_seed_increment_step,
            at_least_two_empty_edges_for_x_turns: 5,
            base_tile_counter: 0,
            global_quest_probability_multiplier,
            last_rewarded_score: 0,
            last_rewarded_step: 0,
            score_treshold: 2500,
            increase_amount: 25,
            increase_treshold: true,
        }
    }

    /// 1. Sinh BaseTile mộc ban đầu (giống GenerateBaseTile trong C#)
    pub fn generate_base_tile(&mut self, seed: i32, name_prefix: &str) -> BaseTile {
        let id = self.base_tile_counter;
        let name = format!("{} {}", name_prefix, id);
        self.base_tile_counter += 1;
        BaseTile {
            id,
            name,
            seed,
            is_generated: false,
        }
    }

    /// Kiểm tra xem có nên thưởng Train Station khi đạt ngưỡng điểm không.
    /// Logic khớp C# SpecialTileSpawner.CheckScoreCondition().
    pub fn should_grant_reward(&self, score: usize) -> bool {
        if self.increase_treshold {
            score > self.last_rewarded_score + self.score_treshold + self.last_rewarded_step as usize * self.increase_amount
        } else {
            score > self.last_rewarded_score + self.score_treshold
        }
    }

    /// Cấp thưởng Train Station: cập nhật state spawner + trả về tile station.
    pub fn grant_reward(&mut self) -> GeneratedTile {
        // C#: lastRewardedScore += scoreTreshold + lastRewardedStep * increaseAmount; lastRewardedStep++
        self.last_rewarded_score += self.score_treshold + self.last_rewarded_step as usize * self.increase_amount;
        self.last_rewarded_step += 1;

        let id = self.base_tile_counter;
        self.base_tile_counter += 1;
        let base = BaseTile::new(id, -1, "SpecialTile");
        GeneratedTile::Reward { base_tile: base }
    }

    /// 2. Bảng xác suất QuestTile theo số lượng Active Quest trong game
    pub fn quest_tile_probability(&self, active_quest_count: i32) -> f32 {
        let base_prob = match active_quest_count {
            ..=0 => 1.0000,
            1 => 0.5000,
            2 => 0.3280,
            3 => 0.2705,
            4 => 0.2210,
            5 => 0.1770,
            6 => 0.1356,
            7 => 0.0942,
            _ => 0.0500, // active_quest_count >= 8 luôn trả về 0.05 (5%) do Unity AnimationCurve PostWrapMode.Clamp
        };
        base_prob * self.global_quest_probability_multiplier
    }

    /// 3. Sinh QuestTile chuẩn theo 3 lượt quay ngẫu nhiên trọng số C# (có lọc excludedGroupTypes và usedFilter)
    pub fn generate_quest_tile(&mut self, quest_seed: i32, used_filter: TileGenFilter, level: usize) -> QuestTileData {
        let quest_seed_mul2 = quest_seed.wrapping_mul(2);
        let mut rng = UnityRandom::init_state(quest_seed_mul2);
        let quest_configs = &*QUEST_CONFIGS;

        // println!("\n=== GENERATE QUEST TILE DETAILS (Quest Seed: {}, InitState Seed: {}, Filter: {:?}) ===", quest_seed, quest_seed_mul2, used_filter);
        // println!("  [Excluded Group Types]: {:?}", quest_configs.excluded_group_types);

        // Roll 1: Chọn QuestCollection
        let col_options: Vec<_> = quest_configs
            .collections
            .iter()
            .map(|c| (c.clone(), c.probability))
            .collect();
        let (selected_col, _col_roll, col_ratio, col_total) = rng
            .select_weighted_info(&col_options)
            .unwrap_or_else(|| (quest_configs.collections[0].clone(), 0.0, 0.0, 0.0));
        // println!("  [Roll 1: Collection] CollProb: {:.8}, RawProb: {}, Total Prob: {}, Roll Val: {:.4}, Chosen: '{}'", col_ratio, selected_col.probability, col_total, col_roll, selected_col.name);

        // 1. Lọc danh sách SubCollection theo excludedGroupTypes (C# dòng 24421: loại bỏ subcollection chứa group_type bị cấm)
        let mut filtered_sub_cols: Vec<_> = selected_col
            .sub_collections
            .into_iter()
            .filter(|sub| {
                !sub.all_segment_types.iter().any(|gt| quest_configs.excluded_group_types.contains(gt))
            })
            .collect();

        // 2. Lọc danh sách SubCollection theo used_filter (C# dòng 24425: nếu AtLeastTwoEmptyEdges thì occupiedEdges < 5)
        filtered_sub_cols.retain(|sub| match used_filter {
            TileGenFilter::AtLeastTwoEmptyEdges => sub.occupied_edges < 5,
            TileGenFilter::None => true,
        });

        // Roll 2: Chọn QuestSubCollection từ danh sách đã lọc
        let sub_options: Vec<_> = filtered_sub_cols
            .iter()
            .map(|s| (s.clone(), s.probability))
            .collect();
        let (selected_sub, _sub_roll, _sub_ratio, sub_total) = rng
            .select_weighted_info(&sub_options)
            .unwrap_or_else(|| (filtered_sub_cols[0].clone(), 0.0, 0.0, 0.0));
        let _sub_coll_prob = if col_total == 0.0 || sub_total == 0.0 { 0.0 } else { (selected_sub.probability / sub_total) * col_ratio };
        // println!("  [Roll 2: SubCollection (Filtered)] subCollectionProbability: {:.8}, SubRawProb: {:.4}, Total Filtered SubProb: {:.4}, Roll Val: {:.4}, OccupiedEdges: {}", sub_coll_prob, selected_sub.probability, sub_total, sub_roll, selected_sub.occupied_edges);

        // Roll 3: Chọn QuestOption từ SubCollection đã chọn
        let opt_options: Vec<_> = selected_sub
            .quest_tiles
            .iter()
            .map(|o| (o.clone(), o.probability))
            .collect();
        let (selected_opt, _opt_roll, _opt_ratio, _opt_total) = rng
            .select_weighted_info(&opt_options)
            .unwrap_or_else(|| (selected_sub.quest_tiles[0].clone(), 0.0, 0.0, 0.0));
        // println!("  [Roll 3: Option] Prob Ratio: {:.8}, Total Prob: {}, Roll Val: {:.4}, Chosen Prefab: '{}'\n", opt_ratio, opt_total, opt_roll, selected_opt.prefab_name);

        let (equality, _) = crate::game_config::get_quest_prefab_condition_target_value_with_level(&selected_opt.prefab_name, selected_col.group_type, quest_seed, level);

        QuestTileData {
            seed: quest_seed,
            quest_type: selected_opt.prefab_name.clone(),
            target_count: 0,
            equality,
            level,
            quest_id: None,
            stack_quest_id: None,
        }
    }

    /// 4. Hàm generate_tile khớp 100% thứ tự thực thi trong C#
    pub fn generate_tile(
        &mut self,
        base_tile_opt: Option<BaseTile>,
        active_quest_count: i32,
        overwrite_quest_prob: Option<f32>,
        level: usize,
    ) -> GeneratedTile {
        let mut base_tile = base_tile_opt.unwrap_or_else(|| {
            self.generate_base_tile(-1, "Stacked Tile")
        });

        let before_tile_count = self.generated_tile_count;
        let before_quest_count = self.generated_quest_count;

        // Dòng 43168 C#: Tính num (Tile Seed) TRƯỚC KHI tăng generatedTileCount
        let num = self.tile_generation_seed
            .wrapping_add((before_tile_count - before_quest_count).wrapping_mul(self.tile_seed_increment_step));
        
        // Dòng 43169 C#: generatedTileCount++
        self.generated_tile_count += 1;
        base_tile.seed = num;
        base_tile.is_generated = true;

        // Dòng 43171 C#: TileGenFilter
        let used_filter = if self.generated_tile_count <= self.at_least_two_empty_edges_for_x_turns {
            TileGenFilter::AtLeastTwoEmptyEdges
        } else {
            TileGenFilter::None
        };

        // println!("\n================================----------------------------------");
        // println!("[TILE GENERATION LOG] Tile #{}", self.generated_tile_count);
        // println!("  - Generated Tile Count (Before): {}", before_tile_count);
        // println!("  - Generated Quest Count (Before): {}", before_quest_count);
        // println!("  - Active Quest Count (Board): {}", active_quest_count);
        // println!("  - Tile Seed (num): {}", num);
        // println!("Tile Generation Seed: {}, Tile Seed Increment Step: {}", self.tile_generation_seed, self.tile_seed_increment_step);

        let seed_quest_check = self.tile_generation_seed
            .wrapping_add(self.generated_tile_count.wrapping_mul(self.tile_seed_increment_step));
        // println!("Seed Quest Check: {}", seed_quest_check);

        let mut rng_quest = UnityRandom::init_state(seed_quest_check);
        let quest_roll = rng_quest.value();
        let quest_prob = overwrite_quest_prob.unwrap_or_else(|| self.quest_tile_probability(active_quest_count));

        let is_quest = quest_roll <= quest_prob;
        // println!("  - Quest Roll: {:.8} | Threshold (Prob): {:.8}", quest_roll, quest_prob);
        // println!("  ==> DECISION ROLL: {}", if is_quest { "QUEST TILE" } else { "NORMAL TILE" });

        if is_quest {
            // Dòng 43176 C#: Tính num2 (Quest Seed) dùng generatedQuestCount TRƯỚC KHI TĂNG
            let num2 = self.tile_generation_seed
                .wrapping_add(before_quest_count.wrapping_mul(self.tile_seed_increment_step));
            
            // Dòng 43178 C#: GeneratedQuestCount++
            self.generated_quest_count += 1;

            let quest_data = self.generate_quest_tile(num2, used_filter, level);
            // println!("  ==> ACTUAL RETURNED QUEST TILE: '{}'", quest_data.quest_type);
            // println!("================================----------------------------------\n");

            return GeneratedTile::Quest {
                base_tile,
                quest_data,
            };
        }

        // Nếu là Normal Tile
        let mut rng_tile = UnityRandom::init_state(num);
        let tile_configs = &*TILE_PRESETS;
        let seg_configs = &*SEG_PRESETS;

        // Lọc tilePresets theo used_filter (nếu AtLeastTwoEmptyEdges thì occupied_edges < 5)
        let filtered_presets: Vec<_> = tile_configs
            .all_tiles_flat
            .iter()
            .filter(|p| match used_filter {
                TileGenFilter::AtLeastTwoEmptyEdges => p.occupied_edges < 5,
                TileGenFilter::None => true,
            })
            .cloned()
            .collect();

        let preset_options: Vec<_> = filtered_presets
            .iter()
            .map(|p| (p.clone(), p.final_probability))
            .collect();

        let selected_preset = rng_tile
            .select_weighted(&preset_options)
            .unwrap_or_else(|| filtered_presets[0].clone());

        // println!("  ==> ACTUAL RETURNED TILE OBJECT: '{}'", selected_preset.name);

        fn segments_adjacent(edges1: &[usize], edges2: &[usize]) -> bool {
            for &e1 in edges1 {
                for &e2 in edges2 {
                    let diff = (e1 as i32 - e2 as i32).abs();
                    if diff == 1 || diff == 5 {
                        return true;
                    }
                }
            }
            false
        }

        fn rotations_to_fit_on_tile(base_edges: &[usize], occupied_tile_edges: &[usize]) -> Vec<usize> {
            let mut valid_rotations = Vec::new();
            let num = if !occupied_tile_edges.is_empty() {
                *occupied_tile_edges.iter().max().unwrap()
            } else {
                0
            };

            for i in 0..6 {
                let rot = (i + num) % 6;
                let rotated_edges: Vec<usize> = base_edges.iter().map(|&b| (b + rot) % 6).collect();
                let has_overlap = rotated_edges.iter().any(|e| occupied_tile_edges.contains(e));
                if !has_overlap {
                    valid_rotations.push(rot);
                }
            }
            valid_rotations
        }

        let mut segments: Vec<SegmentData> = Vec::new();

        for seg_type in &selected_preset.segments {
            let seg_type = *seg_type;
            let occupied_tile_edges: Vec<usize> = segments.iter().flat_map(|s| s.occupied_edges.iter().cloned()).collect();
            let valid_rotations = rotations_to_fit_on_tile(seg_type.base_edges(), &occupied_tile_edges);
            let rotation = if !valid_rotations.is_empty() {
                let idx = rng_tile.range_usize(0, valid_rotations.len());
                valid_rotations[idx]
            } else {
                rng_tile.range_usize(0, 6)
            };
            let edges: Vec<usize> = seg_type.base_edges().iter().map(|&b| (b + rotation) % 6).collect();

            let seg_preset_opt = seg_configs
                .all_segment_presets
                .iter()
                .find(|s| s.segment_type == seg_type);

            let group_type = if let Some(seg_preset) = seg_preset_opt {
                let mut group_options: Vec<_> = seg_preset
                    .possible_types
                    .iter()
                    .filter(|gt| gt.probability_in_percent > 0.0)
                    .map(|gt| (gt.group_type, gt.probability_in_percent))
                    .collect();

                // Dòng 43242 C#: Loại bỏ GroupType của các segment kề cạnh (SegmentsAdjacent)
                for existing_seg in &segments {
                    if segments_adjacent(&existing_seg.occupied_edges, &edges) {
                        group_options.retain(|(gt, _)| *gt != existing_seg.group_type);
                    }
                }

                rng_tile
                    .select_weighted(&group_options)
                    .unwrap_or(GroupType::Agriculture)
            } else {
                GroupType::Agriculture
            };

            // Dòng 43262 C#: Unity gọi Random.value để kiểm tra hybrid variant cho mỗi segment
            let value2 = rng_tile.value();
            let is_hybrid = if group_type == GroupType::Water {
                match seg_type {
                    SegmentType::ST6A => true,
                    SegmentType::ST2A => value2 <= 0.5,
                    SegmentType::ST3A => value2 <= 0.9,
                    SegmentType::ST4A => value2 <= 0.9,
                    SegmentType::ST5A => value2 <= 0.95,
                    _ => false,
                }
            } else {
                false
            };

            segments.push(SegmentData {
                index: segments.len(),
                group_type,
                segment_type: seg_type,
                occupied_edges: edges,
                rotation,
                is_hybrid,
            });
        }

        // println!("      [NORMAL TILE DETAILS] Total Segments: {}", segments.len());
        // for (idx, seg) in segments.iter().enumerate() {
        //     println!("        - Segment #{}: GroupType = {:?}, SegmentType = {:?}, Rotation = {}, Edges = {:?}",
        //         idx, seg.group_type, seg.segment_type, seg.rotation, seg.occupied_edges);
        // }
        // println!("================================----------------------------------\n");

        GeneratedTile::Normal {
            base_tile,
            segments,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile1_generation() {
        let mut gen = TileGenerator::new(-2093096630);
        let mut quest_count = 0;
        for i in 1..=22 {
            let tile = gen.generate_tile(None, quest_count, None, 0);
            if let GeneratedTile::Quest { ref quest_data, .. } = tile {
                quest_count += 1;
                println!("Tile #{}: Prefab='{}', Equality={:?}", i, quest_data.quest_type, quest_data.equality);
            }
        }
    }
}

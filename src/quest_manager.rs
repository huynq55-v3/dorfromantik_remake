use crate::game_config::GroupType;
use crate::tile::QuestTileData;

/// QuestManager quản lý danh sách nhiệm vụ và tính toán TargetValue theo công thức C# gốc
pub struct QuestManager {
    pub global_difficulty_multiplier: f32,
    pub exponential_difficulty_factor: f32,
    pub levels_needed_per_increase: usize,
    pub target_value_increase: f32,
    pub level: usize,
}

impl Default for QuestManager {
    fn default() -> Self {
        Self::new()
    }
}

impl QuestManager {
    pub fn new() -> Self {
        Self::from_file("monthly_game_info.txt")
    }

    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Self {
        let mut global_difficulty_multiplier = 1.0;

        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                if let Some((key, val)) = line.split_once('=') {
                    let key = key.trim();
                    let val = val.trim();
                    if key == "ACTIVE_QuestDifficulty" {
                        if let Ok(num) = val.parse::<f32>() {
                            global_difficulty_multiplier = num;
                        }
                    }
                }
            }
        }

        Self {
            global_difficulty_multiplier,
            exponential_difficulty_factor: 1.0,
            levels_needed_per_increase: 1,
            target_value_increase: 0.33333334,
            level: 0,
        }
    }

    /// Trả về số lượng object/segment của cụm địa hình liên thông trên bàn chơi (ReferenceGroupCount)
    pub fn reference_group_count(&self, _group_type: GroupType) -> usize {
        // Bản thân QuestTile khi sinh ra đã chứa ít nhất 1 cụm địa hình ban đầu trên tile.
        // Khi chưa ghép cụm nào lớn hơn trên bàn chơi, ReferenceGroupCount = 1.
        1
    }

    /// Trả về targetValueIncrease chuẩn theo từng loại địa hình (Unity Prefab Asset Config):
    /// - Ruộng (Agriculture), Rừng (Forest), Làng (Village): 1/3 (0.33333334)
    /// - Sông (Water), Đường ray (TrainTracks): 1/6 (0.16666667)
    pub fn target_value_increase(&self, group_type: GroupType) -> f32 {
        match group_type {
            GroupType::Water | GroupType::TrainTracks => 0.16666667,
            _ => 0.33333334,
        }
    }

    /// Công thức tính mức tăng độ khó theo level (Dorfromantik2.cs dòng 22354):
    /// DifficultyIncrease = Round( (Level ^ ExponentialFactor) / levelsNeeded * targetValueIncrease * Multipliers )
    pub fn difficulty_increase(&self, group_type: GroupType) -> usize {
        if self.level == 0 {
            return 0;
        }

        let target_increase = self.target_value_increase(group_type);
        let level_f = self.level as f32;
        let pow_level = level_f.powf(self.exponential_difficulty_factor);
        let increase = (pow_level / self.levels_needed_per_increase as f32)
            * target_increase
            * self.global_difficulty_multiplier;

        increase.round() as usize
    }

    /// Công thức chuẩn C# gốc (Dorfromantik2.cs dòng 22854):
    /// TargetValue = max(ReferenceGroupCount, minTargetCount) + condition.targetValue + DifficultyIncrease
    pub fn calculate_target_value(
        &self,
        quest_tile: &QuestTileData,
        min_target_count: usize,
        condition_target_value: usize,
    ) -> usize {
        let group_type = quest_tile.primary_group_type();
        let ref_count = self.reference_group_count(group_type);
        let base = std::cmp::max(ref_count, min_target_count);
        let diff = self.difficulty_increase(group_type);

        base + condition_target_value + diff
    }
}

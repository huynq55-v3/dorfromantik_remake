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
        Self {
            global_difficulty_multiplier: 1.0,
            exponential_difficulty_factor: 1.0,
            levels_needed_per_increase: 1,
            target_value_increase: 1.0,
            level: 0, // Level 0 khi mới bắt đầu game
        }
    }

    /// Trả về số lượng object/segment của cụm địa hình liên thông trên bàn chơi (ReferenceGroupCount)
    pub fn reference_group_count(&self, _group_type: GroupType) -> usize {
        // Khi bàn chơi chưa có cụm hoặc vừa bắt đầu game -> trả về 0.
        // Khi ghép thêm tile trên bàn chơi (Board / ElementGroupManager), hàm này sẽ đếm cụm lớn nhất.
        0
    }

    /// Công thức tính mức tăng độ khó theo level (Dorfromantik2.cs dòng 22354):
    /// DifficultyIncrease = Round( (Level ^ ExponentialFactor) / levelsNeeded * targetValueIncrease * Multipliers )
    pub fn difficulty_increase(&self, _group_type: GroupType) -> usize {
        if self.level == 0 {
            return 0;
        }

        let level_f = self.level as f32;
        let pow_level = level_f.powf(self.exponential_difficulty_factor);
        let increase = (pow_level / self.levels_needed_per_increase as f32)
            * self.target_value_increase
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

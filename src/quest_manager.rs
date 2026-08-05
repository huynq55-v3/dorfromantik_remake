use std::collections::VecDeque;
use crate::game_config::GroupType;
use crate::tile::QuestTileData;

#[derive(Debug, Clone)]
pub struct ActiveQuest {
    pub quest_id: usize,
    pub quest_type: String,
    pub counts_towards_limit: bool,
}

/// QuestManager quản lý danh sách nhiệm vụ và tính toán TargetValue theo công thức C# gốc
#[derive(Debug, Clone)]
pub struct QuestManager {
    pub global_difficulty_multiplier: f32,
    pub exponential_difficulty_factor: f32,
    pub levels_needed_per_increase: usize,
    pub target_value_increase: f32,
    pub level: usize,

    // Quản lý các Active Quest đang mở trên bàn chơi / trong stack (Dorfromantik2.cs:24124)
    pub active_quests: Vec<ActiveQuest>,
    next_quest_id: usize,

    // ── Active Quest Count Formula (3-step lag) ──
    // Công thức: active_quest_count_for_tile(n+3) =
    //     prev_active
    //     + (tile[n] là Quest ? +2 : 0)
    //     - (tile[n-1] là Quest ? -1 : 0)
    //     - m  (số quest hoàn thành/thất bại trong lượt n)
    //
    // - 3 tile đầu tiên (index 0,1,2) → count = 0 (khởi tạo sẵn)
    // - Khi đặt tile thứ n → tính count cho tile n+3 và push vào queue
    // - Khi generate tile k → pop từ front queue (count đã tính từ 3 bước trước)
    upcoming_quest_counts: VecDeque<i32>,
    /// Giá trị count được tính và push gần nhất (dùng làm prev_active cho lượt tiếp)
    /// Tách riêng khỏi queue vì queue có thể bị pop sạch trước khi update_after_placement chạy
    last_computed_count: i32,
    /// Tile vừa đặt có phải Quest không (dùng cho công thức lượt kế tiếp)
    prev_tile_was_quest: bool,
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
            active_quests: Vec::new(),
            next_quest_id: 1,
            // 3 tile đầu tiên dùng count = 0
            upcoming_quest_counts: VecDeque::from([0, 0, 0]),
            last_computed_count: 0,
            prev_tile_was_quest: false,
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

    /// Kiểm tra xem một prefab Quest Tile có được tính vào giới hạn ActiveQuestCount hay không (countsTowardsQuestLimit C#)
    /// Tất cả Quest tiêu chuẩn (kể cả Quest Thuyền Boat) đều được tính. Chỉ có Unlock/Challenge/Crown quests mới bị trừ ra.
    pub fn counts_towards_quest_limit(quest_type: &str) -> bool {
        let lower = quest_type.to_lowercase();
        if lower.contains("unlock") || lower.contains("challenge") || lower.contains("crown") {
            return false;
        }
        true
    }

    /// Trả về số lượng ActiveQuestCount đang mở chuẩn theo C# (Dorfromantik2.cs:24124)
    /// Đây là đếm nội bộ active_quests vec, KHÔNG dùng cho quest_tile_probability.
    pub fn active_quest_count(&self) -> i32 {
        self.active_quests
            .iter()
            .filter(|q| q.counts_towards_limit)
            .count() as i32
    }

    /// Lấy active_quest_count để truyền vào generate_tile() cho tile tiếp theo.
    /// Dùng TRƯỚC khi generate tile, tuân theo công thức 3-step lag.
    /// Trả về 0 nếu queue rỗng (vượt quá giới hạn — không nên xảy ra).
    pub fn pop_next_active_quest_count(&mut self) -> i32 {
        self.upcoming_quest_counts.pop_front().unwrap_or(0)
    }

    /// Cập nhật active_quest_count sau khi đặt tile thứ n xuống bàn.
    /// Tính và enqueue count cho tile n+3.
    ///
    /// - `is_quest`: tile thứ n vừa đặt có phải QuestTile không
    /// - `quests_resolved`: m = số quest hoàn thành/thất bại trong lượt này
    pub fn update_after_placement(&mut self, is_quest: bool, quests_resolved: usize) {
        // prev = giá trị count đã tính ở lượt trước (KHÔNG dùng queue.back()
        // vì queue có thể đã bị pop sạch trước khi hàm này được gọi)
        let prev = self.last_computed_count;
        let plus_quest = if is_quest { 2 } else { 0 };
        let minus_prev_quest = if self.prev_tile_was_quest { 1 } else { 0 };
        let new_count = (prev + plus_quest - minus_prev_quest - quests_resolved as i32).max(0);
        self.last_computed_count = new_count;
        self.upcoming_quest_counts.push_back(new_count);
        self.prev_tile_was_quest = is_quest;
    }

    /// Thêm một Quest mới khi được sinh ra (AddQuest C# dòng 24171)
    pub fn add_quest(&mut self, quest_type: &str) -> usize {
        let quest_id = self.next_quest_id;
        self.next_quest_id += 1;
        let counts_towards_limit = Self::counts_towards_quest_limit(quest_type);
        self.active_quests.push(ActiveQuest {
            quest_id,
            quest_type: quest_type.to_string(),
            counts_towards_limit,
        });
        quest_id
    }

    /// Xóa một Quest khi hoàn thành/đóng nhiệm vụ (RemoveQuest C# dòng 24201)
    pub fn remove_quest(&mut self, quest_id: usize) {
        self.active_quests.retain(|q| q.quest_id != quest_id);
    }

    /// Xóa sạch tất cả quest active khi reset bàn chơi (Clear C# dòng 24270)
    pub fn clear(&mut self) {
        self.active_quests.clear();
    }

    /// Công thức tính chuẩn C# cho TargetValue (Dorfromantik2.cs dòng 22854):
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

use crate::board::{Board, get_neighbor_pos, opposite_direction};
use crate::tile::EdgeType;

/// Kiểm tra 2 loại cạnh có ghép khớp địa hình với nhau không (được cộng điểm Fit Edge)
/// Theo đúng quy tắc C# class Tile trong Dorfromantik2.cs:
/// 1. Cùng loại cạnh (Plain-Plain, Forest-Forest, Village-Village, Agri-Agri, Water-Water, Train-Train, FlexibleWater-FlexibleWater, Station-Station) -> MATCH
/// 2. FlexibleWater (Lake hybrid) ghép với Plain (đồng cỏ / cạnh trống), Water (nước cứng), WaterTrainStation -> MATCH
/// 3. WaterTrainStation (Station hybrid) ghép với Plain (đồng cỏ / cạnh trống), Water, TrainTracks, FlexibleWater -> MATCH
pub fn is_matching_edge(a: EdgeType, b: EdgeType) -> bool {
    match (a, b) {
        // 1. Cùng loại cạnh
        (x, y) if x == y => true,

        // 2. FlexibleWater (Lake hybrid) ghép với Plain (đồng cỏ), Water (nước cứng), WaterTrainStation
        (EdgeType::FlexibleWater, EdgeType::Plain | EdgeType::Water | EdgeType::WaterTrainStation)
        | (EdgeType::Plain | EdgeType::Water | EdgeType::WaterTrainStation, EdgeType::FlexibleWater) => true,

        // 3. WaterTrainStation (Station hybrid) ghép với Plain (đồng cỏ), Water, TrainTracks
        (EdgeType::WaterTrainStation, EdgeType::Plain | EdgeType::Water | EdgeType::TrainTracks)
        | (EdgeType::Plain | EdgeType::Water | EdgeType::TrainTracks, EdgeType::WaterTrainStation) => true,

        _ => false,
    }
}

/// Chi tiết điểm thưởng và phần thưởng thu thập được trong 1 lượt đặt tile
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlacementScoreBreakdown {
    /// Số lượng cạnh khớp trên tile vừa đặt
    pub matching_edges: usize,
    /// Điểm thưởng ghép cạnh (matching_edges * 10)
    pub fit_score: usize,
    /// Số lượng ô đạt Perfect Placement (tile vừa đặt và/hoặc các ô hàng xóm lân cận vừa bị vây kín 6 hướng hoàn hảo)
    pub perfect_count: usize,
    /// Điểm Perfect (perfect_count * 60)
    pub perfect_score: usize,
    /// Số lượt Quest Bubble hoàn thành
    pub bubble_quests_completed: usize,
    /// Điểm thưởng Quest Bubble (bubble_quests_completed * 100)
    pub bubble_quest_score: usize,
    /// Số lượt Quest Flag hoàn thành
    pub flag_quests_completed: usize,
    /// Điểm thưởng Quest Flag (flag_quests_completed * 50)
    pub flag_quest_score: usize,
    /// Điểm thưởng đóng kín cụm địa hình
    pub group_completion_score: usize,
    /// Tổng điểm cộng thêm trong lượt này
    pub total_score_gained: usize,
    /// Số tile thưởng được cộng thêm vào cọc bài (+1 tile / perfect, +5 tiles / quest)
    pub bonus_tiles_gained: usize,
}

/// Quản lý điểm số và cọc bài (Tile Stack) cho Simulator
#[derive(Debug, Clone)]
pub struct ScoreManager {
    pub total_score: usize,
    pub placed_tiles_count: usize,
    pub perfect_count: usize,
    pub consecutive_perfects: usize,
    pub remaining_tiles: usize,
    pub is_game_over: bool,

    // Cấu hình điểm số
    pub fit_score_per_edge: usize,
    pub perfect_placement_score: usize,
    pub perfect_placement_tile_reward: usize,
    pub quest_bubble_score: usize,
    pub quest_bubble_tile_reward: usize,
    pub quest_flag_score: usize,
}

impl Default for ScoreManager {
    fn default() -> Self {
        Self::new(10)
    }
}

impl ScoreManager {
    pub fn new(initial_stack_height: usize) -> Self {
        Self {
            total_score: 0,
            placed_tiles_count: 0,
            perfect_count: 0,
            consecutive_perfects: 0,
            remaining_tiles: initial_stack_height,
            is_game_over: false,

            fit_score_per_edge: 10,
            perfect_placement_score: 60,
            perfect_placement_tile_reward: 1, // Perfect thưởng +1 tile vào cọc
            quest_bubble_score: 100,
            quest_bubble_tile_reward: 5,     // Quest thưởng +5 tiles vào cọc
            quest_flag_score: 50,
        }
    }

    /// Đếm số cạnh khớp và số ô hàng xóm đã đóng xung quanh ô (q, r)
    pub fn count_matching_edges(&self, board: &Board, q: i32, r: i32) -> (usize, usize) {
        let placed_tile = match board.placed_tiles.get(&(q, r)) {
            Some(pt) => pt,
            None => return (0, 0),
        };

        let mut matching = 0;
        let mut closed_neighbors = 0;

        for dir in 0..6 {
            let neighbor_pos = get_neighbor_pos(q, r, dir);
            if let Some(neighbor) = board.placed_tiles.get(&neighbor_pos) {
                closed_neighbors += 1;
                let my_edge = placed_tile.edge_config.edges[dir];
                let neighbor_edge = neighbor.edge_config.edges[opposite_direction(dir)];
                if is_matching_edge(my_edge, neighbor_edge) {
                    matching += 1;
                }
            }
        }

        (matching, closed_neighbors)
    }

    /// Đánh giá điểm số và cập nhật cọc bài ngay sau khi một tile được đặt thành công lên bàn chơi
    pub fn on_tile_placed(
        &mut self,
        board: &Board,
        placed_q: i32,
        placed_r: i32,
        bubble_quests_completed: usize,
        flag_quests_completed: usize,
    ) -> PlacementScoreBreakdown {
        let mut breakdown = PlacementScoreBreakdown::default();

        if self.is_game_over {
            return breakdown;
        }

        self.placed_tiles_count += 1;
        if self.remaining_tiles > 0 {
            self.remaining_tiles -= 1;
        }

        // 1. Tính điểm ghép cạnh (Tile Fit Score) cho ô vừa đặt
        let (matching_edges, closed_neighbors) = self.count_matching_edges(board, placed_q, placed_r);
        breakdown.matching_edges = matching_edges;
        if matching_edges > 0 {
            breakdown.fit_score = matching_edges * self.fit_score_per_edge;
        }

        // 2. Kiểm tra Perfect Placement cho chính ô vừa đặt (được bao quanh 6 hướng VÀ 6 cạnh đều khớp)
        if closed_neighbors == 6 && matching_edges == 6 {
            breakdown.perfect_count += 1;
            breakdown.perfect_score += self.perfect_placement_score;
            breakdown.bonus_tiles_gained += self.perfect_placement_tile_reward;
        }

        // 3. Kiểm tra Perfect Placement cho các ô hàng xóm lân cận vừa bị bao kín 6 hướng nhờ ô vừa đặt
        for dir in 0..6 {
            let neighbor_pos = get_neighbor_pos(placed_q, placed_r, dir);
            if board.placed_tiles.contains_key(&neighbor_pos) {
                let (n_matching, n_closed) = self.count_matching_edges(board, neighbor_pos.0, neighbor_pos.1);
                // Ô hàng xóm vừa mới được vây kín 6 hướng và khớp cả 6 cạnh
                if n_closed == 6 && n_matching == 6 {
                    breakdown.perfect_count += 1;
                    breakdown.perfect_score += self.perfect_placement_score;
                    breakdown.bonus_tiles_gained += self.perfect_placement_tile_reward;
                }
            }
        }

        // 4. Tính điểm Quest và phần thưởng tile
        if bubble_quests_completed > 0 {
            breakdown.bubble_quests_completed = bubble_quests_completed;
            breakdown.bubble_quest_score = bubble_quests_completed * self.quest_bubble_score;
            breakdown.bonus_tiles_gained += bubble_quests_completed * self.quest_bubble_tile_reward;
        }

        if flag_quests_completed > 0 {
            breakdown.flag_quests_completed = flag_quests_completed;
            breakdown.flag_quest_score = flag_quests_completed * self.quest_flag_score;
        }

        // 5. Cộng tổng điểm và cộng số tile thưởng vào cọc bài
        breakdown.total_score_gained = breakdown.fit_score
            + breakdown.perfect_score
            + breakdown.bubble_quest_score
            + breakdown.flag_quest_score
            + breakdown.group_completion_score;

        self.total_score += breakdown.total_score_gained;
        self.perfect_count += breakdown.perfect_count;

        if breakdown.perfect_count > 0 {
            self.consecutive_perfects += breakdown.perfect_count;
        } else {
            self.consecutive_perfects = 0;
        }

        self.remaining_tiles += breakdown.bonus_tiles_gained;

        // 6. Kiểm tra Game Over nếu cọc bài về 0
        if self.remaining_tiles == 0 {
            self.is_game_over = true;
        }

        breakdown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_matching_edge_rules() {
        // Plain matches Plain, FlexibleWater, WaterTrainStation
        assert!(is_matching_edge(EdgeType::Plain, EdgeType::Plain));
        assert!(is_matching_edge(EdgeType::Plain, EdgeType::FlexibleWater));
        assert!(is_matching_edge(EdgeType::Plain, EdgeType::WaterTrainStation));
        assert!(!is_matching_edge(EdgeType::Plain, EdgeType::Forest));
        assert!(!is_matching_edge(EdgeType::Plain, EdgeType::Water));

        // FlexibleWater (Lake hybrid) matches Water, FlexibleWater, WaterTrainStation, Plain
        assert!(is_matching_edge(EdgeType::FlexibleWater, EdgeType::Plain));
        assert!(is_matching_edge(EdgeType::FlexibleWater, EdgeType::Water));
        assert!(is_matching_edge(EdgeType::FlexibleWater, EdgeType::FlexibleWater));
        assert!(is_matching_edge(EdgeType::FlexibleWater, EdgeType::WaterTrainStation));
        assert!(!is_matching_edge(EdgeType::FlexibleWater, EdgeType::Forest));

        // WaterTrainStation matches Plain, Water, TrainTracks, FlexibleWater, WaterTrainStation
        assert!(is_matching_edge(EdgeType::WaterTrainStation, EdgeType::Plain));
        assert!(is_matching_edge(EdgeType::WaterTrainStation, EdgeType::Water));
        assert!(is_matching_edge(EdgeType::WaterTrainStation, EdgeType::TrainTracks));
        assert!(is_matching_edge(EdgeType::WaterTrainStation, EdgeType::FlexibleWater));
        assert!(is_matching_edge(EdgeType::WaterTrainStation, EdgeType::WaterTrainStation));
        assert!(!is_matching_edge(EdgeType::WaterTrainStation, EdgeType::Village));
    }
}

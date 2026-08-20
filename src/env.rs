use std::collections::VecDeque;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use crate::board::{Board, get_neighbor_pos};
use crate::game_config::GroupType;
use crate::generator::TileGenerator;
use crate::quest_manager::QuestManager;
use crate::score_manager::ScoreManager;
use crate::tile::{EqualityComparison, GeneratedTile};

/// Node feature indices (70 dims). Định nghĩa tường minh để tránh magic numbers.
pub mod node_feat {
    pub const IS_PLACED: usize = 0;
    pub const IS_CANDIDATE: usize = 1;
    pub const EDGE_TERRAIN_START: usize = 2;   // 6 edges
    pub const OPEN_EDGE_START: usize = 8;      // 6 flags
    pub const EFFECTIVE_REMAINING_TILES: usize = 14; // Số tile thực sự còn lại (min giữa cọc bài và giới hạn game)
    pub const GROUP_OPEN_EDGES: usize = 15;
    pub const QUEST_ACTIVE: usize = 16;
    pub const QUEST_GROUP_TYPE_START: usize = 17; // 5 groups
    pub const QUEST_EQUALITY_MORE: usize = 22;
    pub const QUEST_EQUALITY_EXACTLY: usize = 23;
    pub const QUEST_REMAINING: usize = 24;
    pub const CURRENT_SCORE_DIV_10: usize = 25; // Điểm số hiện tại chia 10 (đơn vị gốc của game)
    pub const UPCOMING_TILE1_START: usize = 27;   // 6 edges
    pub const UPCOMING_TILE2_START: usize = 33;   // 6 edges
    pub const STEP_RATIO: usize = 39;
    pub const EDGE_FEATURES_START: usize = 40;    // 6 edges × 5 groups
    pub const DIM: usize = 70;
}

/// Action feature indices (16 dims).
pub mod action_feat {
    pub const MATCHING_COUNT: usize = 0;
    pub const MISMATCHING_COUNT: usize = 1;
    pub const CURR_REMAINING: usize = 2;
    pub const NEIGHBOR_COUNT: usize = 3;
    pub const QUEST_ADJ: usize = 4;
    pub const CURR_EQUALITY_MORE: usize = 5;
    pub const IS_QUEST_TILE: usize = 6;
    pub const ROTATION: usize = 7;
    pub const EDGE_TYPES_START: usize = 8;  // 6 edge types
    pub const POS_Q: usize = 14;
    pub const POS_R: usize = 15;
    pub const DIM: usize = 16;
}

/// Hành động đặt tile trong môi trường RL
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Action {
    pub q: i32,
    pub r: i32,
    pub rotation: usize, // 0..6
}

/// Kết quả thu được sau mỗi bước đi (Step Result)
#[derive(Debug, Clone)]
pub struct StepResult {
    pub reward: f32,
    pub done: bool,
    pub total_score: usize,
    pub placed_count: usize,
    pub stack_height: usize,
    pub breakdown: crate::score_manager::PlacementScoreBreakdown,
}

/// Trạng thái Đồ thị cho GNN Feature Extraction
#[derive(Debug, Clone)]
pub struct GraphObservation {
    /// Danh sách vị trí tọa độ của tất cả các node trong đồ thị (Placed + Candidates)
    pub node_positions: Vec<(i32, i32)>,
    /// Tensor đặc trưng của các node: [N, 70]
    pub node_features: Vec<[f32; 70]>,
    /// Danh sách các cạnh nối giữa các node kề nhau: Vec<(from_idx, to_idx)>
    pub edge_index: Vec<(usize, usize)>,
    /// Danh sách tất cả các Action hợp lệ ở bước đi hiện tại
    pub valid_actions: Vec<Action>,
    /// Đặc trưng Hình học Tường minh (Explicit Geometric Features) cho từng valid action: [Num_Actions, 16]
    pub action_features: Vec<[f32; 16]>,
}

/// Môi trường RL Headless cho Dorfromantik
#[derive(Debug, Clone)]
pub struct DorfromantikEnv {
    pub seed: i32,
    pub initial_stack: usize,
    pub tile_limit: usize,
    pub board: Board,
    pub generator: TileGenerator,
    pub quest_manager: QuestManager,
    pub score_manager: ScoreManager,
    pub tile_queue: VecDeque<GeneratedTile>,
    pub placed_count: usize,
}

/// Trạng thái lưu tạm để undo 1 bước step() — không clone placed_tiles.
#[derive(Debug, Clone)]
pub struct EnvUndoState {
    pub board_undo: crate::board::BoardUndoState,
    pub action_q: Option<i32>,
    pub action_r: Option<i32>,
    pub placed_count: usize,
    pub total_score: usize,
    pub remaining_tiles: usize,
    pub is_game_over: bool,
    pub placed_tiles_count: usize,
    pub perfect_count: usize,
    pub consecutive_perfects: usize,
    pub quest_manager_clone: QuestManager,
    pub generator_clone: TileGenerator,
    pub tile_queue_clone: VecDeque<GeneratedTile>,
}

impl DorfromantikEnv {
    pub fn new(seed: i32, initial_stack: usize, tile_limit: usize) -> Self {
        let mut env = Self {
            seed,
            initial_stack,
            tile_limit,
            board: Board::new(),
            generator: TileGenerator::new(seed),
            quest_manager: QuestManager::new(),
            score_manager: ScoreManager::new(initial_stack),
            tile_queue: VecDeque::new(),
            placed_count: 0,
        };
        env.reset();
        env
    }

    /// Khởi tạo lại môi trường (Reset State)
    pub fn reset(&mut self) {
        self.board = Board::new();
        self.generator = TileGenerator::new(self.seed);
        self.quest_manager = QuestManager::new();
        self.score_manager = ScoreManager::new(self.initial_stack);
        self.tile_queue.clear();
        self.placed_count = 0;

        // 1. Place initial starting tile at center (0, 0)
        let initial_tile = GeneratedTile::Normal {
            base_tile: crate::tile::BaseTile::new(0, self.seed, "Initial Center Plain Tile"),
            segments: Vec::new(),
        };
        self.board.place_tile(0, 0, initial_tile, 0);

        // 2. Sinh 3 tile preview ban đầu (Tile #1, #2, #3) — khớp với simulator.rs
        for _ in 0..3 {
            let active_count = self.quest_manager.pop_next_active_quest_count(); // pops 0, 0, 0
            let tile = self.generator.generate_tile(None, active_count, None, self.quest_manager.level);
            self.tile_queue.push_back(tile);
        }

        // Kích hoạt QuestWatcher CHỈ cho ô đầu cọc bài (Active Tile ở vị trí topStackPreview)
        if let Some(front_tile) = self.tile_queue.front_mut() {
            if let GeneratedTile::Quest { ref mut quest_data, .. } = front_tile {
                if quest_data.quest_id.is_none() {
                    let qid = self.quest_manager.add_quest(&quest_data.quest_type);
                    quest_data.quest_id = Some(qid);
                }
            }
            crate::quest_manager::initialize_active_quest_tile(front_tile, &self.board, &mut self.quest_manager);
        }
    }

    /// Tile hiện tại cần đặt ở lượt này
    pub fn current_tile(&self) -> Option<&GeneratedTile> {
        self.tile_queue.front()
    }

    /// Trả về danh sách tất cả các Action hợp lệ tại lượt hiện tại (loại bỏ các góc xoay đẳng cấu)
    pub fn get_valid_actions(&self) -> Vec<Action> {
        let mut valid = Vec::new();
        let Some(current_tile) = self.current_tile() else {
            return valid;
        };

        let period = current_tile.rotation_symmetry_period();
        let candidates = self.board.get_candidate_placements();
        for (q, r) in candidates {
            for rotation in 0..period {
                if self.board.can_place_tile(q, r, current_tile, rotation) {
                    valid.push(Action { q, r, rotation });
                }
            }
        }

        // Force nước đi đầu tiên vào vị trí cố định (0, -1) (phía dưới ô trung tâm).
        // Vì nước đầu đều đối xứng (đặt đâu cũng được), việc cố định vị trí giúp
        // chuẩn hóa dữ liệu tự chơi, tránh nhiễu tie-breaking ngẫu nhiên giữa các
        // ô tương đương. Model VẪN tự chọn góc xoay tại vị trí này.
        // Nếu (0, -1) không có rotation hợp lệ (vd tile toàn nước hiếm gặp) thì
        // rơi về hành vi bình thường để tránh game over ngay nước đầu.
        if self.placed_count == 0 {
            let forced: Vec<Action> = valid
                .iter()
                .copied()
                .filter(|a| a.q == 0 && a.r == -1)
                .collect();
            if !forced.is_empty() {
                valid = forced;
            }
        }

        valid
    }

    /// Kiểm tra NHANH xem có ít nhất 1 placement hợp lệ hay không (chỉ để biết đã hết nước đi).
    /// Dừng ngay khi tìm được nước đi hợp lệ đầu tiên -> cực kỳ nhanh với board thưa.
    /// Cùng kết quả boolean với `!get_valid_actions().is_empty()`, nhưng không dựng toàn bộ Vec.
    fn has_valid_action(&self) -> bool {
        let Some(current_tile) = self.current_tile() else {
            return false;
        };
        let period = current_tile.rotation_symmetry_period();
        let candidates = self.board.get_candidate_placements();
        for (q, r) in candidates {
            for rotation in 0..period {
                if self.board.can_place_tile(q, r, current_tile, rotation) {
                    return true;
                }
            }
        }
        false
    }

    /// Thực hiện 1 bước đi (Step Action)
    pub fn step(&mut self, action: Action) -> StepResult {
        let prev_score = self.score_manager.total_score;

        let Some(current_tile) = self.tile_queue.pop_front() else {
            return StepResult {
                reward: 0.0,
                done: true,
                total_score: self.score_manager.total_score,
                placed_count: self.placed_count,
                stack_height: self.score_manager.remaining_tiles,
                breakdown: crate::score_manager::PlacementScoreBreakdown::default(),
            };
        };

        // Đặt tile lên bàn bài
        let (placed_ok, quest_succeeded_count) = self.board.place_tile_with_manager(
            action.q,
            action.r,
            current_tile,
            action.rotation,
            Some(&mut self.quest_manager),
        );

        if !placed_ok {
            return StepResult {
                reward: -100.0,
                done: true,
                total_score: self.score_manager.total_score,
                placed_count: self.placed_count,
                stack_height: self.score_manager.remaining_tiles,
                breakdown: crate::score_manager::PlacementScoreBreakdown::default(),
            };
        }

        self.placed_count += 1;

        let prev_remaining = self.score_manager.remaining_tiles;
        // Cập nhật điểm số và số lượng tile trong stack qua ScoreManager
        let breakdown = self.score_manager.on_tile_placed(
            &self.board,
            action.q,
            action.r,
            quest_succeeded_count,
            0,
        );

        let step_score_delta = self.score_manager.total_score - prev_score;
        let new_remaining = self.score_manager.remaining_tiles;

        // Tile Economy Bonus: Khuyến khích duy trì và tích lũy cọc bài (sinh mệnh của ván đấu)
        let tile_delta = (new_remaining as f32) - (prev_remaining.saturating_sub(1) as f32);
        let tile_economy_bonus = if tile_delta > 0.0 {
            tile_delta * 12.0 // +12 điểm tiềm năng cho mỗi tile kiếm thêm được (từ Quest hoặc Perfect)
        } else if new_remaining <= 3 {
            -15.0 // Cảnh báo nguy hiểm khi cọc bài sắp hết (tránh game over sớm)
        } else {
            0.0
        };

        let reward = step_score_delta as f32 + tile_economy_bonus;

        // Thêm tile mới vào cuối tile_queue (KHÔNG activate quest ngay — chỉ khi lên front)
        let active_count = self.quest_manager.pop_next_active_quest_count();
        let new_tile = self.generator.generate_tile(None, active_count, None, self.quest_manager.level);
        self.tile_queue.push_back(new_tile);

        // Nếu đạt ngưỡng điểm -> tile N+4 trở thành Train Station (KHÔNG thay tile N+3)
        if self.generator.should_grant_reward(self.score_manager.total_score) {
            let reward = self.generator.grant_reward();
            self.tile_queue.push_back(reward);
        }

        // Kích hoạt QuestWatcher CHỈ cho ô đầu cọc bài (front) — giống simulator.rs dòng 350-368
        if let Some(front_tile) = self.tile_queue.front_mut() {
            if let GeneratedTile::Quest { ref mut quest_data, .. } = front_tile {
                if quest_data.quest_id.is_none() {
                    let qid = self.quest_manager.add_quest(&quest_data.quest_type);
                    quest_data.quest_id = Some(qid);
                }
            }
            crate::quest_manager::initialize_active_quest_tile(front_tile, &self.board, &mut self.quest_manager);
        }

        // Kiểm tra điều kiện kết thúc: chỉ check cạn cọc bài hoặc chạm mốc tile_limit.
        // Tránh gọi has_valid_action() quét HashSet toàn bộ bàn cờ hàng triệu lần trong MCTS simulations.
        let done = self.score_manager.remaining_tiles == 0
            || self.placed_count >= self.tile_limit;

        StepResult {
            reward,
            done,
            total_score: self.score_manager.total_score,
            placed_count: self.placed_count,
            stack_height: self.score_manager.remaining_tiles,
            breakdown,
        }
    }

    /// Kiểm tra trạng thái kết thúc ván đấu
    pub fn is_game_over(&self) -> bool {
        self.score_manager.remaining_tiles == 0
            || self.placed_count >= self.tile_limit
            || self.get_valid_actions().is_empty()
    }

    /// Lưu trạng thái trước khi step, để có thể undo.
    pub fn save_checkpoint(&self, action: Action) -> EnvUndoState {
        EnvUndoState {
            board_undo: self.board.save_undo_state(),
            action_q: Some(action.q),
            action_r: Some(action.r),
            placed_count: self.placed_count,
            total_score: self.score_manager.total_score,
            remaining_tiles: self.score_manager.remaining_tiles,
            is_game_over: self.score_manager.is_game_over,
            placed_tiles_count: self.score_manager.placed_tiles_count,
            perfect_count: self.score_manager.perfect_count,
            consecutive_perfects: self.score_manager.consecutive_perfects,
            quest_manager_clone: self.quest_manager.clone(),
            generator_clone: self.generator.clone(),
            tile_queue_clone: self.tile_queue.clone(),
        }
    }

    /// Lưu checkpoint gốc (không có action — dùng để quay về trạng thái đầu MCTS search).
    pub fn save_root_checkpoint(&self) -> EnvUndoState {
        EnvUndoState {
            board_undo: self.board.save_undo_state(),
            action_q: None,
            action_r: None,
            placed_count: self.placed_count,
            total_score: self.score_manager.total_score,
            remaining_tiles: self.score_manager.remaining_tiles,
            is_game_over: self.score_manager.is_game_over,
            placed_tiles_count: self.score_manager.placed_tiles_count,
            perfect_count: self.score_manager.perfect_count,
            consecutive_perfects: self.score_manager.consecutive_perfects,
            quest_manager_clone: self.quest_manager.clone(),
            generator_clone: self.generator.clone(),
            tile_queue_clone: self.tile_queue.clone(),
        }
    }

    /// Khôi phục trạng thái trước bước step gần nhất.
    pub fn restore_checkpoint(&mut self, state: EnvUndoState) {
        if let (Some(q), Some(r)) = (state.action_q, state.action_r) {
            self.board.restore_undo_state(state.board_undo, (q, r));
        } else {
            // Root checkpoint: chỉ restore groups + edge_to_group, không xóa tile
            self.board.groups = state.board_undo.groups;
            self.board.edge_to_group = state.board_undo.edge_to_group;
            self.board.next_group_id = state.board_undo.next_group_id;
        }
        // tile_queue_clone đã lưu queue trước khi pop front
        self.tile_queue = state.tile_queue_clone;
        self.placed_count = state.placed_count;
        self.score_manager.total_score = state.total_score;
        self.score_manager.remaining_tiles = state.remaining_tiles;
        self.score_manager.is_game_over = state.is_game_over;
        self.score_manager.placed_tiles_count = state.placed_tiles_count;
        self.score_manager.perfect_count = state.perfect_count;
        self.score_manager.consecutive_perfects = state.consecutive_perfects;
        self.quest_manager = state.quest_manager_clone;
        self.generator = state.generator_clone;
    }

    /// Trích xuất Đặc trưng Đồ thị (Graph Feature Extraction) cho GNN
    pub fn extract_graph_observation(&self) -> GraphObservation {
        let placed = &self.board.placed_tiles;
        let candidates = self.board.get_candidate_placements();

        let num_nodes_est = placed.len() + candidates.len();
        let mut node_positions = Vec::with_capacity(num_nodes_est);
        let mut pos_to_idx = HashMap::with_capacity_and_hasher(num_nodes_est, Default::default());

        for &pos in placed.keys() {
            pos_to_idx.insert(pos, node_positions.len());
            node_positions.push(pos);
        }

        for &pos in &candidates {
            if !pos_to_idx.contains_key(&pos) {
                pos_to_idx.insert(pos, node_positions.len());
                node_positions.push(pos);
            }
        }

        let mut node_features = Vec::with_capacity(node_positions.len());

        let tile_curr = self.tile_queue.get(0);
        let tile_1 = self.tile_queue.get(1);
        let tile_2 = self.tile_queue.get(2);

        for &pos in &node_positions {
            let mut feature = [0.0f32; 70];
            let is_placed = placed.contains_key(&pos);

            feature[0] = if is_placed { 1.0 } else { 0.0 };
            feature[1] = if !is_placed { 1.0 } else { 0.0 };

            if is_placed {
                let pt = &placed[&pos];
                // 2..8: 6 Edges terrain normalized (0.0 .. 1.0)
                for dir in 0..6 {
                    let edge_type = pt.edge_config.edges[dir];
                    feature[node_feat::EDGE_TERRAIN_START + dir] = (edge_type as usize as f32) / 7.0;
                }

                // 8..14: Open/Closed edge flags
                for dir in 0..6 {
                    let n_pos = get_neighbor_pos(pos.0, pos.1, dir);
                    feature[node_feat::OPEN_EDGE_START + dir] = if placed.contains_key(&n_pos) { 0.0 } else { 1.0 };
                }

                // 14: (bỏ — không dùng, giữ = 0 để không phá layout kênh feature)
                // 15: Group Open Edges (đếm nhanh số cạnh mở trực tiếp của tile)
                let mut open_edges = 0;
                for dir in 0..6 {
                    let n_pos = get_neighbor_pos(pos.0, pos.1, dir);
                    if !placed.contains_key(&n_pos) {
                        open_edges += 1;
                    }
                }
                feature[15] = (open_edges as f32 / 6.0).clamp(0.0, 1.0);

                // 16..27: Quest features
                if let GeneratedTile::Quest { quest_data, .. } = &pt.tile {
                    // Quest đang cần xử lý hay không (1 = active, 0 = đã kết thúc / không còn là quest)
                    feature[node_feat::QUEST_ACTIVE] = if pt.quest_finalized { 0.0 } else { 1.0 };
                    if !pt.quest_finalized {
                        let gt = quest_data.primary_group_type();
                        let gt_idx = match gt {
                            GroupType::Agriculture => 0,
                            GroupType::Forest => 1,
                            GroupType::Village => 2,
                            GroupType::Water => 3,
                            GroupType::TrainTracks => 4,
                        };
                        feature[node_feat::QUEST_GROUP_TYPE_START + gt_idx] = 1.0;

                        match quest_data.equality {
                            EqualityComparison::MoreThan => feature[node_feat::QUEST_EQUALITY_MORE] = 1.0,
                            EqualityComparison::Exactly => feature[node_feat::QUEST_EQUALITY_EXACTLY] = 1.0,
                        }

                        // Con số requirement hiện tại = số object cần thêm bên ngoài để hoàn thành quest
                        // (đã trừ object sẵn có trên chính quest tile, khớp với badge hiển thị +5 / =7)
                        let remaining_need = quest_data.remaining_display_value() as f32;
                        feature[node_feat::QUEST_REMAINING] = (remaining_need / 100.0).clamp(0.0, 1.0);
                    }
                }

                // 40..70: 30 feature đếm group theo cạnh (6 cạnh × 5 loại: nhà, cây, rock, water, train).
                // Chỉ áp dụng cho tile ĐÃ ĐẶT (ô trống không có địa hình nên vẫn = 0).
                let edge_feats = self.board.count_edge_features(pos);
                for edge_idx in 0..6 {
                    for ch in 0..5 {
                        feature[node_feat::EDGE_FEATURES_START + edge_idx * 5 + ch] = edge_feats[edge_idx][ch];
                    }
                }
            } else {
                // CANDIDATE NODE FEATURES (Đặc trưng ô trống ứng viên)
                // 2..8: Terrain của các ô đã đặt vây quanh
                for dir in 0..6 {
                    let n_pos = get_neighbor_pos(pos.0, pos.1, dir);
                    if let Some(n_tile) = placed.get(&n_pos) {
                        let opp_dir = (dir + 3) % 6;
                        let edge_type = n_tile.edge_config.edges[opp_dir];
                        feature[node_feat::EDGE_TERRAIN_START + dir] = (edge_type as usize as f32) / 7.0;
                    }
                }

                // 8..14: Số cạnh GHÉP KHỚP HOÀN HẢO (Perfect Terrain Match) cho 6 góc xoay
                if let Some(curr) = tile_curr {
                    let curr_cfg = curr.to_hex_edge_config();
                    for rot in 0..6 {
                        if self.board.can_place_tile(pos.0, pos.1, curr, rot) {
                            let mut perfect_matches = 0;
                            for dir in 0..6 {
                                let n_pos = get_neighbor_pos(pos.0, pos.1, dir);
                                if let Some(n_tile) = placed.get(&n_pos) {
                                    let opp_dir = (dir + 3) % 6;
                                    let n_edge = n_tile.edge_config.edges[opp_dir];
                                    let c_edge = curr_cfg.edge_at(dir, rot);

                                    if c_edge == n_edge || (c_edge.to_group_type().is_some() && c_edge.to_group_type() == n_edge.to_group_type()) {
                                        perfect_matches += 1;
                                    }
                                }
                            }
                            feature[node_feat::OPEN_EDGE_START + rot] = (perfect_matches as f32) / 6.0;
                        }
                    }
                }
            }

            // 14: Số tile THỰC SỰ CÒN LẠI (Tính chuẩn xác: min(cọc bài, tile_limit - placed_count))
            let tiles_left_in_game = self.tile_limit.saturating_sub(self.placed_count);
            let effective_remaining_tiles = self.score_manager.remaining_tiles.min(tiles_left_in_game);
            feature[node_feat::EFFECTIVE_REMAINING_TILES] = effective_remaining_tiles as f32;

            // 25: Đã bỏ điểm hiện tại, giữ = 0.0 theo chuẩn Markov AlphaZero
            feature[node_feat::CURRENT_SCORE_DIV_10] = 0.0;

            // 27..33: Upcoming Tile 1 Features
            if let Some(t1) = tile_1 {
                let cfg = t1.to_hex_edge_config();
                for i in 0..6 {
                    feature[node_feat::UPCOMING_TILE1_START + i] = (cfg.edges[i] as usize as f32) / 7.0;
                }
            }

            // 33..39: Upcoming Tile 2 Features
            if let Some(t2) = tile_2 {
                let cfg = t2.to_hex_edge_config();
                for i in 0..6 {
                    feature[node_feat::UPCOMING_TILE2_START + i] = (cfg.edges[i] as usize as f32) / 7.0;
                }
            }

            // 39: Step ratio (placed_count / tile_limit)
            feature[node_feat::STEP_RATIO] = (self.placed_count as f32 / self.tile_limit.max(1) as f32).clamp(0.0, 1.0);

            node_features.push(feature);
        }

        // Build Graph Edge Index với pre-allocated memory
        let mut edge_index = Vec::with_capacity(node_positions.len() * 4);
        for (idx, &pos) in node_positions.iter().enumerate() {
            for dir in 0..6 {
                let n_pos = get_neighbor_pos(pos.0, pos.1, dir);
                if let Some(&n_idx) = pos_to_idx.get(&n_pos) {
                    edge_index.push((idx, n_idx));
                }
            }
        }

        let valid_actions = self.get_valid_actions();
        let mut action_features = Vec::with_capacity(valid_actions.len());

        if let Some(curr) = tile_curr {
            let curr_cfg = curr.to_hex_edge_config();
            let is_quest_tile = if matches!(curr, GeneratedTile::Quest { .. }) { 1.0 } else { 0.0 };

            // Quest info của tile SẮP ĐẶT: equality (MoreThan/Exactly) + con số bubble ban đầu.
            let mut curr_equality_more = 0.0f32;
            let mut curr_remaining = 0.0f32;
            if let GeneratedTile::Quest { quest_data, .. } = curr {
                curr_remaining = (quest_data.remaining_display_value() as f32 / 100.0).clamp(0.0, 1.0);
                if let EqualityComparison::MoreThan = quest_data.equality {
                    curr_equality_more = 1.0;
                }
            }

            for act in &valid_actions {
                let mut feat = [0.0f32; 16];
                let mut matching_count = 0;
                let mut mismatching_count = 0;
                let mut quest_adj = 0.0f32;

                for dir in 0..6 {
                    let n_pos = get_neighbor_pos(act.q, act.r, dir);
                    let c_edge = curr_cfg.edge_at(dir, act.rotation);
                    feat[action_feat::EDGE_TYPES_START + dir] = (c_edge as usize as f32) / 7.0;

                    if let Some(n_tile) = placed.get(&n_pos) {
                        let opp_dir = (dir + 3) % 6;
                        let n_edge = n_tile.edge_config.edges[opp_dir];
                        let is_match = c_edge == n_edge || (c_edge.to_group_type().is_some() && c_edge.to_group_type() == n_edge.to_group_type());

                        if is_match {
                            matching_count += 1;
                        } else {
                            mismatching_count += 1;
                        }

                        if let GeneratedTile::Quest { quest_data, .. } = &n_tile.tile {
                            if !n_tile.quest_finalized && Some(quest_data.primary_group_type()) == c_edge.to_group_type() {
                                quest_adj = 1.0;
                            }
                        }
                    }
                }

                feat[action_feat::MATCHING_COUNT] = matching_count as f32 / 6.0;
                feat[action_feat::MISMATCHING_COUNT] = mismatching_count as f32 / 6.0;
                feat[action_feat::CURR_REMAINING] = curr_remaining;
                feat[action_feat::NEIGHBOR_COUNT] = (matching_count + mismatching_count) as f32 / 6.0;
                feat[action_feat::QUEST_ADJ] = quest_adj;
                feat[action_feat::CURR_EQUALITY_MORE] = curr_equality_more;
                feat[action_feat::IS_QUEST_TILE] = is_quest_tile;
                feat[action_feat::ROTATION] = act.rotation as f32 / 6.0;
                feat[action_feat::POS_Q] = 0.0; // Bỏ tọa độ tuyệt đối để giữ tính bất biến tịnh tiến
                feat[action_feat::POS_R] = 0.0;

                action_features.push(feat);
            }
        }

        GraphObservation {
            node_positions,
            node_features,
            edge_index,
            valid_actions,
            action_features,
        }
    }
}

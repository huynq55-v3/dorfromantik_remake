use std::collections::{HashMap, VecDeque};
use crate::board::{Board, get_neighbor_pos};
use crate::game_config::GroupType;
use crate::generator::TileGenerator;
use crate::quest_manager::QuestManager;
use crate::score_manager::ScoreManager;
use crate::tile::{EqualityComparison, GeneratedTile};

/// Hành động đặt tile trong môi trường RL
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
}

/// Trạng thái Đồ thị cho GNN Feature Extraction
#[derive(Debug, Clone)]
pub struct GraphObservation {
    /// Danh sách vị trí tọa độ của tất cả các node trong đồ thị (Placed + Candidates)
    pub node_positions: Vec<(i32, i32)>,
    /// Tensor đặc trưng của các node: [N, 40]
    pub node_features: Vec<[f32; 40]>,
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

        // 2. Pre-fill tile queue with 4 upcoming tiles
        for _ in 0..4 {
            let active_count = self.quest_manager.pop_next_active_quest_count();
            let mut tile = self.generator.generate_tile(None, active_count, None, self.quest_manager.level);
            if matches!(tile, GeneratedTile::Quest { .. }) {
                crate::quest_manager::initialize_active_quest_tile(&mut tile, &self.board, &mut self.quest_manager);
            }
            self.tile_queue.push_back(tile);
        }
    }

    /// Tile hiện tại cần đặt ở lượt này
    pub fn current_tile(&self) -> Option<&GeneratedTile> {
        self.tile_queue.front()
    }

    /// Trả về danh sách tất cả các Action hợp lệ tại lượt hiện tại
    pub fn get_valid_actions(&self) -> Vec<Action> {
        let mut valid = Vec::new();
        let Some(current_tile) = self.current_tile() else {
            return valid;
        };

        let candidates = self.board.get_candidate_placements();
        for (q, r) in candidates {
            for rotation in 0..6 {
                if self.board.can_place_tile(q, r, current_tile, rotation) {
                    valid.push(Action { q, r, rotation });
                }
            }
        }

        valid
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
            };
        };

        // Tính tiến độ nhiệm vụ trước khi đặt tile
        let prev_quest_progress: usize = self.board.placed_tiles.iter()
            .filter_map(|(&pos, pt)| {
                if let GeneratedTile::Quest { quest_data, .. } = &pt.tile {
                    if !pt.quest_finalized {
                        let gt = quest_data.primary_group_type();
                        let target = quest_data.remaining_display_value();
                        let cur = self.board.get_quest_external_count(pos, gt);
                        return Some(cur.min(target));
                    }
                }
                None
            })
            .sum();

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
            };
        }

        self.placed_count += 1;

        // Cập nhật điểm số và số lượng tile trong stack qua ScoreManager
        let breakdown = self.score_manager.on_tile_placed(
            &self.board,
            action.q,
            action.r,
            quest_succeeded_count,
            0,
        );

        // Tính tiến độ nhiệm vụ sau khi đặt tile
        let new_quest_progress: usize = self.board.placed_tiles.iter()
            .filter_map(|(&pos, pt)| {
                if let GeneratedTile::Quest { quest_data, .. } = &pt.tile {
                    let gt = quest_data.primary_group_type();
                    let target = quest_data.remaining_display_value();
                    let cur = self.board.get_quest_external_count(pos, gt);
                    return Some(cur.min(target));
                }
                None
            })
            .sum();
        let quest_progress_delta = new_quest_progress.saturating_sub(prev_quest_progress);

        let step_score_delta = self.score_manager.total_score - prev_score;

        let mut shaped_reward = step_score_delta as f32;
        if quest_succeeded_count > 0 {
            shaped_reward += quest_succeeded_count as f32 * 150.0;
        }
        if breakdown.perfect_count > 0 {
            shaped_reward += breakdown.perfect_count as f32 * 80.0;
        }
        if quest_progress_delta > 0 {
            shaped_reward += quest_progress_delta as f32 * 15.0;
        }
        if breakdown.matching_edges > 0 {
            shaped_reward += breakdown.matching_edges as f32 * 5.0;
        } else {
            shaped_reward -= 5.0;
        }
        // Thưởng sinh tồn từng bước để Agent muốn duy trì ván chơi
        shaped_reward += 2.0;

        // Thêm tile mới vào cuối tile_queue
        let active_count = self.quest_manager.pop_next_active_quest_count();
        let mut new_tile = self.generator.generate_tile(None, active_count, None, self.quest_manager.level);
        if matches!(new_tile, GeneratedTile::Quest { .. }) {
            crate::quest_manager::initialize_active_quest_tile(&mut new_tile, &self.board, &mut self.quest_manager);
        }
        self.tile_queue.push_back(new_tile);

        // Kiểm tra điều kiện kết thúc
        let valid_actions = self.get_valid_actions();
        let done = self.score_manager.remaining_tiles == 0
            || self.placed_count >= self.tile_limit
            || valid_actions.is_empty();

        if done && self.placed_count < self.tile_limit {
            shaped_reward -= 20.0; // Phạt chết sớm
        }

        StepResult {
            reward: shaped_reward,
            done,
            total_score: self.score_manager.total_score,
            placed_count: self.placed_count,
            stack_height: self.score_manager.remaining_tiles,
        }
    }

    /// Kiểm tra trạng thái kết thúc ván đấu
    pub fn is_game_over(&self) -> bool {
        self.score_manager.remaining_tiles == 0
            || self.placed_count >= self.tile_limit
            || self.get_valid_actions().is_empty()
    }

    /// Trích xuất Đặc trưng Đồ thị (Graph Feature Extraction) cho GNN
    pub fn extract_graph_observation(&self) -> GraphObservation {
        let placed = &self.board.placed_tiles;
        let candidates = self.board.get_candidate_placements();

        let mut node_positions = Vec::new();
        let mut pos_to_idx = HashMap::new();

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
            let mut feature = [0.0f32; 40];
            let is_placed = placed.contains_key(&pos);

            feature[0] = if is_placed { 1.0 } else { 0.0 };
            feature[1] = if !is_placed { 1.0 } else { 0.0 };

            if is_placed {
                let pt = &placed[&pos];
                // 2..8: 6 Edges terrain normalized (0.0 .. 1.0)
                for dir in 0..6 {
                    let edge_type = pt.edge_config.edges[dir];
                    feature[2 + dir] = (edge_type as usize as f32) / 7.0;
                }

                // 8..14: Open/Closed edge flags
                for dir in 0..6 {
                    let n_pos = get_neighbor_pos(pos.0, pos.1, dir);
                    feature[8 + dir] = if placed.contains_key(&n_pos) { 0.0 } else { 1.0 };
                }

                // 14 & 15: Group Element Count & Group Open Edges
                let mut max_group_count = 0;
                let mut max_open_edges = 0;
                for dir in 0..6 {
                    if let Some(gt) = pt.edge_config.edges[dir].to_group_type() {
                        let open = self.board.count_group_open_edges(pos, gt);
                        let count = self.board.get_quest_external_count(pos, gt);
                        if count > max_group_count {
                            max_group_count = count;
                        }
                        if open > max_open_edges {
                            max_open_edges = open;
                        }
                    }
                }
                feature[14] = ((1.0 + max_group_count as f32).log2() / (100.0_f32).log2()).clamp(0.0, 1.0);
                feature[15] = (max_open_edges as f32 / 12.0).clamp(0.0, 1.0);

                // 16..27: Quest features
                if let GeneratedTile::Quest { quest_data, .. } = &pt.tile {
                    feature[16] = if pt.quest_finalized { 0.0 } else { 1.0 };
                    let gt = quest_data.primary_group_type();
                    let gt_idx = match gt {
                        GroupType::Agriculture => 0,
                        GroupType::Forest => 1,
                        GroupType::Village => 2,
                        GroupType::Water => 3,
                        GroupType::TrainTracks => 4,
                    };
                    feature[17 + gt_idx] = 1.0;

                    match quest_data.equality {
                        EqualityComparison::MoreThan => feature[22] = 1.0,
                        EqualityComparison::Exactly => feature[23] = 1.0,
                    }

                    let target_val = quest_data.target_count as f32;
                    let current_ext = self.board.get_quest_external_count(pos, gt) as f32;

                    feature[24] = ((1.0 + target_val).log2() / (101.0_f32).log2()).clamp(0.0, 1.0);
                    feature[25] = if target_val > 0.0 { (current_ext / target_val).clamp(0.0, 2.0) } else { 0.0 };

                    if quest_data.equality == EqualityComparison::Exactly && current_ext > target_val {
                        feature[26] = 1.0; // Overfilled Exact Quest
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
                        feature[2 + dir] = (edge_type as usize as f32) / 7.0;
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
                            feature[8 + rot] = (perfect_matches as f32) / 6.0;
                        }
                    }
                }
            }

            // 27..33: Upcoming Tile 1 Features
            if let Some(t1) = tile_1 {
                let cfg = t1.to_hex_edge_config();
                for i in 0..6 {
                    feature[27 + i] = (cfg.edges[i] as usize as f32) / 7.0;
                }
            }

            // 33..39: Upcoming Tile 2 Features
            if let Some(t2) = tile_2 {
                let cfg = t2.to_hex_edge_config();
                for i in 0..6 {
                    feature[33 + i] = (cfg.edges[i] as usize as f32) / 7.0;
                }
            }

            // 39: Step ratio
            feature[39] = (self.placed_count as f32 / self.tile_limit as f32).clamp(0.0, 1.0);

            node_features.push(feature);
        }

        // Build Graph Edge Index
        let mut edge_index = Vec::new();
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

            for act in &valid_actions {
                let mut feat = [0.0f32; 16];
                let mut matching_count = 0;
                let mut mismatching_count = 0;
                let mut open_count = 0;
                let mut quest_adj = 0.0f32;
                let mut quest_connect = 0.0f32;

                for dir in 0..6 {
                    let n_pos = get_neighbor_pos(act.q, act.r, dir);
                    let c_edge = curr_cfg.edge_at(dir, act.rotation);
                    feat[8 + dir] = (c_edge as usize as f32) / 7.0;

                    if let Some(n_tile) = placed.get(&n_pos) {
                        let opp_dir = (dir + 3) % 6;
                        let n_edge = n_tile.edge_config.edges[opp_dir];
                        let is_match = c_edge == n_edge || (c_edge.to_group_type().is_some() && c_edge.to_group_type() == n_edge.to_group_type());

                        if is_match {
                            matching_count += 1;
                            if let GeneratedTile::Quest { quest_data, .. } = &n_tile.tile {
                                if !n_tile.quest_finalized && Some(quest_data.primary_group_type()) == c_edge.to_group_type() {
                                    quest_connect = 1.0;
                                }
                            }
                        } else {
                            mismatching_count += 1;
                        }

                        if let GeneratedTile::Quest { quest_data, .. } = &n_tile.tile {
                            if !n_tile.quest_finalized && Some(quest_data.primary_group_type()) == c_edge.to_group_type() {
                                quest_adj = 1.0;
                            }
                        }
                    } else {
                        open_count += 1;
                    }
                }

                feat[0] = matching_count as f32 / 6.0;
                feat[1] = mismatching_count as f32 / 6.0;
                feat[2] = open_count as f32 / 6.0;
                feat[3] = if matching_count > 0 && mismatching_count == 0 {
                    if open_count == 0 { 1.0 } else { 0.5 }
                } else {
                    0.0
                };
                feat[4] = quest_adj;
                feat[5] = quest_connect;
                feat[6] = is_quest_tile;
                feat[7] = act.rotation as f32 / 6.0;
                feat[14] = (matching_count as f32 * 10.0) / 60.0;
                feat[15] = if quest_connect > 0.0 { 1.0 } else { 0.0 };

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

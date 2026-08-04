use std::collections::{HashMap, HashSet};
use crate::game_config::{get_segment_element_count, GroupType};
use crate::tile::{EdgeType, GeneratedTile, HexEdgeConfig, EqualityComparison};

/// Trạng thái hoàn thành của Quest
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FulfillmentStatus {
    Incomplete,
    Success,
    Failed,
}

/// Một ô tile đã được đặt trên bàn chơi tại tọa độ Axial (q, r)
#[derive(Debug, Clone)]
pub struct PlacedTile {
    pub q: i32,
    pub r: i32,
    pub tile: GeneratedTile,
    pub rotation: usize, // 0..6
    pub edge_config: HexEdgeConfig,
    /// Trạng thái Quest (nếu đây là ô Quest)
    pub quest_status: Option<FulfillmentStatus>,
    /// Đánh dấu quest đã hoàn thành/thất bại và không theo dõi nữa
    pub quest_finalized: bool,
}

/// Tọa độ 6 hướng láng giềng Axial Hex (Flat-Topped Hex Grid)
pub const HEX_DIRECTIONS: [(i32, i32); 6] = [
    (1, 0),   // 0: Right
    (0, 1),   // 1: Bottom-Right
    (-1, 1),  // 2: Bottom-Left
    (-1, 0),  // 3: Left
    (0, -1),  // 4: Top-Left
    (1, -1),  // 5: Top-Right
];

/// Trả về hướng đối diện (0 <-> 3, 1 <-> 4, 2 <-> 5)
pub fn opposite_direction(dir: usize) -> usize {
    (dir + 3) % 6
}

/// Cụm phần tử địa hình liên thông (ElementGroup) trên bàn chơi
#[derive(Debug, Clone)]
pub struct ElementGroup {
    pub id: usize,
    pub group_type: GroupType,
    pub total_element_count: usize,
    pub total_segment_count: usize,
    pub member_tiles: HashSet<(i32, i32)>,
    pub is_closed: bool,
}

/// Board quản lý bàn chơi và thuật toán ghép cụm ElementGroupManager
pub struct Board {
    pub placed_tiles: HashMap<(i32, i32), PlacedTile>,
    pub groups: HashMap<usize, ElementGroup>,
    pub tile_to_group: HashMap<((i32, i32), GroupType), usize>,
    next_group_id: usize,
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {
    pub fn new() -> Self {
        Self {
            placed_tiles: HashMap::new(),
            groups: HashMap::new(),
            tile_to_group: HashMap::new(),
            next_group_id: 1,
        }
    }

    /// Trả về số lượng object/segment của cụm địa hình lớn nhất chưa đóng trên bàn (ReferenceGroupCount)
    pub fn reference_group_count(&self, group_type: GroupType) -> usize {
        let mut max_count = 0;
        for group in self.groups.values() {
            if group.group_type == group_type && !group.is_closed {
                let count = match group_type {
                    GroupType::Forest | GroupType::Village => group.total_element_count,
                    _ => group.total_segment_count,
                };
                if count > max_count {
                    max_count = count;
                }
            }
        }
        max_count
    }

    /// Kiểm tra quy tắc đặt tile có hợp lệ không (Placement Validation)
    /// - Water edge chỉ được nối với Water edge (hoặc ô trống)
    /// - TrainTrack edge chỉ được nối với TrainTrack edge (hoặc ô trống)
    pub fn can_place_tile(&self, q: i32, r: i32, tile: &GeneratedTile, rotation: usize) -> bool {
        if self.placed_tiles.contains_key(&(q, r)) {
            return false;
        }

        // Bắt buộc phải kề với ít nhất 1 ô đã đặt (trừ ô trung tâm 0,0 đầu tiên)
        if !self.placed_tiles.is_empty() && (q != 0 || r != 0) {
            let has_neighbor = HEX_DIRECTIONS.iter().any(|&(dq, dr)| {
                self.placed_tiles.contains_key(&(q + dq, r + dr))
            });
            if !has_neighbor {
                return false;
            }
        }

        let mut cfg = tile.to_hex_edge_config();
        cfg.rotate(rotation);

        for (dir, &(dq, dr)) in HEX_DIRECTIONS.iter().enumerate() {
            let neighbor_pos = (q + dq, r + dr);
            if let Some(neighbor) = self.placed_tiles.get(&neighbor_pos) {
                let my_edge = cfg.edges[dir];
                let neighbor_edge = neighbor.edge_config.edges[opposite_direction(dir)];

                // Constraining placement rules:
                // 1. Water edge MUST match Water edge
                if my_edge == EdgeType::Water && neighbor_edge != EdgeType::Water {
                    return false;
                }
                if neighbor_edge == EdgeType::Water && my_edge != EdgeType::Water {
                    return false;
                }

                // 2. TrainTrack edge MUST match TrainTrack edge
                if my_edge == EdgeType::TrainTracks && neighbor_edge != EdgeType::TrainTracks {
                    return false;
                }
                if neighbor_edge == EdgeType::TrainTracks && my_edge != EdgeType::TrainTracks {
                    return false;
                }
            }
        }

        true
    }

    /// Đặt 1 tile lên bàn chơi tại vị trí (q, r), quay góc rotation
    pub fn place_tile(&mut self, q: i32, r: i32, tile: GeneratedTile, rotation: usize) -> bool {
        if !self.can_place_tile(q, r, &tile, rotation) {
            return false;
        }

        let mut cfg = tile.to_hex_edge_config();
        cfg.rotate(rotation);

        let is_quest = matches!(tile, GeneratedTile::Quest { .. });
        let placed = PlacedTile {
            q,
            r,
            tile,
            rotation,
            edge_config: cfg,
            quest_status: if is_quest { Some(FulfillmentStatus::Incomplete) } else { None },
            quest_finalized: false,
        };

        self.placed_tiles.insert((q, r), placed);

        // Hợp nhất cụm địa hình liên thông (ElementGroupManager)
        self.update_element_groups(q, r);

        // ĐÁNH GIÁ LIÊN TỤC TẤT CẢ CÁC QUEST DANG ACTIVE TRÊN BÀN
        self.evaluate_all_active_quests();

        true
    }

    /// Cập nhật và hợp nhất cụm địa hình liên thông (ElementGroupManager)
    fn update_element_groups(&mut self, q: i32, r: i32) {
        let placed = match self.placed_tiles.get(&(q, r)) {
            Some(t) => t.clone(),
            None => return,
        };

        let mut present_groups = HashSet::new();
        for &edge in &placed.edge_config.edges {
            if let Some(gt) = edge.to_group_type() {
                present_groups.insert(gt);
            }
        }

        for gt in present_groups {
            let element_count = self.get_tile_element_count(&placed.tile, gt);
            let segment_count = 1;

            let mut connected_group_ids = HashSet::new();
            for (dir, &(dq, dr)) in HEX_DIRECTIONS.iter().enumerate() {
                let neighbor_pos = (q + dq, r + dr);
                if let Some(neighbor) = self.placed_tiles.get(&neighbor_pos) {
                    let my_edge = placed.edge_config.edges[dir];
                    let neighbor_edge = neighbor.edge_config.edges[opposite_direction(dir)];
                    if my_edge.to_group_type() == Some(gt) && neighbor_edge.to_group_type() == Some(gt) {
                        if let Some(&gid) = self.tile_to_group.get(&(neighbor_pos, gt)) {
                            connected_group_ids.insert(gid);
                        }
                    }
                }
            }

            if connected_group_ids.is_empty() {
                let gid = self.next_group_id;
                self.next_group_id += 1;
                let mut members = HashSet::new();
                members.insert((q, r));

                let group = ElementGroup {
                    id: gid,
                    group_type: gt,
                    total_element_count: element_count,
                    total_segment_count: segment_count,
                    member_tiles: members,
                    is_closed: false,
                };
                self.groups.insert(gid, group);
                self.tile_to_group.insert(((q, r), gt), gid);
            } else {
                let group_ids_vec: Vec<usize> = connected_group_ids.into_iter().collect();
                let main_gid = group_ids_vec[0];

                self.tile_to_group.insert(((q, r), gt), main_gid);
                if let Some(main_group) = self.groups.get_mut(&main_gid) {
                    main_group.member_tiles.insert((q, r));
                    main_group.total_element_count += element_count;
                    main_group.total_segment_count += segment_count;
                }

                for &other_gid in &group_ids_vec[1..] {
                    if let Some(other_group) = self.groups.remove(&other_gid) {
                        for &member_pos in &other_group.member_tiles {
                            self.tile_to_group.insert((member_pos, gt), main_gid);
                        }
                        if let Some(main_group) = self.groups.get_mut(&main_gid) {
                            main_group.member_tiles.extend(other_group.member_tiles);
                            main_group.total_element_count += other_group.total_element_count;
                            main_group.total_segment_count += other_group.total_segment_count;
                        }
                    }
                }
            }
        }
    }

    /// Trả về số lượng element của ô bài (Fix bug: Trả về 0 nếu ô không chứa địa hình GroupType)
    pub fn get_tile_element_count(&self, tile: &GeneratedTile, gt: GroupType) -> usize {
        match tile {
            GeneratedTile::Normal { segments, .. } => {
                let mut sum = 0;
                for seg in segments {
                    if seg.group_type == gt {
                        sum += get_segment_element_count(gt, seg.segment_type);
                    }
                }
                sum
            }
            GeneratedTile::Quest { quest_data, .. } => {
                if quest_data.primary_group_type() == gt {
                    1
                } else {
                    0
                }
            }
        }
    }

    /// Tìm góc xoay hợp lệ tiếp theo (Smart Tile Rotation)
    pub fn get_next_valid_rotation(
        &self,
        q: i32,
        r: i32,
        tile: &GeneratedTile,
        current_rotation: usize,
        forward: bool,
    ) -> usize {
        let valid_rotations: Vec<usize> = (0..6)
            .filter(|&rot| self.can_place_tile(q, r, tile, rot))
            .collect();

        if valid_rotations.is_empty() {
            return (current_rotation + if forward { 1 } else { 5 }) % 6;
        }

        if valid_rotations.contains(&current_rotation) {
            let idx = valid_rotations.iter().position(|&r| r == current_rotation).unwrap();
            if forward {
                valid_rotations[(idx + 1) % valid_rotations.len()]
            } else {
                valid_rotations[(idx + valid_rotations.len() - 1) % valid_rotations.len()]
            }
        } else {
            valid_rotations[0]
        }
    }

    /// Trả về tất cả các vị trí ô trống kề với bàn chơi hiện tại (kèm thông tin có góc xoay hợp lệ nào không)
    pub fn get_available_placement_slots(&self, tile: &GeneratedTile) -> Vec<((i32, i32), bool)> {
        let mut slots = HashMap::new();
        if self.placed_tiles.is_empty() {
            slots.insert((0, 0), true);
            return slots.into_iter().collect();
        }

        for &(pq, pr) in self.placed_tiles.keys() {
            for &(dq, dr) in HEX_DIRECTIONS.iter() {
                let n_pos = (pq + dq, pr + dr);
                if !self.placed_tiles.contains_key(&n_pos) {
                    let is_valid = (0..6).any(|rot| self.can_place_tile(n_pos.0, n_pos.1, tile, rot));
                    slots.entry(n_pos).or_insert(is_valid);
                }
            }
        }

        slots.into_iter().collect()
    }

    /// Lấy số lượng element/segment NGOÀI (không tính ô Quest bản thân nó) đã kết nối vào cụm chứa pos
    pub fn get_quest_external_count(&self, pos: (i32, i32), group_type: GroupType) -> usize {
        if let Some(&gid) = self.tile_to_group.get(&(pos, group_type)) {
            if let Some(group) = self.groups.get(&gid) {
                let mut external_count = 0;
                for &m_pos in &group.member_tiles {
                    if m_pos != pos {
                        if let Some(pt) = self.placed_tiles.get(&m_pos) {
                            let count = match group_type {
                                GroupType::Forest | GroupType::Village => self.get_tile_element_count(&pt.tile, group_type),
                                _ => 1,
                            };
                            external_count += count;
                        }
                    }
                }
                return external_count;
            }
        }
        0
    }

    /// Trả về số lượng Quest đang active (Incomplete) trên bàn bài
    pub fn active_quest_count(&self) -> i32 {
        self.placed_tiles
            .values()
            .filter(|pt| matches!(pt.tile, GeneratedTile::Quest { .. }) && !pt.quest_finalized)
            .count() as i32
    }

    /// Trả về con số target còn lại hiện tại của ô Quest (target_count - external_count)
    pub fn get_quest_remaining_target(&self, pos: (i32, i32)) -> usize {
        if let Some(pt) = self.placed_tiles.get(&pos) {
            if let GeneratedTile::Quest { quest_data, .. } = &pt.tile {
                let external_count = self.get_quest_external_count(pos, quest_data.primary_group_type());
                return quest_data.target_count.saturating_sub(external_count);
            }
        }
        0
    }

    /// ĐÁNH GIÁ LIÊN TỤC TẤT CẢ CÁC QUEST ACTIVE TRÊN BÀN CHƠI
    pub fn evaluate_all_active_quests(&mut self) {
        let active_keys: Vec<(i32, i32)> = self.placed_tiles
            .iter()
            .filter(|(_, pt)| matches!(pt.tile, GeneratedTile::Quest { .. }) && !pt.quest_finalized)
            .map(|(&pos, _)| pos)
            .collect();

        for pos in active_keys {
            let (target_count, equality, group_type) = {
                let pt = &self.placed_tiles[&pos];
                if let GeneratedTile::Quest { quest_data, .. } = &pt.tile {
                    (quest_data.target_count, quest_data.equality, quest_data.primary_group_type())
                } else {
                    continue;
                }
            };

            let external_count = self.get_quest_external_count(pos, group_type);

            let new_status = match equality {
                EqualityComparison::MoreThan => {
                    if external_count >= target_count {
                        FulfillmentStatus::Success
                    } else {
                        FulfillmentStatus::Incomplete
                    }
                }
                EqualityComparison::Exactly => {
                    if external_count == target_count {
                        FulfillmentStatus::Success
                    } else if external_count > target_count {
                        FulfillmentStatus::Failed
                    } else {
                        FulfillmentStatus::Incomplete
                    }
                }
            };

            if let Some(pt) = self.placed_tiles.get_mut(&pos) {
                pt.quest_status = Some(new_status);
                if new_status == FulfillmentStatus::Success || new_status == FulfillmentStatus::Failed {
                    pt.quest_finalized = true;
                }
            }
        }
    }

    /// PREVIEW TÍNH NĂNG CẬP NHẬT CON SỐ QUEST NGAY KHI RÊ TILE ĐẾN VỊ TRÍ ỨỚM THỬ (hover_q, hover_r)
    pub fn preview_quest_counts(
        &self,
        hover_q: i32,
        hover_r: i32,
        hover_tile: &GeneratedTile,
        hover_rotation: usize,
    ) -> HashMap<(i32, i32), (usize, FulfillmentStatus)> {
        let mut preview_results = HashMap::new();

        if !self.can_place_tile(hover_q, hover_r, hover_tile, hover_rotation) {
            return preview_results;
        }

        let mut preview_cfg = hover_tile.to_hex_edge_config();
        preview_cfg.rotate(hover_rotation);

        let active_keys: Vec<(i32, i32)> = self.placed_tiles
            .iter()
            .filter(|(_, pt)| matches!(pt.tile, GeneratedTile::Quest { .. }) && !pt.quest_finalized)
            .map(|(&pos, _)| pos)
            .collect();

        for pos in active_keys {
            let (target_count, equality, group_type) = {
                let pt = &self.placed_tiles[&pos];
                if let GeneratedTile::Quest { quest_data, .. } = &pt.tile {
                    (quest_data.target_count, quest_data.equality, quest_data.primary_group_type())
                } else {
                    continue;
                }
            };

            let current_external = self.get_quest_external_count(pos, group_type);
            let mut added_external = 0;

            // Kiểm tra xem tile đang ướm thử có cạnh MATCHING group_type nối vào pos hay cụm của pos không
            for (dir, &(dq, dr)) in HEX_DIRECTIONS.iter().enumerate() {
                let n_pos = (hover_q + dq, hover_r + dr);
                if n_pos == pos {
                    // Hover tile kề trực tiếp với Quest tile pos
                    let my_edge = preview_cfg.edges[dir];
                    let quest_edge = self.placed_tiles[&pos].edge_config.edges[opposite_direction(dir)];

                    if my_edge.to_group_type() == Some(group_type) && quest_edge.to_group_type() == Some(group_type) {
                        let delta = match group_type {
                            GroupType::Forest | GroupType::Village => self.get_tile_element_count(hover_tile, group_type),
                            _ => 1,
                        };
                        added_external = delta;
                        break;
                    }
                } else if let Some(&gid) = self.tile_to_group.get(&(pos, group_type)) {
                    // Hover tile kề với 1 tile khác trong cùng cụm với pos
                    let my_edge = preview_cfg.edges[dir];
                    if my_edge.to_group_type() == Some(group_type) {
                        if let Some(&n_gid) = self.tile_to_group.get(&(n_pos, group_type)) {
                            if n_gid == gid {
                                let neighbor_edge = self.placed_tiles[&n_pos].edge_config.edges[opposite_direction(dir)];
                                if neighbor_edge.to_group_type() == Some(group_type) {
                                    let delta = match group_type {
                                        GroupType::Forest | GroupType::Village => self.get_tile_element_count(hover_tile, group_type),
                                        _ => 1,
                                    };
                                    added_external = delta;
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            let simulated_external = current_external + added_external;
            let remaining_target = target_count.saturating_sub(simulated_external);

            let preview_status = match equality {
                EqualityComparison::MoreThan => {
                    if simulated_external >= target_count {
                        FulfillmentStatus::Success
                    } else {
                        FulfillmentStatus::Incomplete
                    }
                }
                EqualityComparison::Exactly => {
                    if simulated_external == target_count {
                        FulfillmentStatus::Success
                    } else if simulated_external > target_count {
                        FulfillmentStatus::Failed
                    } else {
                        FulfillmentStatus::Incomplete
                    }
                }
            };

            preview_results.insert(pos, (remaining_target, preview_status));
        }

        // Thêm tính toán preview cho chính ô hover_tile (nếu hover_tile bản thân nó là 1 ô Quest)
        if let GeneratedTile::Quest { quest_data, .. } = hover_tile {
            let target_count = quest_data.target_count;
            let equality = quest_data.equality;
            let group_type = quest_data.primary_group_type();

            let mut simulated_external = 0;
            let mut connected_gids = HashSet::new();

            for (dir, &(dq, dr)) in HEX_DIRECTIONS.iter().enumerate() {
                let n_pos = (hover_q + dq, hover_r + dr);
                let my_edge = preview_cfg.edges[dir];

                if my_edge.to_group_type() == Some(group_type) {
                    if let Some(neighbor) = self.placed_tiles.get(&n_pos) {
                        let neighbor_edge = neighbor.edge_config.edges[opposite_direction(dir)];
                        if neighbor_edge.to_group_type() == Some(group_type) {
                            if let Some(&gid) = self.tile_to_group.get(&(n_pos, group_type)) {
                                if !connected_gids.contains(&gid) {
                                    connected_gids.insert(gid);
                                    if let Some(group) = self.groups.get(&gid) {
                                        let count = match group_type {
                                            GroupType::Forest | GroupType::Village => group.total_element_count,
                                            _ => group.total_segment_count,
                                        };
                                        simulated_external += count;
                                    }
                                }
                            } else {
                                // Hàng xóm đơn lẻ chưa có group ID
                                let count = match group_type {
                                    GroupType::Forest | GroupType::Village => self.get_tile_element_count(&neighbor.tile, group_type),
                                    _ => 1,
                                };
                                simulated_external += count;
                            }
                        }
                    }
                }
            }

            let remaining_target = target_count.saturating_sub(simulated_external);
            let preview_status = match equality {
                EqualityComparison::MoreThan => {
                    if simulated_external >= target_count {
                        FulfillmentStatus::Success
                    } else {
                        FulfillmentStatus::Incomplete
                    }
                }
                EqualityComparison::Exactly => {
                    if simulated_external == target_count {
                        FulfillmentStatus::Success
                    } else if simulated_external > target_count {
                        FulfillmentStatus::Failed
                    } else {
                        FulfillmentStatus::Incomplete
                    }
                }
            };

            preview_results.insert((hover_q, hover_r), (remaining_target, preview_status));
        }

        preview_results
    }
}

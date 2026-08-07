use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use crate::game_config::GroupType;
use crate::tile::{GeneratedTile, HexEdgeConfig, EqualityComparison};

static LAST_LOGGED_HOVER: Mutex<Option<((i32, i32), usize, String)>> = Mutex::new(None);

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
    /// ID Quest được đăng ký bởi QuestManager (nếu có)
    pub quest_id: Option<usize>,
}

/// Tọa độ 6 hướng láng giềng Unity Offset Grid (Even-Q / Uneven-Q Column Offset)
pub const EVEN_COLUMN_DIRECTIONS: [(i32, i32); 6] = [
    (0, 1),   // 0: Top / North
    (1, 1),   // 1: Top-Right / North-East
    (1, 0),   // 2: Bottom-Right / South-East
    (0, -1),  // 3: Bottom / South
    (-1, 0),  // 4: Bottom-Left / South-West
    (-1, 1),  // 5: Top-Left / North-West
];

pub const UNEVEN_COLUMN_DIRECTIONS: [(i32, i32); 6] = [
    (0, 1),   // 0: Top / North
    (1, 0),   // 1: Top-Right / North-East
    (1, -1),  // 2: Bottom-Right / South-East
    (0, -1),  // 3: Bottom / South
    (-1, -1), // 4: Bottom-Left / South-West
    (-1, 0),  // 5: Top-Left / North-West
];

/// Trả về vị trí láng giềng kề cạnh `dir` theo đúng hệ tọa độ Unity Offset Grid (Vector2Int x, y)
pub fn get_neighbor_pos(q: i32, r: i32, dir: usize) -> (i32, i32) {
    let dirs = if q % 2 == 0 {
        &EVEN_COLUMN_DIRECTIONS
    } else {
        &UNEVEN_COLUMN_DIRECTIONS
    };
    let (dq, dr) = dirs[dir % 6];
    (q + dq, r + dr)
}

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
#[derive(Debug, Clone)]
pub struct Board {
    pub placed_tiles: HashMap<(i32, i32), PlacedTile>,
    pub groups: HashMap<usize, ElementGroup>,
    pub edge_to_group: HashMap<((i32, i32), usize), usize>,
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
            edge_to_group: HashMap::new(),
            next_group_id: 1,
        }
    }

    /// Lấy danh sách các Group ID duy nhất của ô tile tại vị trí pos tương ứng với group_type
    pub fn get_group_ids_for_tile(&self, pos: (i32, i32), group_type: GroupType) -> HashSet<usize> {
        let mut set = HashSet::new();
        if let Some(pt) = self.placed_tiles.get(&pos) {
            for dir in 0..6 {
                if pt.edge_config.edges[dir].to_group_type() == Some(group_type) {
                    if let Some(&gid) = self.edge_to_group.get(&(pos, dir)) {
                        set.insert(gid);
                    }
                }
            }
        }
        set
    }

    /// Trả về số lượng object/segment của cụm địa hình lớn nhất chưa đóng trên bàn (ReferenceGroupCount)
    pub fn reference_group_count(&self, group_type: GroupType) -> usize {
        let mut max_count = 0;
        for group in self.groups.values() {
            if group.group_type == group_type && !group.is_closed {
                let count = match group_type {
                    GroupType::Forest | GroupType::Village | GroupType::Agriculture => group.total_element_count,
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
            let has_neighbor = (0..6).any(|dir| {
                let n_pos = get_neighbor_pos(q, r, dir);
                self.placed_tiles.contains_key(&n_pos)
            });
            if !has_neighbor {
                return false;
            }
        }

        let mut cfg = tile.to_hex_edge_config();
        cfg.rotate(rotation);

        for dir in 0..6 {
            let neighbor_pos = get_neighbor_pos(q, r, dir);
            if let Some(neighbor) = self.placed_tiles.get(&neighbor_pos) {
                let my_edge = cfg.edges[dir];
                let neighbor_edge = neighbor.edge_config.edges[opposite_direction(dir)];

                if !my_edge.is_compatible_with(neighbor_edge) {
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

        let (is_quest, quest_id) = match &tile {
            GeneratedTile::Quest { quest_data, .. } => (true, quest_data.quest_id),
            _ => (false, None),
        };
        let placed = PlacedTile {
            q,
            r,
            tile,
            rotation,
            edge_config: cfg,
            quest_status: if is_quest { Some(FulfillmentStatus::Incomplete) } else { None },
            quest_finalized: false,
            quest_id,
        };

        self.placed_tiles.insert((q, r), placed);

        // Hợp nhất cụm địa hình liên thông (ElementGroupManager)
        self.update_element_groups(q, r);

        true
    }

    /// Đặt bài đồng thời hỗ trợ cập nhật tự động QuestManager (trả về (thành công, số_quest_hoàn_thành))
    pub fn place_tile_with_manager(&mut self, q: i32, r: i32, tile: GeneratedTile, rotation: usize, mut quest_manager: Option<&mut crate::quest_manager::QuestManager>) -> (bool, usize) {
        let is_quest = matches!(&tile, GeneratedTile::Quest { .. });
        let ok = self.place_tile(q, r, tile, rotation);
        if ok {
            let (quests_resolved, quests_succeeded) = self.evaluate_all_active_quests(quest_manager.as_deref_mut());
            if let Some(qm) = quest_manager {
                qm.update_after_placement(is_quest, quests_resolved);
            }
            (true, quests_succeeded)
        } else {
            (false, 0)
        }
    }

    /// Cập nhật và hợp nhất cụm địa hình liên thông theo từng Segment (ElementGroupManager C#)
    fn update_element_groups(&mut self, q: i32, r: i32) {
        let placed = match self.placed_tiles.get(&(q, r)) {
            Some(t) => t.clone(),
            None => return,
        };

        let tile_segments = placed.tile.get_segments(placed.rotation);

        for seg in tile_segments {
            let gt = seg.group_type;
            let element_count = seg.element_count;
            let segment_count = 1;

            let mut connected_group_ids = HashSet::new();
            for &dir in &seg.edges {
                let neighbor_pos = get_neighbor_pos(q, r, dir);
                let neighbor_dir = opposite_direction(dir);
                if let Some(neighbor) = self.placed_tiles.get(&neighbor_pos) {
                    let my_edge = placed.edge_config.edges[dir];
                    let neighbor_edge = neighbor.edge_config.edges[neighbor_dir];
                    if my_edge.to_group_type() == Some(gt) && neighbor_edge.to_group_type() == Some(gt) {
                        if let Some(&gid) = self.edge_to_group.get(&(neighbor_pos, neighbor_dir)) {
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
                for &dir in &seg.edges {
                    self.edge_to_group.insert(((q, r), dir), gid);
                }
            } else {
                let group_ids_vec: Vec<usize> = connected_group_ids.into_iter().collect();
                let main_gid = group_ids_vec[0];

                for &dir in &seg.edges {
                    self.edge_to_group.insert(((q, r), dir), main_gid);
                }

                if let Some(main_group) = self.groups.get_mut(&main_gid) {
                    main_group.member_tiles.insert((q, r));
                    main_group.total_element_count += element_count;
                    main_group.total_segment_count += segment_count;
                }

                for &other_gid in &group_ids_vec[1..] {
                    if let Some(other_group) = self.groups.remove(&other_gid) {
                        if let Some(main_group) = self.groups.get_mut(&main_gid) {
                            main_group.member_tiles.extend(other_group.member_tiles);
                            main_group.total_element_count += other_group.total_element_count;
                            main_group.total_segment_count += other_group.total_segment_count;
                        }
                        for g in self.edge_to_group.values_mut() {
                            if *g == other_gid {
                                *g = main_gid;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Trả về tổng số lượng element/cạnh địa hình của toàn bộ ô bài đối với GroupType.
    /// LƯU Ý: Hàm này trả về tổng số nhà/cây của TẤT CẢ các segment trên ô bài.
    /// NẾU CẦN ĐẾM SỐ PHẦN TỬ CỦA 1 CỤM LIÊN THÔNG (ElementGroup), KHÔNG DÙNG HÀM NÀY
    /// mà phải dùng `group.total_element_count` để tránh đếm nhầm các segment biệt lập chưa nối.
    pub fn get_tile_element_count(&self, tile: &GeneratedTile, gt: GroupType) -> usize {
        match tile {
            GeneratedTile::Quest { quest_data, .. } => quest_data.own_elements_for_group(gt),
            GeneratedTile::Normal { segments, .. } => {
                let mut total = 0;
                for seg in segments {
                    if seg.group_type == gt {
                        total += crate::game_config::get_segment_element_count(gt, seg.segment_type);
                    }
                }
                if total > 0 {
                    total
                } else {
                    let cfg = tile.to_hex_edge_config();
                    let count = cfg.edges.iter().filter(|&&e| e.to_group_type() == Some(gt)).count();
                    if count > 0 { count } else { 0 }
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
            for dir in 0..6 {
                let n_pos = get_neighbor_pos(pq, pr, dir);
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
        let gids = self.get_group_ids_for_tile(pos, group_type);
        if gids.is_empty() {
            return 0;
        }

        let quest_own_elements = if let Some(pt) = self.placed_tiles.get(&pos) {
            match group_type {
                GroupType::Forest | GroupType::Village | GroupType::Water | GroupType::Agriculture => self.get_tile_element_count(&pt.tile, group_type),
                _ => 1,
            }
        } else {
            0
        };

        let mut total = 0;
        for gid in gids {
            if let Some(group) = self.groups.get(&gid) {
                let count = match group_type {
                    GroupType::Forest | GroupType::Village | GroupType::Water | GroupType::Agriculture => group.total_element_count,
                    _ => group.total_segment_count,
                };
                total += count;
            }
        }

        total.saturating_sub(quest_own_elements)
    }

    /// Đếm số cạnh mở cho một danh sách các ô thuộc cụm (tính cả hover_tile nếu có)
    pub fn count_open_edges_for_tiles(
        &self,
        member_tiles: &HashSet<(i32, i32)>,
        group_type: GroupType,
        hover_pos: Option<((i32, i32), &HexEdgeConfig)>,
    ) -> usize {
        let mut open_edges = 0;
        for &m_pos in member_tiles {
            let edges = if let Some((h_pos, h_cfg)) = hover_pos {
                if m_pos == h_pos {
                    h_cfg.edges
                } else if let Some(pt) = self.placed_tiles.get(&m_pos) {
                    pt.edge_config.edges
                } else {
                    continue;
                }
            } else if let Some(pt) = self.placed_tiles.get(&m_pos) {
                pt.edge_config.edges
            } else {
                continue;
            };

            for dir in 0..6 {
                if edges[dir].to_group_type() == Some(group_type) {
                    let n_pos = get_neighbor_pos(m_pos.0, m_pos.1, dir);
                    let is_occupied = self.placed_tiles.contains_key(&n_pos) || hover_pos.map_or(false, |(hp, _)| n_pos == hp);
                    if !is_occupied {
                        open_edges += 1;
                    }
                }
            }
        }
        open_edges
    }

    /// Đếm số cạnh mở (open edges) của cụm địa hình chứa `pos` cho `group_type`
    pub fn count_group_open_edges(&self, pos: (i32, i32), group_type: GroupType) -> usize {
        let gids = self.get_group_ids_for_tile(pos, group_type);
        let mut member_tiles = HashSet::new();

        if gids.is_empty() {
            member_tiles.insert(pos);
        } else {
            for gid in gids {
                if let Some(group) = self.groups.get(&gid) {
                    member_tiles.extend(group.member_tiles.iter().copied());
                }
            }
            if member_tiles.is_empty() {
                member_tiles.insert(pos);
            }
        }

        self.count_open_edges_for_tiles(&member_tiles, group_type, None)
    }

    /// Trả về số lượng Quest đang active (Incomplete) trên bàn bài
    pub fn active_quest_count(&self) -> i32 {
        self.placed_tiles
            .values()
            .filter(|pt| matches!(pt.tile, GeneratedTile::Quest { .. }) && !pt.quest_finalized)
            .count() as i32
    }

    /// Trả về con số target còn lại hiện tại của ô Quest (remaining_display_value - external_count)
    pub fn get_quest_remaining_target(&self, pos: (i32, i32)) -> usize {
        if let Some(pt) = self.placed_tiles.get(&pos) {
            if let GeneratedTile::Quest { quest_data, .. } = &pt.tile {
                let external_count = self.get_quest_external_count(pos, quest_data.primary_group_type());
                return quest_data.remaining_display_value().saturating_sub(external_count);
            }
        }
        0
    }

    /// Đánh giá liên tục tất cả các Quest active trên bàn chơi.
    /// Trả về (số quest được finalize, số quest hoàn thành thành công) trong lượt này.
    pub fn evaluate_all_active_quests(&mut self, mut quest_manager: Option<&mut crate::quest_manager::QuestManager>) -> (usize, usize) {
        let active_keys: Vec<(i32, i32)> = self.placed_tiles
            .iter()
            .filter(|(_, pt)| matches!(pt.tile, GeneratedTile::Quest { .. }) && !pt.quest_finalized)
            .map(|(&pos, _)| pos)
            .collect();

        let mut resolved_count = 0;
        let mut succeeded_count = 0;

        for pos in active_keys {
            let (target_count, equality, group_type) = {
                let pt = &self.placed_tiles[&pos];
                if let GeneratedTile::Quest { quest_data, .. } = &pt.tile {
                    (quest_data.remaining_display_value(), quest_data.equality, quest_data.primary_group_type())
                } else {
                    continue;
                }
            };

            let external_count = self.get_quest_external_count(pos, group_type);
            let open_edges = self.count_group_open_edges(pos, group_type);

            let new_status = match equality {
                EqualityComparison::MoreThan => {
                    if external_count >= target_count {
                        FulfillmentStatus::Success
                    } else if open_edges == 0 {
                        FulfillmentStatus::Failed
                    } else {
                        FulfillmentStatus::Incomplete
                    }
                }
                EqualityComparison::Exactly => {
                    if external_count == target_count {
                        FulfillmentStatus::Success
                    } else if external_count > target_count {
                        FulfillmentStatus::Failed
                    } else if open_edges == 0 {
                        FulfillmentStatus::Failed
                    } else {
                        FulfillmentStatus::Incomplete
                    }
                }
            };

            if let Some(pt) = self.placed_tiles.get_mut(&pos) {
                pt.quest_status = Some(new_status);
                if new_status == FulfillmentStatus::Success {
                    succeeded_count += 1;
                    if let Some(ref mut qm) = quest_manager {
                        qm.gain_levels(1);
                    }
                }
                if new_status == FulfillmentStatus::Success || new_status == FulfillmentStatus::Failed {
                    resolved_count += 1;
                    if let Some(qid) = pt.quest_id {
                        if let Some(ref mut qm) = quest_manager {
                            qm.remove_quest(qid);
                        }
                    }
                    pt.quest_finalized = true;
                }
            }
        }

        (resolved_count, succeeded_count)
    }

    /// Tính số cụm địa hình (Group ID) duy nhất đang chứa quest active chưa hoàn thành trên bàn bài
    pub fn active_quest_group_count_on_board(&self) -> usize {
        let mut active_gids = std::collections::HashSet::new();
        for (&pos, pt) in &self.placed_tiles {
            if matches!(pt.tile, GeneratedTile::Quest { .. }) && !pt.quest_finalized {
                if let GeneratedTile::Quest { quest_data, .. } = &pt.tile {
                    let gt = quest_data.primary_group_type();
                    let gids = self.get_group_ids_for_tile(pos, gt);
                    if !gids.is_empty() {
                        active_gids.extend(gids);
                    } else {
                        active_gids.insert((pos.0 as isize * 100000 + pos.1 as isize) as usize);
                    }
                }
            }
        }
        active_gids.len()
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

        let hover_pos = (hover_q, hover_r);

        for pos in active_keys {
            let (target_count, equality, group_type) = {
                let pt = &self.placed_tiles[&pos];
                if let GeneratedTile::Quest { quest_data, .. } = &pt.tile {
                    (quest_data.remaining_display_value(), quest_data.equality, quest_data.primary_group_type())
                } else {
                    continue;
                }
            };

            let current_external = self.get_quest_external_count(pos, group_type);
            let gids = self.get_group_ids_for_tile(pos, group_type);

            // Tìm tất cả các group_id chạm vào hover_tile ở những cạnh matching group_type
            let mut connected_gids = HashSet::new();
            for dir in 0..6 {
                let my_edge = preview_cfg.edges[dir];
                if my_edge.to_group_type() == Some(group_type) {
                    let n_pos = get_neighbor_pos(hover_q, hover_r, dir);
                    let neighbor_dir = opposite_direction(dir);
                    if let Some(neighbor) = self.placed_tiles.get(&n_pos) {
                        let neighbor_edge = neighbor.edge_config.edges[neighbor_dir];
                        if neighbor_edge.to_group_type() == Some(group_type) {
                            if let Some(&n_gid) = self.edge_to_group.get(&(n_pos, neighbor_dir)) {
                                connected_gids.insert(n_gid);
                            }
                        }
                    }
                }
            }

            // Kiểm tra xem hover_tile có chạm vào bất kỳ group nào thuộc quest này (hoặc chạm vào pos nếu gids rỗng)
            let hover_connects = if gids.is_empty() {
                (0..6).any(|dir| {
                    let n_pos = get_neighbor_pos(hover_q, hover_r, dir);
                    n_pos == pos && preview_cfg.edges[dir].to_group_type() == Some(group_type)
                })
            } else {
                connected_gids.iter().any(|gid| gids.contains(gid))
            };

            if hover_connects {
                let delta_hover = match group_type {
                    GroupType::Forest | GroupType::Village | GroupType::Agriculture | GroupType::Water => self.get_tile_element_count(hover_tile, group_type),
                    _ => 1,
                };

                let mut delta_extra_groups = 0;
                for &n_gid in &connected_gids {
                    if !gids.contains(&n_gid) {
                        if let Some(group) = self.groups.get(&n_gid) {
                            let count = match group_type {
                                GroupType::Forest | GroupType::Village | GroupType::Agriculture | GroupType::Water => group.total_element_count,
                                _ => group.total_segment_count,
                            };
                            delta_extra_groups += count;
                        }
                    }
                }

                let simulated_external = current_external + delta_hover + delta_extra_groups;

                let current_key = (hover_pos, hover_rotation, hover_tile.tile_preset_string());
                let should_log = {
                    let mut last = LAST_LOGGED_HOVER.lock().unwrap();
                    if last.as_ref() != Some(&current_key) {
                        *last = Some(current_key);
                        true
                    } else {
                        false
                    }
                };

                let mut member_tiles = HashSet::new();
                for gid in &gids {
                    if let Some(group) = self.groups.get(gid) {
                        member_tiles.extend(group.member_tiles.iter().copied());
                    }
                }
                for gid in &connected_gids {
                    if let Some(group) = self.groups.get(gid) {
                        member_tiles.extend(group.member_tiles.iter().copied());
                    }
                }
                member_tiles.insert(hover_pos);

                let simulated_open_edges = self.count_open_edges_for_tiles(&member_tiles, group_type, Some((hover_pos, &preview_cfg)));
                let remaining_target = target_count.saturating_sub(simulated_external);

                let preview_status = match equality {
                    EqualityComparison::MoreThan => {
                        if simulated_external >= target_count {
                            FulfillmentStatus::Success
                        } else if simulated_open_edges == 0 {
                            FulfillmentStatus::Failed
                        } else {
                            FulfillmentStatus::Incomplete
                        }
                    }
                    EqualityComparison::Exactly => {
                        if simulated_external == target_count {
                            FulfillmentStatus::Success
                        } else if simulated_external > target_count {
                            FulfillmentStatus::Failed
                        } else if simulated_open_edges == 0 {
                            FulfillmentStatus::Failed
                        } else {
                            FulfillmentStatus::Incomplete
                        }
                    }
                };

                if should_log {
                    println!(
                        "[HOVER PREVIEW DETAILED LOG] HoverPos={:?} Tile='{}' Rot={} | QuestPos={:?} GroupType={:?} | TargetCount={} | CurrentExternal={} | DeltaHover={} | DeltaExtraGroups={}",
                        hover_pos, hover_tile.tile_preset_string(), hover_rotation, pos, group_type, target_count, current_external, delta_hover, delta_extra_groups
                    );
                    for &n_gid in &connected_gids {
                        if !gids.contains(&n_gid) {
                            if let Some(group) = self.groups.get(&n_gid) {
                                println!(
                                    "   [EXTRA GROUP CONNECTED] GID={} | ElementCount={} | SegmentCount={} | MemberTiles={:?}",
                                    n_gid, group.total_element_count, group.total_segment_count, group.member_tiles
                                );
                            }
                        } else {
                            println!("   [MAIN QUEST GROUP CONNECTED] GID={}", n_gid);
                            if let Some(group) = self.groups.get(&n_gid) {
                                println!("      -> MAIN GROUP GID={} DETAILS: TotalElements={}", n_gid, group.total_element_count);
                                for &(mq, mr) in &group.member_tiles {
                                    if let Some(pt) = self.placed_tiles.get(&(mq, mr)) {
                                        let elem_cnt = self.get_tile_element_count(&pt.tile, group_type);
                                        println!("         - Tile at ({}, {}) = '{}' | Houses={}", mq, mr, pt.tile.tile_preset_string(), elem_cnt);
                                    }
                                }
                            }
                        }
                    }
                    println!(
                        "   => RESULT: SimulatedExternal={} | OpenEdges={} | RemainingTargetDisplayed={} | Status={:?}",
                        simulated_external, simulated_open_edges, remaining_target, preview_status
                    );
                }

                preview_results.insert(pos, (remaining_target, preview_status));
            }
        }

        // Thêm tính toán preview cho chính ô hover_tile (nếu hover_tile bản thân nó là 1 ô Quest)
        if let GeneratedTile::Quest { quest_data, .. } = hover_tile {
            let target_count = quest_data.remaining_display_value();
            let equality = quest_data.equality;
            let group_type = quest_data.primary_group_type();

            let mut simulated_external = 0;
            let mut connected_gids = HashSet::new();
            let mut member_tiles = HashSet::new();
            member_tiles.insert(hover_pos);

            for dir in 0..6 {
                let n_pos = get_neighbor_pos(hover_q, hover_r, dir);
                let my_edge = preview_cfg.edges[dir];

                if my_edge.to_group_type() == Some(group_type) {
                    if let Some(neighbor) = self.placed_tiles.get(&n_pos) {
                        let neighbor_edge = neighbor.edge_config.edges[opposite_direction(dir)];
                        if neighbor_edge.to_group_type() == Some(group_type) {
                            if let Some(&gid) = self.edge_to_group.get(&(n_pos, opposite_direction(dir))) {
                                if !connected_gids.contains(&gid) {
                                    connected_gids.insert(gid);
                                    if let Some(group) = self.groups.get(&gid) {
                                        member_tiles.extend(group.member_tiles.iter().copied());
                                        let count = match group_type {
                                            GroupType::Forest | GroupType::Village | GroupType::Agriculture => group.total_element_count,
                                            _ => group.total_segment_count,
                                        };
                                        simulated_external += count;
                                    }
                                }
                            } else {
                                member_tiles.insert(n_pos);
                                let count = match group_type {
                                    GroupType::Forest | GroupType::Village | GroupType::Agriculture => self.get_tile_element_count(&neighbor.tile, group_type),
                                    _ => 1,
                                };
                                simulated_external += count;
                            }
                        }
                    }
                }
            }

            let simulated_open_edges = self.count_open_edges_for_tiles(&member_tiles, group_type, Some((hover_pos, &preview_cfg)));
            let remaining_target = target_count.saturating_sub(simulated_external);

            let preview_status = match equality {
                EqualityComparison::MoreThan => {
                    if simulated_external >= target_count {
                        FulfillmentStatus::Success
                    } else if simulated_open_edges == 0 {
                        FulfillmentStatus::Failed
                    } else {
                        FulfillmentStatus::Incomplete
                    }
                }
                EqualityComparison::Exactly => {
                    if simulated_external == target_count {
                        FulfillmentStatus::Success
                    } else if simulated_external > target_count {
                        FulfillmentStatus::Failed
                    } else if simulated_open_edges == 0 {
                        FulfillmentStatus::Failed
                    } else {
                        FulfillmentStatus::Incomplete
                    }
                }
            };

            preview_results.insert(hover_pos, (remaining_target, preview_status));
        }

        preview_results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::{BaseTile, QuestTileData, GeneratedTile, EqualityComparison, SegmentData};
    use crate::game_config::{GroupType, SegmentType};

    fn create_test_quest_tile(id: usize, quest_type: &str, target_count: usize, equality: EqualityComparison) -> GeneratedTile {
        GeneratedTile::Quest {
            base_tile: BaseTile::new(id, 12345, "QuestTile"),
            quest_data: QuestTileData {
                seed: 12345,
                quest_type: quest_type.to_string(),
                target_count,
                equality,
                level: 0,
                quest_id: Some(id),
                stack_quest_id: None,
            },
        }
    }

    #[test]
    fn test_quest_exactly_exceeded_fails() {
        let mut board = Board::new();
        // Place an Exactly quest (=1 village, target_count=2, own=1)
        let quest_tile = create_test_quest_tile(1, "QuestTile_Village_1AV", 2, EqualityComparison::Exactly);
        assert!(board.place_tile(0, 0, quest_tile, 0));

        // Initial eval: target 1 external, currently 0 external -> Incomplete
        let (resolved, succeeded) = board.evaluate_all_active_quests(None);
        assert_eq!(resolved, 0);
        assert_eq!(succeeded, 0);

        // Place a normal village tile with 3 village elements (ST3A occupies edges 0,1,2; when rotated by 3 steps, occupies edges 3,4,5)
        let village_tile = GeneratedTile::Normal {
            base_tile: BaseTile::new(2, 54321, "NormalTile"),
            segments: vec![
                SegmentData {
                    index: 0,
                    group_type: GroupType::Village,
                    segment_type: SegmentType::ST3A, // 3 elements
                    occupied_edges: vec![0, 1, 2],
                    rotation: 3,
                    is_hybrid: false,
                }
            ],
        };
        assert!(board.place_tile(0, 1, village_tile, 0)); // dir 3 of village_tile (Village) faces dir 0 of (0,0) (Village)

        let (resolved, succeeded) = board.evaluate_all_active_quests(None);
        // Exceeded! Should be resolved as Failed (resolved=1, succeeded=0)
        assert_eq!(resolved, 1);
        assert_eq!(succeeded, 0);
        assert_eq!(board.placed_tiles[&(0, 0)].quest_status, Some(FulfillmentStatus::Failed));
    }

    #[test]
    fn test_quest_blocked_prematurely_fails() {
        let mut board = Board::new();
        // Place a MoreThan quest (+5 forest) at (0, 0)
        let quest_tile = create_test_quest_tile(1, "QuestTile_Forest_1AF", 6, EqualityComparison::MoreThan);
        assert!(board.place_tile(0, 0, quest_tile, 0));

        // Place plain tiles around (0, 0) blocking all 6 neighbor directions
        for dir in 0..6 {
            let n_pos = get_neighbor_pos(0, 0, dir);
            let plain_tile = GeneratedTile::Normal {
                base_tile: BaseTile::new(10 + dir, 100, "PlainTile"),
                segments: vec![],
            };
            board.place_tile(n_pos.0, n_pos.1, plain_tile, 0);
        }

        // All edges of (0, 0) are now blocked (open_edges == 0), count = 0 < 5 remaining target
        let (resolved, succeeded) = board.evaluate_all_active_quests(None);
        assert_eq!(resolved, 1);
        assert_eq!(succeeded, 0);
        assert_eq!(board.placed_tiles[&(0, 0)].quest_status, Some(FulfillmentStatus::Failed));
    }

    #[test]
    fn test_quest_completed_then_closed_stays_success() {
        let mut board = Board::new();
        // Place an Exactly quest (=1 village) target 2 total, own 1
        let quest_tile = create_test_quest_tile(1, "QuestTile_Village_1AV", 2, EqualityComparison::Exactly);
        assert!(board.place_tile(0, 0, quest_tile, 0));

        // Connect 1 village tile -> ST1A rotated by 3 has Village at dir 3 facing (0,0)'s dir 0
        let village_tile = GeneratedTile::Normal {
            base_tile: BaseTile::new(2, 54321, "NormalTile"),
            segments: vec![
                SegmentData {
                    index: 0,
                    group_type: GroupType::Village,
                    segment_type: SegmentType::ST1A,
                    occupied_edges: vec![0],
                    rotation: 3,
                    is_hybrid: false,
                }
            ],
        };
        assert!(board.place_tile(0, 1, village_tile, 0));

        let (resolved, succeeded) = board.evaluate_all_active_quests(None);
        assert_eq!(resolved, 1);
        assert_eq!(succeeded, 1);
        assert_eq!(board.placed_tiles[&(0, 0)].quest_status, Some(FulfillmentStatus::Success));
        assert!(board.placed_tiles[&(0, 0)].quest_finalized);

        // Block remaining sides after completion
        for dir in 1..6 {
            let n_pos = get_neighbor_pos(0, 0, dir);
            let plain_tile = GeneratedTile::Normal {
                base_tile: BaseTile::new(10 + dir, 100, "PlainTile"),
                segments: vec![],
            };
            board.place_tile(n_pos.0, n_pos.1, plain_tile, 0);
        }

        // Re-eval should not fail it because it is already finalized with Success
        assert_eq!(board.placed_tiles[&(0, 0)].quest_status, Some(FulfillmentStatus::Success));
    }

    #[test]
    fn test_quest_with_unconnected_multi_segment_tile() {
        let mut board = Board::new();

        // Quest Tile Village 6AV (7 own houses, target = 12, remaining target = 5)
        let quest_tile = create_test_quest_tile(1, "QuestTile_Village_6AV", 12, EqualityComparison::MoreThan);
        assert!(board.place_tile(0, 0, quest_tile, 0));

        // Multi-segment tile with 2 separate 1AV segments:
        // Seg #0 (1AV) on edge 3 (connects to (0,0) Edge 0)
        // Seg #1 (1AV) on edge 1 (faces empty hex (1,1) Edge 1)
        let multi_seg_tile = GeneratedTile::Normal {
            base_tile: BaseTile::new(2, 20, "2AV_Tile"),
            segments: vec![
                SegmentData {
                    index: 0,
                    group_type: GroupType::Village,
                    segment_type: SegmentType::ST1A,
                    occupied_edges: vec![3],
                    rotation: 3,
                    is_hybrid: false,
                },
                SegmentData {
                    index: 1,
                    group_type: GroupType::Village,
                    segment_type: SegmentType::ST1A,
                    occupied_edges: vec![1],
                    rotation: 1,
                    is_hybrid: false,
                },
            ],
        };

        assert!(board.place_tile(0, 1, multi_seg_tile, 0));

        // External count must be 1 house (only Seg #0 connected), NOT 2 houses!
        let ext_count = board.get_quest_external_count((0, 0), GroupType::Village);
        assert_eq!(ext_count, 1);

        // Remaining target must be 4 houses (5 - 1 = 4), NOT 3 houses!
        let remaining = board.get_quest_remaining_target((0, 0));
        assert_eq!(remaining, 4);
    }
}

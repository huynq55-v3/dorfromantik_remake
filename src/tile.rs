use crate::game_config::{GroupType, SegmentType};

/// Cấu trúc BaseTile khớp chuẩn với các thuộc tính lưu trữ trong C# class Tile:
/// 1. name: Tên GameObject (ví dụ: "Stacked Tile 0")
/// 2. seed: Chỉ số seed ngẫu nhiên (int)
/// 3. is_generated: Đánh dấu bài đã gen chi tiết địa hình/quest chưa (bool)
#[derive(Debug, Clone)]
pub struct BaseTile {
    pub id: usize,
    pub name: String,
    pub seed: i32,
    pub is_generated: bool,
}

impl BaseTile {
    pub fn new(id: usize, seed: i32, name_prefix: &str) -> Self {
        Self {
            id,
            name: format!("{} {}", name_prefix, id),
            seed,
            is_generated: false,
        }
    }
}

impl GroupType {
    pub fn letter(&self) -> char {
        match self {
            GroupType::Agriculture => 'A',
            GroupType::Forest => 'F',
            GroupType::Village => 'V',
            GroupType::TrainTracks => 'T',
            GroupType::Water => 'W',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeType {
    Plain,
    Agriculture,
    Forest,
    Village,
    Water,
    TrainTracks,
    FlexibleWater,
    WaterTrainStation,
}

impl From<GroupType> for EdgeType {
    fn from(gt: GroupType) -> Self {
        match gt {
            GroupType::Agriculture => EdgeType::Agriculture,
            GroupType::Forest => EdgeType::Forest,
            GroupType::Village => EdgeType::Village,
            GroupType::Water => EdgeType::Water,
            GroupType::TrainTracks => EdgeType::TrainTracks,
        }
    }
}

impl EdgeType {
    pub fn to_group_type(&self) -> Option<GroupType> {
        match self {
            EdgeType::Agriculture => Some(GroupType::Agriculture),
            EdgeType::Forest => Some(GroupType::Forest),
            EdgeType::Village => Some(GroupType::Village),
            EdgeType::Water | EdgeType::FlexibleWater => Some(GroupType::Water),
            EdgeType::TrainTracks => Some(GroupType::TrainTracks),
            EdgeType::WaterTrainStation => None,
            EdgeType::Plain => None,
        }
    }

    pub fn is_compatible_with(&self, other: EdgeType) -> bool {
        match (self, other) {
            // Water cứng: Bắt buộc ghép Nước (Water, FlexibleWater, WaterTrainStation)
            (EdgeType::Water, EdgeType::Water | EdgeType::FlexibleWater | EdgeType::WaterTrainStation) => true,
            (EdgeType::Water, _) => false,

            // TrainTracks cứng: Bắt buộc ghép Xe Lửa (TrainTracks, WaterTrainStation)
            (EdgeType::TrainTracks, EdgeType::TrainTracks | EdgeType::WaterTrainStation) => true,
            (EdgeType::TrainTracks, _) => false,

            // FlexibleWater ghép Nước, Plain, FlexibleWater, WaterTrainStation, nhưng không ghép TrainTracks cứng
            (EdgeType::FlexibleWater, EdgeType::TrainTracks) => false,
            (EdgeType::FlexibleWater, _) => true,

            // WaterTrainStation ghép tất cả (Water, TrainTracks, Plain, FlexibleWater, WaterTrainStation)
            (EdgeType::WaterTrainStation, _) => true,

            // Plain ghép Plain, FlexibleWater, WaterTrainStation và các loại không ràng buộc (Forest, Village, Agri)
            (EdgeType::Plain, EdgeType::Water) | (EdgeType::Plain, EdgeType::TrainTracks) => false,
            (EdgeType::Plain, _) => true,

            // Các loại không ràng buộc (Agriculture, Forest, Village) ghép được với tất cả trừ Water/TrainTracks cứng
            (EdgeType::Agriculture | EdgeType::Forest | EdgeType::Village, EdgeType::Water | EdgeType::TrainTracks) => false,
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HexEdgeConfig {
    pub edges: [EdgeType; 6],
}

impl HexEdgeConfig {
    pub fn new(edges: [EdgeType; 6]) -> Self {
        Self { edges }
    }

    pub fn plain() -> Self {
        Self {
            edges: [EdgeType::Plain; 6],
        }
    }

    /// Trả về loại cạnh ở hướng world_dir (0..5) khi tile bị xoay góc rotation_index (0..5)
    pub fn edge_at(&self, world_dir: usize, rotation_index: usize) -> EdgeType {
        let local_dir = (world_dir + 6 - (rotation_index % 6)) % 6;
        self.edges[local_dir]
    }

    /// Xoay các cạnh theo chiều kim đồng hồ theo số bước steps (0..5)
    pub fn rotate(&mut self, steps: usize) {
        let steps = steps % 6;
        if steps == 0 { return; }
        let mut rotated = [EdgeType::Plain; 6];
        for i in 0..6 {
            rotated[(i + steps) % 6] = self.edges[i];
        }
        self.edges = rotated;
    }
}

impl SegmentType {
    pub fn is_lake_shape(&self) -> bool {
        matches!(self, SegmentType::ST2A | SegmentType::ST3A | SegmentType::ST4A | SegmentType::ST5A | SegmentType::ST6A)
    }

    pub fn shape_code(&self) -> &'static str {
        match self {
            SegmentType::ST1A => "1A",
            SegmentType::ST2A => "2A",
            SegmentType::ST2B => "2B",
            SegmentType::ST2C => "2C",
            SegmentType::ST3A => "3A",
            SegmentType::ST3B => "3B",
            SegmentType::ST3C => "3C",
            SegmentType::ST3D => "3D",
            SegmentType::ST4A => "4A",
            SegmentType::ST4B => "4B",
            SegmentType::ST4C => "4C",
            SegmentType::ST5A => "5A",
            SegmentType::ST6A => "6A",
        }
    }

    pub fn edge_count(&self) -> usize {
        match self {
            SegmentType::ST1A => 1,
            SegmentType::ST2A | SegmentType::ST2B | SegmentType::ST2C => 2,
            SegmentType::ST3A | SegmentType::ST3B | SegmentType::ST3C | SegmentType::ST3D => 3,
            SegmentType::ST4A | SegmentType::ST4B | SegmentType::ST4C => 4,
            SegmentType::ST5A => 5,
            SegmentType::ST6A => 6,
        }
    }

    pub fn base_edges(&self) -> &'static [usize] {
        match self {
            SegmentType::ST1A => &[0],
            SegmentType::ST2A => &[0, 1],
            SegmentType::ST2B => &[0, 2],
            SegmentType::ST2C => &[0, 3],
            SegmentType::ST3A => &[0, 1, 2],
            SegmentType::ST3B => &[0, 1, 3],
            SegmentType::ST3C => &[0, 1, 4],
            SegmentType::ST3D => &[0, 2, 4],
            SegmentType::ST4A => &[0, 1, 2, 3],
            SegmentType::ST4B => &[0, 1, 2, 4],
            SegmentType::ST4C => &[0, 1, 3, 4],
            SegmentType::ST5A => &[0, 1, 2, 3, 4],
            SegmentType::ST6A => &[0, 1, 2, 3, 4, 5],
        }
    }
}

/// Thông tin phân đoạn địa hình (Segment) được đắp lên ô Tile
#[derive(Debug, Clone)]
pub struct SegmentData {
    pub index: usize,
    pub group_type: GroupType,
    pub segment_type: SegmentType,
    pub occupied_edges: Vec<usize>,
    pub rotation: usize,
    pub is_hybrid: bool,
}

impl SegmentData {
    pub fn config_code(&self) -> String {
        format!("{}{}", self.segment_type.shape_code(), self.group_type.letter())
    }
}

/// Thông tin bài Nhiệm Vụ (Quest)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqualityComparison {
    MoreThan, // Nhiệm vụ "+" (ví dụ: +3)
    Exactly,  // Nhiệm vụ "=" (ví dụ: =2)
}

#[derive(Debug, Clone)]
pub struct QuestTileData {
    pub seed: i32,
    pub quest_type: String,
    pub target_count: usize,
    pub equality: EqualityComparison,
    pub level: usize,
    pub quest_id: Option<usize>,
    pub stack_quest_id: Option<usize>,
}

impl QuestTileData {
    /// Trích xuất chuỗi cấu hình địa hình từ tên prefab quest (ví dụ: "QuestTile_Water_4A_HybridW_2AF" -> "4A_HybridW 2AF")
    pub fn config_string(&self) -> String {
        let clean_name = self.quest_type.replace('-', "_");
        let parts: Vec<&str> = clean_name.split('_').collect();
        let mut seg_codes = Vec::new();

        let mut i = 0;
        while i < parts.len() {
            let part = parts[i].trim();
            if part.is_empty() || part == "QuestTile" || part == "Train" || part == "WaterTrainStation" {
                i += 1;
                continue;
            }

            if i + 1 < parts.len() && parts[i + 1].to_lowercase().contains("hybrid") {
                let combined = format!("{}_{}", part, parts[i + 1]);
                if is_segment_code(&combined) {
                    seg_codes.push(combined);
                    i += 2;
                    continue;
                }
            }

            if is_segment_code(part) {
                seg_codes.push(part.to_string());
            }
            i += 1;
        }

        if !seg_codes.is_empty() {
            seg_codes.join(" ")
        } else {
            self.quest_type.clone()
        }
    }

    /// Xác định GroupType chính của Nhiệm vụ
    pub fn primary_group_type(&self) -> GroupType {
        let name = self.quest_type.to_lowercase();
        if name.contains("village") {
            GroupType::Village
        } else if name.contains("forest") {
            GroupType::Forest
        } else if name.contains("agriculture") {
            GroupType::Agriculture
        } else if name.contains("water") {
            GroupType::Water
        } else if name.contains("train") {
            GroupType::TrainTracks
        } else {
            let cfg = self.config_string();
            for part in cfg.split_whitespace() {
                if let Some((_, gt, _)) = parse_segment_code(part) {
                    return gt;
                }
            }
            GroupType::Agriculture
        }
    }

    /// Đếm tổng số object/element của mảnh QuestTile này thuộc primary_group_type dựa theo config2.txt
    pub fn own_elements(&self) -> usize {
        self.own_elements_for_group(self.primary_group_type())
    }

    /// Đếm tổng số object/element của mảnh QuestTile này thuộc group_type cụ thể
    pub fn own_elements_for_group(&self, gt: GroupType) -> usize {
        let cfg = self.config_string();
        let mut total = 0;

        for part in cfg.split_whitespace() {
            if let Some((seg_type, group_type, _)) = parse_segment_code(part) {
                if group_type == gt {
                    total += crate::game_config::get_segment_element_count(group_type, seg_type);
                }
            }
        }

        if total == 0 {
            if self.primary_group_type() == gt { 1 } else { 0 }
        } else {
            total
        }
    }

    /// Số ô/cây/nhà còn thiếu hiển thị trên bóng bóng nhiệm vụ vàng (RemainingValue = TargetValue - own_elements)
    pub fn remaining_display_value(&self) -> usize {
        let own = self.own_elements();
        if self.target_count > own {
            self.target_count - own
        } else {
            0
        }
    }
}

fn is_segment_code(s: &str) -> bool {
    let lower = s.to_lowercase();
    if lower.contains("hybrid") {
        return true;
    }
    let len = s.len();
    if len < 2 { return false; }
    let last = s.chars().last().unwrap();
    if !['A', 'F', 'T', 'V', 'W'].contains(&last) {
        return false;
    }
    let first_char = s.chars().next().unwrap();
    first_char.is_ascii_digit()
}

pub fn parse_segment_code(code: &str) -> Option<(SegmentType, GroupType, bool)> {
    if code.is_empty() { return None; }
    let is_hybrid_explicit = code.to_lowercase().contains("hybrid");

    let mut cleaned = code.replace("Hybrid", "").replace("hybrid", "").replace('_', "");
    if cleaned.is_empty() { return None; }

    let group_char = cleaned.chars().last()?;
    let group_type = match group_char {
        'A' => GroupType::Agriculture,
        'F' => GroupType::Forest,
        'V' => GroupType::Village,
        'T' => GroupType::TrainTracks,
        'W' => GroupType::Water,
        _ => return None,
    };

    cleaned.pop(); // Remove last group char

    if cleaned.starts_with('h') || cleaned.starts_with('H') {
        cleaned.remove(0);
    }

    let shape_str = cleaned;
    let seg_type = match shape_str.as_str() {
        "1A" => SegmentType::ST1A,
        "2A" => SegmentType::ST2A,
        "2B" => SegmentType::ST2B,
        "2C" => SegmentType::ST2C,
        "3A" => SegmentType::ST3A,
        "3B" => SegmentType::ST3B,
        "3C" => SegmentType::ST3C,
        "3D" => SegmentType::ST3D,
        "4A" => SegmentType::ST4A,
        "4B" => SegmentType::ST4B,
        "4C" => SegmentType::ST4C,
        "5A" => SegmentType::ST5A,
        "6A" => SegmentType::ST6A,
        _ => return None,
    };

    let is_hybrid = is_hybrid_explicit || (group_type == GroupType::Water && seg_type == SegmentType::ST6A);

    Some((seg_type, group_type, is_hybrid))
}

/// Kết quả bài hoàn chỉnh sau khi Generate
#[derive(Debug, Clone)]
pub enum GeneratedTile {
    Normal {
        base_tile: BaseTile,
        segments: Vec<SegmentData>,
    },
    Quest {
        base_tile: BaseTile,
        quest_data: QuestTileData,
    },
    Reward {
        base_tile: BaseTile,
    },
}

impl GeneratedTile {
    /// Tạo tile Station (Reward) đặc biệt với 6 cạnh WaterTrainStation.
    /// 2 segment chồng lên nhau trên cả 6 cạnh: Water (element=1) và TrainTracks (element=6).
    pub fn new_reward_station(id: usize, seed: i32) -> Self {
        GeneratedTile::Reward {
            base_tile: BaseTile::new(id, seed, "SpecialTile"),
        }
    }

    /// Xuất chuỗi cấu hình địa hình giống hệt text vàng trong C# Plugin (ví dụ: "3AA 1AV", "2AA 4AF", "1AF")
    pub fn tile_preset_string(&self) -> String {
        match self {
            GeneratedTile::Normal { segments, .. } => {
                if segments.is_empty() {
                    return "Plain".to_string();
                }
                let codes: Vec<String> = segments.iter().map(|s| s.config_code()).collect();
                codes.join(" ")
            }
            GeneratedTile::Quest { quest_data, .. } => {
                quest_data.config_string()
            }
            GeneratedTile::Reward { .. } => "6AW 6AT".to_string(),
        }
    }

    pub fn base_tile(&self) -> &BaseTile {
        match self {
            GeneratedTile::Normal { base_tile, .. } => base_tile,
            GeneratedTile::Quest { base_tile, .. } => base_tile,
            GeneratedTile::Reward { base_tile, .. } => base_tile,
        }
    }

    /// Trả về chu kỳ đối xứng quay tối thiểu (1, 2, 3 hoặc 6) của Tile.
    /// Ví dụ:
    /// - 6 cạnh đồng nhất (toàn Plain/Water/Station): period = 1 (chỉ cần rotation 0).
    /// - Dạng A-B-A-B-A-B: period = 2 (chỉ cần rotation 0, 1).
    /// - Dạng A-B-C-A-B-C: period = 3 (chỉ cần rotation 0, 1, 2).
    /// - Bất đối xứng: period = 6 (cần cả 6 rotations).
    pub fn rotation_symmetry_period(&self) -> usize {
        let cfg = self.to_hex_edge_config();
        for period in [1, 2, 3] {
            let is_symmetric = (0..6).all(|i| cfg.edges[i] == cfg.edges[(i + period) % 6]);
            if is_symmetric {
                return period;
            }
        }
        6
    }

    /// Chuyển đổi Tile Preset thành cấu hình 6 cạnh hex thật (HexEdgeConfig)
    pub fn to_hex_edge_config(&self) -> HexEdgeConfig {
        let mut edges = [EdgeType::Plain; 6];

        match self {
            GeneratedTile::Normal { segments, .. } => {
                for seg in segments {
                    let base = seg.segment_type.base_edges();
                    let edge_type = match seg.group_type {
                        GroupType::Water => {
                            if seg.segment_type == SegmentType::ST6A || seg.is_hybrid {
                                EdgeType::FlexibleWater
                            } else {
                                EdgeType::Water
                            }
                        }
                        gt => EdgeType::from(gt),
                    };
                    for &b in base {
                        let idx = (b + seg.rotation) % 6;
                        edges[idx] = edge_type;
                    }
                }
            }
            GeneratedTile::Quest { quest_data, .. } => {
                let name_lower = quest_data.quest_type.to_lowercase();
                if name_lower.contains("watertrainstation") || name_lower.contains("water_trainstation") {
                    return HexEdgeConfig::new([EdgeType::WaterTrainStation; 6]);
                }

                let preset_str = quest_data.config_string();
                let parts: Vec<&str> = preset_str.split_whitespace().collect();
                let mut occupied = [false; 6];

                for part in parts {
                    if let Some((seg_type, group_type, is_hybrid)) = parse_segment_code(part) {
                        let base = seg_type.base_edges();
                        let edge_type = match group_type {
                            GroupType::Water => {
                                if seg_type.is_lake_shape() || is_hybrid {
                                    EdgeType::FlexibleWater
                                } else {
                                    EdgeType::Water
                                }
                            }
                            gt => EdgeType::from(gt),
                        };
                        
                        // Tìm rot hợp lệ đầu tiên không bị đè cạnh đã có
                        let mut best_rot = 0;
                        for rot in 0..6 {
                            let fits = base.iter().all(|&b| !occupied[(b + rot) % 6]);
                            if fits {
                                best_rot = rot;
                                break;
                            }
                        }

                        for &b in base {
                            let idx = (b + best_rot) % 6;
                            edges[idx] = edge_type;
                            occupied[idx] = true;
                        }
                    }
                }
            }
            GeneratedTile::Reward { .. } => {
                return HexEdgeConfig::new([EdgeType::WaterTrainStation; 6]);
            }
        }

        HexEdgeConfig::new(edges)
    }

    /// Lấy danh sách các segment cụ thể (TileSegmentData) của ô tile khi đã xoay góc placement_rotation
    pub fn get_segments(&self, placement_rotation: usize) -> Vec<TileSegmentData> {
        let mut result = Vec::new();
        match self {
            GeneratedTile::Normal { segments, .. } => {
                for seg in segments {
                    let base = seg.segment_type.base_edges();
                    let rotated_edges: Vec<usize> = base
                        .iter()
                        .map(|&b| (b + seg.rotation + placement_rotation) % 6)
                        .collect();
                    let element_count = crate::game_config::get_segment_element_count(seg.group_type, seg.segment_type);
                    result.push(TileSegmentData {
                        group_type: seg.group_type,
                        segment_type: seg.segment_type,
                        element_count,
                        edges: rotated_edges,
                    });
                }
            }
            GeneratedTile::Quest { quest_data, .. } => {
                let preset_str = quest_data.config_string();
                let parts: Vec<&str> = preset_str.split_whitespace().collect();
                let mut occupied = [false; 6];

                for part in parts {
                    if let Some((seg_type, group_type, _)) = parse_segment_code(part) {
                        let base = seg_type.base_edges();
                        let mut best_rot = 0;
                        for rot in 0..6 {
                            let fits = base.iter().all(|&b| !occupied[(b + rot) % 6]);
                            if fits {
                                best_rot = rot;
                                break;
                            }
                        }
                        for &b in base {
                            let idx = (b + best_rot) % 6;
                            occupied[idx] = true;
                        }
                        let rotated_edges: Vec<usize> = base
                            .iter()
                            .map(|&b| (b + best_rot + placement_rotation) % 6)
                            .collect();
                        let element_count = crate::game_config::get_segment_element_count(group_type, seg_type);
                        result.push(TileSegmentData {
                            group_type,
                            segment_type: seg_type,
                            element_count,
                            edges: rotated_edges,
                        });
                    }
                }

                if result.is_empty() {
                    let gt = quest_data.primary_group_type();
                    let mut cfg = self.to_hex_edge_config();
                    cfg.rotate(placement_rotation);
                    let edges: Vec<usize> = (0..6).filter(|&d| cfg.edges[d].to_group_type() == Some(gt)).collect();
                    let element_count = quest_data.own_elements_for_group(gt);
                    result.push(TileSegmentData {
                        group_type: gt,
                        segment_type: SegmentType::ST1A,
                        element_count,
                        edges,
                    });
                }
            }
            GeneratedTile::Reward { .. } => {
                // Water segment: element_count = 1 (1 river layer phủ toàn tile)
                result.push(TileSegmentData {
                    group_type: GroupType::Water,
                    segment_type: SegmentType::ST6A,
                    element_count: 1,
                    edges: (0..6).collect(),
                });
                // Train segment: element_count = 6 (6 train tracks, mỗi cạnh 1)
                result.push(TileSegmentData {
                    group_type: GroupType::TrainTracks,
                    segment_type: SegmentType::ST6A,
                    element_count: 6,
                    edges: (0..6).collect(),
                });
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
pub struct TileSegmentData {
    pub group_type: GroupType,
    pub segment_type: SegmentType,
    pub element_count: usize,
    pub edges: Vec<usize>,
}

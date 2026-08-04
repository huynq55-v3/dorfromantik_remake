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
            EdgeType::Water => Some(GroupType::Water),
            EdgeType::TrainTracks => Some(GroupType::TrainTracks),
            EdgeType::Plain => None,
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
}

impl QuestTileData {
    /// Trích xuất chuỗi cấu hình địa hình từ tên prefab quest (ví dụ: "QuestTile_Agriculture_3AA_1AV_Windmill" -> "3AA 1AV")
    pub fn config_string(&self) -> String {
        let clean_name = self.quest_type.replace('-', "_");
        let parts: Vec<&str> = clean_name.split('_').collect();
        let mut seg_codes = Vec::new();

        for part in parts {
            let part = part.trim();
            if part.is_empty() || part == "QuestTile" || part == "Train" || part == "WaterTrainStation" {
                continue;
            }
            if is_segment_code(part) {
                seg_codes.push(part.to_string());
            }
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
        if name.contains("village") || name.contains("house") {
            GroupType::Village
        } else if name.contains("forest") || name.contains("tree") {
            GroupType::Forest
        } else if name.contains("agriculture") || name.contains("field") || name.contains("windmill") || name.contains("granary") {
            GroupType::Agriculture
        } else if name.contains("water") || name.contains("river") || name.contains("boat") {
            GroupType::Water
        } else if name.contains("train") || name.contains("rail") || name.contains("station") {
            GroupType::TrainTracks
        } else {
            let cfg = self.config_string();
            for part in cfg.split_whitespace() {
                if let Some((_, gt)) = parse_segment_code(part) {
                    return gt;
                }
            }
            GroupType::Agriculture
        }
    }

    /// Đếm tổng số object/element của mảnh QuestTile này thuộc primary_group_type dựa theo config2.txt
    pub fn own_elements(&self) -> usize {
        let primary_gt = self.primary_group_type();
        let cfg = self.config_string();
        let mut total = 0;

        for part in cfg.split_whitespace() {
            if let Some((seg_type, group_type)) = parse_segment_code(part) {
                if group_type == primary_gt {
                    total += crate::game_config::get_segment_element_count(group_type, seg_type);
                }
            }
        }

        if total == 0 {
            1
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
    let len = s.len();
    if len < 2 { return false; }
    let last = s.chars().last().unwrap();
    if !['A', 'F', 'T', 'V', 'W'].contains(&last) {
        return false;
    }
    let first_char = s.chars().next().unwrap();
    first_char.is_ascii_digit()
}

pub fn parse_segment_code(code: &str) -> Option<(SegmentType, GroupType)> {
    if code.len() < 2 { return None; }
    let group_char = code.chars().last()?;
    let group_type = match group_char {
        'A' => GroupType::Agriculture,
        'F' => GroupType::Forest,
        'V' => GroupType::Village,
        'T' => GroupType::TrainTracks,
        'W' => GroupType::Water,
        _ => return None,
    };
    let shape_str = &code[..code.len() - 1];
    let seg_type = match shape_str {
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
    Some((seg_type, group_type))
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
}

impl GeneratedTile {
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
        }
    }

    pub fn base_tile(&self) -> &BaseTile {
        match self {
            GeneratedTile::Normal { base_tile, .. } => base_tile,
            GeneratedTile::Quest { base_tile, .. } => base_tile,
        }
    }

    /// Chuyển đổi Tile Preset thành cấu hình 6 cạnh hex thật (HexEdgeConfig)
    pub fn to_hex_edge_config(&self) -> HexEdgeConfig {
        let mut edges = [EdgeType::Plain; 6];

        match self {
            GeneratedTile::Normal { segments, .. } => {
                for seg in segments {
                    let base = seg.segment_type.base_edges();
                    let edge_type = EdgeType::from(seg.group_type);
                    for &b in base {
                        let idx = (b + seg.rotation) % 6;
                        edges[idx] = edge_type;
                    }
                }
            }
            GeneratedTile::Quest { quest_data, .. } => {
                let preset_str = quest_data.config_string();
                let parts: Vec<&str> = preset_str.split_whitespace().collect();
                let mut occupied = [false; 6];

                for part in parts {
                    if let Some((seg_type, group_type)) = parse_segment_code(part) {
                        let base = seg_type.base_edges();
                        let edge_type = EdgeType::from(group_type);
                        
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
        }

        HexEdgeConfig::new(edges)
    }
}

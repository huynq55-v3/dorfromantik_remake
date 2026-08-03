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

/// Thông tin phân đoạn địa hình (Segment) được đắp lên ô Tile
#[derive(Debug, Clone)]
pub struct SegmentData {
    pub index: usize,
    pub group_type: GroupType,
    pub segment_type: SegmentType,
    pub occupied_edges: Vec<usize>,
    pub rotation: usize,
}

/// Thông tin bài Nhiệm Vụ (Quest)
#[derive(Debug, Clone)]
pub struct QuestTileData {
    pub seed: i32,
    pub quest_type: String,
    pub target_count: usize,
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

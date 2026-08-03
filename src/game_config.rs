use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GroupType {
    Village,
    Forest,
    Agriculture,
    TrainTracks,
    Water,
}

#[derive(Debug, Clone)]
pub struct GroupTypeConfiguration {
    pub group_type: GroupType,
    pub raw_probability: f32,
    pub probability_in_percent: f32,
    pub display_probability: f32,
}

#[derive(Debug, Clone)]
pub struct GlobalGroupTypeConfiguration {
    pub global_group_type_probabilities: Vec<GroupTypeConfiguration>,
}

impl GlobalGroupTypeConfiguration {
    pub fn default_table() -> Self {
        let raws = vec![
            (GroupType::Village, 10.0),
            (GroupType::Forest, 10.0),
            (GroupType::Agriculture, 10.0),
            (GroupType::TrainTracks, 5.0),
            (GroupType::Water, 7.0),
        ];

        let total: f32 = raws.iter().map(|(_, r)| r).sum();

        let global_group_type_probabilities: Vec<GroupTypeConfiguration> = raws
            .into_iter()
            .map(|(gt, raw)| {
                let prob_in_percent = raw / total;
                GroupTypeConfiguration {
                    group_type: gt,
                    raw_probability: raw,
                    probability_in_percent: prob_in_percent,
                    display_probability: (prob_in_percent * 100.0 * 10.0).round() / 10.0,
                }
            })
            .collect();

        GlobalGroupTypeConfiguration { global_group_type_probabilities }
    }
}

/* ==================================================================================================== */
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SegmentType {
    ST1A,
    ST2A,
    ST2B,
    ST2C,
    ST3A,
    ST3B,
    ST3C,
    ST3D,
    ST4A,
    ST4B,
    ST4C,
    ST5A,
    ST6A,
}

/* ==================================================================================================== */
#[derive(Debug, Clone)]
pub struct SegmentPresetCollection {
    pub collection_name: String,
    pub group_type_probabilities: Vec<GroupTypeConfiguration>,
    pub segment_presets: Vec<SegmentType>,
}

#[derive(Debug, Clone)]
pub struct SegmentPresetConfiguration {
    pub segment_type: SegmentType,
    pub possible_types: Vec<GroupTypeConfiguration>,
}

#[derive(Debug, Clone)]
pub struct SegmentPresetConfigurations {
    pub segment_preset_collections: Vec<SegmentPresetCollection>,
    pub all_segment_presets: Vec<SegmentPresetConfiguration>,
}

impl SegmentPresetConfigurations {
    fn pct_for(list: &[GroupTypeConfiguration], gt: GroupType) -> f32 {
        list.iter().find(|g| g.group_type == gt).map(|g| g.probability_in_percent).unwrap_or(0.0)
    }

    pub fn default() -> Self {
        let global = GlobalGroupTypeConfiguration::default_table();
        let global_pcts: Vec<(GroupType, f32)> = global.global_group_type_probabilities.iter()
            .map(|g| (g.group_type, g.probability_in_percent)).collect();

        fn global_pct_for(list: &[(GroupType, f32)], gt: GroupType) -> f32 {
            list.iter().find(|(g, _)| *g == gt).map(|(_, p)| *p).unwrap_or(0.0)
        }

        let raw_collections = vec![
            (
                "Segment Collection 1X",
                vec![
                    (GroupType::Agriculture, 10.0),
                    (GroupType::Forest, 10.0),
                    (GroupType::Village, 10.0),
                    (GroupType::TrainTracks, 1.0),
                    (GroupType::Water, 0.0),
                ],
                vec![SegmentType::ST1A],
            ),
            (
                "Segment Collection 2X",
                vec![
                    (GroupType::Agriculture, 5.0),
                    (GroupType::Forest, 5.0),
                    (GroupType::Village, 5.0),
                    (GroupType::TrainTracks, 15.0),
                    (GroupType::Water, 15.0),
                ],
                vec![SegmentType::ST2A, SegmentType::ST2B, SegmentType::ST2C],
            ),
            (
                "Segment Collection 3X",
                vec![
                    (GroupType::Agriculture, 10.0),
                    (GroupType::Forest, 10.0),
                    (GroupType::Village, 10.0),
                    (GroupType::TrainTracks, 4.0),
                    (GroupType::Water, 4.0),
                ],
                vec![SegmentType::ST3A, SegmentType::ST3B, SegmentType::ST3C, SegmentType::ST3D],
            ),
            (
                "Segment Collection 4X",
                vec![
                    (GroupType::Agriculture, 10.0),
                    (GroupType::Forest, 10.0),
                    (GroupType::Village, 10.0),
                    (GroupType::TrainTracks, 3.0),
                    (GroupType::Water, 3.0),
                ],
                vec![SegmentType::ST4A, SegmentType::ST4B, SegmentType::ST4C],
            ),
            (
                "Segment Collection 5X",
                vec![
                    (GroupType::Agriculture, 10.0),
                    (GroupType::Forest, 10.0),
                    (GroupType::Village, 10.0),
                    (GroupType::TrainTracks, 2.0),
                    (GroupType::Water, 6.0),
                ],
                vec![SegmentType::ST5A],
            ),
            (
                "Segment Collection 6X",
                vec![
                    (GroupType::Agriculture, 10.0),
                    (GroupType::Forest, 10.0),
                    (GroupType::Village, 10.0),
                    (GroupType::TrainTracks, 0.0),
                    (GroupType::Water, 20.0),
                ],
                vec![SegmentType::ST6A],
            ),
        ];

        let mut collections: Vec<SegmentPresetCollection> = Vec::new();
        for (name, raws, presets) in raw_collections {
            let weighted_sum: f32 = raws.iter()
                .map(|(gt, r)| r * global_pct_for(&global_pcts, *gt))
                .sum();
            let gtp: Vec<GroupTypeConfiguration> = raws.iter().map(|(gt, raw)| {
                let w = *raw * global_pct_for(&global_pcts, *gt);
                let pct = if weighted_sum > 0.0 { w / weighted_sum } else { 0.0 };
                GroupTypeConfiguration {
                    group_type: *gt,
                    raw_probability: *raw,
                    probability_in_percent: pct,
                    display_probability: (pct * 100.0 * 10.0).round() / 10.0,
                }
            }).collect();

            collections.push(SegmentPresetCollection {
                collection_name: name.into(),
                group_type_probabilities: gtp,
                segment_presets: presets,
            });
        }

        fn coll_pcts_for<'a>(collections: &'a [SegmentPresetCollection], st: SegmentType) -> &'a [GroupTypeConfiguration] {
            collections.iter()
                .find(|c| c.segment_presets.contains(&st))
                .map(|c| c.group_type_probabilities.as_slice())
                .unwrap_or(&[])
        }

        let all_possible_types_raws: Vec<(SegmentType, Vec<(GroupType, f32)>)> = vec![
            (SegmentType::ST1A, vec![(GroupType::Agriculture, 10.0), (GroupType::Forest, 10.0), (GroupType::Village, 10.0), (GroupType::TrainTracks, 10.0), (GroupType::Water, 0.0)]),
            (SegmentType::ST2A, vec![(GroupType::Agriculture, 10.0), (GroupType::Forest, 10.0), (GroupType::Village, 10.0), (GroupType::TrainTracks, 2.0), (GroupType::Water, 3.0)]),
            (SegmentType::ST2B, vec![(GroupType::Agriculture, 10.0), (GroupType::Forest, 10.0), (GroupType::Village, 10.0), (GroupType::TrainTracks, 15.0), (GroupType::Water, 15.0)]),
            (SegmentType::ST2C, vec![(GroupType::Agriculture, 10.0), (GroupType::Forest, 10.0), (GroupType::Village, 10.0), (GroupType::TrainTracks, 20.0), (GroupType::Water, 15.0)]),
            (SegmentType::ST3A, vec![(GroupType::Agriculture, 10.0), (GroupType::Forest, 10.0), (GroupType::Village, 10.0), (GroupType::TrainTracks, 5.0), (GroupType::Water, 35.0)]),
            (SegmentType::ST3B, vec![(GroupType::Agriculture, 10.0), (GroupType::Forest, 10.0), (GroupType::Village, 10.0), (GroupType::TrainTracks, 10.0), (GroupType::Water, 10.0)]),
            (SegmentType::ST3C, vec![(GroupType::Agriculture, 10.0), (GroupType::Forest, 10.0), (GroupType::Village, 10.0), (GroupType::TrainTracks, 10.0), (GroupType::Water, 10.0)]),
            (SegmentType::ST3D, vec![(GroupType::Agriculture, 3.0), (GroupType::Forest, 3.0), (GroupType::Village, 2.0), (GroupType::TrainTracks, 20.0), (GroupType::Water, 10.0)]),
            (SegmentType::ST4A, vec![(GroupType::Agriculture, 10.0), (GroupType::Forest, 10.0), (GroupType::Village, 10.0), (GroupType::TrainTracks, 10.0), (GroupType::Water, 40.0)]),
            (SegmentType::ST4B, vec![(GroupType::Agriculture, 10.0), (GroupType::Forest, 10.0), (GroupType::Village, 10.0), (GroupType::TrainTracks, 10.0), (GroupType::Water, 10.0)]),
            (SegmentType::ST4C, vec![(GroupType::Agriculture, 10.0), (GroupType::Forest, 10.0), (GroupType::Village, 10.0), (GroupType::TrainTracks, 10.0), (GroupType::Water, 10.0)]),
            (SegmentType::ST5A, vec![(GroupType::Agriculture, 10.0), (GroupType::Forest, 10.0), (GroupType::Village, 10.0), (GroupType::TrainTracks, 10.0), (GroupType::Water, 20.0)]),
            (SegmentType::ST6A, vec![(GroupType::Agriculture, 10.0), (GroupType::Forest, 10.0), (GroupType::Village, 10.0), (GroupType::TrainTracks, 10.0), (GroupType::Water, 10.0)]),
        ];

        let all_segment_presets: Vec<SegmentPresetConfiguration> = all_possible_types_raws.into_iter().map(|(st, raws)| {
            let coll_pcts = coll_pcts_for(&collections, st);
            let num2: f32 = raws.iter().map(|(gt, raw)| {
                raw * Self::pct_for(coll_pcts, *gt)
            }).sum();
            let possible_types = raws.into_iter().map(|(gt, raw)| {
                let cp = Self::pct_for(coll_pcts, gt);
                let pct = if num2 > 0.0 { raw * cp / num2 } else { 0.0 };
                GroupTypeConfiguration {
                    group_type: gt,
                    raw_probability: raw,
                    probability_in_percent: pct,
                    display_probability: (pct * 100.0 * 10.0).round() / 10.0,
                }
            }).collect();
            SegmentPresetConfiguration { segment_type: st, possible_types }
        }).collect();

        SegmentPresetConfigurations {
            segment_preset_collections: collections,
            all_segment_presets,
        }
    }
}

/* ==================================================================================================== */
#[derive(Debug, Clone)]
pub struct TilePresetConfiguration {
    pub name: String,
    pub raw_probability: f32,
    pub final_probability: f32,
    pub occupied_edges: usize,
    pub segments: Vec<SegmentType>,
}

#[derive(Debug, Clone)]
pub struct TilePresetSubCollection {
    pub name: String,
    pub raw_probability: f32,
    pub sub_probability: f32,
    pub tile_presets: Vec<TilePresetConfiguration>,
}

#[derive(Debug, Clone)]
pub struct TilePresetCollection {
    pub name: String,
    pub raw_probability: f32,
    pub collection_probability: f32,
    pub tile_presets: Vec<TilePresetConfiguration>,
    pub sub_collections: Vec<TilePresetSubCollection>,
}

#[derive(Debug, Clone)]
pub struct TilePresetConfigurations {
    pub all_tile_presets: Vec<TilePresetCollection>,
    pub all_tiles_flat: Vec<TilePresetConfiguration>,
}

pub fn parse_segments(name: &str) -> Vec<SegmentType> {
    name.split('_').filter_map(|s| match s {
        "1A" => Some(SegmentType::ST1A),
        "2A" => Some(SegmentType::ST2A),
        "2B" => Some(SegmentType::ST2B),
        "2C" => Some(SegmentType::ST2C),
        "3A" => Some(SegmentType::ST3A),
        "3B" => Some(SegmentType::ST3B),
        "3C" => Some(SegmentType::ST3C),
        "3D" => Some(SegmentType::ST3D),
        "4A" => Some(SegmentType::ST4A),
        "4B" => Some(SegmentType::ST4B),
        "4C" => Some(SegmentType::ST4C),
        "5A" => Some(SegmentType::ST5A),
        "6A" => Some(SegmentType::ST6A),
        _ => None,
    }).collect()
}

const TOTAL_COLLECTION_RAW: f32 = 1669.68;

fn tp_c(name: &str, raw: f32, occupied: usize) -> TilePresetConfiguration {
    TilePresetConfiguration {
        name: name.into(),
        raw_probability: raw,
        final_probability: raw / TOTAL_COLLECTION_RAW,
        occupied_edges: occupied,
        segments: parse_segments(name),
    }
}

fn tp_sub(name: &str, raw: f32, tiles: Vec<(&str, f32, usize)>) -> TilePresetSubCollection {
    TilePresetSubCollection {
        name: name.into(),
        raw_probability: raw,
        sub_probability: raw / TOTAL_COLLECTION_RAW,
        tile_presets: tiles.into_iter().map(|(n, r, o)| tp_c(n, r, o)).collect(),
    }
}

impl TilePresetConfigurations {
    pub fn default() -> Self {
        let total_collection_raw: f32 = TOTAL_COLLECTION_RAW;

        let collections = vec![
            TilePresetCollection {
                name: "Collection 0X".into(),
                raw_probability: 35.1,
                collection_probability: 35.1 / total_collection_raw,
                tile_presets: vec![tp_c("0A", 35.1, 0)],
                sub_collections: vec![],
            },
            TilePresetCollection {
                name: "Collection 1X".into(),
                raw_probability: 189.52,
                collection_probability: 189.52 / total_collection_raw,
                tile_presets: vec![
                    tp_c("1A", 70.2, 1),
                    tp_c("1A_1A", 58.5, 2),
                    tp_c("1A_1A_1A", 40.27, 3),
                    tp_c("1A_1A_1A_1A", 14.77, 4),
                    tp_c("1A_1A_1A_1A_1A", 3.76, 5),
                    tp_c("1A_1A_1A_1A_1A_1A", 2.02, 6),
                ],
                sub_collections: vec![],
            },
            TilePresetCollection {
                name: "Collection 2X".into(),
                raw_probability: 496.02,
                collection_probability: 496.02 / total_collection_raw,
                tile_presets: vec![],
                sub_collections: vec![
                    tp_sub("Collection 2A", 217.68, vec![
                        ("2A", 64.8, 2),
                        ("2A_1A", 54.0, 3),
                        ("2A_1A_1A", 27.27, 4),
                        ("2A_1A_1A_1A", 10.0, 5),
                        ("2A_1A_1A_1A_1A", 2.6, 6),
                        ("2A_2A", 29.46, 4),
                        ("2A_2A_1A", 18.46, 5),
                        ("2A_2A_1A_1A", 2.31, 6),
                        ("2A_2A_2A", 8.78, 6),
                    ]),
                    tp_sub("Collection 2B", 175.4, vec![
                        ("2B", 49.55, 2),
                        ("2B_1A", 41.29, 3),
                        ("2B_1A_1A", 20.86, 4),
                        ("2B_1A_1A_1A", 7.65, 5),
                        ("2B_1A_1A_1A_1A", 1.99, 6),
                        ("2B_2A", 34.65, 4),
                        ("2B_2A_1A", 14.12, 5),
                        ("2B_2A_1A_1A", 5.29, 6),
                    ]),
                    tp_sub("Collection 2C", 102.94, vec![
                        ("2C", 28.08, 2),
                        ("2C_1A", 23.4, 3),
                        ("2C_1A_1A", 11.82, 4),
                        ("2C_1A_1A_1A", 4.33, 5),
                        ("2C_1A_1A_1A_1A", 1.13, 6),
                        ("2C_2A", 19.64, 4),
                        ("2C_2A_1A", 8.0, 5),
                        ("2C_2A_1A_1A", 1.0, 6),
                        ("2C_2A_2A", 5.54, 6),
                    ]),
                ],
            },
            TilePresetCollection {
                name: "Collection 3X".into(),
                raw_probability: 533.02,
                collection_probability: 533.02 / total_collection_raw,
                tile_presets: vec![],
                sub_collections: vec![
                    tp_sub("Collection 3A", 198.83, vec![
                        ("3A", 52.65, 3),
                        ("3A_1A", 39.89, 4),
                        ("3A_1A_1A", 16.25, 5),
                        ("3A_1A_1A_1A", 6.09, 6),
                        ("3A_2A", 27.0, 5),
                        ("3A_2A_1A", 11.25, 6),
                        ("3A_2B", 20.65, 5),
                        ("3A_2B_1A", 8.6, 6),
                        ("3A_3A", 16.45, 6),
                    ]),
                    tp_sub("Collection 3B", 144.12, vec![
                        ("3B", 49.55, 3),
                        ("3B_1A", 37.54, 4),
                        ("3B_1A_1A", 15.29, 5),
                        ("3B_1A_1A_1A", 5.74, 6),
                        ("3B_2A", 25.41, 5),
                        ("3B_2A_1A", 10.59, 6),
                    ]),
                    tp_sub("Collection 3C", 144.12, vec![
                        ("3C", 49.55, 3),
                        ("3C_1A", 37.54, 4),
                        ("3C_1A_1A", 15.29, 5),
                        ("3C_1A_1A_1A", 5.74, 6),
                        ("3C_2A", 25.41, 5),
                        ("3C_2A_1A", 10.59, 6),
                    ]),
                    tp_sub("Collection 3D", 45.95, vec![
                        ("3D", 21.06, 3),
                        ("3D_1A", 15.95, 4),
                        ("3D_1A_1A", 6.5, 5),
                        ("3D_1A_1A_1A", 2.44, 6),
                    ]),
                ],
            },
            TilePresetCollection {
                name: "Collection 4X".into(),
                raw_probability: 285.65,
                collection_probability: 285.65 / total_collection_raw,
                tile_presets: vec![],
                sub_collections: vec![
                    tp_sub("Collection 4A", 134.83, vec![
                        ("4A", 58.91, 4),
                        ("4A_1A", 36.0, 5),
                        ("4A_1A_1A", 15.0, 6),
                        ("4A_2A", 24.92, 6),
                    ]),
                    tp_sub("Collection 4B", 71.44, vec![
                        ("4B", 38.29, 4),
                        ("4B_1A", 23.4, 5),
                        ("4B_1A_1A", 9.75, 6),
                    ]),
                    tp_sub("Collection 4C", 79.38, vec![
                        ("4C", 42.55, 4),
                        ("4C_1A", 26.0, 5),
                        ("4C_1A_1A", 10.83, 6),
                    ]),
                ],
            },
            TilePresetCollection {
                name: "Collection 5X".into(),
                raw_probability: 70.2,
                collection_probability: 70.2 / total_collection_raw,
                tile_presets: vec![
                    tp_c("5A", 43.2, 5),
                    tp_c("5A_1A", 27.0, 6),
                ],
                sub_collections: vec![],
            },
            TilePresetCollection {
                name: "Collection 6X".into(),
                raw_probability: 60.17,
                collection_probability: 60.17 / total_collection_raw,
                tile_presets: vec![tp_c("6A", 60.17, 6)],
                sub_collections: vec![],
            },
        ];

        let mut all_tiles_flat = Vec::new();
        for coll in &collections {
            for tile in &coll.tile_presets {
                all_tiles_flat.push(tile.clone());
            }
            for sub in &coll.sub_collections {
                for tile in &sub.tile_presets {
                    all_tiles_flat.push(tile.clone());
                }
            }
        }

        TilePresetConfigurations {
            all_tile_presets: collections,
            all_tiles_flat,
        }
    }
}

/* ==================================================================================================== */
/* QUEST TILE CONFIGURATION (THỨ TỰ VÀ TRỌNG SỐ CHUẨN TỪ UNITY RAM DUMP)                                */
/* ==================================================================================================== */

#[derive(Debug, Clone)]
pub struct QuestOption {
    pub prefab_name: String,
    pub probability: f32,
}

#[derive(Debug, Clone)]
pub struct QuestSubCollection {
    pub probability: f32,
    pub occupied_edges: usize,
    pub all_segment_types: Vec<GroupType>,
    pub quest_tiles: Vec<QuestOption>,
}

#[derive(Debug, Clone)]
pub struct QuestCollection {
    pub name: String,
    pub group_type: GroupType,
    pub probability: f32,
    pub sub_collections: Vec<QuestSubCollection>,
}

#[derive(Debug, Clone)]
pub struct QuestConfigurations {
    pub collections: Vec<QuestCollection>,
    pub excluded_group_types: Vec<GroupType>,
}

impl QuestConfigurations {
    /// Khởi tạo cấu hình Quest bằng cách đọc file monthly_game_info.txt (hoặc mặc định nếu không thấy file)
    pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
        let mut forest_prob = 10.0;
        let mut agri_prob = 10.0;
        let mut village_prob = 10.0;
        let mut train_prob = 6.0;
        let mut water_prob = 8.0;

        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                if let Some((key, val)) = line.split_once('=') {
                    let key = key.trim();
                    let val = val.trim();
                    if let Ok(num) = val.parse::<f32>() {
                        match key {
                            "ACTIVE_ForestProbability" => forest_prob = num,
                            "ACTIVE_AgricultureProbability" => agri_prob = num,
                            "ACTIVE_VillageProbability" => village_prob = num,
                            "ACTIVE_TrainTrackProbability" => train_prob = num,
                            "ACTIVE_WaterProbability" => water_prob = num,
                            _ => {}
                        }
                    }
                }
            }
        }

        Self::new(forest_prob, agri_prob, village_prob, train_prob, water_prob)
    }

    /// Khởi tạo động cấu hình Quest và tự động tính toán mảng excluded_group_types theo C# dòng 7467-7477
    pub fn new(forest_prob: f32, agri_prob: f32, village_prob: f32, train_prob: f32, water_prob: f32) -> Self {
        // Trong C# (dòng 7470): Nếu xác suất nhóm địa hình == 0.0 thì thêm nhóm đó vào excludedGroupTypes
        let mut excluded_group_types = Vec::new();
        if forest_prob <= 0.0 { excluded_group_types.push(GroupType::Forest); }
        if agri_prob <= 0.0 { excluded_group_types.push(GroupType::Agriculture); }
        if village_prob <= 0.0 { excluded_group_types.push(GroupType::Village); }
        if train_prob <= 0.0 { excluded_group_types.push(GroupType::TrainTracks); }
        if water_prob <= 0.0 { excluded_group_types.push(GroupType::Water); }

        let collections = vec![
            // ── Collection 1: Forest Quests (Vị trí #1 trong questTileCollections của Unity) ──
            QuestCollection {
                name: "Forest Quest Collection".into(),
                group_type: GroupType::Forest,
                probability: forest_prob,
                sub_collections: vec![
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 1,
                        all_segment_types: vec![GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Forest_1AF".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Forest_1AF_Deer".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Forest_1AF_Bear".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Forest_1AF_Boar".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 3,
                        all_segment_types: vec![GroupType::Forest, GroupType::Water],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Forest_1AF_2AW".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Forest_1AF_2AW_Deer".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 2,
                        all_segment_types: vec![GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Forest_2AF".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Forest_2AF_Deer".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Forest_2AF_Bear".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Forest_2AF_Boar".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 3,
                        all_segment_types: vec![GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Forest_3AF".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Forest_3AF_Deer".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Forest_3AF_Bear".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Forest_3AF_Boar".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 4,
                        all_segment_types: vec![GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Forest_4AF".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Forest_4AF_Ruin".into(), probability: 0.25 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Forest_6AF".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Forest_6AF_Deer".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Forest_6AF_Ruin".into(), probability: 0.5 },
                            QuestOption { prefab_name: "QuestTile_Forest_6AF_Bear".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Forest_6AF_Boar".into(), probability: 1.0 },
                        ],
                    },
                ],
            },

            // ── Collection 2: Agriculture Quests (Vị trí #2 trong questTileCollections của Unity) ──
            QuestCollection {
                name: "Agriculture Quest Collection".into(),
                group_type: GroupType::Agriculture,
                probability: agri_prob,
                sub_collections: vec![
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 2,
                        all_segment_types: vec![GroupType::Agriculture],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Agriculture_2AA".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 5,
                        all_segment_types: vec![GroupType::Agriculture, GroupType::Village],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Agriculture_2AA_2AV_1AV".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::Agriculture, GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Agriculture_2AA_4AF".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Agriculture_2AA_4AF_Windmill".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Agriculture_2AA_4AF_Granary".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Agriculture_2AA_4AF_BigTree".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 4,
                        all_segment_types: vec![GroupType::Agriculture, GroupType::Village],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Agriculture_3AA_1AV".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Agriculture_3AA_1AV_Windmill".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Agriculture_3AA_1AV_Granary".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::Agriculture, GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Agriculture_4BA_1AF_1AF".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Agriculture_4BA_1AF_1AF_BigTree".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::Agriculture, GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Agriculture_4AA_2AF".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Agriculture_4AA_2AF_Granary".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::Agriculture],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Agriculture_6AA".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Agriculture_6AA_Windmill".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Agriculture_6AA_BigTree".into(), probability: 1.0 },
                        ],
                    },
                ],
            },

            // ── Collection 3: Village Quests (Vị trí #3 trong questTileCollections của Unity) ──
            QuestCollection {
                name: "Village Quest Collection".into(),
                group_type: GroupType::Village,
                probability: village_prob,
                sub_collections: vec![
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 2,
                        all_segment_types: vec![GroupType::Village],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Village_2AV".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::Village, GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Village_3AV_3AF".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Village_3AV_3AF_Fountain".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Village_3AV_3AF_Tower".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Village_3AV_3AF_Fox".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::Village, GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Village_4BV_1AF_1AF".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Village_4BV_1AF_1AF_Fountain".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Village_4BV_1AF_1AF_Tower".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Village_4BV_1AF_1AF_Fox".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::Village, GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Village_5AV_1AF".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Village_5AV_1AF_Fox".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::Village],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Village_6AV".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Village_6AV_Fountain".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Village_6AV_Tower".into(), probability: 1.0 },
                        ],
                    },
                ],
            },

            // ── Collection 4: Train Quests (Vị trí #4 trong questTileCollections của Unity) ──
            QuestCollection {
                name: "Train Quest Collection".into(),
                group_type: GroupType::TrainTracks,
                probability: train_prob,
                sub_collections: vec![
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::TrainTracks, GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Train_2BT-3AF-1AF".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 5,
                        all_segment_types: vec![GroupType::TrainTracks, GroupType::Agriculture],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Train_2BT-2AA-1AA".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::TrainTracks, GroupType::Village],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Train_2BT-3AV-1AV".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 2,
                        all_segment_types: vec![GroupType::TrainTracks],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Train_2CT".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Train_2CT_Locomotive".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 4,
                        all_segment_types: vec![GroupType::TrainTracks, GroupType::Forest, GroupType::Village],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Train_2CT-1AF-1AV".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Train_2CT-1AF-1AV_Locomotive".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::TrainTracks, GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Train_4CT_1AF_1AF".into(), probability: 1.0 },
                        ],
                    },
                ],
            },

            // ── Collection 5: Water Quests (Vị trí #5 trong questTileCollections của Unity) ──
            QuestCollection {
                name: "Water Quest Collection".into(),
                group_type: GroupType::Water,
                probability: water_prob,
                sub_collections: vec![
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::Water, GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Water_2BW_3AF_1AF".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Water_2BW_3AF_1AF_Boat".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 2,
                        all_segment_types: vec![GroupType::Water],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Water_2CW".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Water_2CW_Boat".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 5,
                        all_segment_types: vec![GroupType::Water, GroupType::Village],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Water_2CW_2AV_1AV".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Water_2CW_2AV_2AV_Watermill".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 5,
                        all_segment_types: vec![GroupType::Water, GroupType::Forest, GroupType::Agriculture],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Water_2CW_2AF_1AA".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Water_2CW_2AF_1AA_Watermill".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 5,
                        all_segment_types: vec![GroupType::Water, GroupType::Agriculture, GroupType::Village],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Water_2CW_2AA_1AV".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Water_2CW_2AA_1AV_Watermill".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::Water, GroupType::Forest, GroupType::Agriculture],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Water_2CW_2AF_2AA".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Water_2CW_2AF_2AA_Beaver".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::Water, GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Water_3AW_3AF".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Water_3AW_3AF_Beaver".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Water_3AW_3AF_SwanGoose".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::Water, GroupType::Forest],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Water_4AW_2AF".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Water_4AW_2AF_Beaver".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Water_4AW_2AF_SwanGoose".into(), probability: 1.0 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 6,
                        all_segment_types: vec![GroupType::Water],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_Water_6AW".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Water_6AW_Boat".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Water_6AW_Beaver".into(), probability: 1.0 },
                            QuestOption { prefab_name: "QuestTile_Water_6AW_Ruin".into(), probability: 0.5 },
                        ],
                    },
                    QuestSubCollection {
                        probability: 1.0, occupied_edges: 12,
                        all_segment_types: vec![GroupType::Water, GroupType::TrainTracks],
                        quest_tiles: vec![
                            QuestOption { prefab_name: "QuestTile_WaterTrainStation_6AW_6AT".into(), probability: 1.0 },
                        ],
                    },
                ],
            },
        ];

        QuestConfigurations { collections, excluded_group_types }
    }

    pub fn default() -> Self {
        Self::from_file("monthly_game_info.txt")
    }
}

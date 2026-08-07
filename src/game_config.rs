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
        Self::new(10.0, 10.0, 10.0, 5.0, 7.0)
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
        let mut village_prob = 10.0;
        let mut forest_prob = 10.0;
        let mut agri_prob = 10.0;
        let mut train_prob = 5.0;
        let mut water_prob = 7.0;

        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                if let Some((key, val)) = line.split_once('=') {
                    let key = key.trim();
                    let val = val.trim();
                    if let Ok(num) = val.parse::<f32>() {
                        match key {
                            "ACTIVE_VillageProbability" => village_prob = num,
                            "ACTIVE_ForestProbability" => forest_prob = num,
                            "ACTIVE_AgricultureProbability" => agri_prob = num,
                            "ACTIVE_TrainTrackProbability" => train_prob = num,
                            "ACTIVE_WaterProbability" => water_prob = num,
                            _ => {}
                        }
                    }
                }
            }
        }

        Self::new(village_prob, forest_prob, agri_prob, train_prob, water_prob)
    }

    pub fn new(village_prob: f32, forest_prob: f32, agri_prob: f32, train_prob: f32, water_prob: f32) -> Self {
        let raws = vec![
            (GroupType::Village, village_prob),
            (GroupType::Forest, forest_prob),
            (GroupType::Agriculture, agri_prob),
            (GroupType::TrainTracks, train_prob),
            (GroupType::Water, water_prob),
        ];

        let total: f32 = raws.iter().map(|(_, r)| r).sum();

        let global_group_type_probabilities: Vec<GroupTypeConfiguration> = raws
            .into_iter()
            .map(|(gt, raw)| {
                let prob_in_percent = if total > 0.0 { raw / total } else { 0.0 };
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

    pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
        let global = GlobalGroupTypeConfiguration::from_file(path);
        Self::with_global(global)
    }

    pub fn default() -> Self {
        let global = GlobalGroupTypeConfiguration::default_table();
        Self::with_global(global)
    }

    pub fn with_global(global: GlobalGroupTypeConfiguration) -> Self {
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

        let mut collections = vec![
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

        // Recalculate probabilities matching Unity C# logic (UpdateAllTilePresetsList in Dorfromantik2.cs)
        for coll in &mut collections {
            coll.collection_probability = coll.raw_probability / total_collection_raw;

            if !coll.sub_collections.is_empty() {
                let sum_sub_raws: f32 = coll.sub_collections.iter().map(|s| s.raw_probability).sum();
                for sub in &mut coll.sub_collections {
                    sub.sub_probability = if sum_sub_raws == 0.0 {
                        0.0
                    } else {
                        (sub.raw_probability / sum_sub_raws) * coll.collection_probability
                    };

                    let sum_tile_raws: f32 = sub.tile_presets.iter().map(|t| t.raw_probability).sum();
                    for tile in &mut sub.tile_presets {
                        tile.final_probability = if sum_tile_raws * sub.sub_probability == 0.0 {
                            0.0
                        } else {
                            (tile.raw_probability / sum_tile_raws) * sub.sub_probability
                        };
                    }
                }
            } else if !coll.tile_presets.is_empty() {
                let sum_tile_raws: f32 = coll.tile_presets.iter().map(|t| t.raw_probability).sum();
                for tile in &mut coll.tile_presets {
                    tile.final_probability = if sum_tile_raws * coll.collection_probability == 0.0 {
                        0.0
                    } else {
                        (tile.raw_probability / sum_tile_raws) * coll.collection_probability
                    };
                }
            }
        }

        let mut all_tiles_flat = Vec::new();
        for coll in &collections {
            if !coll.sub_collections.is_empty() {
                for sub in &coll.sub_collections {
                    for tile in &sub.tile_presets {
                        all_tiles_flat.push(tile.clone());
                    }
                }
            } else if !coll.tile_presets.is_empty() {
                for tile in &coll.tile_presets {
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

/// Bảng tra cứu minTargetCount đã dump từ BepInEx trên Unity Game
pub fn get_quest_prefab_min_target_count(prefab_name: &str) -> usize {
    let clean = prefab_name.trim_start_matches("QuestTile_").trim_end_matches("(Clone)");
    match clean {
        "Village_6AV" => 7,
        "Village_6AV_Tower" | "Village_6AV_Fountain" => 6,
        "Village_4BV_1AF_1AF_Tower" | "Village_4BV_1AF_1AF_Fountain" 
        | "Village_4BV_1AF_1AF" | "Village_4BV_1AF_1AF_Fox"
        | "Village_5AV_1AF" | "Village_5AV_1AF_Fox" => 5,
        _ => 0,
    }
}

/// Tra cứu số lượng object nhỏ (house, tree, field, river, traintrack) được hardcode trực tiếp từ config2.txt
pub fn get_segment_element_count(group_type: GroupType, segment_type: SegmentType) -> usize {
    match group_type {
        GroupType::Village => match segment_type {
            SegmentType::ST1A => 1,
            SegmentType::ST2A => 2,
            SegmentType::ST2B | SegmentType::ST2C | SegmentType::ST3A => 3,
            SegmentType::ST3B | SegmentType::ST3C | SegmentType::ST3D => 4,
            SegmentType::ST4A | SegmentType::ST4B | SegmentType::ST4C => 5,
            SegmentType::ST5A | SegmentType::ST6A => 7,
        },
        GroupType::Forest => match segment_type {
            SegmentType::ST1A => 4,
            SegmentType::ST2A => 10,
            SegmentType::ST2B => 15,
            SegmentType::ST2C | SegmentType::ST3A => 17,
            SegmentType::ST3B | SegmentType::ST3C | SegmentType::ST3D => 20,
            SegmentType::ST4A => 21,
            SegmentType::ST4B | SegmentType::ST4C => 24,
            SegmentType::ST5A => 29,
            SegmentType::ST6A => 33,
        },
        GroupType::Agriculture => match segment_type {
            SegmentType::ST1A | SegmentType::ST2A | SegmentType::ST3A => 1,
            SegmentType::ST2B | SegmentType::ST2C | SegmentType::ST3B | SegmentType::ST3C 
            | SegmentType::ST4A | SegmentType::ST4B | SegmentType::ST5A => 2,
            SegmentType::ST3D | SegmentType::ST4C | SegmentType::ST6A => 3,
        },
        GroupType::TrainTracks | GroupType::Water => 1,
    }
}

/// Điểm nút Keyframe trên đường cong xác suất AnimationCurve dump trực tiếp từ Unity Asset
#[derive(Debug, Clone, Copy)]
pub struct Keyframe {
    pub time: f32,
    pub value: f32,
    pub in_tangent: f32,
    pub out_tangent: f32,
}

/// Đường cong xác suất mô phỏng 1:1 theo class AnimationCurve trong Unity C# (Dorfromantik2.cs)
/// Dữ liệu Keyframe được dump trực tiếp từ Unity Asset qua BepInEx ActiveQuestDumper.
/// 
/// Dữ liệu Keyframe cấu hình từ Unity Game:
/// - MoreThan Quests (field_moreThan, forest_moreThan, village_moreThan, train_moreThan, water_moreThan):
///     Keyframe #1: time=0.0000, value=1.0000 (CurveWeight=1.0)
///     Keyframe #2: time=50.0000, value=1.0000
/// - Exactly Quests (field_exactly, village_exactly, train_exactly, water_exactly):
///     Keyframe #1: time=0.0000
///     Keyframe #2: time=5.0000
///     Keyframe #3: time=20.0000
///     Keyframe #4: time=50.0000
#[derive(Debug, Clone)]
pub struct AnimationCurve {
    pub keys: Vec<Keyframe>,
}

impl AnimationCurve {
    pub fn new(keys: Vec<Keyframe>) -> Self {
        Self { keys }
    }

    pub fn linear(time_start: f32, value_start: f32, time_end: f32, value_end: f32) -> Self {
        Self {
            keys: vec![
                Keyframe { time: time_start, value: value_start, in_tangent: 0.0, out_tangent: 0.0 },
                Keyframe { time: time_end, value: value_end, in_tangent: 0.0, out_tangent: 0.0 },
            ],
        }
    }

    pub fn evaluate(&self, time: f32) -> f32 {
        if self.keys.is_empty() {
            return 0.0;
        }
        if time <= self.keys[0].time {
            return self.keys[0].value;
        }
        let last_idx = self.keys.len() - 1;
        if time >= self.keys[last_idx].time {
            return self.keys[last_idx].value;
        }
        for i in 0..last_idx {
            let k1 = &self.keys[i];
            let k2 = &self.keys[i + 1];
            if time >= k1.time && time <= k2.time {
                let range = k2.time - k1.time;
                if range == 0.0 {
                    return k1.value;
                }
                let t = (time - k1.time) / range;
                return k1.value + t * (k2.value - k1.value);
            }
        }
        self.keys[0].value
    }
}

/// Lựa chọn nhiệm vụ ngẫu nhiên theo trọng số SelectWeightedRandom (Dorfromantik2.cs dòng 24448)
pub fn select_random_quest(
    _prefab_name: &str,
    group_type: GroupType,
    quest_seed: i32,
    level: usize,
) -> (crate::tile::EqualityComparison, usize) {
    let mut rng = crate::unity_random::UnityRandom::init_state(quest_seed);

    // C# Candidate 1: Quest MoreThan (+) - AnimationCurve (Keyframe 0: 1.0, Keyframe 50: 1.0)
    let c1_equality = crate::tile::EqualityComparison::MoreThan;
    let c1_val = match group_type {
        GroupType::Forest => 5,      // ForestQuest_01_Elements_MoreThan
        GroupType::Agriculture => 3, // FieldQuest_01_Elements_MoreThan
        GroupType::Village => 3,     // VillageQuest_01_Elements_MoreThan
        GroupType::TrainTracks => 3, // TrainQuest_01_Segments_MoreThan
        GroupType::Water => 2,       // WaterQuest_01_Segments_MoreThan
    };
    let c1_curve = AnimationCurve::new(vec![
        Keyframe { time: 0.0, value: 1.0, in_tangent: 0.0, out_tangent: 0.0 },
        Keyframe { time: 50.0, value: 1.0, in_tangent: 0.0, out_tangent: 0.0 },
    ]);

    // C# Candidate 2: Quest Exactly (=) - AnimationCurve (Dumped 1:1 từ Unity Asset)
    // Keyframes: t=0: v=0.0, t=5: v=0.2, t=20: v=1.0, t=50: v=1.0
    let c2_equality = crate::tile::EqualityComparison::Exactly;
    let c2_val = match group_type {
        GroupType::Forest => 4,      // ForestQuest_02_Elements_Exactly
        GroupType::Agriculture => 3, // FieldQuest_02_Elements_Exactly
        GroupType::Village => 2,     // VillageQuest_02_Elements_Exactly
        GroupType::TrainTracks => 3, // TrainQuest_02_Segments_Exactly
        GroupType::Water => 2,       // WaterQuest_02_Segments_Exactly
    };
    let c2_curve = AnimationCurve::new(vec![
        Keyframe { time: 0.0, value: 0.0, in_tangent: 0.0, out_tangent: 0.0 },
        Keyframe { time: 5.0, value: 0.2, in_tangent: 0.0, out_tangent: 0.05333333 },
        Keyframe { time: 20.0, value: 1.0, in_tangent: 0.05333333, out_tangent: 0.0 },
        Keyframe { time: 50.0, value: 1.0, in_tangent: 0.0, out_tangent: 0.018 },
    ]);

    let level_f = level as f32;
    let w1 = c1_curve.evaluate(level_f);
    let w2 = c2_curve.evaluate(level_f);
    let total_weight = w1 + w2;

    // Trong Unity C#, SelectWeightedRandom(Dictionary) gọi Random.Range(0f, total_weight)
    let roll_val = rng.range_f32(0.0, total_weight);

    let result = if roll_val < w1 {
        (c1_equality, c1_val)
    } else {
        (c2_equality, c2_val)
    };

    // println!(
    //     "  [SelectRandomQuest] Prefab='{}' GroupType={:?} Seed={} Level={} | w1(+)={:.4} w2(=)={:.4} total={:.4} | roll={:.8} | => {:?} val={}",
    //     _prefab_name, group_type, quest_seed, level, w1, w2, total_weight, roll_val,
    //     result.0, result.1
    // );

    result
}


/// Trả về (EqualityComparison, condition.targetValue) theo chuẩn C# SelectRandomQuest (Dorfromantik2.cs dòng 24448)
pub fn get_quest_prefab_condition_target_value(prefab_name: &str, group_type: GroupType, quest_seed: i32) -> (crate::tile::EqualityComparison, usize) {
    select_random_quest(prefab_name, group_type, quest_seed, 0)
}

pub fn get_quest_prefab_condition_target_value_with_level(prefab_name: &str, group_type: GroupType, quest_seed: i32, level: usize) -> (crate::tile::EqualityComparison, usize) {
    select_random_quest(prefab_name, group_type, quest_seed, level)
}



#[derive(Debug, Clone)]
pub struct QuestSubCollection {
    pub raw_probability: f32,
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
        let mut density = 1.4;

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
                            "ACTIVE_Density" => density = num,
                            _ => {}
                        }
                    }
                }
            }
        }

        Self::new(forest_prob, agri_prob, village_prob, train_prob, water_prob, density)
    }

    /// Tính toán động collectionProbability (CollProb) tương ứng Unity C# dòng 24501
    pub fn collection_probability(&self, group_type: GroupType) -> f32 {
        let total_raw: f32 = self.collections.iter().map(|c| c.probability).sum();
        if total_raw == 0.0 {
            return 0.0;
        }
        if let Some(col) = self.collections.iter().find(|c| c.group_type == group_type) {
            col.probability / total_raw
        } else {
            0.0
        }
    }

    /// Tính toán động subCollectionProbability cho bất kỳ Game Config nào theo công thức C# dòng 24506 (UpdateValues):
    /// subCollectionProbability = (subCollectionRawProbability / TotalSubRawProb) * collectionProbability
    pub fn sub_collection_probability(&self, group_type: GroupType, sub_index: usize) -> f32 {
        let total_col_raw: f32 = self.collections.iter().map(|c| c.probability).sum();
        if total_col_raw == 0.0 {
            return 0.0;
        }
        if let Some(col) = self.collections.iter().find(|c| c.group_type == group_type) {
            let col_prob = col.probability / total_col_raw;
            let total_sub_raw: f32 = col.sub_collections.iter().map(|s| s.raw_probability).sum();
            if total_sub_raw == 0.0 || col.sub_collections.len() <= sub_index {
                return 0.0;
            }
            (col.sub_collections[sub_index].raw_probability / total_sub_raw) * col_prob
        } else {
            0.0
        }
    }

    /// Khởi tạo động cấu hình Quest và tự động tính toán mảng excluded_group_types theo C# dòng 7467-7477
    pub fn new(
        forest_prob: f32,
        agri_prob: f32,
        village_prob: f32,
        train_prob: f32,
        water_prob: f32,
        density: f32,
    ) -> Self {
        // Trong C# (dòng 7470): Nếu xác suất nhóm địa hình == 0.0 thì thêm nhóm đó vào excludedGroupTypes
        let mut excluded_group_types = Vec::new();
        if forest_prob <= 0.0 { excluded_group_types.push(GroupType::Forest); }
        if agri_prob <= 0.0 { excluded_group_types.push(GroupType::Agriculture); }
        if village_prob <= 0.0 { excluded_group_types.push(GroupType::Village); }
        if train_prob <= 0.0 { excluded_group_types.push(GroupType::TrainTracks); }
        if water_prob <= 0.0 { excluded_group_types.push(GroupType::Water); }

        let total_col_raw = forest_prob + agri_prob + village_prob + train_prob + water_prob;

        // Helper nhân density.powf(occupied_edges + 1) tương ứng C# dòng 3415:
        // questTileSubCollection.subCollectionRawProbability *= Mathf.Pow(Density, occupiedEdges + 1);
        let make_sub_raw = |base_raw: f32, occupied_edges: usize| -> f32 {
            base_raw * density.powf((occupied_edges + 1) as f32)
        };

        let forest_sub_total = make_sub_raw(10.0, 1) + make_sub_raw(1.0, 3) + make_sub_raw(10.0, 2)
            + make_sub_raw(10.0, 3) + make_sub_raw(15.0, 4) + make_sub_raw(15.0, 6);

        let agri_sub_total = make_sub_raw(10.0, 2) + make_sub_raw(7.0, 5) + make_sub_raw(5.0, 6)
            + make_sub_raw(5.0, 4) + make_sub_raw(5.0, 6) + make_sub_raw(10.0, 6) + make_sub_raw(15.0, 6);

        let village_sub_total = make_sub_raw(10.0, 2) + make_sub_raw(10.0, 6) + make_sub_raw(3.0, 6)
            + make_sub_raw(10.0, 6) + make_sub_raw(10.0, 6);

        let train_sub_total = make_sub_raw(10.0, 6) + make_sub_raw(10.0, 5) + make_sub_raw(7.0, 6)
            + make_sub_raw(20.0, 2) + make_sub_raw(5.0, 4) + make_sub_raw(0.0, 6);

        let water_sub_total = make_sub_raw(10.0, 6) + make_sub_raw(15.0, 2) + make_sub_raw(5.0, 5)
            + make_sub_raw(5.0, 5) + make_sub_raw(5.0, 5) + make_sub_raw(5.0, 6)
            + make_sub_raw(10.0, 6) + make_sub_raw(10.0, 6) + make_sub_raw(25.0, 6) + make_sub_raw(0.0, 12);

        // Helper function to build QuestSubCollection with dynamically calculated probability according to C# UpdateValues (line 24506)
        let make_sub = |base_raw: f32, total_sub_raw: f32, col_prob: f32, occupied_edges: usize, all_seg: Vec<GroupType>, tiles: Vec<QuestOption>| -> QuestSubCollection {
            let raw_prob = make_sub_raw(base_raw, occupied_edges);
            let col_share = if total_col_raw > 0.0 { col_prob / total_col_raw } else { 0.0 };
            let sub_prob = if total_sub_raw > 0.0 { (raw_prob / total_sub_raw) * col_share } else { 0.0 };
            QuestSubCollection {
                raw_probability: raw_prob,
                probability: sub_prob,
                occupied_edges,
                all_segment_types: all_seg,
                quest_tiles: tiles,
            }
        };

        let collections = vec![
            // ── Collection 1: Forest Quests (Index #0) ──
            QuestCollection {
                name: "Forest Quest Collection".into(),
                group_type: GroupType::Forest,
                probability: forest_prob,
                sub_collections: vec![
                    make_sub(10.0, forest_sub_total, forest_prob, 1, vec![GroupType::Forest], vec![
                        QuestOption { prefab_name: "QuestTile_Forest_1AF".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Forest_1AF_Deer".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Forest_1AF_Bear".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Forest_1AF_Boar".into(), probability: 0.0 },
                    ]),
                    make_sub(1.0, forest_sub_total, forest_prob, 3, vec![GroupType::Forest, GroupType::Water], vec![
                        QuestOption { prefab_name: "QuestTile_Forest_1AF_2AW".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Forest_1AF_2AW_Deer".into(), probability: 0.0 },
                    ]),
                    make_sub(10.0, forest_sub_total, forest_prob, 2, vec![GroupType::Forest], vec![
                        QuestOption { prefab_name: "QuestTile_Forest_2AF".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Forest_2AF_Deer".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Forest_2AF_Bear".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Forest_2AF_Boar".into(), probability: 0.0 },
                    ]),
                    make_sub(10.0, forest_sub_total, forest_prob, 3, vec![GroupType::Forest], vec![
                        QuestOption { prefab_name: "QuestTile_Forest_3AF".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Forest_3AF_Deer".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Forest_3AF_Bear".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Forest_3AF_Boar".into(), probability: 0.0 },
                    ]),
                    make_sub(15.0, forest_sub_total, forest_prob, 4, vec![GroupType::Forest], vec![
                        QuestOption { prefab_name: "QuestTile_Forest_4AF".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Forest_4AF_Ruin".into(), probability: 0.0 },
                    ]),
                    make_sub(15.0, forest_sub_total, forest_prob, 6, vec![GroupType::Forest], vec![
                        QuestOption { prefab_name: "QuestTile_Forest_6AF".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Forest_6AF_Deer".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Forest_6AF_Ruin".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Forest_6AF_Bear".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Forest_6AF_Boar".into(), probability: 0.0 },
                    ]),
                ],
            },

            // ── Collection 2: Agriculture Quests (Index #1) ──
            QuestCollection {
                name: "Agriculture Quest Collection".into(),
                group_type: GroupType::Agriculture,
                probability: agri_prob,
                sub_collections: vec![
                    make_sub(10.0, agri_sub_total, agri_prob, 2, vec![GroupType::Agriculture], vec![
                        QuestOption { prefab_name: "QuestTile_Agriculture_2AA".into(), probability: 1.0 },
                    ]),
                    make_sub(7.0, agri_sub_total, agri_prob, 5, vec![GroupType::Agriculture, GroupType::Village], vec![
                        QuestOption { prefab_name: "QuestTile_Agriculture_2AA_2AV_1AV".into(), probability: 1.0 },
                    ]),
                    make_sub(5.0, agri_sub_total, agri_prob, 6, vec![GroupType::Agriculture, GroupType::Forest], vec![
                        QuestOption { prefab_name: "QuestTile_Agriculture_2AA_4AF".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Agriculture_2AA_4AF_Windmill".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Agriculture_2AA_4AF_Granary".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Agriculture_2AA_4AF_BigTree".into(), probability: 0.0 },
                    ]),
                    make_sub(5.0, agri_sub_total, agri_prob, 4, vec![GroupType::Agriculture, GroupType::Village], vec![
                        QuestOption { prefab_name: "QuestTile_Agriculture_3AA_1AV".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Agriculture_3AA_1AV_Windmill".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Agriculture_3AA_1AV_Granary".into(), probability: 0.0 },
                    ]),
                    make_sub(5.0, agri_sub_total, agri_prob, 6, vec![GroupType::Agriculture, GroupType::Forest], vec![
                        QuestOption { prefab_name: "QuestTile_Agriculture_4BA_1AF_1AF".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Agriculture_4BA_1AF_1AF_BigTree".into(), probability: 0.0 },
                    ]),
                    make_sub(10.0, agri_sub_total, agri_prob, 6, vec![GroupType::Agriculture, GroupType::Forest], vec![
                        QuestOption { prefab_name: "QuestTile_Agriculture_4AA_2AF".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Agriculture_4AA_2AF_Granary".into(), probability: 0.0 },
                    ]),
                    make_sub(15.0, agri_sub_total, agri_prob, 6, vec![GroupType::Agriculture], vec![
                        QuestOption { prefab_name: "QuestTile_Agriculture_6AA".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Agriculture_6AA_Windmill".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Agriculture_6AA_BigTree".into(), probability: 0.0 },
                    ]),
                ],
            },

            // ── Collection 3: Village Quests (Index #2) ──
            QuestCollection {
                name: "Village Quest Collection".into(),
                group_type: GroupType::Village,
                probability: village_prob,
                sub_collections: vec![
                    make_sub(10.0, village_sub_total, village_prob, 2, vec![GroupType::Village], vec![
                        QuestOption { prefab_name: "QuestTile_Village_2AV".into(), probability: 1.0 },
                    ]),
                    make_sub(10.0, village_sub_total, village_prob, 6, vec![GroupType::Village, GroupType::Forest], vec![
                        QuestOption { prefab_name: "QuestTile_Village_3AV_3AF".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Village_3AV_3AF_Fountain".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Village_3AV_3AF_Tower".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Village_3AV_3AF_Fox".into(), probability: 0.0 },
                    ]),
                    make_sub(3.0, village_sub_total, village_prob, 6, vec![GroupType::Village, GroupType::Forest], vec![
                        QuestOption { prefab_name: "QuestTile_Village_4BV_1AF_1AF".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Village_4BV_1AF_1AF_Fountain".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Village_4BV_1AF_1AF_Tower".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Village_4BV_1AF_1AF_Fox".into(), probability: 0.0 },
                    ]),
                    make_sub(10.0, village_sub_total, village_prob, 6, vec![GroupType::Village, GroupType::Forest], vec![
                        QuestOption { prefab_name: "QuestTile_Village_5AV_1AF".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Village_5AV_1AF_Fox".into(), probability: 0.0 },
                    ]),
                    make_sub(10.0, village_sub_total, village_prob, 6, vec![GroupType::Village], vec![
                        QuestOption { prefab_name: "QuestTile_Village_6AV".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Village_6AV_Fountain".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Village_6AV_Tower".into(), probability: 0.0 },
                    ]),
                ],
            },

            // ── Collection 4: Train Quests (Index #3) ──
            QuestCollection {
                name: "Train Quest Collection".into(),
                group_type: GroupType::TrainTracks,
                probability: train_prob,
                sub_collections: vec![
                    make_sub(10.0, train_sub_total, train_prob, 6, vec![GroupType::TrainTracks, GroupType::Forest], vec![
                        QuestOption { prefab_name: "QuestTile_Train_2BT-3AF-1AF".into(), probability: 1.0 },
                    ]),
                    make_sub(10.0, train_sub_total, train_prob, 5, vec![GroupType::TrainTracks, GroupType::Agriculture], vec![
                        QuestOption { prefab_name: "QuestTile_Train_2BT-2AA-1AA".into(), probability: 1.0 },
                    ]),
                    make_sub(7.0, train_sub_total, train_prob, 6, vec![GroupType::TrainTracks, GroupType::Village], vec![
                        QuestOption { prefab_name: "QuestTile_Train_2BT-3AV-1AV".into(), probability: 1.0 },
                    ]),
                    make_sub(20.0, train_sub_total, train_prob, 2, vec![GroupType::TrainTracks], vec![
                        QuestOption { prefab_name: "QuestTile_Train_2CT".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Train_2CT_Locomotive".into(), probability: 1.0 },
                    ]),
                    make_sub(5.0, train_sub_total, train_prob, 4, vec![GroupType::TrainTracks, GroupType::Forest, GroupType::Village], vec![
                        QuestOption { prefab_name: "QuestTile_Train_2CT-1AF-1AV".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Train_2CT-1AF-1AV_Locomotive".into(), probability: 1.0 },
                    ]),
                    make_sub(0.0, train_sub_total, train_prob, 6, vec![GroupType::TrainTracks], vec![
                        QuestOption { prefab_name: "QuestTile_Train_4CT_1AF_1AF".into(), probability: 1.0 },
                    ]),
                ],
            },

            // ── Collection 5: Water Quests (Index #4) ──
            QuestCollection {
                name: "Water Quest Collection".into(),
                group_type: GroupType::Water,
                probability: water_prob,
                sub_collections: vec![
                    make_sub(10.0, water_sub_total, water_prob, 6, vec![GroupType::Water, GroupType::Forest], vec![
                        QuestOption { prefab_name: "QuestTile_Water_2BW_3AF_1AF".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Water_2BW_3AF_1AF_Boat".into(), probability: 1.0 },
                    ]),
                    make_sub(15.0, water_sub_total, water_prob, 2, vec![GroupType::Water], vec![
                        QuestOption { prefab_name: "QuestTile_Water_2CW".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Water_2CW_Boat".into(), probability: 1.0 },
                    ]),
                    make_sub(5.0, water_sub_total, water_prob, 5, vec![GroupType::Water, GroupType::Village], vec![
                        QuestOption { prefab_name: "QuestTile_Water_2CW_2AV_1AV".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Water_2CW_2AV_2AV_Watermill".into(), probability: 0.0 },
                    ]),
                    make_sub(5.0, water_sub_total, water_prob, 5, vec![GroupType::Water, GroupType::Forest, GroupType::Agriculture], vec![
                        QuestOption { prefab_name: "QuestTile_Water_2CW_2AF_1AA".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Water_2CW_2AF_1AA_Watermill".into(), probability: 0.0 },
                    ]),
                    make_sub(5.0, water_sub_total, water_prob, 5, vec![GroupType::Water, GroupType::Agriculture, GroupType::Village], vec![
                        QuestOption { prefab_name: "QuestTile_Water_2CW_2AA_1AV".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Water_2CW_2AA_1AV_Watermill".into(), probability: 0.0 },
                    ]),
                    make_sub(5.0, water_sub_total, water_prob, 6, vec![GroupType::Water, GroupType::Forest, GroupType::Agriculture], vec![
                        QuestOption { prefab_name: "QuestTile_Water_2CW_2AF_2AA".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Water_2CW_2AF_2AA_Beaver".into(), probability: 1.0 },
                    ]),
                    make_sub(10.0, water_sub_total, water_prob, 6, vec![GroupType::Water, GroupType::Forest], vec![
                        QuestOption { prefab_name: "QuestTile_Water_3AW_3AF".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Water_3AW_3AF_Beaver".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Water_3AW_3AF_SwanGoose".into(), probability: 0.0 },
                    ]),
                    make_sub(10.0, water_sub_total, water_prob, 6, vec![GroupType::Water, GroupType::Forest], vec![
                        QuestOption { prefab_name: "QuestTile_Water_4AW_2AF".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Water_4AW_2AF_Beaver".into(), probability: 0.0 },
                        QuestOption { prefab_name: "QuestTile_Water_4AW_2AF_SwanGoose".into(), probability: 0.0 },
                    ]),
                    make_sub(25.0, water_sub_total, water_prob, 6, vec![GroupType::Water], vec![
                        QuestOption { prefab_name: "QuestTile_Water_6AW".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Water_6AW_Boat".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Water_6AW_Beaver".into(), probability: 1.0 },
                        QuestOption { prefab_name: "QuestTile_Water_6AW_Ruin".into(), probability: 0.0 },
                    ]),
                    make_sub(0.0, water_sub_total, water_prob, 12, vec![GroupType::Water, GroupType::TrainTracks], vec![
                        QuestOption { prefab_name: "QuestTile_WaterTrainStation_6AW_6AT".into(), probability: 1.0 },
                    ]),
                ],
            },
        ];

        QuestConfigurations { collections, excluded_group_types }
    }

    pub fn default() -> Self {
        Self::from_file("monthly_game_info.txt")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monthly_game_august_2026_config() {
        // Active Session config cho Monthly Game tháng 8/2026
        // ACTIVE_VillageProbability=125
        // ACTIVE_ForestProbability=125
        // ACTIVE_AgricultureProbability=125
        // ACTIVE_WaterProbability=1000
        // ACTIVE_TrainTrackProbability=0
        // ACTIVE_Density=1.4
        let config = QuestConfigurations::new(125.0, 125.0, 125.0, 0.0, 1000.0, 1.4);

        // 1. Kiểm tra mảng excluded_group_types
        assert!(config.excluded_group_types.contains(&GroupType::TrainTracks));
        assert_eq!(config.excluded_group_types.len(), 1);

        // 2. Kiểm tra Collection Probabilities tự động
        // Total raw = 125 + 125 + 125 + 0 + 1000 = 1375
        let water_col_prob = config.collection_probability(GroupType::Water);
        let forest_col_prob = config.collection_probability(GroupType::Forest);
        let train_col_prob = config.collection_probability(GroupType::TrainTracks);

        assert!((water_col_prob - (1000.0 / 1375.0)).abs() < 1e-5);
        assert!((forest_col_prob - (125.0 / 1375.0)).abs() < 1e-5);
        assert_eq!(train_col_prob, 0.0);

        // 3. Kiểm tra SubCollection Probabilities tự động cho Water 6AW (sub_index #8)
        let water_6aw_prob = config.sub_collection_probability(GroupType::Water, 8);
        let make_sub_raw = |base_raw: f32, occupied_edges: usize| base_raw * 1.4f32.powf((occupied_edges + 1) as f32);
        let water_sub_total = make_sub_raw(10.0, 6) + make_sub_raw(15.0, 2) + make_sub_raw(5.0, 5)
            + make_sub_raw(5.0, 5) + make_sub_raw(5.0, 5) + make_sub_raw(5.0, 6)
            + make_sub_raw(10.0, 6) + make_sub_raw(10.0, 6) + make_sub_raw(25.0, 6) + make_sub_raw(0.0, 12);
        let expected_water_6aw = (make_sub_raw(25.0, 6) / water_sub_total) * (1000.0 / 1375.0);
        assert!((water_6aw_prob - expected_water_6aw).abs() < 1e-5);
    }

    #[test]
    fn test_standard_game_config() {
        // Active Session config cho Standard Game Map
        // ACTIVE_VillageProbability=50
        // ACTIVE_ForestProbability=50
        // ACTIVE_AgricultureProbability=50
        // ACTIVE_WaterProbability=38
        // ACTIVE_TrainTrackProbability=25
        // ACTIVE_Density=1.4
        let config = QuestConfigurations::new(50.0, 50.0, 50.0, 25.0, 38.0, 1.4);

        // 1. Excluded group types phải rỗng vì tất cả prob > 0
        assert!(config.excluded_group_types.is_empty());

        // 2. Collection Probabilities: Total raw = 50 + 50 + 50 + 25 + 38 = 213
        let train_col_prob = config.collection_probability(GroupType::TrainTracks);
        let water_col_prob = config.collection_probability(GroupType::Water);

        assert!((train_col_prob - (25.0 / 213.0)).abs() < 1e-5);
        assert!((water_col_prob - (38.0 / 213.0)).abs() < 1e-5);
    }
}

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
                let prob_in_percent = raw / total; // 0.238..., 0.166... etc
                GroupTypeConfiguration {
                    group_type: gt,
                    raw_probability: raw,
                    probability_in_percent: prob_in_percent,
                    display_probability: (prob_in_percent * 100.0 * 10.0).round() / 10.0, // 23.8, 16.7 etc
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
    /// Compute pct for a group from a list of GroupTypeConfiguration
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

        // ── Layer 1: compute collection-level probabilityInPercent ──
        // weighted by globalPct: collPct[g] = raw[g] * globalPct[g] / sum(raw[i] * globalPct[i])
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

        // ── Layer 2: compute segment-level probabilityInPercent ──
        // using collectionPct as weight:
        //   segPct[g] = raw[g] * collPct[g] / sum(raw[i] * collPct[i])
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

// helper function to create a TilePresetConfiguration
fn tp_c(name: &str, raw: f32, occupied: usize) -> TilePresetConfiguration {
    TilePresetConfiguration {
        name: name.into(),
        raw_probability: raw,
        final_probability: raw / TOTAL_COLLECTION_RAW,
        occupied_edges: occupied,
        segments: parse_segments(name),
    }
}

// helper function to create a TilePresetSubCollection
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
                // ── Collection 0X ──
                TilePresetCollection {
                    name: "Collection 0X".into(),
                    raw_probability: 35.1,
                    collection_probability: 35.1 / total_collection_raw,
                    tile_presets: vec![tp_c("0A", 35.1, 0)],
                    sub_collections: vec![],
                },

                // ── Collection 1X ──
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

                // ── Collection 2X ──
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

                // ── Collection 3X ──
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

                // ── Collection 4X ──
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

                // ── Collection 5X ──
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

                // ── Collection 6X ──
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub segmentPresetCollections: Vec<SegmentPresetCollection>,
}

impl SegmentPresetConfiguration {
    pub fn default() -> Self {
        let raw_collections = vec![
            (
                "Segment Collection 1X",
                vec![
                    (GroupTypeId::Agriculture, 10.0),
                    (GroupTypeId::Forest, 10.0),
                    (GroupTypeId::Village, 10.0),
                    (GroupTypeId::TrainTracks, 1.0),
                    (GroupTypeId::Water, 0.0),
                ],
                vec![SegmentType::ST1A],
            ),
            (
                "Segment Collection 2X",
                vec![
                    (GroupTypeId::Agriculture, 5.0),
                    (GroupTypeId::Forest, 5.0),
                    (GroupTypeId::Village, 5.0),
                    (GroupTypeId::TrainTracks, 15.0),
                    (GroupTypeId::Water, 15.0),
                ],
                vec![SegmentType::ST2A, SegmentType::ST2B, SegmentType::ST2C],
            ),
            (
                "Segment Collection 3X",
                vec![
                    (GroupTypeId::Agriculture, 10.0),
                    (GroupTypeId::Forest, 10.0),
                    (GroupTypeId::Village, 10.0),
                    (GroupTypeId::TrainTracks, 4.0),
                    (GroupTypeId::Water, 4.0),
                ],
                vec![SegmentType::ST3A, SegmentType::ST3B, SegmentType::ST3C, SegmentType::ST3D],
            ),
            (
                "Segment Collection 4X",
                vec![
                    (GroupTypeId::Agriculture, 10.0),
                    (GroupTypeId::Forest, 10.0),
                    (GroupTypeId::Village, 10.0),
                    (GroupTypeId::TrainTracks, 3.0),
                    (GroupTypeId::Water, 3.0),
                ],
                vec![SegmentType::ST4A, SegmentType::ST4B, SegmentType::ST4C],
            ),
            (
                "Segment Collection 5X",
                vec![
                    (GroupTypeId::Agriculture, 10.0),
                    (GroupTypeId::Forest, 10.0),
                    (GroupTypeId::Village, 10.0),
                    (GroupTypeId::TrainTracks, 2.0),
                    (GroupTypeId::Water, 6.0),
                ],
                vec![SegmentType::ST5A],
            ),
            (
                "Segment Collection 6X",
                vec![
                    (GroupTypeId::Agriculture, 10.0),
                    (GroupTypeId::Forest, 10.0),
                    (GroupTypeId::Village, 10.0),
                    (GroupTypeId::TrainTracks, 0.0),
                    (GroupTypeId::Water, 20.0),
                ],
                vec![SegmentType::ST6A],
            ),
        ];

        SegmentPresetConfiguration {
            segmentPresetCollections: raw_collections.into_iter().map(|(name, raws, presets)| {
                let total: f32 = raws.iter().map(|(_, r)| r).sum();
                SegmentPresetCollection {
                    collection_name: name.into(),
                    group_type_probabilities: raws.into_iter().map(|(gt, raw)| {
                        let pct = if total > 0.0 { raw / total } else { 0.0 };
                        GroupTypeConfig {
                            group_type: gt,
                            raw_probability: raw,
                            probability_in_percent: pct,
                            display_probability: (pct * 100.0 * 10.0).round() / 10.0,
                        }
                    }).collect(),
                    segment_presets: presets,
                }
            }).collect(),
        }
    }
}
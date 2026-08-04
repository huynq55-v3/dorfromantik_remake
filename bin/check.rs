use dorfromantik_remake::game_config;
use game_config::*;
use std::collections::HashMap;

/// Dữ liệu tham chiếu từ game gốc
static REF_TILES: &[(&str, f64, usize)] = &[
    ("0A", 0.02102199, 0),
    ("1A", 0.04204398, 1),
    ("1A_1A", 0.03503665, 2),
    ("1A_1A_1A", 0.02411839, 3),
    ("1A_1A_1A_1A", 0.008846006, 4),
    ("1A_1A_1A_1A_1A", 0.002251928, 5),
    ("1A_1A_1A_1A_1A_1A", 0.001209813, 6),
    ("2A", 0.03880984, 2),
    ("2A_1A", 0.03234153, 3),
    ("2A_1A_1A", 0.01633247, 4),
    ("2A_1A_1A_1A", 0.005989172, 5),
    ("2A_1A_1A_1A_1A", 0.001557185, 6),
    ("2A_2A", 0.0176441, 4),
    ("2A_2A_1A", 0.01105601, 5),
    ("2A_2A_1A_1A", 0.001383499, 6),
    ("2A_2A_2A", 0.005258493, 6),
    ("2B", 0.02967635, 2),
    ("2B_1A", 0.02472929, 3),
    ("2B_1A_1A", 0.01249341, 4),
    ("2B_1A_1A_1A", 0.004581717, 5),
    ("2B_1A_1A_1A_1A", 0.001191845, 6),
    ("2B_2A", 0.02075248, 4),
    ("2B_2A_1A", 0.008456711, 5),
    ("2B_2A_1A_1A", 0.003168272, 6),
    ("2C", 0.0168176, 2),
    ("2C_1A", 0.01401466, 3),
    ("2C_1A_1A", 0.007079201, 4),
    ("2C_1A_1A_1A", 0.002593311, 5),
    ("2C_1A_1A_1A_1A", 0.0006767765, 6),
    ("2C_2A", 0.01176273, 4),
    ("2C_2A_1A", 0.004791338, 5),
    ("2C_2A_1A_1A", 0.0005989172, 6),
    ("2C_2A_2A", 0.003318001, 6),
    ("3A", 0.03153299, 3),
    ("3A_1A", 0.0238908, 4),
    ("3A_1A_1A", 0.009732404, 5),
    ("3A_1A_1A_1A", 0.003647405, 6),
    ("3A_2A", 0.01617076, 5),
    ("3A_2A_1A", 0.006737818, 6),
    ("3A_2B", 0.01236764, 5),
    ("3A_2B_1A", 0.005150688, 6),
    ("3A_3A", 0.009852188, 6),
    ("3B", 0.02967634, 3),
    ("3B_1A", 0.02248335, 4),
    ("3B_1A_1A", 0.009157442, 5),
    ("3B_1A_1A_1A", 0.003437784, 6),
    ("3B_2A", 0.01521848, 5),
    ("3B_2A_1A", 0.006342532, 6),
    ("3C", 0.02967634, 3),
    ("3C_1A", 0.02248335, 4),
    ("3C_1A_1A", 0.009157442, 5),
    ("3C_1A_1A_1A", 0.003437784, 6),
    ("3C_2A", 0.01521848, 5),
    ("3C_2A_1A", 0.006342532, 6),
    ("3D", 0.01261319, 3),
    ("3D_1A", 0.009552727, 4),
    ("3D_1A_1A", 0.003892961, 5),
    ("3D_1A_1A_1A", 0.001461358, 6),
    ("4A", 0.03528221, 4),
    ("4A_1A", 0.02156102, 5),
    ("4A_1A_1A", 0.008983756, 6),
    ("4A_2A", 0.01492501, 6),
    ("4B", 0.02293254, 4),
    ("4B_1A", 0.01401466, 5),
    ("4B_1A_1A", 0.005839442, 6),
    ("4C", 0.02548392, 4),
    ("4C_1A", 0.01557184, 5),
    ("4C_1A_1A", 0.006486272, 6),
    ("5A", 0.02587322, 5),
    ("5A_1A", 0.01617076, 6),
    ("6A", 0.03603684, 6),
];

struct SegRef {
    name: &'static str,
    raws: [f32; 5],
    pcts: [f32; 5],
}

static REF_SEGMENTS: &[SegRef] = &[
    SegRef { name: "1A", raws: [10.0,10.0,10.0,10.0,0.0], pcts: [0.3278688,0.3278688,0.3278688,0.01639344,0.0] },
    SegRef { name: "2A", raws: [10.0,10.0,10.0,2.0,3.0], pcts: [0.2544529,0.2544529,0.2544529,0.07633588,0.1603053] },
    SegRef { name: "2B", raws: [10.0,10.0,10.0,15.0,15.0], pcts: [0.1190476,0.1190476,0.1190476,0.2678571,0.375] },
    SegRef { name: "2C", raws: [10.0,10.0,10.0,20.0,15.0], pcts: [0.1092896,0.1092896,0.1092896,0.3278688,0.3442623] },
    SegRef { name: "3A", raws: [10.0,10.0,10.0,5.0,35.0], pcts: [0.245098,0.245098,0.245098,0.0245098,0.2401961] },
    SegRef { name: "3B", raws: [10.0,10.0,10.0,10.0,10.0], pcts: [0.2873563,0.2873563,0.2873563,0.05747126,0.08045977] },
    SegRef { name: "3C", raws: [10.0,10.0,10.0,10.0,10.0], pcts: [0.2873563,0.2873563,0.2873563,0.05747126,0.08045977] },
    SegRef { name: "3D", raws: [3.0,3.0,2.0,20.0,10.0], pcts: [0.2027027,0.2027027,0.1351351,0.2702703,0.1891892] },
    SegRef { name: "4A", raws: [10.0,10.0,10.0,10.0,40.0], pcts: [0.2506266,0.2506266,0.2506266,0.03759399,0.2105263] },
    SegRef { name: "4B", raws: [10.0,10.0,10.0,10.0,10.0], pcts: [0.297619,0.297619,0.297619,0.04464286,0.0625] },
    SegRef { name: "4C", raws: [10.0,10.0,10.0,10.0,10.0], pcts: [0.297619,0.297619,0.297619,0.04464286,0.0625] },
    SegRef { name: "5A", raws: [10.0,10.0,10.0,10.0,20.0], pcts: [0.2538071,0.2538071,0.2538071,0.02538071,0.213198] },
    SegRef { name: "6A", raws: [10.0,10.0,10.0,10.0,10.0], pcts: [0.2272727,0.2272727,0.2272727,0.0,0.3181818] },
];

const GROUP_ORDER: [GroupType; 5] = [
    GroupType::Agriculture, GroupType::Forest, GroupType::Village,
    GroupType::TrainTracks, GroupType::Water,
];

fn seg_name(st: SegmentType) -> &'static str {
    match st {
        SegmentType::ST1A => "1A", SegmentType::ST2A => "2A", SegmentType::ST2B => "2B",
        SegmentType::ST2C => "2C", SegmentType::ST3A => "3A", SegmentType::ST3B => "3B",
        SegmentType::ST3C => "3C", SegmentType::ST3D => "3D", SegmentType::ST4A => "4A",
        SegmentType::ST4B => "4B", SegmentType::ST4C => "4C", SegmentType::ST5A => "5A",
        SegmentType::ST6A => "6A",
    }
}

fn main() {
    println!("=== KIỂM TRA DỮ LIỆU VS GAME GỐC ===\n");
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut max_prob_diff = 0.0f64;

    let tile_config = TilePresetConfigurations::default();
    let seg_config = SegmentPresetConfigurations::default();

    // ── 1. Tiles ──
    println!("─── 1. So sánh {} tiles ───", REF_TILES.len());
    let ref_probs: HashMap<&str, f64> = REF_TILES.iter().map(|&(n, p, _)| (n, p)).collect();
    let ref_edges: HashMap<&str, usize> = REF_TILES.iter().map(|&(n, _, e)| (n, e)).collect();

    for tile in &tile_config.all_tiles_flat {
        let name = tile.name.as_str();
        
        if let Some(&ref_p) = ref_probs.get(name) {
            let diff = (tile.final_probability as f64 - ref_p).abs();
            if diff > 0.0001 {
                println!("  ❌ [prob] \"{name}\": final={:.8} ref={ref_p:.8} (lệch {:.8})", tile.final_probability, diff);
                if diff > max_prob_diff { max_prob_diff = diff; }
                failed += 1;
            }
            used(&mut passed);
        } else {
            println!("  ❌ \"{name}\" không có trong reference");
            failed += 1;
        }
        
        if let Some(&ref_e) = ref_edges.get(name) {
            if tile.occupied_edges != ref_e {
                println!("  ❌ [edge] \"{name}\": edges={} ref={ref_e}", tile.occupied_edges);
                failed += 1;
            }
        }

        let expected = parse_segments(name);
        if tile.segments != expected {
            println!("  ❌ \"{name}\": segments {:?} != parse {:?}", tile.segments, expected);
            failed += 1;
        }
    }

    // ── 2. Segment presets ──
    println!("\n─── 2. So sánh {} segment presets ───", REF_SEGMENTS.len());

    for ref_seg in REF_SEGMENTS {
        let seg = seg_config.all_segment_presets.iter().find(|s| seg_name(s.segment_type) == ref_seg.name);
        if let Some(entry) = seg {
            for (gi, &gt) in GROUP_ORDER.iter().enumerate() {
                if let Some(cfg) = entry.possible_types.iter().find(|p| p.group_type == gt) {
                    let rd = (cfg.raw_probability - ref_seg.raws[gi]).abs() as f64;
                    let pd = (cfg.probability_in_percent - ref_seg.pcts[gi]).abs() as f64;
                    if rd > 0.001 {
                        println!("  ❌ [seg {}] {:?}: raw={} (ref {}) pct={:.7} (ref {})",
                            ref_seg.name, gt, cfg.raw_probability, ref_seg.raws[gi],
                            cfg.probability_in_percent, ref_seg.pcts[gi]);
                        if pd > max_prob_diff { max_prob_diff = pd; }
                        failed += 1;
                    }
                } else {
                    println!("  ❌ [seg {}] thiếu {:?}", ref_seg.name, gt);
                    failed += 1;
                }
            }
            used(&mut passed);
        } else {
            println!("  ❌ Không tìm thấy segment type {}", ref_seg.name);
            failed += 1;
        }
    }

    // ── 3. Thống kê ──
    println!("\n─── 4. Test generate_quest_tile với Seed -2093096630 ───");
    let mut gen = dorfromantik_remake::generator::TileGenerator::new(-2093096630);
    let quest_tile = gen.generate_quest_tile(-2093096630, dorfromantik_remake::generator::TileGenFilter::AtLeastTwoEmptyEdges);
    println!("-> Result Prefab: '{}'", quest_tile.quest_type);

    println!("\n─── 5. Test Chuỗi Cấu Hình Thẻ Tile (Giống Text Vàng C# Plugin) ───");
    let mut gen2 = dorfromantik_remake::generator::TileGenerator::new(-2093096630);
    for i in 1..=10 {
        let tile = gen2.generate_tile(None, 0, None);
        println!("Tile #{:02} | ID: {} | Code Chữ Vàng: '{}' | (Tile Name: {})",
            i, tile.base_tile().id, tile.tile_preset_string(), tile.base_tile().name);
    }
    let tile_total: f64 = tile_config.all_tiles_flat.iter().map(|t| t.final_probability as f64).sum();
    println!("  Tổng final_probability: {:.8} (kỳ vọng ≈ 1.0)", tile_total);
    println!();

    println!("═══ KẾT QUẢ ═══");
    println!("  Lệch probability lớn nhất: {:.8}", max_prob_diff);
    if failed == 0 {
        println!("  ✅ {} checks, 0 failed!", passed);
    } else {
        println!("  {} passed, {} failed", passed, failed);
    }
}

fn used(p: &mut u32) { *p += 1; }

use dorfromantik_remake::game_config;
use std::collections::HashSet;

fn main() {
    println!("=== KIỂM TRA TÍNH NHẤT QUÁN DỮ LIỆU ===\n");

    let mut passed = 0u32;
    let mut failed = 0u32;

    // ── 1. Global Group Type Configuration ──
    println!("─── 1. Global Group Type Configuration ───");
    let global = game_config::GlobalGroupTypeConfiguration::default_table();
    
    check(global.global_group_type_probabilities.len() == 5, &mut passed, &mut failed,
        &format!("Có đúng 5 group types (={})", global.global_group_type_probabilities.len()));

    let pct_sum: f32 = global.global_group_type_probabilities.iter()
        .map(|g| g.probability_in_percent).sum();
    let pct_diff = (pct_sum - 1.0).abs();
    check(pct_diff < 0.001, &mut passed, &mut failed,
        &format!("Tổng probability_in_percent ≈ 1.0 (={:.4}, lệch {:.6})", pct_sum, pct_diff));

    let raw_sum: f32 = global.global_group_type_probabilities.iter()
        .map(|g| g.raw_probability).sum();
    let raw_diff = (raw_sum - 42.0).abs();
    check(raw_diff < 0.01, &mut passed, &mut failed,
        &format!("Tổng raw_probability = 42 (={raw_sum}, lệch {raw_diff})"));

    // ── 2. Segment Preset Configuration ──
    println!("\n─── 2. Segment Preset Configuration ───");
    let seg_config = game_config::SegmentPresetConfiguration::default();
    
    check(seg_config.segment_preset_collections.len() == 6, &mut passed, &mut failed,
        &format!("Có đúng 6 segment collections"));

    let all_seg_types: HashSet<&game_config::SegmentType> = seg_config.segment_preset_collections.iter()
        .flat_map(|c| &c.segment_presets)
        .collect();
    check(all_seg_types.len() == 13, &mut passed, &mut failed,
        &format!("Có đủ 13 segment types (={})", all_seg_types.len()));

    for (i, coll) in seg_config.segment_preset_collections.iter().enumerate() {
        let coll_total: f32 = coll.group_type_probabilities.iter()
            .map(|g| g.probability_in_percent).sum();
        let coll_diff = (coll_total - 1.0).abs();
        check(coll_diff < 0.001, &mut passed, &mut failed,
            &format!("[{}] {}: tổng probability_in_percent ≈ 1.0 (={:.6}, lệch {:.6})", i, coll.collection_name, coll_total, coll_diff));

        check(!coll.segment_presets.is_empty(), &mut passed, &mut failed,
            &format!("[{}] {}: có segment presets", i, coll.collection_name));
    }

    // ── 3. Tile Preset Configurations ──
    println!("\n─── 3. Tile Preset Configuration ───");
    let tile_config = game_config::TilePresetConfigurations::default();
    
    check(tile_config.all_tile_presets.len() == 7, &mut passed, &mut failed,
        &format!("Có đúng 7 tile collections"));

    let total_raw: f32 = tile_config.all_tile_presets.iter().map(|c| c.raw_probability).sum();
    let raw_diff2 = (total_raw - 1669.68).abs();
    check(raw_diff2 < 0.1, &mut passed, &mut failed,
        &format!("Tổng raw_probability ≈ 1669.68 (={:.2}, lệch {:.6})", total_raw, raw_diff2));

    // ── 4. Duyệt từng tile ──
    println!("\n─── 4. Kiểm tra từng tile ───");
    let mut tile_count = 0u32;
    let mut edge_errors = 0u32;
    let mut used_segments: HashSet<game_config::SegmentType> = HashSet::new();
    let mut tile_presets_all: Vec<&game_config::TilePresetConfiguration> = Vec::new();

    for coll in &tile_config.all_tile_presets {
        for tile in &coll.tile_presets {
            tile_count += 1;
            tile_presets_all.push(tile);
            for seg in &tile.segments {
                used_segments.insert(*seg);
            }
        }
        for sub in &coll.sub_collections {
            for tile in &sub.tile_presets {
                tile_count += 1;
                tile_presets_all.push(tile);
                for seg in &tile.segments {
                    used_segments.insert(*seg);
                }
            }
        }
    }

    check(tile_count == 71, &mut passed, &mut failed,
        &format!("Tổng tiles = 71 (={tile_count})"));

    fn seg_edge_count(seg: &game_config::SegmentType) -> usize {
        match seg {
            game_config::SegmentType::ST1A => 1,
            game_config::SegmentType::ST2A | game_config::SegmentType::ST2B | game_config::SegmentType::ST2C => 2,
            game_config::SegmentType::ST3A | game_config::SegmentType::ST3B | game_config::SegmentType::ST3C | game_config::SegmentType::ST3D => 3,
            game_config::SegmentType::ST4A | game_config::SegmentType::ST4B | game_config::SegmentType::ST4C => 4,
            game_config::SegmentType::ST5A => 5,
            game_config::SegmentType::ST6A => 6,
        }
    }

    let mut max_diff = 0usize;
    for tile in &tile_presets_all {
        let expected: usize = tile.segments.iter().map(seg_edge_count).sum();
        let diff = if tile.occupied_edges > expected { tile.occupied_edges - expected } else { expected - tile.occupied_edges };
        if diff > max_diff { max_diff = diff; }
        if tile.occupied_edges != expected {
            println!("  ❌ \"{}\": occupied_edges={} nhưng segments {:?} chiếm {} cạnh (lệch {})",
                tile.name, tile.occupied_edges, tile.segments, expected, diff);
            edge_errors += 1;
        }
    }

    if edge_errors == 0 {
        check(true, &mut passed, &mut failed, &format!("Tất cả {tile_count} tiles có occupied_edges đúng"));
    } else {
        check(false, &mut passed, &mut failed, 
            &format!("{edge_errors}/{tile_count} tiles có occupied_edges sai (lệch tối đa {max_diff})"));
    }

    // ── 5. Segment types consistency ──
    println!("\n─── 5. Segment types consistency ───");
    let defined_segments: HashSet<&game_config::SegmentType> = seg_config.segment_preset_collections.iter()
        .flat_map(|c| &c.segment_presets)
        .collect();

    let mut missing = 0u32;
    for seg in &used_segments {
        if !defined_segments.contains(seg) {
            missing += 1;
            check(false, &mut passed, &mut failed,
                &format!("SegmentType {:?} dùng trong tiles nhưng không trong SegmentPresetConfiguration", seg));
        }
    }
    if missing == 0 {
        check(true, &mut passed, &mut failed, "Tất cả segment types trong tiles có trong SegmentPresetConfiguration");
    }

    println!();
    println!("  {} segment types dùng trong tiles", used_segments.len());
    println!("  {} segment types định nghĩa trong SegmentPresetConfiguration", defined_segments.len());

    // ── KẾT QUẢ ──
    println!("\n═══ KẾT QUẢ ═══");
    if failed == 0 {
        println!("  ✅ {} checks passed, 0 failed!", passed);
    } else {
        println!("  {} passed, {} failed", passed, failed);
    }
}

fn check(cond: bool, passed: &mut u32, failed: &mut u32, msg: &str) {
    if cond {
        println!("  ✅ {}", msg);
        *passed += 1;
    } else {
        println!("  ❌ {}", msg);
        *failed += 1;
    }
}

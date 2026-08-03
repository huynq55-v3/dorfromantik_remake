use dorfromantik_remake::generator::TileGenerator;
use dorfromantik_remake::tile::{BaseTile, GeneratedTile};
use std::fs;

fn main() {
    println!("=== DORFROMANTIK EXACT PREVIEW TILE GENERATOR ===");

    // 1. Đọc seed từ monthly_game_info.txt
    let info_path = "monthly_game_info.txt";
    let seed: i32 = if let Ok(content) = fs::read_to_string(info_path) {
        content
            .lines()
            .find(|line| line.starts_with("REAL_TILE_SEED="))
            .and_then(|line| line.split('=').nth(1))
            .and_then(|val| val.trim().parse::<i32>().ok())
            .unwrap_or(-2093096630)
    } else {
        -2093096630
    };

    println!("-> Seed ván chơi (TileGenerationSeed): {}", seed);
    let mut generator = TileGenerator::new(seed);
    println!("-> Bước nhảy seed (tileSeedIncrementStep tính được từ UnityInitState): {}\n", generator.tile_seed_increment_step);

    // 2. GIAI ĐOẠN 1: Setup() ban đầu -> Tạo 16 BaseTile mộc ban đầu
    let mut stack_queue: Vec<BaseTile> = Vec::new();
    for _ in 0..16 {
        let base_tile = generator.generate_base_tile(-1, "Stacked Tile");
        stack_queue.push(base_tile);
    }

    // 3. GIAI ĐOẠN 2: Regenerate() -> Xóa 3 lá mộc đầu (index 1, index 1, index 0) và nạp 3 lá mộc mới vào cuối
    let _d1 = stack_queue.remove(1);
    let n1 = generator.generate_base_tile(-1, "Stacked Tile");
    stack_queue.push(n1);

    let _d2 = stack_queue.remove(1);
    let n2 = generator.generate_base_tile(-1, "Stacked Tile");
    stack_queue.push(n2);

    let _d3 = stack_queue.remove(0);
    let n3 = generator.generate_base_tile(-1, "Stacked Tile");
    stack_queue.push(n3);

    println!("-> Đã hoàn tất quy trình Regenerate (Loại bỏ 3 lá rác ban đầu).\n");

    // 4. GIAI ĐOẠN 3: Sinh 3 ô bài THẬT đầu tiên xuất hiện trên màn hình game UI (active_quest_count = 0)
    println!("=== DUMP 3 Ô BÀI THẬT ĐẦU TIÊN CỦA GAME (UI PREVIEW TILES) ===");

    for i in 1..=3 {
        let base_tile = stack_queue.remove(0);
        let generated_tile = generator.generate_tile(Some(base_tile), 0, None);

        println!("[LÁ BÀI THẬT THỨ #{}]", i);
        match generated_tile {
            GeneratedTile::Quest { base_tile, quest_data } => {
                println!("  * Tên BaseTile   : {}", base_tile.name);
                println!("  * Seed           : {}", base_tile.seed);
                println!("  * Phân loại      : [QUEST TILE - Bài Nhiệm Vụ]");
                println!("  * Tên Quest Prefab: [{}]", quest_data.quest_type);
                println!("  * Seed Quest     : {}", quest_data.seed);
            }
            GeneratedTile::Normal { base_tile, segments } => {
                println!("  * Tên BaseTile   : {}", base_tile.name);
                println!("  * Seed           : {}", base_tile.seed);
                println!("  * Phân loại      : [NORMAL TILE - Bài Địa Hình Thường]");
                println!("  * Chi tiết địa hình ({} nhóm phân đoạn):", segments.len());
                for seg in &segments {
                    println!(
                        "     - Phân đoạn #{}: Loại = [{:?}] | Cạnh chiếm dụng = [{:?}] | Rotation = {}",
                        seg.index + 1,
                        seg.group_type,
                        seg.occupied_edges,
                        seg.rotation
                    );
                }
            }
        }
    }
    println!("\n=> Hoàn tất sinh 3 ô bài thật đầu tiên!");
}

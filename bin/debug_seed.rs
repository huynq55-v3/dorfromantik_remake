use dorfromantik_remake::env::DorfromantikEnv;
use dorfromantik_remake::tile::GeneratedTile;

fn main() {
    let seed = -2093096630;
    let mut env = DorfromantikEnv::new(seed, 10, 100);

    println!("=== FIRST 25 TILES FOR SEED {} ===", seed);
    for i in 0..25 {
        if let Some(tile) = env.current_tile().cloned() {
            match &tile {
                GeneratedTile::Normal { base_tile, .. } => {
                    println!("Tile {:02}: Normal | Name: {}", i, base_tile.name);
                }
                GeneratedTile::Quest { quest_data, base_tile } => {
                    println!(
                        "Tile {:02}: QUEST  | Group: {:?} | Target: {} | Eq: {:?} | Name: {}",
                        i,
                        quest_data.primary_group_type(),
                        quest_data.target_count,
                        quest_data.equality,
                        base_tile.name
                    );
                }
                GeneratedTile::Reward { base_tile } => {
                    println!("Tile {:02}: REWARD | Name: {}", i, base_tile.name);
                }
            }
        }
        let valid = env.get_valid_actions();
        if valid.is_empty() {
            println!("No valid actions!");
            break;
        }
        // Take first valid action
        env.step(valid[0]);
    }
}

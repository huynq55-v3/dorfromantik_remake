use macroquad::prelude::*;
use dorfromantik_remake::board::{Board, FulfillmentStatus};
use dorfromantik_remake::generator::TileGenerator;
use dorfromantik_remake::tile::{EdgeType, GeneratedTile, HexEdgeConfig};
use std::collections::VecDeque;
use std::fs;

const HEX_RADIUS: f32 = 60.0;



#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HexPos {
    pub q: i32,
    pub r: i32,
}

impl HexPos {
    pub fn new(q: i32, r: i32) -> Self {
        Self { q, r }
    }

    pub fn to_screen(&self, radius: f32) -> Vec2 {
        let x = radius * 1.5 * self.q as f32;
        let y = radius * (3.0_f32.sqrt() / 2.0 * self.q as f32 + 3.0_f32.sqrt() * self.r as f32);
        Vec2::new(x, y)
    }
}

pub fn screen_to_hex(world_pos: Vec2, radius: f32) -> HexPos {
    let q_float = (2.0 / 3.0 * world_pos.x) / radius;
    let r_float = (-1.0 / 3.0 * world_pos.x + 3.0_f32.sqrt() / 3.0 * world_pos.y) / radius;
    let s_float = -q_float - r_float;

    let mut q_round = q_float.round();
    let mut r_round = r_float.round();
    let s_round = s_float.round();

    let q_diff = (q_round - q_float).abs();
    let r_diff = (r_round - r_float).abs();
    let s_diff = (s_round - s_float).abs();

    if q_diff > r_diff && q_diff > s_diff {
        q_round = -r_round - s_round;
    } else if r_diff > s_diff {
        r_round = -q_round - s_round;
    }

    HexPos::new(q_round as i32, r_round as i32)
}

fn get_edge_color(edge_type: EdgeType) -> Color {
    match edge_type {
        EdgeType::Plain => Color::from_rgba(144, 190, 109, 255),        // Meadow Green
        EdgeType::Agriculture => Color::from_rgba(230, 194, 41, 255),  // Wheat Gold
        EdgeType::Forest => Color::from_rgba(45, 106, 79, 255),         // Deep Forest Green
        EdgeType::Village => Color::from_rgba(217, 4, 41, 255),         // Roof Terracotta
        EdgeType::Water => Color::from_rgba(0, 119, 182, 255),          // Sapphire Blue
        EdgeType::TrainTracks => Color::from_rgba(74, 78, 105, 255),    // Railway Steel
    }
}

fn load_game_seed() -> i32 {
    if let Ok(content) = fs::read_to_string("monthly_game_info.txt") {
        for line in content.lines() {
            if line.starts_with("REAL_TILE_SEED=") {
                if let Ok(seed) = line.trim_start_matches("REAL_TILE_SEED=").parse::<i32>() {
                    return seed;
                }
            }
        }
    }
    -2093096630
}

fn init_active_quest_tile_target(front_tile: &mut GeneratedTile, game_board: &Board) {
    if let GeneratedTile::Quest { quest_data, .. } = front_tile {
        if quest_data.target_count == 0 {
            let gt = quest_data.primary_group_type();
            let board_ref = game_board.reference_group_count(gt);
            let min_target = dorfromantik_remake::game_config::get_quest_prefab_min_target_count(&quest_data.quest_type);
            let (eq, cond_val) = dorfromantik_remake::game_config::get_quest_prefab_condition_target_value(&quest_data.quest_type, gt, quest_data.seed);
            let mut qm = dorfromantik_remake::quest_manager::QuestManager::new();
            let fulfilled_count = game_board.placed_tiles.values().filter(|pt| pt.quest_status == Some(dorfromantik_remake::board::FulfillmentStatus::Success)).count();
            qm.level = fulfilled_count;
            quest_data.level = fulfilled_count;
            let ref_base = std::cmp::max(board_ref, 1);
            let base = std::cmp::max(ref_base, min_target);
            let diff = qm.difficulty_increase(gt);

            quest_data.target_count = base + cond_val + diff;
            quest_data.equality = eq;

            let own_elements = quest_data.own_elements();
            let remaining_display = quest_data.remaining_display_value();

            println!("================================----------------------------------");
            println!("[ACTIVE TILE INIT - QUEST TARGET CALCULATION]");
            println!("  - Prefab Name: '{}'", quest_data.quest_type);
            println!("  - Quest Seed: {}", quest_data.seed);
            println!("  - Primary GroupType: {:?}", gt);
            println!("  - Equality: {:?}", eq);
            println!("  - minTargetCount: {}", min_target);
            println!("  - conditionTargetValue: {}", cond_val);
            println!("  - ReferenceGroupCount (On Board): {}", board_ref);
            println!("  - DifficultyIncrease: {}", diff);
            println!("  ==> INTERNAL TARGET VALUE (TargetValue): {}", quest_data.target_count);
            println!("  ==> OWN TILE ELEMENTS (Số Object trên tile): {}", own_elements);
            println!("  ==> REMAINING DISPLAY VALUE (Số hiển thị trên bóng bóng): {}", remaining_display);
            println!("================================----------------------------------\n");
        }
    }
}

#[macroquad::main("Dorfromantik Simulator")]
async fn main() {
    let seed = load_game_seed();
    let mut generator = TileGenerator::new(seed);
    let mut game_board = Board::new();

    // Tile Queue buffer maintains 4 tiles ahead
    let mut tile_queue: VecDeque<GeneratedTile> = VecDeque::new();
    let mut current_rotation: usize = 0;
    let mut placed_count: usize = 0;
    let mut score: usize = 0;

    // 1. Place initial starting tile at center (0, 0)
    let initial_tile = GeneratedTile::Normal {
        base_tile: dorfromantik_remake::tile::BaseTile::new(0, seed, "Initial Center Plain Tile"),
        segments: Vec::new(),
    };
    game_board.place_tile(0, 0, initial_tile, 0);

    // 2. Pre-generate top 3 tiles (Tile 1, Tile 2, Tile 3) for the queue buffer at startup
    for _ in 0..3 {
        let t = generator.generate_tile(None, 0, None);
        tile_queue.push_back(t);
    }

    // Camera variables
    let mut camera_pos = Vec2::ZERO;
    let mut zoom: f32 = 1.0;
    let mut last_mouse_pos = mouse_position();

    loop {
        clear_background(Color::from_rgba(20, 24, 33, 255));

        let delta = get_frame_time();
        let (screen_w, screen_h) = (screen_width(), screen_height());

        // Active tile in hand is the first tile in queue
        let active_tile_opt = tile_queue.front().cloned();

        // ── 1. Controls ──
        let current_mouse_pos = mouse_position();
        if is_mouse_button_down(MouseButton::Right) {
            let dx = current_mouse_pos.0 - last_mouse_pos.0;
            let dy = current_mouse_pos.1 - last_mouse_pos.1;
            camera_pos.x -= dx / zoom;
            camera_pos.y -= dy / zoom;
        }
        last_mouse_pos = current_mouse_pos;

        let speed = 400.0 / zoom;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) { camera_pos.y -= speed * delta; }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) { camera_pos.y += speed * delta; }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) { camera_pos.x -= speed * delta; }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) { camera_pos.x += speed * delta; }

        if is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl) {
            let wheel = mouse_wheel().1;
            if wheel > 0.0 { zoom *= 1.1; }
            if wheel < 0.0 { zoom *= 0.9; }
        }
        if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::Key1) { zoom *= 1.1; }
        if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::Key2) { zoom *= 0.9; }
        zoom = zoom.clamp(0.4, 3.0);

        // World coordinates & Hovered Hex
        let mouse_vec = Vec2::new(current_mouse_pos.0, current_mouse_pos.1);
        let center_vec = Vec2::new(screen_w * 0.5, screen_h * 0.5);
        let mouse_world = (mouse_vec - center_vec) / zoom + camera_pos;
        let hovered_hex = screen_to_hex(mouse_world, HEX_RADIUS);

        // Smart Rotation: Skip invalid rotations when rotating tile
        let wheel = mouse_wheel().1;
        if !(is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl)) {
            if let Some(ref active_tile) = active_tile_opt {
                if wheel > 0.0 {
                    current_rotation = game_board.get_next_valid_rotation(hovered_hex.q, hovered_hex.r, active_tile, current_rotation, true);
                } else if wheel < 0.0 {
                    current_rotation = game_board.get_next_valid_rotation(hovered_hex.q, hovered_hex.r, active_tile, current_rotation, false);
                }
            }
        }

        if is_mouse_button_pressed(MouseButton::Middle) || is_key_pressed(KeyCode::R) || is_key_pressed(KeyCode::E) || is_key_pressed(KeyCode::Space) {
            if let Some(ref active_tile) = active_tile_opt {
                current_rotation = game_board.get_next_valid_rotation(hovered_hex.q, hovered_hex.r, active_tile, current_rotation, true);
            }
        }
        if is_key_pressed(KeyCode::Q) {
            if let Some(ref active_tile) = active_tile_opt {
                current_rotation = game_board.get_next_valid_rotation(hovered_hex.q, hovered_hex.r, active_tile, current_rotation, false);
            }
        }



        // ── 1.5. Render Grid Slots for All Possible Placement Locations ──
        if let Some(ref active_tile) = active_tile_opt {
            let available_slots = game_board.get_available_placement_slots(active_tile);
            for ((sq, sr), is_slot_valid) in available_slots {
                let slot_pos = HexPos::new(sq, sr);
                if slot_pos == hovered_hex {
                    continue; // Skip hovered hex (drawn separately in hover preview section)
                }

                let screen_pos = (slot_pos.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;

                if is_slot_valid {
                    // Valid slot: Subtle Cyan dashed outline
                    draw_hex_lines(screen_pos, HEX_RADIUS * zoom, 1.8 * zoom, Color::from_rgba(80, 220, 240, 90));
                } else {
                    // Invalid slot (cannot place due to Water/Train placement restrictions): Subtle Red dashed outline
                    draw_hex_lines(screen_pos, HEX_RADIUS * zoom, 1.8 * zoom, Color::from_rgba(255, 70, 70, 100));
                }
            }
        }

        // Auto-Snap Hover Orientation: Automatically snap current_rotation to a valid orientation when hovering any slot
        if let Some(ref active_tile) = active_tile_opt {
            if !game_board.can_place_tile(hovered_hex.q, hovered_hex.r, active_tile, current_rotation) {
                if let Some(valid_rot) = (0..6).find(|&rot| game_board.can_place_tile(hovered_hex.q, hovered_hex.r, active_tile, rot)) {
                    current_rotation = valid_rot;
                }
            }
        }

        // Validate placement against Board rules (Water MUST connect Water, Train MUST connect Train)
        let can_place = if let Some(ref active_tile) = active_tile_opt {
            game_board.can_place_tile(hovered_hex.q, hovered_hex.r, active_tile, current_rotation)
        } else {
            false
        };

        // Compute live hover preview quest updates when mouse hovers over a valid cell
        let preview_map = if can_place {
            if let Some(ref active_tile) = active_tile_opt {
                game_board.preview_quest_counts(hovered_hex.q, hovered_hex.r, active_tile, current_rotation)
            } else {
                std::collections::HashMap::new()
            }
        } else {
            std::collections::HashMap::new()
        };

        // Active Quest Count Calculation according to exact user formula:
        let active_on_board = game_board.active_quest_count();
        let current_is_quest = active_tile_opt.as_ref().map_or(false, |t| matches!(t, GeneratedTile::Quest { .. }));
        let effective_active_quests = if current_is_quest {
            active_on_board + 2
        } else {
            active_on_board
        };

        // Click LMB to Place Tile
        if can_place && is_mouse_button_pressed(MouseButton::Left) {
            if let Some(active_tile) = active_tile_opt.as_ref() {
                // Store active quest count calculated for CURRENT tile (Tile #1) to generate Tile #4 (current + 3 ahead)
                let active_quest_count_for_tile4 = effective_active_quests;

                if game_board.place_tile(hovered_hex.q, hovered_hex.r, active_tile.clone(), current_rotation) {
                    placed_count += 1;
                    score += 10;

                    // Pop placed tile (Tile 1) from queue
                    tile_queue.pop_front();
                    current_rotation = 0;

                    // Generate Tile #4 using active_quest_count_for_tile4 (calculated from Tile 1)
                    let next_gen = generator.generate_tile(None, active_quest_count_for_tile4, None);
                    tile_queue.push_back(next_gen);

                    // Update TargetValue for the new front tile (Tile #2) based on current board state
                    if let Some(front_tile) = tile_queue.front_mut() {
                        init_active_quest_tile_target(front_tile, &game_board);
                    }
                }
            }
        }

        // Active tile initialization at start of frame if uninitialized
        if let Some(front_tile) = tile_queue.front_mut() {
            init_active_quest_tile_target(front_tile, &game_board);
        }

        // ── 2. Render Placed Tiles on Board ──
        for (&(q, r), placed_tile) in &game_board.placed_tiles {
            let hex_pos = HexPos::new(q, r);
            let screen_pos = (hex_pos.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;

            draw_hex_tile(screen_pos, HEX_RADIUS * zoom, &placed_tile.edge_config, 0, 1.0);

            // Draw Quest Status Badge with LIVE HOVER PREVIEW updates!
            draw_board_quest_badge(&game_board, screen_pos, placed_tile, HEX_RADIUS * zoom, preview_map.get(&(q, r)));
        }

        // ── 3. Render Placement Preview under Cursor ──
        if can_place {
            if let Some(ref active_tile) = active_tile_opt {
                let mut preview_cfg = active_tile.to_hex_edge_config();
                preview_cfg.rotate(current_rotation);

                let screen_pos = (hovered_hex.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;

                // Draw semi-transparent preview hex
                draw_hex_tile(screen_pos, HEX_RADIUS * zoom, &preview_cfg, 0, 0.65);

                // Draw dashed outline
                draw_hex_lines(screen_pos, HEX_RADIUS * zoom, 3.0 * zoom, Color::from_rgba(255, 220, 100, 230));

                // Draw Quest Target Count Badge in Preview (with live preview reduction!)
                if let GeneratedTile::Quest { quest_data, .. } = active_tile {
                    let display_target = if let Some(&(rem_target, _)) = preview_map.get(&(hovered_hex.q, hovered_hex.r)) {
                        rem_target
                    } else {
                        quest_data.remaining_display_value()
                    };
                    draw_custom_badge_text(screen_pos, quest_data.primary_group_type(), quest_data.equality, display_target, HEX_RADIUS * zoom, 0.85);
                }
            }
        } else if !game_board.placed_tiles.contains_key(&(hovered_hex.q, hovered_hex.r)) {
            // Draw invalid placement indicator if cursor is on an empty cell
            let screen_pos = (hovered_hex.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;
            draw_hex_lines(screen_pos, HEX_RADIUS * zoom, 1.5 * zoom, Color::from_rgba(255, 60, 60, 120));
        }

        // ── 4. UI Overlay (Score, Active Quest Counts & Queue Preview) ──
        draw_rectangle(15.0, 15.0, 360.0, 160.0, Color::from_rgba(10, 12, 18, 220));
        draw_rectangle_lines(15.0, 15.0, 360.0, 160.0, 2.0, SKYBLUE);

        draw_text("DORFROMANTIK SIMULATOR", 28.0, 38.0, 20.0, SKYBLUE);
        draw_text(&format!("Tiles Placed: {}  |  Score: {} pts", placed_count, score), 28.0, 65.0, 16.0, WHITE);
        draw_text(&format!("Active Quests on Board: {}", active_on_board), 28.0, 88.0, 16.0, GOLD);
        draw_text(&format!("Gen Active Count (for Tile #4): {}", effective_active_quests), 28.0, 111.0, 16.0, GREEN);
        draw_text("LMB: Place | RMB Drag: Pan | Wheel: Rotate", 28.0, 134.0, 13.0, LIGHTGRAY);
        draw_text("Ctrl + Wheel: Zoom", 28.0, 150.0, 13.0, LIGHTGRAY);

        // ── Top 3 Preview Queue Panel (Right Side UI) ──
        let panel_w = 170.0;
        let panel_h = 440.0;
        let panel_x = screen_w - panel_w - 20.0;
        let panel_y = 20.0;

        draw_rectangle(panel_x, panel_y, panel_w, panel_h, Color::from_rgba(12, 16, 24, 230));
        draw_rectangle_lines(panel_x, panel_y, panel_w, panel_h, 2.0, SKYBLUE);

        draw_text("TILE QUEUE", panel_x + 35.0, panel_y + 30.0, 18.0, SKYBLUE);

        // Render Top 3 Preview Tiles from the 4-Tile Buffer Queue
        let slot_offsets_y = [110.0, 235.0, 355.0];
        let slot_radii = [42.0, 34.0, 28.0];
        let slot_labels = ["ACTIVE TILE", "NEXT #1", "NEXT #2"];

        for (idx, tile) in tile_queue.iter().take(3).enumerate() {
            let slot_y = panel_y + slot_offsets_y[idx];
            let slot_center = Vec2::new(panel_x + panel_w * 0.5, slot_y);
            let rot = if idx == 0 { current_rotation } else { 0 };
            let cfg = tile.to_hex_edge_config();

            let label_color = if idx == 0 { SKYBLUE } else { LIGHTGRAY };
            let label_size = if idx == 0 { 15.0 } else { 13.0 };
            let label_str = slot_labels[idx];
            let text_w = measure_text(label_str, None, label_size as u16, 1.0).width;
            draw_text(label_str, slot_center.x - text_w * 0.5, slot_y - slot_radii[idx] - 10.0, label_size, label_color);

            let tile_alpha = if idx == 0 { 1.0 } else { 0.85 };
            draw_hex_tile(slot_center, slot_radii[idx], &cfg, rot, tile_alpha);

            if idx == 0 {
                if let GeneratedTile::Quest { quest_data, .. } = tile {
                    draw_badge_text(slot_center, quest_data, slot_radii[idx], tile_alpha);
                }
            }

            let p_code = tile.tile_preset_string();
            let code_size = if idx == 0 { 14.0 } else { 12.0 };
            let code_w = measure_text(&p_code, None, code_size as u16, 1.0).width;
            draw_text(&p_code, slot_center.x - code_w * 0.5, slot_y + slot_radii[idx] + 16.0, code_size, WHITE);
        }

        next_frame().await;
    }
}

/// Helper function to draw a flat-topped Hexagon tile
fn draw_hex_tile(center: Vec2, radius: f32, config: &HexEdgeConfig, rotation: usize, alpha: f32) {
    let mut points = [Vec2::ZERO; 6];
    let angles: [f32; 6] = [
        0.0,
        std::f32::consts::FRAC_PI_3,
        2.0 * std::f32::consts::FRAC_PI_3,
        std::f32::consts::PI,
        4.0 * std::f32::consts::FRAC_PI_3,
        5.0 * std::f32::consts::FRAC_PI_3,
    ];

    for i in 0..6 {
        points[i] = Vec2::new(
            center.x + radius * angles[i].cos(),
            center.y + radius * angles[i].sin(),
        );
    }

    // Draw base background hex fill (Meadow Green for plain tiles)
    let mut fill_color = Color::from_rgba(144, 190, 109, 255);
    fill_color.a *= alpha;
    for i in 0..6 {
        let p1 = points[i];
        let p2 = points[(i + 1) % 6];
        draw_triangle(center, p1, p2, fill_color);
    }

    // Draw 6 colored edge sectors
    for i in 0..6 {
        let edge_type = config.edge_at(i, rotation);
        if edge_type != EdgeType::Plain {
            let p1 = points[i];
            let p2 = points[(i + 1) % 6];
            let mut edge_color = get_edge_color(edge_type);
            edge_color.a *= alpha;
            let mid_p1 = center + (p1 - center) * 0.90;
            let mid_p2 = center + (p2 - center) * 0.90;

            draw_triangle(p1, p2, mid_p2, edge_color);
            draw_triangle(p1, mid_p2, mid_p1, edge_color);
        }
    }

    // Draw dark border lines
    let mut border_color = Color::from_rgba(15, 18, 26, 255);
    border_color.a *= alpha;
    for i in 0..6 {
        let p1 = points[i];
        let p2 = points[(i + 1) % 6];
        draw_line(p1.x, p1.y, p2.x, p2.y, 2.5 * (radius / 60.0), border_color);
    }
}

/// Helper function to draw hex outline
fn draw_hex_lines(center: Vec2, radius: f32, thickness: f32, color: Color) {
    let angles: [f32; 6] = [
        0.0,
        std::f32::consts::FRAC_PI_3,
        2.0 * std::f32::consts::FRAC_PI_3,
        std::f32::consts::PI,
        4.0 * std::f32::consts::FRAC_PI_3,
        5.0 * std::f32::consts::FRAC_PI_3,
    ];

    for i in 0..6 {
        let p1 = Vec2::new(center.x + radius * angles[i].cos(), center.y + radius * angles[i].sin());
        let p2 = Vec2::new(center.x + radius * angles[(i + 1) % 6].cos(), center.y + radius * angles[(i + 1) % 6].sin());
        draw_line(p1.x, p1.y, p2.x, p2.y, thickness, color);
    }
}

/// Draw Quest Badge on Placed Tiles with Fulfillment Status (Success / Failed / Incomplete) & Live Hover Preview
fn draw_board_quest_badge(
    game_board: &dorfromantik_remake::board::Board,
    center: Vec2,
    placed_tile: &dorfromantik_remake::board::PlacedTile,
    hex_radius: f32,
    preview: Option<&(usize, FulfillmentStatus)>,
) {
    if let GeneratedTile::Quest { quest_data, .. } = &placed_tile.tile {
        let badge_radius = hex_radius * 0.38;

        // Check if there is an active hover preview override
        let effective_status = preview.map(|p| p.1).or(placed_tile.quest_status);

        match effective_status {
            Some(FulfillmentStatus::Success) => {
                // Success: Green circle badge with white OK checkmark
                draw_circle(center.x, center.y, badge_radius, Color::from_rgba(40, 180, 80, 240));
                draw_circle_lines(center.x, center.y, badge_radius, 2.5, WHITE);
                let text_w = measure_text("OK", None, (badge_radius * 1.0) as u16, 1.0).width;
                draw_text("OK", center.x - text_w * 0.5, center.y + badge_radius * 0.35, badge_radius * 1.0, WHITE);
            }
            Some(FulfillmentStatus::Failed) => {
                // Failed: Dark Red circle badge with FAIL X mark
                draw_circle(center.x, center.y, badge_radius, Color::from_rgba(180, 40, 40, 240));
                draw_circle_lines(center.x, center.y, badge_radius, 2.5, WHITE);
                let text_w = measure_text("X", None, (badge_radius * 1.1) as u16, 1.0).width;
                draw_text("X", center.x - text_w * 0.5, center.y + badge_radius * 0.35, badge_radius * 1.1, WHITE);
            }
            _ => {
                // Incomplete: Draw target count badge (or preview remaining target count if hovering)
                let display_target = if let Some(&(rem_target, _)) = preview {
                    rem_target
                } else {
                    game_board.get_quest_remaining_target((placed_tile.q, placed_tile.r))
                };

                draw_custom_badge_text(center, quest_data.primary_group_type(), quest_data.equality, display_target, hex_radius, 1.0);
            }
        }
    }
}

/// Helper function to draw badge text (+3, =2) with custom target count
fn draw_custom_badge_text(
    center: Vec2,
    primary_gt: dorfromantik_remake::game_config::GroupType,
    equality: dorfromantik_remake::tile::EqualityComparison,
    target_count: usize,
    hex_radius: f32,
    alpha: f32,
) {
    let mut badge_color = get_edge_color(primary_gt.into());
    badge_color.a *= alpha;

    let badge_radius = hex_radius * 0.35;
    draw_circle(center.x, center.y, badge_radius, badge_color);

    let mut border_color = WHITE;
    border_color.a *= alpha;
    draw_circle_lines(center.x, center.y, badge_radius, 2.0 * (hex_radius / 60.0), border_color);

    let text_str = match equality {
        dorfromantik_remake::tile::EqualityComparison::MoreThan => format!("+{}", target_count),
        dorfromantik_remake::tile::EqualityComparison::Exactly => format!("={}", target_count),
    };
    let font_size = badge_radius * 1.05;
    let text_w = measure_text(&text_str, None, font_size as u16, 1.0).width;

    let mut text_color = WHITE;
    text_color.a *= alpha;
    draw_text(
        &text_str,
        center.x - text_w * 0.5,
        center.y + font_size * 0.35,
        font_size,
        text_color,
    );
}

/// Helper function to draw badge text (+3, =2)
fn draw_badge_text(center: Vec2, quest_data: &dorfromantik_remake::tile::QuestTileData, hex_radius: f32, alpha: f32) {
    draw_custom_badge_text(center, quest_data.primary_group_type(), quest_data.equality, quest_data.remaining_display_value(), hex_radius, alpha);
}

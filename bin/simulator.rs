use macroquad::prelude::*;
use dorfromantik_remake::board::{Board, FulfillmentStatus};
use dorfromantik_remake::generator::TileGenerator;
use dorfromantik_remake::quest_manager::QuestManager;
use dorfromantik_remake::score_manager::ScoreManager;
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
        let tile_size_x = radius * 2.0;
        let tile_size_y = radius * 3.0_f32.sqrt();
        let x = self.q as f32 * tile_size_x * 0.75;
        let y = -((self.r as f32) - (self.q.abs() % 2) as f32 / 2.0) * tile_size_y;
        Vec2::new(x, y)
    }
}

pub fn screen_to_hex(world_pos: Vec2, radius: f32) -> HexPos {
    let tile_size_x = radius * 2.0;
    let tile_size_y = radius * 3.0_f32.sqrt();
    let q = (world_pos.x / (tile_size_x * 0.75)).round() as i32;
    let r = ((-world_pos.y / tile_size_y) + (q.abs() % 2) as f32 / 2.0).round() as i32;
    HexPos::new(q, r)
}

fn get_edge_color(edge_type: EdgeType) -> Color {
    match edge_type {
        EdgeType::Plain => Color::from_rgba(144, 190, 109, 255),        // Meadow Green
        EdgeType::Agriculture => Color::from_rgba(230, 194, 41, 255),  // Wheat Gold
        EdgeType::Forest => Color::from_rgba(45, 106, 79, 255),         // Deep Forest Green
        EdgeType::Village => Color::from_rgba(224, 86, 60, 255),        // Roof Terracotta Warm Red-Orange
        EdgeType::Water         => Color::from_rgba(0,  119, 182, 255), // Sapphire Blue (nước cứng)
        EdgeType::FlexibleWater => Color::from_rgba(0,  200, 190, 255), // Teal Cyan (nước mềm / lake)
        EdgeType::TrainTracks => Color::from_rgba(74, 78, 105, 255),    // Railway Steel
        EdgeType::WaterTrainStation => Color::from_rgba(114, 9, 183, 255), // Purple Tower
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

fn load_tile_stack_height() -> usize {
    if let Ok(content) = fs::read_to_string("monthly_game_info.txt") {
        for line in content.lines() {
            if line.starts_with("ACTIVE_TileStackHeight=") {
                if let Ok(height) = line.trim_start_matches("ACTIVE_TileStackHeight=").parse::<usize>() {
                    return height;
                }
            }
        }
    }
    10
}

fn init_active_quest_tile_target(front_tile: &mut GeneratedTile, game_board: &Board, quest_manager: &mut QuestManager) {
    dorfromantik_remake::quest_manager::initialize_active_quest_tile(front_tile, game_board, quest_manager);
}

/// Floating Toast Notification Popup (Hiển thị điểm số bay lên màn hình khi hoàn thành Fit / Perfect / Quest)
struct FloatingToast {
    text: String,
    pos: Vec2,
    color: Color,
    timer: f32,
}

#[macroquad::main("Dorfromantik Simulator")]
async fn main() {
    let seed = load_game_seed();
    let initial_stack = load_tile_stack_height();

    let mut generator = TileGenerator::new(seed);
    let mut quest_manager = QuestManager::new();
    let mut game_board = Board::new();
    let mut score_manager = ScoreManager::new(initial_stack);

    let mut floating_toasts: Vec<FloatingToast> = Vec::new();

    // Tile Queue buffer maintains 4 tiles ahead
    let mut tile_queue: VecDeque<GeneratedTile> = VecDeque::new();
    let mut current_rotation: usize = 0;

    // 1. Place initial starting tile at center (0, 0)
    let initial_tile = GeneratedTile::Normal {
        base_tile: dorfromantik_remake::tile::BaseTile::new(0, seed, "Initial Center Plain Tile"),
        segments: Vec::new(),
    };
    game_board.place_tile(0, 0, initial_tile, 0);

    // 2. Sinh 3 tile preview ban đầu (Tile #1, #2, #3) — pop queue để consume đúng 3 slot count = 0
    for _ in 0..3 {
        let active_count = quest_manager.pop_next_active_quest_count(); // pops 0, 0, 0
        let t = generator.generate_tile(None, active_count, None, quest_manager.level);
        tile_queue.push_back(t);
    }

    // Kích hoạt/Đăng ký QuestWatcher CHỈ cho ô đầu cọc bài (Active Tile ở vị trí topStackPreview)
    if let Some(front_tile) = tile_queue.front_mut() {
        if let GeneratedTile::Quest { ref mut quest_data, .. } = front_tile {
            let qid = quest_manager.add_quest(&quest_data.quest_type);
            quest_data.quest_id = Some(qid);
        }
    }

    // Camera variables
    let mut camera_pos = Vec2::ZERO;
    let mut zoom: f32 = 1.0;
    let mut last_mouse_pos = mouse_position();
    let mut total_drag_dist: f32 = 0.0;

    loop {
        clear_background(Color::from_rgba(20, 24, 33, 255));

        let delta = get_frame_time();
        let (screen_w, screen_h) = (screen_width(), screen_height());

        // Update floating toast notifications
        floating_toasts.retain_mut(|t| {
            t.timer -= delta;
            t.pos.y -= 30.0 * delta;
            t.timer > 0.0
        });

        // Active tile in hand is the first tile in queue
        let active_tile_opt = tile_queue.front().cloned();

        // ── 1. Controls & Mouse Dragging ──
        let current_mouse_pos = mouse_position();
        let mouse_delta = Vec2::new(current_mouse_pos.0 - last_mouse_pos.0, current_mouse_pos.1 - last_mouse_pos.1);

        if is_mouse_button_pressed(MouseButton::Left) || is_mouse_button_pressed(MouseButton::Right) || is_mouse_button_pressed(MouseButton::Middle) {
            total_drag_dist = 0.0;
        }

        if is_mouse_button_down(MouseButton::Left) || is_mouse_button_down(MouseButton::Right) || is_mouse_button_down(MouseButton::Middle) {
            total_drag_dist += mouse_delta.length();
            if total_drag_dist > 4.0 {
                camera_pos.x -= mouse_delta.x / zoom;
                camera_pos.y -= mouse_delta.y / zoom;
            }
        }
        last_mouse_pos = current_mouse_pos;

        let speed = 500.0 / zoom;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) { camera_pos.y -= speed * delta; }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) { camera_pos.y += speed * delta; }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) { camera_pos.x -= speed * delta; }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) { camera_pos.x += speed * delta; }

        // Mouse Wheel: Scroll to rotate tile, Ctrl + Scroll to zoom camera
        let wheel = mouse_wheel().1;
        if is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl) {
            if wheel > 0.0 { zoom *= 1.12; }
            if wheel < 0.0 { zoom *= 0.88; }
        } else if wheel != 0.0 && !score_manager.is_game_over {
            if let Some(ref active_tile) = active_tile_opt {
                let mouse_vec_early = Vec2::new(current_mouse_pos.0, current_mouse_pos.1);
                let center_vec_early = Vec2::new(screen_w * 0.5, screen_h * 0.5);
                let mouse_world_early = (mouse_vec_early - center_vec_early) / zoom + camera_pos;
                let hex_early = screen_to_hex(mouse_world_early, HEX_RADIUS);
                let forward = wheel < 0.0;
                current_rotation = game_board.get_next_valid_rotation(hex_early.q, hex_early.r, active_tile, current_rotation, forward);
            } else {
                if wheel < 0.0 { current_rotation = (current_rotation + 1) % 6; }
                else           { current_rotation = (current_rotation + 5) % 6; }
            }
        }

        if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::Key1) { zoom *= 1.15; }
        if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::Key2) { zoom *= 0.85; }
        if is_key_pressed(KeyCode::C) || is_key_pressed(KeyCode::Home) {
            camera_pos = Vec2::ZERO;
            zoom = 1.0;
        }
        zoom = zoom.clamp(0.2, 4.0);

        // World coordinates & Hovered Hex
        let mouse_vec = Vec2::new(current_mouse_pos.0, current_mouse_pos.1);
        let center_vec = Vec2::new(screen_w * 0.5, screen_h * 0.5);
        let mouse_world = (mouse_vec - center_vec) / zoom + camera_pos;
        let hovered_hex = screen_to_hex(mouse_world, HEX_RADIUS);

        // Rotate tile controls
        let rmb_clicked = is_mouse_button_released(MouseButton::Right) && total_drag_dist <= 6.0;
        if (!score_manager.is_game_over) && (rmb_clicked || is_mouse_button_pressed(MouseButton::Middle) || is_key_pressed(KeyCode::R) || is_key_pressed(KeyCode::E) || is_key_pressed(KeyCode::Space)) {
            if let Some(ref active_tile) = active_tile_opt {
                current_rotation = game_board.get_next_valid_rotation(hovered_hex.q, hovered_hex.r, active_tile, current_rotation, true);
            } else {
                current_rotation = (current_rotation + 1) % 6;
            }
        }
        if (!score_manager.is_game_over) && is_key_pressed(KeyCode::Q) {
            if let Some(ref active_tile) = active_tile_opt {
                current_rotation = game_board.get_next_valid_rotation(hovered_hex.q, hovered_hex.r, active_tile, current_rotation, false);
            } else {
                current_rotation = (current_rotation + 5) % 6;
            }
        }

        // Auto-snap rotation
        if !score_manager.is_game_over {
            if let Some(ref active_tile) = active_tile_opt {
                if !game_board.can_place_tile(hovered_hex.q, hovered_hex.r, active_tile, current_rotation) {
                    let valid_rots: Vec<usize> = (0..6)
                        .filter(|&rot| game_board.can_place_tile(hovered_hex.q, hovered_hex.r, active_tile, rot))
                        .collect();
                    if !valid_rots.is_empty() {
                        current_rotation = *valid_rots.iter()
                            .min_by_key(|&&rot| {
                                let diff = (rot as i32 - current_rotation as i32).rem_euclid(6) as usize;
                                diff.min(6 - diff)
                            })
                            .unwrap();
                    }
                }
            }
        }

        // Render available slot outlines
        if let Some(ref active_tile) = active_tile_opt {
            if !score_manager.is_game_over {
                let available_slots = game_board.get_available_placement_slots(active_tile);
                for ((sq, sr), is_slot_valid) in available_slots {
                    let slot_pos = HexPos::new(sq, sr);
                    if slot_pos == hovered_hex {
                        continue;
                    }
                    let screen_pos = (slot_pos.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;

                    if is_slot_valid {
                        draw_hex_lines(screen_pos, HEX_RADIUS * zoom, 1.8 * zoom, Color::from_rgba(80, 220, 240, 90));
                    } else {
                        draw_hex_lines(screen_pos, HEX_RADIUS * zoom, 1.8 * zoom, Color::from_rgba(255, 70, 70, 100));
                    }
                }
            }
        }

        // Validate placement against Board rules
        let can_place = if !score_manager.is_game_over {
            if let Some(ref active_tile) = active_tile_opt {
                game_board.can_place_tile(hovered_hex.q, hovered_hex.r, active_tile, current_rotation)
            } else {
                false
            }
        } else {
            false
        };

        // Compute live hover preview quest updates
        let preview_map = if can_place {
            if let Some(ref active_tile) = active_tile_opt {
                game_board.preview_quest_counts(hovered_hex.q, hovered_hex.r, active_tile, current_rotation)
            } else {
                std::collections::HashMap::new()
            }
        } else {
            std::collections::HashMap::new()
        };

        // Click LMB (without dragging) to Place Tile & Evaluate Score
        if can_place && is_mouse_button_released(MouseButton::Left) && total_drag_dist <= 6.0 {
            if let Some(active_tile) = active_tile_opt.as_ref() {
                let (placed_ok, bubble_quests_completed) = game_board.place_tile_with_manager(
                    hovered_hex.q,
                    hovered_hex.r,
                    active_tile.clone(),
                    current_rotation,
                    Some(&mut quest_manager),
                );

                if placed_ok {
                    let breakdown = score_manager.on_tile_placed(
                        &game_board,
                        hovered_hex.q,
                        hovered_hex.r,
                        bubble_quests_completed,
                        0,
                    );

                    let place_screen_pos = (hovered_hex.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;

                    println!(
                        "   ===> [TILE PLACED] Tile '{}' placed at ({}, {}) | FitScore: +{} ({} matches) | Perfects: {} | Quests: {} | TotalGain: +{} | RemainingStack: {}",
                        active_tile.tile_preset_string(),
                        hovered_hex.q,
                        hovered_hex.r,
                        breakdown.fit_score,
                        breakdown.matching_edges,
                        breakdown.perfect_count,
                        breakdown.bubble_quests_completed,
                        breakdown.total_score_gained,
                        score_manager.remaining_tiles
                    );

                    // Add Floating Toast Notifications
                    if breakdown.perfect_count > 0 {
                        floating_toasts.push(FloatingToast {
                            text: format!("+{} PERFECT! (+{} Tile)", breakdown.perfect_score, breakdown.perfect_count * score_manager.perfect_placement_tile_reward),
                            pos: place_screen_pos + Vec2::new(0.0, -40.0),
                            color: GOLD,
                            timer: 2.5,
                        });
                    }
                    if breakdown.bubble_quests_completed > 0 {
                        floating_toasts.push(FloatingToast {
                            text: format!("+{} QUEST! (+{} Tiles)", breakdown.bubble_quest_score, breakdown.bubble_quests_completed * score_manager.quest_bubble_tile_reward),
                            pos: place_screen_pos + Vec2::new(0.0, -20.0),
                            color: Color::from_rgba(80, 220, 120, 255),
                            timer: 2.5,
                        });
                    }
                    if breakdown.fit_score > 0 && breakdown.perfect_count == 0 && breakdown.bubble_quests_completed == 0 {
                        floating_toasts.push(FloatingToast {
                            text: format!("+{} Fit", breakdown.fit_score),
                            pos: place_screen_pos,
                            color: WHITE,
                            timer: 1.8,
                        });
                    }

                    // Pop placed tile khỏi cọc bài
                    tile_queue.pop_front();
                    current_rotation = 0;

                    // Kích hoạt QuestWatcher cho ô mới tiến lên vị trí đầu cọc bài (topStackPreview)
                    if let Some(front_tile) = tile_queue.front_mut() {
                        if let GeneratedTile::Quest { ref mut quest_data, .. } = front_tile {
                            if quest_data.quest_id.is_none() {
                                let qid = quest_manager.add_quest(&quest_data.quest_type);
                                quest_data.quest_id = Some(qid);
                            }
                        }
                    }

                    // Sinh Tile tiếp theo (Tile N+3) sau khi đặt ô xuống bàn chơi
                    let active_count = quest_manager.pop_next_active_quest_count();
                    let next_gen = generator.generate_tile(None, active_count, None, quest_manager.level);
                    tile_queue.push_back(next_gen);

                    // Nếu đạt ngưỡng điểm -> tile N+4 trở thành Train Station (KHÔNG thay tile N+3)
                    let should_reward = generator.should_grant_reward(score_manager.total_score);
                    if should_reward {
                        let reward = generator.grant_reward();
                        println!(
                            "   ===> 🏆 REWARD SPAWN (Train Station) | Score: {} | Step: {}",
                            score_manager.total_score, generator.last_rewarded_step
                        );
                        tile_queue.push_back(reward);
                    }

                    // Update TargetValue cho ô mới ở đầu cọc bài dựa theo trạng thái bàn chơi
                    if let Some(front_tile) = tile_queue.front_mut() {
                        init_active_quest_tile_target(front_tile, &game_board, &mut quest_manager);
                    }
                }
            }
        }

        // Active tile initialization at start of frame if uninitialized
        if let Some(front_tile) = tile_queue.front_mut() {
            init_active_quest_tile_target(front_tile, &game_board, &mut quest_manager);
        }

        // ── 2. Render Placed Tiles on Board ──
        for (&(q, r), placed_tile) in &game_board.placed_tiles {
            let hex_pos = HexPos::new(q, r);
            let screen_pos = (hex_pos.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;

            draw_hex_tile(screen_pos, HEX_RADIUS * zoom, &placed_tile.edge_config, 0, 1.0);
            draw_board_quest_badge(&game_board, screen_pos, placed_tile, HEX_RADIUS * zoom, preview_map.get(&(q, r)));
        }

        // ── 3. Render Placement Preview under Cursor ──
        if can_place {
            if let Some(ref active_tile) = active_tile_opt {
                let mut preview_cfg = active_tile.to_hex_edge_config();
                preview_cfg.rotate(current_rotation);

                let screen_pos = (hovered_hex.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;

                draw_hex_tile(screen_pos, HEX_RADIUS * zoom, &preview_cfg, 0, 0.65);
                draw_hex_lines(screen_pos, HEX_RADIUS * zoom, 3.0 * zoom, Color::from_rgba(255, 220, 100, 230));

                if let GeneratedTile::Quest { quest_data, .. } = active_tile {
                    let display_target = if let Some(&(rem_target, _)) = preview_map.get(&(hovered_hex.q, hovered_hex.r)) {
                        rem_target
                    } else {
                        quest_data.remaining_display_value()
                    };
                    draw_custom_badge_text(screen_pos, quest_data.primary_group_type(), quest_data.equality, display_target, HEX_RADIUS * zoom, 0.85);
                }
            }
        } else if !score_manager.is_game_over && !game_board.placed_tiles.contains_key(&(hovered_hex.q, hovered_hex.r)) {
            let screen_pos = (hovered_hex.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;
            draw_hex_lines(screen_pos, HEX_RADIUS * zoom, 1.5 * zoom, Color::from_rgba(255, 60, 60, 120));
        }

        // ── Render Floating Toasts ──
        for toast in &floating_toasts {
            let font_size = 20.0;
            let tw = measure_text(&toast.text, None, font_size as u16, 1.0).width;
            let mut col = toast.color;
            col.a = (toast.timer / 0.5).min(1.0);
            draw_text(&toast.text, toast.pos.x - tw * 0.5, toast.pos.y, font_size, col);
        }

        // ── 4. UI Overlay (Score, Active Quest Counts & Queue Preview) ──
        draw_rectangle(15.0, 15.0, 380.0, 180.0, Color::from_rgba(10, 12, 18, 230));
        draw_rectangle_lines(15.0, 15.0, 380.0, 180.0, 2.0, SKYBLUE);

        draw_text("DORFROMANTIK SIMULATOR", 28.0, 38.0, 20.0, SKYBLUE);
        draw_text(&format!("Total Score: {} pts", score_manager.total_score), 28.0, 65.0, 18.0, GOLD);

        let stack_color = if score_manager.remaining_tiles > 5 {
            Color::from_rgba(80, 220, 120, 255)
        } else if score_manager.remaining_tiles > 0 {
            Color::from_rgba(240, 180, 40, 255)
        } else {
            Color::from_rgba(240, 60, 60, 255)
        };
        draw_text(&format!("Tile Stack: {} remaining", score_manager.remaining_tiles), 28.0, 88.0, 16.0, stack_color);

        draw_text(&format!("Tiles Placed: {}  |  Perfects: {}", score_manager.placed_tiles_count, score_manager.perfect_count), 28.0, 110.0, 15.0, WHITE);
        draw_text(&format!("Active Quests: {}", quest_manager.active_quest_count()), 28.0, 130.0, 15.0, LIGHTGRAY);
        draw_text("LMB: Place | RMB Drag: Pan | Wheel / R: Rotate", 28.0, 156.0, 13.0, GRAY);
        draw_text("Ctrl + Wheel: Zoom", 28.0, 172.0, 13.0, GRAY);

        // ── Top 3 Preview Queue Panel (Right Side UI) ──
        let panel_w = 170.0;
        let panel_h = 440.0;
        let panel_x = screen_w - panel_w - 20.0;
        let panel_y = 20.0;

        draw_rectangle(panel_x, panel_y, panel_w, panel_h, Color::from_rgba(12, 16, 24, 230));
        draw_rectangle_lines(panel_x, panel_y, panel_w, panel_h, 2.0, SKYBLUE);

        draw_text("TILE QUEUE", panel_x + 35.0, panel_y + 30.0, 18.0, SKYBLUE);

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

        // ── 5. Render Game Over Screen Overlay ──
        if score_manager.is_game_over {
            draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::from_rgba(5, 8, 15, 200));

            let box_w = 460.0;
            let box_h = 240.0;
            let box_x = (screen_w - box_w) * 0.5;
            let box_y = (screen_h - box_h) * 0.5;

            draw_rectangle(box_x, box_y, box_w, box_h, Color::from_rgba(16, 20, 30, 245));
            draw_rectangle_lines(box_x, box_y, box_w, box_h, 3.0, GOLD);

            let title = "GAME OVER";
            let tw = measure_text(title, None, 36, 1.0).width;
            draw_text(title, (screen_w - tw) * 0.5, box_y + 50.0, 36.0, RED);

            let score_text = format!("FINAL SCORE: {} PTS", score_manager.total_score);
            let stw = measure_text(&score_text, None, 24, 1.0).width;
            draw_text(&score_text, (screen_w - stw) * 0.5, box_y + 95.0, 24.0, GOLD);

            let stats_text = format!("Tiles Placed: {}  |  Perfect Placements: {}", score_manager.placed_tiles_count, score_manager.perfect_count);
            let statsw = measure_text(&stats_text, None, 16, 1.0).width;
            draw_text(&stats_text, (screen_w - statsw) * 0.5, box_y + 130.0, 16.0, WHITE);

            let hint_text = "Out of tiles in stack!";
            let hw = measure_text(hint_text, None, 16, 1.0).width;
            draw_text(hint_text, (screen_w - hw) * 0.5, box_y + 175.0, 16.0, LIGHTGRAY);
        }

        next_frame().await;
    }
}

/// Helper function to draw a flat-topped Hexagon tile
fn draw_hex_tile(center: Vec2, radius: f32, config: &HexEdgeConfig, rotation: usize, alpha: f32) {
    let mut points = [Vec2::ZERO; 6];
    let angles: [f32; 6] = [
        4.0 * std::f32::consts::FRAC_PI_3,
        5.0 * std::f32::consts::FRAC_PI_3,
        0.0,
        std::f32::consts::FRAC_PI_3,
        2.0 * std::f32::consts::FRAC_PI_3,
        std::f32::consts::PI,
    ];

    for i in 0..6 {
        points[i] = Vec2::new(
            center.x + radius * angles[i].cos(),
            center.y + radius * angles[i].sin(),
        );
    }

    let mut fill_color = Color::from_rgba(144, 190, 109, 255);
    fill_color.a *= alpha;
    for i in 0..6 {
        let p1 = points[i];
        let p2 = points[(i + 1) % 6];
        draw_triangle(center, p1, p2, fill_color);
    }

    for i in 0..6 {
        let edge_type = config.edge_at(i, rotation);
        if edge_type != EdgeType::Plain {
            let p1 = points[i];
            let p2 = points[(i + 1) % 6];
            let mut edge_color = get_edge_color(edge_type);
            edge_color.a *= alpha;
            let mid_p1 = center + (p1 - center) * 0.72;
            let mid_p2 = center + (p2 - center) * 0.72;

            draw_triangle(p1, p2, mid_p2, edge_color);
            draw_triangle(p1, mid_p2, mid_p1, edge_color);
        }
    }

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
        4.0 * std::f32::consts::FRAC_PI_3,
        5.0 * std::f32::consts::FRAC_PI_3,
        0.0,
        std::f32::consts::FRAC_PI_3,
        2.0 * std::f32::consts::FRAC_PI_3,
        std::f32::consts::PI,
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
        let effective_status = preview.map(|p| p.1).or(placed_tile.quest_status);

        match effective_status {
            Some(FulfillmentStatus::Success) => {
                draw_circle(center.x, center.y, badge_radius, Color::from_rgba(40, 180, 80, 240));
                draw_circle_lines(center.x, center.y, badge_radius, 2.5, WHITE);
                let text_w = measure_text("OK", None, (badge_radius * 1.0) as u16, 1.0).width;
                draw_text("OK", center.x - text_w * 0.5, center.y + badge_radius * 0.35, badge_radius * 1.0, WHITE);
            }
            Some(FulfillmentStatus::Failed) => {
                draw_circle(center.x, center.y, badge_radius, Color::from_rgba(180, 40, 40, 240));
                draw_circle_lines(center.x, center.y, badge_radius, 2.5, WHITE);
                let text_w = measure_text("X", None, (badge_radius * 1.1) as u16, 1.0).width;
                draw_text("X", center.x - text_w * 0.5, center.y + badge_radius * 0.35, badge_radius * 1.1, WHITE);
            }
            _ => {
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

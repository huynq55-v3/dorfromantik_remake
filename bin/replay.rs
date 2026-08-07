use macroquad::prelude::*;
use dorfromantik_remake::env::DorfromantikEnv;
use dorfromantik_remake::tile::{EdgeType, GeneratedTile, HexEdgeConfig, EqualityComparison};
use dorfromantik_remake::game_config::GroupType;
use serde::Deserialize;
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
        EdgeType::Plain => Color::from_rgba(144, 190, 109, 255),
        EdgeType::Agriculture => Color::from_rgba(230, 194, 41, 255),
        EdgeType::Forest => Color::from_rgba(45, 106, 79, 255),
        EdgeType::Village => Color::from_rgba(224, 86, 60, 255),
        EdgeType::Water => Color::from_rgba(0, 119, 182, 255),
        EdgeType::FlexibleWater => Color::from_rgba(0, 200, 190, 255),
        EdgeType::TrainTracks => Color::from_rgba(74, 78, 105, 255),
        EdgeType::WaterTrainStation => Color::from_rgba(114, 9, 183, 255),
    }
}

#[derive(Debug, Deserialize)]
struct ReplayMove {
    step: usize,
    q: i32,
    r: i32,
    rotation: usize,
    score_gained: usize,
    total_score: usize,
    remaining_tiles: usize,
}

#[derive(Debug, Deserialize)]
struct GameRecord {
    seed: i32,
    total_score: usize,
    total_placed: usize,
    is_eval: bool,
    moves: Vec<ReplayMove>,
}

#[macroquad::main("Dorfromantik Replay Visualizer")]
async fn main() {
    let file_content = fs::read_to_string("models/best_game_record.json")
        .expect("Failed to read models/best_game_record.json");
    let record: GameRecord = serde_json::from_str(&file_content)
        .expect("Failed to parse JSON");

    println!("Loaded Replay | Seed: {} | Total Score: {} | Moves: {}", record.seed, record.total_score, record.moves.len());

    let mut stack_height = 10;
    if let Ok(content) = fs::read_to_string("monthly_game_info.txt") {
        for line in content.lines() {
            if line.contains("ACTIVE_TileStackHeight=") {
                if let Ok(v) = line.trim_start_matches("ACTIVE_TileStackHeight=").parse::<usize>() {
                    stack_height = v;
                }
            }
        }
    }

    let mut env = DorfromantikEnv::new(record.seed, stack_height, 100);
    let mut current_step = 0;

    let mut camera_pos = Vec2::ZERO;
    let mut zoom: f32 = 1.0;
    let mut last_mouse_pos = mouse_position();
    let mut total_drag_dist: f32 = 0.0;

    loop {
        clear_background(Color::from_rgba(20, 24, 33, 255));

        let delta = get_frame_time();
        let (screen_w, screen_h) = (screen_width(), screen_height());

        // Controls
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
        if is_key_down(KeyCode::W) { camera_pos.y -= speed * delta; }
        if is_key_down(KeyCode::S) { camera_pos.y += speed * delta; }
        if is_key_down(KeyCode::A) { camera_pos.x -= speed * delta; }
        if is_key_down(KeyCode::D) { camera_pos.x += speed * delta; }

        let wheel = mouse_wheel().1;
        if wheel > 0.0 { zoom *= 1.12; }
        if wheel < 0.0 { zoom *= 0.88; }
        if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::Key1) { zoom *= 1.15; }
        if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::Key2) { zoom *= 0.85; }
        if is_key_pressed(KeyCode::C) || is_key_pressed(KeyCode::Home) {
            camera_pos = Vec2::ZERO;
            zoom = 1.0;
        }
        zoom = zoom.clamp(0.2, 4.0);

        let center_vec = Vec2::new(screen_w * 0.5, screen_h * 0.5);

        // Step Forward
        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::Space) {
            if current_step < record.moves.len() {
                let m = &record.moves[current_step];
                let legal_actions = env.get_valid_actions();
                
                // Find action index that matches q, r, rotation
                let mut found_action = None;
                for action in legal_actions.iter() {
                    if action.q == m.q && action.r == m.r && action.rotation == m.rotation {
                        found_action = Some(dorfromantik_remake::env::Action { q: m.q, r: m.r, rotation: m.rotation });
                        break;
                    }
                }
                
                if let Some(action) = found_action {
                    let result = env.step(action);
                    println!("Step {} -> Placed at ({}, {}) Rot: {} | Gained: {} | Score: {} | Done: {}", 
                        current_step, m.q, m.r, m.rotation, result.reward, env.score_manager.total_score, result.done);
                    current_step += 1;
                } else {
                    println!("WARNING: Move not legal! Step {} ({}, {}) Rot: {}", current_step, m.q, m.r, m.rotation);
                }
            }
        }

        // Step Backward (Rewind)
        if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::Backspace) {
            if current_step > 0 {
                current_step -= 1;
                env = DorfromantikEnv::new(record.seed, stack_height, 100);
                for s in 0..current_step {
                    let m = &record.moves[s];
                    let legal_actions = env.get_valid_actions();
                    for action in legal_actions.iter() {
                        if action.q == m.q && action.r == m.r && action.rotation == m.rotation {
                            env.step(*action);
                            break;
                        }
                    }
                }
                println!("Step Backward -> Step {}", current_step);
            }
        }
        
        if is_key_pressed(KeyCode::R) {
            // Reset to beginning
            env = DorfromantikEnv::new(record.seed, stack_height, 100);
            current_step = 0;
            println!("Reset Replay");
        }

        // Render Placed Tiles
        for (&(q, r), placed_tile) in &env.board.placed_tiles {
            let hex_pos = HexPos::new(q, r);
            let screen_pos = (hex_pos.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;

            draw_hex_tile(screen_pos, HEX_RADIUS * zoom, &placed_tile.edge_config, 0, 1.0);
            
            // Quest badge
            if let GeneratedTile::Quest { quest_data, .. } = &placed_tile.tile {
                let badge_radius = HEX_RADIUS * zoom * 0.38;
                match placed_tile.quest_status {
                    Some(dorfromantik_remake::board::FulfillmentStatus::Success) => {
                        draw_circle(screen_pos.x, screen_pos.y, badge_radius, Color::from_rgba(40, 180, 80, 240));
                        draw_circle_lines(screen_pos.x, screen_pos.y, badge_radius, 2.5, WHITE);
                        let text_w = measure_text("OK", None, (badge_radius * 1.0) as u16, 1.0).width;
                        draw_text("OK", screen_pos.x - text_w * 0.5, screen_pos.y + badge_radius * 0.35, badge_radius * 1.0, WHITE);
                    }
                    Some(dorfromantik_remake::board::FulfillmentStatus::Failed) => {
                        draw_circle(screen_pos.x, screen_pos.y, badge_radius, Color::from_rgba(180, 40, 40, 240));
                        draw_circle_lines(screen_pos.x, screen_pos.y, badge_radius, 2.5, WHITE);
                        let text_w = measure_text("X", None, (badge_radius * 1.1) as u16, 1.0).width;
                        draw_text("X", screen_pos.x - text_w * 0.5, screen_pos.y + badge_radius * 0.35, badge_radius * 1.1, WHITE);
                    }
                    _ => {
                        let target = env.board.get_quest_remaining_target((q, r));
                        draw_custom_badge_text(screen_pos, quest_data.primary_group_type(), quest_data.equality, target, HEX_RADIUS * zoom, 1.0);
                    }
                }
            }
        }

        // Render Active Tile Preview (if not done)
        if current_step < record.moves.len() {
            let m = &record.moves[current_step];
            if let Some(active_tile) = env.tile_queue.front() {
                let mut preview_cfg = active_tile.to_hex_edge_config();
                preview_cfg.rotate(m.rotation);
                
                let hex_pos = HexPos::new(m.q, m.r);
                let screen_pos = (hex_pos.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;

                draw_hex_tile(screen_pos, HEX_RADIUS * zoom, &preview_cfg, 0, 0.5);
                draw_hex_lines(screen_pos, HEX_RADIUS * zoom, 3.0 * zoom, Color::from_rgba(255, 220, 100, 230));
            }
        }

        // UI Overlay
        draw_rectangle(15.0, 15.0, 390.0, 175.0, Color::from_rgba(10, 12, 18, 230));
        draw_rectangle_lines(15.0, 15.0, 390.0, 175.0, 2.0, SKYBLUE);

        draw_text("REPLAY VISUALIZER", 28.0, 38.0, 20.0, SKYBLUE);
        draw_text(&format!("Step: {} / {}", current_step, record.moves.len()), 28.0, 65.0, 18.0, WHITE);
        draw_text(&format!("Total Score: {} pts", env.score_manager.total_score), 28.0, 88.0, 18.0, GOLD);
        draw_text(&format!("Tile Stack: {} remaining", env.score_manager.remaining_tiles), 28.0, 110.0, 16.0, WHITE);
        draw_text("Right Arrow / Space: Step Forward", 28.0, 132.0, 14.0, LIGHTGRAY);
        draw_text("Left Arrow / Backspace: Step Backward", 28.0, 149.0, 14.0, LIGHTGRAY);
        draw_text("WASD: Move Camera | R: Reset Replay", 28.0, 166.0, 14.0, LIGHTGRAY);

        next_frame().await;
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn draw_custom_badge_text(
    center: Vec2,
    primary_gt: GroupType,
    equality: EqualityComparison,
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
        EqualityComparison::MoreThan => format!("+{}", target_count),
        EqualityComparison::Exactly => format!("={}", target_count),
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

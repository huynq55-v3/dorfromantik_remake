use macroquad::prelude::*;
use dorfromantik_remake::alphazero::{AlphaZeroPipeline, AlphaZeroTrainerConfig, GameMoveRecord, MaxScoreStateRecord};
use dorfromantik_remake::board::{Board, FulfillmentStatus, PlacedTile};
use dorfromantik_remake::env::{Action, DorfromantikEnv, GraphObservation};
use dorfromantik_remake::game_config::GroupType;
use dorfromantik_remake::mcts::MCTSConfig;
use dorfromantik_remake::nn::HexGNNModel;
use dorfromantik_remake::tile::{EdgeType, EqualityComparison, GeneratedTile, HexEdgeConfig, QuestTileData};
use std::fs;
use std::path::Path;

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

fn load_game_config() -> (i32, usize, usize) {
    let mut seed = -2093096630;
    let mut stack = 10;
    let mut limit = 100;
    if let Ok(content) = fs::read_to_string("monthly_game_info.txt") {
        for line in content.lines() {
            if let Some((k, v)) = line.split_once('=') {
                match k.trim() {
                    "REAL_TILE_SEED" => if let Ok(s) = v.trim().parse() { seed = s; },
                    "ACTIVE_TileStackHeight" => if let Ok(s) = v.trim().parse() { stack = s; },
                    "ACTIVE_TileLimit" => if let Ok(s) = v.trim().parse() { limit = s; },
                    _ => {}
                }
            }
        }
    }
    (seed, stack, limit)
}

struct FloatingToast {
    text: String,
    pos: Vec2,
    color: Color,
    timer: f32,
}

#[derive(Clone)]
enum AppState {
    SelectMode,
    Playing {
        env: DorfromantikEnv,
        move_history: Vec<GameMoveRecord>,
        redo_history: Vec<GameMoveRecord>,
        raw_steps: Vec<(GraphObservation, f32)>, // obs, reward
        is_recording: bool,
        record_start_idx: usize,
    },
    GameOver {
        final_score: usize,
        final_placed: usize,
        evaluated_states_count: usize,
        qualified_states_count: usize,
    },
}

#[macroquad::main("Human Expert Play - Dorfromantik")]
async fn main() {
    let (seed, initial_stack, tile_limit) = load_game_config();
    let max_score_states_path = "models/max_score_states.json";
    let model_path = if Path::new("models/alphazero_best.bin").exists() {
        "models/alphazero_best.bin"
    } else {
        "models/alphazero_latest.bin"
    };

    let model = match HexGNNModel::load_from_file(model_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error: Cannot load evaluation model from `{}`: {:?}", model_path, e);
            std::process::exit(1);
        }
    };
    println!(">>> Loaded evaluation model from `{}` (Step count = {}) <<<", model_path, model.step_count);

    let mut states_list: Vec<MaxScoreStateRecord> = Vec::new();
    if Path::new(max_score_states_path).exists() {
        if let Ok(content) = fs::read_to_string(max_score_states_path) {
            if let Ok(st) = serde_json::from_str::<Vec<MaxScoreStateRecord>>(&content) {
                states_list = st;
            }
        }
    }
    println!(">>> Loaded {} max-score states from `{}` <<<", states_list.len(), max_score_states_path);

    let mut app_state = AppState::SelectMode;
    let mut camera_pos = Vec2::ZERO;
    let mut zoom: f32 = 1.0;
    let mut last_mouse_pos = mouse_position();
    let mut total_drag_dist: f32 = 0.0;
    let mut current_rotation: usize = 0;
    let mut floating_toasts: Vec<FloatingToast> = Vec::new();

    loop {
        clear_background(Color::from_rgba(20, 24, 33, 255));
        let delta = get_frame_time();
        let (screen_w, screen_h) = (screen_width(), screen_height());
        let current_mouse_pos = mouse_position();
        let mouse_delta = Vec2::new(current_mouse_pos.0 - last_mouse_pos.0, current_mouse_pos.1 - last_mouse_pos.1);

        if is_mouse_button_pressed(MouseButton::Left) || is_mouse_button_pressed(MouseButton::Right) {
            total_drag_dist = 0.0;
        }
        if is_mouse_button_down(MouseButton::Left) || is_mouse_button_down(MouseButton::Right) {
            total_drag_dist += mouse_delta.length();
        }

        // Update floating toasts
        floating_toasts.retain_mut(|t| {
            t.timer -= delta;
            t.pos.y -= 30.0 * delta;
            t.timer > 0.0
        });

        let mut next_app_state = None;

        match &mut app_state {
            AppState::SelectMode => {
                let box_w = 600.0;
                let box_h = 320.0;
                let bx = (screen_w - box_w) * 0.5;
                let by = (screen_h - box_h) * 0.5;

                draw_rectangle(bx, by, box_w, box_h, Color::from_rgba(16, 22, 32, 245));
                draw_rectangle_lines(bx, by, box_w, box_h, 2.5, SKYBLUE);

                let title = "EXPERT HUMAN PLAY";
                let tw = measure_text(title, None, 28, 1.0).width;
                draw_text(title, (screen_w - tw) * 0.5, by + 45.0, 28.0, SKYBLUE);

                // Option 1: New Game
                draw_rectangle(bx + 35.0, by + 80.0, box_w - 70.0, 80.0, Color::from_rgba(25, 45, 35, 255));
                draw_rectangle_lines(bx + 35.0, by + 80.0, box_w - 70.0, 80.0, 1.5, GREEN);
                draw_text("[1] START NEW GAME (Turn 0)", bx + 55.0, by + 115.0, 22.0, GREEN);
                draw_text("Build full board from scratch with expert human strategy.", bx + 55.0, by + 142.0, 15.0, LIGHTGRAY);

                // Option 2: Random Max Score State
                let count_str = format!("Continue from random top board ({}/2000 states available).", states_list.len());
                draw_rectangle(bx + 35.0, by + 175.0, box_w - 70.0, 80.0, Color::from_rgba(30, 42, 65, 255));
                draw_rectangle_lines(bx + 35.0, by + 175.0, box_w - 70.0, 80.0, 1.5, SKYBLUE);
                draw_text("[2] LOAD RANDOM MAX-SCORE STATE", bx + 55.0, by + 210.0, 22.0, SKYBLUE);
                draw_text(&count_str, bx + 55.0, by + 237.0, 15.0, LIGHTGRAY);

                let hint = "Press [1] or [2] on your keyboard to start";
                let hw = measure_text(hint, None, 17, 1.0).width;
                draw_text(hint, (screen_w - hw) * 0.5, by + 290.0, 17.0, YELLOW);

                if is_key_pressed(KeyCode::Key1) || is_key_pressed(KeyCode::Kp1) {
                    let env = DorfromantikEnv::new(seed, initial_stack, tile_limit);
                    next_app_state = Some(AppState::Playing {
                        env,
                        move_history: Vec::new(),
                        redo_history: Vec::new(),
                        raw_steps: Vec::new(),
                        is_recording: true,
                        record_start_idx: 0,
                    });
                }

                if (is_key_pressed(KeyCode::Key2) || is_key_pressed(KeyCode::Kp2)) && !states_list.is_empty() {
                    use ::rand::Rng;
                    let mut rng = ::rand::thread_rng();
                    let r_bias = rng.gen::<f32>().powi(2); // Quadratic bias towards top Q
                    let pick_idx = (r_bias * states_list.len() as f32) as usize;
                    let st = &states_list[pick_idx];

                    let mut env = DorfromantikEnv::new(seed, initial_stack, tile_limit);
                    let mut replay_history = Vec::new();
                    for m in &st.moves {
                        let curr_tile = env.current_tile().cloned().unwrap();
                        let canonical_rot = m.rotation % curr_tile.rotation_symmetry_period();
                        let prev_sc = env.score_manager.total_score;
                        env.step(Action { q: m.q, r: m.r, rotation: canonical_rot });
                        let gained = env.score_manager.total_score.saturating_sub(prev_sc);
                        replay_history.push(GameMoveRecord {
                            step: replay_history.len(),
                            q: m.q,
                            r: m.r,
                            rotation: canonical_rot,
                            score_gained: gained,
                            total_score: env.score_manager.total_score,
                            remaining_tiles: env.score_manager.remaining_tiles,
                        });
                    }
                    println!(
                        "\n>>> Loaded State #{}/{} (Score: {}, Placed: {} tiles, Stack: {} tiles, Q-Value: {:.1}) <<<",
                        pick_idx + 1, states_list.len(), env.score_manager.total_score, env.placed_count, env.score_manager.remaining_tiles, st.q_value
                    );
                    next_app_state = Some(AppState::Playing {
                        env,
                        move_history: replay_history,
                        redo_history: Vec::new(),
                        raw_steps: Vec::new(),
                        is_recording: true,
                        record_start_idx: st.moves.len(),
                    });
                }
            }

            AppState::Playing { env, move_history, redo_history, raw_steps, is_recording, record_start_idx } => {
                // Toggle Record [T]
                if is_key_pressed(KeyCode::T) {
                    *is_recording = !*is_recording;
                    if *is_recording {
                        *record_start_idx = move_history.len();
                        raw_steps.clear();
                        println!("🔴 [RECORD ON] Started recording states from Move #{}!", move_history.len());
                    } else {
                        println!("⚪ [RECORD PAUSED] Paused recording.");
                    }
                }

                // Handle Undo [U] / [Z]
                if (is_key_pressed(KeyCode::U) || (is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::Z)) || is_key_pressed(KeyCode::Z)) && !move_history.is_empty() {
                    let popped = move_history.pop().unwrap();
                    redo_history.push(popped);
                    if !raw_steps.is_empty() {
                        raw_steps.pop();
                    }

                    // Reset env and replay up to current history
                    let mut fresh_env = DorfromantikEnv::new(seed, initial_stack, tile_limit);
                    let mut fresh_history = Vec::new();
                    for m in move_history.iter() {
                        let curr_tile = fresh_env.current_tile().cloned().unwrap();
                        let canonical_rot = m.rotation % curr_tile.rotation_symmetry_period();
                        let prev_sc = fresh_env.score_manager.total_score;
                        fresh_env.step(Action { q: m.q, r: m.r, rotation: canonical_rot });
                        let gained = fresh_env.score_manager.total_score.saturating_sub(prev_sc);
                        fresh_history.push(GameMoveRecord {
                            step: fresh_history.len(),
                            q: m.q,
                            r: m.r,
                            rotation: canonical_rot,
                            score_gained: gained,
                            total_score: fresh_env.score_manager.total_score,
                            remaining_tiles: fresh_env.score_manager.remaining_tiles,
                        });
                    }
                    *env = fresh_env;
                    *move_history = fresh_history;
                    if *record_start_idx > move_history.len() {
                        *record_start_idx = move_history.len();
                    }
                    println!("↩️ [UNDO] Reverted 1 move! Placed = {}, Score = {} (Redo stack: {})", env.placed_count, env.score_manager.total_score, redo_history.len());
                }

                // Handle Redo [Y] / [Ctrl+Y]
                let is_redo_key = is_key_pressed(KeyCode::Y) || (is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::Y)) || (is_key_down(KeyCode::LeftControl) && is_key_down(KeyCode::LeftShift) && is_key_pressed(KeyCode::Z));
                if is_redo_key && !redo_history.is_empty() {
                    let next_m = redo_history.pop().unwrap();
                    let obs = env.extract_graph_observation();
                    let prev_sc = env.score_manager.total_score;
                    let act = Action { q: next_m.q, r: next_m.r, rotation: next_m.rotation };
                    let res = env.step(act);
                    let gained = env.score_manager.total_score.saturating_sub(prev_sc);

                    move_history.push(GameMoveRecord {
                        step: move_history.len(),
                        q: act.q,
                        r: act.r,
                        rotation: act.rotation,
                        score_gained: gained,
                        total_score: env.score_manager.total_score,
                        remaining_tiles: env.score_manager.remaining_tiles,
                    });
                    if *is_recording {
                        raw_steps.push((obs, res.reward * 0.01));
                    }
                    println!("↪️ [REDO] Re-applied move #{:02} ({}, {}) rot:{} -> Score: {}", move_history.len(), act.q, act.r, act.rotation, env.score_manager.total_score);
                }

                // Camera Dragging Controls (LMB / RMB / MMB drag on empty space or anywhere)
                if (is_mouse_button_down(MouseButton::Left) || is_mouse_button_down(MouseButton::Right) || is_mouse_button_down(MouseButton::Middle)) && total_drag_dist > 4.0 {
                    camera_pos.x -= mouse_delta.x / zoom;
                    camera_pos.y -= mouse_delta.y / zoom;
                }
                let speed = 650.0 / zoom;
                if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) { camera_pos.y -= speed * delta; }
                if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) { camera_pos.y += speed * delta; }
                if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) { camera_pos.x -= speed * delta; }
                if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) { camera_pos.x += speed * delta; }

                if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::Key1) { zoom *= 1.15; }
                if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::Key2) { zoom *= 0.85; }
                if is_key_pressed(KeyCode::C) || is_key_pressed(KeyCode::Home) {
                    camera_pos = Vec2::ZERO;
                    zoom = 1.0;
                }

                let center_vec = Vec2::new(screen_w * 0.5, screen_h * 0.5);
                let mouse_vec = Vec2::new(current_mouse_pos.0, current_mouse_pos.1);
                let mouse_world = (mouse_vec - center_vec) / zoom + camera_pos;
                let hovered_hex = screen_to_hex(mouse_world, HEX_RADIUS);

                let active_tile_opt = env.current_tile();
                let is_hovering_slot = if let Some(curr) = active_tile_opt {
                    let available_slots = env.board.get_available_placement_slots(curr);
                    available_slots.iter().any(|&((sq, sr), _)| sq == hovered_hex.q && sr == hovered_hex.r)
                } else {
                    false
                };

                // Mouse Wheel Behavior:
                // If hovering over a valid/available candidate placement slot -> Rotate Tile!
                // If hovering over empty background / elsewhere -> Zoom In / Zoom Out!
                let wheel = mouse_wheel().1;
                if wheel != 0.0 {
                    if is_hovering_slot {
                        if let Some(curr) = env.current_tile() {
                            let forward = wheel < 0.0;
                            current_rotation = env.board.get_next_valid_rotation(hovered_hex.q, hovered_hex.r, curr, current_rotation, forward);
                        } else {
                            if wheel < 0.0 { current_rotation = (current_rotation + 1) % 6; }
                            else { current_rotation = (current_rotation + 5) % 6; }
                        }
                    } else {
                        if wheel > 0.0 { zoom *= 1.12; }
                        if wheel < 0.0 { zoom *= 0.88; }
                    }
                }
                zoom = zoom.clamp(0.15, 4.0);

                // Rotate Tile: Right Click, Space, or Key R
                let rmb_clicked = is_mouse_button_released(MouseButton::Right) && total_drag_dist <= 6.0;
                if rmb_clicked || is_mouse_button_pressed(MouseButton::Middle) || is_key_pressed(KeyCode::R) || is_key_pressed(KeyCode::Space) {
                    if let Some(curr) = env.current_tile() {
                        current_rotation = env.board.get_next_valid_rotation(hovered_hex.q, hovered_hex.r, curr, current_rotation, true);
                    } else {
                        current_rotation = (current_rotation + 1) % 6;
                    }
                }
                if is_key_pressed(KeyCode::Q) {
                    if let Some(curr) = env.current_tile() {
                        current_rotation = env.board.get_next_valid_rotation(hovered_hex.q, hovered_hex.r, curr, current_rotation, false);
                    } else {
                        current_rotation = (current_rotation + 5) % 6;
                    }
                }

                // Auto-snap rotation to valid slot
                if let Some(curr) = active_tile_opt {
                    if !env.board.can_place_tile(hovered_hex.q, hovered_hex.r, curr, current_rotation) {
                        let valid_rots: Vec<usize> = (0..6)
                            .filter(|&rot| env.board.can_place_tile(hovered_hex.q, hovered_hex.r, curr, rot))
                            .collect();
                        if !valid_rots.is_empty() {
                            current_rotation = *valid_rots.iter().min_by_key(|&&rot| {
                                let diff = (rot as i32 - current_rotation as i32).rem_euclid(6) as usize;
                                diff.min(6 - diff)
                            }).unwrap();
                        }
                    }
                }

                let can_place = if let Some(curr) = active_tile_opt {
                    env.board.can_place_tile(hovered_hex.q, hovered_hex.r, curr, current_rotation)
                } else {
                    false
                };

                let preview_map = if can_place {
                    if let Some(curr) = active_tile_opt {
                        env.board.preview_quest_counts(hovered_hex.q, hovered_hex.r, curr, current_rotation)
                    } else {
                        std::collections::HashMap::new()
                    }
                } else {
                    std::collections::HashMap::new()
                };

                // ── 1. Draw Available Slot Outlines ──
                if let Some(curr) = active_tile_opt {
                    let available_slots = env.board.get_available_placement_slots(curr);
                    for ((sq, sr), is_slot_valid) in available_slots {
                        let slot_pos = HexPos::new(sq, sr);
                        if slot_pos == hovered_hex { continue; }
                        let screen_pos = (slot_pos.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;
                        if is_slot_valid {
                            draw_hex_lines(screen_pos, HEX_RADIUS * zoom, 1.8 * zoom, Color::from_rgba(80, 220, 240, 90));
                        } else {
                            draw_hex_lines(screen_pos, HEX_RADIUS * zoom, 1.8 * zoom, Color::from_rgba(255, 70, 70, 100));
                        }
                    }
                }

                // ── 2. Draw Board Tiles (Beautiful Solid Render with Edge Wedges) ──
                for (&pos, pt) in &env.board.placed_tiles {
                    let hp = HexPos::new(pos.0, pos.1);
                    let sp = (hp.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;
                    draw_hex_tile(sp, HEX_RADIUS * zoom, &pt.edge_config, 0, 1.0);
                    draw_board_quest_badge(&env.board, sp, pt, HEX_RADIUS * zoom, preview_map.get(&(pos.0, pos.1)));
                }

                // ── 3. Draw Hover Tile Preview ──
                if let Some(curr) = active_tile_opt {
                    let sp = (hovered_hex.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;
                    if can_place {
                        let mut preview_cfg = curr.to_hex_edge_config();
                        preview_cfg.rotate(current_rotation);
                        draw_hex_tile(sp, HEX_RADIUS * zoom, &preview_cfg, 0, 0.65);
                        draw_hex_lines(sp, HEX_RADIUS * zoom, 3.0 * zoom, Color::from_rgba(255, 220, 100, 230));

                        if let GeneratedTile::Quest { quest_data, .. } = curr {
                            let display_target = if let Some(&(rem_target, _)) = preview_map.get(&(hovered_hex.q, hovered_hex.r)) {
                                rem_target
                            } else {
                                quest_data.remaining_display_value()
                            };
                            draw_custom_badge_text(sp, quest_data.primary_group_type(), quest_data.equality, display_target, HEX_RADIUS * zoom, 0.85);
                        }
                    } else if !env.board.placed_tiles.contains_key(&(hovered_hex.q, hovered_hex.r)) {
                        draw_hex_lines(sp, HEX_RADIUS * zoom, 1.5 * zoom, Color::from_rgba(255, 60, 60, 120));
                    }
                }

                // ── 4. Render Floating Toasts ──
                for toast in &floating_toasts {
                    let font_size = 20.0;
                    let tw = measure_text(&toast.text, None, font_size as u16, 1.0).width;
                    let mut col = toast.color;
                    col.a = (toast.timer / 0.5).min(1.0);
                    draw_text(&toast.text, toast.pos.x - tw * 0.5, toast.pos.y, font_size, col);
                }

                // ── 5. UI Overlay (Top Left Panel) ──
                draw_rectangle(15.0, 15.0, 360.0, 150.0, Color::from_rgba(10, 14, 22, 230));
                draw_rectangle_lines(15.0, 15.0, 360.0, 150.0, 2.0, SKYBLUE);

                draw_text("HUMAN EXPERT MODE", 28.0, 38.0, 20.0, SKYBLUE);
                draw_text(&format!("Total Score: {} pts", env.score_manager.total_score), 28.0, 65.0, 18.0, GOLD);

                let stack_color = if env.score_manager.remaining_tiles > 5 {
                    Color::from_rgba(80, 220, 120, 255)
                } else if env.score_manager.remaining_tiles > 0 {
                    Color::from_rgba(240, 180, 40, 255)
                } else {
                    Color::from_rgba(240, 60, 60, 255)
                };
                draw_text(&format!("Tile Stack: {} remaining", env.score_manager.remaining_tiles), 28.0, 88.0, 16.0, stack_color);
                draw_text(&format!("Tiles Placed: {}/{}  |  Perfects: {}", env.placed_count, tile_limit, env.score_manager.perfect_count), 28.0, 110.0, 15.0, WHITE);
                draw_text(&format!("Active Quests: {}", env.quest_manager.active_quest_count()), 28.0, 130.0, 15.0, LIGHTGRAY);

                // ── 6. Top Right Record Button & Queue Panel ──
                let panel_w = 160.0;
                let panel_h = 420.0;
                let panel_x = screen_w - panel_w - 20.0;
                let panel_y = 20.0;

                // Record Button Header
                let (rec_text, rec_bg, rec_color) = if *is_recording {
                    (format!("REC ON (#{})", *record_start_idx + 1), Color::from_rgba(180, 40, 40, 240), WHITE)
                } else {
                    ("REC PAUSED [T]".to_string(), Color::from_rgba(60, 65, 75, 240), LIGHTGRAY)
                };
                draw_rectangle(panel_x, panel_y, panel_w, 36.0, rec_bg);
                draw_rectangle_lines(panel_x, panel_y, panel_w, 36.0, 1.5, WHITE);
                let rec_tw = measure_text(&rec_text, None, 16, 1.0).width;
                draw_text(&rec_text, panel_x + (panel_w - rec_tw) * 0.5, panel_y + 24.0, 16.0, rec_color);

                // Click toggle REC button
                if is_mouse_button_released(MouseButton::Left) && current_mouse_pos.0 >= panel_x && current_mouse_pos.0 <= panel_x + panel_w && current_mouse_pos.1 >= panel_y && current_mouse_pos.1 <= panel_y + 36.0 {
                    *is_recording = !*is_recording;
                    if *is_recording {
                        *record_start_idx = move_history.len();
                        raw_steps.clear();
                        println!("🔴 [RECORD ON] Started recording from Move #{}!", move_history.len());
                    }
                }

                // Tile Queue Preview Box
                let queue_y = panel_y + 46.0;
                let queue_h = panel_h - 46.0;
                draw_rectangle(panel_x, queue_y, panel_w, queue_h, Color::from_rgba(12, 16, 24, 230));
                draw_rectangle_lines(panel_x, queue_y, panel_w, queue_h, 2.0, SKYBLUE);
                draw_text("TILE QUEUE", panel_x + 30.0, queue_y + 25.0, 16.0, SKYBLUE);

                let slot_offsets_y = [85.0, 195.0, 295.0];
                let slot_radii = [38.0, 30.0, 25.0];
                let slot_labels = ["ACTIVE", "NEXT #1", "NEXT #2"];

                for (idx, tile) in env.tile_queue.iter().take(3).enumerate() {
                    let slot_y = queue_y + slot_offsets_y[idx];
                    let slot_center = Vec2::new(panel_x + panel_w * 0.5, slot_y);
                    let rot = if idx == 0 { current_rotation } else { 0 };
                    let cfg = tile.to_hex_edge_config();

                    let label_color = if idx == 0 { SKYBLUE } else { LIGHTGRAY };
                    let label_str = slot_labels[idx];
                    let text_w = measure_text(label_str, None, 13, 1.0).width;
                    draw_text(label_str, slot_center.x - text_w * 0.5, slot_y - slot_radii[idx] - 8.0, 13.0, label_color);

                    let tile_alpha = if idx == 0 { 1.0 } else { 0.85 };
                    draw_hex_tile(slot_center, slot_radii[idx], &cfg, rot, tile_alpha);

                    if idx == 0 {
                        if let GeneratedTile::Quest { quest_data, .. } = tile {
                            draw_badge_text(slot_center, quest_data, slot_radii[idx], tile_alpha);
                        }
                    }

                    let p_code = tile.tile_preset_string();
                    let code_w = measure_text(&p_code, None, 12, 1.0).width;
                    draw_text(&p_code, slot_center.x - code_w * 0.5, slot_y + slot_radii[idx] + 15.0, 12.0, WHITE);
                }

                // ── Bottom Navigation HUD ──
                let bottom_hud = "LMB: Place | Drag: Pan | Hover Slot + Wheel: Rotate | Space/RMB/R: Rotate | U/Z: Undo | Y: Redo | T: Rec | ESC: Finish";
                draw_text(bottom_hud, 20.0, screen_h - 15.0, 14.5, LIGHTGRAY);

                // Handle Place Tile
                let lmb_clicked = is_mouse_button_released(MouseButton::Left) && total_drag_dist <= 6.0 && current_mouse_pos.1 > 40.0 && current_mouse_pos.0 < panel_x;
                if can_place && lmb_clicked {
                    let obs = env.extract_graph_observation();
                    let prev_score = env.score_manager.total_score;
                    let act = Action { q: hovered_hex.q, r: hovered_hex.r, rotation: current_rotation };
                    let res = env.step(act);
                    let gained = env.score_manager.total_score.saturating_sub(prev_score);

                    let place_screen_pos = (hovered_hex.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;

                    // Add Toast
                    if gained > 100 {
                        floating_toasts.push(FloatingToast {
                            text: format!("+{} QUEST COMPLETED!", gained),
                            pos: place_screen_pos + Vec2::new(0.0, -25.0),
                            color: Color::from_rgba(80, 240, 120, 255),
                            timer: 2.2,
                        });
                    } else if gained >= 60 {
                        floating_toasts.push(FloatingToast {
                            text: format!("+{} PERFECT!", gained),
                            pos: place_screen_pos + Vec2::new(0.0, -35.0),
                            color: GOLD,
                            timer: 2.0,
                        });
                    } else if gained > 0 {
                        floating_toasts.push(FloatingToast {
                            text: format!("+{} Fit", gained),
                            pos: place_screen_pos,
                            color: WHITE,
                            timer: 1.5,
                        });
                    }

                    move_history.push(GameMoveRecord {
                        step: move_history.len(),
                        q: act.q,
                        r: act.r,
                        rotation: act.rotation,
                        score_gained: gained,
                        total_score: env.score_manager.total_score,
                        remaining_tiles: env.score_manager.remaining_tiles,
                    });
                    if *is_recording {
                        raw_steps.push((obs, res.reward * 0.01));
                    }

                    println!(
                        "   [HUMAN MOVE #{}] ({}, {}) rot:{} -> +{} pts (Total: {}) | Stack: {} {}",
                        move_history.len(), act.q, act.r, act.rotation, gained, env.score_manager.total_score, env.score_manager.remaining_tiles,
                        if *is_recording { "🔴" } else { "⚪" }
                    );

                    current_rotation = 0;
                }

                // Check Game Over or Manual Exit
                if env.is_game_over() || is_key_pressed(KeyCode::Escape) {
                    println!("\n=======================================================");
                    println!(">>> FINISHED EXPERT GAME: {} POINTS (Placed {} tiles) <<<", env.score_manager.total_score, env.placed_count);
                    println!("Evaluating with HexGNN from Record index #{} into `{}`...", *record_start_idx + 1, max_score_states_path);
                    println!("=======================================================");

                    let total_steps = raw_steps.len();
                    let mut g_vals = vec![0.0f32; total_steps];
                    let mut running_g = 0.0f32;
                    for t in (0..total_steps).rev() {
                        running_g = raw_steps[t].1 + 0.995 * running_g;
                        g_vals[t] = running_g;
                    }

                    let config = AlphaZeroTrainerConfig {
                        lr: 0.0003, gamma: 0.995, value_loss_coeff: 0.5, batch_size: 1024,
                        train_epochs_per_iter: 1, mcts_config: MCTSConfig::default(),
                        num_parallel_envs: 1, target_seed: seed, initial_stack, tile_limit,
                        replay_buffer_capacity: Some(200_000),
                    };
                    let mut pipeline = AlphaZeroPipeline::new(config);
                    pipeline.max_score_states = states_list.clone();

                    let mut qualified = 0usize;
                    for t in 0..total_steps {
                        let real_idx = *record_start_idx + t;
                        if real_idx < move_history.len() {
                            let m = &move_history[real_idx];
                            if m.remaining_tiles >= 10 {
                                let q = m.total_score as f32 + g_vals[t] * 100.0;
                                pipeline.add_high_q_state(q, m.remaining_tiles, &move_history[..=real_idx]);
                                
                                println!(
                                    "   [State Accepted] Move #{:02} | Pos: ({:2}, {:2}) | Score: {:4} | Stack: {:2} | Q-Value: {:6.1} -> QUALIFIED!",
                                    real_idx + 1, m.q, m.r, m.total_score, m.remaining_tiles, q
                                );
                                qualified += 1;
                            } else {
                                println!(
                                    "   [State Skipped]  Move #{:02} | Score: {:4} | Stack: {:2} (< 10 tiles)",
                                    real_idx + 1, m.total_score, m.remaining_tiles
                                );
                            }
                        }
                    }

                    if let Ok(json) = serde_json::to_string_pretty(&pipeline.max_score_states) {
                        let _ = fs::write(max_score_states_path, json);
                        println!("\n✅ SAVED {} MAX-SCORE STATES TO `{}`!", pipeline.max_score_states.len(), max_score_states_path);
                    }

                    next_app_state = Some(AppState::GameOver {
                        final_score: env.score_manager.total_score,
                        final_placed: env.placed_count,
                        evaluated_states_count: total_steps,
                        qualified_states_count: qualified,
                    });
                }
            }

            AppState::GameOver { final_score, final_placed, evaluated_states_count, qualified_states_count } => {
                let box_w = 480.0;
                let box_h = 260.0;
                let box_x = (screen_w - box_w) * 0.5;
                let box_y = (screen_h - box_h) * 0.5;

                draw_rectangle(box_x, box_y, box_w, box_h, Color::from_rgba(16, 22, 32, 245));
                draw_rectangle_lines(box_x, box_y, box_w, box_h, 3.0, GOLD);

                let title = "GAME FINISHED";
                let tw = measure_text(title, None, 30, 1.0).width;
                draw_text(title, (screen_w - tw) * 0.5, box_y + 48.0, 30.0, GOLD);

                let t1 = format!("Final Score: {} pts | Placed: {} tiles", final_score, final_placed);
                let t2 = format!("Evaluated Steps: {}", evaluated_states_count);
                let t3 = format!("Qualified States Injected to MaxScore: {}", qualified_states_count);
                draw_text(&t1, box_x + 35.0, box_y + 95.0, 20.0, WHITE);
                draw_text(&t2, box_x + 35.0, box_y + 128.0, 18.0, LIGHTGRAY);
                draw_text(&t3, box_x + 35.0, box_y + 158.0, 18.0, GREEN);

                let hint = "Press [SPACE] or [ENTER] to return to menu";
                let hw = measure_text(hint, None, 16, 1.0).width;
                draw_text(hint, (screen_w - hw) * 0.5, box_y + 220.0, 16.0, YELLOW);

                if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) {
                    if Path::new(max_score_states_path).exists() {
                        if let Ok(content) = fs::read_to_string(max_score_states_path) {
                            if let Ok(st) = serde_json::from_str::<Vec<MaxScoreStateRecord>>(&content) {
                                states_list = st;
                            }
                        }
                    }
                    next_app_state = Some(AppState::SelectMode);
                }
            }
        }

        if let Some(st) = next_app_state {
            app_state = st;
        }

        last_mouse_pos = current_mouse_pos;
        next_frame().await;
    }
}

/// Helper function to draw a flat-topped Hexagon tile (Exact matching simulator.rs)
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
    game_board: &Board,
    center: Vec2,
    placed_tile: &PlacedTile,
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

/// Helper function to draw badge text (+3, =2)
fn draw_badge_text(center: Vec2, quest_data: &QuestTileData, hex_radius: f32, alpha: f32) {
    draw_custom_badge_text(center, quest_data.primary_group_type(), quest_data.equality, quest_data.remaining_display_value(), hex_radius, alpha);
}

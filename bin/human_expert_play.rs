use macroquad::prelude::*;
use dorfromantik_remake::alphazero::{AlphaZeroPipeline, AlphaZeroTrainerConfig, GameMoveRecord, MaxScoreStateRecord};
use dorfromantik_remake::env::{Action, DorfromantikEnv};
use dorfromantik_remake::mcts::MCTSConfig;
use dorfromantik_remake::nn::HexGNNModel;
use dorfromantik_remake::tile::EdgeType;
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

#[derive(Clone)]
enum AppState {
    SelectMode,
    Playing {
        env: DorfromantikEnv,
        move_history: Vec<GameMoveRecord>,
        raw_steps: Vec<(dorfromantik_remake::env::GraphObservation, f32)>, // obs, reward
        initial_moves_len: usize,
    },
    GameOver {
        final_score: usize,
        final_placed: usize,
        evaluated_states_count: usize,
        qualified_states_count: usize,
    },
}

#[macroquad::main("Human Expert Interactive Trainer - Dorfromantik")]
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
            eprintln!("Lỗi: không đọc được model từ `{}`: {:?}", model_path, e);
            std::process::exit(1);
        }
    };
    println!(">>> Đã nạp model thẩm định từ `{}` (Step count = {}) <<<", model_path, model.step_count);

    let mut states_list: Vec<MaxScoreStateRecord> = Vec::new();
    if Path::new(max_score_states_path).exists() {
        if let Ok(content) = fs::read_to_string(max_score_states_path) {
            if let Ok(st) = serde_json::from_str::<Vec<MaxScoreStateRecord>>(&content) {
                states_list = st;
            }
        }
    }
    println!(">>> Đã nạp {} max-score states từ `{}` <<<", states_list.len(), max_score_states_path);

    let mut app_state = AppState::SelectMode;
    let mut camera_pos = Vec2::ZERO;
    let mut zoom: f32 = 1.0;
    let mut last_mouse_pos = mouse_position();
    let mut total_drag_dist: f32 = 0.0;
    let mut current_rotation: usize = 0;

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

        let mut next_app_state = None;

        match &mut app_state {
            AppState::SelectMode => {
                let box_w = 640.0;
                let box_h = 320.0;
                let bx = screen_w * 0.5 - box_w * 0.5;
                let by = screen_h * 0.5 - box_h * 0.5;

                draw_rectangle(bx, by, box_w, box_h, Color::from_rgba(25, 32, 45, 250));
                draw_poly_lines(screen_w * 0.5, screen_h * 0.5, 4, 300.0, 45.0, 2.0, GOLD);

                draw_text("CHỌN CHẾ ĐỘ CHƠI TAY EXPERT", bx + 80.0, by + 50.0, 30.0, GOLD);

                // Option 1: New Game
                draw_rectangle(bx + 40.0, by + 90.0, box_w - 80.0, 75.0, Color::from_rgba(35, 55, 40, 255));
                draw_text("[1] BẮT ĐẦU VÁN MỚI (Từ Turn 0 / Trống)", bx + 60.0, by + 125.0, 22.0, GREEN);
                draw_text("Xây dựng bàn cờ từ đầu, thử nghiệm các hướng mở cờ mới.", bx + 60.0, by + 150.0, 16.0, LIGHTGRAY);

                // Option 2: Random Max Score State
                let count_str = format!("(Ngẫu nhiên trong {} thế cờ điểm cao có sẵn)", states_list.len());
                draw_rectangle(bx + 40.0, by + 185.0, box_w - 80.0, 75.0, Color::from_rgba(40, 50, 75, 255));
                draw_text("[2] CHƠI TIẾP TỪ THẾ CỜ ĐỈNH CAO", bx + 60.0, by + 220.0, 22.0, SKYBLUE);
                draw_text(&count_str, bx + 60.0, by + 245.0, 16.0, LIGHTGRAY);

                draw_text("Bấm phím [1] hoặc [2] để bắt đầu chơi ngay", bx + 140.0, by + 295.0, 18.0, YELLOW);

                if is_key_pressed(KeyCode::Key1) || is_key_pressed(KeyCode::Kp1) {
                    let env = DorfromantikEnv::new(seed, initial_stack, tile_limit);
                    next_app_state = Some(AppState::Playing {
                        env,
                        move_history: Vec::new(),
                        raw_steps: Vec::new(),
                        initial_moves_len: 0,
                    });
                }

                if (is_key_pressed(KeyCode::Key2) || is_key_pressed(KeyCode::Kp2)) && !states_list.is_empty() {
                    use ::rand::Rng;
                    let mut rng = ::rand::thread_rng();
                    let r_bias = rng.gen::<f32>().powi(2); // Quadratic bias ưu tiên top Q cao nhất
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
                        "\n>>> Đã nạp thành công ngẫu nhiên State #{}/{} (Score: {}, Placed: {} tiles, Stack: {} tiles, Q-Value: {:.1}) <<<",
                        pick_idx + 1, states_list.len(), env.score_manager.total_score, env.placed_count, env.score_manager.remaining_tiles, st.q_value
                    );
                    next_app_state = Some(AppState::Playing {
                        env,
                        move_history: replay_history,
                        raw_steps: Vec::new(),
                        initial_moves_len: st.moves.len(),
                    });
                }
            }

            AppState::Playing { env, move_history, raw_steps, initial_moves_len } => {
                // Controls camera
                if is_mouse_button_down(MouseButton::Right) && total_drag_dist > 4.0 {
                    camera_pos.x -= mouse_delta.x / zoom;
                    camera_pos.y -= mouse_delta.y / zoom;
                }
                let speed = 600.0 / zoom;
                if is_key_down(KeyCode::W) { camera_pos.y -= speed * delta; }
                if is_key_down(KeyCode::S) { camera_pos.y += speed * delta; }
                if is_key_down(KeyCode::A) { camera_pos.x -= speed * delta; }
                if is_key_down(KeyCode::D) { camera_pos.x += speed * delta; }

                let wheel = mouse_wheel().1;
                if is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl) {
                    if wheel > 0.0 { zoom *= 1.12; }
                    if wheel < 0.0 { zoom *= 0.88; }
                } else if wheel != 0.0 {
                    if wheel < 0.0 { current_rotation = (current_rotation + 1) % 6; }
                    else { current_rotation = (current_rotation + 5) % 6; }
                }
                if is_key_pressed(KeyCode::R) || is_key_pressed(KeyCode::Space) {
                    current_rotation = (current_rotation + 1) % 6;
                }
                zoom = zoom.clamp(0.2, 4.0);

                let center_vec = Vec2::new(screen_w * 0.5, screen_h * 0.5);
                let mouse_vec = Vec2::new(current_mouse_pos.0, current_mouse_pos.1);
                let mouse_world = (mouse_vec - center_vec) / zoom + camera_pos;
                let hovered_hex = screen_to_hex(mouse_world, HEX_RADIUS);

                // Draw Board
                for (&pos, pt) in &env.board.placed_tiles {
                    let hp = HexPos::new(pos.0, pos.1);
                    let sp = (hp.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;
                    draw_poly(sp.x, sp.y, 6, HEX_RADIUS * zoom, 30.0, Color::from_rgba(40, 50, 65, 255));
                    
                    // 6 Edges
                    for dir in 0..6 {
                        let e_color = get_edge_color(pt.edge_config.edges[dir]);
                        let a1 = (dir as f32 * 60.0 - 30.0).to_radians();
                        let a2 = (dir as f32 * 60.0 + 30.0).to_radians();
                        let p1 = sp + Vec2::new(a1.cos(), a1.sin()) * HEX_RADIUS * zoom;
                        let p2 = sp + Vec2::new(a2.cos(), a2.sin()) * HEX_RADIUS * zoom;
                        draw_line(p1.x, p1.y, p2.x, p2.y, 4.0 * zoom, e_color);
                    }

                    // Quest label
                    if let dorfromantik_remake::tile::GeneratedTile::Quest { quest_data, .. } = &pt.tile {
                        let type_name = match quest_data.primary_group_type() {
                            dorfromantik_remake::game_config::GroupType::Forest => "Forest",
                            dorfromantik_remake::game_config::GroupType::Village => "House",
                            dorfromantik_remake::game_config::GroupType::Agriculture => "Field",
                            dorfromantik_remake::game_config::GroupType::Water => "Water",
                            dorfromantik_remake::game_config::GroupType::TrainTracks => "Train",
                        };
                        let txt = format!("{}:{}", type_name, quest_data.remaining_display_value());
                        let color = if pt.quest_finalized { GRAY } else { GOLD };
                        draw_text(&txt, sp.x - 22.0 * zoom, sp.y + 5.0 * zoom, 15.0 * zoom, color);
                    }
                }

                // Draw Hover candidate
                let active_tile_opt = env.current_tile();
                let can_place = if let Some(curr) = active_tile_opt {
                    env.board.can_place_tile(hovered_hex.q, hovered_hex.r, curr, current_rotation)
                } else {
                    false
                };

                if let Some(curr) = active_tile_opt {
                    let hp = hovered_hex;
                    let sp = (hp.to_screen(HEX_RADIUS) - camera_pos) * zoom + center_vec;
                    let mut cfg = curr.to_hex_edge_config();
                    cfg.rotate(current_rotation);
                    let outline_color = if can_place { GREEN } else { RED };
                    draw_poly_lines(sp.x, sp.y, 6, HEX_RADIUS * zoom, 30.0, 2.5 * zoom, outline_color);

                    for dir in 0..6 {
                        let e_color = get_edge_color(cfg.edges[dir]);
                        let a1 = (dir as f32 * 60.0 - 30.0).to_radians();
                        let a2 = (dir as f32 * 60.0 + 30.0).to_radians();
                        let p1 = sp + Vec2::new(a1.cos(), a1.sin()) * HEX_RADIUS * zoom * 0.9;
                        let p2 = sp + Vec2::new(a2.cos(), a2.sin()) * HEX_RADIUS * zoom * 0.9;
                        draw_line(p1.x, p1.y, p2.x, p2.y, 3.5 * zoom, e_color);
                    }
                }

                // HUD Info
                let header = format!(
                    "HUMAN PLAY | Score: {} | Placed: {}/{} | Stack: {} tiles",
                    env.score_manager.total_score, env.placed_count, tile_limit, env.score_manager.remaining_tiles
                );
                draw_rectangle(0.0, 0.0, screen_w, 40.0, Color::from_rgba(15, 20, 28, 220));
                draw_text(&header, 20.0, 28.0, 22.0, GOLD);
                draw_text("Chuột Trái: Đặt tile | Chuột Phải / Space: Xoay | Giữ Chuột Phải: Kéo map | [ESC]: Kết thúc & Thẩm định", 20.0, screen_h - 15.0, 16.0, LIGHTGRAY);

                // Handle Place Tile
                let lmb_clicked = is_mouse_button_released(MouseButton::Left) && total_drag_dist <= 6.0;
                if can_place && lmb_clicked {
                    let obs = env.extract_graph_observation();
                    let prev_score = env.score_manager.total_score;
                    let act = Action { q: hovered_hex.q, r: hovered_hex.r, rotation: current_rotation };
                    let res = env.step(act);
                    let gained = env.score_manager.total_score.saturating_sub(prev_score);

                    move_history.push(GameMoveRecord {
                        step: move_history.len(),
                        q: act.q,
                        r: act.r,
                        rotation: act.rotation,
                        score_gained: gained,
                        total_score: env.score_manager.total_score,
                        remaining_tiles: env.score_manager.remaining_tiles,
                    });
                    raw_steps.push((obs, res.reward * 0.01));

                    println!(
                        "   [HUMAN MOVE #{}] ({}, {}) rot:{} -> +{} pts (Total: {}) | Stack: {}",
                        move_history.len(), act.q, act.r, act.rotation, gained, env.score_manager.total_score, env.score_manager.remaining_tiles
                    );

                    current_rotation = 0;
                }

                // Check Game Over or Manual Exit
                if env.is_game_over() || is_key_pressed(KeyCode::Escape) {
                    println!("\n=======================================================");
                    println!(">>> KẾT THÚC VÁN ĐẤU CON NGƯỜI: {} ĐIỂM (Placed {} tiles) <<<", env.score_manager.total_score, env.placed_count);
                    println!("Đang dùng mạng GNN thẩm định và cập nhật vào `{}`...", max_score_states_path);
                    println!("=======================================================");

                    // Compute Discounted G for human steps
                    let total_steps = raw_steps.len();
                    let mut g_vals = vec![0.0f32; total_steps];
                    let mut running_g = 0.0f32;
                    for t in (0..total_steps).rev() {
                        running_g = raw_steps[t].1 + 0.995 * running_g;
                        g_vals[t] = running_g;
                    }

                    // Tải pipeline config tạm để dùng add_high_q_state
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
                        let real_idx = *initial_moves_len + t;
                        if real_idx < move_history.len() {
                            let m = &move_history[real_idx];
                            if m.remaining_tiles >= 10 {
                                let q = m.total_score as f32 + g_vals[t] * 100.0;
                                let old_len = pipeline.max_score_states.len();
                                pipeline.add_high_q_state(q, m.remaining_tiles, &move_history[..=real_idx]);
                                
                                println!(
                                    "   [State Duyệt] Move #{:02} | Pos: ({:2}, {:2}) | Score: {:4} | Stack: {:2} | Q-Value: {:6.1} -> DUYỆT THÀNH CÔNG!",
                                    real_idx + 1, m.q, m.r, m.total_score, m.remaining_tiles, q
                                );
                                qualified += 1;
                            } else {
                                println!(
                                    "   [State Bỏ qua] Move #{:02} | Score: {:4} | Stack: {:2} (< 10 tiles)",
                                    real_idx + 1, m.total_score, m.remaining_tiles
                                );
                            }
                        }
                    }

                    // Lưu lại max_score_states.json
                    if let Ok(json) = serde_json::to_string_pretty(&pipeline.max_score_states) {
                        let _ = fs::write(max_score_states_path, json);
                        println!("\n✅ ĐÃ CẬP NHẬT VÀ LƯU {} MAX-SCORE STATES VÀO `{}`!", pipeline.max_score_states.len(), max_score_states_path);
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
                draw_rectangle(screen_w * 0.5 - 250.0, screen_h * 0.5 - 150.0, 500.0, 300.0, Color::from_rgba(25, 32, 45, 250));
                draw_poly_lines(screen_w * 0.5, screen_h * 0.5, 4, 250.0, 45.0, 2.0, GOLD);

                draw_text("VÁN ĐẤU HOÀN THÀNH!", screen_w * 0.5 - 160.0, screen_h * 0.5 - 90.0, 30.0, GOLD);
                let t1 = format!("Tổng điểm: {} | Placed: {} tiles", final_score, final_placed);
                let t2 = format!("Đã thẩm định: {} nước đi", evaluated_states_count);
                let t3 = format!("Số States đạt chuẩn Q và nạp vào MaxScore: {}", qualified_states_count);
                draw_text(&t1, screen_w * 0.5 - 200.0, screen_h * 0.5 - 40.0, 22.0, WHITE);
                draw_text(&t2, screen_w * 0.5 - 200.0, screen_h * 0.5 - 10.0, 20.0, LIGHTGRAY);
                draw_text(&t3, screen_w * 0.5 - 200.0, screen_h * 0.5 + 20.0, 20.0, GREEN);

                draw_text("Bấm [SPACE] hoặc [ENTER] để quay lại màn hình chọn", screen_w * 0.5 - 220.0, screen_h * 0.5 + 90.0, 18.0, YELLOW);

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

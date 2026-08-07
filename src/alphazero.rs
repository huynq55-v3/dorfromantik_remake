use rand::prelude::*;
use rayon::prelude::*;
use std::collections::VecDeque;

use crate::env::{DorfromantikEnv, GraphObservation};
use crate::mcts::{MCTSConfig, MCTSSearch};
use crate::nn::HexGNNModel;

/// Mẫu dữ liệu huấn luyện AlphaZero (State Observation, Target Policy Distribution từ MCTS, Target Value từ Game Return)
#[derive(Debug, Clone)]
pub struct AlphaZeroSample {
    pub obs: GraphObservation,
    pub target_pi: Vec<f32>,
    pub target_val: f32,
}

/// Bộ nhớ đệm Replay Buffer lưu trữ các bước đi tự chơi của MCTS
pub struct AlphaZeroReplayBuffer {
    pub capacity: usize,
    pub buffer: VecDeque<AlphaZeroSample>,
}

impl AlphaZeroReplayBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            buffer: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, sample: AlphaZeroSample) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(sample);
    }

    pub fn push_batch(&mut self, samples: Vec<AlphaZeroSample>) {
        for s in samples {
            self.push(s);
        }
    }

    pub fn sample_batch(&self, batch_size: usize) -> Vec<AlphaZeroSample> {
        let mut rng = rand::thread_rng();
        let len = self.buffer.len();
        if len == 0 {
            return Vec::new();
        }
        (0..batch_size)
            .map(|_| {
                let idx = rng.gen_range(0..len);
                self.buffer[idx].clone()
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        use std::io::Write;
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);

        writer.write_all(b"DORF_BUF_V1")?;
        writer.write_all(&(self.buffer.len() as u64).to_le_bytes())?;

        for sample in &self.buffer {
            // 1. node_positions & node_features
            let n_nodes = sample.obs.node_positions.len();
            writer.write_all(&(n_nodes as u64).to_le_bytes())?;
            for &(q, r) in &sample.obs.node_positions {
                writer.write_all(&q.to_le_bytes())?;
                writer.write_all(&r.to_le_bytes())?;
            }
            for feat in &sample.obs.node_features {
                for &val in feat {
                    writer.write_all(&val.to_le_bytes())?;
                }
            }

            // 2. edge_index
            let n_edges = sample.obs.edge_index.len();
            writer.write_all(&(n_edges as u64).to_le_bytes())?;
            for &(u, v) in &sample.obs.edge_index {
                writer.write_all(&(u as u64).to_le_bytes())?;
                writer.write_all(&(v as u64).to_le_bytes())?;
            }

            // 3. valid_actions & action_features
            let n_actions = sample.obs.valid_actions.len();
            writer.write_all(&(n_actions as u64).to_le_bytes())?;
            for act in &sample.obs.valid_actions {
                writer.write_all(&act.q.to_le_bytes())?;
                writer.write_all(&act.r.to_le_bytes())?;
                writer.write_all(&(act.rotation as u64).to_le_bytes())?;
            }
            for feat in &sample.obs.action_features {
                for &val in feat {
                    writer.write_all(&val.to_le_bytes())?;
                }
            }

            // 4. target_pi
            let pi_len = sample.target_pi.len();
            writer.write_all(&(pi_len as u64).to_le_bytes())?;
            for &p in &sample.target_pi {
                writer.write_all(&p.to_le_bytes())?;
            }

            // 5. target_val
            writer.write_all(&sample.target_val.to_le_bytes())?;
        }

        writer.flush()?;
        Ok(())
    }

    pub fn load_from_file(&mut self, path: &str) -> std::io::Result<usize> {
        use std::io::Read;
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);

        let mut magic = [0u8; 11];
        reader.read_exact(&mut magic)?;
        if &magic != b"DORF_BUF_V1" {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid buffer format"));
        }

        let mut buf8 = [0u8; 8];
        let mut buf4 = [0u8; 4];

        reader.read_exact(&mut buf8)?;
        let num_samples = u64::from_le_bytes(buf8) as usize;

        for _ in 0..num_samples {
            // 1. node_positions & node_features
            reader.read_exact(&mut buf8)?;
            let n_nodes = u64::from_le_bytes(buf8) as usize;

            let mut node_positions = Vec::with_capacity(n_nodes);
            for _ in 0..n_nodes {
                reader.read_exact(&mut buf4)?;
                let q = i32::from_le_bytes(buf4);
                reader.read_exact(&mut buf4)?;
                let r = i32::from_le_bytes(buf4);
                node_positions.push((q, r));
            }

            let mut node_features = Vec::with_capacity(n_nodes);
            for _ in 0..n_nodes {
                let mut feat = [0.0f32; 40];
                for val in feat.iter_mut() {
                    reader.read_exact(&mut buf4)?;
                    *val = f32::from_le_bytes(buf4);
                }
                node_features.push(feat);
            }

            // 2. edge_index
            reader.read_exact(&mut buf8)?;
            let n_edges = u64::from_le_bytes(buf8) as usize;
            let mut edge_index = Vec::with_capacity(n_edges);
            for _ in 0..n_edges {
                reader.read_exact(&mut buf8)?;
                let u = u64::from_le_bytes(buf8) as usize;
                reader.read_exact(&mut buf8)?;
                let v = u64::from_le_bytes(buf8) as usize;
                edge_index.push((u, v));
            }

            // 3. valid_actions & action_features
            reader.read_exact(&mut buf8)?;
            let n_actions = u64::from_le_bytes(buf8) as usize;
            let mut valid_actions = Vec::with_capacity(n_actions);
            for _ in 0..n_actions {
                reader.read_exact(&mut buf4)?;
                let q = i32::from_le_bytes(buf4);
                reader.read_exact(&mut buf4)?;
                let r = i32::from_le_bytes(buf4);
                reader.read_exact(&mut buf8)?;
                let rotation = u64::from_le_bytes(buf8) as usize;
                valid_actions.push(crate::env::Action { q, r, rotation });
            }

            let mut action_features = Vec::with_capacity(n_actions);
            for _ in 0..n_actions {
                let mut feat = [0.0f32; 16];
                for val in feat.iter_mut() {
                    reader.read_exact(&mut buf4)?;
                    *val = f32::from_le_bytes(buf4);
                }
                action_features.push(feat);
            }

            // 4. target_pi
            reader.read_exact(&mut buf8)?;
            let pi_len = u64::from_le_bytes(buf8) as usize;
            let mut target_pi = Vec::with_capacity(pi_len);
            for _ in 0..pi_len {
                reader.read_exact(&mut buf4)?;
                target_pi.push(f32::from_le_bytes(buf4));
            }

            // 5. target_val
            reader.read_exact(&mut buf4)?;
            let target_val = f32::from_le_bytes(buf4);

            let obs = GraphObservation {
                node_positions,
                node_features,
                edge_index,
                valid_actions,
                action_features,
            };

            self.push(AlphaZeroSample {
                obs,
                target_pi,
                target_val,
            });
        }

        Ok(self.len())
    }
}

/// Cấu hình huấn luyện AlphaZero / Expert Iteration
#[derive(Debug, Clone)]
pub struct AlphaZeroTrainerConfig {
    pub lr: f32,
    pub gamma: f32,
    pub value_loss_coeff: f32,
    pub batch_size: usize,
    pub train_epochs_per_iter: usize,
    pub mcts_config: MCTSConfig,
    pub temp_threshold_moves: usize,
    pub num_parallel_envs: usize,
    pub target_seed: i32,
    pub tile_limit: usize,
}

impl Default for AlphaZeroTrainerConfig {
    fn default() -> Self {
        Self {
            lr: 0.0003,
            gamma: 0.99,
            value_loss_coeff: 0.5,
            batch_size: 128,
            train_epochs_per_iter: 4,
            mcts_config: MCTSConfig {
                c_puct: 1.5,
                gamma: 0.99,
                n_simulations: 200,
                dirichlet_alpha: 0.3,
                dirichlet_eps: 0.25,
            },
            temp_threshold_moves: 12,
            num_parallel_envs: 16,
            target_seed: -2093096630,
            tile_limit: 100,
        }
    }
}

/// Bản ghi chi tiết từng nước đi trong ván chơi (dùng để lưu kỷ lục và replay)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameMoveRecord {
    pub step: usize,
    pub q: i32,
    pub r: i32,
    pub rotation: usize,
    pub score_gained: usize,
    pub total_score: usize,
    pub remaining_tiles: usize,
}

/// Bản ghi toàn bộ thông tin của 1 ván chơi (Match Record)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameMatchRecord {
    pub seed: i32,
    pub total_score: usize,
    pub total_placed: usize,
    pub is_eval: bool,
    pub moves: Vec<GameMoveRecord>,
}

/// Thu thập dữ liệu 1 ván tự chơi (Self-Play Episode) sử dụng MCTS (200 simulations)
pub fn run_self_play_episode(
    seed: i32,
    tile_limit: usize,
    model: &HexGNNModel,
    mcts_config: &MCTSConfig,
    temp_threshold: usize,
) -> (Vec<AlphaZeroSample>, GameMatchRecord) {
    let mut env = DorfromantikEnv::new(seed, 10, tile_limit);
    let mcts = MCTSSearch::new(mcts_config.clone());

    let mut raw_steps: Vec<(GraphObservation, Vec<f32>, f32)> = Vec::new();
    let mut move_records: Vec<GameMoveRecord> = Vec::new();
    let mut move_count = 0;

    loop {
        let obs = env.extract_graph_observation();
        if obs.valid_actions.is_empty() {
            break;
        }

        // Trong các nước đi đầu: nhiệt độ tau = 1.0 + dirichlet noise để khám phá chiến thuật đa dạng
        // Sau đó: nhiệt độ tau = 0.2 để tập trung vào các nhánh xuất sắc nhất
        let (temperature, add_dirichlet) = if move_count < temp_threshold {
            (1.0f32, true)
        } else {
            (0.2f32, false)
        };

        let (pi_probs, _, chosen_action, _) = mcts.search(&env, model, add_dirichlet, temperature);
        let prev_score = env.score_manager.total_score;
        let res = env.step(chosen_action);
        let score_gained = env.score_manager.total_score.saturating_sub(prev_score);
        let scaled_r = res.reward * 0.05;

        move_records.push(GameMoveRecord {
            step: move_count,
            q: chosen_action.q,
            r: chosen_action.r,
            rotation: chosen_action.rotation,
            score_gained,
            total_score: env.score_manager.total_score,
            remaining_tiles: env.score_manager.remaining_tiles,
        });

        raw_steps.push((obs, pi_probs, scaled_r));
        move_count += 1;

        if res.done {
            break;
        }
    }

    let final_score = env.score_manager.total_score;
    let placed_count = env.placed_count;

    // Tính toán Discounted Return $z_t = \sum_{k} \gamma^k r_{t+k+1}$ cho từng bước
    let total_steps = raw_steps.len();
    let mut samples = Vec::with_capacity(total_steps);
    let mut g = 0.0f32;

    for t in (0..total_steps).rev() {
        let (obs, pi, r) = raw_steps[t].clone();
        g = r + mcts_config.gamma * g;
        samples.push(AlphaZeroSample {
            obs,
            target_pi: pi,
            target_val: g,
        });
    }

    samples.reverse();
    let record = GameMatchRecord {
        seed,
        total_score: final_score,
        total_placed: placed_count,
        is_eval: false,
        moves: move_records,
    };
    (samples, record)
}

/// Đánh giá sức mạnh của Model hiện tại với MCTS Greedy (Nhiệt độ = 0.0) trên Seed mục tiêu
pub fn evaluate_alphazero_agent(
    seed: i32,
    tile_limit: usize,
    model: &HexGNNModel,
    mcts_config: &MCTSConfig,
) -> (usize, usize, GameMatchRecord) {
    let mut env = DorfromantikEnv::new(seed, 10, tile_limit);
    let mcts = MCTSSearch::new(mcts_config.clone());
    let mut move_records: Vec<GameMoveRecord> = Vec::new();
    let mut move_count = 0;

    while !env.is_game_over() {
        let obs = env.extract_graph_observation();
        if obs.valid_actions.is_empty() {
            break;
        }
        // Đánh giá thuần túy: temperature = 0.0 (chọn max visit count), không dirichlet noise
        let (_, _, chosen_action, _) = mcts.search(&env, model, false, 0.0);
        let prev_score = env.score_manager.total_score;
        let res = env.step(chosen_action);
        let score_gained = env.score_manager.total_score.saturating_sub(prev_score);

        move_records.push(GameMoveRecord {
            step: move_count,
            q: chosen_action.q,
            r: chosen_action.r,
            rotation: chosen_action.rotation,
            score_gained,
            total_score: env.score_manager.total_score,
            remaining_tiles: env.score_manager.remaining_tiles,
        });
        move_count += 1;

        if res.done {
            break;
        }
    }

    let record = GameMatchRecord {
        seed,
        total_score: env.score_manager.total_score,
        total_placed: env.placed_count,
        is_eval: true,
        moves: move_records,
    };

    (env.score_manager.total_score, env.placed_count, record)
}

/// Pipeline Huấn luyện AlphaZero / Expert Iteration
pub struct AlphaZeroPipeline {
    pub config: AlphaZeroTrainerConfig,
    pub model: HexGNNModel,
    pub replay_buffer: AlphaZeroReplayBuffer,
}

impl AlphaZeroPipeline {
    pub fn new(config: AlphaZeroTrainerConfig) -> Self {
        let model = HexGNNModel::new();
        let replay_buffer = AlphaZeroReplayBuffer::new(50_000);
        Self {
            config,
            model,
            replay_buffer,
        }
    }

    /// Thu thập dữ liệu tự chơi qua Rayon đa luồng (Parallel Self-Play)
    pub fn collect_self_play_data(&mut self) -> (f32, usize, usize, Option<GameMatchRecord>) {
        let n_envs = self.config.num_parallel_envs;
        let base_seed = self.config.target_seed;
        let tile_limit = self.config.tile_limit;
        let mcts_cfg = self.config.mcts_config.clone();
        let temp_thresh = self.config.temp_threshold_moves;
        let model_ref = &self.model;

        // Sinh danh sách seeds ngẫu nhiên xoay quanh target_seed để mô hình học tổng quát
        let seeds: Vec<i32> = (0..n_envs)
            .map(|i| {
                if i == 0 {
                    base_seed
                } else {
                    base_seed.wrapping_add((i as i32) * 98765 + 13)
                }
            })
            .collect();

        // Chạy đa luồng song song các ván đấu MCTS
        let results: Vec<(Vec<AlphaZeroSample>, GameMatchRecord)> = seeds
            .into_par_iter()
            .map(|s| run_self_play_episode(s, tile_limit, model_ref, &mcts_cfg, temp_thresh))
            .collect();

        let mut total_score = 0;
        let mut max_score = 0;
        let mut total_placed = 0;
        let mut best_record: Option<GameMatchRecord> = None;

        for (samples, record) in results {
            let score = record.total_score;
            let placed = record.total_placed;
            total_score += score;
            if score >= max_score {
                max_score = score;
                best_record = Some(record);
            }
            total_placed += placed;
            self.replay_buffer.push_batch(samples);
        }

        let avg_score = total_score as f32 / n_envs as f32;
        let avg_placed = total_placed / n_envs;
        (avg_score, max_score, avg_placed, best_record)
    }

    /// Huấn luyện mạng GNN trên mini-batches từ Replay Buffer bằng Adam Optimizer
    pub fn train_step(&mut self) -> (f32, f32, f32) {
        let buf_len = self.replay_buffer.len();
        if buf_len < self.config.batch_size {
            return (0.0, 0.0, 0.0);
        }

        let num_batches = (buf_len / self.config.batch_size).clamp(4, 32);
        let mut total_policy_loss = 0.0f32;
        let mut total_value_loss = 0.0f32;
        let mut step_count = 0;

        for _ in 0..self.config.train_epochs_per_iter {
            for _ in 0..num_batches {
                let batch = self.replay_buffer.sample_batch(self.config.batch_size);
                if batch.is_empty() {
                    continue;
                }

                let model_ref = &self.model;
                let val_coeff = self.config.value_loss_coeff;

                let (mb_grads, (mb_pi_loss, mb_val_loss)) = batch
                    .into_par_iter()
                    .map(|sample| {
                        let obs = &sample.obs;
                        let (action_logits, pred_val) = model_ref.forward(
                            &obs.node_positions,
                            &obs.node_features,
                            &obs.edge_index,
                            &obs.valid_actions,
                            &obs.action_features,
                        );

                        if action_logits.is_empty() || sample.target_pi.is_empty() {
                            return (HexGNNModel::new_zero(), (0.0f32, 0.0f32));
                        }

                        // 1. Policy Loss: Cross Entropy -sum(pi_target * ln(p_model))
                        let max_l = action_logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                        let exps: Vec<f32> = action_logits.iter().map(|l| (l - max_l).exp()).collect();
                        let sum_exp: f32 = exps.iter().sum::<f32>().max(1e-8);
                        let probs: Vec<f32> = exps.iter().map(|e| e / sum_exp).collect();

                        let mut sample_pi_loss = 0.0f32;
                        for (i, &t_p) in sample.target_pi.iter().enumerate() {
                            if t_p > 1e-8 && i < probs.len() {
                                sample_pi_loss -= t_p * probs[i].max(1e-8).ln();
                            }
                        }

                        // 2. Value Loss: Huber Loss giữa Predicted Value và Target Game Return
                        let val_err = pred_val - sample.target_val;
                        let sample_val_loss = if val_err.abs() <= 1.0 {
                            0.5 * val_err * val_err
                        } else {
                            val_err.abs() - 0.5
                        };
                        let val_grad = val_err.clamp(-1.0, 1.0);

                        let mut local_grads = HexGNNModel::new_zero();
                        model_ref.backward_accumulate_alphazero(
                            &obs.node_positions,
                            &obs.node_features,
                            &obs.edge_index,
                            &obs.valid_actions,
                            &obs.action_features,
                            &sample.target_pi,
                            val_grad * val_coeff,
                            &mut local_grads,
                        );

                        (local_grads, (sample_pi_loss, sample_val_loss))
                    })
                    .reduce(
                        || (HexGNNModel::new_zero(), (0.0f32, 0.0f32)),
                        |(mut g1, (pi1, v1)), (g2, (pi2, v2))| {
                            g1.add_assign(&g2);
                            (g1, (pi1 + pi2, v1 + v2))
                        },
                    );

                let mb_len = self.config.batch_size as f32;
                let mut scaled_grads = mb_grads;
                scaled_grads.scale_assign(1.0 / mb_len);
                scaled_grads.clip_grad_norm(1.0);

                // Cập nhật trọng số mạng bằng Adam Optimizer
                self.model.update_weights_adam(&scaled_grads, self.config.lr);

                total_policy_loss += mb_pi_loss / mb_len;
                total_value_loss += mb_val_loss / mb_len;
                step_count += 1;
            }
        }

        let avg_pi_loss = if step_count > 0 { total_policy_loss / step_count as f32 } else { 0.0 };
        let avg_val_loss = if step_count > 0 { total_value_loss / step_count as f32 } else { 0.0 };
        let total_loss = avg_pi_loss + self.config.value_loss_coeff * avg_val_loss;

        (total_loss, avg_pi_loss, avg_val_loss)
    }

    pub fn buffer_len(&self) -> usize {
        self.replay_buffer.len()
    }

    pub fn save_checkpoint(&self, model_path: &str, buffer_path: &str) -> std::io::Result<()> {
        self.model.save_to_file(model_path)?;
        self.replay_buffer.save_to_file(buffer_path)?;
        Ok(())
    }

    pub fn load_checkpoint(&mut self, model_path: &str, buffer_path: &str) -> std::io::Result<()> {
        if std::path::Path::new(model_path).exists() {
            self.model = HexGNNModel::load_from_file(model_path)?;
        }
        if std::path::Path::new(buffer_path).exists() {
            let _ = self.replay_buffer.load_from_file(buffer_path)?;
        }
        Ok(())
    }
}

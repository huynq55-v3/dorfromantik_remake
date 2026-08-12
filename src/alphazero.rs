use rand::prelude::*;
use rayon::prelude::*;
use std::collections::VecDeque;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use crate::env::{DorfromantikEnv, GraphObservation};
use crate::mcts::{MCTSConfig, MCTSSearch};
use crate::nn::HexGNNModel;
use crate::gpu_nn::GpuNNExecutor;

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

    /// Thêm một batch sample vào buffer, lọc trùng (không thêm sample trùng HOÀN TOÀN:
    /// cùng state + cùng target_pi + cùng target_val). Hai sample cùng board nhưng khác
    /// target sẽ được thêm riêng biệt (đúng tinh thần AlphaZero).
    /// Trả về số sample MỚI thực sự được giữ lại trong batch này (trừ các sample bị trùng).
    /// Số này là lượng train cho iteration tương ứng.
    pub fn push_batch(&mut self, samples: Vec<AlphaZeroSample>) -> usize {
        // Tập chữ ký của các sample hiện có trong buffer (để loại trùng khi thêm mới)
        let mut seen: HashSet<u64> = HashSet::with_capacity(self.buffer.len() + samples.len());
        for s in &self.buffer {
            seen.insert(sample_full_hash(s));
        }

        let mut retained = 0usize;
        for s in samples {
            let sig = sample_full_hash(&s);
            if seen.insert(sig) {
                self.push(s);
                retained += 1;
            }
            // Nếu trùng chữ ký => bỏ qua, không thêm
        }
        retained
    }

    pub fn sample_batch_indices(&self, batch_size: usize) -> Vec<usize> {
        let mut rng = rand::thread_rng();
        let len = self.buffer.len();
        if len == 0 {
            return Vec::new();
        }
        (0..batch_size)
            .map(|_| rng.gen_range(0..len))
            .collect()
    }

    /// Lấy `count` indices ngẫu nhiên KHÔNG TRÙNG từ buffer và trộn ngẫu nhiên (subset để train).
    /// Dùng Fisher-Yates partial để tránh allocate / shuffle toàn bộ buffer mỗi lần.
    pub fn sample_unique_indices(&self, count: usize) -> Vec<usize> {
        let len = self.buffer.len();
        let count = count.min(len);
        if count == 0 {
            return Vec::new();
        }
        let mut rng = rand::thread_rng();
        // Khởi tạo danh sách index 0..len và thực hiện Fisher-Yates partial (chỉ count phần đầu)
        let mut indices: Vec<usize> = (0..len).collect();
        for i in 0..count {
            let j = i + rng.gen_range(0..(len - i));
            indices.swap(i, j);
        }
        indices.truncate(count);
        indices
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

    /// Lọc bỏ các sample trùng lặp HOÀN TOÀN (cùng state + target_pi + target_val).
    /// Giữ lại sample đầu tiên cho mỗi chữ ký duy nhất, bỏ các bản sao còn lại.
    /// Các sample cùng board nhưng khác target giữ nguyên (không bị dedup).
    /// Được gọi ngay trước khi train để tránh model overfit vào những bản sao y hệt.
    pub fn deduplicate(&mut self) -> usize {
        let mut seen: HashSet<u64> = HashSet::with_capacity(self.buffer.len());
        let mut unique: VecDeque<AlphaZeroSample> = VecDeque::with_capacity(self.buffer.len());
        let mut removed = 0usize;

        for sample in self.buffer.drain(..) {
            let sig = sample_full_hash(&sample);
            if seen.insert(sig) {
                unique.push_back(sample);
            } else {
                removed += 1;
            }
        }
        self.buffer = unique;
        removed
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
                let mut feat = [0.0f32; 70];
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
    pub initial_stack: usize,
    pub tile_limit: usize,
    /// Dung lượng Replay Buffer (số sample). None => tự động tính theo công thức mặc định.
    pub replay_buffer_capacity: Option<usize>,
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
                dirichlet_alpha: 0.5,
                dirichlet_eps: 0.4,
            },
            temp_threshold_moves: 12,
            num_parallel_envs: 16,
            target_seed: -2093096630,
            initial_stack: 10,
            tile_limit: 100,
            replay_buffer_capacity: None,
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

/// Bản ghi board state đạt điểm cao nhất tại 1 depth (placed_count).
/// Dùng để khởi động lại 80% envs từ vị thế tốt thay vì từ bàn trống.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MaxScoreStateRecord {
    pub depth: usize,
    /// Điểm cao nhất đạt được tại depth này.
    pub best_score: usize,
    /// Tất cả các cấu hình game cùng đạt best_score (mỗi cấu hình là 1 chuỗi moves để replay).
    pub states: Vec<GameStateRecord>,
}

/// Một cấu hình board state cụ thể (score tại depth đó + moves để tái hiện).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameStateRecord {
    pub score: usize,
    /// Các actions dùng để đạt board state này (replay deterministic theo seed).
    pub moves: Vec<GameMoveRecord>,
}

/// Thu thập dữ liệu 1 ván tự chơi (Self-Play Episode) sử dụng MCTS (200 simulations)
pub fn run_self_play_episode(
    seed: i32,
    initial_stack: usize,
    tile_limit: usize,
    model: &HexGNNModel,
    mcts_config: &MCTSConfig,
    temp_threshold: usize,
) -> (Vec<AlphaZeroSample>, GameMatchRecord) {
    let mut env = DorfromantikEnv::new(seed, initial_stack, tile_limit);
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
        let scaled_r = res.reward * 0.01;

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

/// Thu thập dữ liệu 1 ván tự chơi GPU (Self-Play Episode) qua GpuEvalQueue
pub fn run_self_play_episode_gpu(
    seed: i32,
    initial_stack: usize,
    tile_limit: usize,
    eval_tx: &crossbeam_channel::Sender<crate::gpu_engine::GpuEvalRequest>,
    mcts_config: &MCTSConfig,
    temp_threshold: usize,
) -> (Vec<AlphaZeroSample>, GameMatchRecord) {
    let mut env = DorfromantikEnv::new(seed, initial_stack, tile_limit);
    let mcts = MCTSSearch::new(mcts_config.clone());

    let mut raw_steps: Vec<(GraphObservation, Vec<f32>, f32)> = Vec::new();
    let mut move_records: Vec<GameMoveRecord> = Vec::new();
    let mut move_count = 0;

    loop {
        let obs = env.extract_graph_observation();
        if obs.valid_actions.is_empty() {
            break;
        }

        let (temperature, add_dirichlet) = if move_count < temp_threshold {
            (1.0f32, true)
        } else {
            (0.2f32, false)
        };

        let (pi_probs, _, chosen_action, _) = mcts.search_gpu(&env, eval_tx, add_dirichlet, temperature);
        let prev_score = env.score_manager.total_score;
        let res = env.step(chosen_action);
        let score_gained = env.score_manager.total_score.saturating_sub(prev_score);
        let scaled_r = res.reward * 0.01;

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

/// Đánh giá sức mạnh của Model hiện tại với MCTS Greedy trên GPU
pub fn evaluate_alphazero_agent_gpu(
    seed: i32,
    initial_stack: usize,
    tile_limit: usize,
    eval_tx: &crossbeam_channel::Sender<crate::gpu_engine::GpuEvalRequest>,
    mcts_config: &MCTSConfig,
) -> (usize, usize, GameMatchRecord) {
    let mut env = DorfromantikEnv::new(seed, initial_stack, tile_limit);
    let mcts = MCTSSearch::new(mcts_config.clone());
    let mut move_records: Vec<GameMoveRecord> = Vec::new();
    let mut move_count = 0;

    while !env.is_game_over() {
        let obs = env.extract_graph_observation();
        if obs.valid_actions.is_empty() {
            break;
        }
        let (_, _, chosen_action, _) = mcts.search_gpu(&env, eval_tx, false, 0.0);
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

/// Đánh giá sức mạnh của Model hiện tại với MCTS Greedy (Nhiệt độ = 0.0) trên Seed mục tiêu

pub fn evaluate_alphazero_agent(
    seed: i32,
    initial_stack: usize,
    tile_limit: usize,
    model: &HexGNNModel,
    mcts_config: &MCTSConfig,
) -> (usize, usize, GameMatchRecord) {
    let mut env = DorfromantikEnv::new(seed, initial_stack, tile_limit);
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
    /// Số sample MỚI thực sự được giữ lại trong lần self-play vừa rồi (dùng để quyết định lượng train).
    pub last_new_samples: usize,
    /// Danh sách board state đạt điểm cao nhất tại mỗi depth (khởi động lại 80% envs).
    pub max_score_states: Vec<MaxScoreStateRecord>,
}

impl AlphaZeroPipeline {
    pub fn new(config: AlphaZeroTrainerConfig) -> Self {
        let model = HexGNNModel::new();
        // Dung lượng Replay Buffer mặc định 100k sample (nếu không được cấu hình)
        let buf_capacity = config.replay_buffer_capacity.unwrap_or(100_000);
        let replay_buffer = AlphaZeroReplayBuffer::new(buf_capacity);
        Self {
            config,
            model,
            replay_buffer,
            last_new_samples: 0,
            max_score_states: Vec::new(),
        }
    }

    /// Gộp điểm cao nhất của 1 ván chơi vào danh sách max-score states theo depth.
    /// Mỗi depth giữ best_score + TẤT CẢ cấu hình đạt đúng best_score đó.
    pub fn merge_max_score_state(&mut self, moves: &[GameMoveRecord]) {
        for (i, m) in moves.iter().enumerate() {
            let depth = i + 1; // placed_count sau khi xong move i
            let score = m.total_score;
            // Tìm state hiện có cho depth này
            let existing_idx = self.max_score_states.iter().position(|s| s.depth == depth);
            match existing_idx {
                Some(idx) => {
                    let best = &mut self.max_score_states[idx].best_score;
                    if score > *best {
                        // Score mới cao hơn => reset best, chỉ giữ state này.
                        *best = score;
                        self.max_score_states[idx].states = vec![GameStateRecord {
                            score,
                            moves: moves[..depth].to_vec(),
                        }];
                    } else if score == self.max_score_states[idx].best_score {
                        // Cùng đạt best score => thêm cấu hình vào danh sách (nếu chưa có moves này).
                        let already = self.max_score_states[idx].states.iter().any(|s| {
                            s.moves.len() == depth
                                && s.moves.iter().zip(moves[..depth].iter()).all(|(a, b)| {
                                    a.q == b.q && a.r == b.r && a.rotation == b.rotation
                                })
                        });
                        if !already {
                            self.max_score_states[idx].states.push(GameStateRecord {
                                score,
                                moves: moves[..depth].to_vec(),
                            });
                        }
                    }
                }
                None => {
                    self.max_score_states.push(MaxScoreStateRecord {
                        depth,
                        best_score: score,
                        states: vec![GameStateRecord {
                            score,
                            moves: moves[..depth].to_vec(),
                        }],
                    });
                }
            }
        }
    }

    /// Thu thập dữ liệu tự chơi GPU (Parallel GPU Self-Play)
    pub fn collect_self_play_data_gpu(
        &mut self,
        eval_tx: &crossbeam_channel::Sender<crate::gpu_engine::GpuEvalRequest>,
    ) -> (f32, usize, usize, Option<GameMatchRecord>) {
        let n_envs = self.config.num_parallel_envs;
        let base_seed = self.config.target_seed;
        let initial_stack = self.config.initial_stack;
        let tile_limit = self.config.tile_limit;
        let mcts_cfg = self.config.mcts_config.clone();
        let temp_thresh = self.config.temp_threshold_moves;

        let seeds: Vec<i32> = vec![base_seed; n_envs];

        let results: Vec<(Vec<AlphaZeroSample>, GameMatchRecord)> = seeds
            .into_par_iter()
            .map(|s| run_self_play_episode_gpu(s, initial_stack, tile_limit, eval_tx, &mcts_cfg, temp_thresh))
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

    /// Thu thập dữ liệu tự chơi bằng Vectorized Batch MCTS (Không channel, không lock-step stall)
    /// Khi có gpu_exec: dùng GPU inference cho neural network evaluation.
    pub fn collect_self_play_data_batch(&mut self, gpu_exec: Option<&GpuNNExecutor>) -> (f32, usize, usize, Option<GameMatchRecord>) {
        self.last_new_samples = 0;
        let n_envs = self.config.num_parallel_envs;
        let base_seed = self.config.target_seed;
        let initial_stack = self.config.initial_stack;
        let tile_limit = self.config.tile_limit;
        let mcts_cfg = self.config.mcts_config.clone();
        let temp_thresh = self.config.temp_threshold_moves;
        let mcts = MCTSSearch::new(mcts_cfg.clone());

        // Xác định trước 80% envs sẽ khởi động từ board state max-score (nếu có sẵn).
        let mut envs: Vec<DorfromantikEnv> = Vec::with_capacity(n_envs);
        let mut from_state = vec![false; n_envs];
        let mut move_counts = vec![0usize; n_envs];
        if !self.max_score_states.is_empty() {
            let count = ((n_envs as f32) * 0.80) as usize;
            // Dùng 1 RNG đơn giản để chọn ngẫu nhiên các env nào từ state (không phụ thuộc ngoài)
            let mut chosen = std::collections::HashSet::new();
            let seed_rng = base_seed as u64;
            for k in 0..count {
                let idx = ((seed_rng.wrapping_add((k * 2654435761) as u64) % (n_envs as u64)) as usize).max(0);
                chosen.insert(idx);
            }
            for idx in 0..n_envs {
                from_state[idx] = chosen.contains(&idx);
            }
        }
        for idx in 0..n_envs {
            let mut env = DorfromantikEnv::new(base_seed, initial_stack, tile_limit);
            if from_state[idx] {
                // Chọn 1 record depth + 1 cấu hình trong states (đơn giản dựa trên idx).
                if let Some(state) = self.max_score_states.get(idx % self.max_score_states.len()) {
                    if let Some(cfg) = state.states.get(idx % state.states.len()) {
                        // Replay moves để đạt board state tại depth = state.depth
                        for m in &cfg.moves {
                            let _ = env.step(crate::env::Action { q: m.q, r: m.r, rotation: m.rotation });
                            if env.is_game_over() { break; }
                        }
                        move_counts[idx] = state.depth;
                    }
                }
            }
            envs.push(env);
        }

        let mut raw_steps: Vec<Vec<(GraphObservation, Vec<f32>, f32)>> = vec![Vec::new(); n_envs];
        let mut move_records: Vec<Vec<GameMoveRecord>> = vec![Vec::new(); n_envs];
        let mut active = vec![true; n_envs];
        let mut turn_counter = 0usize;
        let mut total_moves = 0usize;
        use std::io::Write;

        print!("[Self-Play Progress] ");
        let _ = std::io::stdout().flush();

        while active.iter().any(|&a| a) {
            let active_indices: Vec<usize> = (0..n_envs).filter(|&i| active[i]).collect();
            if active_indices.is_empty() {
                break;
            }

            turn_counter += 1;
            let add_dirichlet = move_counts[active_indices[0]] < temp_thresh;
            let temp = if move_counts[active_indices[0]] < temp_thresh { 1.0f32 } else { 0.2f32 };

            let batch_results = mcts.search_batch_indexed(&envs, &active_indices, &self.model, gpu_exec, add_dirichlet, temp);

            for (k, &idx) in active_indices.iter().enumerate() {
                let (pi_probs, _, chosen_action, _, obs) = &batch_results[k];

                if obs.valid_actions.is_empty() {
                    active[idx] = false;
                    print!("\n  ✓ [Env #{} Kết thúc] Score: {}, Placed: {} tiles\n[Self-Play Progress] ", idx + 1, envs[idx].score_manager.total_score, envs[idx].placed_count);
                    let _ = std::io::stdout().flush();
                    continue;
                }

                let prev_score = envs[idx].score_manager.total_score;
                let res = envs[idx].step(*chosen_action);
                let score_gained = envs[idx].score_manager.total_score.saturating_sub(prev_score);
                let scaled_r = res.reward * 0.01;

                move_records[idx].push(GameMoveRecord {
                    step: move_counts[idx],
                    q: chosen_action.q,
                    r: chosen_action.r,
                    rotation: chosen_action.rotation,
                    score_gained,
                    total_score: envs[idx].score_manager.total_score,
                    remaining_tiles: envs[idx].score_manager.remaining_tiles,
                });

                raw_steps[idx].push((obs.clone(), pi_probs.clone(), scaled_r));
                move_counts[idx] += 1;
                total_moves += 1;

                if res.done || envs[idx].is_game_over() {
                    active[idx] = false;
                    print!("\n  ✓ [Env #{} Kết thúc] Score: {}, Placed: {} tiles\n[Self-Play Progress] ", idx + 1, envs[idx].score_manager.total_score, envs[idx].placed_count);
                    let _ = std::io::stdout().flush();
                }
            }

            // In đúng 1 dấu chấm nhịp nhàng sau mỗi lượt đi hoàn thành (giống Wisdom engine)
            print!(".");
            if turn_counter % 25 == 0 {
                print!(" [Turn {} | Active: {}/{} | {} moves]\n[Self-Play Progress] ", turn_counter, active_indices.len(), n_envs, total_moves);
            }
            let _ = std::io::stdout().flush();
        }
        println!("\n[Self-Play Done] Hoàn thành {} turns, tổng cộng {} nước đi.", turn_counter, total_moves);

        let mut total_score = 0;
        let mut max_score = 0;
        let mut total_placed = 0;
        let mut best_record: Option<GameMatchRecord> = None;

        for i in 0..n_envs {
            // Ghi nhận max-score states tại các depth (dùng để khởi động lại 80% envs iter sau).
            self.merge_max_score_state(&move_records[i]);

            let final_score = envs[i].score_manager.total_score;
            let placed_count = envs[i].placed_count;
            total_score += final_score;
            if final_score >= max_score {
                max_score = final_score;
                best_record = Some(GameMatchRecord {
                    seed: base_seed,
                    total_score: final_score,
                    total_placed: placed_count,
                    is_eval: false,
                    moves: move_records[i].clone(),
                });
            }
            total_placed += placed_count;

            let total_steps = raw_steps[i].len();
            let mut samples = Vec::with_capacity(total_steps);
            let mut g = 0.0f32;

            for t in (0..total_steps).rev() {
                let (obs, pi, r) = raw_steps[i][t].clone();
                g = r + mcts_cfg.gamma * g;
                samples.push(AlphaZeroSample {
                    obs,
                    target_pi: pi,
                    target_val: g,
                });
            }
            samples.reverse();
            self.last_new_samples += self.replay_buffer.push_batch(samples);
        }

        let avg_score = total_score as f32 / n_envs as f32;
        let avg_placed = total_placed / n_envs;
        (avg_score, max_score, avg_placed, best_record)
    }

    /// Thu thập dữ liệu tự chơi qua Rayon đa luồng (Parallel Self-Play CPU)

    pub fn collect_self_play_data(&mut self) -> (f32, usize, usize, Option<GameMatchRecord>) {
        let n_envs = self.config.num_parallel_envs;
        let base_seed = self.config.target_seed;
        let initial_stack = self.config.initial_stack;
        let tile_limit = self.config.tile_limit;
        let mcts_cfg = self.config.mcts_config.clone();
        let temp_thresh = self.config.temp_threshold_moves;
        let model_ref = &self.model;

        // 100% tất cả luồng chạy trên cùng target_seed của file monthly
        let seeds: Vec<i32> = vec![base_seed; n_envs];

        // Chạy đa luồng song song các ván đấu MCTS
        let results: Vec<(Vec<AlphaZeroSample>, GameMatchRecord)> = seeds
            .into_par_iter()
            .map(|s| run_self_play_episode(s, initial_stack, tile_limit, model_ref, &mcts_cfg, temp_thresh))
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

    /// Huấn luyện mạng GNN trên mini-batches từ Replay Buffer bằng Adam Optimizer (Zero-Copy Sampling)
    pub fn train_step(&mut self) -> (f32, f32, f32) {
        // Lọc bỏ các state trùng lặp trước khi train để tránh overfit
        let removed = self.replay_buffer.deduplicate();
        if removed > 0 {
            println!("[Train] Đã lọc {} sample trùng lặp khỏi replay buffer (còn {}).", removed, self.replay_buffer.len());
        }
        let buf_len = self.replay_buffer.len();
        if buf_len < self.config.batch_size {
            return (0.0, 0.0, 0.0);
        }

        // Warm-up: chỉ bắt đầu train khi buffer đã nạp đủ 20% dung lượng, tránh overfit vào data khởi đầu ít ỏi
        let warmup_threshold = (self.replay_buffer.capacity as f32 * 0.20) as usize;
        if buf_len < warmup_threshold {
            println!(
                "[Train] Warm-up: buffer {}/{} sample (cần ≥ {}) — chưa train, tiếp tục self-play tích lũy.",
                buf_len, self.replay_buffer.capacity, warmup_threshold
            );
            return (0.0, 0.0, 0.0);
        }

        // Train đúng bằng số sample MỚI thực sự giữ lại trong iteration này (số push, lọc trùng)
        let mut m = self.last_new_samples;
        if m < self.config.batch_size {
            m = self.config.batch_size.min(buf_len);
        }
        let num_batches = (m / self.config.batch_size).max(1);
        let total_epochs = self.config.train_epochs_per_iter;
        let train_start = std::time::Instant::now();
        println!(
            "[Train] Bắt đầu: {} epochs × {} batches (batch_size={}) | train trên {} sample mới (buffer {}/{}) CPU...",
            total_epochs, num_batches, self.config.batch_size, m, buf_len, self.replay_buffer.capacity
        );
        let mut total_policy_loss = 0.0f32;
        let mut total_value_loss = 0.0f32;
        let mut step_count = 0;

        for epoch in 0..total_epochs {
            // Bốc ngẫu nhiên M indices (trộn) từ buffer để train cho epoch này
            let epoch_indices = self.replay_buffer.sample_unique_indices(m);
            let epoch_batches = if epoch_indices.is_empty() {
                0
            } else {
                (epoch_indices.len() / self.config.batch_size).min(num_batches).max(1)
            };
            use std::io::Write;
            print!("[Train Epoch {}/{}] ", epoch + 1, total_epochs);
            let _ = std::io::stdout().flush();
            for batch in 0..epoch_batches {
                let start = batch * self.config.batch_size;
                let end = ((batch + 1) * self.config.batch_size).min(epoch_indices.len());
                let indices = epoch_indices[start..end].to_vec();
                if indices.is_empty() {
                    continue;
                }

                let model_ref = &self.model;
                let val_coeff = self.config.value_loss_coeff;
                let buffer_ref = &self.replay_buffer.buffer;

                let (mb_grads, (mb_pi_loss, mb_val_loss)) = indices
                    .into_par_iter()
                    .map(|idx| {
                        let sample = &buffer_ref[idx];
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

                if (batch + 1) % 4 == 0 || batch + 1 == epoch_batches {
                    print!("{}/{} ", batch + 1, epoch_batches);
                    let _ = std::io::stdout().flush();
                }

                total_policy_loss += mb_pi_loss / mb_len;
                total_value_loss += mb_val_loss / mb_len;
                step_count += 1;
            }
            let elapsed = train_start.elapsed();
            println!("({:.1}s)", elapsed.as_secs_f32());
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
            // Tự động chuẩn hóa target_val nếu dữ liệu cũ đang ở scale 0.05 (target_val > 20.0)
            let avg_val: f32 = if !self.replay_buffer.is_empty() {
                self.replay_buffer.buffer.iter().take(100).map(|s| s.target_val).sum::<f32>() / 100.0f32.min(self.replay_buffer.len() as f32)
            } else {
                0.0
            };
            if avg_val > 20.0 {
                for sample in self.replay_buffer.buffer.iter_mut() {
                    sample.target_val *= 0.2; // Chuyển từ scale 0.05 sang 0.01 (0.01 / 0.05 = 0.2)
                }
            }
        }
        Ok(())
    }
}

/// Tính chữ ký hash (u64) cho toàn bộ observation — dùng để nhận diện các ván state trùng lặp.
/// Hash ngay chính input mà model nhìn thấy (node_positions, node_features, edge_index,
/// valid_actions, action_features) nên trạng thái giống y hệt sẽ có cùng chữ ký.
fn observation_hash(obs: &GraphObservation) -> u64 {
    use std::hash::DefaultHasher;
    let mut h = DefaultHasher::new();
    for &(q, r) in &obs.node_positions {
        q.hash(&mut h);
        r.hash(&mut h);
    }
    for feat in &obs.node_features {
        for &v in feat {
            v.to_bits().hash(&mut h);
        }
    }
    for &(u, v) in &obs.edge_index {
        u.hash(&mut h);
        v.hash(&mut h);
    }
    for act in &obs.valid_actions {
        act.q.hash(&mut h);
        act.r.hash(&mut h);
        act.rotation.hash(&mut h);
    }
    for feat in &obs.action_features {
        for &v in feat {
            v.to_bits().hash(&mut h);
        }
    }
    h.finish()
}

/// Hash TOÀN BỘ sample gồm state (obs) + target_pi + target_val.
/// Chỉ coi 2 sample là TRÙNG khi cả state lẫn target huấn luyện đều giống hệt nhau.
/// Điều này đúng tinh thần AlphaZero: cùng board nhưng khác target (do model/exploration
/// khác lần) vẫn là 2 sample hợp lệ cần train riêng, không bị dedup.
fn sample_full_hash(sample: &AlphaZeroSample) -> u64 {
    use std::hash::DefaultHasher;
    let mut h = DefaultHasher::new();
    observation_hash(&sample.obs).hash(&mut h);
    for &p in &sample.target_pi {
        p.to_bits().hash(&mut h);
    }
    sample.target_val.to_bits().hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_max_score_state_keeps_all_best_per_depth() {
        let mut pipe = AlphaZeroPipeline::new(AlphaZeroTrainerConfig::default());

        // Ván 1: depth 1 score 10, depth 2 score 20
        let moves1 = vec![
            GameMoveRecord { step: 0, q: 0, r: 1, rotation: 0, score_gained: 10, total_score: 10, remaining_tiles: 99 },
            GameMoveRecord { step: 1, q: 0, r: 2, rotation: 1, score_gained: 10, total_score: 20, remaining_tiles: 98 },
        ];
        pipe.merge_max_score_state(&moves1);
        assert_eq!(pipe.max_score_states.len(), 2);

        // Ván 2: depth 1 score 30 (cao hơn), depth 2 score 20 (bằng best => thêm cấu hình)
        let moves2 = vec![
            GameMoveRecord { step: 0, q: 1, r: 1, rotation: 0, score_gained: 30, total_score: 30, remaining_tiles: 99 },
            GameMoveRecord { step: 1, q: 1, r: 2, rotation: 2, score_gained: 0, total_score: 20, remaining_tiles: 98 },
        ];
        pipe.merge_max_score_state(&moves2);

        // depth 1: best = 30, chỉ 1 cấu hình (ván 2), duration 20 bị thay thế
        let d1 = pipe.max_score_states.iter().find(|s| s.depth == 1).unwrap();
        assert_eq!(d1.best_score, 30);
        assert_eq!(d1.states.len(), 1); // ván 1 (score 10) bị thay bằng ván 2 (score 30)
        assert_eq!(d1.states[0].moves[0].q, 1);

        // depth 2: best = 20, có 2 cấu hình (ván 1 và ván 2 đều đạt 20)
        let d2 = pipe.max_score_states.iter().find(|s| s.depth == 2).unwrap();
        assert_eq!(d2.best_score, 20);
        assert_eq!(d2.states.len(), 2);
    }
}

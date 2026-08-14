use rand::prelude::*;
use rayon::prelude::*;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

use crate::env::{DorfromantikEnv, GraphObservation};
use crate::gpu_nn::GpuNNExecutor;
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
        (0..batch_size).map(|_| rng.gen_range(0..len)).collect()
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
        let mut indices: Vec<usize> = (0..len).collect();
        for i in 0..count {
            let j = i + rng.gen_range(0..(len - i));
            indices.swap(i, j);
        }
        indices.truncate(count);
        indices
    }

    /// Lấy `count` indices từ buffer với cơ chế Prioritized Experience Replay (PER):
    /// - 50% samples được ưu tiên lấy từ các mẫu có target_val cao nhất (Top trajectory positions)
    /// - 50% samples được lấy ngẫu nhiên đều từ toàn bộ buffer để duy trì tính đa dạng
    pub fn sample_prioritized_unique_indices(&self, count: usize) -> Vec<usize> {
        let len = self.buffer.len();
        let count = count.min(len);
        if count == 0 {
            return Vec::new();
        }

        let mut rng = rand::thread_rng();
        let top_target = count / 2;

        let mut indices_val: Vec<(usize, f32)> = self
            .buffer
            .iter()
            .enumerate()
            .map(|(i, s)| (i, s.target_val))
            .collect();

        // Sắp xếp giảm dần theo target_val
        indices_val.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let top_pool_len = ((len as f32) * 0.35).ceil() as usize;
        let top_pool_len = top_pool_len.max(top_target).min(len);

        let mut selected: HashSet<usize> = HashSet::with_capacity(count);
        let mut result: Vec<usize> = Vec::with_capacity(count);

        // 1. Lấy mẫu từ top_pool (những nước đi dẫn đến ván chơi/hậu kỳ điểm cao)
        let mut attempts = 0;
        while result.len() < top_target && attempts < count * 2 {
            attempts += 1;
            let pick = rng.gen_range(0..top_pool_len);
            let idx = indices_val[pick].0;
            if selected.insert(idx) {
                result.push(idx);
            }
        }

        // 2. Lấy mẫu ngẫu nhiên từ toàn bộ buffer cho phần còn lại
        attempts = 0;
        while result.len() < count && attempts < count * 3 {
            attempts += 1;
            let idx = rng.gen_range(0..len);
            if selected.insert(idx) {
                result.push(idx);
            }
        }

        // 3. Fallback lấp đầy nếu chưa đủ count
        if result.len() < count {
            for i in 0..len {
                if result.len() >= count {
                    break;
                }
                if selected.insert(i) {
                    result.push(i);
                }
            }
        }

        result.shuffle(&mut rng);
        result
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

    /// Làm mềm (un-sharpen) lại phân phối target_pi của tất cả sample trong buffer bằng cách
    /// áp dụng hàm lũy thừa ngược pi'_i = (pi_i)^factor rồi chuẩn hóa lại.
    /// Giúp khôi phục các buffer cũ từng bị nhọn hóa bởi sampling temperature (ví dụ factor=0.2 - 0.5).
    /// Chạy cực nhanh (< 50ms cho 200k samples).
    pub fn unsharpen_target_pi(&mut self, factor: f32) {
        if factor <= 0.0 || (factor - 1.0).abs() < 1e-4 {
            return;
        }
        for sample in self.buffer.iter_mut() {
            if sample.target_pi.is_empty() {
                continue;
            }
            let powered: Vec<f32> = sample.target_pi.iter().map(|&p| p.max(0.0).powf(factor)).collect();
            let sum: f32 = powered.iter().sum::<f32>().max(1e-8);
            sample.target_pi = powered.into_iter().map(|p| p / sum).collect();
        }
    }

    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        use std::io::Write;
        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
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
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid buffer format",
            ));
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
            gamma: 0.995,
            value_loss_coeff: 0.5,
            batch_size: 128,
            train_epochs_per_iter: 4,
            mcts_config: MCTSConfig {
                c_puct: 1.5,
                gamma: 0.995,
                n_simulations: 200,
                dirichlet_alpha: 0.3,
                dirichlet_eps: 0.25,
                explore_by_entropy: true,
                temp_high: 1.0,
                temp_low: 0.2,
            },
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

/// Bản ghi board state có Q-value cao.
/// Dùng để khởi động lại 80% envs từ vị thế tốt thay vì từ bàn trống.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MaxScoreStateRecord {
    /// Q-value ước lượng tổng điểm cuối game: total_score + target_val * 100.
    pub q_value: f32,
    /// Số tile còn lại (phải >= 10 mới được lưu).
    pub remaining_tiles: usize,
    /// Các actions dùng để đạt board state này (replay deterministic theo seed).
    pub moves: Vec<GameMoveRecord>,
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
        let (_, _, chosen_action, _) = mcts.search(&mut env, model, false, 0.0);
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

    /// Thêm 1 board state có Q-value cao vào danh sách (top 2000 state Q tốt nhất).
    /// Q = total_score + target_val * 100.
    pub fn add_high_q_state(
        &mut self,
        q_value: f32,
        remaining_tiles: usize,
        moves: &[GameMoveRecord],
    ) {
        // Tìm state trùng chuỗi moves (cùng board config qua dãy nước đi).
        let mut found = None;
        for (i, s) in self.max_score_states.iter().enumerate() {
            if s.moves.len() == moves.len()
                && s.moves
                    .iter()
                    .zip(moves.iter())
                    .all(|(a, b)| a.q == b.q && a.r == b.r && a.rotation == b.rotation)
            {
                found = Some(i);
                break;
            }
        }
        match found {
            // Luôn ghi đè Q-value và remaining_tiles bằng giá trị mới (kể cả thấp hơn).
            Some(i) => {
                self.max_score_states[i].q_value = q_value;
                self.max_score_states[i].remaining_tiles = remaining_tiles;
            }
            None => {
                self.max_score_states.push(MaxScoreStateRecord {
                    q_value,
                    remaining_tiles,
                    moves: moves.to_vec(),
                });
            }
        }
        // Sắp xếp giảm dần theo Q-value, giữ tối đa 2000 state.
        // State bị ghi đè bằng Q thấp sẽ bị đẩy ra ngoài top 2000 và bị loại bỏ.
        self.max_score_states.sort_by(|a, b| {
            b.q_value
                .partial_cmp(&a.q_value)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if self.max_score_states.len() > 2000 {
            self.max_score_states.truncate(2000);
        }
    }
    /// Cập nhật lại toàn bộ Q-value của max_score_states bằng MCTS song song (batch).
    /// Mỗi state: replay moves trong file để dựng board, rồi chạy MCTS `n_simulations` sim
    /// (temp thấp, không dirichlet) lấy root value; Q mới = total_score hiện tại + root_value * 100.
    /// State không replay được sẽ GIỮ NGUYÊN (không cập nhật, không xóa). Trả về số state đã cập nhật.
    pub fn refresh_max_score_state_q_values(
        &mut self,
        gpu_exec: Option<&GpuNNExecutor>,
        n_simulations: usize,
    ) -> usize {
        let base_seed = self.config.target_seed;
        let initial_stack = self.config.initial_stack;
        let tile_limit = self.config.tile_limit;
        let n = self.max_score_states.len();
        if n == 0 {
            return 0;
        }

        let mut mcts_cfg = self.config.mcts_config.clone();
        mcts_cfg.n_simulations = n_simulations;
        let mcts = MCTSSearch::new(mcts_cfg.clone());

        // BƯỚC 1: replay từng moves trong file để dựng lại board state (kiểm tra hợp lệ).
        let mut valid_envs: Vec<DorfromantikEnv> = Vec::with_capacity(n);
        let mut orig_idx: Vec<usize> = Vec::with_capacity(n);
        let mut base_scores: Vec<usize> = Vec::with_capacity(n);
        for (i, st) in self.max_score_states.iter().enumerate() {
            let mut env = DorfromantikEnv::new(base_seed, initial_stack, tile_limit);
            let mut ok = true;
            for m in &st.moves {
                let act = crate::env::Action {
                    q: m.q,
                    r: m.r,
                    rotation: m.rotation,
                };
                if !env.get_valid_actions().contains(&act) {
                    ok = false;
                    break;
                }
                env.step(act);
                if env.is_game_over() {
                    break;
                }
            }
            if ok && env.placed_count == st.moves.len() {
                base_scores.push(env.score_manager.total_score);
                valid_envs.push(env);
                orig_idx.push(i);
            } else {
                base_scores.push(0);
            }
        }
        let b_count = valid_envs.len();
        if b_count == 0 {
            return 0;
        }

        // BƯỚC 2: sau khi đã make move, chạy MCTS 800 sim batch (temp 0.2, KHÔNG dirichlet).
        let active: Vec<usize> = (0..b_count).collect();
        let results =
            mcts.search_batch_indexed(&valid_envs, &active, &self.model, gpu_exec, false, 0.2);

        // BƯỚC 3: ghi đè Q value: total_score + root_value * 100.
        for (k, &st_idx) in orig_idx.iter().enumerate() {
            let root_val = results[k].3;
            self.max_score_states[st_idx].q_value = base_scores[st_idx] as f32 + root_val * 100.0;
        }

        // BƯỚC 4: sort giảm dần, giữ top 2000 (giống add_high_q_state).
        self.max_score_states.sort_by(|a, b| {
            b.q_value
                .partial_cmp(&a.q_value)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if self.max_score_states.len() > 2000 {
            self.max_score_states.truncate(2000);
        }

        b_count
    }

    /// Thu thập dữ liệu tự chơi bằng Vectorized Batch MCTS (Không channel, không lock-step stall)
    /// Khi có gpu_exec: dùng GPU inference cho neural network evaluation.
    pub fn collect_self_play_data_batch(
        &mut self,
        gpu_exec: Option<&GpuNNExecutor>,
    ) -> (f32, usize, usize, Option<GameMatchRecord>) {
        self.last_new_samples = 0;
        let n_envs = self.config.num_parallel_envs;
        let base_seed = self.config.target_seed;
        let initial_stack = self.config.initial_stack;
        let tile_limit = self.config.tile_limit;
        let mcts_cfg = self.config.mcts_config.clone();
        let mcts = MCTSSearch::new(mcts_cfg.clone());

        // Xác định trước 80% envs sẽ khởi động từ board state max-score (nếu có sẵn).
        let mut envs: Vec<DorfromantikEnv> = Vec::with_capacity(n_envs);
        let mut from_state = vec![false; n_envs];
        let mut move_counts = vec![0usize; n_envs];
        // Số nước đi gốc khi env khởi động từ board state (offset lịch sử cho move_records).
        let mut hist_offset = vec![0usize; n_envs];
        // Moves gốc của state để replay về đúng board khi from-state envs tự chơi tiếp.
        let mut env_source_moves: Vec<Vec<GameMoveRecord>> = vec![Vec::new(); n_envs];
        if !self.max_score_states.is_empty() {
            let mut rng = rand::thread_rng();
            let count = ((n_envs as f32) * 0.80) as usize;
            let mut all_indices: Vec<usize> = (0..n_envs).collect();
            all_indices.shuffle(&mut rng);
            for &idx in &all_indices[..count] {
                from_state[idx] = true;
            }
        }
        let mut rng = rand::thread_rng();
        for idx in 0..n_envs {
            let mut env = DorfromantikEnv::new(base_seed, initial_stack, tile_limit);
            if from_state[idx] {
                // Chọn ngẫu nhiên có trọng số (ưu tiên top Q-value cao nhất ở đầu danh sách)
                let r_bias = rng.gen::<f32>().powi(2); // quadratic bias towards 0 (top Q)
                let state_idx = (r_bias * self.max_score_states.len() as f32) as usize;
                if let Some(state) = self.max_score_states.get(state_idx) {
                    // Replay moves để đạt board state (depth = moves.len()).
                    let mut replay_ok = true;
                    for m in state.moves.iter() {
                        let valid = env.get_valid_actions();
                        let act = crate::env::Action {
                            q: m.q,
                            r: m.r,
                            rotation: m.rotation,
                        };
                        if !valid.contains(&act) {
                            replay_ok = false;
                            break;
                        }
                        let _ = env.step(act);
                        if env.is_game_over() {
                            break;
                        }
                    }
                    if replay_ok && env.placed_count == state.moves.len() {
                        move_counts[idx] = state.moves.len();
                        hist_offset[idx] = state.moves.len();
                        env_source_moves[idx] = state.moves.clone();
                    }
                }
            }
            envs.push(env);
        }

        let mut raw_steps: Vec<Vec<(GraphObservation, Vec<f32>, f32)>> = vec![Vec::new(); n_envs];
        let mut move_records: Vec<Vec<GameMoveRecord>> = vec![Vec::new(); n_envs];
        // Khởi tạo move_records từ moves gốc (env from-state) để add_high_q_state lưu state với lịch sử đầy đủ.
        for idx in 0..n_envs {
            move_records[idx].extend(env_source_moves[idx].clone());
        }
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
            // Bat_note: Khi MCTSConfig.explore_by_entropy bật, MCTS tự quyết nhiệt độ/noise
            // từng turn theo prior entropy (tự tin → explore, bối rối → exploit).
            let batch_results = mcts.search_batch_indexed(
                &envs,
                &active_indices,
                &self.model,
                gpu_exec,
                true,
                1.0,
            );

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
                let score_gained = envs[idx]
                    .score_manager
                    .total_score
                    .saturating_sub(prev_score);
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
                print!(
                    " [Turn {} | Active: {}/{} | {} moves]\n[Self-Play Progress] ",
                    turn_counter,
                    active_indices.len(),
                    n_envs,
                    total_moves
                );
            }
            let _ = std::io::stdout().flush();
        }
        println!(
            "\n[Self-Play Done] Hoàn thành {} turns, tổng cộng {} nước đi.",
            turn_counter, total_moves
        );

        let mut total_score = 0;
        let mut max_score = 0;
        let mut total_placed = 0;
        let mut best_record: Option<GameMatchRecord> = None;

        for i in 0..n_envs {
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

                // Thu thập board state có Q-value cao (điểm hiện tại + tiềm năng tương lai).
                // Chỉ giữ khi còn >= 10 tile chưa đặt.
                let off = hist_offset[i];
                if t < move_records[i].len() - off {
                    let real = off + t;
                    let m = &move_records[i][real];
                    if m.remaining_tiles >= 10 {
                        let q = m.total_score as f32 + g * 100.0;
                        self.add_high_q_state(q, m.remaining_tiles, &move_records[i][..=real]);
                    }
                }
            }
            samples.reverse();
            self.last_new_samples += self.replay_buffer.push_batch(samples);
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
            println!(
                "[Train] Đã lọc {} sample trùng lặp khỏi replay buffer (còn {}).",
                removed,
                self.replay_buffer.len()
            );
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
            // Bốc ngẫu nhiên M indices (trộn) từ buffer theo PER (50% top-value, 50% uniform) để train
            let epoch_indices = self.replay_buffer.sample_prioritized_unique_indices(m);
            let epoch_batches = if epoch_indices.is_empty() {
                0
            } else {
                (epoch_indices.len() / self.config.batch_size)
                    .min(num_batches)
                    .max(1)
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
                        let max_l = action_logits
                            .iter()
                            .cloned()
                            .fold(f32::NEG_INFINITY, f32::max);
                        let exps: Vec<f32> =
                            action_logits.iter().map(|l| (l - max_l).exp()).collect();
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
                self.model
                    .update_weights_adam(&scaled_grads, self.config.lr);

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

        let avg_pi_loss = if step_count > 0 {
            total_policy_loss / step_count as f32
        } else {
            0.0
        };
        let avg_val_loss = if step_count > 0 {
            total_value_loss / step_count as f32
        } else {
            0.0
        };
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
                self.replay_buffer
                    .buffer
                    .iter()
                    .take(100)
                    .map(|s| s.target_val)
                    .sum::<f32>()
                    / 100.0f32.min(self.replay_buffer.len() as f32)
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
    fn test_add_high_q_state_keeps_top_n_sorted() {
        let mut pipe = AlphaZeroPipeline::new(AlphaZeroTrainerConfig::default());
        let mk = |q: i32| GameMoveRecord {
            step: 0,
            q,
            r: 0,
            rotation: 0,
            score_gained: 0,
            total_score: q as usize,
            remaining_tiles: 50,
        };

        // Thêm 2 state với Q khác nhau => giữ 2, sort Q giảm dần.
        pipe.add_high_q_state(50.0, 50, &[mk(1)]);
        pipe.add_high_q_state(80.0, 50, &[mk(2)]);
        assert_eq!(pipe.max_score_states.len(), 2);
        assert_eq!(pipe.max_score_states[0].q_value, 80.0);
        assert_eq!(pipe.max_score_states[1].q_value, 50.0);

        // Trùng moves (cùng q) => không thêm mới, giữ 2 state.
        pipe.add_high_q_state(90.0, 50, &[mk(2)]);
        assert_eq!(pipe.max_score_states.len(), 2);
        assert_eq!(pipe.max_score_states[0].q_value, 90.0); // cập nhật Q nhưng giữ vị trí sort lại
    }

    #[test]
    fn test_add_high_q_state_truncates_to_2000() {
        let mut pipe = AlphaZeroPipeline::new(AlphaZeroTrainerConfig::default());
        // Thêm hơn 2000 state Q khác nhau.
        for i in 0..2200 {
            let m = GameMoveRecord {
                step: 0,
                q: i as i32,
                r: 0,
                rotation: 0,
                score_gained: 0,
                total_score: i,
                remaining_tiles: 50,
            };
            pipe.add_high_q_state(i as f32, 50, &[m]);
        }
        assert_eq!(pipe.max_score_states.len(), 2000);
        // Giữ 2000 Q cao nhất: 2199..2000.
        assert_eq!(pipe.max_score_states[0].q_value, 2199.0);
        assert_eq!(pipe.max_score_states[1999].q_value, 200.0);
    }
}

#[cfg(test)]
mod tests_overwrite {
    use super::*;

    #[test]
    fn test_high_q_state_overwrites_lower_q() {
        let mut pipe = AlphaZeroPipeline::new(AlphaZeroTrainerConfig::default());
        let mk = |tag: i32| GameMoveRecord {
            step: 0,
            q: tag,
            r: 0,
            rotation: 0,
            score_gained: 0,
            total_score: tag as usize,
            remaining_tiles: 50,
        };

        // State cùng moves (cùng tag=5): đầu Q=90, sau ghi đè bằng Q=40 (thấp hơn).
        pipe.add_high_q_state(90.0, 40, &[mk(5)]);
        assert_eq!(pipe.max_score_states.len(), 1);
        pipe.add_high_q_state(40.0, 30, &[mk(5)]);
        assert_eq!(
            pipe.max_score_states.len(),
            1,
            "trùng moves phải ghi đè, không thêm mới"
        );
        assert_eq!(
            pipe.max_score_states[0].q_value, 40.0,
            "Q mới thấp hơn vẫn phải ghi đè"
        );
        assert_eq!(pipe.max_score_states[0].remaining_tiles, 30);
    }

    #[test]
    fn test_high_q_state_overwrite_sorts_out_of_top() {
        let mut pipe = AlphaZeroPipeline::new(AlphaZeroTrainerConfig::default());
        let mk = |tag: i32| GameMoveRecord {
            step: 0,
            q: tag,
            r: 0,
            rotation: 0,
            score_gained: 0,
            total_score: tag as usize,
            remaining_tiles: 50,
        };

        // Tạo 2001 state khác nhau Q từ 0..2000.
        for i in 0..2001 {
            pipe.add_high_q_state(i as f32, 40, &[mk(i)]);
        }
        assert_eq!(pipe.max_score_states.len(), 2000, "chỉ giữ top 2000");
        // State có tag=0 (Q thấp nhất) bị loại khỏi top.
        assert!(
            pipe.max_score_states.iter().all(|s| s.q_value > 0.0),
            "state Q thấp nhất bị loại"
        );
        // State Q=2000 (top) hiện nằm trong danh sách.
        assert!(pipe.max_score_states.iter().any(|s| s.q_value == 2000.0));

        // Ghi đè state Q=2000 bằng Q=-5 (rất thấp). State vẫn chiếm 1 slot (số lượng không đổi),
        // nên -5 trở thành mức Q thấp nhất đang hiện diện trong top.
        pipe.add_high_q_state(-5.0, 40, &[mk(2000)]);
        assert_eq!(pipe.max_score_states.len(), 2000);
        assert!(
            pipe.max_score_states.iter().any(|s| s.q_value == -5.0),
            "ghi đè thấp vẫn trong top vì không làm tăng số lượng"
        );

        // Push thêm 1 state mới (Q=1999) -> vượt 2001 -> cắt xuống 2000,
        // state có Q thấp nhất (=-5) bị loại khỏi top.
        pipe.add_high_q_state(1999.0, 40, &[mk(3000)]);
        assert_eq!(pipe.max_score_states.len(), 2000);
        assert!(
            pipe.max_score_states.iter().all(|s| s.q_value > -5.0),
            "state bị ghi đè Q âm đã bị loại khỏi top"
        );
    }

    #[test]
    fn test_sample_prioritized_unique_indices() {
        let mut buffer = AlphaZeroReplayBuffer::new(100);
        for i in 0..50 {
            buffer.push(AlphaZeroSample {
                obs: GraphObservation {
                    node_positions: vec![(0, 0)],
                    node_features: vec![[0.0; crate::env::node_feat::DIM]],
                    edge_index: Vec::new(),
                    valid_actions: Vec::new(),
                    action_features: Vec::new(),
                },
                target_pi: vec![1.0],
                target_val: (i as f32) * 10.0,
            });
        }
        let sample = buffer.sample_prioritized_unique_indices(20);
        assert_eq!(sample.len(), 20);
        let mut seen = std::collections::HashSet::new();
        for &idx in &sample {
            assert!(idx < 50);
            assert!(seen.insert(idx), "Indices must be unique");
        }
    }
}

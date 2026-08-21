use rand::Rng;
use rand_distr::{Distribution, Gamma};
use rayon::prelude::*;
use crate::env::{Action, DorfromantikEnv};
use crate::nn::HexGNNModel;
use crate::gpu_nn::GpuNNExecutor;

/// Cấu trúc lưu trữ 1 Node trong Cây Tìm kiếm Monte Carlo (MCTS)
#[derive(Debug, Clone)]
pub struct MCTSNode {
    pub prior: f32,
    pub visit_count: u32,
    pub total_value: f32,
    pub immediate_reward: f32,
    pub is_expanded: bool,
    pub is_terminal: bool,
    pub children: Vec<(Action, MCTSNode)>,
}

impl MCTSNode {
    pub fn new(prior: f32) -> Self {
        Self {
            prior,
            visit_count: 0,
            total_value: 0.0,
            immediate_reward: 0.0,
            is_expanded: false,
            is_terminal: false,
            children: Vec::new(),
        }
    }

    /// Q-value trung bình (được chuẩn hóa khi tính UCB)
    pub fn q_value(&self) -> f32 {
        if self.visit_count == 0 {
            0.0
        } else {
            self.total_value / self.visit_count as f32
        }
    }
}

/// Quản lý tham số và vòng lặp MCTS
#[derive(Debug, Clone)]
pub struct MCTSConfig {
    pub c_puct: f32,
    pub gamma: f32,
    pub n_simulations: usize,
    pub dirichlet_alpha: f32,
    pub dirichlet_eps: f32,
    /// Bật quyết định exploration theo entropy của prior từng turn.
    /// - Prior entropy THẤP (model tự tin) → tăng noise + temp cao (explore)
    /// - Prior entropy CAO (model bối rối) → giảm/không noise + temp thấp (exploit)
    pub explore_by_entropy: bool,
    /// Nhiệt độ cao nhất (khi prior cực tự tin) — dùng suy temp liên tục.
    pub temp_high: f32,
    /// Nhiệt độ thấp nhất (khi prior cực bối rối).
    pub temp_low: f32,
}

impl MCTSConfig {
    /// Quyết định (add_dirichlet, temperature) cho 1 turn dựa trên prior normalized entropy `prior_e`.
    /// `prior_e` ∈ [0,1]: 0 = one-hot (tự tin), 1 = uniform (bối rối).
    /// Chiều: tự tin → mạnh explore; bối rối → nhẹ explore (exploit).
    /// Khi `explore_by_entropy = false`: giữ nguyên hành vi cũ (dùng các tham số gọi truyền vào).
    pub fn entropy_explore(&self, prior_e: f32) -> (bool, f32) {
        if !self.explore_by_entropy {
            return (false, 0.2);
        }
        // prior_e=0 (tự tin) → strength=1; prior_e=1 (bối rối) → strength=0
        let strength = (1.0 - prior_e).clamp(0.0, 1.0);
        let mut rng = rand::thread_rng();
        let add_noise = rng.gen::<f32>() < strength;
        let temp = self.temp_low + (self.temp_high - self.temp_low) * strength;
        (add_noise, temp)
    }
}

impl Default for MCTSConfig {
    fn default() -> Self {
        Self {
            c_puct: 1.5,
            gamma: 0.995,
            n_simulations: 200,
            dirichlet_alpha: 0.3,
            dirichlet_eps: 0.25,
            explore_by_entropy: false,
            temp_high: 1.0,
            temp_low: 0.2,
        }
    }
}

pub struct MCTSSearch {
    pub config: MCTSConfig,
}

/// Kết quả lá trong MCTS batch (để backprop biết cách áp value / mở rộng node).
/// - Terminal: lá kết thúc (game over / hết nước đi), value = 0.
/// - Expand: lá cần mở rộng (chưa expanded) với children (priors) và value từ NN.
/// - StoredValue: lá đã expanded nhưng traversal dừng lại, dùng value hiện tại.
/// - PendingEval: tạm thời đánh dấu lá chưa có GPU result (sẽ được chuyển thành `Expand`).
enum LeafEval {
    Terminal,
    Expand { children: Vec<(Action, MCTSNode)>, value: f32 },
    StoredValue(f32),
    PendingExtract,
    PendingEval,
}

#[derive(Clone)]
enum IndexedLeafEval {
    Terminal,
    StoredValue(f32),
    PendingExpand(usize),
}

struct IndexedSimPath {
    node_path: Vec<usize>,
    rewards: Vec<f32>,
    leaf_eval: IndexedLeafEval,
}


impl MCTSSearch {
    pub fn new(config: MCTSConfig) -> Self {
        Self { config }
    }

    /// Tính Dirichlet Alpha thích ứng theo số lượng action hợp lệ |A|.
    /// Theo AlphaZero (Silver et al.), alpha xấp xỉ tỉ lệ nghịch với |A|: alpha ≈ 10.0 / |A|
    /// Giới hạn trong khoảng [0.05, 0.35] để đảm bảo phân phối hợp lý cả khi ít hay nhiều actions.
    pub fn get_adaptive_dirichlet_alpha(num_actions: usize, base_alpha: f32) -> f32 {
        if num_actions <= 1 {
            return base_alpha;
        }
        (10.0 / num_actions as f32).clamp(0.05, 0.35)
    }

    /// Tạo mẫu Dirichlet Noise cho Policy Prior tại Node Gốc
    fn sample_dirichlet(k: usize, alpha: f32) -> Vec<f32> {
        if k == 0 {
            return Vec::new();
        }
        let mut rng = rand::thread_rng();
        let gamma_dist = Gamma::new(alpha, 1.0).unwrap();
        let samples: Vec<f32> = (0..k).map(|_| gamma_dist.sample(&mut rng) as f32).collect();
        let sum: f32 = samples.iter().sum::<f32>().max(1e-8);
        samples.into_iter().map(|s| s / sum).collect()
    }

    /// Entropy Shannon chuẩn hóa của một phân phối xác suất: H / ln(n) ∈ [0, 1].
    /// 0 = one-hot, 1 = uniform. Dùng làm tín hiệu độ tự tin của model tại root.
    fn normalized_entropy(p: &[f32]) -> f32 {
        if p.len() <= 1 {
            return 0.0;
        }
        let mut h = 0.0f32;
        for &x in p {
            if x > 1e-8 {
                h -= x * x.ln();
            }
        }
        (h / (p.len() as f32).ln()).clamp(0.0, 1.0)
    }

    /// Thực hiện MCTS Search với số lượt simulations quy định.
    /// Dùng save_checkpoint/restore_checkpoint thay vì clone env.
    /// Trả về: (Phân phối xác suất π_mcts chuẩn hóa tau=1.0, Action index được chọn, Action được chọn, Giá trị ước tính Value tại Root)
    pub fn search(
        &self,
        env: &mut DorfromantikEnv,
        model: &HexGNNModel,
        add_dirichlet: bool,
        temperature: f32,
    ) -> (Vec<f32>, usize, Action, f32) {
        let obs = env.extract_graph_observation();
        let num_actions = obs.valid_actions.len();

        if num_actions == 0 {
            let default_act = Action { q: 0, r: 0, rotation: 0 };
            return (Vec::new(), 0, default_act, 0.0);
        }

        // 1. Khởi tạo Root Node và Expand bằng Neural Network
        let (action_logits, root_val) = model.forward(
            &obs.node_positions,
            &obs.node_features,
            &obs.edge_index,
            &obs.valid_actions,
            &obs.action_features,
        );

        let max_logit = action_logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = action_logits.iter().map(|l| (l - max_logit).exp()).collect();
        let sum_exp: f32 = exps.iter().sum::<f32>().max(1e-8);
        let mut priors: Vec<f32> = exps.iter().map(|e| e / sum_exp).collect();

        // Nếu bật explore_by_entropy: MCTS tự quyết định chen noise + temperature từ prior entropy
        // (thay cho việc caller truyền add_dirichlet/temperature cứng nhắc). Chỉ áp dụng một cách
        // tự động khi caller đang yêu cầu exploration (add_dirichlet=true); còn eval (false, temp 0)
        // luôn giữ greedy để đánh giá ổn định.
        let (add_noise, temperature) = if self.config.explore_by_entropy && add_dirichlet && num_actions > 1 {
            let prior_e = Self::normalized_entropy(&priors);
            self.config.entropy_explore(prior_e)
        } else {
            (add_dirichlet, temperature)
        };

        if add_noise && num_actions > 1 {
            let alpha = Self::get_adaptive_dirichlet_alpha(num_actions, self.config.dirichlet_alpha);
            let noise = Self::sample_dirichlet(num_actions, alpha);
            let eps = self.config.dirichlet_eps;
            for i in 0..num_actions {
                priors[i] = (1.0 - eps) * priors[i] + eps * noise[i];
            }
        }

        let mut root = MCTSNode::new(1.0);
        root.is_expanded = true;
        root.children = obs.valid_actions
            .iter()
            .zip(priors.iter())
            .map(|(&act, &p)| (act, MCTSNode::new(p)))
            .collect();

        let mut q_min = f32::INFINITY;
        let mut q_max = f32::NEG_INFINITY;

        // Lưu checkpoint gốc (trước khi bắt đầu simulations)
        let root_cp = env.save_root_checkpoint();

        // 2. Chạy N lượt MCTS Simulations với undo
        for _ in 0..self.config.n_simulations {
            // Khôi phục về trạng thái gốc
            env.restore_checkpoint(root_cp.clone());

            let mut node_path: Vec<usize> = Vec::new();
            let mut step_rewards: Vec<f32> = Vec::new();
            let mut terminal_leaf = false;

            // --- SELECTION PHASE ---
            let mut curr: &MCTSNode = &root;
            while curr.is_expanded && !curr.children.is_empty() && !curr.is_terminal {
                let total_n = curr.children.iter().map(|(_, c)| c.visit_count).sum::<u32>() as f32;
                let sqrt_n = (total_n).sqrt();

                let mut best_idx = 0;
                let mut best_ucb = f32::NEG_INFINITY;

                for (idx, (_, child)) in curr.children.iter().enumerate() {
                    let q_val = if child.visit_count > 0 {
                        let q = child.q_value();
                        if q_max > q_min + 1e-6 {
                            (q - q_min) / (q_max - q_min)
                        } else {
                            0.5
                        }
                    } else {
                        0.0
                    };

                    let ucb = q_val + self.config.c_puct * child.prior * sqrt_n / (1.0 + child.visit_count as f32);
                    if ucb > best_ucb {
                        best_ucb = ucb;
                        best_idx = idx;
                    }
                }

                let (chosen_action, _) = curr.children[best_idx];
                let res = env.step(chosen_action);
                let scaled_r = res.reward * 0.01;

                node_path.push(best_idx);
                step_rewards.push(scaled_r);

                if res.done {
                    terminal_leaf = true;
                    break;
                }

                curr = &curr.children[best_idx].1;
            }

            // --- EXPANSION & EVALUATION PHASE ---
            // Tìm node lá trong cây (đi theo path)
            let leaf_node = {
                let mut leaf: &mut MCTSNode = &mut root;
                for &idx in &node_path {
                    leaf = &mut leaf.children[idx].1;
                }
                leaf
            };

            if terminal_leaf {
                leaf_node.is_terminal = true;
            }

            let leaf_value = if leaf_node.is_terminal {
                0.0
            } else if !leaf_node.is_expanded {
                let leaf_obs = env.extract_graph_observation();
                if leaf_obs.valid_actions.is_empty() {
                    leaf_node.is_terminal = true;
                    0.0
                } else {
                    let (leaf_logits, val) = model.forward(
                        &leaf_obs.node_positions,
                        &leaf_obs.node_features,
                        &leaf_obs.edge_index,
                        &leaf_obs.valid_actions,
                        &leaf_obs.action_features,
                    );

                    let l_max = leaf_logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                    let l_exps: Vec<f32> = leaf_logits.iter().map(|l| (l - l_max).exp()).collect();
                    let l_sum: f32 = l_exps.iter().sum::<f32>().max(1e-8);
                    let l_probs: Vec<f32> = l_exps.iter().map(|e| e / l_sum).collect();

                    leaf_node.is_expanded = true;
                    leaf_node.children = leaf_obs.valid_actions
                        .iter()
                        .zip(l_probs.iter())
                        .map(|(&act, &p)| (act, MCTSNode::new(p)))
                        .collect();

                    val
                }
            } else {
                leaf_node.q_value()
            };

            // --- BACKPROPAGATION (BACKUP) PHASE ---
            let mut g = leaf_value;
            let depth = node_path.len();

            let mut returns = vec![0.0f32; depth];
            for d in (0..depth).rev() {
                g = step_rewards[d] + self.config.gamma * g;
                returns[d] = g;
            }

            root.visit_count += 1;
            root.total_value += g;

            for d in 0..depth {
                let child_idx = node_path[d];
                let child = &mut root.children[child_idx].1;
                child.visit_count += 1;
                child.total_value += returns[d];

                let q = child.q_value();
                if q < q_min { q_min = q; }
                if q > q_max { q_max = q; }
            }
        }

        // Khôi phục env về root sau tất cả simulations
        env.restore_checkpoint(root_cp);

        // 3. Tính Target Policy π_mcts chuẩn (tau = 1.0) cho Replay Buffer / Training
        let visit_counts: Vec<f32> = root.children.iter().map(|(_, c)| c.visit_count as f32).collect();
        let total_visits: f32 = visit_counts.iter().sum::<f32>().max(1.0);
        let target_pi: Vec<f32> = visit_counts.iter().map(|&v| v / total_visits).collect();

        // 4. Chọn Action: áp dụng temperature cho sampling
        let chosen_idx = if temperature <= 1e-3 {
            // Greedy: Chọn Action có Visit Count cao nhất
            visit_counts
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx)
                .unwrap_or(0)
        } else {
            // Softmax Temperature trên Visit Counts: N(a)^(1/tau)
            let powered: Vec<f32> = visit_counts.iter().map(|&v| (v / total_visits).powf(1.0 / temperature)).collect();
            let sum_pow: f32 = powered.iter().sum::<f32>().max(1e-8);
            let sample_probs: Vec<f32> = powered.iter().map(|p| p / sum_pow).collect();

            let mut rng = rand::thread_rng();
            let r: f32 = rng.gen_range(0.0..1.0);
            let mut cum = 0.0f32;
            let mut selected = num_actions - 1;
            for (i, &p) in sample_probs.iter().enumerate() {
                cum += p;
                if r <= cum {
                    selected = i;
                    break;
                }
            }
            selected
        };

        let chosen_action = root.children[chosen_idx].0;
        (target_pi, chosen_idx, chosen_action, root_val)
    }

    /// Vectorized Batch MCTS Search cho danh sách các active envs theo index (Zero Env-Clone & Zero HashMap overhead)
    /// Trả về: Vec<(pi_probs, chosen_idx, chosen_action, root_val, root_obs)>
    pub fn search_batch_indexed(
        &self,
        all_envs: &[DorfromantikEnv],
        active_indices: &[usize],
        model: &HexGNNModel,
        gpu_exec: Option<&GpuNNExecutor>,
        add_dirichlet: bool,
        temperature: f32,
    ) -> Vec<(Vec<f32>, usize, Action, f32, crate::env::GraphObservation)> {
        let b_count = active_indices.len();
        if b_count == 0 {
            return Vec::new();
        }

        let obs_batch: Vec<crate::env::GraphObservation> = active_indices
            .iter()
            .map(|&idx| all_envs[idx].extract_graph_observation())
            .collect();
        let obs_refs: Vec<&crate::env::GraphObservation> = obs_batch.iter().collect();

        let root_evals = if let Some(gpu) = gpu_exec {
            gpu.forward_batch_gpu(&obs_refs)
        } else {
            model.forward_batch(&obs_refs)
        };

        let mut roots: Vec<Option<MCTSNode>> = Vec::with_capacity(b_count);
        let mut root_vals: Vec<f32> = Vec::with_capacity(b_count);
        let mut q_mins: Vec<f32> = vec![f32::INFINITY; b_count];
        let mut q_maxs: Vec<f32> = vec![f32::NEG_INFINITY; b_count];
        // Nếu bật explore_by_entropy: mỗi env tự quyết nhiệt độ theo prior entropy của chính nó.
        let mut root_temps: Vec<f32> = vec![temperature; b_count];

        for (i, obs) in obs_batch.iter().enumerate() {
            let num_actions = obs.valid_actions.len();
            if num_actions == 0 {
                roots.push(None);
                root_vals.push(0.0);
                continue;
            }

            let (action_logits, root_val) = &root_evals[i];
            root_vals.push(*root_val);

            let max_logit = action_logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let exps: Vec<f32> = action_logits.iter().map(|l| (l - max_logit).exp()).collect();
            let sum_exp: f32 = exps.iter().sum::<f32>().max(1e-8);
            let mut priors: Vec<f32> = exps.iter().map(|e| e / sum_exp).collect();

            // Quyết định noise + nhiệt độ theo prior entropy (per-env), nếu bật explore_by_entropy
            // và caller đang yêu cầu exploration (add_dirichlet=true). Eval/refresh (false) luôn greedy.
            let (add_noise, this_temp) = if self.config.explore_by_entropy && add_dirichlet && num_actions > 1 {
                let prior_e = Self::normalized_entropy(&priors);
                self.config.entropy_explore(prior_e)
            } else {
                (add_dirichlet, temperature)
            };
            root_temps[i] = this_temp;

            if add_noise && num_actions > 1 {
                let alpha = Self::get_adaptive_dirichlet_alpha(num_actions, self.config.dirichlet_alpha);
                let noise = Self::sample_dirichlet(num_actions, alpha);
                let eps = self.config.dirichlet_eps;
                for a in 0..num_actions {
                    priors[a] = (1.0 - eps) * priors[a] + eps * noise[a];
                }
            }

            let mut root = MCTSNode::new(1.0);
            root.is_expanded = true;
            root.children = obs.valid_actions
                .iter()
                .zip(priors.iter())
                .map(|(&act, &p)| (act, MCTSNode::new(p)))
                .collect();

            roots.push(Some(root));
        }

        // 2. Chạy N lượt MCTS Simulations (Multi-Virtual-Loss Double-Buffering Pipelined nếu có GPU executor)
        let k_vloss = std::env::var("DORFO_VLOSS_K")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(4)
            .max(1);
        let num_rounds = (self.config.n_simulations + k_vloss - 1) / k_vloss;

        let clone_ns = std::sync::atomic::AtomicU64::new(0);
        let step_ns = std::sync::atomic::AtomicU64::new(0);
        let ucb_ns = std::sync::atomic::AtomicU64::new(0);
        let obs_ns = std::sync::atomic::AtomicU64::new(0);
        let perf = std::env::var("DORFO_PERF").is_ok();
        if let Some(gpu) = gpu_exec {
            if perf {
                use std::io::Write;
                let _ = writeln!(
                    std::io::stdout(),
                    "[PERF] mode=GPU batch={} rounds={} vloss_k={}",
                    b_count, self.config.n_simulations, k_vloss
                );
                let _ = std::io::stdout().flush();
            }
            let mut t_trav_total = std::time::Duration::ZERO;
            let mut t_gpu_total = std::time::Duration::ZERO;
            let mut t_bp_total = std::time::Duration::ZERO;
            let t_search0 = std::time::Instant::now();

            if b_count >= 4 {
                let mid = b_count / 2;
                let range_a = 0..mid;
                let range_b = mid..b_count;

                // Khởi động: CPU duyệt UCB Group A cho round 0 và gửi lệnh GPU Async vào Slot 0
                let init_k = self.config.n_simulations.min(k_vloss);
                let (mut sim_res_a, mut leaf_obs_a) = self.run_sim_traversal_range(
                    range_a.clone(),
                    &mut roots,
                    all_envs,
                    active_indices,
                    &q_mins,
                    &q_maxs,
                    init_k,
                    &clone_ns,
                    &step_ns,
                    &ucb_ns,
                    &obs_ns,
                );
                let mut pending_a = if !leaf_obs_a.is_empty() {
                    let leaf_refs_a: Vec<&crate::env::GraphObservation> = leaf_obs_a.iter().collect();
                    gpu.forward_batch_gpu_async_slot(0, &leaf_refs_a)
                } else {
                    None
                };

                for r in 0..num_rounds {
                    let this_k = (self.config.n_simulations - r * k_vloss).min(k_vloss);

                    // CPU chạy Rayon MCTS Traversal cho Group B trong lúc GPU đang tính Slot 0!
                    let ts = std::time::Instant::now();
                    let (sim_res_b, leaf_obs_b) = self.run_sim_traversal_range(
                        range_b.clone(),
                        &mut roots,
                        all_envs,
                        active_indices,
                        &q_mins,
                        &q_maxs,
                        this_k,
                        &clone_ns,
                        &step_ns,
                        &ucb_ns,
                        &obs_ns,
                    );
                    t_trav_total += ts.elapsed();

                    // Nhận kết quả GPU Group A (Slot 0)
                    let tg = std::time::Instant::now();
                    let results_a = if let Some(p_a) = pending_a {
                        p_a.wait(&gpu.device)
                    } else {
                        Vec::new()
                    };
                    t_gpu_total += tg.elapsed();

                    // Gửi ngay lệnh GPU Async cho Group B vào Slot 1 (Early Skip nếu không có lá)
                    let pending_b = if !leaf_obs_b.is_empty() {
                        let leaf_refs_b: Vec<&crate::env::GraphObservation> = leaf_obs_b.iter().collect();
                        gpu.forward_batch_gpu_async_slot(1, &leaf_refs_b)
                    } else {
                        None
                    };

                    // CPU Backprop cho Group A
                    let tsb = std::time::Instant::now();
                    self.run_sim_backprop_range(range_a.clone(), &sim_res_a, &leaf_obs_a, &results_a, &mut roots, &mut q_mins, &mut q_maxs);
                    t_bp_total += tsb.elapsed();

                    // Nếu còn round tiếp theo, CPU chạy Traversal Group A trong lúc GPU đang tính Slot 1
                    if r + 1 < num_rounds {
                        let next_k = (self.config.n_simulations - (r + 1) * k_vloss).min(k_vloss);
                        let ts = std::time::Instant::now();
                        let (next_sim_res_a, next_leaf_obs_a) = self.run_sim_traversal_range(
                            range_a.clone(),
                            &mut roots,
                            all_envs,
                            active_indices,
                            &q_mins,
                            &q_maxs,
                            next_k,
                            &clone_ns,
                            &step_ns,
                            &ucb_ns,
                            &obs_ns,
                        );
                        t_trav_total += ts.elapsed();

                        // Nhận kết quả GPU Group B (Slot 1)
                        let tg = std::time::Instant::now();
                        let results_b = if let Some(p_b) = pending_b {
                            p_b.wait(&gpu.device)
                        } else {
                            Vec::new()
                        };
                        t_gpu_total += tg.elapsed();

                        // Gửi ngay lệnh GPU Async cho Group A vào Slot 0 lượt kế tiếp
                        pending_a = if !next_leaf_obs_a.is_empty() {
                            let next_leaf_refs_a: Vec<&crate::env::GraphObservation> = next_leaf_obs_a.iter().collect();
                            gpu.forward_batch_gpu_async_slot(0, &next_leaf_refs_a)
                        } else {
                            None
                        };

                        // CPU Backprop cho Group B
                        let tsb = std::time::Instant::now();
                        self.run_sim_backprop_range(range_b.clone(), &sim_res_b, &leaf_obs_b, &results_b, &mut roots, &mut q_mins, &mut q_maxs);
                        t_bp_total += tsb.elapsed();

                        sim_res_a = next_sim_res_a;
                        leaf_obs_a = next_leaf_obs_a;
                    } else {
                        // Lượt cuối: nhận kết quả GPU Group B và Backprop Group B
                        let tg = std::time::Instant::now();
                        let results_b = if let Some(p_b) = pending_b {
                            p_b.wait(&gpu.device)
                        } else {
                            Vec::new()
                        };
                        t_gpu_total += tg.elapsed();

                        let tsb = std::time::Instant::now();
                        self.run_sim_backprop_range(range_b.clone(), &sim_res_b, &leaf_obs_b, &results_b, &mut roots, &mut q_mins, &mut q_maxs);
                        t_bp_total += tsb.elapsed();

                        pending_a = None;
                    }
                }
            } else {
                // Fallback đơn lẻ nếu b_count < 4
                for r in 0..num_rounds {
                    let this_k = (self.config.n_simulations - r * k_vloss).min(k_vloss);
                    let (sim_res, leaf_obs) = self.run_sim_traversal_range(
                        0..b_count,
                        &mut roots,
                        all_envs,
                        active_indices,
                        &q_mins,
                        &q_maxs,
                        this_k,
                        &clone_ns,
                        &step_ns,
                        &ucb_ns,
                        &obs_ns,
                    );
                    let results = if !leaf_obs.is_empty() {
                        let leaf_refs: Vec<&crate::env::GraphObservation> = leaf_obs.iter().collect();
                        gpu.forward_batch_gpu(&leaf_refs)
                    } else {
                        Vec::new()
                    };
                    self.run_sim_backprop_range(0..b_count, &sim_res, &leaf_obs, &results, &mut roots, &mut q_mins, &mut q_maxs);
                }
            }
            if perf {
                use std::io::Write;
                let wall = t_search0.elapsed().as_secs_f64();
                let _ = writeln!(
                    std::io::stdout(),
                    "[PERF] search_batch_indexed mode=GPU rounds={} vloss_k={} wall={:.3}s trav={:.3}s gpu_wait={:.3}s backprop={:.3}s | clone={:.3}s ucb={:.3}s step={:.3}s obs_ext={:.3}s",
                    self.config.n_simulations, k_vloss, wall,
                    t_trav_total.as_secs_f64(),
                    t_gpu_total.as_secs_f64(),
                    t_bp_total.as_secs_f64(),
                    clone_ns.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e9,
                    ucb_ns.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e9,
                    step_ns.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e9,
                    obs_ns.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e9,
                );
                let _ = std::io::stdout().flush();
            }
        } else {
            // CPU fallback nếu không có GPU
            let t_search0 = std::time::Instant::now();
            let mut t_trav_total = std::time::Duration::ZERO;
            let mut t_bp_total = std::time::Duration::ZERO;
            for r in 0..num_rounds {
                let this_k = (self.config.n_simulations - r * k_vloss).min(k_vloss);
                let ts = std::time::Instant::now();
                let (sim_res, leaf_obs) = self.run_sim_traversal_range(
                    0..b_count,
                    &mut roots,
                    all_envs,
                    active_indices,
                    &q_mins,
                    &q_maxs,
                    this_k,
                    &clone_ns,
                    &step_ns,
                    &ucb_ns,
                    &obs_ns,
                );
                t_trav_total += ts.elapsed();
                let results = if !leaf_obs.is_empty() {
                    let leaf_refs: Vec<&crate::env::GraphObservation> = leaf_obs.iter().collect();
                    model.forward_batch(&leaf_refs)
                } else {
                    Vec::new()
                };
                let tsb = std::time::Instant::now();
                self.run_sim_backprop_range(0..b_count, &sim_res, &leaf_obs, &results, &mut roots, &mut q_mins, &mut q_maxs);
                t_bp_total += tsb.elapsed();
            }
            if perf {
                use std::io::Write;
                let wall = t_search0.elapsed().as_secs_f64();
                let _ = writeln!(
                    std::io::stdout(),
                    "[PERF] search_batch_indexed mode=CPU rounds={} vloss_k={} wall={:.3}s trav={:.3}s nn+backprop={:.3}s",
                    self.config.n_simulations, k_vloss, wall, t_trav_total.as_secs_f64(), t_bp_total.as_secs_f64(),
                );
                let _ = std::io::stdout().flush();
            }
        }

        let mut results = Vec::with_capacity(b_count);
        for (i, obs) in obs_batch.into_iter().enumerate() {
            let num_actions = obs.valid_actions.len();

            if num_actions == 0 || roots[i].is_none() {
                let default_act = Action { q: 0, r: 0, rotation: 0 };
                results.push((Vec::new(), 0, default_act, 0.0, obs));
                continue;
            }

            let root = roots[i].as_ref().unwrap();
            let root_val = root_vals[i];

            let visit_counts: Vec<f32> = root.children.iter().map(|(_, c)| c.visit_count as f32).collect();
            let total_visits: f32 = visit_counts.iter().sum::<f32>().max(1.0);
            // 1. Target Policy chuẩn cho huấn luyện (luôn là tỉ lệ Visit Counts thuần túy, tau = 1.0)
            let target_pi: Vec<f32> = visit_counts.iter().map(|&v| v / total_visits).collect();

            // 2. Dùng nhiệt độ đã quyết định riêng cho env này để lấy mẫu nước đi (sampling)
            let pi_temperature = root_temps[i];

            let chosen_idx = if pi_temperature <= 1e-3 {
                visit_counts
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                    .map(|(idx, _)| idx)
                    .unwrap_or(0)
            } else {
                let powered: Vec<f32> = visit_counts.iter().map(|&v| (v / total_visits).powf(1.0 / pi_temperature)).collect();
                let sum_pow: f32 = powered.iter().sum::<f32>().max(1e-8);
                let sample_probs: Vec<f32> = powered.iter().map(|p| p / sum_pow).collect();

                let mut rng = rand::thread_rng();
                let r: f32 = rng.gen_range(0.0..1.0);
                let mut cum = 0.0f32;
                let mut selected = num_actions - 1;
                for (k, &p) in sample_probs.iter().enumerate() {
                    cum += p;
                    if r <= cum {
                        selected = k;
                        break;
                    }
                }
                selected
            };

            let chosen_action = root.children[chosen_idx].0;
            results.push((target_pi, chosen_idx, chosen_action, root_val, obs));
        }

        results
    }

    /// MCTS Batch Search cho 1 env ĐƠN LẺ dùng Virtual Loss (node ảo) để thăm dò nhiều lá
    /// trong cùng 1 lượt, gom thành 1 GPU batch leaf-eval để GPU được dùng triệt để.
    ///
    /// Thay vì chạy N lượt, mỗi lượt eval 1 lá (hàng nghìn round-trip GPU), hàm này chạy
    /// `batch_size` lượt traversal tuần tự trên CÙNG 1 cây MCTS, đánh virtual loss lên từng
    /// đường đi để các lượt kế tiếp trong round phân tán sang nhánh khác, rồi gom toàn bộ
    /// các lá cần mở rộng thành 1 batch GPU eval duy nhất. Việc trích xuất GraphObservation
    /// của các lá được song song hóa bằng Rayon (nặng: quét candidate + group queries).
    pub fn search_virtual_loss_batch(
        &self,
        env: &DorfromantikEnv,
        model: &HexGNNModel,
        gpu_exec: Option<&GpuNNExecutor>,
        add_dirichlet: bool,
        temperature: f32,
        batch_size: usize,
    ) -> (Vec<f32>, usize, Action, f32) {
        use rayon::prelude::*;

        let obs = env.extract_graph_observation();
        let num_actions = obs.valid_actions.len();
        if num_actions == 0 {
            let default_act = Action { q: 0, r: 0, rotation: 0 };
            return (Vec::new(), 0, default_act, 0.0);
        }

        // 1. Eval Root qua GPU batch (batch 1 phần tử)
        let root_refs = [&obs];
        let root_evals = if let Some(gpu) = gpu_exec {
            gpu.forward_batch_gpu(&root_refs)
        } else {
            model.forward_batch(&root_refs)
        };
        let (action_logits, root_val) = root_evals[0].clone();

        let max_logit = action_logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = action_logits.iter().map(|l| (l - max_logit).exp()).collect();
        let sum_exp: f32 = exps.iter().sum::<f32>().max(1e-8);
        let mut priors: Vec<f32> = exps.iter().map(|e| e / sum_exp).collect();

        // Quyết định noise + nhiệt độ theo prior entropy, nếu bật explore_by_entropy
        // và caller đang yêu cầu exploration (add_dirichlet=true). Eval/replay (false) luôn greedy.
        let (add_noise, temperature) = if self.config.explore_by_entropy && add_dirichlet && num_actions > 1 {
            let prior_e = Self::normalized_entropy(&priors);
            self.config.entropy_explore(prior_e)
        } else {
            (add_dirichlet, temperature)
        };

        if add_noise && num_actions > 1 {
            let alpha = Self::get_adaptive_dirichlet_alpha(num_actions, self.config.dirichlet_alpha);
            let noise = Self::sample_dirichlet(num_actions, alpha);
            let eps = self.config.dirichlet_eps;
            for a in 0..num_actions {
                priors[a] = (1.0 - eps) * priors[a] + eps * noise[a];
            }
        }

        let mut root = MCTSNode::new(1.0);
        root.is_expanded = true;
        root.children = obs.valid_actions
            .iter()
            .zip(priors.iter())
            .map(|(&act, &p)| (act, MCTSNode::new(p)))
            .collect();

        let mut q_min_f = f32::INFINITY;
        let mut q_max_f = f32::NEG_INFINITY;

        const VLOSS: f32 = 3.0;
        let bs = batch_size.max(1);
        let mut done_count = 0usize;

        while done_count < self.config.n_simulations {
            let round_bs = bs.min(self.config.n_simulations - done_count);

            // path_sims[s] = (env tại lá, node_path, rewards) cho lượt s
            let mut path_sims: Vec<(DorfromantikEnv, Vec<usize>, Vec<f32>)> = Vec::with_capacity(round_bs);
            // eval_spec[s] = mô tả kết quả lá của lượt s
            let mut eval_spec: Vec<LeafEval> = Vec::with_capacity(round_bs);
            // các sim có lá cần trích xuất obs (được parallel hóa ở Phase B)
            let mut pending_extract: Vec<usize> = Vec::new();

            // ---- Phase A: round_bs lượt traversal tuần tự, đánh virtual loss mỗi lượt ----
            for s in 0..round_bs {
                let mut sim_env = env.clone();
                let mut node_path: Vec<usize> = Vec::new();
                let mut rewards: Vec<f32> = Vec::new();

                // SELECTION (read-only; cây đang tính cả virtual loss của các lượt trước trong round)
                let mut curr: &MCTSNode = &root;
                let mut traversal_done = false;
                loop {
                    if curr.is_terminal || !curr.is_expanded || curr.children.is_empty() {
                        break;
                    }
                    let total_n = curr.children.iter().map(|(_, c)| c.visit_count).sum::<u32>() as f32;
                    let sqrt_n = total_n.sqrt();
                    let mut best_idx = 0usize;
                    let mut best_ucb = f32::NEG_INFINITY;
                    for (idx, (_, child)) in curr.children.iter().enumerate() {
                        let q_val = if child.visit_count > 0 {
                            let q = child.q_value();
                            if q_max_f > q_min_f + 1e-6 {
                                (q - q_min_f) / (q_max_f - q_min_f)
                            } else {
                                0.5
                            }
                        } else {
                            0.0
                        };
                        let ucb = q_val
                            + self.config.c_puct * child.prior * sqrt_n
                                / (1.0 + child.visit_count as f32);
                        if ucb > best_ucb {
                            best_ucb = ucb;
                            best_idx = idx;
                        }
                    }

                    let (chosen_action, _) = curr.children[best_idx];
                    let res = sim_env.step(chosen_action);
                    let scaled_r = res.reward * 0.01;
                    node_path.push(best_idx);
                    rewards.push(scaled_r);
                    curr = &curr.children[best_idx].1;
                    if res.done {
                        traversal_done = true;
                        break;
                    }
                }

                // Phân loại lá: terminal | cần trích xuất obs (pending) | stored value
                if traversal_done || curr.is_terminal {
                    eval_spec.push(LeafEval::Terminal);
                } else if !curr.is_expanded {
                    eval_spec.push(LeafEval::PendingExtract);
                    pending_extract.push(s);
                } else {
                    eval_spec.push(LeafEval::StoredValue(curr.q_value()));
                }

                // Đánh virtual loss lên đường đi (CHỈ trên các node con trong path)
                Self::apply_virtual_loss(&mut root, &node_path, VLOSS);

                path_sims.push((sim_env, node_path, rewards));
            }

            // ---- Phase B-0: trích xuất GraphObservation song song cho các lá pending ----
            // (parallel hóa cost nặng của extract_graph_observation)
            let extracted: Vec<(usize, crate::env::GraphObservation)> = pending_extract
                .par_iter()
                .filter_map(|&s| {
                    let env_clone = &path_sims[s].0;
                    let leaf_obs = env_clone.extract_graph_observation();
                    if leaf_obs.valid_actions.is_empty() { None } else { Some((s, leaf_obs)) }
                })
                .collect();
            for (s, _) in &extracted {
                eval_spec[*s] = LeafEval::PendingEval; // đánh dấu cần eval (đã có obs thật)
            }
            // các pending không nằm trong extracted => hết nước đi => Terminal
            for &s in &pending_extract {
                if matches!(eval_spec[s], LeafEval::PendingExtract) {
                    eval_spec[s] = LeafEval::Terminal;
                }
            }

            // ---- Phase B: gom tất cả lá PendingEval thành 1 GPU batch ----
            let mut layer_obs: Vec<crate::env::GraphObservation> = Vec::new();
            let mut layer_positions: Vec<usize> = Vec::new();
            for &(s, ref leaf_obs) in &extracted {
                layer_obs.push(leaf_obs.clone());
                layer_positions.push(s);
            }

            let mut layer_res: Vec<(Vec<f32>, f32)> = Vec::new();
            if !layer_obs.is_empty() {
                let leaf_refs: Vec<&crate::env::GraphObservation> = layer_obs.iter().collect();
                layer_res = if let Some(gpu) = gpu_exec {
                    gpu.forward_batch_gpu(&leaf_refs)
                } else {
                    model.forward_batch(&leaf_refs)
                };
            }

            // Chuyển PendingEval (đã có obs + GPU result) thành Expand
            for (pos, &sidx) in layer_positions.iter().enumerate() {
                if let LeafEval::PendingEval = eval_spec[sidx] {
                    let gpu_r = layer_res.get(pos).cloned().unwrap_or_else(|| {
                        (vec![0.0f32; layer_obs[pos].valid_actions.len()], 0.0f32)
                    });
                    let (logits, val) = gpu_r;
                    let obs = &layer_obs[pos];
                    let l_max = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                    let l_exps: Vec<f32> = logits.iter().map(|l| (l - l_max).exp()).collect();
                    let l_sum: f32 = l_exps.iter().sum::<f32>().max(1e-8);
                    let l_probs: Vec<f32> = l_exps.iter().map(|e| e / l_sum).collect();
                    let children: Vec<(Action, MCTSNode)> = obs.valid_actions
                        .iter()
                        .zip(l_probs.iter())
                        .map(|(&act, &p)| (act, MCTSNode::new(p)))
                        .collect();
                    eval_spec[sidx] = LeafEval::Expand { children, value: val };
                }
            }

            // ---- Phase C: Backprop từng lượt (gỡ virtual loss, áp value thật, mở rộng lá) ----
            for s in 0..round_bs {
                let (_, node_path, rewards) = &path_sims[s];
                let leaf_eval = std::mem::replace(&mut eval_spec[s], LeafEval::Terminal);
                Self::backprop_single(
                    &mut root, node_path, rewards,
                    leaf_eval, VLOSS, self.config.gamma,
                    &mut q_min_f, &mut q_max_f,
                );
            }

            done_count += round_bs;
        }

        // ---- Chọn action cuối (greedy theo visit count) ----
        let visit_counts: Vec<f32> = root.children.iter().map(|(_, c)| c.visit_count as f32).collect();
        let total_visits: f32 = visit_counts.iter().sum::<f32>().max(1.0);

        if std::env::var("DORFO_DEBUG").is_ok() {
            // In phân bố priors + visit_counts top-5 để xem cây MCTS có khám phá đa nhánh không
            let mut entries: Vec<(f32, f32)> = root.children
                .iter()
                .zip(visit_counts.iter())
                .map(|((_, c), &v)| (c.prior, v))
                .collect();
            entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let entrop = {
                let s: f32 = visit_counts.iter().sum::<f32>().max(1.0);
                let mut e = 0.0f32;
                for &v in &visit_counts {
                    if v > 0.0 {
                        let p = v / s;
                        e -= p * p.ln();
                    }
                }
                e
            };
            eprintln!("[DEBUG] sims={} total_visits={} entropy={:.3} | top5(vis,p): {:?}",
                self.config.n_simulations, total_visits, entrop,
                entries.iter().take(5).map(|&(p, v)| (v, (p * 100.0).round() as i32)).collect::<Vec<_>>());
        }


        // 1. Target Policy chuẩn cho huấn luyện (luôn là tỉ lệ Visit Counts thuần túy, tau = 1.0)
        let target_pi: Vec<f32> = visit_counts.iter().map(|&v| v / total_visits).collect();

        // 2. Dùng nhiệt độ để lấy mẫu nước đi (sampling)
        let chosen_idx = if temperature <= 1e-3 {
            visit_counts
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx)
                .unwrap_or(0)
        } else {
            let powered: Vec<f32> = visit_counts.iter().map(|&v| (v / total_visits).powf(1.0 / temperature)).collect();
            let sum_pow: f32 = powered.iter().sum::<f32>().max(1e-8);
            let sample_probs: Vec<f32> = powered.iter().map(|p| p / sum_pow).collect();

            let mut rng = rand::thread_rng();
            let r: f32 = rng.gen_range(0.0..1.0);
            let mut cum = 0.0f32;
            let mut selected = num_actions - 1;
            for (k, &p) in sample_probs.iter().enumerate() {
                cum += p;
                if r <= cum {
                    selected = k;
                    break;
                }
            }
            selected
        };

        let chosen_action = root.children[chosen_idx].0;
        (target_pi, chosen_idx, chosen_action, root_val)
    }

    /// Đánh dấu Virtual Loss lên toàn bộ đường đi (tăng visit_count, giảm total_value)
    /// trên CÁC NODE CON trong path, để trong cùng 1 round các lượt traversal kế tiếp
    /// tránh trùng nhánh. Root không bị đánh (shared, không nên bị discourage).
    fn apply_virtual_loss(
        root: &mut MCTSNode,
        node_path: &[usize],
        vloss: f32,
    ) {
        let mut curr = &mut *root;
        for &idx in node_path {
            if idx < curr.children.len() {
                curr = &mut curr.children[idx].1;
                curr.visit_count += 1;
                curr.total_value -= vloss;
            }
        }
    }

    /// Backprop 1 lượt cho batch MCTS: gỡ virtual loss trên path, áp value thật (đã discount),
    /// và mở rộng lá nếu cần. Root chỉ nhận value thật (không virtual loss). Cập nhật q_min/q_max.
    fn backprop_single(
        root: &mut MCTSNode,
        node_path: &[usize],
        rewards: &[f32],
        leaf_eval: LeafEval,
        vloss: f32,
        gamma: f32,
        q_min: &mut f32,
        q_max: &mut f32,
    ) {
        // Vị trí node tại lá
        let mut curr = &mut *root;
        for &idx in node_path {
            if idx < curr.children.len() {
                curr = &mut curr.children[idx].1;
            } else {
                break;
            }
        }

        // Áp dụng outcome của lá lên node lá
        let leaf_value = match &leaf_eval {
            LeafEval::Terminal => {
                curr.is_terminal = true;
                0.0
            }
            LeafEval::Expand { children, value } => {
                if !curr.is_expanded {
                    curr.is_expanded = true;
                    curr.children = children.clone();
                }
                *value
            }
            LeafEval::StoredValue(v) => *v,
            LeafEval::PendingExtract | LeafEval::PendingEval => curr.q_value(),
        };

        // Sinh chuỗi discounted returns dọc theo path
        let depth = node_path.len();
        let mut returns = vec![0.0f32; depth];
        let mut g = leaf_value;
        for d in (0..depth).rev() {
            g = rewards[d] + gamma * g;
            returns[d] = g;
        }

        // Root: chỉ nhận value thật (+1 visit, +g). Không virtual loss vì root không bị đánh.
        root.visit_count += 1;
        root.total_value += g;
        let rq = root.q_value();
        if rq < *q_min { *q_min = rq; }
        if rq > *q_max { *q_max = rq; }

        // Các node con dọc path: gỡ virtual loss (1 lượt) rồi áp returns thật.
        let mut trav = &mut *root;
        for d in 0..depth {
            let idx = node_path[d];
            if idx < trav.children.len() {
                trav = &mut trav.children[idx].1;
                trav.visit_count = trav.visit_count.saturating_sub(1);
                trav.total_value += vloss;
                trav.visit_count += 1;
                trav.total_value += returns[d];

                let q = trav.q_value();
                if q < *q_min { *q_min = q; }
                if q > *q_max { *q_max = q; }
            }
        }
    }

    fn run_sim_traversal_range(
        &self,
        range: std::ops::Range<usize>,
        roots: &mut [Option<MCTSNode>],
        all_envs: &[DorfromantikEnv],
        active_indices: &[usize],
        q_mins: &[f32],
        q_maxs: &[f32],
        k_sims: usize,
        clone_ns: &std::sync::atomic::AtomicU64,
        step_ns: &std::sync::atomic::AtomicU64,
        ucb_ns: &std::sync::atomic::AtomicU64,
        obs_ns: &std::sync::atomic::AtomicU64,
    ) -> (
        Vec<Vec<IndexedSimPath>>,
        Vec<crate::env::GraphObservation>,
    ) {
        const VLOSS: f32 = 3.0;
        let range_len = range.len();
        let roots_slice = &mut roots[range.clone()];
        let q_mins_slice = &q_mins[range.clone()];
        let q_maxs_slice = &q_maxs[range.clone()];
        let active_slice = &active_indices[range];

        let env_results: Vec<(Vec<IndexedSimPath>, Vec<crate::env::GraphObservation>)> = roots_slice
            .par_iter_mut()
            .zip(q_mins_slice.par_iter())
            .zip(q_maxs_slice.par_iter())
            .zip(active_slice.par_iter())
            .map(|(((root_opt, &q_min), &q_max), &env_idx)| {
                if let Some(ref mut root) = root_opt {
                    let mut env_paths = Vec::with_capacity(k_sims);
                    let mut env_leaf_obs = Vec::new();

                    for _ in 0..k_sims {
                        let t0 = std::time::Instant::now();
                        let mut sim_env = all_envs[env_idx].clone();
                        clone_ns.fetch_add(t0.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);

                        let mut node_path = Vec::new();
                        let mut rewards = Vec::new();
                        let mut curr: &MCTSNode = root;

                        while curr.is_expanded && !curr.children.is_empty() && !curr.is_terminal {
                            let t_ucb0 = std::time::Instant::now();
                            let total_n = curr.children.iter().map(|(_, c)| c.visit_count).sum::<u32>() as f32;
                            let sqrt_n = total_n.sqrt();

                            let mut best_idx = 0;
                            let mut best_ucb = f32::NEG_INFINITY;

                            for (idx, (_, child)) in curr.children.iter().enumerate() {
                                let q_val = if child.visit_count > 0 {
                                    let q = child.q_value();
                                    if q_max > q_min + 1e-6 {
                                        (q - q_min) / (q_max - q_min)
                                    } else {
                                        0.5
                                    }
                                } else {
                                    0.0
                                };

                                let ucb = q_val + self.config.c_puct * child.prior * sqrt_n / (1.0 + child.visit_count as f32);
                                if ucb > best_ucb {
                                    best_ucb = ucb;
                                    best_idx = idx;
                                }
                            }
                            ucb_ns.fetch_add(t_ucb0.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);

                            let (chosen_action, _) = curr.children[best_idx];
                            let ts = std::time::Instant::now();
                            let res = sim_env.step(chosen_action);
                            step_ns.fetch_add(ts.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
                            let scaled_r = res.reward * 0.01;

                            node_path.push(best_idx);
                            rewards.push(scaled_r);

                            curr = &curr.children[best_idx].1;
                            if res.done {
                                break;
                            }
                        }

                        let t_obs0 = std::time::Instant::now();
                        let leaf_eval = if curr.is_terminal {
                            IndexedLeafEval::Terminal
                        } else if !curr.is_expanded {
                            let leaf_obs = sim_env.extract_graph_observation();
                            if leaf_obs.valid_actions.is_empty() {
                                IndexedLeafEval::Terminal
                            } else {
                                let local_obs_idx = env_leaf_obs.len();
                                env_leaf_obs.push(leaf_obs);
                                IndexedLeafEval::PendingExpand(local_obs_idx)
                            }
                        } else {
                            IndexedLeafEval::StoredValue(curr.q_value())
                        };
                        obs_ns.fetch_add(t_obs0.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);

                        Self::apply_virtual_loss(root, &node_path, VLOSS);

                        env_paths.push(IndexedSimPath {
                            node_path,
                            rewards,
                            leaf_eval,
                        });
                    }

                    (env_paths, env_leaf_obs)
                } else {
                    (Vec::new(), Vec::new())
                }
            })
            .collect();

        let mut all_leaf_obs = Vec::new();
        let mut adjusted_group_paths = Vec::with_capacity(range_len);

        for (mut paths, env_obs) in env_results {
            let global_offset = all_leaf_obs.len();
            for path in &mut paths {
                if let IndexedLeafEval::PendingExpand(ref mut local_idx) = path.leaf_eval {
                    *local_idx += global_offset;
                }
            }
            all_leaf_obs.extend(env_obs);
            adjusted_group_paths.push(paths);
        }

        (adjusted_group_paths, all_leaf_obs)
    }

    fn run_sim_backprop_range(
        &self,
        range: std::ops::Range<usize>,
        group_paths: &[Vec<IndexedSimPath>],
        eval_leaf_obs: &[crate::env::GraphObservation],
        gpu_results: &[(Vec<f32>, f32)],
        roots: &mut [Option<MCTSNode>],
        q_mins: &mut [f32],
        q_maxs: &mut [f32],
    ) {
        const VLOSS: f32 = 3.0;
        let roots_slice = &mut roots[range.clone()];
        let q_mins_slice = &mut q_mins[range.clone()];
        let q_maxs_slice = &mut q_maxs[range];

        group_paths
            .par_iter()
            .zip(roots_slice.par_iter_mut())
            .zip(q_mins_slice.par_iter_mut())
            .zip(q_maxs_slice.par_iter_mut())
            .for_each(|(((paths, root_opt), q_min), q_max)| {
                if let Some(ref mut root) = root_opt {
                    for path in paths {
                        let mut curr = &mut *root;
                        for &idx in &path.node_path {
                            if idx < curr.children.len() {
                                curr = &mut curr.children[idx].1;
                            } else {
                                break;
                            }
                        }

                        let leaf_value = match path.leaf_eval {
                            IndexedLeafEval::Terminal => {
                                curr.is_terminal = true;
                                0.0
                            }
                            IndexedLeafEval::PendingExpand(global_obs_idx) => {
                                if !curr.is_expanded && global_obs_idx < eval_leaf_obs.len() && global_obs_idx < gpu_results.len() {
                                    let (leaf_logits, val) = &gpu_results[global_obs_idx];
                                    let obs = &eval_leaf_obs[global_obs_idx];

                                    let l_max = leaf_logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                                    let l_exps: Vec<f32> = leaf_logits.iter().map(|l| (l - l_max).exp()).collect();
                                    let l_sum: f32 = l_exps.iter().sum::<f32>().max(1e-8);
                                    let l_probs: Vec<f32> = l_exps.iter().map(|e| e / l_sum).collect();

                                    curr.is_expanded = true;
                                    curr.children = obs.valid_actions
                                        .iter()
                                        .zip(l_probs.iter())
                                        .map(|(&act, &p)| (act, MCTSNode::new(p)))
                                        .collect();

                                    *val
                                } else {
                                    curr.q_value()
                                }
                            }
                            IndexedLeafEval::StoredValue(v) => v,
                        };

                        let mut g = leaf_value;
                        let depth = path.node_path.len();
                        let mut returns = vec![0.0f32; depth];
                        for d in (0..depth).rev() {
                            g = path.rewards[d] + self.config.gamma * g;
                            returns[d] = g;
                        }

                        root.visit_count += 1;
                        root.total_value += g;
                        let rq = root.q_value();
                        if rq < *q_min { *q_min = rq; }
                        if rq > *q_max { *q_max = rq; }

                        let mut trav = &mut *root;
                        for d in 0..depth {
                            let child_idx = path.node_path[d];
                            if child_idx < trav.children.len() {
                                trav = &mut trav.children[child_idx].1;
                                trav.visit_count = trav.visit_count.saturating_sub(1);
                                trav.total_value += VLOSS;
                                trav.visit_count += 1;
                                trav.total_value += returns[d];

                                let q = trav.q_value();
                                if q < *q_min { *q_min = q; }
                                if q > *q_max { *q_max = q; }
                            }
                        }
                    }
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalized_entropy_extremes() {
        // One-hot → 0
        assert_eq!(MCTSSearch::normalized_entropy(&[1.0f32, 0.0, 0.0, 0.0]), 0.0);
        // Uniform → 1
        assert!((MCTSSearch::normalized_entropy(&[0.25f32; 4]) - 1.0).abs() < 1e-5);
        // Lệch → trong (0,1)
        let e = MCTSSearch::normalized_entropy(&[0.7f32, 0.15, 0.1, 0.05]);
        assert!(e > 0.0 && e < 1.0);
        // 1 phần tử → 0
        assert_eq!(MCTSSearch::normalized_entropy(&[1.0]), 0.0);
    }

    #[test]
    fn test_entropy_explore_direction() {
        // Tự tin (prior_e=0) → explore mạnh: temp cao (~temp_high), hay chen noise
        let cfg = MCTSConfig {
            explore_by_entropy: true,
            temp_high: 1.0,
            temp_low: 0.2,
            ..MCTSConfig::default()
        };
        let mut noise_count = 0;
        let mut temp_sum = 0.0;
        for _ in 0..2000 {
            let (add, t) = cfg.entropy_explore(0.0);
            if add { noise_count += 1; }
            temp_sum += t;
        }
        // prior_e=0 → strength=1 → temp = temp_high = 1.0
        assert!((temp_sum / 2000.0 - 1.0).abs() < 0.01);
        // strength=1 → luôn chen noise
        assert_eq!(noise_count, 2000);

        // Bối rối (prior_e=1) → exploit: temp thấp (~temp_low), ít/không noise
        let (add2, t2) = cfg.entropy_explore(1.0);
        assert!(!add2, "prior bối rối → không chen noise (strength=0)");
        assert!((t2 - 0.2).abs() < 1e-6, "prior bối rối → temp = temp_low");

        // Tắt explore_by_entropy → luôn (false, temp_low), không đổi theo entropy
        let cfg_off = MCTSConfig::default();
        assert!(!cfg_off.explore_by_entropy);
        let (add3, t3) = cfg_off.entropy_explore(0.0);
        assert!(!add3);
        assert_eq!(t3, 0.2);
    }

    #[test]
    fn test_search_batch_indexed_virtual_loss() {
        let env1 = DorfromantikEnv::new(42, 10, 5);
        let env2 = DorfromantikEnv::new(43, 10, 5);
        let envs = vec![env1, env2];
        let model = HexGNNModel::new();
        let mcts = MCTSSearch::new(MCTSConfig {
            n_simulations: 20,
            ..MCTSConfig::default()
        });

        let active_indices = vec![0, 1];
        let results = mcts.search_batch_indexed(&envs, &active_indices, &model, None, false, 0.0);
        assert_eq!(results.len(), 2);
        for (pi, chosen_idx, act, _val, obs) in results {
            assert_eq!(pi.len(), obs.valid_actions.len());
            assert!(chosen_idx < obs.valid_actions.len());
            assert_eq!(act, obs.valid_actions[chosen_idx]);
        }
    }
}


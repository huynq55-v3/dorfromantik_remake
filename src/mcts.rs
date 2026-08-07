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
}

impl Default for MCTSConfig {
    fn default() -> Self {
        Self {
            c_puct: 1.5,
            gamma: 0.99,
            n_simulations: 200,
            dirichlet_alpha: 0.3,
            dirichlet_eps: 0.25,
        }
    }
}

pub struct MCTSSearch {
    pub config: MCTSConfig,
}

impl MCTSSearch {
    pub fn new(config: MCTSConfig) -> Self {
        Self { config }
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

    /// Thực hiện MCTS Search với số lượt simulations quy định (200 simulations)
    /// Trả về: (Phân phối xác suất π_mcts, Action index được chọn, Giá trị ước tính Value tại Root)
    pub fn search(
        &self,
        env: &DorfromantikEnv,
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

        // Thêm Dirichlet noise tại Root trong quá trình Self-Play để khám phá
        if add_dirichlet && num_actions > 1 {
            let noise = Self::sample_dirichlet(num_actions, self.config.dirichlet_alpha);
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

        // 2. Chạy N lượt MCTS Simulations (mỗi lượt là 1 chuỗi Selection -> Expansion -> Value Evaluation -> Backup)
        for _ in 0..self.config.n_simulations {
            let mut sim_env = env.clone();
            let mut node_path: Vec<usize> = Vec::new();
            let mut step_rewards: Vec<f32> = Vec::new();

            // --- SELECTION PHASE ---
            let mut curr = &mut root;
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
                            // Tất cả Q-value bằng nhau (model mới init) → trả về mid-point
                            0.5
                        }
                    } else {
                        // Chưa visited: prior term hoàn toàn quyết định exploration
                        0.0
                    };

                    let ucb = q_val + self.config.c_puct * child.prior * sqrt_n / (1.0 + child.visit_count as f32);
                    if ucb > best_ucb {
                        best_ucb = ucb;
                        best_idx = idx;
                    }
                }

                let (chosen_action, _) = curr.children[best_idx];
                let res = sim_env.step(chosen_action);
                let scaled_r = res.reward * 0.01;

                node_path.push(best_idx);
                step_rewards.push(scaled_r);

                curr = &mut curr.children[best_idx].1;
                curr.immediate_reward = scaled_r;

                if res.done {
                    curr.is_terminal = true;
                    break;
                }
            }

            // --- EXPANSION & EVALUATION PHASE ---
            let leaf_value = if curr.is_terminal {
                0.0
            } else if !curr.is_expanded {
                let leaf_obs = sim_env.extract_graph_observation();
                if leaf_obs.valid_actions.is_empty() {
                    curr.is_terminal = true;
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

                    curr.is_expanded = true;
                    curr.children = leaf_obs.valid_actions
                        .iter()
                        .zip(l_probs.iter())
                        .map(|(&act, &p)| (act, MCTSNode::new(p)))
                        .collect();

                    val
                }
            } else {
                curr.q_value()
            };

            // --- BACKPROPAGATION (BACKUP) PHASE ---
            // Cập nhật giá trị ngược từ nút lá lên đến nút gốc
            let mut g = leaf_value;
            let depth = node_path.len();

            // Tính toán cumulative return cho từng tầng
            let mut returns = vec![0.0f32; depth];
            for d in (0..depth).rev() {
                g = step_rewards[d] + self.config.gamma * g;
                returns[d] = g;
            }

            // Đi theo path từ Root để cập nhật visit_count và total_value
            let mut traverse = &mut root;
            traverse.visit_count += 1;
            traverse.total_value += g;

            for d in 0..depth {
                let child_idx = node_path[d];
                traverse = &mut traverse.children[child_idx].1;
                traverse.visit_count += 1;
                traverse.total_value += returns[d];

                let q = traverse.q_value();
                if q < q_min { q_min = q; }
                if q > q_max { q_max = q; }
            }
        }

        // 3. Tính Target Policy π_mcts từ Visit Count của các con tại Root
        let visit_counts: Vec<f32> = root.children.iter().map(|(_, c)| c.visit_count as f32).collect();
        let total_visits: f32 = visit_counts.iter().sum::<f32>().max(1.0);

        let pi_probs = if temperature <= 1e-3 {
            // Greedy: Chọn Action có Visit Count cao nhất
            let max_idx = visit_counts
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            let mut p = vec![0.0f32; num_actions];
            p[max_idx] = 1.0;
            p
        } else {
            // Softmax Temperature trên Visit Counts: N(a)^(1/tau)
            let powered: Vec<f32> = visit_counts.iter().map(|&v| (v / total_visits).powf(1.0 / temperature)).collect();
            let sum_pow: f32 = powered.iter().sum::<f32>().max(1e-8);
            powered.iter().map(|p| p / sum_pow).collect()
        };

        // Chọn Action theo phân phối pi_probs
        let chosen_idx = if temperature <= 1e-3 {
            visit_counts
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx)
                .unwrap_or(0)
        } else {
            let mut rng = rand::thread_rng();
            let r: f32 = rng.gen_range(0.0..1.0);
            let mut cum = 0.0f32;
            let mut selected = num_actions - 1;
            for (i, &p) in pi_probs.iter().enumerate() {
                cum += p;
                if r <= cum {
                    selected = i;
                    break;
                }
            }
            selected
        };

        let chosen_action = root.children[chosen_idx].0;
        (pi_probs, chosen_idx, chosen_action, root_val)
    }

    /// Helper: Gửi GraphObservation sang GpuEvalQueue và nhận kết quả từ GPU
    fn eval_obs_via_gpu(
        obs: &crate::env::GraphObservation,
        eval_tx: &crossbeam_channel::Sender<crate::gpu_engine::GpuEvalRequest>,
    ) -> (Vec<f32>, f32) {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        let req = crate::gpu_engine::GpuEvalRequest {
            obs: obs.clone(),
            response_tx: resp_tx,
        };
        if eval_tx.send(req).is_ok() {
            if let Ok(res) = resp_rx.recv() {
                return res;
            }
        }
        (vec![0.0; obs.valid_actions.len()], 0.0)
    }

    /// MCTS Search GPU Batch Evaluation
    pub fn search_gpu(
        &self,
        env: &DorfromantikEnv,
        eval_tx: &crossbeam_channel::Sender<crate::gpu_engine::GpuEvalRequest>,
        add_dirichlet: bool,
        temperature: f32,
    ) -> (Vec<f32>, usize, Action, f32) {
        let obs = env.extract_graph_observation();
        let num_actions = obs.valid_actions.len();

        if num_actions == 0 {
            let default_act = Action { q: 0, r: 0, rotation: 0 };
            return (Vec::new(), 0, default_act, 0.0);
        }

        // 1. Khởi tạo Root Node và Expand bằng GPU Eval Queue
        let (action_logits, root_val) = Self::eval_obs_via_gpu(&obs, eval_tx);

        let max_logit = action_logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = action_logits.iter().map(|l| (l - max_logit).exp()).collect();
        let sum_exp: f32 = exps.iter().sum::<f32>().max(1e-8);
        let mut priors: Vec<f32> = exps.iter().map(|e| e / sum_exp).collect();

        if add_dirichlet && num_actions > 1 {
            let noise = Self::sample_dirichlet(num_actions, self.config.dirichlet_alpha);
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

        // 2. Chạy N lượt MCTS Simulations
        for _ in 0..self.config.n_simulations {
            let mut sim_env = env.clone();
            let mut node_path: Vec<usize> = Vec::new();
            let mut step_rewards: Vec<f32> = Vec::new();

            let mut curr = &mut root;
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
                let res = sim_env.step(chosen_action);
                let scaled_r = res.reward * 0.01;

                node_path.push(best_idx);
                step_rewards.push(scaled_r);

                curr = &mut curr.children[best_idx].1;
                curr.immediate_reward = scaled_r;

                if res.done {
                    curr.is_terminal = true;
                    break;
                }
            }

            // GPU EXPANSION & EVALUATION PHASE
            let leaf_value = if curr.is_terminal {
                0.0
            } else if !curr.is_expanded {
                let leaf_obs = sim_env.extract_graph_observation();
                if leaf_obs.valid_actions.is_empty() {
                    curr.is_terminal = true;
                    0.0
                } else {
                    let (leaf_logits, val) = Self::eval_obs_via_gpu(&leaf_obs, eval_tx);

                    let l_max = leaf_logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                    let l_exps: Vec<f32> = leaf_logits.iter().map(|l| (l - l_max).exp()).collect();
                    let l_sum: f32 = l_exps.iter().sum::<f32>().max(1e-8);
                    let l_probs: Vec<f32> = l_exps.iter().map(|e| e / l_sum).collect();

                    curr.is_expanded = true;
                    curr.children = leaf_obs.valid_actions
                        .iter()
                        .zip(l_probs.iter())
                        .map(|(&act, &p)| (act, MCTSNode::new(p)))
                        .collect();

                    val
                }
            } else {
                curr.q_value()
            };

            // BACKPROPAGATION PHASE
            let mut g = leaf_value;
            let depth = node_path.len();

            let mut returns = vec![0.0f32; depth];
            for d in (0..depth).rev() {
                g = step_rewards[d] + self.config.gamma * g;
                returns[d] = g;
            }

            let mut traverse = &mut root;
            traverse.visit_count += 1;
            traverse.total_value += g;

            for d in 0..depth {
                let child_idx = node_path[d];
                traverse = &mut traverse.children[child_idx].1;
                traverse.visit_count += 1;
                traverse.total_value += returns[d];

                let q = traverse.q_value();
                if q < q_min { q_min = q; }
                if q > q_max { q_max = q; }
            }
        }

        let visit_counts: Vec<f32> = root.children.iter().map(|(_, c)| c.visit_count as f32).collect();
        let total_visits: f32 = visit_counts.iter().sum::<f32>().max(1.0);

        let pi_probs = if temperature <= 1e-3 {
            let max_idx = visit_counts
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            let mut p = vec![0.0f32; num_actions];
            p[max_idx] = 1.0;
            p
        } else {
            let powered: Vec<f32> = visit_counts.iter().map(|&v| (v / total_visits).powf(1.0 / temperature)).collect();
            let sum_pow: f32 = powered.iter().sum::<f32>().max(1e-8);
            powered.iter().map(|p| p / sum_pow).collect()
        };

        let chosen_idx = if temperature <= 1e-3 {
            visit_counts
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx)
                .unwrap_or(0)
        } else {
            let mut rng = rand::thread_rng();
            let r: f32 = rng.gen_range(0.0..1.0);
            let mut cum = 0.0f32;
            let mut selected = num_actions - 1;
            for (i, &p) in pi_probs.iter().enumerate() {
                cum += p;
                if r <= cum {
                    selected = i;
                    break;
                }
            }
            selected
        };

        let chosen_action = root.children[chosen_idx].0;
        (pi_probs, chosen_idx, chosen_action, root_val)
    }

    /// Vectorized Batch MCTS Search cho danh sách B environments đồng thời.
    /// Khi có gpu_exec, dùng GPU để evaluate neural network (nhanh hơn CPU).
    pub fn search_batch(
        &self,
        envs: &[DorfromantikEnv],
        model: &HexGNNModel,
        gpu_exec: Option<&GpuNNExecutor>,
        add_dirichlet: bool,
        temperature: f32,
    ) -> Vec<(Vec<f32>, usize, Action, f32)> {
        let b_count = envs.len();
        if b_count == 0 {
            return Vec::new();
        }

        let obs_batch: Vec<crate::env::GraphObservation> = envs.iter().map(|e| e.extract_graph_observation()).collect();
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

            if add_dirichlet && num_actions > 1 {
                let noise = Self::sample_dirichlet(num_actions, self.config.dirichlet_alpha);
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

        for _ in 0..self.config.n_simulations {
            let sim_results: Vec<(Option<DorfromantikEnv>, Vec<usize>, Vec<f32>)> = (0..b_count)
                .into_par_iter()
                .map(|i| {
                    if let Some(ref root) = roots[i] {
                        let mut sim_env = envs[i].clone();
                        let mut node_path = Vec::new();
                        let mut rewards = Vec::new();

                        let mut curr: &MCTSNode = root;
                        let q_min = q_mins[i];
                        let q_max = q_maxs[i];

                        while curr.is_expanded && !curr.children.is_empty() && !curr.is_terminal {
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

                            let (chosen_action, _) = curr.children[best_idx];
                            let res = sim_env.step(chosen_action);
                            let scaled_r = res.reward * 0.01;

                            node_path.push(best_idx);
                            rewards.push(scaled_r);

                            curr = &curr.children[best_idx].1;
                            if res.done {
                                break;
                            }
                        }

                        (Some(sim_env), node_path, rewards)
                    } else {
                        (None, Vec::new(), Vec::new())
                    }
                })
                .collect();

            let mut sim_envs = Vec::with_capacity(b_count);
            let mut node_paths = Vec::with_capacity(b_count);
            let mut step_rewards = Vec::with_capacity(b_count);

            for (sim_env, node_path, rewards) in sim_results {
                sim_envs.push(sim_env);
                node_paths.push(node_path);
                step_rewards.push(rewards);
            }

            let mut eval_indices = Vec::new();
            let mut eval_leaf_obs = Vec::new();

            for (i, sim_env_opt) in sim_envs.iter().enumerate() {
                if let Some(ref sim_env) = sim_env_opt {
                    if let Some(ref mut root) = roots[i] {
                        let path = &node_paths[i];
                        let mut curr = root;
                        for &idx in path {
                            curr = &mut curr.children[idx].1;
                        }

                        if !curr.is_terminal && !curr.is_expanded {
                            let leaf_obs = sim_env.extract_graph_observation();
                            if leaf_obs.valid_actions.is_empty() {
                                curr.is_terminal = true;
                            } else {
                                eval_indices.push(i);
                                eval_leaf_obs.push(leaf_obs);
                            }
                        }
                    }
                }
            }

            let mut leaf_eval_results = Vec::new();
            if !eval_leaf_obs.is_empty() {
                let leaf_refs: Vec<&crate::env::GraphObservation> = eval_leaf_obs.iter().collect();
                leaf_eval_results = if let Some(gpu) = gpu_exec {
                    gpu.forward_batch_gpu(&leaf_refs)
                } else {
                    model.forward_batch(&leaf_refs)
                };
            }

            let mut eval_result_map = std::collections::HashMap::new();
            for (idx, res) in eval_indices.iter().zip(leaf_eval_results) {
                eval_result_map.insert(*idx, res);
            }

            for (i, sim_env_opt) in sim_envs.iter().enumerate() {
                if let Some(_) = sim_env_opt {
                    if let Some(ref mut root) = roots[i] {
                        let node_path = &node_paths[i];
                        let rewards = &step_rewards[i];

                        let eval_res = eval_result_map.get(&i).cloned();
                        let obs_opt = eval_indices.iter().position(|&x| x == i).map(|pos| &eval_leaf_obs[pos]);

                        // 1. Expand leaf node if evaluated
                        let mut curr = &mut *root;
                        for &idx in node_path {
                            curr = &mut curr.children[idx].1;
                        }

                        let leaf_value = if curr.is_terminal {
                            0.0
                        } else if let (Some((ref leaf_logits, val)), Some(obs)) = (eval_res, obs_opt) {
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

                            val
                        } else {
                            curr.q_value()
                        };

                        // 2. Backup values up the path
                        let mut g = leaf_value;
                        let depth = node_path.len();
                        let mut returns = vec![0.0f32; depth];
                        for d in (0..depth).rev() {
                            g = rewards[d] + self.config.gamma * g;
                            returns[d] = g;
                        }

                        let mut trav = &mut *root;
                        trav.visit_count += 1;
                        trav.total_value += g;

                        for d in 0..depth {
                            let child_idx = node_path[d];
                            trav = &mut trav.children[child_idx].1;
                            trav.visit_count += 1;
                            trav.total_value += returns[d];

                            let q = trav.q_value();
                            if q < q_mins[i] { q_mins[i] = q; }
                            if q > q_maxs[i] { q_maxs[i] = q; }
                        }
                    }
                }
            }
        }

        let mut results = Vec::with_capacity(b_count);
        for i in 0..b_count {
            let obs = &obs_batch[i];
            let num_actions = obs.valid_actions.len();

            if num_actions == 0 || roots[i].is_none() {
                let default_act = Action { q: 0, r: 0, rotation: 0 };
                results.push((Vec::new(), 0, default_act, 0.0));
                continue;
            }

            let root = roots[i].as_ref().unwrap();
            let root_val = root_vals[i];

            let visit_counts: Vec<f32> = root.children.iter().map(|(_, c)| c.visit_count as f32).collect();
            let total_visits: f32 = visit_counts.iter().sum::<f32>().max(1.0);

            let pi_probs = if temperature <= 1e-3 {
                let max_idx = visit_counts
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                    .map(|(idx, _)| idx)
                    .unwrap_or(0);
                let mut p = vec![0.0f32; num_actions];
                p[max_idx] = 1.0;
                p
            } else {
                let powered: Vec<f32> = visit_counts.iter().map(|&v| (v / total_visits).powf(1.0 / temperature)).collect();
                let sum_pow: f32 = powered.iter().sum::<f32>().max(1e-8);
                powered.iter().map(|p| p / sum_pow).collect()
            };

            let chosen_idx = if temperature <= 1e-3 {
                visit_counts
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                    .map(|(idx, _)| idx)
                    .unwrap_or(0)
            } else {
                let mut rng = rand::thread_rng();
                let r: f32 = rng.gen_range(0.0..1.0);
                let mut cum = 0.0f32;
                let mut selected = num_actions - 1;
                for (k, &p) in pi_probs.iter().enumerate() {
                    cum += p;
                    if r <= cum {
                        selected = k;
                        break;
                    }
                }
                selected
            };

            let chosen_action = root.children[chosen_idx].0;
            results.push((pi_probs, chosen_idx, chosen_action, root_val));
        }

        results
    }
}



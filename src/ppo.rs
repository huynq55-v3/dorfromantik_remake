use rand::Rng;
use rayon::prelude::*;
use crate::env::{Action, DorfromantikEnv, GraphObservation};
use crate::nn::HexGNNModel;

/// Một kinh nghiệm bước đi (Experience Transition) lưu trong Buffer
#[derive(Debug, Clone)]
pub struct Transition {
    pub obs: GraphObservation,
    pub action: Action,
    pub action_idx: usize, // index trong obs.valid_actions
    pub reward: f32,
    pub value: f32,
    pub log_prob: f32,
    pub done: bool,
}

/// Trải nghiệm thu thập được từ 1 đợt Rollout Batch
#[derive(Debug, Clone)]
pub struct RolloutBatch {
    pub transitions: Vec<Transition>,
    pub returns: Vec<f32>,
    pub advantages: Vec<f32>,
}

/// Bộ quản lý Thuật toán PPO (Proximal Policy Optimization)
pub struct PPOAgent {
    pub model: HexGNNModel,
    pub target_seed: i32,
    pub tile_limit: usize,
    pub lr: f32,
    pub gamma: f32,
    pub gae_lambda: f32,
    pub clip_eps: f32,
}

impl PPOAgent {
    pub fn new(target_seed: i32, tile_limit: usize, lr: f32) -> Self {
        Self {
            model: HexGNNModel::new(),
            target_seed,
            tile_limit,
            lr,
            gamma: 0.99,
            gae_lambda: 0.95,
            clip_eps: 0.2,
        }
    }

    /// Chọn Action bằng Softmax Sampling từ Logits đã được Action Masking
    pub fn select_action(
        model: &HexGNNModel,
        obs: &GraphObservation,
        deterministic: bool,
    ) -> (Action, usize, f32, f32) {
        if obs.valid_actions.is_empty() {
            return (Action { q: 0, r: 0, rotation: 0 }, 0, 0.0, 0.0);
        }

        let (node_logits, state_value) = model.forward(&obs.node_features, &obs.edge_index);

        // Ánh xạ valid_actions sang Logits tương ứng
        let mut action_logits = Vec::with_capacity(obs.valid_actions.len());
        for act in &obs.valid_actions {
            let pos_idx = obs.node_positions.iter().position(|&p| p == (act.q, act.r));
            let logit = if let Some(idx) = pos_idx {
                if idx < node_logits.len() {
                    node_logits[idx][act.rotation]
                } else {
                    -1e9
                }
            } else {
                -1e9
            };
            action_logits.push(logit);
        }

        // Softmax
        let max_logit = action_logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = action_logits.iter().map(|l| (l - max_logit).exp()).collect();
        let sum_exp: f32 = exps.iter().sum();
        let probs: Vec<f32> = exps.iter().map(|e| e / sum_exp.max(1e-8)).collect();

        let chosen_idx = if deterministic {
            // Chọn action có xác suất cao nhất khi Eval
            probs
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx)
                .unwrap_or(0)
        } else {
            // Sample ngẫu nhiên theo xác suất khi Train
            let mut rng = rand::thread_rng();
            let mut r: f32 = rng.gen_range(0.0..1.0);
            let mut selected = 0;
            for (i, &p) in probs.iter().enumerate() {
                r -= p;
                if r <= 0.0 {
                    selected = i;
                    break;
                }
            }
            selected
        };

        let chosen_action = obs.valid_actions[chosen_idx];
        let chosen_prob = probs[chosen_idx].max(1e-8);
        let log_prob = chosen_prob.ln();

        (chosen_action, chosen_idx, log_prob, state_value)
    }

    /// DATA GENERATION PIPELINE: Thu thập dữ liệu mô phỏng từ nhiều Worker CPU Threads song song
    pub fn collect_rollout_parallel(&self, num_envs: usize, steps_per_env: usize) -> RolloutBatch {
        let model_ref = &self.model;
        let seed = self.target_seed;
        let limit = self.tile_limit;

        // Chạy song song thu thập Rollout từ num_envs các thread CPU bằng Rayon
        let env_rollouts: Vec<Vec<Transition>> = (0..num_envs)
            .into_par_iter()
            .map(|_env_id| {
                let mut env = DorfromantikEnv::new(seed, 10, limit);
                let mut transitions = Vec::with_capacity(steps_per_env);

                for _ in 0..steps_per_env {
                    let obs = env.extract_graph_observation();
                    if obs.valid_actions.is_empty() {
                        break;
                    }

                    let (action, act_idx, log_prob, val) = Self::select_action(model_ref, &obs, false);
                    let res = env.step(action);

                    transitions.push(Transition {
                        obs,
                        action,
                        action_idx: act_idx,
                        reward: res.reward,
                        value: val,
                        log_prob,
                        done: res.done,
                    });

                    if res.done {
                        env.reset();
                    }
                }
                transitions
            })
            .collect();

        // Gom tất cả transitions về 1 mảng phẳng
        let mut all_transitions = Vec::new();
        for mut env_tr in env_rollouts {
            all_transitions.append(&mut env_tr);
        }

        // GAE (Generalized Advantage Estimation) calculation
        let n = all_transitions.len();
        let mut returns = vec![0.0f32; n];
        let mut advantages = vec![0.0f32; n];

        let mut gae = 0.0f32;
        for i in (0..n).rev() {
            let next_value = if i + 1 < n && !all_transitions[i].done {
                all_transitions[i + 1].value
            } else {
                0.0
            };
            let delta = all_transitions[i].reward + self.gamma * next_value - all_transitions[i].value;
            gae = delta + self.gamma * self.gae_lambda * (if all_transitions[i].done { 0.0 } else { 1.0 }) * gae;
            advantages[i] = gae;
            returns[i] = gae + all_transitions[i].value;
        }

        RolloutBatch {
            transitions: all_transitions,
            returns,
            advantages,
        }
    }

    /// TRAINING PIPELINE: Cập nhật trọng số Policy & Value bằng Parallel Backpropagation trên Rayon + Adam Optimizer
    pub fn train_step(&mut self, batch: &RolloutBatch) -> f32 {
        if batch.transitions.is_empty() {
            return 0.0;
        }

        let n = batch.transitions.len();

        // 1. Advantage Normalization
        let mean_adv = batch.advantages.iter().sum::<f32>() / n as f32;
        let var_adv = batch.advantages.iter().map(|a| (a - mean_adv).powi(2)).sum::<f32>() / n as f32;
        let std_adv = (var_adv + 1e-8).sqrt();

        let model_ref = &self.model;
        let clip_eps = self.clip_eps;

        // 2. Parallel Backpropagation qua các CPU Worker Threads bằng Rayon
        let (accumulated_grads, total_loss) = (0..n)
            .into_par_iter()
            .map(|i| {
                let tr = &batch.transitions[i];
                let norm_adv = (batch.advantages[i] - mean_adv) / std_adv;
                let norm_return = batch.returns[i] / 100.0;

                let (_new_action, _new_idx, new_log_prob, new_val) = Self::select_action(model_ref, &tr.obs, false);

                let ratio = (new_log_prob - tr.log_prob).exp();
                let surr1 = ratio * norm_adv;
                let surr2 = ratio.clamp(1.0 - clip_eps, 1.0 + clip_eps) * norm_adv;
                let policy_loss = -surr1.min(surr2);

                let value_loss = 0.5 * (new_val - norm_return).powi(2);
                let loss = policy_loss + 0.5 * value_loss;

                let val_grad = (new_val - norm_return) * 0.1;

                let mut local_grads = HexGNNModel::new_zero();

                model_ref.backward_accumulate(
                    &tr.obs.node_positions,
                    &tr.obs.node_features,
                    &tr.obs.edge_index,
                    &tr.obs.valid_actions,
                    tr.action_idx,
                    norm_adv,
                    val_grad,
                    &mut local_grads,
                );

                (local_grads, loss)
            })
            .reduce(
                || (HexGNNModel::new_zero(), 0.0f32),
                |(mut g1, l1), (g2, l2)| {
                    g1.add_assign(&g2);
                    (g1, l1 + l2)
                },
            );

        // Cập nhật trọng số qua Adam Optimizer
        self.model.update_weights_adam(&accumulated_grads, self.lr / n as f32);

        total_loss / n as f32
    }

    /// EVALUATION PIPELINE: Chạy thử nghiệm ván chơi đánh giá điểm số thực tế bằng Policy hiện tại (Deterministic)
    pub fn evaluate(&self, num_episodes: usize) -> (f64, i32, usize) {
        let scores: Vec<(i32, usize)> = (0..num_episodes)
            .into_par_iter()
            .map(|_| {
                let mut env = DorfromantikEnv::new(self.target_seed, 10, self.tile_limit);
                let mut total_placed = 0;

                loop {
                    let obs = env.extract_graph_observation();
                    if obs.valid_actions.is_empty() {
                        break;
                    }

                    let (action, _, _, _) = Self::select_action(&self.model, &obs, true);
                    let res = env.step(action);

                    total_placed += 1;
                    if res.done {
                        break;
                    }
                }
                (env.score_manager.total_score as i32, total_placed)
            })
            .collect();

        let max_score = scores.iter().map(|(s, _)| *s).max().unwrap_or(0);
        let avg_score = scores.iter().map(|(s, _)| *s as f64).sum::<f64>() / num_episodes as f64;
        let avg_placed = scores.iter().map(|(_, p)| *p).sum::<usize>() / num_episodes;

        (avg_score, max_score, avg_placed)
    }
}

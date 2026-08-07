use rand::seq::SliceRandom;
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
    pub ppo_epochs: usize,
    pub mini_batch_size: usize,
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
            ppo_epochs: 4,
            mini_batch_size: 128,
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
            let r: f32 = rng.gen_range(0.0..1.0);
            let mut cum = 0.0f32;
            let mut selected = probs.len() - 1;
            for (i, &p) in probs.iter().enumerate() {
                cum += p;
                if r <= cum {
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

                    // Reward scaling 0.05 đưa reward về dải [-1.0, 10.0] hoàn hảo cho Neural Network
                    let scaled_reward = res.reward * 0.05;

                    transitions.push(Transition {
                        obs,
                        action,
                        action_idx: act_idx,
                        reward: scaled_reward,
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

        // Tính GAE RIÊNG cho từng env (tránh boundary leakage giữa các env)
        let mut all_transitions = Vec::new();
        let mut all_returns = Vec::new();
        let mut all_advantages = Vec::new();

        for env_tr in env_rollouts {
            let n = env_tr.len();
            if n == 0 {
                continue;
            }

            let mut returns = vec![0.0f32; n];
            let mut advantages = vec![0.0f32; n];
            let mut gae = 0.0f32;

            for i in (0..n).rev() {
                let next_value = if i + 1 < n && !env_tr[i].done {
                    env_tr[i + 1].value
                } else {
                    0.0
                };
                let delta = env_tr[i].reward + self.gamma * next_value - env_tr[i].value;
                gae = delta + self.gamma * self.gae_lambda * (if env_tr[i].done { 0.0 } else { 1.0 }) * gae;
                advantages[i] = gae;
                returns[i] = gae + env_tr[i].value;
            }

            all_transitions.extend(env_tr);
            all_returns.extend(returns);
            all_advantages.extend(advantages);
        }

        RolloutBatch {
            transitions: all_transitions,
            returns: all_returns,
            advantages: all_advantages,
        }
    }

    /// TRAINING PIPELINE: Cập nhật trọng số Policy & Value bằng Multi-Epoch Mini-Batch PPO + Adam
    pub fn train_step(&mut self, batch: &RolloutBatch) -> f32 {
        if batch.transitions.is_empty() {
            return 0.0;
        }

        let n = batch.transitions.len();
        let value_coef: f32 = 0.5;
        let entropy_coef: f32 = 0.01;

        // Advantage Normalization trên toàn bộ Batch
        let mean_adv = batch.advantages.iter().sum::<f32>() / n as f32;
        let var_adv = batch.advantages.iter().map(|a| (a - mean_adv).powi(2)).sum::<f32>() / n as f32;
        let std_adv = (var_adv + 1e-8).sqrt();

        let norm_advantages: Vec<f32> = batch.advantages.iter().map(|&a| (a - mean_adv) / std_adv).collect();

        let mut total_loss_accum = 0.0f32;
        let mut total_updates = 0;

        let mut rng = rand::thread_rng();
        let mut indices: Vec<usize> = (0..n).collect();

        for _epoch in 0..self.ppo_epochs {
            indices.shuffle(&mut rng);

            for chunk in indices.chunks(self.mini_batch_size) {
                let model_ref = &self.model;
                let clip_eps = self.clip_eps;

                let (mb_grads, mb_loss) = chunk
                    .into_par_iter()
                    .map(|&i| {
                        let tr = &batch.transitions[i];
                        let norm_adv = norm_advantages[i];
                        let target_return = batch.returns[i];

                        let (new_log_prob, new_val, entropy) = model_ref.evaluate_action(
                            &tr.obs,
                            &tr.action,
                            tr.action_idx,
                        );

                        // PPO Clipped Objective
                        let ratio = (new_log_prob - tr.log_prob).exp();
                        let surr1 = ratio * norm_adv;
                        let surr2 = ratio.clamp(1.0 - clip_eps, 1.0 + clip_eps) * norm_adv;
                        let policy_loss = -surr1.min(surr2);

                        let is_clipped = (norm_adv > 0.0 && ratio > 1.0 + clip_eps)
                            || (norm_adv < 0.0 && ratio < 1.0 - clip_eps);
                        let effective_policy_adv = if is_clipped { 0.0 } else { norm_adv * ratio };

                        // Value loss: Sử dụng Huber Loss (Smooth L1) để tránh bùng nổ gradient khi Returns lớn
                        let diff = new_val - target_return;
                        let abs_diff = diff.abs();
                        let (value_loss, val_grad) = if abs_diff <= 1.0 {
                            (0.5 * diff.powi(2), diff * value_coef)
                        } else {
                            (abs_diff - 0.5, diff.signum() * value_coef)
                        };

                        let loss = policy_loss + value_coef * value_loss - entropy_coef * entropy;

                        let mut local_grads = HexGNNModel::new_zero();
                        model_ref.backward_accumulate(
                            &tr.obs.node_positions,
                            &tr.obs.node_features,
                            &tr.obs.edge_index,
                            &tr.obs.valid_actions,
                            tr.action_idx,
                            effective_policy_adv,
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

                let mb_len = chunk.len() as f32;
                let mut scaled_grads = mb_grads;
                scaled_grads.scale_assign(1.0 / mb_len);
                scaled_grads.clip_grad_norm(1.0);

                self.model.update_weights_adam(&scaled_grads, self.lr);

                total_loss_accum += mb_loss / mb_len;
                total_updates += 1;
            }
        }

        if total_updates > 0 {
            total_loss_accum / total_updates as f32
        } else {
            0.0
        }
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

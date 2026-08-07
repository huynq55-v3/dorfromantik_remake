use rand::Rng;

/// Trọng số Ma trận Linear Layer trong Rust cùng với Adam Optimizer Moments (m, v)
#[derive(Debug, Clone)]
pub struct Linear {
    pub in_features: usize,
    pub out_features: usize,
    pub weight: Vec<f32>, // row-major: [out_features, in_features]
    pub bias: Vec<f32>,   // [out_features]
    pub m_w: Vec<f32>,    // Adam 1st moment for weight
    pub v_w: Vec<f32>,    // Adam 2nd moment for weight
    pub m_b: Vec<f32>,    // Adam 1st moment for bias
    pub v_b: Vec<f32>,    // Adam 2nd moment for bias
}

impl Linear {
    pub fn new(in_features: usize, out_features: usize) -> Self {
        let mut rng = rand::thread_rng();
        // Kaiming / Xavier Uniform Initialization
        let bound = (6.0 / (in_features + out_features) as f32).sqrt();
        let mut weight = vec![0.0f32; out_features * in_features];
        for w in weight.iter_mut() {
            *w = rng.gen_range(-bound..bound);
        }
        let bias = vec![0.0f32; out_features];
        let m_w = vec![0.0f32; out_features * in_features];
        let v_w = vec![0.0f32; out_features * in_features];
        let m_b = vec![0.0f32; out_features];
        let v_b = vec![0.0f32; out_features];

        Self {
            in_features,
            out_features,
            weight,
            bias,
            m_w,
            v_w,
            m_b,
            v_b,
        }
    }

    pub fn new_zero(in_features: usize, out_features: usize) -> Self {
        Self {
            in_features,
            out_features,
            weight: vec![0.0f32; out_features * in_features],
            bias: vec![0.0f32; out_features],
            m_w: vec![0.0f32; out_features * in_features],
            v_w: vec![0.0f32; out_features * in_features],
            m_b: vec![0.0f32; out_features],
            v_b: vec![0.0f32; out_features],
        }
    }

    /// Linear Forward: Y = X * W^T + b
    pub fn forward(&self, input: &[f32], input_len: usize) -> Vec<f32> {
        let batch_size = input_len / self.in_features;
        let mut output = vec![0.0f32; batch_size * self.out_features];

        for b in 0..batch_size {
            let in_offset = b * self.in_features;
            let out_offset = b * self.out_features;

            for o in 0..self.out_features {
                let mut sum = self.bias[o];
                let w_offset = o * self.in_features;
                for i in 0..self.in_features {
                    sum += input[in_offset + i] * self.weight[w_offset + i];
                }
                output[out_offset + o] = sum;
            }
        }
        output
    }

    /// Adam Optimizer Step cho Linear Layer
    pub fn adam_update(&mut self, grad_w: &[f32], grad_b: &[f32], lr: f32, beta1: f32, beta2: f32, eps: f32, t: usize) {
        let beta1_t = 1.0 - beta1.powi(t as i32);
        let beta2_t = 1.0 - beta2.powi(t as i32);

        for i in 0..self.weight.len() {
            let g = grad_w[i].clamp(-1.0, 1.0);
            self.m_w[i] = beta1 * self.m_w[i] + (1.0 - beta1) * g;
            self.v_w[i] = beta2 * self.v_w[i] + (1.0 - beta2) * g * g;

            let m_hat = self.m_w[i] / beta1_t;
            let v_hat = self.v_w[i] / beta2_t;

            self.weight[i] -= lr * m_hat / (v_hat.sqrt() + eps);
        }

        for i in 0..self.bias.len() {
            let g = grad_b[i].clamp(-1.0, 1.0);
            self.m_b[i] = beta1 * self.m_b[i] + (1.0 - beta1) * g;
            self.v_b[i] = beta2 * self.v_b[i] + (1.0 - beta2) * g * g;

            let m_hat = self.m_b[i] / beta1_t;
            let v_hat = self.v_b[i] / beta2_t;

            self.bias[i] -= lr * m_hat / (v_hat.sqrt() + eps);
        }
    }
}

/// Mạng Neural Graph Neural Network (GNN Hex Model) cho Dorfromantik trên CPU
#[derive(Debug, Clone)]
pub struct HexGNNModel {
    pub w_self: Linear,     // 38 -> 64
    pub w_neigh: Linear,    // 38 -> 64
    pub w_policy: Linear,   // 64 -> 6 (Logits cho 6 góc xoay)
    pub w_val1: Linear,     // 64 -> 64
    pub w_val2: Linear,     // 64 -> 1 (Scalar Value V(s))
    pub step_count: usize,
}

impl HexGNNModel {
    pub fn new() -> Self {
        Self {
            w_self: Linear::new(38, 64),
            w_neigh: Linear::new(38, 64),
            w_policy: Linear::new(64, 6),
            w_val1: Linear::new(64, 64),
            w_val2: Linear::new(64, 1),
            step_count: 0,
        }
    }

    pub fn new_zero() -> Self {
        Self {
            w_self: Linear::new_zero(38, 64),
            w_neigh: Linear::new_zero(38, 64),
            w_policy: Linear::new_zero(64, 6),
            w_val1: Linear::new_zero(64, 64),
            w_val2: Linear::new_zero(64, 1),
            step_count: 0,
        }
    }

    pub fn add_assign(&mut self, other: &HexGNNModel) {
        let add = |w: &mut Vec<f32>, o: &[f32]| {
            for (w_i, o_i) in w.iter_mut().zip(o.iter()) {
                *w_i += *o_i;
            }
        };
        add(&mut self.w_self.weight, &other.w_self.weight);
        add(&mut self.w_self.bias, &other.w_self.bias);
        add(&mut self.w_neigh.weight, &other.w_neigh.weight);
        add(&mut self.w_neigh.bias, &other.w_neigh.bias);
        add(&mut self.w_policy.weight, &other.w_policy.weight);
        add(&mut self.w_policy.bias, &other.w_policy.bias);
        add(&mut self.w_val1.weight, &other.w_val1.weight);
        add(&mut self.w_val1.bias, &other.w_val1.bias);
        add(&mut self.w_val2.weight, &other.w_val2.weight);
        add(&mut self.w_val2.bias, &other.w_val2.bias);
    }

    /// Forward pass trên Graph Observation:
    pub fn forward(
        &self,
        node_features: &[[f32; 38]],
        edge_index: &[(usize, usize)],
    ) -> (Vec<Vec<f32>>, f32) {
        let n_nodes = node_features.len();
        if n_nodes == 0 {
            return (Vec::new(), 0.0);
        }

        let mut neighbor_sum = vec![[0.0f32; 38]; n_nodes];
        let mut neighbor_count = vec![0usize; n_nodes];

        for &(u, v) in edge_index {
            if u < n_nodes && v < n_nodes {
                for i in 0..38 {
                    neighbor_sum[u][i] += node_features[v][i];
                }
                neighbor_count[u] += 1;
            }
        }

        let mut neighbor_mean = vec![0.0f32; n_nodes * 38];
        for u in 0..n_nodes {
            let count = neighbor_count[u].max(1) as f32;
            for i in 0..38 {
                neighbor_mean[u * 38 + i] = neighbor_sum[u][i] / count;
            }
        }

        let mut self_flat = vec![0.0f32; n_nodes * 38];
        for u in 0..n_nodes {
            self_flat[u * 38..(u + 1) * 38].copy_from_slice(&node_features[u]);
        }

        let out_self = self.w_self.forward(&self_flat, n_nodes * 38);
        let out_neigh = self.w_neigh.forward(&neighbor_mean, n_nodes * 38);

        let mut h = vec![0.0f32; n_nodes * 64];
        for i in 0..n_nodes * 64 {
            let sum = out_self[i] + out_neigh[i];
            h[i] = if sum > 0.0 { sum } else { 0.0 };
        }

        let pol_logits_flat = self.w_policy.forward(&h, n_nodes * 64);
        let mut node_logits = vec![vec![0.0f32; 6]; n_nodes];

        for u in 0..n_nodes {
            for r in 0..6 {
                node_logits[u][r] = pol_logits_flat[u * 6 + r];
            }
        }

        let mut mean_h = vec![0.0f32; 64];
        for u in 0..n_nodes {
            for i in 0..64 {
                mean_h[i] += h[u * 64 + i];
            }
        }
        for i in 0..64 {
            mean_h[i] /= n_nodes as f32;
        }

        let val_hidden = self.w_val1.forward(&mean_h, 64);
        let mut val_relu = vec![0.0f32; 64];
        for i in 0..64 {
            val_relu[i] = if val_hidden[i] > 0.0 { val_hidden[i] } else { 0.0 };
        }
        let val_out = self.w_val2.forward(&val_relu, 64);
        let state_value = val_out[0];

        (node_logits, state_value)
    }

    /// Backpropagation: Tính đạo hàm chính xác trên toàn bộ Action Space và tích lũy gradients
    pub fn backward_accumulate(
        &self,
        node_positions: &[(i32, i32)],
        node_features: &[[f32; 38]],
        edge_index: &[(usize, usize)],
        valid_actions: &[crate::env::Action],
        chosen_action_idx: usize,
        advantage: f32,
        value_grad: f32,
        grads: &mut HexGNNModel,
    ) {
        let n_nodes = node_features.len();
        if n_nodes == 0 || valid_actions.is_empty() {
            return;
        }

        let mut neighbor_sum = vec![[0.0f32; 38]; n_nodes];
        let mut neighbor_count = vec![0usize; n_nodes];
        for &(u, v) in edge_index {
            if u < n_nodes && v < n_nodes {
                for i in 0..38 {
                    neighbor_sum[u][i] += node_features[v][i];
                }
                neighbor_count[u] += 1;
            }
        }

        let mut neighbor_mean = vec![0.0f32; n_nodes * 38];
        for u in 0..n_nodes {
            let count = neighbor_count[u].max(1) as f32;
            for i in 0..38 {
                neighbor_mean[u * 38 + i] = neighbor_sum[u][i] / count;
            }
        }

        let mut self_flat = vec![0.0f32; n_nodes * 38];
        for u in 0..n_nodes {
            self_flat[u * 38..(u + 1) * 38].copy_from_slice(&node_features[u]);
        }

        let out_self = self.w_self.forward(&self_flat, n_nodes * 38);
        let out_neigh = self.w_neigh.forward(&neighbor_mean, n_nodes * 38);

        let mut h = vec![0.0f32; n_nodes * 64];
        let mut h_pre_relu = vec![0.0f32; n_nodes * 64];
        for i in 0..n_nodes * 64 {
            let sum = out_self[i] + out_neigh[i];
            h_pre_relu[i] = sum;
            h[i] = if sum > 0.0 { sum } else { 0.0 };
        }

        let pol_logits_flat = self.w_policy.forward(&h, n_nodes * 64);

        let mut mean_h = vec![0.0f32; 64];
        for u in 0..n_nodes {
            for i in 0..64 {
                mean_h[i] += h[u * 64 + i];
            }
        }
        for i in 0..64 {
            mean_h[i] /= n_nodes as f32;
        }

        let val_hidden = self.w_val1.forward(&mean_h, 64);
        let mut val_relu = vec![0.0f32; 64];
        for i in 0..64 {
            val_relu[i] = if val_hidden[i] > 0.0 { val_hidden[i] } else { 0.0 };
        }

        // --- BACKWARD PASS ---
        // 1. Value Head Backward
        for i in 0..64 {
            grads.w_val2.weight[i] += value_grad * val_relu[i];
        }
        grads.w_val2.bias[0] += value_grad;

        let mut d_val_relu = vec![0.0f32; 64];
        for i in 0..64 {
            d_val_relu[i] = value_grad * self.w_val2.weight[i];
        }

        let mut d_val_hidden = vec![0.0f32; 64];
        for i in 0..64 {
            if val_hidden[i] > 0.0 {
                d_val_hidden[i] = d_val_relu[i];
            }
        }

        for o in 0..64 {
            grads.w_val1.bias[o] += d_val_hidden[o];
            for i in 0..64 {
                grads.w_val1.weight[o * 64 + i] += d_val_hidden[o] * mean_h[i];
            }
        }

        let mut d_mean_h = vec![0.0f32; 64];
        for i in 0..64 {
            for o in 0..64 {
                d_mean_h[i] += d_val_hidden[o] * self.w_val1.weight[o * 64 + i];
            }
        }

        let mut d_h = vec![0.0f32; n_nodes * 64];
        for u in 0..n_nodes {
            for i in 0..64 {
                d_h[u * 64 + i] += d_mean_h[i] / n_nodes as f32;
            }
        }

        // 2. Policy Head Backward qua toàn bộ Action Space
        let m_actions = valid_actions.len();
        let mut act_logits = Vec::with_capacity(m_actions);
        let mut act_node_rot = Vec::with_capacity(m_actions);

        for act in valid_actions {
            let pos_idx = node_positions.iter().position(|&p| p == (act.q, act.r));
            if let Some(node_idx) = pos_idx {
                let logit = pol_logits_flat[node_idx * 6 + act.rotation];
                act_logits.push(logit);
                act_node_rot.push((node_idx, act.rotation));
            } else {
                act_logits.push(-1e9);
                act_node_rot.push((0, 0));
            }
        }

        let max_l = act_logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = act_logits.iter().map(|l| (l - max_l).exp()).collect();
        let sum_e: f32 = exps.iter().sum();
        let probs: Vec<f32> = exps.iter().map(|e| e / sum_e.max(1e-8)).collect();

        for m in 0..m_actions {
            let p_m = probs[m];
            let delta = if m == chosen_action_idx { 1.0 } else { 0.0 };
            let d_logit = (p_m - delta) * advantage;

            let (node_idx, rot) = act_node_rot[m];
            let o = rot;
            grads.w_policy.bias[o] += d_logit;
            let h_offset = node_idx * 64;
            for i in 0..64 {
                grads.w_policy.weight[o * 64 + i] += d_logit * h[h_offset + i];
                d_h[h_offset + i] += d_logit * self.w_policy.weight[o * 64 + i];
            }
        }

        // 3. HexConv Layer Backward
        let mut d_h_pre = vec![0.0f32; n_nodes * 64];
        for i in 0..n_nodes * 64 {
            if h_pre_relu[i] > 0.0 {
                d_h_pre[i] = d_h[i];
            }
        }

        for u in 0..n_nodes {
            let h_off = u * 64;
            let self_off = u * 38;
            for o in 0..64 {
                let d_l = d_h_pre[h_off + o];
                grads.w_self.bias[o] += d_l;
                grads.w_neigh.bias[o] += d_l;
                for i in 0..38 {
                    grads.w_self.weight[o * 38 + i] += d_l * self_flat[self_off + i];
                    grads.w_neigh.weight[o * 38 + i] += d_l * neighbor_mean[self_off + i];
                }
            }
        }
    }

    /// Cập nhật trọng số bằng Adam Optimizer (hội tụ nhanh gấp 10 lần SGD)
    pub fn update_weights_adam(&mut self, grads: &HexGNNModel, lr: f32) {
        self.step_count += 1;
        let t = self.step_count;
        let beta1 = 0.9;
        let beta2 = 0.999;
        let eps = 1e-8;

        self.w_self.adam_update(&grads.w_self.weight, &grads.w_self.bias, lr, beta1, beta2, eps, t);
        self.w_neigh.adam_update(&grads.w_neigh.weight, &grads.w_neigh.bias, lr, beta1, beta2, eps, t);
        self.w_policy.adam_update(&grads.w_policy.weight, &grads.w_policy.bias, lr, beta1, beta2, eps, t);
        self.w_val1.adam_update(&grads.w_val1.weight, &grads.w_val1.bias, lr, beta1, beta2, eps, t);
        self.w_val2.adam_update(&grads.w_val2.weight, &grads.w_val2.bias, lr, beta1, beta2, eps, t);
    }
}

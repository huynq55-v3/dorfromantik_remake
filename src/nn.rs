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
            let g = grad_w[i];
            self.m_w[i] = beta1 * self.m_w[i] + (1.0 - beta1) * g;
            self.v_w[i] = beta2 * self.v_w[i] + (1.0 - beta2) * g * g;

            let m_hat = self.m_w[i] / beta1_t;
            let v_hat = self.v_w[i] / beta2_t;

            self.weight[i] -= lr * m_hat / (v_hat.sqrt() + eps);
        }

        for i in 0..self.bias.len() {
            let g = grad_b[i];
            self.m_b[i] = beta1 * self.m_b[i] + (1.0 - beta1) * g;
            self.v_b[i] = beta2 * self.v_b[i] + (1.0 - beta2) * g * g;

            let m_hat = self.m_b[i] / beta1_t;
            let v_hat = self.v_b[i] / beta2_t;

            self.bias[i] -= lr * m_hat / (v_hat.sqrt() + eps);
        }
    }
}

/// Mạng 3-Hop Residual Graph Neural Network kết hợp Action Scoring MLP Head
#[derive(Debug, Clone)]
pub struct HexGNNModel {
    // 3 GNN Layers (Backbone)
    pub w_self1: Linear,     // 40 -> 64
    pub w_neigh1: Linear,    // 40 -> 64
    pub w_self2: Linear,     // 64 -> 64
    pub w_neigh2: Linear,    // 64 -> 64
    pub w_self3: Linear,     // 64 -> 64
    pub w_neigh3: Linear,    // 64 -> 64

    // Action Scoring MLP Head: [h(u) (64) + action_feature (16)] = 80 -> 64 -> 1
    pub w_act1: Linear,      // 80 -> 64
    pub w_act2: Linear,      // 64 -> 1

    // Value Head: mean_h (64) -> 64 -> 1
    pub w_val1: Linear,      // 64 -> 64
    pub w_val2: Linear,      // 64 -> 1

    pub step_count: usize,
}

impl HexGNNModel {
    pub fn new() -> Self {
        Self {
            w_self1: Linear::new(40, 64),
            w_neigh1: Linear::new(40, 64),
            w_self2: Linear::new(64, 64),
            w_neigh2: Linear::new(64, 64),
            w_self3: Linear::new(64, 64),
            w_neigh3: Linear::new(64, 64),
            w_act1: Linear::new(80, 64),
            w_act2: Linear::new(64, 1),
            w_val1: Linear::new(64, 64),
            w_val2: Linear::new(64, 1),
            step_count: 0,
        }
    }

    pub fn new_zero() -> Self {
        Self {
            w_self1: Linear::new_zero(40, 64),
            w_neigh1: Linear::new_zero(40, 64),
            w_self2: Linear::new_zero(64, 64),
            w_neigh2: Linear::new_zero(64, 64),
            w_self3: Linear::new_zero(64, 64),
            w_neigh3: Linear::new_zero(64, 64),
            w_act1: Linear::new_zero(80, 64),
            w_act2: Linear::new_zero(64, 1),
            w_val1: Linear::new_zero(64, 64),
            w_val2: Linear::new_zero(64, 1),
            step_count: 0,
        }
    }

    pub fn scale_assign(&mut self, factor: f32) {
        let scale = |w: &mut Vec<f32>| {
            for x in w.iter_mut() {
                *x *= factor;
            }
        };
        scale(&mut self.w_self1.weight);
        scale(&mut self.w_self1.bias);
        scale(&mut self.w_neigh1.weight);
        scale(&mut self.w_neigh1.bias);
        scale(&mut self.w_self2.weight);
        scale(&mut self.w_self2.bias);
        scale(&mut self.w_neigh2.weight);
        scale(&mut self.w_neigh2.bias);
        scale(&mut self.w_self3.weight);
        scale(&mut self.w_self3.bias);
        scale(&mut self.w_neigh3.weight);
        scale(&mut self.w_neigh3.bias);
        scale(&mut self.w_act1.weight);
        scale(&mut self.w_act1.bias);
        scale(&mut self.w_act2.weight);
        scale(&mut self.w_act2.bias);
        scale(&mut self.w_val1.weight);
        scale(&mut self.w_val1.bias);
        scale(&mut self.w_val2.weight);
        scale(&mut self.w_val2.bias);
    }

    pub fn clip_grad_norm(&mut self, max_norm: f32) {
        let mut total_sq = 0.0f32;
        let sum_sq = |w: &[f32]| -> f32 { w.iter().map(|x| x * x).sum() };
        total_sq += sum_sq(&self.w_self1.weight) + sum_sq(&self.w_self1.bias);
        total_sq += sum_sq(&self.w_neigh1.weight) + sum_sq(&self.w_neigh1.bias);
        total_sq += sum_sq(&self.w_self2.weight) + sum_sq(&self.w_self2.bias);
        total_sq += sum_sq(&self.w_neigh2.weight) + sum_sq(&self.w_neigh2.bias);
        total_sq += sum_sq(&self.w_self3.weight) + sum_sq(&self.w_self3.bias);
        total_sq += sum_sq(&self.w_neigh3.weight) + sum_sq(&self.w_neigh3.bias);
        total_sq += sum_sq(&self.w_act1.weight) + sum_sq(&self.w_act1.bias);
        total_sq += sum_sq(&self.w_act2.weight) + sum_sq(&self.w_act2.bias);
        total_sq += sum_sq(&self.w_val1.weight) + sum_sq(&self.w_val1.bias);
        total_sq += sum_sq(&self.w_val2.weight) + sum_sq(&self.w_val2.bias);

        let norm = total_sq.sqrt();
        if norm > max_norm && norm > 1e-8 {
            let scale = max_norm / norm;
            self.scale_assign(scale);
        }
    }

    pub fn add_assign(&mut self, other: &HexGNNModel) {
        let add = |w: &mut Vec<f32>, o: &[f32]| {
            for (w_i, o_i) in w.iter_mut().zip(o.iter()) {
                *w_i += *o_i;
            }
        };
        add(&mut self.w_self1.weight, &other.w_self1.weight);
        add(&mut self.w_self1.bias, &other.w_self1.bias);
        add(&mut self.w_neigh1.weight, &other.w_neigh1.weight);
        add(&mut self.w_neigh1.bias, &other.w_neigh1.bias);
        add(&mut self.w_self2.weight, &other.w_self2.weight);
        add(&mut self.w_self2.bias, &other.w_self2.bias);
        add(&mut self.w_neigh2.weight, &other.w_neigh2.weight);
        add(&mut self.w_neigh2.bias, &other.w_neigh2.bias);
        add(&mut self.w_self3.weight, &other.w_self3.weight);
        add(&mut self.w_self3.bias, &other.w_self3.bias);
        add(&mut self.w_neigh3.weight, &other.w_neigh3.weight);
        add(&mut self.w_neigh3.bias, &other.w_neigh3.bias);
        add(&mut self.w_act1.weight, &other.w_act1.weight);
        add(&mut self.w_act1.bias, &other.w_act1.bias);
        add(&mut self.w_act2.weight, &other.w_act2.weight);
        add(&mut self.w_act2.bias, &other.w_act2.bias);
        add(&mut self.w_val1.weight, &other.w_val1.weight);
        add(&mut self.w_val1.bias, &other.w_val1.bias);
        add(&mut self.w_val2.weight, &other.w_val2.weight);
        add(&mut self.w_val2.bias, &other.w_val2.bias);
    }

    /// Helper: Tính Mean của Hàng xóm qua Graph Edges
    fn aggregate_neighbors(h_in: &[f32], dim: usize, n_nodes: usize, edge_index: &[(usize, usize)]) -> Vec<f32> {
        let mut sum = vec![0.0f32; n_nodes * dim];
        let mut count = vec![0usize; n_nodes];

        for &(u, v) in edge_index {
            if u < n_nodes && v < n_nodes {
                for i in 0..dim {
                    sum[u * dim + i] += h_in[v * dim + i];
                }
                count[u] += 1;
            }
        }

        let mut mean = vec![0.0f32; n_nodes * dim];
        for u in 0..n_nodes {
            let c = count[u].max(1) as f32;
            for i in 0..dim {
                mean[u * dim + i] = sum[u * dim + i] / c;
            }
        }
        mean
    }

    /// Forward pass trên Graph Observation:
    /// Trả về: (action_logits: Vec<f32>, state_value: f32)
    pub fn forward(
        &self,
        node_positions: &[(i32, i32)],
        node_features: &[[f32; 40]],
        edge_index: &[(usize, usize)],
        valid_actions: &[crate::env::Action],
        action_features: &[[f32; 16]],
    ) -> (Vec<f32>, f32) {
        let n_nodes = node_features.len();
        let num_actions = valid_actions.len();
        if n_nodes == 0 || num_actions == 0 {
            return (vec![0.0; num_actions], 0.0);
        }

        // --- GNN LAYER 1: 40 -> 64 ---
        let mut x_flat = vec![0.0f32; n_nodes * 40];
        for u in 0..n_nodes {
            x_flat[u * 40..(u + 1) * 40].copy_from_slice(&node_features[u]);
        }
        let neigh1 = Self::aggregate_neighbors(&x_flat, 40, n_nodes, edge_index);
        let out_self1 = self.w_self1.forward(&x_flat, n_nodes * 40);
        let out_neigh1 = self.w_neigh1.forward(&neigh1, n_nodes * 40);

        let mut h1 = vec![0.0f32; n_nodes * 64];
        for i in 0..n_nodes * 64 {
            let sum = out_self1[i] + out_neigh1[i];
            h1[i] = if sum > 0.0 { sum } else { 0.0 };
        }

        // --- GNN LAYER 2: 64 -> 64 (Residual) ---
        let neigh2 = Self::aggregate_neighbors(&h1, 64, n_nodes, edge_index);
        let out_self2 = self.w_self2.forward(&h1, n_nodes * 64);
        let out_neigh2 = self.w_neigh2.forward(&neigh2, n_nodes * 64);

        let mut h2 = vec![0.0f32; n_nodes * 64];
        for i in 0..n_nodes * 64 {
            let sum = out_self2[i] + out_neigh2[i];
            let relu = if sum > 0.0 { sum } else { 0.0 };
            h2[i] = relu + h1[i]; // Residual connection
        }

        // --- GNN LAYER 3: 64 -> 64 (Residual) ---
        let neigh3 = Self::aggregate_neighbors(&h2, 64, n_nodes, edge_index);
        let out_self3 = self.w_self3.forward(&h2, n_nodes * 64);
        let out_neigh3 = self.w_neigh3.forward(&neigh3, n_nodes * 64);

        let mut h3 = vec![0.0f32; n_nodes * 64];
        for i in 0..n_nodes * 64 {
            let sum = out_self3[i] + out_neigh3[i];
            let relu = if sum > 0.0 { sum } else { 0.0 };
            h3[i] = relu + h2[i]; // Residual connection
        }

        // --- ACTION SCORING MLP HEAD: [h3(u) (64) + act_feat (16)] = 80 -> 64 -> 1 ---
        let mut act_in = vec![0.0f32; num_actions * 80];
        for (a_idx, act) in valid_actions.iter().enumerate() {
            let pos_idx = node_positions.iter().position(|&p| p == (act.q, act.r)).unwrap_or(0);
            let u = pos_idx.min(n_nodes - 1);

            // Copy 64 dims from h3[u]
            act_in[a_idx * 80..a_idx * 80 + 64].copy_from_slice(&h3[u * 64..(u + 1) * 64]);
            // Copy 16 dims from action_features[a_idx]
            if a_idx < action_features.len() {
                act_in[a_idx * 80 + 64..(a_idx + 1) * 80].copy_from_slice(&action_features[a_idx]);
            }
        }

        let act_hidden = self.w_act1.forward(&act_in, num_actions * 80);
        let mut act_relu = vec![0.0f32; num_actions * 64];
        for i in 0..num_actions * 64 {
            act_relu[i] = if act_hidden[i] > 0.0 { act_hidden[i] } else { 0.0 };
        }
        let action_logits = self.w_act2.forward(&act_relu, num_actions * 64);

        // --- VALUE HEAD: mean_h3 (64) -> 64 -> 1 ---
        let mut mean_h3 = vec![0.0f32; 64];
        for u in 0..n_nodes {
            for i in 0..64 {
                mean_h3[i] += h3[u * 64 + i];
            }
        }
        for i in 0..64 {
            mean_h3[i] /= n_nodes as f32;
        }

        let val_hidden = self.w_val1.forward(&mean_h3, 64);
        let mut val_relu = vec![0.0f32; 64];
        for i in 0..64 {
            val_relu[i] = if val_hidden[i] > 0.0 { val_hidden[i] } else { 0.0 };
        }
        let val_out = self.w_val2.forward(&val_relu, 64);
        let state_value = val_out[0];

        (action_logits, state_value)
    }

    /// Evaluate a specific action: compute log_prob and value WITHOUT re-sampling
    pub fn evaluate_action(
        &self,
        obs: &crate::env::GraphObservation,
        _chosen_action: &crate::env::Action,
        chosen_action_idx: usize,
    ) -> (f32, f32, f32) {
        let (action_logits, state_value) = self.forward(
            &obs.node_positions,
            &obs.node_features,
            &obs.edge_index,
            &obs.valid_actions,
            &obs.action_features,
        );

        if action_logits.is_empty() {
            return (0.0, state_value, 0.0);
        }

        let max_logit = action_logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = action_logits.iter().map(|l| (l - max_logit).exp()).collect();
        let sum_exp: f32 = exps.iter().sum();
        let probs: Vec<f32> = exps.iter().map(|e| e / sum_exp.max(1e-8)).collect();

        let chosen_idx = chosen_action_idx.min(probs.len() - 1);
        let chosen_prob = probs[chosen_idx].max(1e-8);
        let log_prob = chosen_prob.ln();

        let entropy: f32 = probs.iter()
            .map(|&p| if p > 1e-8 { -p * p.ln() } else { 0.0 })
            .sum();

        (log_prob, state_value, entropy)
    }

    /// Backpropagation: Tính đạo hàm giải tích chính xác qua toàn bộ 3 Layers GNN + Action Scoring MLP Head
    pub fn backward_accumulate(
        &self,
        node_positions: &[(i32, i32)],
        node_features: &[[f32; 40]],
        edge_index: &[(usize, usize)],
        valid_actions: &[crate::env::Action],
        action_features: &[[f32; 16]],
        chosen_action_idx: usize,
        advantage: f32,
        value_grad: f32,
        grads: &mut HexGNNModel,
    ) {
        let n_nodes = node_features.len();
        let num_actions = valid_actions.len();
        if n_nodes == 0 || num_actions == 0 {
            return;
        }

        // ================= FORWARD PASS TRACING =================
        // GNN 1
        let mut x_flat = vec![0.0f32; n_nodes * 40];
        for u in 0..n_nodes {
            x_flat[u * 40..(u + 1) * 40].copy_from_slice(&node_features[u]);
        }
        let neigh1 = Self::aggregate_neighbors(&x_flat, 40, n_nodes, edge_index);
        let out_self1 = self.w_self1.forward(&x_flat, n_nodes * 40);
        let out_neigh1 = self.w_neigh1.forward(&neigh1, n_nodes * 40);
        let mut h1_pre = vec![0.0f32; n_nodes * 64];
        let mut h1 = vec![0.0f32; n_nodes * 64];
        for i in 0..n_nodes * 64 {
            let sum = out_self1[i] + out_neigh1[i];
            h1_pre[i] = sum;
            h1[i] = if sum > 0.0 { sum } else { 0.0 };
        }

        // GNN 2
        let neigh2 = Self::aggregate_neighbors(&h1, 64, n_nodes, edge_index);
        let out_self2 = self.w_self2.forward(&h1, n_nodes * 64);
        let out_neigh2 = self.w_neigh2.forward(&neigh2, n_nodes * 64);
        let mut h2_pre = vec![0.0f32; n_nodes * 64];
        let mut h2 = vec![0.0f32; n_nodes * 64];
        for i in 0..n_nodes * 64 {
            let sum = out_self2[i] + out_neigh2[i];
            h2_pre[i] = sum;
            let relu = if sum > 0.0 { sum } else { 0.0 };
            h2[i] = relu + h1[i];
        }

        // GNN 3
        let neigh3 = Self::aggregate_neighbors(&h2, 64, n_nodes, edge_index);
        let out_self3 = self.w_self3.forward(&h2, n_nodes * 64);
        let out_neigh3 = self.w_neigh3.forward(&neigh3, n_nodes * 64);
        let mut h3_pre = vec![0.0f32; n_nodes * 64];
        let mut h3 = vec![0.0f32; n_nodes * 64];
        for i in 0..n_nodes * 64 {
            let sum = out_self3[i] + out_neigh3[i];
            h3_pre[i] = sum;
            let relu = if sum > 0.0 { sum } else { 0.0 };
            h3[i] = relu + h2[i];
        }

        // Action MLP Forward
        let mut act_in = vec![0.0f32; num_actions * 80];
        let mut act_node_idx = Vec::with_capacity(num_actions);
        for (a_idx, act) in valid_actions.iter().enumerate() {
            let pos_idx = node_positions.iter().position(|&p| p == (act.q, act.r)).unwrap_or(0);
            let u = pos_idx.min(n_nodes - 1);
            act_node_idx.push(u);

            act_in[a_idx * 80..a_idx * 80 + 64].copy_from_slice(&h3[u * 64..(u + 1) * 64]);
            if a_idx < action_features.len() {
                act_in[a_idx * 80 + 64..(a_idx + 1) * 80].copy_from_slice(&action_features[a_idx]);
            }
        }
        let act_hidden = self.w_act1.forward(&act_in, num_actions * 80);
        let mut act_relu = vec![0.0f32; num_actions * 64];
        for i in 0..num_actions * 64 {
            act_relu[i] = if act_hidden[i] > 0.0 { act_hidden[i] } else { 0.0 };
        }
        let action_logits = self.w_act2.forward(&act_relu, num_actions * 64);

        // Value Forward
        let mut mean_h3 = vec![0.0f32; 64];
        for u in 0..n_nodes {
            for i in 0..64 {
                mean_h3[i] += h3[u * 64 + i];
            }
        }
        for i in 0..64 {
            mean_h3[i] /= n_nodes as f32;
        }
        let val_hidden = self.w_val1.forward(&mean_h3, 64);
        let mut val_relu = vec![0.0f32; 64];
        for i in 0..64 {
            val_relu[i] = if val_hidden[i] > 0.0 { val_hidden[i] } else { 0.0 };
        }

        // ================= BACKPROPAGATION =================
        // 1. Policy Gradient on Action Logits
        let max_logit = action_logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = action_logits.iter().map(|l| (l - max_logit).exp()).collect();
        let sum_exp: f32 = exps.iter().sum();
        let probs: Vec<f32> = exps.iter().map(|e| e / sum_exp.max(1e-8)).collect();

        // dL/dz(a) = -advantage * (1(a == chosen) - prob(a))
        let mut d_logits = vec![0.0f32; num_actions];
        for a in 0..num_actions {
            let target = if a == chosen_action_idx { 1.0 } else { 0.0 };
            d_logits[a] = -advantage * (target - probs[a]);
        }

        // Backprop through w_act2
        let mut d_act_relu = vec![0.0f32; num_actions * 64];
        for a in 0..num_actions {
            let d_z = d_logits[a];
            grads.w_act2.bias[0] += d_z;
            for j in 0..64 {
                grads.w_act2.weight[j] += d_z * act_relu[a * 64 + j];
                d_act_relu[a * 64 + j] = d_z * self.w_act2.weight[j];
            }
        }

        // Backprop through ReLU + w_act1
        let mut d_act_in = vec![0.0f32; num_actions * 80];
        for a in 0..num_actions {
            for j in 0..64 {
                let d_h = if act_hidden[a * 64 + j] > 0.0 { d_act_relu[a * 64 + j] } else { 0.0 };
                grads.w_act1.bias[j] += d_h;
                let w_off = j * 80;
                for i in 0..80 {
                    grads.w_act1.weight[w_off + i] += d_h * act_in[a * 80 + i];
                    d_act_in[a * 80 + i] += d_h * self.w_act1.weight[w_off + i];
                }
            }
        }

        // Accumulate gradient from Action Head into d_h3[u]
        let mut d_h3 = vec![0.0f32; n_nodes * 64];
        for a in 0..num_actions {
            let u = act_node_idx[a];
            for i in 0..64 {
                d_h3[u * 64 + i] += d_act_in[a * 80 + i];
            }
        }

        // 2. Value Head Gradient
        grads.w_val2.bias[0] += value_grad;
        let mut d_val_relu = vec![0.0f32; 64];
        for i in 0..64 {
            grads.w_val2.weight[i] += value_grad * val_relu[i];
            d_val_relu[i] = value_grad * self.w_val2.weight[i];
        }

        let mut d_mean_h3 = vec![0.0f32; 64];
        for i in 0..64 {
            let d_h = if val_hidden[i] > 0.0 { d_val_relu[i] } else { 0.0 };
            grads.w_val1.bias[i] += d_h;
            let w_off = i * 64;
            for j in 0..64 {
                grads.w_val1.weight[w_off + j] += d_h * mean_h3[j];
                d_mean_h3[j] += d_h * self.w_val1.weight[w_off + j];
            }
        }

        // Accumulate gradient from Value Head into d_h3[u]
        for u in 0..n_nodes {
            for i in 0..64 {
                d_h3[u * 64 + i] += d_mean_h3[i] / n_nodes as f32;
            }
        }

        // 3. Backprop through GNN LAYER 3
        let mut d_h2 = vec![0.0f32; n_nodes * 64];
        let mut d_relu3 = vec![0.0f32; n_nodes * 64];
        for i in 0..n_nodes * 64 {
            d_h2[i] += d_h3[i]; // Residual gradient
            d_relu3[i] = if h3_pre[i] > 0.0 { d_h3[i] } else { 0.0 };
        }

        // d_self3 & d_neigh3
        let mut d_neigh3 = vec![0.0f32; n_nodes * 64];
        for u in 0..n_nodes {
            for o in 0..64 {
                let g = d_relu3[u * 64 + o];
                grads.w_self3.bias[o] += g;
                grads.w_neigh3.bias[o] += g;
                let w_off = o * 64;
                for i in 0..64 {
                    grads.w_self3.weight[w_off + i] += g * h2[u * 64 + i];
                    d_h2[u * 64 + i] += g * self.w_self3.weight[w_off + i];

                    grads.w_neigh3.weight[w_off + i] += g * neigh3[u * 64 + i];
                    d_neigh3[u * 64 + i] += g * self.w_neigh3.weight[w_off + i];
                }
            }
        }

        // Neighbor mean backprop to d_h2
        let mut neighbor_count = vec![0usize; n_nodes];
        for &(u, _) in edge_index {
            if u < n_nodes {
                neighbor_count[u] += 1;
            }
        }
        for &(u, v) in edge_index {
            if u < n_nodes && v < n_nodes {
                let count_u = neighbor_count[u].max(1) as f32;
                for i in 0..64 {
                    d_h2[v * 64 + i] += d_neigh3[u * 64 + i] / count_u;
                }
            }
        }

        // 4. Backprop through GNN LAYER 2
        let mut d_h1 = vec![0.0f32; n_nodes * 64];
        let mut d_relu2 = vec![0.0f32; n_nodes * 64];
        for i in 0..n_nodes * 64 {
            d_h1[i] += d_h2[i]; // Residual gradient
            d_relu2[i] = if h2_pre[i] > 0.0 { d_h2[i] } else { 0.0 };
        }

        let mut d_neigh2 = vec![0.0f32; n_nodes * 64];
        for u in 0..n_nodes {
            for o in 0..64 {
                let g = d_relu2[u * 64 + o];
                grads.w_self2.bias[o] += g;
                grads.w_neigh2.bias[o] += g;
                let w_off = o * 64;
                for i in 0..64 {
                    grads.w_self2.weight[w_off + i] += g * h1[u * 64 + i];
                    d_h1[u * 64 + i] += g * self.w_self2.weight[w_off + i];

                    grads.w_neigh2.weight[w_off + i] += g * neigh2[u * 64 + i];
                    d_neigh2[u * 64 + i] += g * self.w_neigh2.weight[w_off + i];
                }
            }
        }

        for &(u, v) in edge_index {
            if u < n_nodes && v < n_nodes {
                let count_u = neighbor_count[u].max(1) as f32;
                for i in 0..64 {
                    d_h1[v * 64 + i] += d_neigh2[u * 64 + i] / count_u;
                }
            }
        }

        // 5. Backprop through GNN LAYER 1
        let mut d_relu1 = vec![0.0f32; n_nodes * 64];
        for i in 0..n_nodes * 64 {
            d_relu1[i] = if h1_pre[i] > 0.0 { d_h1[i] } else { 0.0 };
        }

        for u in 0..n_nodes {
            for o in 0..64 {
                let g = d_relu1[u * 64 + o];
                grads.w_self1.bias[o] += g;
                grads.w_neigh1.bias[o] += g;
                let w_off = o * 40;
                for i in 0..40 {
                    grads.w_self1.weight[w_off + i] += g * x_flat[u * 40 + i];
                    grads.w_neigh1.weight[w_off + i] += g * neigh1[u * 40 + i];
                }
            }
        }
    }

    pub fn update_weights_adam(&mut self, grads: &HexGNNModel, lr: f32) {
        self.step_count += 1;
        let t = self.step_count;
        let beta1 = 0.9f32;
        let beta2 = 0.999f32;
        let eps = 1e-8f32;

        self.w_self1.adam_update(&grads.w_self1.weight, &grads.w_self1.bias, lr, beta1, beta2, eps, t);
        self.w_neigh1.adam_update(&grads.w_neigh1.weight, &grads.w_neigh1.bias, lr, beta1, beta2, eps, t);
        self.w_self2.adam_update(&grads.w_self2.weight, &grads.w_self2.bias, lr, beta1, beta2, eps, t);
        self.w_neigh2.adam_update(&grads.w_neigh2.weight, &grads.w_neigh2.bias, lr, beta1, beta2, eps, t);
        self.w_self3.adam_update(&grads.w_self3.weight, &grads.w_self3.bias, lr, beta1, beta2, eps, t);
        self.w_neigh3.adam_update(&grads.w_neigh3.weight, &grads.w_neigh3.bias, lr, beta1, beta2, eps, t);
        self.w_act1.adam_update(&grads.w_act1.weight, &grads.w_act1.bias, lr, beta1, beta2, eps, t);
        self.w_act2.adam_update(&grads.w_act2.weight, &grads.w_act2.bias, lr, beta1, beta2, eps, t);
        self.w_val1.adam_update(&grads.w_val1.weight, &grads.w_val1.bias, lr, beta1, beta2, eps, t);
        self.w_val2.adam_update(&grads.w_val2.weight, &grads.w_val2.bias, lr, beta1, beta2, eps, t);
    }
}

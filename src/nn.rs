use rand::Rng;
use std::fs::File;
use std::io::{Read, Write};
use serde::{Serialize, Deserialize};

/// Hidden dimension dùng chung cho toàn bộ backbone GNN và các MLP Head.
pub const HIDDEN_DIM: usize = 128;
/// Số chiều feature của mỗi action (định nghĩa trong env::GraphObservation).
pub const ACTION_FEAT_DIM: usize = 16;
/// Số layer message passing (aggregate + combine / residual block).
pub const N_GNN_LAYERS: usize = 4;
/// Số chiều feature của mỗi node (định nghĩa trong env::GraphObservation).
pub const NODE_FEAT_DIM: usize = 70;

/// Trọng số Ma trận Linear Layer trong Rust cùng với Adam Optimizer Moments (m, v)
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn save_to_writer<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&(self.in_features as u64).to_le_bytes())?;
        writer.write_all(&(self.out_features as u64).to_le_bytes())?;
        for &val in self.weight.iter().chain(&self.bias).chain(&self.m_w).chain(&self.v_w).chain(&self.m_b).chain(&self.v_b) {
            writer.write_all(&val.to_le_bytes())?;
        }
        Ok(())
    }

    pub fn load_from_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut buf8 = [0u8; 8];
        reader.read_exact(&mut buf8)?;
        let in_features = u64::from_le_bytes(buf8) as usize;
        reader.read_exact(&mut buf8)?;
        let out_features = u64::from_le_bytes(buf8) as usize;

        let w_len = out_features * in_features;
        let b_len = out_features;

        let mut read_vec = |len: usize| -> std::io::Result<Vec<f32>> {
            let mut v = Vec::with_capacity(len);
            let mut buf4 = [0u8; 4];
            for _ in 0..len {
                reader.read_exact(&mut buf4)?;
                v.push(f32::from_le_bytes(buf4));
            }
            Ok(v)
        };

        let weight = read_vec(w_len)?;
        let bias = read_vec(b_len)?;
        let m_w = read_vec(w_len)?;
        let v_w = read_vec(w_len)?;
        let m_b = read_vec(b_len)?;
        let v_b = read_vec(b_len)?;

        Ok(Self {
            in_features,
            out_features,
            weight,
            bias,
            m_w,
            v_w,
            m_b,
            v_b,
        })
    }
}

/// Một tầng GNN: self-transform + neighbor-aggregate transform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GNNLayer {
    pub w_self: Linear,
    pub w_neigh: Linear,
}

impl GNNLayer {
    pub fn new(in_dim: usize, out_dim: usize) -> Self {
        Self {
            w_self: Linear::new(in_dim, out_dim),
            w_neigh: Linear::new(in_dim, out_dim),
        }
    }

    pub fn new_zero(in_dim: usize, out_dim: usize) -> Self {
        Self {
            w_self: Linear::new_zero(in_dim, out_dim),
            w_neigh: Linear::new_zero(in_dim, out_dim),
        }
    }
}

/// Mạng N-Hop Residual Graph Neural Network (Hidden Dim = 128) kết hợp Action & Value Head
/// Số tầng GNN được cấu hình qua `N_GNN_LAYERS` (mặc định = 4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HexGNNModel {
    /// Các tầng GNN: layer[0] nhận NODE_FEAT_DIM -> HIDDEN_DIM, các layer sau HIDDEN_DIM -> HIDDEN_DIM
    pub layers: Vec<GNNLayer>,

    // Action Scoring MLP Head: [h(u) (H) + action_feature (16)] = H+16 -> H -> 1
    pub w_act1: Linear,      // H+16 -> H
    pub w_act2: Linear,      // H -> 1

    // Value Head: mean_h (H) -> H -> 1
    pub w_val1: Linear,      // H -> H
    pub w_val2: Linear,      // H -> 1

    pub step_count: usize,
}

impl HexGNNModel {
    pub fn new() -> Self {
        let mut layers = Vec::with_capacity(N_GNN_LAYERS);
        // Layer 0: NODE_FEAT_DIM -> HIDDEN_DIM
        layers.push(GNNLayer::new(NODE_FEAT_DIM, HIDDEN_DIM));
        // Layers 1..N-1: HIDDEN_DIM -> HIDDEN_DIM
        for _ in 1..N_GNN_LAYERS {
            layers.push(GNNLayer::new(HIDDEN_DIM, HIDDEN_DIM));
        }
        Self {
            layers,
            w_act1: Linear::new(HIDDEN_DIM + ACTION_FEAT_DIM, HIDDEN_DIM),
            w_act2: Linear::new(HIDDEN_DIM, 1),
            w_val1: Linear::new(HIDDEN_DIM, HIDDEN_DIM),
            w_val2: Linear::new(HIDDEN_DIM, 1),
            step_count: 0,
        }
    }

    pub fn new_zero() -> Self {
        let mut layers = Vec::with_capacity(N_GNN_LAYERS);
        layers.push(GNNLayer::new_zero(NODE_FEAT_DIM, HIDDEN_DIM));
        for _ in 1..N_GNN_LAYERS {
            layers.push(GNNLayer::new_zero(HIDDEN_DIM, HIDDEN_DIM));
        }
        Self {
            layers,
            w_act1: Linear::new_zero(HIDDEN_DIM + ACTION_FEAT_DIM, HIDDEN_DIM),
            w_act2: Linear::new_zero(HIDDEN_DIM, 1),
            w_val1: Linear::new_zero(HIDDEN_DIM, HIDDEN_DIM),
            w_val2: Linear::new_zero(HIDDEN_DIM, 1),
            step_count: 0,
        }
    }

    pub fn scale_assign(&mut self, factor: f32) {
        let scale = |w: &mut Vec<f32>| {
            for x in w.iter_mut() {
                *x *= factor;
            }
        };
        for layer in &mut self.layers {
            scale(&mut layer.w_self.weight); scale(&mut layer.w_self.bias);
            scale(&mut layer.w_neigh.weight); scale(&mut layer.w_neigh.bias);
        }
        scale(&mut self.w_act1.weight); scale(&mut self.w_act1.bias);
        scale(&mut self.w_act2.weight); scale(&mut self.w_act2.bias);
        scale(&mut self.w_val1.weight); scale(&mut self.w_val1.bias);
        scale(&mut self.w_val2.weight); scale(&mut self.w_val2.bias);
    }

    pub fn clip_grad_norm(&mut self, max_norm: f32) {
        let mut total_sq = 0.0f32;
        let sum_sq = |w: &[f32]| -> f32 { w.iter().map(|x| x * x).sum() };
        for layer in &self.layers {
            total_sq += sum_sq(&layer.w_self.weight) + sum_sq(&layer.w_self.bias);
            total_sq += sum_sq(&layer.w_neigh.weight) + sum_sq(&layer.w_neigh.bias);
        }
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
        for (l, ol) in self.layers.iter_mut().zip(other.layers.iter()) {
            add(&mut l.w_self.weight, &ol.w_self.weight); add(&mut l.w_self.bias, &ol.w_self.bias);
            add(&mut l.w_neigh.weight, &ol.w_neigh.weight); add(&mut l.w_neigh.bias, &ol.w_neigh.bias);
        }
        add(&mut self.w_act1.weight, &other.w_act1.weight); add(&mut self.w_act1.bias, &other.w_act1.bias);
        add(&mut self.w_act2.weight, &other.w_act2.weight); add(&mut self.w_act2.bias, &other.w_act2.bias);
        add(&mut self.w_val1.weight, &other.w_val1.weight); add(&mut self.w_val1.bias, &other.w_val1.bias);
        add(&mut self.w_val2.weight, &other.w_val2.weight); add(&mut self.w_val2.bias, &other.w_val2.bias);
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

    /// Forward pass GNN với N layers (configurable qua N_GNN_LAYERS).
    /// Layer 0: NODE_FEAT_DIM -> HIDDEN_DIM (không residual).
    /// Layer 1..N-1: HIDDEN_DIM -> HIDDEN_DIM (có residual connection).
    /// Trả về (action_logits, state_value) cho một graph.
    pub fn forward(
        &self,
        node_positions: &[(i32, i32)],
        node_features: &[[f32; NODE_FEAT_DIM]],
        edge_index: &[(usize, usize)],
        valid_actions: &[crate::env::Action],
        action_features: &[[f32; ACTION_FEAT_DIM]],
    ) -> (Vec<f32>, f32) {
        let n_nodes = node_features.len();
        let num_actions = valid_actions.len();
        if n_nodes == 0 || num_actions == 0 {
            return (vec![0.0; num_actions], 0.0);
        }

        // --- GNN LAYERS ---
        let mut x_flat = vec![0.0f32; n_nodes * NODE_FEAT_DIM];
        for u in 0..n_nodes {
            x_flat[u * NODE_FEAT_DIM..(u + 1) * NODE_FEAT_DIM].copy_from_slice(&node_features[u]);
        }

        let _n_layers = self.layers.len();
        let mut h_prev: Option<Vec<f32>> = None;
        let mut h_curr = x_flat;
        let mut curr_dim = NODE_FEAT_DIM;

        for (li, layer) in self.layers.iter().enumerate() {
            let out_dim = if li == 0 { HIDDEN_DIM } else { HIDDEN_DIM };
            let neigh = Self::aggregate_neighbors(&h_curr, curr_dim, n_nodes, edge_index);
            let out_self = layer.w_self.forward(&h_curr, n_nodes * curr_dim);
            let out_neigh = layer.w_neigh.forward(&neigh, n_nodes * curr_dim);

            let mut h_next = vec![0.0f32; n_nodes * out_dim];
            let has_residual = li > 0;
            for i in 0..n_nodes * out_dim {
                let sum = out_self[i] + out_neigh[i];
                let relu = if sum > 0.0 { sum } else { 0.0 };
                h_next[i] = if has_residual {
                    relu + h_prev.as_ref().unwrap()[i]
                } else {
                    relu
                };
            }
            h_prev = Some(h_next.clone());
            h_curr = h_next;
            curr_dim = out_dim;
        }

        let h_final = h_prev.unwrap(); // embedding cuối cùng sau tất cả layers

        // --- ACTION SCORING MLP HEAD: [h_final(u) (H) + act_feat (16)] = H+16 -> H -> 1 ---
        let act_in_dim = HIDDEN_DIM + ACTION_FEAT_DIM;
        let mut act_in = vec![0.0f32; num_actions * act_in_dim];
        for (a_idx, act) in valid_actions.iter().enumerate() {
            let pos_idx = node_positions.iter().position(|&p| p == (act.q, act.r)).unwrap_or(0);
            let u = pos_idx.min(n_nodes - 1);
            act_in[a_idx * act_in_dim..a_idx * act_in_dim + HIDDEN_DIM]
                .copy_from_slice(&h_final[u * HIDDEN_DIM..(u + 1) * HIDDEN_DIM]);
            if a_idx < action_features.len() {
                act_in[a_idx * act_in_dim + HIDDEN_DIM..(a_idx + 1) * act_in_dim]
                    .copy_from_slice(&action_features[a_idx]);
            }
        }

        let act_hidden = self.w_act1.forward(&act_in, num_actions * act_in_dim);
        let mut act_relu = vec![0.0f32; num_actions * HIDDEN_DIM];
        for i in 0..num_actions * HIDDEN_DIM {
            act_relu[i] = if act_hidden[i] > 0.0 { act_hidden[i] } else { 0.0 };
        }
        let action_logits = self.w_act2.forward(&act_relu, num_actions * HIDDEN_DIM);

        // --- VALUE HEAD: mean_h_final (H) -> H -> 1 ---
        let mut mean_h = vec![0.0f32; HIDDEN_DIM];
        for u in 0..n_nodes {
            for i in 0..HIDDEN_DIM {
                mean_h[i] += h_final[u * HIDDEN_DIM + i];
            }
        }
        let inv_n = 1.0 / n_nodes as f32;
        for i in 0..HIDDEN_DIM {
            mean_h[i] *= inv_n;
        }

        let val_hidden = self.w_val1.forward(&mean_h, HIDDEN_DIM);
        let mut val_relu = vec![0.0f32; HIDDEN_DIM];
        for i in 0..HIDDEN_DIM {
            val_relu[i] = if val_hidden[i] > 0.0 { val_hidden[i] } else { 0.0 };
        }
        let val_out = self.w_val2.forward(&val_relu, HIDDEN_DIM);
        let state_value = val_out[0];

        (action_logits, state_value)
    }

    /// Forward Batch trên danh sách các GraphObservation (Disjoint Graph Batching V1)
    pub fn forward_batch(
        &self,
        observations: &[&crate::env::GraphObservation],
    ) -> Vec<(Vec<f32>, f32)> {
        let b_count = observations.len();
        if b_count == 0 {
            return Vec::new();
        }
        if b_count == 1 {
            let obs = observations[0];
            let (logits, val) = self.forward(
                &obs.node_positions,
                &obs.node_features,
                &obs.edge_index,
                &obs.valid_actions,
                &obs.action_features,
            );
            return vec![(logits, val)];
        }

        let mut total_nodes = 0usize;
        let mut total_actions = 0usize;
        let mut node_offsets = Vec::with_capacity(b_count);
        let mut action_offsets = Vec::with_capacity(b_count);

        for obs in observations {
            node_offsets.push(total_nodes);
            action_offsets.push(total_actions);
            total_nodes += obs.node_features.len();
            total_actions += obs.valid_actions.len();
        }

        if total_nodes == 0 || total_actions == 0 {
            return observations.iter().map(|obs| (vec![0.0; obs.valid_actions.len()], 0.0)).collect();
        }

        let mut x_flat = vec![0.0f32; total_nodes * NODE_FEAT_DIM];
        let mut combined_edges = Vec::new();

        for (i, obs) in observations.iter().enumerate() {
            let offset = node_offsets[i];
            let n_nodes = obs.node_features.len();
            for u in 0..n_nodes {
                x_flat[(offset + u) * NODE_FEAT_DIM..(offset + u + 1) * NODE_FEAT_DIM]
                    .copy_from_slice(&obs.node_features[u]);
            }
            for &(u, v) in &obs.edge_index {
                combined_edges.push((u + offset, v + offset));
            }
        }

        // --- GNN LAYERS (batched) ---
        let _n_layers = self.layers.len();
        let mut h_prev: Option<Vec<f32>> = None;
        let mut h_curr = x_flat;
        let mut curr_dim = NODE_FEAT_DIM;

        for (li, layer) in self.layers.iter().enumerate() {
            let out_dim = HIDDEN_DIM;
            let neigh = Self::aggregate_neighbors(&h_curr, curr_dim, total_nodes, &combined_edges);
            let out_self = layer.w_self.forward(&h_curr, total_nodes * curr_dim);
            let out_neigh = layer.w_neigh.forward(&neigh, total_nodes * curr_dim);

            let mut h_next = vec![0.0f32; total_nodes * out_dim];
            let has_residual = li > 0;
            for i in 0..total_nodes * out_dim {
                let sum = out_self[i] + out_neigh[i];
                let relu = if sum > 0.0 { sum } else { 0.0 };
                h_next[i] = if has_residual {
                    relu + h_prev.as_ref().unwrap()[i]
                } else {
                    relu
                };
            }
            h_prev = Some(h_next.clone());
            h_curr = h_next;
            curr_dim = out_dim;
        }

        let h_final = h_prev.unwrap();

        // Flatten Action Features & Node Embeddings: [h_final(u) (H) + act_feat (16)] = H+16
        let act_in_dim = HIDDEN_DIM + ACTION_FEAT_DIM;
        let mut act_in = vec![0.0f32; total_actions * act_in_dim];
        let mut global_act_idx = 0usize;

        for (i, obs) in observations.iter().enumerate() {
            let offset = node_offsets[i];
            let n_nodes = obs.node_features.len();

            for (a_idx, act) in obs.valid_actions.iter().enumerate() {
                let pos_idx = obs.node_positions.iter().position(|&p| p == (act.q, act.r)).unwrap_or(0);
                let u = offset + pos_idx.min(n_nodes.saturating_sub(1));

                act_in[global_act_idx * act_in_dim..global_act_idx * act_in_dim + HIDDEN_DIM]
                    .copy_from_slice(&h_final[u * HIDDEN_DIM..(u + 1) * HIDDEN_DIM]);
                if a_idx < obs.action_features.len() {
                    act_in[global_act_idx * act_in_dim + HIDDEN_DIM..(global_act_idx + 1) * act_in_dim]
                        .copy_from_slice(&obs.action_features[a_idx]);
                }
                global_act_idx += 1;
            }
        }

        let act_hidden = self.w_act1.forward(&act_in, total_actions * act_in_dim);
        let mut act_relu = vec![0.0f32; total_actions * HIDDEN_DIM];
        for i in 0..total_actions * HIDDEN_DIM {
            act_relu[i] = if act_hidden[i] > 0.0 { act_hidden[i] } else { 0.0 };
        }
        let all_action_logits = self.w_act2.forward(&act_relu, total_actions * HIDDEN_DIM);

        // Value Head for each Graph
        let mut mean_h_batch = vec![0.0f32; b_count * HIDDEN_DIM];
        for (i, obs) in observations.iter().enumerate() {
            let offset = node_offsets[i];
            let n_nodes = obs.node_features.len();
            if n_nodes > 0 {
                let inv_n = 1.0 / n_nodes as f32;
                for u in 0..n_nodes {
                    for d in 0..HIDDEN_DIM {
                        mean_h_batch[i * HIDDEN_DIM + d] += h_final[(offset + u) * HIDDEN_DIM + d] * inv_n;
                    }
                }
            }
        }

        let val_hidden = self.w_val1.forward(&mean_h_batch, b_count * HIDDEN_DIM);
        let mut val_relu = vec![0.0f32; b_count * HIDDEN_DIM];
        for i in 0..b_count * HIDDEN_DIM {
            val_relu[i] = if val_hidden[i] > 0.0 { val_hidden[i] } else { 0.0 };
        }
        let all_values = self.w_val2.forward(&val_relu, b_count * HIDDEN_DIM);

        let mut results = Vec::with_capacity(b_count);
        for (i, obs) in observations.iter().enumerate() {
            let a_start = action_offsets[i];
            let a_count = obs.valid_actions.len();
            let logits = all_action_logits[a_start..a_start + a_count].to_vec();
            let value = all_values[i];
            results.push((logits, value));
        }

        results
    }



    /// Backpropagation cho AlphaZero / Expert Iteration
    pub fn backward_accumulate_alphazero(
        &self,
        node_positions: &[(i32, i32)],
        node_features: &[[f32; NODE_FEAT_DIM]],
        edge_index: &[(usize, usize)],
        valid_actions: &[crate::env::Action],
        action_features: &[[f32; ACTION_FEAT_DIM]],
        target_probs: &[f32],
        value_grad: f32,
        grads: &mut HexGNNModel,
    ) {
        let num_actions = valid_actions.len();
        if num_actions == 0 || target_probs.len() != num_actions {
            return;
        }
        let (action_logits, _) = self.forward(node_positions, node_features, edge_index, valid_actions, action_features);
        let max_logit = action_logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = action_logits.iter().map(|l| (l - max_logit).exp()).collect();
        let sum_exp: f32 = exps.iter().sum();
        let probs: Vec<f32> = exps.iter().map(|e| e / sum_exp.max(1e-8)).collect();

        let mut d_logits = vec![0.0f32; num_actions];
        for a in 0..num_actions {
            d_logits[a] = probs[a] - target_probs[a];
        }

        self.backward_accumulate_with_d_logits(
            node_positions,
            node_features,
            edge_index,
            valid_actions,
            action_features,
            &d_logits,
            value_grad,
            grads,
        );
    }

    /// Backpropagation cốt lõi nhận vector d_logits và value_grad (4 Layers GNN)
    pub fn backward_accumulate_with_d_logits(
        &self,
        node_positions: &[(i32, i32)],
        node_features: &[[f32; NODE_FEAT_DIM]],
        edge_index: &[(usize, usize)],
        valid_actions: &[crate::env::Action],
        action_features: &[[f32; ACTION_FEAT_DIM]],
        d_logits: &[f32],
        value_grad: f32,
        grads: &mut HexGNNModel,
    ) {
        let n_nodes = node_features.len();
        let num_actions = valid_actions.len();
        if n_nodes == 0 || num_actions == 0 || d_logits.len() != num_actions {
            return;
        }

        // ================= FORWARD PASS TRACING =================
        let n_layers = self.layers.len();

        let mut x_flat = vec![0.0f32; n_nodes * NODE_FEAT_DIM];
        for u in 0..n_nodes {
            x_flat[u * NODE_FEAT_DIM..(u + 1) * NODE_FEAT_DIM].copy_from_slice(&node_features[u]);
        }

        // Lưu kết quả forward của từng layer để dùng trong backprop
        let mut h_vals: Vec<Vec<f32>> = Vec::with_capacity(n_layers);
        let mut h_pres: Vec<Vec<f32>> = Vec::with_capacity(n_layers);
        let mut neighs: Vec<Vec<f32>> = Vec::with_capacity(n_layers);
        let mut layer_in_dims: Vec<usize> = Vec::with_capacity(n_layers);

        let mut h_curr = x_flat.clone();
        let mut curr_dim = NODE_FEAT_DIM;

        for (li, layer) in self.layers.iter().enumerate() {
            let out_dim = HIDDEN_DIM;
            let neigh = Self::aggregate_neighbors(&h_curr, curr_dim, n_nodes, edge_index);
            let out_self = layer.w_self.forward(&h_curr, n_nodes * curr_dim);
            let out_neigh = layer.w_neigh.forward(&neigh, n_nodes * curr_dim);

            let mut h_pre = vec![0.0f32; n_nodes * out_dim];
            let mut h_val = vec![0.0f32; n_nodes * out_dim];
            let has_residual = li > 0;
            for i in 0..n_nodes * out_dim {
                let sum = out_self[i] + out_neigh[i];
                h_pre[i] = sum;
                let relu = if sum > 0.0 { sum } else { 0.0 };
                h_val[i] = if has_residual {
                    relu + h_vals[li - 1][i]
                } else {
                    relu
                };
            }
            layer_in_dims.push(curr_dim);
            neighs.push(neigh);
            h_pres.push(h_pre);
            h_vals.push(h_val.clone());
            h_curr = h_val;
            curr_dim = out_dim;
        }

        let h_final = h_vals.last().unwrap();

        // Action MLP Forward
        let act_in_dim = HIDDEN_DIM + ACTION_FEAT_DIM;
        let mut act_in = vec![0.0f32; num_actions * act_in_dim];
        let mut act_node_idx = Vec::with_capacity(num_actions);
        for (a_idx, act) in valid_actions.iter().enumerate() {
            let pos_idx = node_positions.iter().position(|&p| p == (act.q, act.r)).unwrap_or(0);
            let u = pos_idx.min(n_nodes - 1);
            act_node_idx.push(u);

            act_in[a_idx * act_in_dim..a_idx * act_in_dim + HIDDEN_DIM]
                .copy_from_slice(&h_final[u * HIDDEN_DIM..(u + 1) * HIDDEN_DIM]);
            if a_idx < action_features.len() {
                act_in[a_idx * act_in_dim + HIDDEN_DIM..(a_idx + 1) * act_in_dim]
                    .copy_from_slice(&action_features[a_idx]);
            }
        }
        let act_hidden = self.w_act1.forward(&act_in, num_actions * act_in_dim);
        let mut act_relu = vec![0.0f32; num_actions * HIDDEN_DIM];
        for i in 0..num_actions * HIDDEN_DIM {
            act_relu[i] = if act_hidden[i] > 0.0 { act_hidden[i] } else { 0.0 };
        }
        let _action_logits = self.w_act2.forward(&act_relu, num_actions * HIDDEN_DIM);

        // Value Forward
        let mut mean_h = vec![0.0f32; HIDDEN_DIM];
        for u in 0..n_nodes {
            for i in 0..HIDDEN_DIM {
                mean_h[i] += h_final[u * HIDDEN_DIM + i];
            }
        }
        for i in 0..HIDDEN_DIM {
            mean_h[i] /= n_nodes as f32;
        }
        let val_hidden = self.w_val1.forward(&mean_h, HIDDEN_DIM);
        let mut val_relu = vec![0.0f32; HIDDEN_DIM];
        for i in 0..HIDDEN_DIM {
            val_relu[i] = if val_hidden[i] > 0.0 { val_hidden[i] } else { 0.0 };
        }

        // ================= BACKPROPAGATION =================
        // 1. Action Head Gradient
        let mut d_act_relu = vec![0.0f32; num_actions * HIDDEN_DIM];
        for a in 0..num_actions {
            let d_z = d_logits[a];
            grads.w_act2.bias[0] += d_z;
            for j in 0..HIDDEN_DIM {
                grads.w_act2.weight[j] += d_z * act_relu[a * HIDDEN_DIM + j];
                d_act_relu[a * HIDDEN_DIM + j] = d_z * self.w_act2.weight[j];
            }
        }

        let mut d_act_in = vec![0.0f32; num_actions * act_in_dim];
        for a in 0..num_actions {
            for j in 0..HIDDEN_DIM {
                let d_h = if act_hidden[a * HIDDEN_DIM + j] > 0.0 { d_act_relu[a * HIDDEN_DIM + j] } else { 0.0 };
                grads.w_act1.bias[j] += d_h;
                let w_off = j * act_in_dim;
                for i in 0..act_in_dim {
                    grads.w_act1.weight[w_off + i] += d_h * act_in[a * act_in_dim + i];
                    d_act_in[a * act_in_dim + i] += d_h * self.w_act1.weight[w_off + i];
                }
            }
        }

        let mut d_h_final = vec![0.0f32; n_nodes * HIDDEN_DIM];
        for a in 0..num_actions {
            let u = act_node_idx[a];
            for i in 0..HIDDEN_DIM {
                d_h_final[u * HIDDEN_DIM + i] += d_act_in[a * act_in_dim + i];
            }
        }

        // 2. Value Head Gradient
        grads.w_val2.bias[0] += value_grad;
        let mut d_val_relu = vec![0.0f32; HIDDEN_DIM];
        for i in 0..HIDDEN_DIM {
            grads.w_val2.weight[i] += value_grad * val_relu[i];
            d_val_relu[i] = value_grad * self.w_val2.weight[i];
        }

        let mut d_mean_h = vec![0.0f32; HIDDEN_DIM];
        for i in 0..HIDDEN_DIM {
            let d_h = if val_hidden[i] > 0.0 { d_val_relu[i] } else { 0.0 };
            grads.w_val1.bias[i] += d_h;
            let w_off = i * HIDDEN_DIM;
            for j in 0..HIDDEN_DIM {
                grads.w_val1.weight[w_off + j] += d_h * mean_h[j];
                d_mean_h[j] += d_h * self.w_val1.weight[w_off + j];
            }
        }

        for u in 0..n_nodes {
            for i in 0..HIDDEN_DIM {
                d_h_final[u * HIDDEN_DIM + i] += d_mean_h[i] / n_nodes as f32;
            }
        }

        // Pre-compute neighbor counts
        let mut neighbor_count = vec![0usize; n_nodes];
        for &(u, _) in edge_index {
            if u < n_nodes {
                neighbor_count[u] += 1;
            }
        }

        // 3. Backprop through GNN layers (từ cuối về đầu)
        let mut d_h_curr = d_h_final;
        for li in (0..n_layers).rev() {
            let has_residual = li > 0;
            let in_dim = layer_in_dims[li];
            let out_dim = HIDDEN_DIM;
            let layer = &self.layers[li];
            let grad_layer = &mut grads.layers[li];

            let mut d_h_prev = if has_residual {
                vec![0.0f32; n_nodes * out_dim]
            } else {
                Vec::new()
            };

            let mut d_relu = vec![0.0f32; n_nodes * out_dim];
            if has_residual {
                for i in 0..n_nodes * out_dim {
                    d_h_prev[i] += d_h_curr[i];
                    d_relu[i] = if h_pres[li][i] > 0.0 { d_h_curr[i] } else { 0.0 };
                }
            } else {
                for i in 0..n_nodes * out_dim {
                    d_relu[i] = if h_pres[li][i] > 0.0 { d_h_curr[i] } else { 0.0 };
                }
            }

            let mut d_neigh = vec![0.0f32; n_nodes * out_dim];
            let h_in = if has_residual { &h_vals[li - 1] } else { &x_flat };

            for u in 0..n_nodes {
                for o in 0..out_dim {
                    let g = d_relu[u * out_dim + o];
                    grad_layer.w_self.bias[o] += g;
                    grad_layer.w_neigh.bias[o] += g;
                    let w_off = o * in_dim;
                    for i in 0..in_dim {
                        grad_layer.w_self.weight[w_off + i] += g * h_in[u * in_dim + i];
                        if has_residual {
                            d_h_prev[u * out_dim + i] += g * layer.w_self.weight[w_off + i];
                        }

                        grad_layer.w_neigh.weight[w_off + i] += g * neighs[li][u * in_dim + i];
                        d_neigh[u * out_dim + i] += g * layer.w_neigh.weight[w_off + i];
                    }
                }
            }

            if has_residual {
                for &(u, v) in edge_index {
                    if u < n_nodes && v < n_nodes {
                        let count_u = neighbor_count[u].max(1) as f32;
                        for i in 0..out_dim {
                            d_h_prev[v * out_dim + i] += d_neigh[u * out_dim + i] / count_u;
                        }
                    }
                }
                d_h_curr = d_h_prev;
            }
        }
    }

    pub fn update_weights_adam(&mut self, grads: &HexGNNModel, lr: f32) {
        self.step_count += 1;
        let t = self.step_count;
        let beta1 = 0.9f32;
        let beta2 = 0.999f32;
        let eps = 1e-8f32;

        for (l, gl) in self.layers.iter_mut().zip(grads.layers.iter()) {
            l.w_self.adam_update(&gl.w_self.weight, &gl.w_self.bias, lr, beta1, beta2, eps, t);
            l.w_neigh.adam_update(&gl.w_neigh.weight, &gl.w_neigh.bias, lr, beta1, beta2, eps, t);
        }
        self.w_act1.adam_update(&grads.w_act1.weight, &grads.w_act1.bias, lr, beta1, beta2, eps, t);
        self.w_act2.adam_update(&grads.w_act2.weight, &grads.w_act2.bias, lr, beta1, beta2, eps, t);
        self.w_val1.adam_update(&grads.w_val1.weight, &grads.w_val1.bias, lr, beta1, beta2, eps, t);
        self.w_val2.adam_update(&grads.w_val2.weight, &grads.w_val2.bias, lr, beta1, beta2, eps, t);
    }

    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);

        // V4 format: lưu số layers để forward/backward compatibility
        writer.write_all(b"DORF_GNN_V4")?;
        writer.write_all(&(self.step_count as u64).to_le_bytes())?;
        writer.write_all(&(self.layers.len() as u64).to_le_bytes())?;

        for layer in &self.layers {
            layer.w_self.save_to_writer(&mut writer)?;
            layer.w_neigh.save_to_writer(&mut writer)?;
        }
        self.w_act1.save_to_writer(&mut writer)?;
        self.w_act2.save_to_writer(&mut writer)?;
        self.w_val1.save_to_writer(&mut writer)?;
        self.w_val2.save_to_writer(&mut writer)?;

        writer.flush()?;
        Ok(())
    }

    pub fn load_from_file(path: &str) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let mut reader = std::io::BufReader::new(file);

        let mut magic = [0u8; 11];
        reader.read_exact(&mut magic)?;

        let (step_count, layers) = if &magic == b"DORF_GNN_V4" {
            let mut buf8 = [0u8; 8];
            reader.read_exact(&mut buf8)?;
            let step_count = u64::from_le_bytes(buf8) as usize;
            reader.read_exact(&mut buf8)?;
            let n_layers = u64::from_le_bytes(buf8) as usize;

            let mut layers = Vec::with_capacity(n_layers);
            for _ in 0..n_layers {
                let w_self = Linear::load_from_reader(&mut reader)?;
                let w_neigh = Linear::load_from_reader(&mut reader)?;
                layers.push(GNNLayer { w_self, w_neigh });
            }
            (step_count, layers)
        } else if &magic == b"DORF_GNN_V3" {
            // Backward compatible: V3 format (always 4 layers)
            let mut buf8 = [0u8; 8];
            reader.read_exact(&mut buf8)?;
            let step_count = u64::from_le_bytes(buf8) as usize;

            let w_self1 = Linear::load_from_reader(&mut reader)?;
            let w_neigh1 = Linear::load_from_reader(&mut reader)?;
            let w_self2 = Linear::load_from_reader(&mut reader)?;
            let w_neigh2 = Linear::load_from_reader(&mut reader)?;
            let w_self3 = Linear::load_from_reader(&mut reader)?;
            let w_neigh3 = Linear::load_from_reader(&mut reader)?;
            let w_self4 = Linear::load_from_reader(&mut reader)?;
            let w_neigh4 = Linear::load_from_reader(&mut reader)?;

            let layers = vec![
                GNNLayer { w_self: w_self1, w_neigh: w_neigh1 },
                GNNLayer { w_self: w_self2, w_neigh: w_neigh2 },
                GNNLayer { w_self: w_self3, w_neigh: w_neigh3 },
                GNNLayer { w_self: w_self4, w_neigh: w_neigh4 },
            ];
            (step_count, layers)
        } else {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,
                "Invalid model format (expected DORF_GNN_V3 or DORF_GNN_V4)"));
        };

        let w_act1 = Linear::load_from_reader(&mut reader)?;
        let w_act2 = Linear::load_from_reader(&mut reader)?;
        let w_val1 = Linear::load_from_reader(&mut reader)?;
        let w_val2 = Linear::load_from_reader(&mut reader)?;

        Ok(Self {
            layers,
            w_act1, w_act2,
            w_val1, w_val2,
            step_count,
        })
    }

    /// Đếm tổng số tham số (weights + biases) của toàn bộ model.
    pub fn param_count(&self) -> usize {
        let l = |ln: &Linear| ln.weight.len() + ln.bias.len();
        let mut total = 0;
        for layer in &self.layers {
            total += l(&layer.w_self) + l(&layer.w_neigh);
        }
        total + l(&self.w_act1) + l(&self.w_act2) + l(&self.w_val1) + l(&self.w_val2)
    }
}

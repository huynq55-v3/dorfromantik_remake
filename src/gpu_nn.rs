use wgpu::util::DeviceExt;
use std::sync::Arc;
use crate::nn::HexGNNModel;
use crate::env::GraphObservation;

/// WGSL GEMM Shader tối ưu: Y[row, col] = X[row, :] · W[col, :]^T + B[col], optional ReLU
const GEMM_SHADER: &str = r#"
struct Dims { M: u32, K: u32, N: u32, relu: u32 };

@group(0) @binding(0) var<uniform>            dims: Dims;
@group(0) @binding(1) var<storage, read>      x:    array<f32>;
@group(0) @binding(2) var<storage, read>      w:    array<f32>;
@group(0) @binding(3) var<storage, read>      b:    array<f32>;
@group(0) @binding(4) var<storage, read_write> y:   array<f32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let row = gid.x;
    let col = gid.y;
    if (row >= dims.M || col >= dims.N) { return; }
    var acc: f32 = b[col];
    for (var k: u32 = 0u; k < dims.K; k = k + 1u) {
        acc = acc + x[row * dims.K + k] * w[col * dims.K + k];
    }
    if (dims.relu == 1u && acc < 0.0) { acc = 0.0; }
    y[row * dims.N + col] = acc;
}
"#;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GemmDims { M: u32, K: u32, N: u32, relu: u32 }

/// GPU Neural Network Executor với Persistent Weight Buffers
///
/// Các weight được upload lên GPU VRAM MỘT LẦN DUY NHẤT khi khởi tạo.
/// Khi inference, chỉ cần upload input data (nhỏ) và readback output.
/// Sau mỗi training step, gọi sync_weights() để cập nhật weights.
pub struct GpuNNExecutor {
    pub device: Arc<wgpu::Device>,
    pub queue:  Arc<wgpu::Queue>,
    pipeline: wgpu::ComputePipeline,
    layout:   wgpu::BindGroupLayout,
    // ===== Persistent Weight Buffers (STORAGE | COPY_DST) =====
    // GNN Layer 1: 40 -> 128
    w_self1: wgpu::Buffer, b_self1: wgpu::Buffer,
    w_neigh1: wgpu::Buffer, b_neigh1: wgpu::Buffer,
    // GNN Layer 2: 128 -> 128
    w_self2: wgpu::Buffer, b_self2: wgpu::Buffer,
    w_neigh2: wgpu::Buffer, b_neigh2: wgpu::Buffer,
    // GNN Layer 3: 128 -> 128
    w_self3: wgpu::Buffer, b_self3: wgpu::Buffer,
    w_neigh3: wgpu::Buffer, b_neigh3: wgpu::Buffer,
    // GNN Layer 4: 128 -> 128
    w_self4: wgpu::Buffer, b_self4: wgpu::Buffer,
    w_neigh4: wgpu::Buffer, b_neigh4: wgpu::Buffer,
    // GNN Layer 5: 128 -> 128
    w_self5: wgpu::Buffer, b_self5: wgpu::Buffer,
    w_neigh5: wgpu::Buffer, b_neigh5: wgpu::Buffer,
    // GNN Layer 6: 128 -> 128
    w_self6: wgpu::Buffer, b_self6: wgpu::Buffer,
    w_neigh6: wgpu::Buffer, b_neigh6: wgpu::Buffer,
    // Action Head: 144 -> 128 -> 1
    w_act1: wgpu::Buffer, b_act1: wgpu::Buffer,
    w_act2: wgpu::Buffer, b_act2: wgpu::Buffer,
    // Value Head: 128 -> 128 -> 1
    w_val1: wgpu::Buffer, b_val1: wgpu::Buffer,
    w_val2: wgpu::Buffer, b_val2: wgpu::Buffer,
}

impl GpuNNExecutor {
    fn bgl_uniform(binding: u32) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding, visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false, min_binding_size: None,
            }, count: None,
        }
    }
    fn bgl_storage_ro(binding: u32) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding, visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false, min_binding_size: None,
            }, count: None,
        }
    }
    fn bgl_storage_rw(binding: u32) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding, visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false, min_binding_size: None,
            }, count: None,
        }
    }

    /// Tạo persistent weight buffer: STORAGE (shader read) + COPY_DST (cập nhật qua write_buffer)
    fn make_weight_buf(device: &wgpu::Device, data: &[f32]) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        })
    }

    /// Khởi tạo GpuNNExecutor - compile shader + upload TẤT CẢ weights lên GPU VRAM (1 lần duy nhất)
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, model: &HexGNNModel) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HexGNN GEMM Shader"),
            source: wgpu::ShaderSource::Wgsl(GEMM_SHADER.into()),
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GEMM BGL"),
            entries: &[
                Self::bgl_uniform(0),
                Self::bgl_storage_ro(1),
                Self::bgl_storage_ro(2),
                Self::bgl_storage_ro(3),
                Self::bgl_storage_rw(4),
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None, bind_group_layouts: &[&layout], push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("GEMM Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        macro_rules! wb { ($data:expr) => { Self::make_weight_buf(&device, $data) } }

        Self {
            w_self1:  wb!(&model.w_self1.weight),  b_self1:  wb!(&model.w_self1.bias),
            w_neigh1: wb!(&model.w_neigh1.weight), b_neigh1: wb!(&model.w_neigh1.bias),
            w_self2:  wb!(&model.w_self2.weight),  b_self2:  wb!(&model.w_self2.bias),
            w_neigh2: wb!(&model.w_neigh2.weight), b_neigh2: wb!(&model.w_neigh2.bias),
            w_self3:  wb!(&model.w_self3.weight),  b_self3:  wb!(&model.w_self3.bias),
            w_neigh3: wb!(&model.w_neigh3.weight), b_neigh3: wb!(&model.w_neigh3.bias),
            w_self4:  wb!(&model.w_self4.weight),  b_self4:  wb!(&model.w_self4.bias),
            w_neigh4: wb!(&model.w_neigh4.weight), b_neigh4: wb!(&model.w_neigh4.bias),
            w_self5:  wb!(&model.w_self5.weight),  b_self5:  wb!(&model.w_self5.bias),
            w_neigh5: wb!(&model.w_neigh5.weight), b_neigh5: wb!(&model.w_neigh5.bias),
            w_self6:  wb!(&model.w_self6.weight),  b_self6:  wb!(&model.w_self6.bias),
            w_neigh6: wb!(&model.w_neigh6.weight), b_neigh6: wb!(&model.w_neigh6.bias),
            w_act1:   wb!(&model.w_act1.weight),   b_act1:   wb!(&model.w_act1.bias),
            w_act2:   wb!(&model.w_act2.weight),   b_act2:   wb!(&model.w_act2.bias),
            w_val1:   wb!(&model.w_val1.weight),   b_val1:   wb!(&model.w_val1.bias),
            w_val2:   wb!(&model.w_val2.weight),   b_val2:   wb!(&model.w_val2.bias),
            pipeline, layout, device, queue,
        }
    }

    /// Cập nhật weights sau mỗi training step (dùng queue.write_buffer - zero-copy trên Intel iGPU)
    pub fn sync_weights(&self, model: &HexGNNModel) {
        macro_rules! ww {
            ($buf:expr, $data:expr) => {
                self.queue.write_buffer(&$buf, 0, bytemuck::cast_slice($data));
            }
        }
        ww!(self.w_self1,  &model.w_self1.weight);  ww!(self.b_self1,  &model.w_self1.bias);
        ww!(self.w_neigh1, &model.w_neigh1.weight); ww!(self.b_neigh1, &model.w_neigh1.bias);
        ww!(self.w_self2,  &model.w_self2.weight);  ww!(self.b_self2,  &model.w_self2.bias);
        ww!(self.w_neigh2, &model.w_neigh2.weight); ww!(self.b_neigh2, &model.w_neigh2.bias);
        ww!(self.w_self3,  &model.w_self3.weight);  ww!(self.b_self3,  &model.w_self3.bias);
        ww!(self.w_neigh3, &model.w_neigh3.weight); ww!(self.b_neigh3, &model.w_neigh3.bias);
        ww!(self.w_self4,  &model.w_self4.weight);  ww!(self.b_self4,  &model.w_self4.bias);
        ww!(self.w_neigh4, &model.w_neigh4.weight); ww!(self.b_neigh4, &model.w_neigh4.bias);
        ww!(self.w_self5,  &model.w_self5.weight);  ww!(self.b_self5,  &model.w_self5.bias);
        ww!(self.w_neigh5, &model.w_neigh5.weight); ww!(self.b_neigh5, &model.w_neigh5.bias);
        ww!(self.w_self6,  &model.w_self6.weight);  ww!(self.b_self6,  &model.w_self6.bias);
        ww!(self.w_neigh6, &model.w_neigh6.weight); ww!(self.b_neigh6, &model.w_neigh6.bias);
        ww!(self.w_act1,   &model.w_act1.weight);   ww!(self.b_act1,   &model.w_act1.bias);
        ww!(self.w_act2,   &model.w_act2.weight);   ww!(self.b_act2,   &model.w_act2.bias);
        ww!(self.w_val1,   &model.w_val1.weight);   ww!(self.b_val1,   &model.w_val1.bias);
        ww!(self.w_val2,   &model.w_val2.weight);   ww!(self.b_val2,   &model.w_val2.bias);
        // Flush all pending writes
        self.queue.submit([]);
    }

    // ===== Helper: tạo input buffer từ CPU data =====
    fn input_buf(&self, data: &[f32]) -> wgpu::Buffer {
        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(data),
            usage: wgpu::BufferUsages::STORAGE,
        })
    }

    // ===== Helper: tạo output buffer trên GPU =====
    fn output_buf(&self, n_f32: usize) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (n_f32 * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    // ===== Dispatch 1 GEMM vào encoder (không submit) =====
    fn dispatch_gemm(
        &self,
        enc: &mut wgpu::CommandEncoder,
        x: &wgpu::Buffer,
        w: &wgpu::Buffer,
        b: &wgpu::Buffer,
        y: &wgpu::Buffer,
        m: usize, k: usize, n: usize,
        relu: bool,
    ) {
        let dims = GemmDims { M: m as u32, K: k as u32, N: n as u32, relu: relu as u32 };
        let dims_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&dims),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None, layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: dims_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: x.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: w.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: b.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: y.as_entire_binding() },
            ],
        });
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bg, &[]);
        pass.dispatch_workgroups((m as u32 + 15) / 16, (n as u32 + 15) / 16, 1);
    }

    // ===== Submit 2 GEMMs + readback combined (1 encoder submit, 1 poll, 1 staging) =====
    fn two_gemm_readback(
        &self,
        x1: &wgpu::Buffer, w1: &wgpu::Buffer, b1: &wgpu::Buffer, m1: usize, k1: usize, n1: usize,
        x2: &wgpu::Buffer, w2: &wgpu::Buffer, b2: &wgpu::Buffer, m2: usize, k2: usize, n2: usize,
        relu1: bool, relu2: bool,
    ) -> (Vec<f32>, Vec<f32>) {
        let y1 = self.output_buf(m1 * n1);
        let y2 = self.output_buf(m2 * n2);

        let mut enc = self.device.create_command_encoder(&Default::default());
        self.dispatch_gemm(&mut enc, x1, w1, b1, &y1, m1, k1, n1, relu1);
        self.dispatch_gemm(&mut enc, x2, w2, b2, &y2, m2, k2, n2, relu2);

        // Combined staging buffer (both outputs)
        let size1 = m1 * n1 * 4;
        let size2 = m2 * n2 * 4;
        let combined_size = size1 + size2;
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: combined_size as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        enc.copy_buffer_to_buffer(&y1, 0, &staging, 0, size1 as u64);
        enc.copy_buffer_to_buffer(&y2, 0, &staging, size1 as u64, size2 as u64);
        self.queue.submit(Some(enc.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = crossbeam_channel::bounded(1);
        slice.map_async(wgpu::MapMode::Read, move |r| { let _ = tx.send(r); });
        self.device.poll(wgpu::Maintain::Wait);
        let _ = rx.recv();

        let data = slice.get_mapped_range();
        let all: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);

        let out1 = all[0..m1 * n1].to_vec();
        let out2 = all[m1 * n1..m1 * n1 + m2 * n2].to_vec();
        (out1, out2)
    }

    // ===== CPU Neighbor Aggregation (Mean Pooling) =====
    fn cpu_aggregate(h: &[f32], dim: usize, n_nodes: usize, edges: &[(usize, usize)]) -> Vec<f32> {
        let mut sum = vec![0.0f32; n_nodes * dim];
        let mut cnt = vec![0u32; n_nodes];
        for &(u, v) in edges {
            if u < n_nodes && v < n_nodes {
                for d in 0..dim {
                    sum[u * dim + d] += h[v * dim + d];
                }
                cnt[u] += 1;
            }
        }
        let mut out = vec![0.0f32; n_nodes * dim];
        for u in 0..n_nodes {
            let c = cnt[u].max(1) as f32;
            for d in 0..dim { out[u * dim + d] = sum[u * dim + d] / c; }
        }
        out
    }

    // ===== Một GNN Layer: submit 2 GEMMs (self+neigh) song song trên GPU, CPU add+relu+residual =====
    fn gnn_layer_gpu(
        &self,
        h: &[f32],
        agg: &[f32],
        n_nodes: usize,
        in_dim: usize,
        out_dim: usize,
        w_self: &wgpu::Buffer, b_self: &wgpu::Buffer,
        w_neigh: &wgpu::Buffer, b_neigh: &wgpu::Buffer,
        residual: bool,  // true cho layers 2-6, false cho layer 1
    ) -> Vec<f32> {
        let x_buf = self.input_buf(h);
        let n_buf = self.input_buf(agg);

        let (ys, yn) = self.two_gemm_readback(
            &x_buf, w_self, b_self, n_nodes, in_dim, out_dim,
            &n_buf, w_neigh, b_neigh, n_nodes, in_dim, out_dim,
            false, false,
        );

        let mut h_new = vec![0.0f32; n_nodes * out_dim];
        for i in 0..n_nodes * out_dim {
            let sum = ys[i] + yn[i];
            let relu = sum.max(0.0);
            // Residual connection chỉ khi in_dim == out_dim (layers 2-6)
            h_new[i] = if residual { relu + h[i] } else { relu };
        }
        h_new
    }

    /// Forward Batch toàn bộ trên GPU Intel Iris Xe - Persistent Weights, tối thiểu GPU roundtrips
    ///
    /// Thay vì cấp phát weight buffer mỗi lần, weights đã được upload sẵn trong GPU VRAM.
    /// Mỗi GNN layer: 1 GPU submit (2 GEMMs song song) + 1 readback.
    /// Heads: 2 submits (act+val song song) + 2 readbacks.
    /// Tổng: ~8 GPU submits/call thay vì 32+ như trước.
    pub fn forward_batch_gpu(&self, observations: &[&GraphObservation]) -> Vec<(Vec<f32>, f32)> {
        let batch = observations.len();
        if batch == 0 { return Vec::new(); }

        // ===== 1. Build batched disjoint graph =====
        let mut total_nodes = 0usize;
        let mut total_actions = 0usize;
        let mut node_offsets = Vec::with_capacity(batch);
        let mut action_offsets = Vec::with_capacity(batch);

        for obs in observations.iter() {
            node_offsets.push(total_nodes);
            action_offsets.push(total_actions);
            total_nodes += obs.node_features.len();
            total_actions += obs.valid_actions.len();
        }

        if total_nodes == 0 || total_actions == 0 {
            return observations.iter().map(|o| (vec![0.0f32; o.valid_actions.len()], 0.0f32)).collect();
        }

        // ===== 2. Flatten node features [total_nodes, 40] =====
        let mut h0 = Vec::with_capacity(total_nodes * 40);
        let mut edges: Vec<(usize, usize)> = Vec::new();

        for (i, obs) in observations.iter().enumerate() {
            let off = node_offsets[i];
            for feat in &obs.node_features {
                h0.extend_from_slice(feat);
            }
            for &(u, v) in &obs.edge_index {
                edges.push((u + off, v + off));
            }
        }

        // ===== 3. 6 GNN Layers trên GPU (Weights persistent trong VRAM) =====
        // Layer 1: 40 -> 128
        let agg0 = Self::cpu_aggregate(&h0, 40, total_nodes, &edges);
        let h1 = self.gnn_layer_gpu(&h0, &agg0, total_nodes, 40, 128,
            &self.w_self1, &self.b_self1, &self.w_neigh1, &self.b_neigh1, false);

        // Layer 2: 128 -> 128 (residual)
        let agg1 = Self::cpu_aggregate(&h1, 128, total_nodes, &edges);
        let h2 = self.gnn_layer_gpu(&h1, &agg1, total_nodes, 128, 128,
            &self.w_self2, &self.b_self2, &self.w_neigh2, &self.b_neigh2, true);

        // Layer 3
        let agg2 = Self::cpu_aggregate(&h2, 128, total_nodes, &edges);
        let h3 = self.gnn_layer_gpu(&h2, &agg2, total_nodes, 128, 128,
            &self.w_self3, &self.b_self3, &self.w_neigh3, &self.b_neigh3, true);

        // Layer 4
        let agg3 = Self::cpu_aggregate(&h3, 128, total_nodes, &edges);
        let h4 = self.gnn_layer_gpu(&h3, &agg3, total_nodes, 128, 128,
            &self.w_self4, &self.b_self4, &self.w_neigh4, &self.b_neigh4, true);

        // Layer 5
        let agg4 = Self::cpu_aggregate(&h4, 128, total_nodes, &edges);
        let h5 = self.gnn_layer_gpu(&h4, &agg4, total_nodes, 128, 128,
            &self.w_self5, &self.b_self5, &self.w_neigh5, &self.b_neigh5, true);

        // Layer 6
        let agg5 = Self::cpu_aggregate(&h5, 128, total_nodes, &edges);
        let h6 = self.gnn_layer_gpu(&h5, &agg5, total_nodes, 128, 128,
            &self.w_self6, &self.b_self6, &self.w_neigh6, &self.b_neigh6, true);

        // ===== 4. Gather Action Inputs [total_actions, 144] =====
        let mut act_in = vec![0.0f32; total_actions * 144];
        let mut g_act = 0usize;
        for (i, obs) in observations.iter().enumerate() {
            let off = node_offsets[i];
            let n = obs.node_features.len();
            for (a_idx, act) in obs.valid_actions.iter().enumerate() {
                // Tìm node embedding tương ứng với action position
                let pos = obs.node_positions.iter().position(|&p| p == (act.q, act.r)).unwrap_or(0);
                let u = off + pos.min(n.saturating_sub(1));
                act_in[g_act * 144..g_act * 144 + 128].copy_from_slice(&h6[u * 128..(u + 1) * 128]);
                if a_idx < obs.action_features.len() {
                    act_in[g_act * 144 + 128..(g_act + 1) * 144].copy_from_slice(&obs.action_features[a_idx]);
                }
                g_act += 1;
            }
        }

        // ===== 5. Value Head Input: Mean Pool h6 per graph [batch, 128] =====
        let mut val_in = vec![0.0f32; batch * 128];
        for (i, obs) in observations.iter().enumerate() {
            let off = node_offsets[i];
            let n = obs.node_features.len();
            if n > 0 {
                let inv = 1.0 / n as f32;
                for u in 0..n {
                    for d in 0..128 {
                        val_in[i * 128 + d] += h6[(off + u) * 128 + d] * inv;
                    }
                }
            }
        }

        // ===== 6. Action Head + Value Head Layer 1 (song song trên GPU) =====
        let act_in_buf = self.input_buf(&act_in);
        let val_in_buf = self.input_buf(&val_in);
        let (act_h, val_h) = self.two_gemm_readback(
            &act_in_buf, &self.w_act1, &self.b_act1, total_actions, 144, 128,
            &val_in_buf, &self.w_val1, &self.b_val1, batch,         128, 128,
            true, true,
        );

        // ===== 7. Action Head + Value Head Layer 2 (song song trên GPU) =====
        let act_h_buf = self.input_buf(&act_h);
        let val_h_buf = self.input_buf(&val_h);
        let (act_logits, val_raw) = self.two_gemm_readback(
            &act_h_buf, &self.w_act2, &self.b_act2, total_actions, 128, 1,
            &val_h_buf, &self.w_val2, &self.b_val2, batch,         128, 1,
            false, false,
        );

        // ===== 8. Tách kết quả theo từng graph =====
        let mut results = Vec::with_capacity(batch);
        for (i, obs) in observations.iter().enumerate() {
            let a_start = action_offsets[i];
            let a_count = obs.valid_actions.len();
            let logits = act_logits[a_start..a_start + a_count].to_vec();
            let value = val_raw[i];
            results.push((logits, value));
        }
        results
    }

    /// Legacy: forward đơn lẻ (tương thích ngược)
    pub fn gpu_linear_forward(
        &self,
        input: &[f32],
        batch_size: usize,
        in_features: usize,
        out_features: usize,
        weight: &[f32],
        bias: &[f32],
        add_relu: bool,
    ) -> Vec<f32> {
        let x_buf = self.input_buf(input);
        let w_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None, contents: bytemuck::cast_slice(weight),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let b_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None, contents: bytemuck::cast_slice(bias),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let out_n = batch_size * out_features;
        let y_buf = self.output_buf(out_n);
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None, size: (out_n * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut enc = self.device.create_command_encoder(&Default::default());
        self.dispatch_gemm(&mut enc, &x_buf, &w_buf, &b_buf, &y_buf, batch_size, in_features, out_features, add_relu);
        enc.copy_buffer_to_buffer(&y_buf, 0, &staging, 0, (out_n * 4) as u64);
        self.queue.submit(Some(enc.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = crossbeam_channel::bounded(1);
        slice.map_async(wgpu::MapMode::Read, move |r| { let _ = tx.send(r); });
        self.device.poll(wgpu::Maintain::Wait);
        let _ = rx.recv();
        let data = slice.get_mapped_range();
        bytemuck::cast_slice(&data).to_vec()
    }
}

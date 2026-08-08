use wgpu::util::DeviceExt;
use std::sync::Arc;
use crate::nn::HexGNNModel;
use crate::env::GraphObservation;

/// WGSL GEMM Shader: Y[row, col] = X[row, :] · W[col, :]^T + B[col], optional ReLU
const GEMM_SHADER: &str = r#"
struct Dims { m: u32, k: u32, n: u32, relu: u32 };

@group(0) @binding(0) var<uniform>            dims: Dims;
@group(0) @binding(1) var<storage, read>      x:    array<f32>;
@group(0) @binding(2) var<storage, read>      w:    array<f32>;
@group(0) @binding(3) var<storage, read>      b:    array<f32>;
@group(0) @binding(4) var<storage, read_write> y:   array<f32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let row = gid.x;
    let col = gid.y;
    if (row >= dims.m || col >= dims.n) { return; }
    var acc: f32 = b[col];
    for (var k: u32 = 0u; k < dims.k; k = k + 1u) {
        acc = acc + x[row * dims.k + k] * w[col * dims.k + k];
    }
    if (dims.relu == 1u && acc < 0.0) { acc = 0.0; }
    y[row * dims.n + col] = acc;
}
"#;

/// WGSL CSR Aggregation Shader: h_out[u, d] = mean_{v in N(u)} h_in[v, d] - O(Deg(u)) siêu tốc!
const AGG_CSR_SHADER: &str = r#"
struct AggCsrDims { n_nodes: u32, dim: u32 };

@group(0) @binding(0) var<uniform>          dims:    AggCsrDims;
@group(0) @binding(1) var<storage, read>    h_in:    array<f32>;
@group(0) @binding(2) var<storage, read>    offsets: array<u32>;
@group(0) @binding(3) var<storage, read>    targets: array<u32>;
@group(0) @binding(4) var<storage, read_write> h_out:  array<f32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let node = gid.x;
    let feat = gid.y;
    if (node >= dims.n_nodes || feat >= dims.dim) { return; }

    let start = offsets[node];
    let end   = offsets[node + 1u];
    let count = end - start;

    var sum: f32 = 0.0;
    for (var i: u32 = start; i < end; i = i + 1u) {
        let v = targets[i];
        sum = sum + h_in[v * dims.dim + feat];
    }

    let inv = select(1.0 / f32(count), 0.0, count == 0u);
    h_out[node * dims.dim + feat] = sum * inv;
}
"#;

/// WGSL Combine Shader: h_out = relu(ys + yn) [+ h_prev if residual]
const COMBINE_SHADER: &str = r#"
struct CombineDims { n_nodes: u32, dim: u32, residual: u32, _p1: u32 };

@group(0) @binding(0) var<uniform>          dims:   CombineDims;
@group(0) @binding(1) var<storage, read>    ys:     array<f32>;
@group(0) @binding(2) var<storage, read>    yn:     array<f32>;
@group(0) @binding(3) var<storage, read>    h_prev: array<f32>;
@group(0) @binding(4) var<storage, read_write> h_out: array<f32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let node = gid.x;
    let feat = gid.y;
    if (node >= dims.n_nodes || feat >= dims.dim) { return; }

    let i = node * dims.dim + feat;
    let sum = ys[i] + yn[i];
    let relu = max(sum, 0.0);
    if (dims.residual == 1u) {
        h_out[i] = relu + h_prev[i];
    } else {
        h_out[i] = relu;
    }
}
"#;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GemmDims { m: u32, k: u32, n: u32, relu: u32 }

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct AggCsrDims { n_nodes: u32, dim: u32 }

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CombineDims { n_nodes: u32, dim: u32, residual: u32, _p1: u32 }

/// GPU Neural Network Executor V1 Supercharged (3 Layers GNN, Hidden Dim = 64, O(Deg) CSR Aggregation)
pub struct GpuNNExecutor {
    pub device: Arc<wgpu::Device>,
    pub queue:  Arc<wgpu::Queue>,
    gemm_pipeline:    wgpu::ComputePipeline,
    gemm_layout:      wgpu::BindGroupLayout,
    agg_pipeline:     wgpu::ComputePipeline,
    agg_layout:       wgpu::BindGroupLayout,
    combine_pipeline: wgpu::ComputePipeline,
    combine_layout:   wgpu::BindGroupLayout,

    // Persistent Weights (VRAM)
    w_self1: wgpu::Buffer, b_self1: wgpu::Buffer,
    w_neigh1: wgpu::Buffer, b_neigh1: wgpu::Buffer,
    w_self2: wgpu::Buffer, b_self2: wgpu::Buffer,
    w_neigh2: wgpu::Buffer, b_neigh2: wgpu::Buffer,
    w_self3: wgpu::Buffer, b_self3: wgpu::Buffer,
    w_neigh3: wgpu::Buffer, b_neigh3: wgpu::Buffer,
    w_act1: wgpu::Buffer, b_act1: wgpu::Buffer,
    w_act2: wgpu::Buffer, b_act2: wgpu::Buffer,
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

    fn make_weight_buf(device: &wgpu::Device, data: &[f32]) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        })
    }

    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, model: &HexGNNModel) -> Self {
        // 1. GEMM Pipeline
        let gemm_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("GEMM"), source: wgpu::ShaderSource::Wgsl(GEMM_SHADER.into()),
        });
        let gemm_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GEMM BGL"),
            entries: &[
                Self::bgl_uniform(0), Self::bgl_storage_ro(1),
                Self::bgl_storage_ro(2), Self::bgl_storage_ro(3),
                Self::bgl_storage_rw(4),
            ],
        });
        let gemm_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None, bind_group_layouts: &[&gemm_layout], push_constant_ranges: &[],
        });
        let gemm_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("GEMM Pipeline"), layout: Some(&gemm_pl),
            module: &gemm_shader, entry_point: Some("main"), compilation_options: Default::default(), cache: None,
        });

        // 2. CSR Aggregation Pipeline
        let agg_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Agg CSR"), source: wgpu::ShaderSource::Wgsl(AGG_CSR_SHADER.into()),
        });
        let agg_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Agg CSR BGL"),
            entries: &[
                Self::bgl_uniform(0), Self::bgl_storage_ro(1),
                Self::bgl_storage_ro(2), Self::bgl_storage_ro(3),
                Self::bgl_storage_rw(4),
            ],
        });
        let agg_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None, bind_group_layouts: &[&agg_layout], push_constant_ranges: &[],
        });
        let agg_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Agg CSR Pipeline"), layout: Some(&agg_pl),
            module: &agg_shader, entry_point: Some("main"), compilation_options: Default::default(), cache: None,
        });

        // 3. Combine Pipeline
        let combine_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Combine"), source: wgpu::ShaderSource::Wgsl(COMBINE_SHADER.into()),
        });
        let combine_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Combine BGL"),
            entries: &[
                Self::bgl_uniform(0), Self::bgl_storage_ro(1),
                Self::bgl_storage_ro(2), Self::bgl_storage_ro(3),
                Self::bgl_storage_rw(4),
            ],
        });
        let combine_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None, bind_group_layouts: &[&combine_layout], push_constant_ranges: &[],
        });
        let combine_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Combine Pipeline"), layout: Some(&combine_pl),
            module: &combine_shader, entry_point: Some("main"), compilation_options: Default::default(), cache: None,
        });

        macro_rules! wb { ($slice:expr) => { Self::make_weight_buf(&device, $slice) } }
        let w_self1  = wb!(&model.w_self1.weight);  let b_self1  = wb!(&model.w_self1.bias);
        let w_neigh1 = wb!(&model.w_neigh1.weight); let b_neigh1 = wb!(&model.w_neigh1.bias);
        let w_self2  = wb!(&model.w_self2.weight);  let b_self2  = wb!(&model.w_self2.bias);
        let w_neigh2 = wb!(&model.w_neigh2.weight); let b_neigh2 = wb!(&model.w_neigh2.bias);
        let w_self3  = wb!(&model.w_self3.weight);  let b_self3  = wb!(&model.w_self3.bias);
        let w_neigh3 = wb!(&model.w_neigh3.weight); let b_neigh3 = wb!(&model.w_neigh3.bias);
        let w_act1   = wb!(&model.w_act1.weight);   let b_act1   = wb!(&model.w_act1.bias);
        let w_act2   = wb!(&model.w_act2.weight);   let b_act2   = wb!(&model.w_act2.bias);
        let w_val1   = wb!(&model.w_val1.weight);   let b_val1   = wb!(&model.w_val1.bias);
        let w_val2   = wb!(&model.w_val2.weight);   let b_val2   = wb!(&model.w_val2.bias);

        Self {
            device, queue,
            gemm_pipeline, gemm_layout,
            agg_pipeline, agg_layout,
            combine_pipeline, combine_layout,
            w_self1, b_self1, w_neigh1, b_neigh1,
            w_self2, b_self2, w_neigh2, b_neigh2,
            w_self3, b_self3, w_neigh3, b_neigh3,
            w_act1, b_act1, w_act2, b_act2,
            w_val1, b_val1, w_val2, b_val2,
        }
    }

    pub fn update_weights_from_model(&self, model: &HexGNNModel) {
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
        ww!(self.w_act1,   &model.w_act1.weight);   ww!(self.b_act1,   &model.w_act1.bias);
        ww!(self.w_act2,   &model.w_act2.weight);   ww!(self.b_act2,   &model.w_act2.bias);
        ww!(self.w_val1,   &model.w_val1.weight);   ww!(self.b_val1,   &model.w_val1.bias);
        ww!(self.w_val2,   &model.w_val2.weight);   ww!(self.b_val2,   &model.w_val2.bias);
        self.queue.submit([]);
    }

    pub fn sync_weights(&self, model: &HexGNNModel) {
        self.update_weights_from_model(model);
    }

    fn storage_buf(&self, n_f32: usize) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (n_f32 * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    fn init_storage_buf<T: bytemuck::Pod>(&self, data: &[T]) -> wgpu::Buffer {
        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        })
    }

    fn dispatch_agg_csr(
        &self, enc: &mut wgpu::CommandEncoder,
        h_in: &wgpu::Buffer, offsets: &wgpu::Buffer, targets: &wgpu::Buffer, h_out: &wgpu::Buffer,
        n_nodes: usize, dim: usize,
    ) {
        let dims = AggCsrDims { n_nodes: n_nodes as u32, dim: dim as u32 };
        let dims_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None, contents: bytemuck::bytes_of(&dims), usage: wgpu::BufferUsages::UNIFORM,
        });
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None, layout: &self.agg_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: dims_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: h_in.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: offsets.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: targets.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: h_out.as_entire_binding() },
            ],
        });
        let mut pass = enc.begin_compute_pass(&Default::default());
        pass.set_pipeline(&self.agg_pipeline);
        pass.set_bind_group(0, &bg, &[]);
        pass.dispatch_workgroups((n_nodes as u32 + 15) / 16, (dim as u32 + 15) / 16, 1);
    }

    fn dispatch_gemm(
        &self, enc: &mut wgpu::CommandEncoder,
        x: &wgpu::Buffer, w: &wgpu::Buffer, b: &wgpu::Buffer, y: &wgpu::Buffer,
        m: usize, k: usize, n: usize, relu: bool,
    ) {
        let dims = GemmDims { m: m as u32, k: k as u32, n: n as u32, relu: relu as u32 };
        let dims_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None, contents: bytemuck::bytes_of(&dims), usage: wgpu::BufferUsages::UNIFORM,
        });
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None, layout: &self.gemm_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: dims_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: x.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: w.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: b.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: y.as_entire_binding() },
            ],
        });
        let mut pass = enc.begin_compute_pass(&Default::default());
        pass.set_pipeline(&self.gemm_pipeline);
        pass.set_bind_group(0, &bg, &[]);
        pass.dispatch_workgroups((m as u32 + 15) / 16, (n as u32 + 15) / 16, 1);
    }

    fn dispatch_combine(
        &self, enc: &mut wgpu::CommandEncoder,
        ys: &wgpu::Buffer, yn: &wgpu::Buffer, h_prev: &wgpu::Buffer, h_out: &wgpu::Buffer,
        n_nodes: usize, dim: usize, residual: bool,
    ) {
        let dims = CombineDims { n_nodes: n_nodes as u32, dim: dim as u32, residual: residual as u32, _p1: 0 };
        let dims_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None, contents: bytemuck::bytes_of(&dims), usage: wgpu::BufferUsages::UNIFORM,
        });
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None, layout: &self.combine_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: dims_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: ys.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: yn.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: h_prev.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: h_out.as_entire_binding() },
            ],
        });
        let mut pass = enc.begin_compute_pass(&Default::default());
        pass.set_pipeline(&self.combine_pipeline);
        pass.set_bind_group(0, &bg, &[]);
        pass.dispatch_workgroups((n_nodes as u32 + 15) / 16, (dim as u32 + 15) / 16, 1);
    }

    fn readback(&self, src: &wgpu::Buffer, n_f32: usize) -> Vec<f32> {
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None, size: (n_f32 * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(src, 0, &staging, 0, (n_f32 * 4) as u64);
        self.queue.submit(Some(enc.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = crossbeam_channel::bounded(1);
        slice.map_async(wgpu::MapMode::Read, move |r| { let _ = tx.send(r); });
        self.device.poll(wgpu::Maintain::Wait);
        let _ = rx.recv();
        let data = slice.get_mapped_range();
        bytemuck::cast_slice(&data).to_vec()
    }

    /// Forward Pass CSR Supercharged trên GPU (O(Deg) Aggregation)
    pub fn forward_batch_gpu(&self, observations: &[&GraphObservation]) -> Vec<(Vec<f32>, f32)> {
        let batch = observations.len();
        if batch == 0 { return Vec::new(); }

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

        let mut h0 = Vec::with_capacity(total_nodes * 40);

        // --- Build Compressed Sparse Row (CSR) Format cho Graph Aggregation ---
        let mut csr_offsets = Vec::with_capacity(total_nodes + 1);
        let mut csr_targets = Vec::new();
        let mut curr_csr_offset = 0u32;
        csr_offsets.push(0u32);

        for (i, obs) in observations.iter().enumerate() {
            let off = node_offsets[i] as u32;
            let n = obs.node_features.len();

            for feat in &obs.node_features {
                h0.extend_from_slice(feat);
            }

            let mut adj = vec![Vec::new(); n];
            for &(u, v) in &obs.edge_index {
                if u < n && v < n {
                    adj[u].push((v as u32) + off);
                }
            }

            for u in 0..n {
                for &v_target in &adj[u] {
                    csr_targets.push(v_target);
                    curr_csr_offset += 1;
                }
                csr_offsets.push(curr_csr_offset);
            }
        }

        let h0_buf = self.init_storage_buf(&h0);
        let offsets_buf = self.init_storage_buf(&csr_offsets);
        let targets_buf = self.init_storage_buf(&csr_targets);

        let h1_buf = self.storage_buf(total_nodes * 64);
        let h2_buf = self.storage_buf(total_nodes * 64);
        let h3_buf = self.storage_buf(total_nodes * 64);

        let agg0_buf = self.storage_buf(total_nodes * 40);
        let agg1_buf = self.storage_buf(total_nodes * 64);
        let agg2_buf = self.storage_buf(total_nodes * 64);

        let ys_buf = self.storage_buf(total_nodes * 64);
        let yn_buf = self.storage_buf(total_nodes * 64);

        // ===== 3 LAYERS GNN VỚI CSR AGGREGATION $O(\text{Deg}(u))$ TRÊN GPU =====
        let mut enc = self.device.create_command_encoder(&Default::default());

        macro_rules! pipeline_layer {
            ($h_in:expr, $agg:expr, $h_out:expr, $dim_in:expr, $w_s:expr, $b_s:expr, $w_n:expr, $b_n:expr, $res:expr) => {
                self.dispatch_agg_csr(&mut enc, $h_in, &offsets_buf, &targets_buf, $agg, total_nodes, $dim_in);
                self.dispatch_gemm(&mut enc, $h_in, $w_s, $b_s, &ys_buf, total_nodes, $dim_in, 64, false);
                self.dispatch_gemm(&mut enc, $agg,  $w_n, $b_n, &yn_buf, total_nodes, $dim_in, 64, false);
                self.dispatch_combine(&mut enc, &ys_buf, &yn_buf, $h_in, $h_out, total_nodes, 64, $res);
            };
        }

        pipeline_layer!(&h0_buf, &agg0_buf, &h1_buf, 40, &self.w_self1, &self.b_self1, &self.w_neigh1, &self.b_neigh1, false);
        pipeline_layer!(&h1_buf, &agg1_buf, &h2_buf, 64, &self.w_self2, &self.b_self2, &self.w_neigh2, &self.b_neigh2, true);
        pipeline_layer!(&h2_buf, &agg2_buf, &h3_buf, 64, &self.w_self3, &self.b_self3, &self.w_neigh3, &self.b_neigh3, true);

        // Single submit & readback cho 3 GNN layers
        self.queue.submit(Some(enc.finish()));
        let h3 = self.readback(&h3_buf, total_nodes * 64);

        // ===== Build Action Head & Value Head Inputs trên CPU =====
        let mut act_in = vec![0.0f32; total_actions * 80];
        let mut g_act = 0usize;
        for (i, obs) in observations.iter().enumerate() {
            let off = node_offsets[i];
            let n = obs.node_features.len();
            for (a_idx, act) in obs.valid_actions.iter().enumerate() {
                let pos = obs.node_positions.iter().position(|&p| p == (act.q, act.r)).unwrap_or(0);
                let u = off + pos.min(n.saturating_sub(1));
                act_in[g_act * 80..g_act * 80 + 64].copy_from_slice(&h3[u * 64..(u + 1) * 64]);
                if a_idx < obs.action_features.len() {
                    act_in[g_act * 80 + 64..(g_act + 1) * 80].copy_from_slice(&obs.action_features[a_idx]);
                }
                g_act += 1;
            }
        }

        let mut val_in = vec![0.0f32; batch * 64];
        for (i, obs) in observations.iter().enumerate() {
            let off = node_offsets[i];
            let n = obs.node_features.len();
            if n > 0 {
                let inv = 1.0 / n as f32;
                for u in 0..n {
                    for d in 0..64 {
                        val_in[i * 64 + d] += h3[(off + u) * 64 + d] * inv;
                    }
                }
            }
        }

        // ===== GEMM cho Action Head & Value Head =====
        let act_in_buf = self.init_storage_buf(&act_in);
        let val_in_buf = self.init_storage_buf(&val_in);
        let act_h_buf  = self.storage_buf(total_actions * 64);
        let val_h_buf  = self.storage_buf(batch * 64);
        let act_o_buf  = self.storage_buf(total_actions);
        let val_o_buf  = self.storage_buf(batch);

        let mut enc_heads = self.device.create_command_encoder(&Default::default());
        self.dispatch_gemm(&mut enc_heads, &act_in_buf, &self.w_act1, &self.b_act1, &act_h_buf, total_actions, 80, 64, true);
        self.dispatch_gemm(&mut enc_heads, &val_in_buf, &self.w_val1, &self.b_val1, &val_h_buf, batch,         64, 64, true);
        self.dispatch_gemm(&mut enc_heads, &act_h_buf,  &self.w_act2, &self.b_act2, &act_o_buf, total_actions, 64, 1,  false);
        self.dispatch_gemm(&mut enc_heads, &val_h_buf,  &self.w_val2, &self.b_val2, &val_o_buf, batch,         64, 1,  false);
        self.queue.submit(Some(enc_heads.finish()));

        let all_logits = self.readback(&act_o_buf, total_actions);
        let all_vals   = self.readback(&val_o_buf, batch);

        let mut results = Vec::with_capacity(batch);
        for (i, obs) in observations.iter().enumerate() {
            let a_start = action_offsets[i];
            let a_count = obs.valid_actions.len();
            let logits = all_logits[a_start..a_start + a_count].to_vec();
            let value = all_vals[i];
            results.push((logits, value));
        }

        results
    }
}

use std::sync::Arc;
use rayon::prelude::*;
use crate::nn::{HexGNNModel, HIDDEN_DIM, ACTION_FEAT_DIM, NODE_FEAT_DIM};
use crate::env::GraphObservation;

/// GEMM dims: số chiều input (k) và output (n) được truyền động qua uniform, nên layer-1 (k=40,
/// n=128) và các layer 2-4 (k=n=128) đều tái sử dụng cùng một pipeline.

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

/// WGSL CSR Aggregation Shader: h_out[u, d] = mean_{v in N(u)} h_in[v, d]
const AGG_CSR_SHADER: &str = r#"
struct AggCsrDims { n_nodes: u32, dim: u32, _p1: u32, _p2: u32 };

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

/// WGSL Action Head Gather Shader:
/// Gộp hN[u] (HIDDEN_DIM dims) + action_features (ACTION_FEAT_DIM dims) -> act_in (HIDDEN+ACTION dims).
/// Kích thước hidden & action được truyền qua uniform dims để linh hoạt.
const GATHER_ACT_SHADER: &str = r#"
struct GatherDims { total_actions: u32, hidden_dim: u32, action_dim: u32, _p3: u32 };

@group(0) @binding(0) var<uniform>          dims:        GatherDims;
@group(0) @binding(1) var<storage, read>    hN:          array<f32>;
@group(0) @binding(2) var<storage, read>    act_node_u:  array<u32>;
@group(0) @binding(3) var<storage, read>    act_feat:    array<f32>;
@group(0) @binding(4) var<storage, read_write> act_in:   array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let a = gid.x;
    if (a >= dims.total_actions) { return; }

    let u = act_node_u[a];
    let in_dim = dims.hidden_dim + dims.action_dim;
    let in_off = a * in_dim;

    for (var i: u32 = 0u; i < dims.hidden_dim; i = i + 1u) {
        act_in[in_off + i] = hN[u * dims.hidden_dim + i];
    }
    for (var j: u32 = 0u; j < dims.action_dim; j = j + 1u) {
        act_in[in_off + dims.hidden_dim + j] = act_feat[a * dims.action_dim + j];
    }
}
"#;

/// WGSL Value Head Mean Pool Shader:
/// Gom hN theo node_offsets -> val_in (HIDDEN_DIM dims per env). Kích thước hidden truyền qua uniform.
const MEAN_POOL_VAL_SHADER: &str = r#"
struct PoolDims { batch: u32, hidden_dim: u32, _p2: u32, _p3: u32 };

@group(0) @binding(0) var<uniform>          dims:         PoolDims;
@group(0) @binding(1) var<storage, read>    hN:           array<f32>;
@group(0) @binding(2) var<storage, read>    node_offsets: array<u32>;
@group(0) @binding(3) var<storage, read_write> val_in:    array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let b = gid.x;
    if (b >= dims.batch) { return; }

    let start = node_offsets[b];
    let end   = node_offsets[b + 1u];
    let count = end - start;
    let inv   = select(1.0 / f32(count), 0.0, count == 0u);

    for (var d: u32 = 0u; d < dims.hidden_dim; d = d + 1u) {
        var sum: f32 = 0.0;
        for (var u: u32 = start; u < end; u = u + 1u) {
            sum = sum + hN[u * dims.hidden_dim + d];
        }
        val_in[b * dims.hidden_dim + d] = sum * inv;
    }
}
"#;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GemmDims { m: u32, k: u32, n: u32, relu: u32 }

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct AggCsrDims { n_nodes: u32, dim: u32, _p1: u32, _p2: u32 }

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CombineDims { n_nodes: u32, dim: u32, residual: u32, _p1: u32 }

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GatherDims { total_actions: u32, hidden_dim: u32, action_dim: u32, _p3: u32 }

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct PoolDims { batch: u32, hidden_dim: u32, _p2: u32, _p3: u32 }

const MAX_NODES: usize = 262_144;
const MAX_EDGES: usize = 1_572_864;
const MAX_ACTIONS: usize = 262_144;
const MAX_BATCH: usize = 4_096;
const ACT_IN_DIM: usize = HIDDEN_DIM + ACTION_FEAT_DIM; // 144

/// Kế hoạch thực thi GPU chưa hoàn tất chờ Readback (Async Non-blocking Handle)
pub struct PendingGpuResult {
    staging_act: Arc<wgpu::Buffer>,
    staging_val: Arc<wgpu::Buffer>,
    rx_act: crossbeam_channel::Receiver<Result<(), wgpu::BufferAsyncError>>,
    rx_val: crossbeam_channel::Receiver<Result<(), wgpu::BufferAsyncError>>,
    batch: usize,
    action_offsets: Vec<usize>,
    action_counts: Vec<usize>,
}

impl PendingGpuResult {
    pub fn wait(self, device: &wgpu::Device) -> Vec<(Vec<f32>, f32)> {
        device.poll(wgpu::Maintain::Wait);
        let _ = self.rx_act.recv();
        let _ = self.rx_val.recv();

        let mut results = Vec::with_capacity(self.batch);
        {
            let data_act = self.staging_act.slice(..).get_mapped_range();
            let all_logits: &[f32] = bytemuck::cast_slice(&data_act);

            let data_val = self.staging_val.slice(..).get_mapped_range();
            let all_vals: &[f32] = bytemuck::cast_slice(&data_val);

            for i in 0..self.batch {
                let a_start = self.action_offsets[i];
                let a_count = self.action_counts[i];
                let logits = all_logits[a_start..a_start + a_count].to_vec();
                let value = all_vals[i];
                results.push((logits, value));
            }
        }
        self.staging_act.unmap();
        self.staging_val.unmap();
        results
    }
}

/// Tập hợp toàn bộ Persistent VRAM Buffers & Pre-baked BindGroups cho 1 Slot (Group A hoặc Group B)
struct GpuSlot {
    h0: wgpu::Buffer,
    csr_offsets: wgpu::Buffer,
    csr_targets: wgpu::Buffer,
    act_node_u: wgpu::Buffer,
    act_feat: wgpu::Buffer,
    node_offsets: wgpu::Buffer,

    act_o: wgpu::Buffer,
    val_o: wgpu::Buffer,
    staging_act: Arc<wgpu::Buffer>,
    staging_val: Arc<wgpu::Buffer>,

    u_agg1: wgpu::Buffer,
    u_gemm_s1: wgpu::Buffer,
    u_gemm_n1: wgpu::Buffer,
    u_comb1: wgpu::Buffer,

    u_agg2: wgpu::Buffer,
    u_gemm_s2: wgpu::Buffer,
    u_gemm_n2: wgpu::Buffer,
    u_comb2: wgpu::Buffer,

    u_agg3: wgpu::Buffer,
    u_gemm_s3: wgpu::Buffer,
    u_gemm_n3: wgpu::Buffer,
    u_comb3: wgpu::Buffer,

    u_agg4: wgpu::Buffer,
    u_gemm_s4: wgpu::Buffer,
    u_gemm_n4: wgpu::Buffer,
    u_comb4: wgpu::Buffer,

    u_gather: wgpu::Buffer,
    u_pool: wgpu::Buffer,
    u_act1: wgpu::Buffer,
    u_val1: wgpu::Buffer,
    u_act2: wgpu::Buffer,
    u_val2: wgpu::Buffer,

    bg_agg1: wgpu::BindGroup,
    bg_gemm_s1: wgpu::BindGroup,
    bg_gemm_n1: wgpu::BindGroup,
    bg_comb1: wgpu::BindGroup,

    bg_agg2: wgpu::BindGroup,
    bg_gemm_s2: wgpu::BindGroup,
    bg_gemm_n2: wgpu::BindGroup,
    bg_comb2: wgpu::BindGroup,

    bg_agg3: wgpu::BindGroup,
    bg_gemm_s3: wgpu::BindGroup,
    bg_gemm_n3: wgpu::BindGroup,
    bg_comb3: wgpu::BindGroup,

    bg_agg4: wgpu::BindGroup,
    bg_gemm_s4: wgpu::BindGroup,
    bg_gemm_n4: wgpu::BindGroup,
    bg_comb4: wgpu::BindGroup,

    bg_gather: wgpu::BindGroup,
    bg_pool: wgpu::BindGroup,
    bg_act1: wgpu::BindGroup,
    bg_val1: wgpu::BindGroup,
    bg_act2: wgpu::BindGroup,
    bg_val2: wgpu::BindGroup,
}

/// Zero-Allocation Pre-allocated GPU Neural Network Engine (True Double-Buffering Multi-Stream)
pub struct GpuNNExecutor {
    pub device: Arc<wgpu::Device>,
    pub queue:  Arc<wgpu::Queue>,
    gemm_pipeline:     wgpu::ComputePipeline,
    agg_pipeline:      wgpu::ComputePipeline,
    combine_pipeline:  wgpu::ComputePipeline,
    gather_pipeline:   wgpu::ComputePipeline,
    pool_pipeline:     wgpu::ComputePipeline,

    slots: [GpuSlot; 2],

    // Persistent Weights (VRAM)
    w_self1: wgpu::Buffer, b_self1: wgpu::Buffer,
    w_neigh1: wgpu::Buffer, b_neigh1: wgpu::Buffer,
    w_self2: wgpu::Buffer, b_self2: wgpu::Buffer,
    w_neigh2: wgpu::Buffer, b_neigh2: wgpu::Buffer,
    w_self3: wgpu::Buffer, b_self3: wgpu::Buffer,
    w_neigh3: wgpu::Buffer, b_neigh3: wgpu::Buffer,
    w_self4: wgpu::Buffer, b_self4: wgpu::Buffer,
    w_neigh4: wgpu::Buffer, b_neigh4: wgpu::Buffer,
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

    fn create_storage(device: &wgpu::Device, size_bytes: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: size_bytes as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn create_uniform(device: &wgpu::Device, size_bytes: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: size_bytes as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn create_staging(device: &wgpu::Device, size_bytes: usize) -> Arc<wgpu::Buffer> {
        Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: size_bytes as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }))
    }

    fn make_weight_buf(device: &wgpu::Device, data: &[f32]) -> wgpu::Buffer {
        use wgpu::util::DeviceExt;
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        })
    }

    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, model: &HexGNNModel) -> Self {
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

        let gather_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Gather Act"), source: wgpu::ShaderSource::Wgsl(GATHER_ACT_SHADER.into()),
        });
        let gather_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Gather BGL"),
            entries: &[
                Self::bgl_uniform(0), Self::bgl_storage_ro(1),
                Self::bgl_storage_ro(2), Self::bgl_storage_ro(3),
                Self::bgl_storage_rw(4),
            ],
        });
        let gather_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None, bind_group_layouts: &[&gather_layout], push_constant_ranges: &[],
        });
        let gather_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Gather Pipeline"), layout: Some(&gather_pl),
            module: &gather_shader, entry_point: Some("main"), compilation_options: Default::default(), cache: None,
        });

        let pool_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Pool Val"), source: wgpu::ShaderSource::Wgsl(MEAN_POOL_VAL_SHADER.into()),
        });
        let pool_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Pool BGL"),
            entries: &[
                Self::bgl_uniform(0), Self::bgl_storage_ro(1),
                Self::bgl_storage_ro(2), Self::bgl_storage_rw(3),
            ],
        });
        let pool_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None, bind_group_layouts: &[&pool_layout], push_constant_ranges: &[],
        });
        let pool_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Pool Pipeline"), layout: Some(&pool_pl),
            module: &pool_shader, entry_point: Some("main"), compilation_options: Default::default(), cache: None,
        });

        macro_rules! wb { ($slice:expr) => { Self::make_weight_buf(&device, $slice) } }
        let w_self1  = wb!(&model.w_self1.weight);  let b_self1  = wb!(&model.w_self1.bias);
        let w_neigh1 = wb!(&model.w_neigh1.weight); let b_neigh1 = wb!(&model.w_neigh1.bias);
        let w_self2  = wb!(&model.w_self2.weight);  let b_self2  = wb!(&model.w_self2.bias);
        let w_neigh2 = wb!(&model.w_neigh2.weight); let b_neigh2 = wb!(&model.w_neigh2.bias);
        let w_self3  = wb!(&model.w_self3.weight);  let b_self3  = wb!(&model.w_self3.bias);
        let w_neigh3 = wb!(&model.w_neigh3.weight); let b_neigh3 = wb!(&model.w_neigh3.bias);
        let w_self4  = wb!(&model.w_self4.weight);  let b_self4  = wb!(&model.w_self4.bias);
        let w_neigh4 = wb!(&model.w_neigh4.weight); let b_neigh4 = wb!(&model.w_neigh4.bias);
        let w_act1   = wb!(&model.w_act1.weight);   let b_act1   = wb!(&model.w_act1.bias);
        let w_act2   = wb!(&model.w_act2.weight);   let b_act2   = wb!(&model.w_act2.bias);
        let w_val1   = wb!(&model.w_val1.weight);   let b_val1   = wb!(&model.w_val1.bias);
        let w_val2   = wb!(&model.w_val2.weight);   let b_val2   = wb!(&model.w_val2.bias);

        let make_slot = || {
            // h0: node features (NODE_FEAT_DIM). h1..h4: HIDDEN_DIM.
            let h0 = Self::create_storage(&device, MAX_NODES * NODE_FEAT_DIM * 4);
            let h1 = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);
            let h2 = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);
            let h3 = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);
            let h4 = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);

            let agg0 = Self::create_storage(&device, MAX_NODES * NODE_FEAT_DIM * 4);
            let agg1 = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);
            let agg2 = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);
            let agg3 = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);

            let ys = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);
            let yn = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);

            let csr_offsets = Self::create_storage(&device, (MAX_NODES + 1) * 4);
            let csr_targets = Self::create_storage(&device, MAX_EDGES * 4);
            let act_node_u  = Self::create_storage(&device, MAX_ACTIONS * 4);
            let act_feat    = Self::create_storage(&device, MAX_ACTIONS * ACTION_FEAT_DIM * 4);
            let node_offsets = Self::create_storage(&device, (MAX_BATCH + 1) * 4);

            let act_in = Self::create_storage(&device, MAX_ACTIONS * ACT_IN_DIM * 4);
            let val_in = Self::create_storage(&device, MAX_BATCH * HIDDEN_DIM * 4);
            let act_h  = Self::create_storage(&device, MAX_ACTIONS * HIDDEN_DIM * 4);
            let val_h  = Self::create_storage(&device, MAX_BATCH * HIDDEN_DIM * 4);
            let act_o  = Self::create_storage(&device, MAX_ACTIONS * 4);
            let val_o  = Self::create_storage(&device, MAX_BATCH * 4);

            let staging_act = Self::create_staging(&device, MAX_ACTIONS * 4);
            let staging_val = Self::create_staging(&device, MAX_BATCH * 4);

            let u_agg1 = Self::create_uniform(&device, 16);
            let u_gemm_s1 = Self::create_uniform(&device, 16);
            let u_gemm_n1 = Self::create_uniform(&device, 16);
            let u_comb1 = Self::create_uniform(&device, 16);

            let u_agg2 = Self::create_uniform(&device, 16);
            let u_gemm_s2 = Self::create_uniform(&device, 16);
            let u_gemm_n2 = Self::create_uniform(&device, 16);
            let u_comb2 = Self::create_uniform(&device, 16);

            let u_agg3 = Self::create_uniform(&device, 16);
            let u_gemm_s3 = Self::create_uniform(&device, 16);
            let u_gemm_n3 = Self::create_uniform(&device, 16);
            let u_comb3 = Self::create_uniform(&device, 16);

            let u_agg4 = Self::create_uniform(&device, 16);
            let u_gemm_s4 = Self::create_uniform(&device, 16);
            let u_gemm_n4 = Self::create_uniform(&device, 16);
            let u_comb4 = Self::create_uniform(&device, 16);

            let u_gather = Self::create_uniform(&device, 16);
            let u_pool = Self::create_uniform(&device, 16);
            let u_act1 = Self::create_uniform(&device, 16);
            let u_val1 = Self::create_uniform(&device, 16);
            let u_act2 = Self::create_uniform(&device, 16);
            let u_val2 = Self::create_uniform(&device, 16);

            macro_rules! bg_5 {
                ($layout:expr, $b0:expr, $b1:expr, $b2:expr, $b3:expr, $b4:expr) => {
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None, layout: $layout,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: $b0.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 1, resource: $b1.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 2, resource: $b2.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 3, resource: $b3.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 4, resource: $b4.as_entire_binding() },
                        ],
                    })
                }
            }

            macro_rules! bg_4 {
                ($layout:expr, $b0:expr, $b1:expr, $b2:expr, $b3:expr) => {
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None, layout: $layout,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: $b0.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 1, resource: $b1.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 2, resource: $b2.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 3, resource: $b3.as_entire_binding() },
                        ],
                    })
                }
            }

            let bg_agg1    = bg_5!(&agg_layout, &u_agg1, &h0, &csr_offsets, &csr_targets, &agg0);
            let bg_gemm_s1 = bg_5!(&gemm_layout, &u_gemm_s1, &h0, &w_self1, &b_self1, &ys);
            let bg_gemm_n1 = bg_5!(&gemm_layout, &u_gemm_n1, &agg0, &w_neigh1, &b_neigh1, &yn);
            let bg_comb1   = bg_5!(&combine_layout, &u_comb1, &ys, &yn, &h0, &h1);

            let bg_agg2    = bg_5!(&agg_layout, &u_agg2, &h1, &csr_offsets, &csr_targets, &agg1);
            let bg_gemm_s2 = bg_5!(&gemm_layout, &u_gemm_s2, &h1, &w_self2, &b_self2, &ys);
            let bg_gemm_n2 = bg_5!(&gemm_layout, &u_gemm_n2, &agg1, &w_neigh2, &b_neigh2, &yn);
            let bg_comb2   = bg_5!(&combine_layout, &u_comb2, &ys, &yn, &h1, &h2);

            let bg_agg3    = bg_5!(&agg_layout, &u_agg3, &h2, &csr_offsets, &csr_targets, &agg2);
            let bg_gemm_s3 = bg_5!(&gemm_layout, &u_gemm_s3, &h2, &w_self3, &b_self3, &ys);
            let bg_gemm_n3 = bg_5!(&gemm_layout, &u_gemm_n3, &agg2, &w_neigh3, &b_neigh3, &yn);
            let bg_comb3   = bg_5!(&combine_layout, &u_comb3, &ys, &yn, &h2, &h3);

            let bg_agg4    = bg_5!(&agg_layout, &u_agg4, &h3, &csr_offsets, &csr_targets, &agg3);
            let bg_gemm_s4 = bg_5!(&gemm_layout, &u_gemm_s4, &h3, &w_self4, &b_self4, &ys);
            let bg_gemm_n4 = bg_5!(&gemm_layout, &u_gemm_n4, &agg3, &w_neigh4, &b_neigh4, &yn);
            let bg_comb4   = bg_5!(&combine_layout, &u_comb4, &ys, &yn, &h3, &h4);

            let bg_gather  = bg_5!(&gather_layout, &u_gather, &h4, &act_node_u, &act_feat, &act_in);
            let bg_pool    = bg_4!(&pool_layout, &u_pool, &h4, &node_offsets, &val_in);

            let bg_act1    = bg_5!(&gemm_layout, &u_act1, &act_in, &w_act1, &b_act1, &act_h);
            let bg_val1    = bg_5!(&gemm_layout, &u_val1, &val_in, &w_val1, &b_val1, &val_h);
            let bg_act2    = bg_5!(&gemm_layout, &u_act2, &act_h,  &w_act2, &b_act2, &act_o);
            let bg_val2    = bg_5!(&gemm_layout, &u_val2, &val_h,  &w_val2, &b_val2, &val_o);

            GpuSlot {
                h0, csr_offsets, csr_targets, act_node_u, act_feat, node_offsets,
                act_o, val_o, staging_act, staging_val,
                u_agg1, u_gemm_s1, u_gemm_n1, u_comb1,
                u_agg2, u_gemm_s2, u_gemm_n2, u_comb2,
                u_agg3, u_gemm_s3, u_gemm_n3, u_comb3,
                u_agg4, u_gemm_s4, u_gemm_n4, u_comb4,
                u_gather, u_pool, u_act1, u_val1, u_act2, u_val2,
                bg_agg1, bg_gemm_s1, bg_gemm_n1, bg_comb1,
                bg_agg2, bg_gemm_s2, bg_gemm_n2, bg_comb2,
                bg_agg3, bg_gemm_s3, bg_gemm_n3, bg_comb3,
                bg_agg4, bg_gemm_s4, bg_gemm_n4, bg_comb4,
                bg_gather, bg_pool, bg_act1, bg_val1, bg_act2, bg_val2,
            }
        };

        let slots = [make_slot(), make_slot()];

        Self {
            device, queue,
            gemm_pipeline,
            agg_pipeline,
            combine_pipeline,
            gather_pipeline,
            pool_pipeline,
            slots,
            w_self1, b_self1, w_neigh1, b_neigh1,
            w_self2, b_self2, w_neigh2, b_neigh2,
            w_self3, b_self3, w_neigh3, b_neigh3,
            w_self4, b_self4, w_neigh4, b_neigh4,
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
        ww!(self.w_self4,  &model.w_self4.weight);  ww!(self.b_self4,  &model.w_self4.bias);
        ww!(self.w_neigh4, &model.w_neigh4.weight); ww!(self.b_neigh4, &model.w_neigh4.bias);
        ww!(self.w_act1,   &model.w_act1.weight);   ww!(self.b_act1,   &model.w_act1.bias);
        ww!(self.w_act2,   &model.w_act2.weight);   ww!(self.b_act2,   &model.w_act2.bias);
        ww!(self.w_val1,   &model.w_val1.weight);   ww!(self.b_val1,   &model.w_val1.bias);
        ww!(self.w_val2,   &model.w_val2.weight);   ww!(self.b_val2,   &model.w_val2.bias);
        self.queue.submit([]);
    }

    pub fn sync_weights(&self, model: &HexGNNModel) {
        self.update_weights_from_model(model);
    }

    /// Submit async GPU command encoder vào Slot chỉ định với ZERO ALLOCATIONS (Pre-baked Pipelines)
    pub fn forward_batch_gpu_async_slot(&self, slot_idx: usize, observations: &[&GraphObservation]) -> Option<PendingGpuResult> {
        let batch = observations.len();
        if batch == 0 { return None; }

        let slot = &self.slots[slot_idx % 2];

        let mut total_nodes = 0usize;
        let mut total_actions = 0usize;
        let mut node_offsets = Vec::with_capacity(batch + 1);
        let mut action_offsets = Vec::with_capacity(batch);
        let mut action_counts = Vec::with_capacity(batch);

        for obs in observations.iter() {
            node_offsets.push(total_nodes as u32);
            action_offsets.push(total_actions);
            let a_cnt = obs.valid_actions.len();
            action_counts.push(a_cnt);
            total_nodes += obs.node_features.len();
            total_actions += a_cnt;
        }
        node_offsets.push(total_nodes as u32);

        if total_nodes == 0 || total_actions == 0 || total_nodes > MAX_NODES || total_actions > MAX_ACTIONS {
            return None;
        }

        // Build mảng GPU song song bằng Rayon (Tối ưu 3C): phần tính nặng (dựng feature,
        // degree, tìm node index của action, ...) chạy song song trên từng obs độc lập,
        // rồi ghép (splice) vào mảng bằng offset đã biết trước.
        let mut h0: Vec<f32> = Vec::with_capacity(total_nodes * NODE_FEAT_DIM);
        let mut csr_offsets: Vec<u32> = Vec::with_capacity(total_nodes + 1);
        let mut csr_targets: Vec<u32> = Vec::with_capacity(total_nodes * 6);
        let mut act_node_u: Vec<u32> = Vec::with_capacity(total_actions);
        let mut act_feat: Vec<f32> = Vec::with_capacity(total_actions * ACTION_FEAT_DIM);
        csr_offsets.push(0u32);

        let per_obs: Vec<(usize, Vec<f32>, Vec<u32>, Vec<u32>, Vec<u32>, Vec<f32>)> = observations
            .par_iter()
            .enumerate()
            .map(|(i, obs)| {
                let n = obs.node_features.len();

                // h0: node features flattened
                let h0_seg: Vec<f32> = obs.node_features.iter().flatten().copied().collect();

                // degrees theo local node index
                let mut degrees = vec![0u32; n];
                for &(u, v) in &obs.edge_index {
                    if u < n && v < n {
                        degrees[u] += 1;
                    }
                }

                // csr row-pointers cục bộ (bắt đầu từ 0)
                let mut csr_off_seg = Vec::with_capacity(n + 1);
                let mut acc = 0u32;
                csr_off_seg.push(0u32);
                for d in &degrees {
                    acc += d;
                    csr_off_seg.push(acc);
                }

                // csr targets cục bộ (global node base được cộng sau khi splice)
                let mut csr_targets_seg: Vec<u32> = Vec::new();
                for &(u, v) in &obs.edge_index {
                    if u < n && v < n {
                        csr_targets_seg.push(v as u32);
                    }
                }

                // action node index (local) + action features
                let mut act_u_seg = Vec::with_capacity(obs.valid_actions.len());
                let mut act_feat_seg = Vec::with_capacity(obs.valid_actions.len() * ACTION_FEAT_DIM);
                for (a_idx, act) in obs.valid_actions.iter().enumerate() {
                    let pos = obs.node_positions.iter().position(|&p| p == (act.q, act.r)).unwrap_or(0);
                    let u = pos.min(n.saturating_sub(1)) as u32;
                    act_u_seg.push(u);
                    if a_idx < obs.action_features.len() {
                        act_feat_seg.extend_from_slice(&obs.action_features[a_idx]);
                    } else {
                        act_feat_seg.extend_from_slice(&[0.0f32; ACTION_FEAT_DIM]);
                    }
                }

                (i, h0_seg, csr_off_seg, csr_targets_seg, act_u_seg, act_feat_seg)
            })
            .collect();

        // Ghép theo thứ tự (index i) vào các mảng phẳng
        for (i, h0_seg, csr_off_seg, csr_targets_seg, act_u_seg, act_feat_seg) in per_obs {
            h0.extend_from_slice(&h0_seg);

            let base = *csr_offsets.last().unwrap();
            for &o in &csr_off_seg[1..] {
                csr_offsets.push(base + o);
            }

            let off = node_offsets[i];
            for t in csr_targets_seg {
                csr_targets.push(t + off);
            }

            act_node_u.extend_from_slice(&act_u_seg);
            act_feat.extend_from_slice(&act_feat_seg);
        }

        // 1. Zero-Allocation Fast Memcpy to Persistent GPU Buffers
        self.queue.write_buffer(&slot.h0, 0, bytemuck::cast_slice(&h0));
        self.queue.write_buffer(&slot.csr_offsets, 0, bytemuck::cast_slice(&csr_offsets));
        self.queue.write_buffer(&slot.csr_targets, 0, bytemuck::cast_slice(&csr_targets));
        self.queue.write_buffer(&slot.act_node_u, 0, bytemuck::cast_slice(&act_node_u));
        self.queue.write_buffer(&slot.act_feat, 0, bytemuck::cast_slice(&act_feat));
        self.queue.write_buffer(&slot.node_offsets, 0, bytemuck::cast_slice(&node_offsets));

        // 2. Uniform Dimensions
        let tn = total_nodes as u32;
        let ta = total_actions as u32;
        let b = batch as u32;
        let hd = HIDDEN_DIM as u32;

        self.queue.write_buffer(&slot.u_agg1, 0, bytemuck::bytes_of(&AggCsrDims { n_nodes: tn, dim: NODE_FEAT_DIM as u32, _p1: 0, _p2: 0 }));
        self.queue.write_buffer(&slot.u_gemm_s1, 0, bytemuck::bytes_of(&GemmDims { m: tn, k: NODE_FEAT_DIM as u32, n: hd, relu: 0 }));
        self.queue.write_buffer(&slot.u_gemm_n1, 0, bytemuck::bytes_of(&GemmDims { m: tn, k: NODE_FEAT_DIM as u32, n: hd, relu: 0 }));
        self.queue.write_buffer(&slot.u_comb1, 0, bytemuck::bytes_of(&CombineDims { n_nodes: tn, dim: hd, residual: 0, _p1: 0 }));

        self.queue.write_buffer(&slot.u_agg2, 0, bytemuck::bytes_of(&AggCsrDims { n_nodes: tn, dim: hd, _p1: 0, _p2: 0 }));
        self.queue.write_buffer(&slot.u_gemm_s2, 0, bytemuck::bytes_of(&GemmDims { m: tn, k: hd, n: hd, relu: 0 }));
        self.queue.write_buffer(&slot.u_gemm_n2, 0, bytemuck::bytes_of(&GemmDims { m: tn, k: hd, n: hd, relu: 0 }));
        self.queue.write_buffer(&slot.u_comb2, 0, bytemuck::bytes_of(&CombineDims { n_nodes: tn, dim: hd, residual: 1, _p1: 0 }));

        self.queue.write_buffer(&slot.u_agg3, 0, bytemuck::bytes_of(&AggCsrDims { n_nodes: tn, dim: hd, _p1: 0, _p2: 0 }));
        self.queue.write_buffer(&slot.u_gemm_s3, 0, bytemuck::bytes_of(&GemmDims { m: tn, k: hd, n: hd, relu: 0 }));
        self.queue.write_buffer(&slot.u_gemm_n3, 0, bytemuck::bytes_of(&GemmDims { m: tn, k: hd, n: hd, relu: 0 }));
        self.queue.write_buffer(&slot.u_comb3, 0, bytemuck::bytes_of(&CombineDims { n_nodes: tn, dim: hd, residual: 1, _p1: 0 }));

        self.queue.write_buffer(&slot.u_agg4, 0, bytemuck::bytes_of(&AggCsrDims { n_nodes: tn, dim: hd, _p1: 0, _p2: 0 }));
        self.queue.write_buffer(&slot.u_gemm_s4, 0, bytemuck::bytes_of(&GemmDims { m: tn, k: hd, n: hd, relu: 0 }));
        self.queue.write_buffer(&slot.u_gemm_n4, 0, bytemuck::bytes_of(&GemmDims { m: tn, k: hd, n: hd, relu: 0 }));
        self.queue.write_buffer(&slot.u_comb4, 0, bytemuck::bytes_of(&CombineDims { n_nodes: tn, dim: hd, residual: 1, _p1: 0 }));

        self.queue.write_buffer(&slot.u_gather, 0, bytemuck::bytes_of(&GatherDims { total_actions: ta, hidden_dim: hd, action_dim: ACTION_FEAT_DIM as u32, _p3: 0 }));
        self.queue.write_buffer(&slot.u_pool, 0, bytemuck::bytes_of(&PoolDims { batch: b, hidden_dim: hd, _p2: 0, _p3: 0 }));

        self.queue.write_buffer(&slot.u_act1, 0, bytemuck::bytes_of(&GemmDims { m: ta, k: ACT_IN_DIM as u32, n: hd, relu: 1 }));
        self.queue.write_buffer(&slot.u_val1, 0, bytemuck::bytes_of(&GemmDims { m: b,  k: hd, n: hd, relu: 1 }));
        self.queue.write_buffer(&slot.u_act2, 0, bytemuck::bytes_of(&GemmDims { m: ta, k: hd, n: 1,  relu: 0 }));
        self.queue.write_buffer(&slot.u_val2, 0, bytemuck::bytes_of(&GemmDims { m: b,  k: hd, n: 1,  relu: 0 }));

        let wg_d = || (hd + 15) / 16;

        // 3. One Single Command Encoder and Compute Pass Execution
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());

            // Layer 1
            pass.set_pipeline(&self.agg_pipeline);
            pass.set_bind_group(0, &slot.bg_agg1, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, ((NODE_FEAT_DIM as u32) + 15) / 16, 1);

            pass.set_pipeline(&self.gemm_pipeline);
            pass.set_bind_group(0, &slot.bg_gemm_s1, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, wg_d(), 1);

            pass.set_bind_group(0, &slot.bg_gemm_n1, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, wg_d(), 1);

            pass.set_pipeline(&self.combine_pipeline);
            pass.set_bind_group(0, &slot.bg_comb1, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, wg_d(), 1);

            // Layer 2
            pass.set_pipeline(&self.agg_pipeline);
            pass.set_bind_group(0, &slot.bg_agg2, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, wg_d(), 1);

            pass.set_pipeline(&self.gemm_pipeline);
            pass.set_bind_group(0, &slot.bg_gemm_s2, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, wg_d(), 1);

            pass.set_bind_group(0, &slot.bg_gemm_n2, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, wg_d(), 1);

            pass.set_pipeline(&self.combine_pipeline);
            pass.set_bind_group(0, &slot.bg_comb2, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, wg_d(), 1);

            // Layer 3
            pass.set_pipeline(&self.agg_pipeline);
            pass.set_bind_group(0, &slot.bg_agg3, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, wg_d(), 1);

            pass.set_pipeline(&self.gemm_pipeline);
            pass.set_bind_group(0, &slot.bg_gemm_s3, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, wg_d(), 1);

            pass.set_bind_group(0, &slot.bg_gemm_n3, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, wg_d(), 1);

            pass.set_pipeline(&self.combine_pipeline);
            pass.set_bind_group(0, &slot.bg_comb3, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, wg_d(), 1);

            // Layer 4
            pass.set_pipeline(&self.agg_pipeline);
            pass.set_bind_group(0, &slot.bg_agg4, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, wg_d(), 1);

            pass.set_pipeline(&self.gemm_pipeline);
            pass.set_bind_group(0, &slot.bg_gemm_s4, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, wg_d(), 1);

            pass.set_bind_group(0, &slot.bg_gemm_n4, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, wg_d(), 1);

            pass.set_pipeline(&self.combine_pipeline);
            pass.set_bind_group(0, &slot.bg_comb4, &[]);
            pass.dispatch_workgroups((tn + 15) / 16, wg_d(), 1);

            // Gather & Pool
            pass.set_pipeline(&self.gather_pipeline);
            pass.set_bind_group(0, &slot.bg_gather, &[]);
            pass.dispatch_workgroups((ta + 63) / 64, 1, 1);

            pass.set_pipeline(&self.pool_pipeline);
            pass.set_bind_group(0, &slot.bg_pool, &[]);
            pass.dispatch_workgroups((b + 63) / 64, 1, 1);

            // Head MLPs
            pass.set_pipeline(&self.gemm_pipeline);
            pass.set_bind_group(0, &slot.bg_act1, &[]);
            pass.dispatch_workgroups((ta + 15) / 16, wg_d(), 1);

            pass.set_bind_group(0, &slot.bg_val1, &[]);
            pass.dispatch_workgroups((b + 15) / 16, wg_d(), 1);

            pass.set_bind_group(0, &slot.bg_act2, &[]);
            pass.dispatch_workgroups((ta + 15) / 16, 1, 1);

            pass.set_bind_group(0, &slot.bg_val2, &[]);
            pass.dispatch_workgroups((b + 15) / 16, 1, 1);
        }

        enc.copy_buffer_to_buffer(&slot.act_o, 0, &slot.staging_act, 0, (total_actions * 4) as u64);
        enc.copy_buffer_to_buffer(&slot.val_o, 0, &slot.staging_val, 0, (batch * 4) as u64);
        self.queue.submit(Some(enc.finish()));

        let slice_act = slot.staging_act.slice(..);
        let (tx_act, rx_act) = crossbeam_channel::bounded(1);
        slice_act.map_async(wgpu::MapMode::Read, move |r| { let _ = tx_act.send(r); });

        let slice_val = slot.staging_val.slice(..);
        let (tx_val, rx_val) = crossbeam_channel::bounded(1);
        slice_val.map_async(wgpu::MapMode::Read, move |r| { let _ = tx_val.send(r); });

        Some(PendingGpuResult {
            staging_act: Arc::clone(&slot.staging_act),
            staging_val: Arc::clone(&slot.staging_val),
            rx_act,
            rx_val,
            batch,
            action_offsets,
            action_counts,
        })
    }

    /// Forward Pass Synchronous (Bọc lấy async handle và chờ kết quả ngay lập tức)
    pub fn forward_batch_gpu(&self, observations: &[&GraphObservation]) -> Vec<(Vec<f32>, f32)> {
        if let Some(pending) = self.forward_batch_gpu_async_slot(0, observations) {
            pending.wait(&self.device)
        } else {
            observations.iter().map(|o| (vec![0.0f32; o.valid_actions.len()], 0.0f32)).collect()
        }
    }
}

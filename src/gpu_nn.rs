use std::sync::Arc;
use rayon::prelude::*;
use crate::nn::{HexGNNModel, HIDDEN_DIM, ACTION_FEAT_DIM, NODE_FEAT_DIM};
use crate::env::GraphObservation;

/// WGSL CSR Aggregation Shader: h_out[u, d] = mean_{v in N(u)} h_in[v, d]
const AGG_CSR_SHADER: &str = r#"
struct AllUniforms {
    total_nodes: u32,
    total_actions: u32,
    batch: u32,
    hidden_dim: u32,
    node_feat_dim: u32,
    action_dim: u32,
    _pad0: u32,
    _pad1: u32,
};

@group(0) @binding(0) var<uniform>          dims:    AllUniforms;
@group(0) @binding(1) var<storage, read>    h_in:    array<f32>;
@group(0) @binding(2) var<storage, read>    offsets: array<u32>;
@group(0) @binding(3) var<storage, read>    targets: array<u32>;
@group(0) @binding(4) var<storage, read_write> h_out:  array<f32>;

@compute @workgroup_size(16, 16)
fn main_l1(@builtin(global_invocation_id) gid: vec3<u32>) {
    let node = gid.x;
    let feat = gid.y;
    if (node >= dims.total_nodes || feat >= dims.node_feat_dim) { return; }

    let start = offsets[node];
    let end   = offsets[node + 1u];
    let count = end - start;

    var sum: f32 = 0.0;
    for (var i: u32 = start; i < end; i = i + 1u) {
        let v = targets[i];
        sum = sum + h_in[v * dims.node_feat_dim + feat];
    }

    let inv = select(1.0 / f32(count), 0.0, count == 0u);
    h_out[node * dims.node_feat_dim + feat] = sum * inv;
}

@compute @workgroup_size(16, 16)
fn main_ln(@builtin(global_invocation_id) gid: vec3<u32>) {
    let node = gid.x;
    let feat = gid.y;
    if (node >= dims.total_nodes || feat >= dims.hidden_dim) { return; }

    let start = offsets[node];
    let end   = offsets[node + 1u];
    let count = end - start;

    var sum: f32 = 0.0;
    for (var i: u32 = start; i < end; i = i + 1u) {
        let v = targets[i];
        sum = sum + h_in[v * dims.hidden_dim + feat];
    }

    let inv = select(1.0 / f32(count), 0.0, count == 0u);
    h_out[node * dims.hidden_dim + feat] = sum * inv;
}
"#;

/// WGSL Fused GNN Layer Shader:
/// Tính đồng thời:
///   Y_self  = X · W_self^T + B_self
///   Y_neigh = Neigh · W_neigh^T + B_neigh
///   H_out   = ReLU(Y_self + Y_neigh) [+ H_prev if residual]
const FUSED_GNN_LAYER_SHADER: &str = r#"
struct AllUniforms {
    total_nodes: u32,
    total_actions: u32,
    batch: u32,
    hidden_dim: u32,
    node_feat_dim: u32,
    action_dim: u32,
    _pad0: u32,
    _pad1: u32,
};

@group(0) @binding(0) var<uniform>            dims:    AllUniforms;
@group(0) @binding(1) var<storage, read>      x:       array<f32>;
@group(0) @binding(2) var<storage, read>      neigh:   array<f32>;
@group(0) @binding(3) var<storage, read>      w_self:  array<f32>;
@group(0) @binding(4) var<storage, read>      b_self:  array<f32>;
@group(0) @binding(5) var<storage, read>      w_neigh: array<f32>;
@group(0) @binding(6) var<storage, read>      b_neigh: array<f32>;
@group(0) @binding(7) var<storage, read>      h_prev:  array<f32>;
@group(0) @binding(8) var<storage, read_write> h_out:   array<f32>;

var<workgroup> tile_x:  array<array<f32, 16>, 16>;
var<workgroup> tile_n:  array<array<f32, 16>, 16>;
var<workgroup> tile_ws: array<array<f32, 16>, 16>;
var<workgroup> tile_wn: array<array<f32, 16>, 16>;

@compute @workgroup_size(16, 16)
fn main_l1(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>
) {
    let row = gid.x;
    let col = gid.y;
    let lr = lid.x;
    let lc = lid.y;

    var acc_s: f32 = 0.0;
    var acc_n: f32 = 0.0;
    let in_k = dims.node_feat_dim;
    let num_tiles = (in_k + 15u) / 16u;

    for (var t: u32 = 0u; t < num_tiles; t = t + 1u) {
        let x_col = t * 16u + lc;
        if (row < dims.total_nodes && x_col < in_k) {
            tile_x[lr][lc] = x[row * in_k + x_col];
            tile_n[lr][lc] = neigh[row * in_k + x_col];
        } else {
            tile_x[lr][lc] = 0.0;
            tile_n[lr][lc] = 0.0;
        }

        let w_k = t * 16u + lr;
        if (col < dims.hidden_dim && w_k < in_k) {
            tile_ws[lr][lc] = w_self[col * in_k + w_k];
            tile_wn[lr][lc] = w_neigh[col * in_k + w_k];
        } else {
            tile_ws[lr][lc] = 0.0;
            tile_wn[lr][lc] = 0.0;
        }

        workgroupBarrier();

        for (var i: u32 = 0u; i < 16u; i = i + 1u) {
            acc_s = acc_s + tile_x[lr][i] * tile_ws[i][lc];
            acc_n = acc_n + tile_n[lr][i] * tile_wn[i][lc];
        }

        workgroupBarrier();
    }

    if (row < dims.total_nodes && col < dims.hidden_dim) {
        let total = (acc_s + b_self[col]) + (acc_n + b_neigh[col]);
        h_out[row * dims.hidden_dim + col] = max(total, 0.0);
    }
}

@compute @workgroup_size(16, 16)
fn main_ln(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>
) {
    let row = gid.x;
    let col = gid.y;
    let lr = lid.x;
    let lc = lid.y;

    var acc_s: f32 = 0.0;
    var acc_n: f32 = 0.0;
    let in_k = dims.hidden_dim;
    let num_tiles = (in_k + 15u) / 16u;

    for (var t: u32 = 0u; t < num_tiles; t = t + 1u) {
        let x_col = t * 16u + lc;
        if (row < dims.total_nodes && x_col < in_k) {
            tile_x[lr][lc] = x[row * in_k + x_col];
            tile_n[lr][lc] = neigh[row * in_k + x_col];
        } else {
            tile_x[lr][lc] = 0.0;
            tile_n[lr][lc] = 0.0;
        }

        let w_k = t * 16u + lr;
        if (col < dims.hidden_dim && w_k < in_k) {
            tile_ws[lr][lc] = w_self[col * in_k + w_k];
            tile_wn[lr][lc] = w_neigh[col * in_k + w_k];
        } else {
            tile_ws[lr][lc] = 0.0;
            tile_wn[lr][lc] = 0.0;
        }

        workgroupBarrier();

        for (var i: u32 = 0u; i < 16u; i = i + 1u) {
            acc_s = acc_s + tile_x[lr][i] * tile_ws[i][lc];
            acc_n = acc_n + tile_n[lr][i] * tile_wn[i][lc];
        }

        workgroupBarrier();
    }

    if (row < dims.total_nodes && col < dims.hidden_dim) {
        let total = (acc_s + b_self[col]) + (acc_n + b_neigh[col]);
        let relu = max(total, 0.0);
        h_out[row * dims.hidden_dim + col] = relu + h_prev[row * dims.hidden_dim + col];
    }
}
"#;

/// WGSL Action Head Gather Shader
const GATHER_ACT_SHADER: &str = r#"
struct AllUniforms {
    total_nodes: u32,
    total_actions: u32,
    batch: u32,
    hidden_dim: u32,
    node_feat_dim: u32,
    action_dim: u32,
    _pad0: u32,
    _pad1: u32,
};

@group(0) @binding(0) var<uniform>          dims:        AllUniforms;
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

/// WGSL Value Head Mean Pool Shader
const MEAN_POOL_VAL_SHADER: &str = r#"
struct AllUniforms {
    total_nodes: u32,
    total_actions: u32,
    batch: u32,
    hidden_dim: u32,
    node_feat_dim: u32,
    action_dim: u32,
    _pad0: u32,
    _pad1: u32,
};

@group(0) @binding(0) var<uniform>          dims:         AllUniforms;
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

/// WGSL Head MLPs GEMM Shader
const HEADS_SHADER: &str = r#"
struct AllUniforms {
    total_nodes: u32,
    total_actions: u32,
    batch: u32,
    hidden_dim: u32,
    node_feat_dim: u32,
    action_dim: u32,
    _pad0: u32,
    _pad1: u32,
};

@group(0) @binding(0) var<uniform>            dims: AllUniforms;
@group(0) @binding(1) var<storage, read>      x:    array<f32>;
@group(0) @binding(2) var<storage, read>      w:    array<f32>;
@group(0) @binding(3) var<storage, read>      b:    array<f32>;
@group(0) @binding(4) var<storage, read_write> y:   array<f32>;

var<workgroup> tile_x: array<array<f32, 16>, 16>;
var<workgroup> tile_w: array<array<f32, 16>, 16>;

@compute @workgroup_size(16, 16)
fn main_act1(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {
    let row = gid.x;
    let col = gid.y;
    let lr = lid.x;
    let lc = lid.y;
    let m = dims.total_actions;
    let k = dims.hidden_dim + dims.action_dim;
    let n = dims.hidden_dim;

    var acc: f32 = 0.0;
    let num_tiles = (k + 15u) / 16u;
    for (var t: u32 = 0u; t < num_tiles; t = t + 1u) {
        let x_col = t * 16u + lc;
        if (row < m && x_col < k) { tile_x[lr][lc] = x[row * k + x_col]; } else { tile_x[lr][lc] = 0.0; }
        let w_k = t * 16u + lr;
        if (col < n && w_k < k) { tile_w[lr][lc] = w[col * k + w_k]; } else { tile_w[lr][lc] = 0.0; }
        workgroupBarrier();
        for (var i: u32 = 0u; i < 16u; i = i + 1u) { acc = acc + tile_x[lr][i] * tile_w[i][lc]; }
        workgroupBarrier();
    }
    if (row < m && col < n) {
        y[row * n + col] = max(acc + b[col], 0.0);
    }
}

@compute @workgroup_size(16, 16)
fn main_act2(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {
    let row = gid.x;
    let col = gid.y;
    let lr = lid.x;
    let lc = lid.y;
    let m = dims.total_actions;
    let k = dims.hidden_dim;
    let n = 1u;

    var acc: f32 = 0.0;
    let num_tiles = (k + 15u) / 16u;
    for (var t: u32 = 0u; t < num_tiles; t = t + 1u) {
        let x_col = t * 16u + lc;
        if (row < m && x_col < k) { tile_x[lr][lc] = x[row * k + x_col]; } else { tile_x[lr][lc] = 0.0; }
        let w_k = t * 16u + lr;
        if (col < n && w_k < k) { tile_w[lr][lc] = w[col * k + w_k]; } else { tile_w[lr][lc] = 0.0; }
        workgroupBarrier();
        for (var i: u32 = 0u; i < 16u; i = i + 1u) { acc = acc + tile_x[lr][i] * tile_w[i][lc]; }
        workgroupBarrier();
    }
    if (row < m && col < n) {
        y[row * n + col] = acc + b[col];
    }
}

@compute @workgroup_size(16, 16)
fn main_val1(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {
    let row = gid.x;
    let col = gid.y;
    let lr = lid.x;
    let lc = lid.y;
    let m = dims.batch;
    let k = dims.hidden_dim;
    let n = dims.hidden_dim;

    var acc: f32 = 0.0;
    let num_tiles = (k + 15u) / 16u;
    for (var t: u32 = 0u; t < num_tiles; t = t + 1u) {
        let x_col = t * 16u + lc;
        if (row < m && x_col < k) { tile_x[lr][lc] = x[row * k + x_col]; } else { tile_x[lr][lc] = 0.0; }
        let w_k = t * 16u + lr;
        if (col < n && w_k < k) { tile_w[lr][lc] = w[col * k + w_k]; } else { tile_w[lr][lc] = 0.0; }
        workgroupBarrier();
        for (var i: u32 = 0u; i < 16u; i = i + 1u) { acc = acc + tile_x[lr][i] * tile_w[i][lc]; }
        workgroupBarrier();
    }
    if (row < m && col < n) {
        y[row * n + col] = max(acc + b[col], 0.0);
    }
}

@compute @workgroup_size(16, 16)
fn main_val2(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {
    let row = gid.x;
    let col = gid.y;
    let lr = lid.x;
    let lc = lid.y;
    let m = dims.batch;
    let k = dims.hidden_dim;
    let n = 1u;

    var acc: f32 = 0.0;
    let num_tiles = (k + 15u) / 16u;
    for (var t: u32 = 0u; t < num_tiles; t = t + 1u) {
        let x_col = t * 16u + lc;
        if (row < m && x_col < k) { tile_x[lr][lc] = x[row * k + x_col]; } else { tile_x[lr][lc] = 0.0; }
        let w_k = t * 16u + lr;
        if (col < n && w_k < k) { tile_w[lr][lc] = w[col * k + w_k]; } else { tile_w[lr][lc] = 0.0; }
        workgroupBarrier();
        for (var i: u32 = 0u; i < 16u; i = i + 1u) { acc = acc + tile_x[lr][i] * tile_w[i][lc]; }
        workgroupBarrier();
    }
    if (row < m && col < n) {
        y[row * n + col] = acc + b[col];
    }
}
"#;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AllUniforms {
    pub total_nodes: u32,
    pub total_actions: u32,
    pub batch: u32,
    pub hidden_dim: u32,
    pub node_feat_dim: u32,
    pub action_dim: u32,
    pub _pad0: u32,
    pub _pad1: u32,
}

const MAX_NODES: usize = 131_072;
const MAX_EDGES: usize = 1_048_576;
const MAX_ACTIONS: usize = 131_072;
const MAX_BATCH: usize = 2_048;

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

    uniforms: wgpu::Buffer,

    bg_agg1: wgpu::BindGroup,
    bg_fused1: wgpu::BindGroup,

    bg_agg2: wgpu::BindGroup,
    bg_fused2: wgpu::BindGroup,

    bg_agg3: wgpu::BindGroup,
    bg_fused3: wgpu::BindGroup,

    bg_agg4: wgpu::BindGroup,
    bg_fused4: wgpu::BindGroup,

    bg_gather: wgpu::BindGroup,
    bg_pool: wgpu::BindGroup,
    bg_act1: wgpu::BindGroup,
    bg_val1: wgpu::BindGroup,
    bg_act2: wgpu::BindGroup,
    bg_val2: wgpu::BindGroup,
}

pub struct GpuNNExecutor {
    pub device: Arc<wgpu::Device>,
    pub queue:  Arc<wgpu::Queue>,
    agg_l1_pipeline:   wgpu::ComputePipeline,
    agg_ln_pipeline:   wgpu::ComputePipeline,
    fused_l1_pipeline: wgpu::ComputePipeline,
    fused_ln_pipeline: wgpu::ComputePipeline,
    gather_pipeline:   wgpu::ComputePipeline,
    pool_pipeline:     wgpu::ComputePipeline,
    act1_pipeline:     wgpu::ComputePipeline,
    act2_pipeline:     wgpu::ComputePipeline,
    val1_pipeline:     wgpu::ComputePipeline,
    val2_pipeline:     wgpu::ComputePipeline,

    slots: [GpuSlot; 2],

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
        let agg_l1_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Agg L1 Pipeline"), layout: Some(&agg_pl),
            module: &agg_shader, entry_point: Some("main_l1"), compilation_options: Default::default(), cache: None,
        });
        let agg_ln_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Agg LN Pipeline"), layout: Some(&agg_pl),
            module: &agg_shader, entry_point: Some("main_ln"), compilation_options: Default::default(), cache: None,
        });

        let fused_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Fused GNN Layer"), source: wgpu::ShaderSource::Wgsl(FUSED_GNN_LAYER_SHADER.into()),
        });
        let fused_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Fused GNN Layer BGL"),
            entries: &[
                Self::bgl_uniform(0),
                Self::bgl_storage_ro(1), Self::bgl_storage_ro(2),
                Self::bgl_storage_ro(3), Self::bgl_storage_ro(4),
                Self::bgl_storage_ro(5), Self::bgl_storage_ro(6),
                Self::bgl_storage_ro(7), Self::bgl_storage_rw(8),
            ],
        });
        let fused_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None, bind_group_layouts: &[&fused_layout], push_constant_ranges: &[],
        });
        let fused_l1_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Fused L1 Pipeline"), layout: Some(&fused_pl),
            module: &fused_shader, entry_point: Some("main_l1"), compilation_options: Default::default(), cache: None,
        });
        let fused_ln_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Fused LN Pipeline"), layout: Some(&fused_pl),
            module: &fused_shader, entry_point: Some("main_ln"), compilation_options: Default::default(), cache: None,
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

        let heads_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Heads Shader"), source: wgpu::ShaderSource::Wgsl(HEADS_SHADER.into()),
        });
        let heads_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Heads BGL"),
            entries: &[
                Self::bgl_uniform(0), Self::bgl_storage_ro(1),
                Self::bgl_storage_ro(2), Self::bgl_storage_ro(3),
                Self::bgl_storage_rw(4),
            ],
        });
        let heads_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None, bind_group_layouts: &[&heads_layout], push_constant_ranges: &[],
        });
        let act1_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Act1 Pipeline"), layout: Some(&heads_pl),
            module: &heads_shader, entry_point: Some("main_act1"), compilation_options: Default::default(), cache: None,
        });
        let act2_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Act2 Pipeline"), layout: Some(&heads_pl),
            module: &heads_shader, entry_point: Some("main_act2"), compilation_options: Default::default(), cache: None,
        });
        let val1_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Val1 Pipeline"), layout: Some(&heads_pl),
            module: &heads_shader, entry_point: Some("main_val1"), compilation_options: Default::default(), cache: None,
        });
        let val2_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Val2 Pipeline"), layout: Some(&heads_pl),
            module: &heads_shader, entry_point: Some("main_val2"), compilation_options: Default::default(), cache: None,
        });

        macro_rules! wb { ($slice:expr) => { Self::make_weight_buf(&device, $slice) } }
        let w_self1  = wb!(&model.layers[0].w_self.weight);  let b_self1  = wb!(&model.layers[0].w_self.bias);
        let w_neigh1 = wb!(&model.layers[0].w_neigh.weight); let b_neigh1 = wb!(&model.layers[0].w_neigh.bias);
        let w_self2  = wb!(&model.layers[1].w_self.weight);  let b_self2  = wb!(&model.layers[1].w_self.bias);
        let w_neigh2 = wb!(&model.layers[1].w_neigh.weight); let b_neigh2 = wb!(&model.layers[1].w_neigh.bias);
        let w_self3  = wb!(&model.layers[2].w_self.weight);  let b_self3  = wb!(&model.layers[2].w_self.bias);
        let w_neigh3 = wb!(&model.layers[2].w_neigh.weight); let b_neigh3 = wb!(&model.layers[2].w_neigh.bias);
        let w_self4  = wb!(&model.layers[3].w_self.weight);  let b_self4  = wb!(&model.layers[3].w_self.bias);
        let w_neigh4 = wb!(&model.layers[3].w_neigh.weight); let b_neigh4 = wb!(&model.layers[3].w_neigh.bias);
        let w_act1   = wb!(&model.w_act1.weight);   let b_act1   = wb!(&model.w_act1.bias);
        let w_act2   = wb!(&model.w_act2.weight);   let b_act2   = wb!(&model.w_act2.bias);
        let w_val1   = wb!(&model.w_val1.weight);   let b_val1   = wb!(&model.w_val1.bias);
        let w_val2   = wb!(&model.w_val2.weight);   let b_val2   = wb!(&model.w_val2.bias);

        let make_slot = || {
            let h0 = Self::create_storage(&device, MAX_NODES * NODE_FEAT_DIM * 4);
            let h1 = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);
            let h2 = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);
            let h3 = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);
            let h4 = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);

            let agg0 = Self::create_storage(&device, MAX_NODES * NODE_FEAT_DIM * 4);
            let agg1 = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);
            let agg2 = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);
            let agg3 = Self::create_storage(&device, MAX_NODES * HIDDEN_DIM * 4);

            let csr_offsets = Self::create_storage(&device, (MAX_NODES + 1) * 4);
            let csr_targets = Self::create_storage(&device, MAX_EDGES * 4);
            let act_node_u  = Self::create_storage(&device, MAX_ACTIONS * 4);
            let act_feat    = Self::create_storage(&device, MAX_ACTIONS * ACTION_FEAT_DIM * 4);
            let node_offsets = Self::create_storage(&device, (MAX_BATCH + 1) * 4);

            let act_in = Self::create_storage(&device, MAX_ACTIONS * (HIDDEN_DIM + ACTION_FEAT_DIM) * 4);
            let val_in = Self::create_storage(&device, MAX_BATCH * HIDDEN_DIM * 4);
            let act_h  = Self::create_storage(&device, MAX_ACTIONS * HIDDEN_DIM * 4);
            let val_h  = Self::create_storage(&device, MAX_BATCH * HIDDEN_DIM * 4);
            let act_o  = Self::create_storage(&device, MAX_ACTIONS * 4);
            let val_o  = Self::create_storage(&device, MAX_BATCH * 4);

            let staging_act = Self::create_staging(&device, MAX_ACTIONS * 4);
            let staging_val = Self::create_staging(&device, MAX_BATCH * 4);

            let uniforms = Self::create_uniform(&device, std::mem::size_of::<AllUniforms>());

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

            macro_rules! bg_9 {
                ($layout:expr, $b0:expr, $b1:expr, $b2:expr, $b3:expr, $b4:expr, $b5:expr, $b6:expr, $b7:expr, $b8:expr) => {
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None, layout: $layout,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: $b0.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 1, resource: $b1.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 2, resource: $b2.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 3, resource: $b3.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 4, resource: $b4.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 5, resource: $b5.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 6, resource: $b6.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 7, resource: $b7.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 8, resource: $b8.as_entire_binding() },
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

            let bg_agg1   = bg_5!(&agg_layout, &uniforms, &h0, &csr_offsets, &csr_targets, &agg0);
            let bg_fused1 = bg_9!(&fused_layout, &uniforms, &h0, &agg0, &w_self1, &b_self1, &w_neigh1, &b_neigh1, &h0, &h1);

            let bg_agg2   = bg_5!(&agg_layout, &uniforms, &h1, &csr_offsets, &csr_targets, &agg1);
            let bg_fused2 = bg_9!(&fused_layout, &uniforms, &h1, &agg1, &w_self2, &b_self2, &w_neigh2, &b_neigh2, &h1, &h2);

            let bg_agg3   = bg_5!(&agg_layout, &uniforms, &h2, &csr_offsets, &csr_targets, &agg2);
            let bg_fused3 = bg_9!(&fused_layout, &uniforms, &h2, &agg2, &w_self3, &b_self3, &w_neigh3, &b_neigh3, &h2, &h3);

            let bg_agg4   = bg_5!(&agg_layout, &uniforms, &h3, &csr_offsets, &csr_targets, &agg3);
            let bg_fused4 = bg_9!(&fused_layout, &uniforms, &h3, &agg3, &w_self4, &b_self4, &w_neigh4, &b_neigh4, &h3, &h4);

            let bg_gather = bg_5!(&gather_layout, &uniforms, &h4, &act_node_u, &act_feat, &act_in);
            let bg_pool   = bg_4!(&pool_layout, &uniforms, &h4, &node_offsets, &val_in);

            let bg_act1   = bg_5!(&heads_layout, &uniforms, &act_in, &w_act1, &b_act1, &act_h);
            let bg_val1   = bg_5!(&heads_layout, &uniforms, &val_in, &w_val1, &b_val1, &val_h);
            let bg_act2   = bg_5!(&heads_layout, &uniforms, &act_h,  &w_act2, &b_act2, &act_o);
            let bg_val2   = bg_5!(&heads_layout, &uniforms, &val_h,  &w_val2, &b_val2, &val_o);

            GpuSlot {
                h0, csr_offsets, csr_targets, act_node_u, act_feat, node_offsets,
                act_o, val_o, staging_act, staging_val, uniforms,
                bg_agg1, bg_fused1,
                bg_agg2, bg_fused2,
                bg_agg3, bg_fused3,
                bg_agg4, bg_fused4,
                bg_gather, bg_pool, bg_act1, bg_val1, bg_act2, bg_val2,
            }
        };

        let slots = [make_slot(), make_slot()];

        Self {
            device, queue,
            agg_l1_pipeline, agg_ln_pipeline,
            fused_l1_pipeline, fused_ln_pipeline,
            gather_pipeline, pool_pipeline,
            act1_pipeline, act2_pipeline,
            val1_pipeline, val2_pipeline,
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
        ww!(self.w_self1,  &model.layers[0].w_self.weight);  ww!(self.b_self1,  &model.layers[0].w_self.bias);
        ww!(self.w_neigh1, &model.layers[0].w_neigh.weight); ww!(self.b_neigh1, &model.layers[0].w_neigh.bias);
        ww!(self.w_self2,  &model.layers[1].w_self.weight);  ww!(self.b_self2,  &model.layers[1].w_self.bias);
        ww!(self.w_neigh2, &model.layers[1].w_neigh.weight); ww!(self.b_neigh2, &model.layers[1].w_neigh.bias);
        ww!(self.w_self3,  &model.layers[2].w_self.weight);  ww!(self.b_self3,  &model.layers[2].w_self.bias);
        ww!(self.w_neigh3, &model.layers[2].w_neigh.weight); ww!(self.b_neigh3, &model.layers[2].w_neigh.bias);
        ww!(self.w_self4,  &model.layers[3].w_self.weight);  ww!(self.b_self4,  &model.layers[3].w_self.bias);
        ww!(self.w_neigh4, &model.layers[3].w_neigh.weight); ww!(self.b_neigh4, &model.layers[3].w_neigh.bias);
        ww!(self.w_act1,   &model.w_act1.weight);   ww!(self.b_act1,   &model.w_act1.bias);
        ww!(self.w_act2,   &model.w_act2.weight);   ww!(self.b_act2,   &model.w_act2.bias);
        ww!(self.w_val1,   &model.w_val1.weight);   ww!(self.b_val1,   &model.w_val1.bias);
        ww!(self.w_val2,   &model.w_val2.weight);   ww!(self.b_val2,   &model.w_val2.bias);
        self.queue.submit([]);
    }

    pub fn sync_weights(&self, model: &HexGNNModel) {
        self.update_weights_from_model(model);
    }

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

        if total_nodes == 0 || total_actions == 0 || total_nodes > MAX_NODES || total_actions > MAX_ACTIONS || batch > MAX_BATCH {
            return None;
        }

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
                let h0_seg: Vec<f32> = obs.node_features.iter().flatten().copied().collect();

                let mut degrees = vec![0u32; n];
                for &(u, v) in &obs.edge_index {
                    if u < n && v < n { degrees[u] += 1; }
                }

                let mut csr_off_seg = Vec::with_capacity(n + 1);
                let mut acc = 0u32;
                csr_off_seg.push(0u32);
                for d in &degrees { acc += d; csr_off_seg.push(acc); }

                let mut csr_targets_seg = Vec::new();
                for &(u, v) in &obs.edge_index {
                    if u < n && v < n { csr_targets_seg.push(v as u32); }
                }

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

        for (i, h0_seg, csr_off_seg, csr_targets_seg, act_u_seg, act_feat_seg) in per_obs {
            h0.extend_from_slice(&h0_seg);
            let base = *csr_offsets.last().unwrap();
            for &o in &csr_off_seg[1..] { csr_offsets.push(base + o); }
            let off = node_offsets[i];
            for t in csr_targets_seg { csr_targets.push(t + off); }
            for u in act_u_seg { act_node_u.push(u + off); }
            act_feat.extend_from_slice(&act_feat_seg);
        }

        self.queue.write_buffer(&slot.h0, 0, bytemuck::cast_slice(&h0));
        self.queue.write_buffer(&slot.csr_offsets, 0, bytemuck::cast_slice(&csr_offsets));
        self.queue.write_buffer(&slot.csr_targets, 0, bytemuck::cast_slice(&csr_targets));
        self.queue.write_buffer(&slot.act_node_u, 0, bytemuck::cast_slice(&act_node_u));
        self.queue.write_buffer(&slot.act_feat, 0, bytemuck::cast_slice(&act_feat));
        self.queue.write_buffer(&slot.node_offsets, 0, bytemuck::cast_slice(&node_offsets));

        let tn = total_nodes as u32;
        let ta = total_actions as u32;
        let b = batch as u32;
        let hd = HIDDEN_DIM as u32;
        let nfd = NODE_FEAT_DIM as u32;
        let afd = ACTION_FEAT_DIM as u32;

        let all_uniforms = AllUniforms {
            total_nodes: tn, total_actions: ta, batch: b, hidden_dim: hd,
            node_feat_dim: nfd, action_dim: afd, _pad0: 0, _pad1: 0,
        };
        self.queue.write_buffer(&slot.uniforms, 0, bytemuck::bytes_of(&all_uniforms));

        let wg_d = (hd + 15) / 16;
        let wg_n = (tn + 15) / 16;
        let wg_a = (ta + 15) / 16;
        let wg_b = (b + 15) / 16;

        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());

            pass.set_pipeline(&self.agg_l1_pipeline);
            pass.set_bind_group(0, &slot.bg_agg1, &[]);
            pass.dispatch_workgroups(wg_n, (nfd + 15) / 16, 1);

            pass.set_pipeline(&self.fused_l1_pipeline);
            pass.set_bind_group(0, &slot.bg_fused1, &[]);
            pass.dispatch_workgroups(wg_n, wg_d, 1);

            pass.set_pipeline(&self.agg_ln_pipeline);
            pass.set_bind_group(0, &slot.bg_agg2, &[]);
            pass.dispatch_workgroups(wg_n, wg_d, 1);

            pass.set_pipeline(&self.fused_ln_pipeline);
            pass.set_bind_group(0, &slot.bg_fused2, &[]);
            pass.dispatch_workgroups(wg_n, wg_d, 1);

            pass.set_pipeline(&self.agg_ln_pipeline);
            pass.set_bind_group(0, &slot.bg_agg3, &[]);
            pass.dispatch_workgroups(wg_n, wg_d, 1);

            pass.set_pipeline(&self.fused_ln_pipeline);
            pass.set_bind_group(0, &slot.bg_fused3, &[]);
            pass.dispatch_workgroups(wg_n, wg_d, 1);

            pass.set_pipeline(&self.agg_ln_pipeline);
            pass.set_bind_group(0, &slot.bg_agg4, &[]);
            pass.dispatch_workgroups(wg_n, wg_d, 1);

            pass.set_pipeline(&self.fused_ln_pipeline);
            pass.set_bind_group(0, &slot.bg_fused4, &[]);
            pass.dispatch_workgroups(wg_n, wg_d, 1);

            pass.set_pipeline(&self.gather_pipeline);
            pass.set_bind_group(0, &slot.bg_gather, &[]);
            pass.dispatch_workgroups((ta + 63) / 64, 1, 1);

            pass.set_pipeline(&self.pool_pipeline);
            pass.set_bind_group(0, &slot.bg_pool, &[]);
            pass.dispatch_workgroups((b + 63) / 64, 1, 1);

            pass.set_pipeline(&self.act1_pipeline);
            pass.set_bind_group(0, &slot.bg_act1, &[]);
            pass.dispatch_workgroups(wg_a, wg_d, 1);

            pass.set_pipeline(&self.val1_pipeline);
            pass.set_bind_group(0, &slot.bg_val1, &[]);
            pass.dispatch_workgroups(wg_b, wg_d, 1);

            pass.set_pipeline(&self.act2_pipeline);
            pass.set_bind_group(0, &slot.bg_act2, &[]);
            pass.dispatch_workgroups(wg_a, 1, 1);

            pass.set_pipeline(&self.val2_pipeline);
            pass.set_bind_group(0, &slot.bg_val2, &[]);
            pass.dispatch_workgroups(wg_b, 1, 1);
        }

        enc.copy_buffer_to_buffer(&slot.act_o, 0, &slot.staging_act, 0, (total_actions * 4) as u64);
        enc.copy_buffer_to_buffer(&slot.val_o, 0, &slot.staging_val, 0, (batch * 4) as u64);
        self.queue.submit(Some(enc.finish()));

        let (tx_act, rx_act) = crossbeam_channel::bounded(1);
        slot.staging_act.slice(..).map_async(wgpu::MapMode::Read, move |r| { let _ = tx_act.send(r); });
        let (tx_val, rx_val) = crossbeam_channel::bounded(1);
        slot.staging_val.slice(..).map_async(wgpu::MapMode::Read, move |r| { let _ = tx_val.send(r); });

        Some(PendingGpuResult {
            staging_act: Arc::clone(&slot.staging_act),
            staging_val: Arc::clone(&slot.staging_val),
            rx_act, rx_val, batch, action_offsets, action_counts,
        })
    }

    pub fn forward_batch_gpu(&self, observations: &[&GraphObservation]) -> Vec<(Vec<f32>, f32)> {
        if let Some(pending) = self.forward_batch_gpu_async_slot(0, observations) {
            pending.wait(&self.device)
        } else {
            observations.iter().map(|o| (vec![0.0f32; o.valid_actions.len()], 0.0f32)).collect()
        }
    }
}

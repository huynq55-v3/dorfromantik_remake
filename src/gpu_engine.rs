use wgpu::{Adapter, Device, Instance, Queue};
use std::sync::Arc;
use crate::nn::HexGNNModel;
use crate::env::GraphObservation;

/// Cấu trúc quản lý phần cứng GPU Intel Iris Xe thông qua Vulkan API (wgpu)
pub struct GpuEngine {
    pub instance: Instance,
    pub adapter: Adapter,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub device_name: String,
}

impl GpuEngine {
    /// Khởi tạo GPU context. Tìm kiếm GPU rời (Nvidia / AMD / Intel)
    pub fn new() -> Option<Self> {
        let instance = Instance::default();

        // 1. Quét danh sách tất cả các adapter và ưu tiên GPU rời (DiscreteGPU)
        let adapters = instance.enumerate_adapters(wgpu::Backends::all());
        let mut chosen_adapter = None;

        for a in adapters {
            let info = a.get_info();
            // Bỏ qua llvmpipe (CPU software rendering) nếu có GPU phần cứng
            if info.device_type == wgpu::DeviceType::DiscreteGpu {
                chosen_adapter = Some(a);
                break;
            } else if info.device_type == wgpu::DeviceType::IntegratedGpu && chosen_adapter.is_none() {
                chosen_adapter = Some(a);
            }
        }

        let adapter = if let Some(a) = chosen_adapter {
            a
        } else {
            pollster::block_on(async {
                instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::HighPerformance,
                        compatible_surface: None,
                        force_fallback_adapter: false,
                    })
                    .await
            })?
        };

        let info = adapter.get_info();
        let device_name = format!("{} ({:?}, {:?})", info.name, info.device_type, info.backend);
        let limits = adapter.limits();

        let (device, queue) = pollster::block_on(async {
            adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("Dorfromantik GPU Device"),
                        required_features: wgpu::Features::empty(),
                        required_limits: limits,
                        memory_hints: wgpu::MemoryHints::Performance,
                    },
                    None,
                )
                .await
                .ok()
        })?;

        Some(Self {
            instance,
            adapter,
            device: Arc::new(device),
            queue: Arc::new(queue),
            device_name,
        })
    }
}

/// Yêu cầu đánh giá trạng thái từ luồng MCTS
pub struct GpuEvalRequest {
    pub obs: GraphObservation,
    pub response_tx: crossbeam_channel::Sender<(Vec<f32>, f32)>,
}

/// Hàng đợi Gom Batch Đánh Giá Trạng Thái trên GPU (EvalQueue)
pub struct GpuEvalQueue {
    pub tx: crossbeam_channel::Sender<GpuEvalRequest>,
}

impl GpuEvalQueue {
    pub fn new(
        model: HexGNNModel,
        batch_size: usize,
        timeout_micros: u64,
    ) -> Self {
        let (tx, rx) = crossbeam_channel::bounded::<GpuEvalRequest>(2048);

        std::thread::spawn(move || {
            let mut requests: Vec<GpuEvalRequest> = Vec::with_capacity(batch_size);

            loop {
                requests.clear();

                // Chờ request đầu tiên
                let first_req = match rx.recv() {
                    Ok(req) => req,
                    Err(_) => break, // Channel đóng -> thoát luồng
                };
                requests.push(first_req);

                // Gom thêm các request tiếp theo tới khi đủ batch_size hoặc hết timeout
                let gap_timeout = std::time::Duration::from_micros(timeout_micros);
                while requests.len() < batch_size {
                    match rx.recv_timeout(gap_timeout) {
                        Ok(req) => requests.push(req),
                        Err(_) => break,
                    }
                }

                let current_batch_size = requests.len();
                if current_batch_size == 0 {
                    continue;
                }

                let obs_refs: Vec<&GraphObservation> = requests.iter().map(|r| &r.obs).collect();
                let batch_results = model.forward_batch(&obs_refs);
                for (req, (logits, val)) in requests.drain(..).zip(batch_results) {
                    let _ = req.response_tx.send((logits, val));
                }
            }
        });

        Self { tx }
    }
}

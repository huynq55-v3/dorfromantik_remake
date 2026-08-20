# HƯỚNG DẪN THIẾT LẬP VÀ CHẠY HUẤN LUYỆN ALPHAZERO TRÊN VAST.AI / GPU SERVER

Tài liệu này ghi lại toàn bộ quy trình thiết lập môi trường chuẩn từ đầu trên máy chủ GPU thuê (như Vast.ai với RTX 4090 / 5090), bao gồm:
1. Cài đặt Vulkan Runtime & Driver Nvidia chuẩn.
2. Cài đặt Rust Compiler.
3. Đồng bộ dữ liệu/models qua `rsync`.
4. Lệnh chạy huấn luyện AlphaZero tối ưu.

---

## 1. Cấu Hình Vulkan Chuẩn Cho GPU Nvidia Trên Vast.ai (Chạy 1 Lần Khi Bật Máy)

Trên các Docker container rút gọn của Vast.ai, mặc định chỉ có CUDA driver mà thiếu thư viện Vulkan ICD liên kết. Chạy cụm lệnh sau để kích hoạt $100\%$ nhận diện GPU rời Nvidia:

```bash
# 1.1 Cập nhật và cài đặt các thư viện X11 & Vulkan Loader phụ thuộc
apt-get update && apt-get install -y libvulkan1 vulkan-tools libxext6 libx11-6 libx11-xcb1 libxcb1 zstd htop tmux

# 1.2 Tạo cấu hình Vulkan ICD trỏ đúng vào thư viện driver Nvidia có sẵn trong container
mkdir -p /root/.local/share/vulkan/icd.d
cat << 'EOF' > /root/.local/share/vulkan/icd.d/nvidia_icd.json
{
    "file_format_version": "1.0.0",
    "ICD": {
        "library_path": "libEGL_nvidia.so.0",
        "api_version": "1.3.0"
    }
}
EOF

# 1.3 Thiết lập biến môi trường Vulkan & Nvidia
export VK_ICD_FILENAMES=/root/.local/share/vulkan/icd.d/nvidia_icd.json
export LD_LIBRARY_PATH=/usr/lib/x86_64-linux-gnu:$LD_LIBRARY_PATH
export NVIDIA_DRIVER_CAPABILITIES=all

# (Tùy chọn) Thêm vào ~/.bashrc để mỗi lần mở tab mới không cần gõ lại
echo 'export VK_ICD_FILENAMES=/root/.local/share/vulkan/icd.d/nvidia_icd.json' >> ~/.bashrc
echo 'export LD_LIBRARY_PATH=/usr/lib/x86_64-linux-gnu:$LD_LIBRARY_PATH' >> ~/.bashrc
echo 'export NVIDIA_DRIVER_CAPABILITIES=all' >> ~/.bashrc

# 1.4 Kiểm tra nhận diện GPU (phải thấy hiện NVIDIA GeForce RTX 4090 / 5090)
vulkaninfo --summary
```

---

## 2. Cài Đặt Rust Compiler

```bash
# 2.1 Cài đặt Rust qua rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# 2.2 Kiểm tra phiên bản Rust
rustc --version
cargo --version
```

---

## 3. Clone Repository & Đồng Bộ Dữ Liệu (`rsync`)

### 3.1 Clone mã nguồn từ GitHub
```bash
git clone https://github.com/huynq55-v3/dorfromantik_remake.git
cd dorfromantik_remake
```

### 3.2 Lệnh `rsync` Upload Dữ Liệu / Replay Buffer Từ Máy Cá Nhân Lên Vast.ai
*(Chạy lệnh này từ Terminal trên máy cá nhân của bạn)*:

```bash
# Cú pháp mẫu: rsync -avz -P -e "ssh -p <PORT>" <FILE_HOAC_THU_MUC> root@<IP_VAST_AI>:<DUONG_DAN_DICH>

# Ví dụ 1: Upload file zip nén thư mục models
rsync -avz -P -e "ssh -p 39584" upload_to_faster_machine.zip root@180.189.55.43:~/dorfromantik_remake/

# Ví dụ 2: Đồng bộ trực tiếp cả thư mục models/
rsync -avz -P -e "ssh -p 39584" ./models/ root@180.189.55.43:~/dorfromantik_remake/models/
```

### 3.3 Lệnh `rsync` Tải Checkpoint Mới Nhất Từ Vast.ai Về Máy Cá Nhân
*(Chạy lệnh này từ Terminal trên máy cá nhân)*:

```bash
# Tải toàn bộ thư mục models đã train xong về máy
rsync -avz -P -e "ssh -p 39584" root@180.189.55.43:~/dorfromantik_remake/models/ ./models/
```

---

## 4. Hướng Dẫn Chạy Huấn Luyện AlphaZero

### 4.1 Thứ Tự Tham Số Dòng Lệnh:
```bash
cargo run --release --bin train_alphazero_gpu -- [parallel_envs] [n_simulations] [buffer_capacity] [train_epochs] [iter_max]
```

- `[parallel_envs]`: Số môi trường game chạy song song (Mặc định `512`).
- `[n_simulations]`: Số bước duyệt MCTS mỗi lượt (Mặc định `400`).
- `[buffer_capacity]`: Sức chứa Replay Buffer (Mặc định `300000`).
- `[train_epochs]`: Số epoch huấn luyện gradient mỗi iteration (Mặc định `2`).
- `[iter_max]`: Vòng lặp tối đa để dừng lại (Mặc định chạy liên tục không giới hạn).

### 4.2 Cấu Hình Tối Ưu Cho RTX 4070Ti / 4080 / 4090 / 5090:

#### Cách 1: Chạy nền với `tmux` (Khuyên dùng - Tiện theo dõi) 🏆
```bash
# 1. Mở phiên làm việc tmux:
tmux

# 2. Chạy lệnh train (512 envs, 400 hoặc 800 sims, 300k buffer, 2 epochs):
cargo run --release --bin train_alphazero_gpu -- 512 400 300000 2

# 3. Thoát ra an toàn (tiến trình vẫn chạy ngầm): Nhấn Ctrl+B, thả tay ra rồi bấm phím D.
# 4. Vào lại xem tiến trình bất cứ lúc nào:
tmux attach
```

#### Cách 2: Chạy ngầm vĩnh viễn với `nohup` (Tắt SSH / Tắt máy thoải mái) ⚡
```bash
# 1. Chạy ngầm và ghi toàn bộ log vào file train.log:
nohup cargo run --release --bin train_alphazero_gpu -- 512 400 300000 2 > train.log 2>&1 &

# 2. Xem log tiến trình real-time:
tail -f train.log

# 3. Thoát khỏi xem log (không ảnh hưởng đến train): Nhấn Ctrl + C

# 4. Lệnh dừng/tắt tiến trình train đang chạy ngầm:
pkill -f train_alphazero_gpu
```

---

## 5. Các Lệnh Theo Dõi Hệ Thống Real-time
- **Theo dõi GPU Nvidia (Nhiệt độ, công suất W, VRAM):**
  ```bash
  watch -n 1 nvidia-smi
  ```
- **Theo dõi CPU & RAM:**
  ```bash
  htop
  ```
- **Kiểm tra tiến trình train đang chạy:**
  ```bash
  ps aux | grep train_alphazero
  ```

use std::path::Path;
use std::time::Instant;
use dorfromantik_remake::alphazero::{AlphaZeroPipeline, AlphaZeroTrainerConfig};
use dorfromantik_remake::mcts::MCTSConfig;
use dorfromantik_remake::nn::HexGNNModel;

fn main() {
    // Tham số 1: Số epochs muốn train (mặc định 10)
    // Tham số 2: Batch size (mặc định 1024)
    // Tham số 3: Đường dẫn model khởi đầu (mặc định models/alphazero_latest.bin hoặc alphazero_best.bin)
    let args: Vec<String> = std::env::args().collect();
    let epochs = args.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(10);
    let batch_size = args.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1024);

    let model_dir = "models";
    let buffer_path = format!("{}/alphazero_buffer.bin", model_dir);
    let latest_model_path = format!("{}/alphazero_latest.bin", model_dir);
    let best_model_path = format!("{}/alphazero_best.bin", model_dir);

    let init_model_path = if args.len() > 3 {
        args[3].clone()
    } else if Path::new(&latest_model_path).exists() {
        latest_model_path.clone()
    } else if Path::new(&best_model_path).exists() {
        best_model_path.clone()
    } else {
        println!("Không tìm thấy model checkpoint cũ, khởi tạo model ngẫu nhiên.");
        String::new()
    };

    let config = AlphaZeroTrainerConfig {
        lr: 0.0003,
        gamma: 0.995,
        value_loss_coeff: 0.5,
        batch_size,
        train_epochs_per_iter: epochs,
        mcts_config: MCTSConfig::default(),
        num_parallel_envs: 1,
        target_seed: -2093096630,
        initial_stack: 10,
        tile_limit: 100,
        replay_buffer_capacity: Some(200_000),
    };

    let mut pipeline = AlphaZeroPipeline::new(config);

    if !init_model_path.is_empty() && Path::new(&init_model_path).exists() {
        println!("[Init] Đang nạp model từ `{}`...", init_model_path);
        match HexGNNModel::load_from_file(&init_model_path) {
            Ok(m) => {
                println!("[Init] SUCCESS: Đã nạp model (Step count = {})", m.step_count);
                pipeline.model = m;
            }
            Err(e) => {
                println!("[Init] CẢNH BÁO: Không nạp được model ({:?})", e);
            }
        }
    }

    if !Path::new(&buffer_path).exists() {
        eprintln!("Lỗi: không tìm thấy buffer `{}` để train!", buffer_path);
        std::process::exit(1);
    }

    println!("[Buffer] Đang nạp Replay Buffer từ `{}`...", buffer_path);
    match pipeline.replay_buffer.load_from_file(&buffer_path) {
        Ok(count) => {
            println!("[Buffer] SUCCESS: Đã nạp {} samples từ file buffer!", count);
            let merged = pipeline.replay_buffer.merge_symmetric_actions();
            if merged > 0 {
                println!("[Buffer] Đã gộp và chuẩn hóa {} actions đẳng cấu (đối xứng xoay)!", merged);
            }
            pipeline.replay_buffer.migrate_action_features();
            println!("[Buffer] Đã cập nhật NEIGHBOR_COUNT (kênh 3) cho toàn bộ samples!");
        }
        Err(e) => {
            eprintln!("Lỗi đọc buffer: {:?}", e);
            std::process::exit(1);
        }
    }

    println!("\n>>> BẮT ĐẦU OFFLINE SUPERVISED TRAINING ({} EPOCHS) <<<", epochs);
    let t0 = Instant::now();
    let (total_loss, pi_loss, val_loss) = pipeline.train_step();
    let dur = t0.elapsed();

    println!(
        "\n✅ HOÀN THÀNH OFFLINE TRAIN trong {:.2}s: Policy Loss: {:.4} | Value Loss: {:.4} | Total Loss: {:.4}",
        dur.as_secs_f32(), pi_loss, val_loss, total_loss
    );

    // Lưu lại model
    if let Err(e) = pipeline.model.save_to_file(&latest_model_path) {
        println!("[Save Error] Không thể lưu latest model: {:?}", e);
    } else {
        println!("[Save] Đã cập nhật model vào `{}`", latest_model_path);
    }
    if let Err(e) = pipeline.model.save_to_file(&best_model_path) {
        println!("[Save Error] Không thể lưu best model: {:?}", e);
    } else {
        println!("[Save] Đã cập nhật model vào `{}`", best_model_path);
    }

    // Lưu lại buffer đã migrate
    if let Err(e) = pipeline.replay_buffer.save_to_file(&buffer_path) {
        println!("[Save Error] Không thể lưu replay buffer: {:?}", e);
    } else {
        println!("[Save] Đã lưu lại buffer đã migrate vào `{}`", buffer_path);
    }
}

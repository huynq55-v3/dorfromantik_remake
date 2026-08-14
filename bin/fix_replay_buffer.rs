use std::path::Path;
use std::time::Instant;
use dorfromantik_remake::alphazero::AlphaZeroReplayBuffer;

fn main() {
    println!("============================================================");
    println!("=== CÔNG CỤ KHÔI PHỤC VÀ LÀM MỀM REPLAY BUFFER (UN-SHARPEN) ===");
    println!("============================================================");

    let buffer_path = "models/alphazero_buffer.bin";
    let backup_path = "models/alphazero_buffer_backup.bin";

    if !Path::new(buffer_path).exists() {
        println!("[Lỗi] Không tìm thấy file buffer tại `{}`!", buffer_path);
        return;
    }

    let args: Vec<String> = std::env::args().collect();
    let factor = if args.len() > 1 {
        args[1].parse::<f32>().unwrap_or(0.35)
    } else {
        0.35
    };

    println!("[1/4] Đang nạp buffer từ `{}`...", buffer_path);
    let mut replay_buffer = AlphaZeroReplayBuffer::new(500_000);
    let start_load = Instant::now();
    match replay_buffer.load_from_file(buffer_path) {
        Ok(count) => {
            println!(
                "  ✓ Đã nạp thành công {} samples trong {:.2}ms",
                count,
                start_load.elapsed().as_secs_f64() * 1000.0
            );
        }
        Err(e) => {
            println!("[Lỗi] Không thể đọc buffer: {:?}", e);
            return;
        }
    }

    println!("[2/4] Đang sao lưu buffer cũ sang `{}`...", backup_path);
    if let Err(e) = std::fs::copy(buffer_path, backup_path) {
        println!("  ⚠️ Không thể tạo bản backup ({:?}), tiếp tục...", e);
    } else {
        println!("  ✓ Sao lưu thành công.");
    }

    println!("[3/4] Đang khôi phục làm mềm target_pi với hệ số factor = {}...", factor);
    let start_fix = Instant::now();
    replay_buffer.unsharpen_target_pi(factor);
    println!(
        "  ✓ Đã làm mềm {} samples trong {:.2}ms!",
        replay_buffer.len(),
        start_fix.elapsed().as_secs_f64() * 1000.0
    );

    println!("[4/4] Đang lưu lại buffer đã fix vào `{}`...", buffer_path);
    let start_save = Instant::now();
    match replay_buffer.save_to_file(buffer_path) {
        Ok(_) => {
            println!(
                "  ✓ Đã lưu thành công trong {:.2}ms!",
                start_save.elapsed().as_secs_f64() * 1000.0
            );
        }
        Err(e) => {
            println!("[Lỗi] Không thể ghi file buffer: {:?}", e);
            return;
        }
    }

    println!("============================================================");
    println!("🎉 HOÀN TẤT! Replay buffer đã được khôi phục phân phối xác suất mềm chuẩn.");
    println!("============================================================");
}

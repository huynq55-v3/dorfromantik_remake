use dorfromantik_remake::alphazero::AlphaZeroReplayBuffer;
use rand::prelude::*;
use std::collections::HashSet;

fn main() {
    let buffer_path = "models/alphazero_buffer.bin";
    println!(">>> Đang đọc file Replay Buffer: {} ...", buffer_path);
    
    let mut replay_buffer = AlphaZeroReplayBuffer::new(200_000);
    match replay_buffer.load_from_file(buffer_path) {
        Ok(count) => println!(">>> Tải thành công! Tổng số sample trong buffer: {}", count),
        Err(e) => {
            eprintln!("Lỗi khi đọc file buffer: {}", e);
            return;
        }
    }

    let total_samples = replay_buffer.len();
    if total_samples == 0 {
        println!("Buffer rỗng!");
        return;
    }

    let num_tail = 200.min(total_samples);
    let start_idx = total_samples - num_tail;
    let tail_indices: Vec<usize> = (start_idx..total_samples).collect();

    println!("\n>>> Đang phân tích chi tiết {} MẪU CUỐI CÙNG (Index {} -> {})...", num_tail, start_idx, total_samples - 1);

    let mut nan_inf_count = 0;
    let mut invalid_edge_count = 0;
    let mut invalid_pi_sum_count = 0;
    let mut dim_mismatch_count = 0;
    let mut action_out_of_bounds = 0;
    let mut invalid_rotations = 0;
    let mut duplicate_actions = 0;

    let mut entropies = Vec::with_capacity(num_tail);
    let mut top1_probs = Vec::with_capacity(num_tail);
    let mut top3_probs = Vec::with_capacity(num_tail);
    let mut top5_probs = Vec::with_capacity(num_tail);
    let mut non_zero_counts = Vec::with_capacity(num_tail);
    let mut node_counts = Vec::with_capacity(num_tail);
    let mut edge_counts = Vec::with_capacity(num_tail);
    let mut action_counts = Vec::with_capacity(num_tail);
    let mut target_vals = Vec::with_capacity(num_tail);

    let mut sharp_samples = Vec::new(); // Top-1 >= 50%
    let mut moderate_samples = Vec::new(); // 20% < Top-1 < 50%
    let mut soft_samples = Vec::new();  // Top-1 <= 20%
    let mut flat_samples = Vec::new();  // Top-1 <= 10%

    for (_rel_i, &idx) in tail_indices.iter().enumerate() {
        let sample = &replay_buffer.buffer[idx];
        let obs = &sample.obs;
        let n_nodes = obs.node_features.len();
        let n_edges = obs.edge_index.len();
        let n_actions = obs.valid_actions.len();
        let pi = &sample.target_pi;
        let val = sample.target_val;

        // 1. Sanity Checks
        if n_actions != obs.action_features.len() || n_actions != pi.len() { dim_mismatch_count += 1; }
        let mut has_nan = val.is_nan() || val.is_infinite();
        for f in &obs.node_features { for &x in f { if x.is_nan() || x.is_infinite() { has_nan = true; } } }
        for f in &obs.action_features { for &x in f { if x.is_nan() || x.is_infinite() { has_nan = true; } } }
        for &p in pi { if p.is_nan() || p.is_infinite() { has_nan = true; } }
        if has_nan { nan_inf_count += 1; }

        for &(u, v) in &obs.edge_index { if u >= n_nodes || v >= n_nodes { invalid_edge_count += 1; } }

        let pi_sum: f32 = pi.iter().sum();
        if (pi_sum - 1.0).abs() > 1e-3 && !pi.is_empty() { invalid_pi_sum_count += 1; }

        let mut seen_acts = std::collections::HashSet::new();
        for act in &obs.valid_actions {
            if act.rotation > 5 { invalid_rotations += 1; }
            if !seen_acts.insert((act.q, act.r, act.rotation)) { duplicate_actions += 1; }
            if !obs.node_positions.iter().any(|&p| p == (act.q, act.r)) { action_out_of_bounds += 1; }
        }

        // 2. Metrics
        let mut sorted_pi = pi.clone();
        sorted_pi.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        let top1 = *sorted_pi.first().unwrap_or(&0.0);
        let top3: f32 = sorted_pi.iter().take(3).sum();
        let top5: f32 = sorted_pi.iter().take(5).sum();
        let non_zero = pi.iter().filter(|&&p| p > 0.005).count();

        let entropy: f32 = pi.iter()
            .filter(|&&p| p > 1e-8)
            .map(|&p| -p * p.ln())
            .sum();

        entropies.push(entropy);
        top1_probs.push(top1);
        top3_probs.push(top3);
        top5_probs.push(top5);
        non_zero_counts.push(non_zero);
        node_counts.push(n_nodes);
        edge_counts.push(n_edges);
        action_counts.push(n_actions);
        target_vals.push(val);

        if top1 >= 0.50 {
            sharp_samples.push((idx, n_nodes, n_actions, top1, top3, entropy, val));
        } else if top1 > 0.20 {
            moderate_samples.push((idx, n_nodes, n_actions, top1, top3, entropy, val));
        } else {
            soft_samples.push((idx, n_nodes, n_actions, top1, top3, entropy, val));
            if top1 <= 0.10 {
                flat_samples.push((idx, n_nodes, n_actions, top1, top3, entropy, val));
            }
        }
    }

    let avg_top1: f32 = top1_probs.iter().sum::<f32>() / num_tail as f32;
    let avg_top3: f32 = top3_probs.iter().sum::<f32>() / num_tail as f32;
    let avg_top5: f32 = top5_probs.iter().sum::<f32>() / num_tail as f32;
    let avg_entropy: f32 = entropies.iter().sum::<f32>() / num_tail as f32;
    let avg_nodes: f32 = node_counts.iter().sum::<usize>() as f32 / num_tail as f32;
    let avg_edges: f32 = edge_counts.iter().sum::<usize>() as f32 / num_tail as f32;
    let avg_actions: f32 = action_counts.iter().sum::<usize>() as f32 / num_tail as f32;
    let avg_non_zero: f32 = non_zero_counts.iter().sum::<usize>() as f32 / num_tail as f32;
    let avg_val: f32 = target_vals.iter().sum::<f32>() / num_tail as f32;

    println!("\n=========================================================================================");
    println!("                    BÁO CÁO PHÂN TÍCH 200 MẪU CUỐI CÙNG TRONG BUFFER");
    println!("=========================================================================================");
    println!(">>> Tổng số samples hiện có trong Buffer: {}", total_samples);

    println!("\n1. 🛡️ KIỂM TRA TÍNH TOÀN VẸN (SANITY & INTEGRITY CHECKS):");
    println!("  - Lỗi NaN / Inf: {}/200 {}", nan_inf_count, if nan_inf_count == 0 { "✅ (Sạch 100%)" } else { "❌ LỖI" });
    println!("  - Lệch chiều dữ liệu: {}/200 {}", dim_mismatch_count, if dim_mismatch_count == 0 { "✅ (Khớp 100%)" } else { "❌ LỖI" });
    println!("  - Cạnh trỏ ngoài phạm vi: {} {}", invalid_edge_count, if invalid_edge_count == 0 { "✅ (Chuẩn)" } else { "❌ LỖI" });
    println!("  - Action sai vị trí / rotation: {} {}", action_out_of_bounds + invalid_rotations, if action_out_of_bounds + invalid_rotations == 0 { "✅ (Chuẩn)" } else { "❌ LỖI" });
    println!("  - Action bị trùng lặp trong cùng 1 state: {} {}", duplicate_actions, if duplicate_actions == 0 { "✅ (Chuẩn)" } else { "❌ LỖI" });
    println!("  - Chuẩn hóa xác suất Pi sum=1.0: {}/200 {}", invalid_pi_sum_count, if invalid_pi_sum_count == 0 { "✅ (Chuẩn)" } else { "❌ LỖI" });

    println!("\n2. 📊 ĐẶC TÍNH QUY MÔ & PHÂN PHỐI NƯỚC ĐI:");
    println!("  - Số Nodes trung bình: {:.1} nodes (min: {}, max: {})", avg_nodes, node_counts.iter().min().unwrap(), node_counts.iter().max().unwrap());
    println!("  - Số Cạnh trung bình: {:.1} edges (min: {}, max: {})", avg_edges, edge_counts.iter().min().unwrap(), edge_counts.iter().max().unwrap());
    println!("  - Số Actions hợp lệ: {:.1} actions (min: {}, max: {})", avg_actions, action_counts.iter().min().unwrap(), action_counts.iter().max().unwrap());
    println!("  - Số Actions được MCTS thăm dò (p > 0.5%): {:.1} / {:.1} actions ({:.1}%)", avg_non_zero, avg_actions, (avg_non_zero / avg_actions) * 100.0);
    println!("  - Target Value ($G_t$) trung bình: {:.3} (min: {:.3}, max: {:.3})", avg_val, target_vals.iter().cloned().fold(f32::MAX, f32::min), target_vals.iter().cloned().fold(f32::MIN, f32::max));

    println!("\n3. 🧠 PHÂN TÍCH TẬP TRUNG XÁC SUẤT POLICY (MCTS TARGET PI):");
    println!("  - Top-1 Probability TB: {:.2}% (min: {:.1}%, max: {:.1}%)", avg_top1 * 100.0, top1_probs.iter().cloned().fold(f32::MAX, f32::min) * 100.0, top1_probs.iter().cloned().fold(0.0, f32::max) * 100.0);
    println!("  - Top-3 Probability TB: {:.2}%", avg_top3 * 100.0);
    println!("  - Top-5 Probability TB: {:.2}%", avg_top5 * 100.0);
    println!("  - Policy Entropy TB: {:.3} nats", avg_entropy);

    println!("\n4. 🎯 PHÂN LOẠI CHIẾN THUẬT POLICY (SHARPNESS SPECTRUM):");
    println!("  - Quyết đoán cao (Top-1 >= 50%): {}/{} mẫu ({:.1}%)", sharp_samples.len(), num_tail, (sharp_samples.len() as f32 / num_tail as f32) * 100.0);
    println!("  - Tập trung vừa phải (20% < Top-1 < 50%): {}/{} mẫu ({:.1}%)", moderate_samples.len(), num_tail, (moderate_samples.len() as f32 / num_tail as f32) * 100.0);
    println!("  - Mở rộng thăm dò (Top-1 <= 20%): {}/{} mẫu ({:.1}%)", soft_samples.len(), num_tail, (soft_samples.len() as f32 / num_tail as f32) * 100.0);
    println!("  - Rất phẳng (Top-1 <= 10%): {}/{} mẫu ({:.1}%)", flat_samples.len(), num_tail, (flat_samples.len() as f32 / num_tail as f32) * 100.0);

    println!("\n5. 🔍 MẪU TIÊU BIỂU TRONG 200 MẪU CUỐI:");
    println!("  [Mẫu nước đi bước ngoặt / Quyết đoán cao (Top-1 > 80%)]");
    for &(idx, n_n, n_a, t1, t3, ent, v) in sharp_samples.iter().filter(|s| s.3 > 0.80).take(4) {
        println!("    • Sample #{:<5} | Nodes: {:<2} | Act: {:<3} | Top1: {:>5.1}% | Top3: {:>5.1}% | Entropy: {:.2} | Val: {:<6.2}",
            idx, n_n, n_a, t1 * 100.0, t3 * 100.0, ent, v);
    }
    println!("  [Mẫu phân vân / Thăm dò đa hướng (Top-1 < 20%)]");
    for &(idx, n_n, n_a, t1, t3, ent, v) in soft_samples.iter().take(4) {
        println!("    • Sample #{:<5} | Nodes: {:<2} | Act: {:<3} | Top1: {:>5.1}% | Top3: {:>5.1}% | Entropy: {:.2} | Val: {:<6.2}",
            idx, n_n, n_a, t1 * 100.0, t3 * 100.0, ent, v);
    }
    println!("=========================================================================================");
}

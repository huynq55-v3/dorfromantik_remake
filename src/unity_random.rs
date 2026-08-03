/// Bộ sinh số ngẫu nhiên mô phỏng theo UnityEngine.Random (Xorshift128)
#[derive(Debug, Clone)]
pub struct UnityRandom {
    pub s0: u32,
    pub s1: u32,
    pub s2: u32,
    pub s3: u32,
}

impl UnityRandom {
    /// Khởi tạo trạng thái PRNG chính xác 100% theo Unity Engine C++
    pub fn init_state(seed: i32) -> Self {
        let mut s0 = seed as u32;
        let s1 = 1812433253u32.wrapping_mul(s0).wrapping_add(1);
        let s2 = 1812433253u32.wrapping_mul(s1).wrapping_add(1);
        let s3 = 1812433253u32.wrapping_mul(s2).wrapping_add(1);

        if s0 == 0 && s1 == 0 && s2 == 0 && s3 == 0 {
            s0 = 1;
        }

        UnityRandom { s0, s1, s2, s3 }
    }

    /// Sinh số nguyên không dấu 32-bit tiếp theo (Xorshift128)
    pub fn next_u32(&mut self) -> u32 {
        let mut t = self.s0;
        t ^= t << 11;
        t ^= t >> 8;
        self.s0 = self.s1;
        self.s1 = self.s2;
        self.s2 = self.s3;
        self.s3 ^= self.s3 >> 19;
        self.s3 ^= t;
        self.s3
    }

    /// Trả về số thực trong khoảng [0.0, 1.0] (UnityEngine.Random.value)
    pub fn value(&mut self) -> f32 {
        let val = self.next_u32();
        (val & 0x007FFFFF) as f32 / 8388607.0
    }

    /// Sinh số thực trong khoảng [min, max] (UnityEngine.Random.Range(float, float))
    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + (1.0 - self.value()) * (max - min)
    }

    /// Sinh số nguyên trong khoảng [min, max) (UnityEngine.Random.Range(int, int))
    pub fn range_i32(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        let diff = (max as i64 - min as i64) as u64;
        let val = self.next_u32() as u64;
        min + (val % diff) as i32
    }

    /// Sinh số nguyên trong khoảng [min, max) (UnityEngine.Random.Range(usize, usize))
    pub fn range_usize(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let diff = (max - min) as u64;
        let val = self.next_u32() as u64;
        min + (val % diff) as usize
    }

    /// Chọn ngẫu nhiên có trọng số (Randomizer.SelectWeightedRandom)
    pub fn select_weighted<T: Clone>(&mut self, items: &[(T, f32)]) -> Option<T> {
        let total: f32 = items.iter().map(|(_, p)| p).sum();
        if total <= 0.0 {
            return None;
        }
        let mut roll = self.range_f32(0.0, total);
        for (item, prob) in items {
            roll -= prob;
            if roll <= 0.0 {
                return Some(item.clone());
            }
        }
        items.last().map(|(item, _)| item.clone())
    }

    /// Chọn ngẫu nhiên có trọng số kèm thông tin chi tiết (roll, ratio, total)
    pub fn select_weighted_info<T: Clone>(&mut self, items: &[(T, f32)]) -> Option<(T, f32, f32, f32)> {
        let total: f32 = items.iter().map(|(_, p)| p).sum();
        if total <= 0.0 {
            return None;
        }
        let roll = self.range_f32(0.0, total);
        let ratio = if total > 0.0 { roll / total } else { 0.0 };
        let mut temp = roll;
        for (item, prob) in items {
            temp -= prob;
            if temp <= 0.0 {
                return Some((item.clone(), roll, ratio, total));
            }
        }
        items.last().map(|(item, _)| (item.clone(), roll, ratio, total))
    }
}

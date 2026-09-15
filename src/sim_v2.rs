//! Cụm 3.1+3.2 — V2 sandwich math (constant product, phí 0.25% = 9975/10000
//! theo `DEX_REGISTRY.md`/CLAUDE.md mục "Math") + "victim still ok". U256
//! only, KHÔNG float cho tiền on-chain — `profit_wei` dùng `i128` (không
//! phải U256) vì có thể âm (lỗ), nhưng biên độ luôn nằm trong phạm vi BNB
//! thực tế (< `max_front_bnb` + gas, xa dưới `i128::MAX`) nên không tràn số,
//! khớp `docs/STATE.md` mục "Stack" (chỉ ví/reserve pool mới bắt buộc U256).
//!
//! Sandwich cổ điển (front-run chiều MUA: victim đổi WBNB lấy token):
//! 1. Attacker mua trước `front_in` WBNB -> nhận `front_out` token, đẩy giá.
//! 2. Victim mua sau, nhận `victim_out` token (giá đã xấu hơn do bước 1) —
//!    phải so với `amountOutMin` CHÍNH victim đặt (lấy từ `decoder.rs`,
//!    KHÔNG phải `min_swap_bnb` của `victims.txt`).
//! 3. Attacker bán lại `front_out` token, nhận `back_out` WBNB.
//! `profit = back_out - front_in - gas_front - gas_back`.

use alloy::primitives::U256;

/// Hệ số phí PancakeSwap V2 (0.25%) — nguồn `DEX_REGISTRY.md` mục V2, khớp
/// CLAUDE.md mục "Math".
pub const FEE_NUM: u64 = 9975;
pub const FEE_DEN: u64 = 10000;

/// `amountOut = (amountIn * 9975 * reserveOut) / (reserveIn * 10000 + amountIn * 9975)`
/// — U256 nguyên, chia làm tròn xuống (khớp Solidity `/`). `None` nếu
/// `amountIn`/`reserveIn`/`reserveOut` = 0, hoặc phép nhân tràn `U256`
/// (không xảy ra trong phạm vi số BNB/token thực tế, nhưng vẫn `checked_*`
/// thay vì unwrap để không panic trên input rác).
pub fn get_amount_out(amount_in: U256, reserve_in: U256, reserve_out: U256) -> Option<U256> {
    if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
        return None;
    }
    let amount_in_with_fee = amount_in.checked_mul(U256::from(FEE_NUM))?;
    let numerator = amount_in_with_fee.checked_mul(reserve_out)?;
    let denominator = reserve_in.checked_mul(U256::from(FEE_DEN))?.checked_add(amount_in_with_fee)?;
    if denominator.is_zero() {
        return None;
    }
    Some(numerator / denominator)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolReserves {
    pub reserve_wbnb: U256,
    pub reserve_token: U256,
}

/// Áp front trade rồi trade của victim lên CÙNG một pool (state nối tiếp) —
/// trả `(front_out, victim_out, reserves_sau_ca_hai_buoc)`. `None` nếu bước
/// nào tràn/chia-0 hoặc rút cạn pool (`reserve_token` không đủ trừ).
pub fn simulate_front_then_victim(
    reserves: PoolReserves,
    front_in: U256,
    victim_amount_in: U256,
) -> Option<(U256, U256, PoolReserves)> {
    let front_out = get_amount_out(front_in, reserves.reserve_wbnb, reserves.reserve_token)?;
    let reserve_wbnb_1 = reserves.reserve_wbnb.checked_add(front_in)?;
    let reserve_token_1 = reserves.reserve_token.checked_sub(front_out)?;

    let victim_out = get_amount_out(victim_amount_in, reserve_wbnb_1, reserve_token_1)?;
    let reserve_wbnb_2 = reserve_wbnb_1.checked_add(victim_amount_in)?;
    let reserve_token_2 = reserve_token_1.checked_sub(victim_out)?;

    Some((
        front_out,
        victim_out,
        PoolReserves { reserve_wbnb: reserve_wbnb_2, reserve_token: reserve_token_2 },
    ))
}

fn u256_to_i128(v: U256) -> Option<i128> {
    let as_u128: u128 = v.try_into().ok()?;
    i128::try_from(as_u128).ok()
}

/// Kết quả 1 mức `front_in` cụ thể — dùng cho search VÀ log/sim cuối.
#[derive(Debug, Clone, Copy)]
pub struct SandwichQuote {
    pub front_in: U256,
    pub front_out: U256,
    pub victim_out: U256,
    pub back_out: U256,
    /// `back_out - front_in - gas_wei`, có thể âm (lỗ) — KHÔNG phải U256.
    pub profit_wei: i128,
}

fn quote_at(reserves: PoolReserves, front_in: U256, victim_amount_in: U256, gas_wei: u128) -> Option<SandwichQuote> {
    let (front_out, victim_out, after) = simulate_front_then_victim(reserves, front_in, victim_amount_in)?;
    if front_out.is_zero() {
        // Khong mua duoc token nao (front_in qua nho / pool qua doc) -> khong
        // co gi de ban lai, coi nhu lo toan bo front_in + gas.
        let front_i = u256_to_i128(front_in)?;
        return Some(SandwichQuote {
            front_in,
            front_out,
            victim_out,
            back_out: U256::ZERO,
            profit_wei: -front_i - gas_wei as i128,
        });
    }
    let back_out = get_amount_out(front_out, after.reserve_token, after.reserve_wbnb)?;
    let back_i = u256_to_i128(back_out)?;
    let front_i = u256_to_i128(front_in)?;
    Some(SandwichQuote { front_in, front_out, victim_out, back_out, profit_wei: back_i - front_i - gas_wei as i128 })
}

/// Binary/ternary search `front_in` trong `[0, max_front_wei]` tối đa hoá
/// `profit_wei` — `profit(front_in)` lõm (concave) theo AMM constant-product
/// chuẩn (tăng dần rồi giảm dần do price impact), nên ternary search trên
/// miền số nguyên hội tụ về vùng tối ưu. Luôn kèm `max_front_wei` (và 2 đầu
/// khoảng thu hẹp cuối) trong tập ứng viên cuối để đúng "All-in bị chặn
/// max_front" — kết quả `front_in` KHÔNG BAO GIỜ vượt `max_front_wei` vì mọi
/// ứng viên đều được `clamp` trong `[0, max_front_wei]` ngay từ đầu. `None`
/// nếu `max_front_wei = 0` hoặc pool không mô phỏng được (reserve rác).
pub fn search_max_front_in(
    reserves: PoolReserves,
    victim_amount_in: U256,
    max_front_wei: U256,
    gas_wei: u128,
) -> Option<SandwichQuote> {
    if max_front_wei.is_zero() {
        return None;
    }
    let mut lo = U256::ZERO;
    let mut hi = max_front_wei;
    for _ in 0..256 {
        if hi <= lo + U256::from(1u64) {
            break;
        }
        let third = (hi - lo) / U256::from(3u64);
        let mid1 = lo + third;
        let mid2 = hi - third;
        let p1 = quote_at(reserves, mid1, victim_amount_in, gas_wei).map(|q| q.profit_wei).unwrap_or(i128::MIN);
        let p2 = quote_at(reserves, mid2, victim_amount_in, gas_wei).map(|q| q.profit_wei).unwrap_or(i128::MIN);
        if p1 < p2 {
            lo = mid1;
        } else {
            hi = mid2;
        }
    }

    let candidates = [lo, hi, max_front_wei];
    let mut best: Option<SandwichQuote> = None;
    for &c in &candidates {
        if let Some(q) = quote_at(reserves, c, victim_amount_in, gas_wei) {
            let better = match &best {
                None => true,
                Some(b) => q.profit_wei > b.profit_wei,
            };
            if better {
                best = Some(q);
            }
        }
    }
    best
}

/// `victim_would_revert` khi `amountOut` thật (sau khi bị front-run) thấp
/// hơn `amountOutMin` CHÍNH victim đặt trong calldata (decoder.rs) — khác
/// `min_swap_bnb` của `victims.txt` (đó là ngưỡng lọc vào pipeline, không
/// phải điều kiện revert on-chain).
pub fn victim_still_ok(victim_amount_out: U256, victim_amount_out_min: U256) -> bool {
    victim_amount_out >= victim_amount_out_min
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixture tay, số nguyên tròn để hand-verify không cần float:
    /// reserveIn=1000, reserveOut=1000, amountIn=100.
    /// amountInWithFee = 100*9975 = 997_500
    /// numerator = 997_500*1000 = 997_500_000
    /// denominator = 1000*10000 + 997_500 = 10_997_500
    /// amountOut = 997_500_000 / 10_997_500 = 90 (floor, dư 7_725_000)
    #[test]
    fn get_amount_out_exact_fixture_reserves() {
        let out = get_amount_out(U256::from(100u64), U256::from(1000u64), U256::from(1000u64)).unwrap();
        assert_eq!(out, U256::from(90u64));
    }

    #[test]
    fn get_amount_out_zero_input_or_reserve_is_none() {
        assert!(get_amount_out(U256::ZERO, U256::from(1000u64), U256::from(1000u64)).is_none());
        assert!(get_amount_out(U256::from(100u64), U256::ZERO, U256::from(1000u64)).is_none());
        assert!(get_amount_out(U256::from(100u64), U256::from(1000u64), U256::ZERO).is_none());
    }

    /// Nối tiếp fixture trên: front_in=100 (front_out=90, đã verify), rồi
    /// victim_in=50 trên reserve đã bị front đẩy (1100/910):
    /// amountInWithFee = 50*9975 = 498_750
    /// numerator = 498_750*910 = 453_862_500
    /// denominator = 1100*10000 + 498_750 = 11_498_750
    /// victimOut = 453_862_500 / 11_498_750 = 39 (floor)
    /// Rồi back: reserve sau victim = (1150, 871), bán lại front_out=90 token:
    /// amountInWithFee = 90*9975 = 897_750
    /// numerator = 897_750*1150 = 1_032_412_500
    /// denominator = 871*10000 + 897_750 = 9_607_750
    /// backOut = 1_032_412_500 / 9_607_750 = 107 (floor)
    /// profit (gas=0) = 107 - 100 = 7.
    #[test]
    fn hand_verified_full_sandwich_numbers() {
        let reserves = PoolReserves { reserve_wbnb: U256::from(1000u64), reserve_token: U256::from(1000u64) };
        let front_in = U256::from(100u64);
        let victim_in = U256::from(50u64);
        let (front_out, victim_out, after) = simulate_front_then_victim(reserves, front_in, victim_in).unwrap();
        assert_eq!(front_out, U256::from(90u64));
        assert_eq!(victim_out, U256::from(39u64));
        assert_eq!(after, PoolReserves { reserve_wbnb: U256::from(1150u64), reserve_token: U256::from(871u64) });

        let back_out = get_amount_out(front_out, after.reserve_token, after.reserve_wbnb).unwrap();
        assert_eq!(back_out, U256::from(107u64));
        assert_eq!(back_out - front_in, U256::from(7u64)); // profit voi gas=0
    }

    #[test]
    fn victim_still_ok_pass_and_revert() {
        assert!(victim_still_ok(U256::from(100u64), U256::from(90u64))); // du toi thieu -> pass
        assert!(victim_still_ok(U256::from(100u64), U256::from(100u64))); // bang toi thieu -> pass
        assert!(!victim_still_ok(U256::from(39u64), U256::from(40u64))); // it hon min -> revert
    }

    #[test]
    fn search_never_exceeds_max_front_bnb() {
        // Pool nong (1 WBNB), victim rat lon so pool (0.5 WBNB = 50% reserve)
        // nhung max_front bi khoa rat thap (0.1 WBNB) -> loi nhuan van con
        // dang tang trong suot khoang cho phep -> toi uu bi CHAN o bien tren
        // dung max_front_wei (khong tran ra ngoai) - dung "All-in bi chan
        // max_front" theo CLAUDE.md.
        let reserves = PoolReserves {
            reserve_wbnb: U256::from(1_000_000_000_000_000_000u128), // 1 WBNB
            reserve_token: U256::from(1_000_000u64) * U256::from(1_000_000_000_000_000_000u128),
        };
        let victim_in = U256::from(500_000_000_000_000_000u128); // 0.5 WBNB
        let max_front = U256::from(100_000_000_000_000_000u128); // 0.1 WBNB
        let quote = search_max_front_in(reserves, victim_in, max_front, 0).expect("phai co quote");
        assert!(quote.front_in <= max_front, "front_in khong duoc vuot max_front_bnb");
        assert_eq!(quote.front_in, max_front, "loi nhuan con tang toi bien -> phai bi chan dung tai max_front");
    }

    #[test]
    fn search_result_front_in_always_within_bounds_even_with_large_max() {
        let reserves = PoolReserves { reserve_wbnb: U256::from(1000u64), reserve_token: U256::from(1000u64) };
        let victim_in = U256::from(50u64);
        let max_front = U256::from(10_000u64); // lon hon ca reserve, van khong duoc vuot
        let quote = search_max_front_in(reserves, victim_in, max_front, 0).expect("phai co quote");
        assert!(quote.front_in <= max_front);
    }

}

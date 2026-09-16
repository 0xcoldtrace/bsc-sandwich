//! Cụm 3.1+3.2 — V2 sandwich math (constant product, phí 0.25% = 9975/10000
//! theo `DEX_REGISTRY.md`/AGENTS.md mục "Math") + "victim still ok". U256
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
/// AGENTS.md mục "Math".
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

/// Cụm `truth-victim-ok-and-memleak` (mục 2) — `front_in` LỚN NHẤT trong
/// `[0, max_front_wei]` mà victim VẪN SỐNG (`victim_out >= amount_out_min`).
///
/// `victim_out` giảm ĐƠN ĐIỆU theo `front_in`: front mua càng nhiều thì
/// `reserve_token` càng cạn và `reserve_quote` càng đầy, nên giá victim phải
/// trả chỉ có thể xấu đi. Tính đơn điệu đó là điều kiện đủ để nhị phân tìm
/// đúng biên, không cần quét tuyến tính.
///
/// Trả `None` khi ngay cả `front_in` nhỏ nhất có ý nghĩa cũng đã giết victim
/// (khi đó candidate là `victim_would_revert` thật sự, không cứu được).
pub fn max_front_in_victim_ok(
    reserves: PoolReserves,
    victim_amount_in: U256,
    max_front_wei: U256,
    victim_amount_out_min: U256,
) -> Option<U256> {
    let victim_ok_at = |front_in: U256| -> bool {
        match simulate_front_then_victim(reserves, front_in, victim_amount_in) {
            Some((_, victim_out, _)) => victim_still_ok(victim_out, victim_amount_out_min),
            // front_in = 0: khong co chan front, victim di mot minh.
            None if front_in.is_zero() => get_amount_out(victim_amount_in, reserves.reserve_wbnb, reserves.reserve_token)
                .map(|out| victim_still_ok(out, victim_amount_out_min))
                .unwrap_or(false),
            None => false,
        }
    };
    // Khong front gi ma victim da hong -> khong phai loi cua ta, khong cuu duoc.
    if !victim_ok_at(U256::ZERO) {
        return None;
    }
    if victim_ok_at(max_front_wei) {
        return Some(max_front_wei);
    }
    let mut lo = U256::ZERO; // luon victim_ok
    let mut hi = max_front_wei; // luon KHONG victim_ok
    for _ in 0..256 {
        if hi <= lo + U256::from(1u64) {
            break;
        }
        let mid = lo + (hi - lo) / U256::from(2u64);
        if victim_ok_at(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(lo)
}

/// Cụm `truth-victim-ok-and-memleak` (mục 2) — `search_max_front_in` CÓ RÀNG
/// BUỘC "victim phải sống".
///
/// # Vì sao cần (đo thật, không phải phòng xa)
///
/// `search_max_front_in` tối đa hoá `profit` mà KHÔNG biết gì về
/// `amountOutMin` của victim; `pipeline` chỉ kiểm `victim_still_ok` SAU ĐÓ như
/// một cổng nhị phân, nên mọi candidate mà mức `front_in` sinh lời nhất làm
/// victim revert đều bị VỨT BỎ (`victim_would_revert`) thay vì hạ `front_in`
/// xuống mức victim còn sống. Thí nghiệm `real_rpc_victim_ok_verdict_ladder`
/// (BAOCAO44 ô 5) đo trên 14 victim THẬT: `front_in = 0` thì **14/14** victim
/// sống, `front_in` theo V2-math thì **13/14** chết, 7 trong đó revert đúng
/// chữ `PancakeRouter: INSUFFICIENT_OUTPUT_AMOUNT` — tức chính chân front của
/// ta giết victim, không phải state fork sai như 1 trong 2 giả thuyết cũ ở
/// `docs/STATE.md` mục 5b.
///
/// Trả `None` khi không có mức `front_in` nào vừa hợp lệ vừa giữ victim sống.
pub fn search_max_front_in_victim_ok(
    reserves: PoolReserves,
    victim_amount_in: U256,
    max_front_wei: U256,
    gas_wei: u128,
    victim_amount_out_min: U256,
) -> Option<SandwichQuote> {
    let bound = max_front_in_victim_ok(reserves, victim_amount_in, max_front_wei, victim_amount_out_min)?;
    if bound.is_zero() {
        return None;
    }
    search_max_front_in(reserves, victim_amount_in, bound, gas_wei)
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
        // max_front" theo AGENTS.md.
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

    /// Cụm `truth-victim-ok-and-memleak` (mục 2) — hồi quy cho ĐÚNG hiện
    /// tượng đã đo thật: mức `front_in` tối đa hoá lãi giết victim, cổng cũ
    /// vứt bỏ candidate, cổng mới hạ `front_in` và VẪN có lãi.
    #[test]
    fn search_victim_ok_ha_front_in_thay_vi_vut_bo_candidate() {
        // Pool 1000/1000, victim mua 50 -> khong bi front thi nhan 47.
        let reserves = PoolReserves { reserve_wbnb: U256::from(1000u64), reserve_token: U256::from(1000u64) };
        let victim_in = U256::from(50u64);
        let max_front = U256::from(200u64);
        let no_front = get_amount_out(victim_in, reserves.reserve_wbnb, reserves.reserve_token).unwrap();
        // amountOutMin dat sat muc khong-bi-front (truot 5%) -> front lon giet victim.
        let amount_out_min = no_front * U256::from(95u64) / U256::from(100u64);

        let unconstrained = search_max_front_in(reserves, victim_in, max_front, 0).unwrap();
        assert!(
            !victim_still_ok(unconstrained.victim_out, amount_out_min),
            "fixture phai tai hien duoc hien tuong: front toi uu lai giet victim"
        );

        let constrained = search_max_front_in_victim_ok(reserves, victim_in, max_front, 0, amount_out_min).unwrap();
        assert!(victim_still_ok(constrained.victim_out, amount_out_min), "sau rang buoc victim PHAI song");
        assert!(constrained.front_in < unconstrained.front_in, "front_in phai bi ha xuong");
        assert!(constrained.profit_wei > 0, "van phai con lai, khong phai vut bo candidate");
    }

    /// Biên trả về phải là LỚN NHẤT còn giữ victim sống: thêm 1 đơn vị nữa là
    /// victim chết. Đây là thứ phân biệt "nhị phân đúng biên" với "đoán bừa
    /// một số nhỏ cho chắc" (đoán nhỏ thì mất lãi mà không ai biết).
    #[test]
    fn max_front_in_victim_ok_la_bien_that_su() {
        let reserves = PoolReserves { reserve_wbnb: U256::from(1_000_000u64), reserve_token: U256::from(1_000_000u64) };
        let victim_in = U256::from(50_000u64);
        let max_front = U256::from(500_000u64);
        let no_front = get_amount_out(victim_in, reserves.reserve_wbnb, reserves.reserve_token).unwrap();
        let amount_out_min = no_front * U256::from(90u64) / U256::from(100u64);

        let bound = max_front_in_victim_ok(reserves, victim_in, max_front, amount_out_min).unwrap();
        let at_bound = simulate_front_then_victim(reserves, bound, victim_in).unwrap().1;
        assert!(victim_still_ok(at_bound, amount_out_min), "tai bien victim phai con song");
        let over = simulate_front_then_victim(reserves, bound + U256::from(1u64), victim_in).unwrap().1;
        assert!(!victim_still_ok(over, amount_out_min), "vuot bien 1 don vi la victim phai chet");
    }

    /// `amountOutMin` cao hơn cả mức victim nhận được khi KHÔNG bị front-run
    /// (victim tự đặt điều kiện không thể thoả) -> `None`, và pipeline phải
    /// đọc thành `victim_would_revert` THẬT, không phải lỗi của ta.
    #[test]
    fn victim_tu_hong_thi_khong_co_bien_nao() {
        let reserves = PoolReserves { reserve_wbnb: U256::from(1000u64), reserve_token: U256::from(1000u64) };
        let victim_in = U256::from(50u64);
        let no_front = get_amount_out(victim_in, reserves.reserve_wbnb, reserves.reserve_token).unwrap();
        let impossible = no_front + U256::from(1u64);
        assert!(max_front_in_victim_ok(reserves, victim_in, U256::from(200u64), impossible).is_none());
        assert!(search_max_front_in_victim_ok(reserves, victim_in, U256::from(200u64), 0, impossible).is_none());
    }

    /// `amountOutMin = 0` (rất phổ biến trên BSC) -> ràng buộc không cắt gì,
    /// kết quả PHẢI y hệt search cũ. Nếu không, cụm này đã âm thầm đổi hành vi
    /// của phần lớn candidate.
    #[test]
    fn amount_out_min_bang_0_thi_khong_doi_gi_so_voi_search_cu() {
        let reserves = PoolReserves { reserve_wbnb: U256::from(1_000_000u64), reserve_token: U256::from(5_000_000u64) };
        let victim_in = U256::from(30_000u64);
        let max_front = U256::from(100_000u64);
        let old = search_max_front_in(reserves, victim_in, max_front, 0).unwrap();
        let new = search_max_front_in_victim_ok(reserves, victim_in, max_front, 0, U256::ZERO).unwrap();
        assert_eq!(old.front_in, new.front_in);
        assert_eq!(old.profit_wei, new.profit_wei);
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

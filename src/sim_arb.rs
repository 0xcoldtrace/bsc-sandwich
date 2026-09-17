//! Cụm `planB-backrun-opportunity` (mục 3) — SIM BACKRUN-ARB trên 2 venue
//! Pancake V2 (quote WBNB và/hoặc USDT).
//!
//! # Khác sandwich ở chỗ nào (và vì sao đổi)
//!
//! Sandwich cần đứng TRƯỚC victim (`front`), cần `victim_ok`, cần vốn xoay
//! thật, và theo BAOCAO44/45 thì lãi ngoài cụm đối thủ ≈ 0. Backrun-arb đứng
//! NGAY SAU victim: victim đã tự tay làm lệch giá giữa 2 pool, ta chỉ đi 1
//! vòng khép kín (vay → mua pool rẻ → bán pool đắt → trả nợ) trong ĐÚNG 1 tx.
//! Không cần `victim_ok` (ta không đụng vào giao dịch của họ), không cần vốn
//! (flash loan), và nếu không lãi thì hợp đồng revert nên mất đúng tiền gas.
//!
//! # Hình dạng route
//!
//! ```text
//!   vay  borrow (quote_borrow)
//!     -> pool MUA  : quote_borrow -> token        (get_amount_out)
//!     -> pool BÁN  : token        -> quote_sell   (get_amount_out)
//!     -> [bridge]  : quote_sell   -> quote_borrow (chỉ khi 2 pool khác quote)
//!   trả  borrow + phí flash
//! ```
//!
//! Chân `bridge` là pool V2 WBNB/USDT THẬT (reserve đọc on-chain tại cùng
//! block) — KHÔNG price oracle, KHÔNG quy đổi bằng tỉ giá bịa, đúng luật
//! AGENTS.md mục Math về `profit_usdt`.
//!
//! # Vì sao ternary search là đủ
//!
//! `final_out(borrow)` là hợp của 2–3 hàm `get_amount_out` (đều lõm, tăng
//! dần), nên `net(borrow) = final_out(borrow) − borrow − phí` lõm theo
//! `borrow`. Ternary search trên miền số nguyên hội tụ về vùng tối ưu — cùng
//! lập luận đã dùng ở `sim_v2::search_max_front_in`.
//!
//! Cụm `planB-B5-simarb-v3-measure` — thêm chân V3 (PCS QuoterV2 + Uniswap
//! QuoterV2): fit virtual CPMM từ ≤2 quote / pool, search closed-form (0
//! quote thêm), tổng ≤5 lời gọi quoter mỗi route. Route: V2↔V3, V3 tier A↔
//! V3 tier B (cùng quote). Infinity vẫn chưa sim.
//!
//! Cụm `planB-B6-cap-borrow-v3` — search MỌI route (V2↔V2, V2↔V3, V3↔V3,
//! mixed quote) nằm TRONG trần `max_borrow` đúng quote chân vay. Trước mỗi
//! bước tăng size: `clamp_borrow`. Kết thúc: `best.borrow > max_borrow` bị
//! loại, không trả quote đó. Không nhân borrow ×4.

use alloy::primitives::{Address, U256};

use crate::flash::{FlashPick, FlashSnapshot, FlashSource};
use crate::sim_v2::get_amount_out;

/// Một pool V2 đã sắp đúng chiều theo quote asset của nó.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArbPool {
    pub pair: Address,
    pub quote: Address,
    pub reserve_quote: U256,
    pub reserve_token: U256,
}

/// Chân quy đổi `quote_sell -> quote_borrow` (pool WBNB/USDT thật).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeLeg {
    pub pair: Address,
    /// Reserve của token ĐI VÀO chân bridge (= `quote_sell`).
    pub reserve_in: U256,
    /// Reserve của token ĐI RA (= `quote_borrow`).
    pub reserve_out: U256,
}

/// Một hướng arb cụ thể (mua ở đâu, bán ở đâu). Hai hướng của cùng 1 cặp pool
/// là 2 `ArbRoute` khác nhau — `best_arb_for_pools` thử cả hai.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArbRoute {
    pub token: Address,
    /// Quote ta VAY và TRẢ (= quote của pool mua).
    pub borrow_quote: Address,
    pub buy: ArbPool,
    pub sell: ArbPool,
    /// `None` khi 2 pool cùng quote (chưa xảy ra với V2 vì mỗi cặp chỉ 1 pool,
    /// nhưng cấu trúc để sẵn cho venue V3/Infinity ở cụm sau).
    pub bridge: Option<BridgeLeg>,
}

/// Kết quả sim 1 mức `borrow` cụ thể. Mọi field tiền tính bằng ĐƠN VỊ CỦA
/// `borrow_quote` (wei WBNB hoặc wei USDT — USDT trên BSC cũng 18 decimals).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArbQuote {
    pub borrow: U256,
    pub token_out: U256,
    /// Số `quote_sell` nhận được ở chân bán (TRƯỚC bridge).
    pub quote_sell_out: U256,
    /// Số `borrow_quote` cầm về sau toàn bộ route (SAU bridge nếu có).
    pub final_out: U256,
    pub flash_source: FlashSource,
    pub flash_fee_bps: u32,
    pub flash_fee_wei: U256,
    pub gas_wei: u128,
    pub bribe_wei: u128,
    /// `final_out − borrow − flash_fee` (chưa trừ gas/bribe) — có thể âm.
    pub gross_wei: i128,
    /// `gross − gas` — cái mà bribe được tính % trên đó.
    pub profit_before_bribe_wei: i128,
    /// `gross − gas − bribe` — SỐ QUYẾT ĐỊNH.
    pub net_wei: i128,
}

fn u256_to_i128(v: U256) -> Option<i128> {
    let as_u128: u128 = v.try_into().ok()?;
    i128::try_from(as_u128).ok()
}

/// Cụm `planB-B6-cap-borrow-v3` — kẹp size vào trần vay. Mọi bước tăng size
/// (ternary mid, lưới 5 điểm, candidate cuối) PHẢI đi qua đây. `size > max`
/// → `max`; `max = 0` → 0. Không nhân, không nới.
#[inline]
pub fn clamp_borrow(size: U256, max_borrow: U256) -> U256 {
    size.min(max_borrow)
}

fn accept_quote(q: ArbQuote, max_borrow: U256) -> Option<ArbQuote> {
    if q.borrow > max_borrow {
        None
    } else {
        Some(q)
    }
}

/// Ternary search `borrow` trên `[0, min(max_borrow, available)]`. Mọi điểm
/// thử đã clamp; kết quả `borrow > max_borrow` bị loại (không Simulated).
fn search_borrow_capped(
    max_borrow: U256,
    available: U256,
    quote_fn: impl Fn(U256) -> Option<ArbQuote>,
) -> Option<ArbQuote> {
    let cap = clamp_borrow(available, max_borrow);
    if cap.is_zero() {
        return None;
    }
    let at = |raw: U256| -> Option<ArbQuote> {
        let b = clamp_borrow(raw, cap);
        if b.is_zero() {
            return None;
        }
        quote_fn(b).and_then(|q| accept_quote(q, max_borrow))
    };
    let net_at = |raw: U256| -> i128 { at(raw).map(|q| q.net_wei).unwrap_or(i128::MIN) };

    let mut lo = U256::ZERO;
    let mut hi = cap;
    for _ in 0..256 {
        if hi <= lo + U256::from(1u64) {
            break;
        }
        let third = (hi - lo) / U256::from(3u64);
        let mid1 = clamp_borrow(lo + third, cap);
        let mid2 = clamp_borrow(hi - third, cap);
        if net_at(mid1) < net_at(mid2) {
            lo = mid1;
        } else {
            hi = mid2;
        }
    }

    let mut best: Option<ArbQuote> = None;
    for c in [lo, hi, cap] {
        let Some(q) = at(c) else {
            continue;
        };
        if best.as_ref().map(|b| q.net_wei > b.net_wei).unwrap_or(true) {
            best = Some(q);
        }
    }
    best.and_then(|q| accept_quote(q, max_borrow))
}

/// Chạy route với 1 mức `borrow`, KHÔNG trừ gas/bribe/flash — dùng cho test
/// và cho `quote_at`.
pub fn route_out(route: &ArbRoute, borrow: U256) -> Option<(U256, U256, U256)> {
    let token_out = get_amount_out(borrow, route.buy.reserve_quote, route.buy.reserve_token)?;
    let quote_sell_out = get_amount_out(token_out, route.sell.reserve_token, route.sell.reserve_quote)?;
    let final_out = match route.bridge {
        None => quote_sell_out,
        Some(b) => get_amount_out(quote_sell_out, b.reserve_in, b.reserve_out)?,
    };
    Some((token_out, quote_sell_out, final_out))
}

/// Bribe = `pct` % của lãi TRƯỚC bribe, kẹp `[min,max]` (max = 0 nghĩa không
/// trần). Lãi ≤ 0 → bribe 0 (không bao giờ hối lộ khi đang lỗ).
///
/// Giữ ĐÚNG khuôn `pipeline::compute_bribe_wei` (cùng ngữ nghĩa, cùng cách
/// kẹp) nhưng nhận `i128` để không phải ép kiểu ở mọi chỗ gọi — test
/// `bribe_khop_pipeline` đối chiếu 2 hàm trên cùng input.
pub fn arb_bribe_wei(profit_before_bribe_wei: i128, pct: f64, clamp_bnb: Option<(u128, u128)>) -> u128 {
    if profit_before_bribe_wei <= 0 {
        return 0;
    }
    crate::pipeline::compute_bribe_wei(profit_before_bribe_wei as u128, pct, clamp_bnb)
}

fn quote_at(
    route: &ArbRoute,
    borrow: U256,
    pick: FlashPick,
    gas_wei: u128,
    bribe_pct: f64,
    bribe_clamp: Option<(u128, u128)>,
) -> Option<ArbQuote> {
    let (token_out, quote_sell_out, final_out) = route_out(route, borrow)?;
    let fee_wei = crate::flash::flash_fee_wei(pick.source, borrow, pick.fee_bps)?;
    let final_i = u256_to_i128(final_out)?;
    let borrow_i = u256_to_i128(borrow)?;
    let fee_i = u256_to_i128(fee_wei)?;
    let gross_wei = final_i - borrow_i - fee_i;
    let profit_before_bribe_wei = gross_wei - gas_wei as i128;
    let bribe_wei = arb_bribe_wei(profit_before_bribe_wei, bribe_pct, bribe_clamp);
    let net_wei = profit_before_bribe_wei - bribe_wei as i128;
    Some(ArbQuote {
        borrow,
        token_out,
        quote_sell_out,
        final_out,
        flash_source: pick.source,
        flash_fee_bps: pick.fee_bps,
        flash_fee_wei: fee_wei,
        gas_wei,
        bribe_wei,
        gross_wei,
        profit_before_bribe_wei,
        net_wei,
    })
}

/// Ternary search `borrow` trong `[0, max_borrow]` tối đa hoá `net_wei` với
/// MỘT nguồn flash cố định. `None` khi `max_borrow = 0` hoặc route không chạy
/// được (reserve rác).
pub fn search_borrow_for_source(
    route: &ArbRoute,
    max_borrow: U256,
    source: FlashSource,
    fee_bps: u32,
    available: U256,
    gas_wei: u128,
    bribe_pct: f64,
    bribe_clamp: Option<(u128, u128)>,
) -> Option<ArbQuote> {
    search_borrow_capped(max_borrow, available, |b| {
        let fee_wei = crate::flash::flash_fee_wei(source, b, fee_bps)?;
        let pick = FlashPick { source, fee_bps, fee_wei, available };
        quote_at(route, b, pick, gas_wei, bribe_pct, bribe_clamp)
    })
}

/// Thử MỌI nguồn flash đủ sâu cho `route.borrow_quote` rồi trả kết quả có
/// `net_wei` lớn nhất.
///
/// Phải thử từng nguồn RIÊNG (không chọn nguồn trước rồi mới search) vì phí
/// khác nhau đổi CẢ cỡ vay tối ưu, không chỉ đổi một hằng số trừ đi cuối.
///
/// `v2_flash_depth` = reserve quote của chính pool sẽ arb (cho phép dùng
/// `pancakeCall`); `None` = không cho phép nguồn đó ở route này.
#[allow(clippy::too_many_arguments)]
pub fn search_best_arb(
    route: &ArbRoute,
    max_borrow: U256,
    snapshot: &FlashSnapshot,
    v2_flash_depth: Option<U256>,
    gas_wei: u128,
    bribe_pct: f64,
    bribe_clamp: Option<(u128, u128)>,
) -> Option<ArbQuote> {
    let mut best: Option<ArbQuote> = None;
    for source in FlashSource::all() {
        let (fee_bps, available) = match source {
            FlashSource::PancakeV2FlashSwap => match v2_flash_depth {
                Some(d) => (crate::flash::PANCAKE_V2_FLASH_FEE_BPS, d),
                None => continue,
            },
            _ => {
                let st = match snapshot.state(source) {
                    Some(s) => s,
                    None => continue,
                };
                let fee = match st.fee_bps {
                    Some(f) => f,
                    None => continue,
                };
                match st.available_for(route.borrow_quote) {
                    Some(a) if !a.is_zero() => (fee, a),
                    _ => continue,
                }
            }
        };
        if let Some(q) =
            search_borrow_for_source(route, max_borrow, source, fee_bps, available, gas_wei, bribe_pct, bribe_clamp)
        {
            if q.borrow > max_borrow {
                continue;
            }
            if best.as_ref().map(|b| q.net_wei > b.net_wei).unwrap_or(true) {
                best = Some(q);
            }
        }
    }
    best.and_then(|q| accept_quote(q, max_borrow))
}

/// Dựng 2 hướng arb có thể có từ 2 pool của CÙNG token, rồi trả hướng lãi
/// nhất. `bridge_wbnb_usdt` là pool WBNB/USDT thật — bắt buộc khi 2 pool khác
/// quote; thiếu thì hướng đó bị bỏ (KHÔNG quy đổi bằng tỉ giá bịa).
///
/// `bridge_reserve_wbnb`/`bridge_reserve_usdt` lấy từ chính pool
/// WBNB/USDT tại cùng block.
#[allow(clippy::too_many_arguments)]
pub fn best_arb_for_pools(
    token: Address,
    pool_a: ArbPool,
    pool_b: ArbPool,
    bridge_pair: Option<Address>,
    bridge_reserve_wbnb: U256,
    bridge_reserve_usdt: U256,
    wbnb: Address,
    max_borrow_for_quote: &dyn Fn(Address) -> U256,
    snapshot: &FlashSnapshot,
    gas_wei_for_quote: &dyn Fn(Address) -> u128,
    bribe_pct: f64,
    bribe_clamp: Option<(u128, u128)>,
) -> Option<(ArbRoute, ArbQuote)> {
    let mut best: Option<(ArbRoute, ArbQuote)> = None;
    for (buy, sell) in [(pool_a, pool_b), (pool_b, pool_a)] {
        let bridge = if buy.quote == sell.quote {
            None
        } else {
            let pair = bridge_pair?;
            if bridge_reserve_wbnb.is_zero() || bridge_reserve_usdt.is_zero() {
                continue;
            }
            // Chân bridge đổi `sell.quote` -> `buy.quote`.
            let (reserve_in, reserve_out) = if sell.quote == wbnb {
                (bridge_reserve_wbnb, bridge_reserve_usdt)
            } else {
                (bridge_reserve_usdt, bridge_reserve_wbnb)
            };
            Some(BridgeLeg { pair, reserve_in, reserve_out })
        };
        let route = ArbRoute { token, borrow_quote: buy.quote, buy, sell, bridge };
        let max_borrow = max_borrow_for_quote(buy.quote);
        let gas_wei = gas_wei_for_quote(buy.quote);
        // Pancake V2 flash swap: vay quote từ CHÍNH pool mua -> chiều sâu là
        // reserve quote của pool đó.
        let v2_depth = Some(buy.reserve_quote);
        if let Some(q) = search_best_arb(&route, max_borrow, snapshot, v2_depth, gas_wei, bribe_pct, bribe_clamp) {
            if best.as_ref().map(|(_, b)| q.net_wei > b.net_wei).unwrap_or(true) {
                best = Some((route, q));
            }
        }
    }
    best
}

/// Áp giao dịch của victim lên ĐÚNG pool họ đụng — trả reserve SAU victim.
/// `victim_buys_token = true` nghĩa victim đưa quote vào lấy token ra.
pub fn apply_victim_to_pool(pool: ArbPool, victim_amount_in: U256, victim_buys_token: bool) -> Option<ArbPool> {
    if victim_buys_token {
        let out = get_amount_out(victim_amount_in, pool.reserve_quote, pool.reserve_token)?;
        Some(ArbPool {
            reserve_quote: pool.reserve_quote.checked_add(victim_amount_in)?,
            reserve_token: pool.reserve_token.checked_sub(out)?,
            ..pool
        })
    } else {
        let out = get_amount_out(victim_amount_in, pool.reserve_token, pool.reserve_quote)?;
        Some(ArbPool {
            reserve_token: pool.reserve_token.checked_add(victim_amount_in)?,
            reserve_quote: pool.reserve_quote.checked_sub(out)?,
            ..pool
        })
    }
}

/// Cùng tinh thần `pipeline::sanity_check` (cụm `bugfix-presign-and-contract-plan`
/// A2): kết quả sim vi phạm trần vô lý thì KHÔNG tin, đánh dấu để loại khỏi
/// thống kê thay vì báo lãi khủng do reserve rác/decode sai.
///
/// 3 trần: `borrow > 50 %` reserve quote của pool mua, `net > 5 %` reserve
/// quote pool mua, hoặc `token_out > 50 %` reserve token pool mua. Ngưỡng
/// NỚI hơn sandwich (10 %/2 %) vì arb hợp lệ có thể vay lớn so với pool nhỏ —
/// đây là lưới chặn số vô lý, không phải cổng kinh tế.
pub fn arb_sanity_ok(route: &ArbRoute, q: &ArbQuote) -> bool {
    let r = route.buy.reserve_quote;
    if r.is_zero() {
        return false;
    }
    if q.borrow > r / U256::from(2u64) {
        return false;
    }
    if q.token_out > route.buy.reserve_token / U256::from(2u64) {
        return false;
    }
    if q.net_wei > 0 {
        let net_u = U256::from(q.net_wei as u128);
        if net_u > r / U256::from(20u64) {
            return false;
        }
    }
    true
}

/// Trần lời gọi quoter mỗi lần search 1 route (lệnh B5). Fit 2 điểm + (tuỳ)
/// 1 verify, hoặc 5 điểm rời — không vượt.
pub const QUOTER_SEARCH_BUDGET: u32 = 5;

/// PCS V3 hoặc Uniswap V3 BSC (cùng công thức fee 1e6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum V3Family {
    Pcs,
    Uni,
}

impl V3Family {
    pub fn as_str(self) -> &'static str {
        match self {
            V3Family::Pcs => "pcs_v3",
            V3Family::Uni => "uni_v3",
        }
    }
}

/// Pool V3 đã (hoặc chưa) fit virtual reserve. `reserve_*` = 0 nghĩa chưa fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArbV3Pool {
    pub pool: Address,
    pub quote: Address,
    pub fee: u32,
    pub family: V3Family,
    pub reserve_quote: U256,
    pub reserve_token: U256,
    pub ok: bool,
}

/// Một chân arb: V2 pair hoặc V3 pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArbVenue {
    V2(ArbPool),
    V3(ArbV3Pool),
}

impl ArbVenue {
    pub fn id(self) -> Address {
        match self {
            ArbVenue::V2(p) => p.pair,
            ArbVenue::V3(p) => p.pool,
        }
    }
    pub fn quote(self) -> Address {
        match self {
            ArbVenue::V2(p) => p.quote,
            ArbVenue::V3(p) => p.quote,
        }
    }
    pub fn reserve_quote(self) -> U256 {
        match self {
            ArbVenue::V2(p) => p.reserve_quote,
            ArbVenue::V3(p) => p.reserve_quote,
        }
    }
    pub fn reserve_token(self) -> U256 {
        match self {
            ArbVenue::V2(p) => p.reserve_token,
            ArbVenue::V3(p) => p.reserve_token,
        }
    }
    pub fn kind(self) -> &'static str {
        match self {
            ArbVenue::V2(_) => "v2",
            ArbVenue::V3(p) => p.family.as_str(),
        }
    }
    pub fn fee(self) -> Option<u32> {
        match self {
            ArbVenue::V2(_) => None,
            ArbVenue::V3(p) => Some(p.fee),
        }
    }
    pub fn is_fitted(self) -> bool {
        !self.reserve_quote().is_zero() && !self.reserve_token().is_zero()
    }
}

pub fn route_kind(buy: ArbVenue, sell: ArbVenue) -> &'static str {
    match (buy, sell) {
        (ArbVenue::V2(_), ArbVenue::V2(_)) => "v2_v2",
        (ArbVenue::V2(_), ArbVenue::V3(_)) => "v2_v3",
        (ArbVenue::V3(_), ArbVenue::V2(_)) => "v3_v2",
        (ArbVenue::V3(_), ArbVenue::V3(_)) => "v3_v3",
    }
}

/// Route hỗn hợp V2/V3. V2-only `ArbRoute` vẫn dùng cho test cũ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixedRoute {
    pub token: Address,
    pub borrow_quote: Address,
    pub buy: ArbVenue,
    pub sell: ArbVenue,
    pub bridge: Option<BridgeLeg>,
}

impl From<ArbRoute> for MixedRoute {
    fn from(r: ArbRoute) -> Self {
        Self {
            token: r.token,
            borrow_quote: r.borrow_quote,
            buy: ArbVenue::V2(r.buy),
            sell: ArbVenue::V2(r.sell),
            bridge: r.bridge,
        }
    }
}

/// V3 CPMM trong 1 tick range: fee đơn vị 1e6 (100=0.01%, 2500=0.25%).
/// fee=2500 khớp đúng `get_amount_out` V2 (0.25%).
pub fn get_amount_out_v3(amount_in: U256, reserve_in: U256, reserve_out: U256, fee: u32) -> Option<U256> {
    if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
        return None;
    }
    let fee = fee.min(1_000_000);
    let n = U256::from(1_000_000u64 - u64::from(fee));
    let d = U256::from(1_000_000u64);
    let amount_in_with_fee = amount_in.checked_mul(n)?;
    let numerator = amount_in_with_fee.checked_mul(reserve_out)?;
    let denominator = reserve_in.checked_mul(d)?.checked_add(amount_in_with_fee)?;
    if denominator.is_zero() {
        return None;
    }
    Some(numerator / denominator)
}

/// Fit virtual (R_in, R_out) từ 2 quote cùng chiều. Trả None khi overflow /
/// 2 điểm thẳng / không lõm.
pub fn fit_v3_virtual_reserves(fee: u32, a1: U256, o1: U256, a2: U256, o2: U256) -> Option<(U256, U256)> {
    if a1.is_zero() || a2.is_zero() || o1.is_zero() || o2.is_zero() || a1 == a2 {
        return None;
    }
    let fee = fee.min(1_000_000);
    let n = U256::from(1_000_000u64 - u64::from(fee));
    let d = U256::from(1_000_000u64);
    let (a_lo, o_lo, a_hi, o_hi) = if a2 > a1 { (a1, o1, a2, o2) } else { (a2, o2, a1, o1) };
    let o_diff = o_hi.checked_sub(o_lo)?;
    let left = o_lo.checked_mul(a_hi)?;
    let right = o_hi.checked_mul(a_lo)?;
    let denom_core = left.checked_sub(right)?;
    if denom_core.is_zero() {
        return None;
    }
    let numer = a_lo.checked_mul(a_hi)?.checked_mul(n)?.checked_mul(o_diff)?;
    let denom = d.checked_mul(denom_core)?;
    let rin = numer.checked_div(denom)?;
    if rin.is_zero() {
        return None;
    }
    let rout_num = o_lo
        .checked_mul(d)?
        .checked_mul(rin)?
        .checked_add(o_lo.checked_mul(a_lo)?.checked_mul(n)?)?;
    let rout_den = a_lo.checked_mul(n)?;
    let rout = rout_num.checked_div(rout_den)?;
    if rout.is_zero() {
        return None;
    }
    Some((rin, rout))
}

pub fn apply_victim_to_v3(pool: ArbV3Pool, victim_amount_in: U256, victim_buys_token: bool) -> Option<ArbV3Pool> {
    if victim_buys_token {
        let out = get_amount_out_v3(victim_amount_in, pool.reserve_quote, pool.reserve_token, pool.fee)?;
        Some(ArbV3Pool {
            reserve_quote: pool.reserve_quote.checked_add(victim_amount_in)?,
            reserve_token: pool.reserve_token.checked_sub(out)?,
            ..pool
        })
    } else {
        let out = get_amount_out_v3(victim_amount_in, pool.reserve_token, pool.reserve_quote, pool.fee)?;
        Some(ArbV3Pool {
            reserve_token: pool.reserve_token.checked_add(victim_amount_in)?,
            reserve_quote: pool.reserve_quote.checked_sub(out)?,
            ..pool
        })
    }
}

pub fn apply_victim_to_venue(v: ArbVenue, amount_in: U256, buys_token: bool) -> Option<ArbVenue> {
    match v {
        ArbVenue::V2(p) => apply_victim_to_pool(p, amount_in, buys_token).map(ArbVenue::V2),
        ArbVenue::V3(p) => apply_victim_to_v3(p, amount_in, buys_token).map(ArbVenue::V3),
    }
}

fn hop_out(venue: ArbVenue, amount_in: U256, token_in_is_quote: bool) -> Option<U256> {
    match venue {
        ArbVenue::V2(p) => {
            if token_in_is_quote {
                get_amount_out(amount_in, p.reserve_quote, p.reserve_token)
            } else {
                get_amount_out(amount_in, p.reserve_token, p.reserve_quote)
            }
        }
        ArbVenue::V3(p) => {
            if token_in_is_quote {
                get_amount_out_v3(amount_in, p.reserve_quote, p.reserve_token, p.fee)
            } else {
                get_amount_out_v3(amount_in, p.reserve_token, p.reserve_quote, p.fee)
            }
        }
    }
}

/// Chạy mixed route với virtual reserve đã fit (0 RPC).
pub fn route_out_mixed(route: &MixedRoute, borrow: U256) -> Option<(U256, U256, U256)> {
    if !route.buy.is_fitted() || !route.sell.is_fitted() {
        return None;
    }
    let token_out = hop_out(route.buy, borrow, true)?;
    let quote_sell_out = hop_out(route.sell, token_out, false)?;
    let final_out = match route.bridge {
        None => quote_sell_out,
        Some(b) => get_amount_out(quote_sell_out, b.reserve_in, b.reserve_out)?,
    };
    Some((token_out, quote_sell_out, final_out))
}

fn quote_at_mixed(
    route: &MixedRoute,
    borrow: U256,
    pick: FlashPick,
    gas_wei: u128,
    bribe_pct: f64,
    bribe_clamp: Option<(u128, u128)>,
) -> Option<ArbQuote> {
    let (token_out, quote_sell_out, final_out) = route_out_mixed(route, borrow)?;
    let fee_wei = crate::flash::flash_fee_wei(pick.source, borrow, pick.fee_bps)?;
    let final_i = u256_to_i128(final_out)?;
    let borrow_i = u256_to_i128(borrow)?;
    let fee_i = u256_to_i128(fee_wei)?;
    let gross_wei = final_i - borrow_i - fee_i;
    let profit_before_bribe_wei = gross_wei - gas_wei as i128;
    let bribe_wei = arb_bribe_wei(profit_before_bribe_wei, bribe_pct, bribe_clamp);
    let net_wei = profit_before_bribe_wei - bribe_wei as i128;
    Some(ArbQuote {
        borrow,
        token_out,
        quote_sell_out,
        final_out,
        flash_source: pick.source,
        flash_fee_bps: pick.fee_bps,
        flash_fee_wei: fee_wei,
        gas_wei,
        bribe_wei,
        gross_wei,
        profit_before_bribe_wei,
        net_wei,
    })
}

pub fn search_borrow_mixed_for_source(
    route: &MixedRoute,
    max_borrow: U256,
    source: FlashSource,
    fee_bps: u32,
    available: U256,
    gas_wei: u128,
    bribe_pct: f64,
    bribe_clamp: Option<(u128, u128)>,
) -> Option<ArbQuote> {
    search_borrow_capped(max_borrow, available, |b| {
        let fee_wei = crate::flash::flash_fee_wei(source, b, fee_bps)?;
        let pick = FlashPick { source, fee_bps, fee_wei, available };
        quote_at_mixed(route, b, pick, gas_wei, bribe_pct, bribe_clamp)
    })
}

#[allow(clippy::too_many_arguments)]
pub fn search_best_arb_mixed(
    route: &MixedRoute,
    max_borrow: U256,
    snapshot: &FlashSnapshot,
    v2_flash_depth: Option<U256>,
    gas_wei: u128,
    bribe_pct: f64,
    bribe_clamp: Option<(u128, u128)>,
) -> Option<ArbQuote> {
    let mut best: Option<ArbQuote> = None;
    for source in FlashSource::all() {
        let (fee_bps, available) = match source {
            FlashSource::PancakeV2FlashSwap => match v2_flash_depth {
                Some(d) => (crate::flash::PANCAKE_V2_FLASH_FEE_BPS, d),
                None => continue,
            },
            _ => {
                let st = match snapshot.state(source) {
                    Some(s) => s,
                    None => continue,
                };
                let fee = match st.fee_bps {
                    Some(f) => f,
                    None => continue,
                };
                match st.available_for(route.borrow_quote) {
                    Some(a) if !a.is_zero() => (fee, a),
                    _ => continue,
                }
            }
        };
        if let Some(q) =
            search_borrow_mixed_for_source(route, max_borrow, source, fee_bps, available, gas_wei, bribe_pct, bribe_clamp)
        {
            if q.borrow > max_borrow {
                continue;
            }
            if best.as_ref().map(|b| q.net_wei > b.net_wei).unwrap_or(true) {
                best = Some(q);
            }
        }
    }
    best.and_then(|q| accept_quote(q, max_borrow))
}

pub fn arb_sanity_ok_mixed(route: &MixedRoute, q: &ArbQuote) -> bool {
    let r = route.buy.reserve_quote();
    if r.is_zero() {
        return false;
    }
    if q.borrow > r / U256::from(2u64) {
        return false;
    }
    if q.token_out > route.buy.reserve_token() / U256::from(2u64) {
        return false;
    }
    if q.net_wei > 0 {
        let net_u = U256::from(q.net_wei as u128);
        if net_u > r / U256::from(20u64) {
            return false;
        }
    }
    true
}

fn make_bridge(
    buy: ArbVenue,
    sell: ArbVenue,
    bridge_pair: Option<Address>,
    bridge_reserve_wbnb: U256,
    bridge_reserve_usdt: U256,
    wbnb: Address,
) -> Option<Option<BridgeLeg>> {
    if buy.quote() == sell.quote() {
        return Some(None);
    }
    let pair = bridge_pair?;
    if bridge_reserve_wbnb.is_zero() || bridge_reserve_usdt.is_zero() {
        return None;
    }
    let (reserve_in, reserve_out) = if sell.quote() == wbnb {
        (bridge_reserve_wbnb, bridge_reserve_usdt)
    } else {
        (bridge_reserve_usdt, bridge_reserve_wbnb)
    };
    Some(Some(BridgeLeg { pair, reserve_in, reserve_out }))
}

/// Thử mọi cặp venue (V2↔V3, V3↔V3, V2↔V2), cùng quote ưu tiên; khác quote
/// cần bridge. Victim đã được `apply_victim_to_venue` sẵn trên đúng chân.
#[allow(clippy::too_many_arguments)]
pub fn best_arb_for_venues(
    token: Address,
    venues: &[ArbVenue],
    bridge_pair: Option<Address>,
    bridge_reserve_wbnb: U256,
    bridge_reserve_usdt: U256,
    wbnb: Address,
    max_borrow_for_quote: &dyn Fn(Address) -> U256,
    snapshot: &FlashSnapshot,
    gas_wei_for_quote: &dyn Fn(Address) -> u128,
    bribe_pct: f64,
    bribe_clamp: Option<(u128, u128)>,
) -> Option<(MixedRoute, ArbQuote)> {
    let n = venues.len();
    if n < 2 {
        return None;
    }
    let mut best: Option<(MixedRoute, ArbQuote)> = None;
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }
            let buy = venues[i];
            let sell = venues[j];
            if !buy.is_fitted() || !sell.is_fitted() {
                continue;
            }
            let Some(bridge) = make_bridge(buy, sell, bridge_pair, bridge_reserve_wbnb, bridge_reserve_usdt, wbnb) else {
                continue;
            };
            let route = MixedRoute { token, borrow_quote: buy.quote(), buy, sell, bridge };
            let max_borrow = max_borrow_for_quote(buy.quote());
            let gas_wei = gas_wei_for_quote(buy.quote());
            let v2_depth = match buy {
                ArbVenue::V2(p) => Some(p.reserve_quote),
                ArbVenue::V3(_) => None,
            };
            if let Some(q) = search_best_arb_mixed(&route, max_borrow, snapshot, v2_depth, gas_wei, bribe_pct, bribe_clamp)
            {
                if best.as_ref().map(|(_, b)| q.net_wei > b.net_wei).unwrap_or(true) {
                    best = Some((route, q));
                }
            }
        }
    }
    best
}

/// 5 mức vay hình học trong `(0, max]` — dùng khi fit thất bại, mỗi mức 1
/// quote / chân V3 (V2→V3 = 5 lời gọi, đúng trần).
pub fn five_borrow_grid(max_borrow: U256) -> [U256; 5] {
    let sixteen = U256::from(16u64);
    let eight = U256::from(8u64);
    let four = U256::from(4u64);
    let two = U256::from(2u64);
    [
        clamp_borrow((max_borrow / sixteen).max(U256::from(1u64)), max_borrow),
        clamp_borrow((max_borrow / eight).max(U256::from(1u64)), max_borrow),
        clamp_borrow((max_borrow / four).max(U256::from(1u64)), max_borrow),
        clamp_borrow((max_borrow / two).max(U256::from(1u64)), max_borrow),
        clamp_borrow(max_borrow.max(U256::from(1u64)), max_borrow),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flash::{FlashSourceState, PANCAKE_V2_FLASH_FEE_BPS};
    use std::str::FromStr;

    fn a(n: u8) -> Address {
        let mut b = [0u8; 20];
        b[19] = n;
        Address::from(b)
    }
    fn wbnb() -> Address {
        Address::from_str(crate::venues::WBNB_ADDRESS).unwrap()
    }
    fn usdt() -> Address {
        Address::from_str(crate::venues::USDT_ADDRESS).unwrap()
    }

    fn free_snapshot(quote: Address, depth: u128) -> FlashSnapshot {
        FlashSnapshot {
            block: 1,
            measured_at_unix: 0,
            states: vec![FlashSourceState {
                source: FlashSource::InfinityVault,
                fee_bps: Some(0),
                available: vec![(quote, U256::from(depth))],
                error: None,
            }],
        }
    }

    /// Hai pool CÂN BẰNG hệt nhau -> đi 1 vòng chỉ mất 2 lần phí 0,25 %, LUÔN
    /// lỗ. Nếu test này fail nghĩa là công thức đang tự sinh tiền.
    #[test]
    fn hai_pool_can_bang_thi_arb_luon_lo() {
        let p = ArbPool {
            pair: a(1),
            quote: wbnb(),
            reserve_quote: U256::from(1_000_000_000_000_000_000_000u128), // 1000
            reserve_token: U256::from(1_000_000_000_000_000_000_000u128),
        };
        let route = ArbRoute { token: a(9), borrow_quote: wbnb(), buy: p, sell: ArbPool { pair: a(2), ..p }, bridge: None };
        let snap = free_snapshot(wbnb(), u128::MAX / 2);
        let q = search_best_arb(&route, U256::from(100_000_000_000_000_000_000u128), &snap, None, 0, 0.0, None).unwrap();
        assert!(q.net_wei <= 0, "2 pool can bang KHONG duoc co lai (net={})", q.net_wei);
    }

    /// Pool bị victim đẩy lệch -> arb phải có lãi, và lãi phải nhỏ hơn độ
    /// lệch (vì mất 2 lần phí).
    #[test]
    fn pool_lech_gia_thi_arb_co_lai_va_lai_nho_hon_do_lech() {
        let base = 1_000_000_000_000_000_000_000u128; // 1000 đơn vị
        // Pool A: victim vừa MUA token bằng quote -> token đắt lên ở A.
        let pool_a = ArbPool {
            pair: a(1),
            quote: wbnb(),
            reserve_quote: U256::from(base + 100_000_000_000_000_000_000u128),
            reserve_token: U256::from(base - 90_000_000_000_000_000_000u128),
        };
        // Pool B: chưa ai đụng -> token vẫn rẻ.
        let pool_b = ArbPool { pair: a(2), quote: wbnb(), reserve_quote: U256::from(base), reserve_token: U256::from(base) };
        // Mua ở B (rẻ), bán ở A (đắt).
        let route = ArbRoute { token: a(9), borrow_quote: wbnb(), buy: pool_b, sell: pool_a, bridge: None };
        let snap = free_snapshot(wbnb(), u128::MAX / 2);
        let q = search_best_arb(&route, U256::from(500_000_000_000_000_000_000u128), &snap, None, 0, 0.0, None).unwrap();
        assert!(q.net_wei > 0, "pool lech phai co lai, net={}", q.net_wei);
        assert!(q.borrow > U256::ZERO);
        assert!(q.net_wei < 100_000_000_000_000_000_000i128, "lai khong duoc lon hon ca do lech");
    }

    /// Hướng NGƯỢC (mua ở pool đắt, bán ở pool rẻ) phải LỖ — và
    /// `best_arb_for_pools` phải tự chọn đúng hướng lãi.
    #[test]
    fn best_arb_tu_chon_dung_huong() {
        let base = 1_000_000_000_000_000_000_000u128;
        let pool_a = ArbPool {
            pair: a(1),
            quote: wbnb(),
            reserve_quote: U256::from(base + 100_000_000_000_000_000_000u128),
            reserve_token: U256::from(base - 90_000_000_000_000_000_000u128),
        };
        let pool_b = ArbPool { pair: a(2), quote: wbnb(), reserve_quote: U256::from(base), reserve_token: U256::from(base) };
        let snap = free_snapshot(wbnb(), u128::MAX / 2);
        let max = U256::from(500_000_000_000_000_000_000u128);
        let (route, q) = best_arb_for_pools(
            a(9),
            pool_a,
            pool_b,
            None,
            U256::ZERO,
            U256::ZERO,
            wbnb(),
            &|_| max,
            &snap,
            &|_| 0u128,
            0.0,
            None,
        )
        .unwrap();
        assert_eq!(route.buy.pair, pool_b.pair, "phai MUA o pool re (B)");
        assert_eq!(route.sell.pair, pool_a.pair, "phai BAN o pool dat (A)");
        assert!(q.net_wei > 0);
    }

    /// Phí flash ăn thẳng vào lãi: cùng route, nguồn 25 bps phải cho net THẤP
    /// HƠN nguồn 0 bps. Đây là chỗ dễ quên nhất khi tính "flash loan 0 phí".
    #[test]
    fn phi_flash_lam_giam_lai_that_su() {
        let base = 1_000_000_000_000_000_000_000u128;
        let pool_a = ArbPool {
            pair: a(1),
            quote: wbnb(),
            reserve_quote: U256::from(base + 100_000_000_000_000_000_000u128),
            reserve_token: U256::from(base - 90_000_000_000_000_000_000u128),
        };
        let pool_b = ArbPool { pair: a(2), quote: wbnb(), reserve_quote: U256::from(base), reserve_token: U256::from(base) };
        let route = ArbRoute { token: a(9), borrow_quote: wbnb(), buy: pool_b, sell: pool_a, bridge: None };
        let max = U256::from(500_000_000_000_000_000_000u128);
        let depth = U256::from(u128::MAX / 2);
        let free =
            search_borrow_for_source(&route, max, FlashSource::InfinityVault, 0, depth, 0, 0.0, None).unwrap();
        let paid = search_borrow_for_source(
            &route,
            max,
            FlashSource::PancakeV2FlashSwap,
            PANCAKE_V2_FLASH_FEE_BPS,
            depth,
            0,
            0.0,
            None,
        )
        .unwrap();
        assert!(paid.net_wei < free.net_wei, "25bps phai an vao lai (free={}, paid={})", free.net_wei, paid.net_wei);
        assert!(paid.flash_fee_wei > U256::ZERO);
        assert_eq!(free.flash_fee_wei, U256::ZERO);
    }

    /// Gas và bribe phải trừ THẬT, và bribe chỉ tính trên lãi DƯƠNG.
    #[test]
    fn gas_va_bribe_tru_that_va_bribe_khong_am() {
        let base = 1_000_000_000_000_000_000_000u128;
        let pool_a = ArbPool {
            pair: a(1),
            quote: wbnb(),
            reserve_quote: U256::from(base + 100_000_000_000_000_000_000u128),
            reserve_token: U256::from(base - 90_000_000_000_000_000_000u128),
        };
        let pool_b = ArbPool { pair: a(2), quote: wbnb(), reserve_quote: U256::from(base), reserve_token: U256::from(base) };
        let route = ArbRoute { token: a(9), borrow_quote: wbnb(), buy: pool_b, sell: pool_a, bridge: None };
        let snap = free_snapshot(wbnb(), u128::MAX / 2);
        let max = U256::from(500_000_000_000_000_000_000u128);
        let no_cost = search_best_arb(&route, max, &snap, None, 0, 0.0, None).unwrap();
        let with_cost = search_best_arb(&route, max, &snap, None, 1_000_000_000_000_000u128, 40.0, None).unwrap();
        assert!(with_cost.net_wei < no_cost.net_wei);
        assert!(with_cost.bribe_wei > 0, "co lai thi phai co bribe");
        assert_eq!(with_cost.net_wei, with_cost.profit_before_bribe_wei - with_cost.bribe_wei as i128);
        // Route lo -> bribe = 0 (khong hoi lo khi lo)
        assert_eq!(arb_bribe_wei(-1, 40.0, None), 0);
        assert_eq!(arb_bribe_wei(0, 40.0, None), 0);
    }

    /// Chân bridge khác quote: phải đi qua pool WBNB/USDT THẬT, và thiếu pool
    /// đó thì hướng đó bị BỎ (không quy đổi bịa).
    #[test]
    fn khac_quote_phai_co_bridge_that() {
        let base = 1_000_000_000_000_000_000_000u128;
        let pool_wbnb = ArbPool { pair: a(1), quote: wbnb(), reserve_quote: U256::from(base), reserve_token: U256::from(base) };
        let pool_usdt = ArbPool {
            pair: a(2),
            quote: usdt(),
            reserve_quote: U256::from(base * 600),
            reserve_token: U256::from(base),
        };
        let snap = free_snapshot(wbnb(), u128::MAX / 2);
        let max = U256::from(100_000_000_000_000_000_000u128);
        // Khong co bridge -> khong route nao chay duoc
        let none = best_arb_for_pools(
            a(9),
            pool_wbnb,
            pool_usdt,
            None,
            U256::ZERO,
            U256::ZERO,
            wbnb(),
            &|_| max,
            &snap,
            &|_| 0u128,
            0.0,
            None,
        );
        assert!(none.is_none(), "thieu pool bridge that thi KHONG duoc sim");
        // Co bridge -> chay duoc (co the lai hoac lo, mien la co ket qua)
        let some = best_arb_for_pools(
            a(9),
            pool_wbnb,
            pool_usdt,
            Some(a(3)),
            U256::from(base * 10),
            U256::from(base * 6000),
            wbnb(),
            &|_| max,
            &snap,
            &|_| 0u128,
            0.0,
            None,
        );
        assert!(some.is_some());
    }

    #[test]
    fn ap_victim_doi_dung_chieu_reserve() {
        let p = ArbPool {
            pair: a(1),
            quote: wbnb(),
            reserve_quote: U256::from(1000u64),
            reserve_token: U256::from(1000u64),
        };
        let after_buy = apply_victim_to_pool(p, U256::from(100u64), true).unwrap();
        assert_eq!(after_buy.reserve_quote, U256::from(1100u64));
        assert!(after_buy.reserve_token < p.reserve_token, "victim mua -> token trong pool phai giam");
        let after_sell = apply_victim_to_pool(p, U256::from(100u64), false).unwrap();
        assert_eq!(after_sell.reserve_token, U256::from(1100u64));
        assert!(after_sell.reserve_quote < p.reserve_quote, "victim ban -> quote trong pool phai giam");
    }

    #[test]
    fn sanity_chan_so_vo_ly() {
        let p = ArbPool {
            pair: a(1),
            quote: wbnb(),
            reserve_quote: U256::from(1_000_000u64),
            reserve_token: U256::from(1_000_000u64),
        };
        let route = ArbRoute { token: a(9), borrow_quote: wbnb(), buy: p, sell: p, bridge: None };
        let ok = ArbQuote {
            borrow: U256::from(1000u64),
            token_out: U256::from(1000u64),
            quote_sell_out: U256::from(1000u64),
            final_out: U256::from(1100u64),
            flash_source: FlashSource::InfinityVault,
            flash_fee_bps: 0,
            flash_fee_wei: U256::ZERO,
            gas_wei: 0,
            bribe_wei: 0,
            gross_wei: 100,
            profit_before_bribe_wei: 100,
            net_wei: 100,
        };
        assert!(arb_sanity_ok(&route, &ok));
        let vay_qua_lon = ArbQuote { borrow: U256::from(600_000u64), ..ok };
        assert!(!arb_sanity_ok(&route, &vay_qua_lon), "vay > 50% reserve phai bi chan");
        let lai_vo_ly = ArbQuote { net_wei: 200_000, ..ok };
        assert!(!arb_sanity_ok(&route, &lai_vo_ly), "lai > 5% reserve phai bi chan");
    }

    #[test]
    fn bribe_khop_pipeline() {
        for p in [0.0, 10.0, 40.0, 100.0] {
            assert_eq!(arb_bribe_wei(1_000_000, p, None), crate::pipeline::compute_bribe_wei(1_000_000, p, None));
        }
    }

    #[test]
    fn v3_fee_2500_khop_v2_get_amount_out() {
        let rin = U256::from(1_000_000_000_000_000_000_000u128);
        let rout = U256::from(2_000_000_000_000_000_000_000u128);
        let ain = U256::from(10_000_000_000_000_000_000u128);
        let v2 = get_amount_out(ain, rin, rout).unwrap();
        let v3 = get_amount_out_v3(ain, rin, rout, 2500).unwrap();
        assert_eq!(v2, v3, "fee V3 2500 phai khop V2 0.25%");
    }

    #[test]
    fn fit_v3_khoi_phuc_reserve() {
        let rin = U256::from(5_000_000_000_000_000_000_000u128);
        let rout = U256::from(8_000_000_000_000_000_000_000u128);
        let fee = 500u32;
        let a1 = U256::from(200_000_000_000_000_000u128); // 0.2
        let a2 = U256::from(1_000_000_000_000_000_000u128); // 1
        let o1 = get_amount_out_v3(a1, rin, rout, fee).unwrap();
        let o2 = get_amount_out_v3(a2, rin, rout, fee).unwrap();
        let (fr, fo) = fit_v3_virtual_reserves(fee, a1, o1, a2, o2).unwrap();
        let o1b = get_amount_out_v3(a1, fr, fo, fee).unwrap();
        let o2b = get_amount_out_v3(a2, fr, fo, fee).unwrap();
        let d1 = if o1 > o1b { o1 - o1b } else { o1b - o1 };
        let d2 = if o2 > o2b { o2 - o2b } else { o2b - o2 };
        assert!(d1 * U256::from(1000u64) < o1, "fit lech o1 > 0.1%");
        assert!(d2 * U256::from(1000u64) < o2, "fit lech o2 > 0.1%");
    }

    #[test]
    fn v2_v3_lech_gia_thi_arb_co_lai() {
        let base = 1_000_000_000_000_000_000_000u128;
        let v2 = ArbPool {
            pair: a(1),
            quote: wbnb(),
            reserve_quote: U256::from(base),
            reserve_token: U256::from(base),
        };
        // V3 dat hon: nhieu quote, it token (victim vua mua).
        let v3 = ArbV3Pool {
            pool: a(3),
            quote: wbnb(),
            fee: 500,
            family: V3Family::Pcs,
            reserve_quote: U256::from(base + 80_000_000_000_000_000_000u128),
            reserve_token: U256::from(base - 70_000_000_000_000_000_000u128),
            ok: true,
        };
        let venues = [ArbVenue::V2(v2), ArbVenue::V3(v3)];
        let snap = free_snapshot(wbnb(), u128::MAX / 2);
        let max = U256::from(200_000_000_000_000_000_000u128);
        let (route, q) = best_arb_for_venues(
            a(9),
            &venues,
            None,
            U256::ZERO,
            U256::ZERO,
            wbnb(),
            &|_| max,
            &snap,
            &|_| 0u128,
            0.0,
            None,
        )
        .unwrap();
        assert_eq!(route_kind(route.buy, route.sell), "v2_v3");
        assert!(q.net_wei > 0, "v2 re / v3 dat phai co lai, net={}", q.net_wei);
        assert_eq!(route.buy.id(), v2.pair);
        assert_eq!(route.sell.id(), v3.pool);
    }

    #[test]
    fn v3_v3_khac_tier_lech_gia() {
        let base = 1_000_000_000_000_000_000_000u128;
        let cheap = ArbV3Pool {
            pool: a(4),
            quote: wbnb(),
            fee: 100,
            family: V3Family::Pcs,
            reserve_quote: U256::from(base),
            reserve_token: U256::from(base),
            ok: true,
        };
        let dear = ArbV3Pool {
            pool: a(5),
            quote: wbnb(),
            fee: 500,
            family: V3Family::Uni,
            reserve_quote: U256::from(base + 60_000_000_000_000_000_000u128),
            reserve_token: U256::from(base - 50_000_000_000_000_000_000u128),
            ok: true,
        };
        let venues = [ArbVenue::V3(cheap), ArbVenue::V3(dear)];
        let snap = free_snapshot(wbnb(), u128::MAX / 2);
        let max = U256::from(100_000_000_000_000_000_000u128);
        let (route, q) = best_arb_for_venues(
            a(9),
            &venues,
            None,
            U256::ZERO,
            U256::ZERO,
            wbnb(),
            &|_| max,
            &snap,
            &|_| 0u128,
            0.0,
            None,
        )
        .unwrap();
        assert_eq!(route_kind(route.buy, route.sell), "v3_v3");
        assert!(q.net_wei > 0);
        assert_eq!(route.buy.id(), cheap.pool);
    }

    #[test]
    fn five_borrow_grid_5_diem_tang() {
        let g = five_borrow_grid(U256::from(16u64));
        assert_eq!(g.len(), 5);
        for i in 1..5 {
            assert!(g[i] >= g[i - 1]);
        }
        assert_eq!(g[4], U256::from(16u64));
    }

    fn one_bnb() -> U256 {
        U256::from(1_000_000_000_000_000_000u128)
    }
    fn cap20() -> U256 {
        one_bnb() * U256::from(20u64)
    }
    fn cap40() -> U256 {
        one_bnb() * U256::from(40u64)
    }

    fn skewed_v2_pair() -> (ArbPool, ArbPool) {
        let base = 1_000_000_000_000_000_000_000u128;
        let dear = ArbPool {
            pair: a(1),
            quote: wbnb(),
            reserve_quote: U256::from(base + 200_000_000_000_000_000_000u128),
            reserve_token: U256::from(base - 150_000_000_000_000_000_000u128),
        };
        let cheap = ArbPool {
            pair: a(2),
            quote: wbnb(),
            reserve_quote: U256::from(base),
            reserve_token: U256::from(base),
        };
        (cheap, dear)
    }

    /// Cụm B6 — search V2↔V2 không vượt trần 20 BNB dù unconstrained tối ưu lớn hơn.
    #[test]
    fn search_v2_v2_khong_vuot_tran_20_bnb() {
        let (cheap, dear) = skewed_v2_pair();
        let route = ArbRoute { token: a(9), borrow_quote: wbnb(), buy: cheap, sell: dear, bridge: None };
        let snap = free_snapshot(wbnb(), u128::MAX / 2);
        let uncapped = search_best_arb(&route, cap40() * U256::from(10u64), &snap, None, 0, 0.0, None);
        let q = search_best_arb(&route, cap20(), &snap, None, 0, 0.0, None);
        if let Some(u) = uncapped {
            assert!(u.borrow <= cap40() * U256::from(10u64));
        }
        let q = q.expect("search trong tran 20 van phai ra quote (lai hoac lo)");
        assert!(q.borrow <= cap20(), "search V2↔V2 borrow={} > tran 20", q.borrow);
        assert_eq!(q.borrow, clamp_borrow(q.borrow, cap20()));
    }

    /// Cụm B6 — V2↔V3 MixedRoute.
    #[test]
    fn search_v2_v3_khong_vuot_tran_20_bnb() {
        let base = 1_000_000_000_000_000_000_000u128;
        let v2 = ArbPool {
            pair: a(1),
            quote: wbnb(),
            reserve_quote: U256::from(base),
            reserve_token: U256::from(base),
        };
        let v3 = ArbV3Pool {
            pool: a(3),
            quote: wbnb(),
            fee: 500,
            family: V3Family::Pcs,
            reserve_quote: U256::from(base + 200_000_000_000_000_000_000u128),
            reserve_token: U256::from(base - 150_000_000_000_000_000_000u128),
            ok: true,
        };
        let venues = [ArbVenue::V2(v2), ArbVenue::V3(v3)];
        let snap = free_snapshot(wbnb(), u128::MAX / 2);
        let found = best_arb_for_venues(
            a(9),
            &venues,
            None,
            U256::ZERO,
            U256::ZERO,
            wbnb(),
            &|_| cap20(),
            &snap,
            &|_| 0u128,
            0.0,
            None,
        );
        assert_eq!(route_kind(venues[0], venues[1]), "v2_v3");
        if let Some((_, q)) = found {
            assert!(q.borrow <= cap20(), "V2↔V3 borrow={} > 20 BNB", q.borrow);
        }
    }

    /// Cụm B6 — V3↔V3 (BAOCAO51 币安人生 / token 4 cùng kiểu).
    #[test]
    fn search_v3_v3_khong_vuot_tran_20_bnb() {
        let base = 1_000_000_000_000_000_000_000u128;
        let cheap = ArbV3Pool {
            pool: a(4),
            quote: wbnb(),
            fee: 100,
            family: V3Family::Uni,
            reserve_quote: U256::from(base),
            reserve_token: U256::from(base),
            ok: true,
        };
        let dear = ArbV3Pool {
            pool: a(5),
            quote: wbnb(),
            fee: 500,
            family: V3Family::Pcs,
            reserve_quote: U256::from(base + 180_000_000_000_000_000_000u128),
            reserve_token: U256::from(base - 140_000_000_000_000_000_000u128),
            ok: true,
        };
        let venues = [ArbVenue::V3(cheap), ArbVenue::V3(dear)];
        let snap = free_snapshot(wbnb(), u128::MAX / 2);
        let found = best_arb_for_venues(
            a(9),
            &venues,
            None,
            U256::ZERO,
            U256::ZERO,
            wbnb(),
            &|_| cap20(),
            &snap,
            &|_| 0u128,
            0.0,
            None,
        );
        assert_eq!(route_kind(venues[0], venues[1]), "v3_v3");
        if let Some((_, q)) = found {
            assert!(q.borrow <= cap20(), "V3↔V3 borrow={} > 20 BNB", q.borrow);
        }
    }

    /// Cụm B6 — mixed quote: trần theo ĐÚNG quote chân vay (USDT ≠ BNB).
    #[test]
    fn search_mixed_quote_kep_tran_dung_don_vi() {
        let base = 1_000_000_000_000_000_000_000u128;
        let v2_wbnb = ArbPool { pair: a(1), quote: wbnb(), reserve_quote: U256::from(base), reserve_token: U256::from(base) };
        let v3_usdt = ArbV3Pool {
            pool: a(6),
            quote: usdt(),
            fee: 2500,
            family: V3Family::Pcs,
            reserve_quote: U256::from(base * 600 + 80_000_000_000_000_000_000u128 * 600),
            reserve_token: U256::from(base - 70_000_000_000_000_000_000u128),
            ok: true,
        };
        let venues = [ArbVenue::V2(v2_wbnb), ArbVenue::V3(v3_usdt)];
        let snap = FlashSnapshot {
            block: 1,
            measured_at_unix: 0,
            states: vec![crate::flash::FlashSourceState {
                source: FlashSource::InfinityVault,
                fee_bps: Some(0),
                available: vec![(wbnb(), U256::from(u128::MAX / 2)), (usdt(), U256::from(u128::MAX / 2))],
                error: None,
            }],
        };
        let max_for = |q: Address| {
            if q == wbnb() {
                cap20()
            } else {
                one_bnb() * U256::from(12_000u64)
            }
        };
        let found = best_arb_for_venues(
            a(9),
            &venues,
            Some(a(3)),
            U256::from(base * 10),
            U256::from(base * 6000),
            wbnb(),
            &max_for,
            &snap,
            &|_| 0u128,
            0.0,
            None,
        );
        if let Some((route, q)) = found {
            let cap = max_for(route.borrow_quote);
            assert!(q.borrow <= cap, "mixed quote borrow={} > cap={} quote={:?}", q.borrow, cap, route.borrow_quote);
        }
    }

    #[test]
    fn clamp_borrow_khong_nho_hon_max_va_khong_nhan_x4() {
        assert_eq!(clamp_borrow(cap40(), cap20()), cap20());
        assert_eq!(clamp_borrow(cap20(), cap20()), cap20());
        assert_eq!(clamp_borrow(U256::ZERO, cap20()), U256::ZERO);
        assert_eq!(clamp_borrow(one_bnb(), cap20()), one_bnb());
        let g = five_borrow_grid(cap20());
        for x in g {
            assert!(x <= cap20(), "grid diem {x} vuot tran");
        }
    }

    #[test]
    fn accept_quote_loai_borrow_40_khi_tran_20() {
        let p = ArbPool {
            pair: a(1),
            quote: wbnb(),
            reserve_quote: U256::from(1_000_000u64),
            reserve_token: U256::from(1_000_000u64),
        };
        let q = ArbQuote {
            borrow: cap40(),
            token_out: U256::from(1u64),
            quote_sell_out: U256::from(1u64),
            final_out: U256::from(1u64),
            flash_source: FlashSource::InfinityVault,
            flash_fee_bps: 0,
            flash_fee_wei: U256::ZERO,
            gas_wei: 0,
            bribe_wei: 0,
            gross_wei: 1,
            profit_before_bribe_wei: 1,
            net_wei: 1,
        };
        assert!(accept_quote(q, cap20()).is_none(), "40 BNB / tran 20 phai loai");
        let q20 = ArbQuote { borrow: cap20(), ..q };
        assert!(accept_quote(q20, cap20()).is_some());
        let _ = p;
    }
}

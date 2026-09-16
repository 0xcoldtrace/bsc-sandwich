//! Cụm `planB-B4-multivenue-tool` — quy tắc list đa venue (thuần + RPC probe).
//!
//! Token vào list CHỈ khi: có pool PCS V2 (quote WBNB hoặc USDT) đủ ngưỡng
//! VÀ ≥1 pool PCS V3 **cùng quote** (tier bất kỳ) đủ ngưỡng. Pool Uniswap V3
//! nếu có thì ghi nhận, **không** thay được điều kiện. KHÔNG THENA/Biswap.
//!
//! Ngưỡng (ship, CLI override): V2 reserve_quote ≥ 50 BNB (≥ 35_000 USDT);
//! V3 impact ≤ 2 % khi bán 1 BNB quy đổi qua QuoterV2.

use alloy::primitives::{Address, U256};
use std::str::FromStr;

use crate::venues::{USDT_ADDRESS, WBNB_ADDRESS};

/// 50 BNB (18 decimals).
pub const DEFAULT_MIN_V2_BNB: f64 = 50.0;
/// 35_000 USDT (BSC-USD, 18 decimals).
pub const DEFAULT_MIN_V2_USDT: f64 = 35_000.0;
pub const DEFAULT_MAX_V3_IMPACT_PCT: f64 = 2.0;
pub const DEFAULT_PROBE_BNB: f64 = 1.0;

#[derive(Debug, Clone, Copy)]
pub struct DiscoverThresholds {
    pub min_v2_bnb_wei: U256,
    pub min_v2_usdt_wei: U256,
    pub max_v3_impact_pct: f64,
    pub probe_bnb_wei: U256,
}

impl DiscoverThresholds {
    pub fn from_cli(min_v2_bnb: f64, min_v2_usdt: f64, max_v3_impact_pct: f64, probe_bnb: f64) -> Self {
        Self {
            min_v2_bnb_wei: f64_to_wei(min_v2_bnb),
            min_v2_usdt_wei: f64_to_wei(min_v2_usdt),
            max_v3_impact_pct,
            probe_bnb_wei: f64_to_wei(probe_bnb),
        }
    }

    pub fn ship() -> Self {
        Self::from_cli(DEFAULT_MIN_V2_BNB, DEFAULT_MIN_V2_USDT, DEFAULT_MAX_V3_IMPACT_PCT, DEFAULT_PROBE_BNB)
    }
}

pub fn f64_to_wei(v: f64) -> U256 {
    if !v.is_finite() || v <= 0.0 {
        return U256::ZERO;
    }
    U256::from((v * 1e18).round() as u128)
}

pub fn wbnb() -> Address {
    Address::from_str(WBNB_ADDRESS).expect("WBNB pin")
}
pub fn usdt() -> Address {
    Address::from_str(USDT_ADDRESS).expect("USDT pin")
}

pub fn is_quote(addr: Address) -> bool {
    addr == wbnb() || addr == usdt()
}

pub fn quote_name(q: Address) -> &'static str {
    if q == wbnb() {
        "WBNB"
    } else if q == usdt() {
        "USDT"
    } else {
        "?"
    }
}

/// V2 đủ ngưỡng **từng phía** (quote WBNB vs USDT khác số).
pub fn v2_meets_min(reserve_quote: U256, quote: Address, th: &DiscoverThresholds) -> bool {
    if quote == wbnb() {
        reserve_quote >= th.min_v2_bnb_wei
    } else if quote == usdt() {
        reserve_quote >= th.min_v2_usdt_wei
    } else {
        false
    }
}

/// V3 đủ ngưỡng impact (0 ≤ impact ≤ max). `None` (quote lỗi) = không đủ.
pub fn v3_meets_impact(impact_pct: Option<f64>, th: &DiscoverThresholds) -> bool {
    match impact_pct {
        Some(p) if p.is_finite() && p >= 0.0 => p <= th.max_v3_impact_pct,
        _ => false,
    }
}

/// Impact % khi bán `probe_in` so với giá spot đo bằng `spot_in` nhỏ hơn.
/// `None` nếu mẫu số 0 hoặc overflow.
///
/// `impact = (1 - out_probe / (out_spot * probe_in / spot_in)) * 100`
/// Âm (pool sâu, làm tròn) → 0.
pub fn impact_pct_from_quotes(out_spot: U256, spot_in: U256, out_probe: U256, probe_in: U256) -> Option<f64> {
    if out_spot.is_zero() || spot_in.is_zero() || probe_in.is_zero() {
        return None;
    }
    let numer = out_probe.checked_mul(spot_in)?;
    let denom = out_spot.checked_mul(probe_in)?;
    if denom.is_zero() {
        return None;
    }
    if numer >= denom {
        return Some(0.0);
    }
    let diff = denom - numer;
    // percent * 1e6 (6 chữ số thập phân) rồi chia.
    let scaled = diff.checked_mul(U256::from(100_000_000u64))?; // * 1e8 → percent * 1e6
    let q = scaled / denom;
    let v = u128::try_from(q).ok()? as f64 / 1_000_000.0;
    Some(v)
}

#[derive(Debug, Clone)]
pub struct V2Obs {
    pub quote: Address,
    pub pair: Address,
    pub reserve_quote: U256,
    pub reserve_token: U256,
    pub ok: bool,
}

#[derive(Debug, Clone)]
pub struct V3Obs {
    pub quote: Address,
    pub pool: Address,
    pub fee: u32,
    pub impact_pct: Option<f64>,
    pub ok: bool,
}

#[derive(Debug, Clone)]
pub struct UniV3Obs {
    pub quote: Address,
    pub pool: Address,
    pub fee: u32,
    pub impact_pct: Option<f64>,
    pub ok: bool,
}

#[derive(Debug, Clone)]
pub struct TokenProbe {
    pub token: Address,
    pub symbol: Option<String>,
    pub vol24h_bnb: f64,
    pub v2: Vec<V2Obs>,
    pub v3: Vec<V3Obs>,
    pub uni_v3: Vec<UniV3Obs>,
    pub verified: Option<bool>,
    pub proxy: Option<bool>,
    /// Bổ sung từ `pairs.txt` (Chủ cho phép, BAOCAO49).
    pub from_pairs: bool,
}

impl TokenProbe {
    pub fn v2_ok(&self) -> bool {
        self.v2.iter().any(|p| p.ok)
    }
    /// PCS V3 impact ok **hoặc** Uniswap V3 impact ok.
    pub fn v3_ok(&self) -> bool {
        self.v3.iter().any(|p| p.ok) || self.uni_v3.iter().any(|p| p.ok)
    }
    pub fn has_any_v3_pool(&self) -> bool {
        !self.v3.is_empty() || !self.uni_v3.is_empty()
    }
    /// Volume: V2 đủ ngưỡng + (PCS V3 hoặc Uniswap V3) cùng quote, impact ok.
    pub fn both_ok(&self) -> bool {
        qualifies_v2_v3(&self.v2, &self.v3, &self.uni_v3)
    }
    /// Vào list: `both_ok` **hoặc** (từ pairs.txt **và** có pool V3 PCS/Uni).
    pub fn keep_in_list(&self) -> bool {
        self.both_ok() || (self.from_pairs && self.has_any_v3_pool())
    }
}

/// V2 ok + V3 ok cùng quote. V3 = PCS **hoặc** Uniswap (cùng ngưỡng impact).
pub fn qualifies_v2_v3(v2: &[V2Obs], v3: &[V3Obs], uni: &[UniV3Obs]) -> bool {
    for a in v2.iter().filter(|p| p.ok) {
        if v3.iter().any(|b| b.ok && b.quote == a.quote) {
            return true;
        }
        if uni.iter().any(|b| b.ok && b.quote == a.quote) {
            return true;
        }
    }
    false
}

/// Parse token từ `pairs.txt` (không đụng file vet). Dòng `0xToken` hoặc
/// `0xToken,0xQuote`. Trả unique, bỏ comment/lỗi.
pub fn parse_pairs_tokens(content: &str) -> Vec<(Address, Option<String>)> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for raw in content.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let comment = raw.splitn(2, '#').nth(1).unwrap_or("");
        let symbol = comment
            .split('|')
            .next()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let addr_part = line.split(',').next().unwrap_or("").trim();
        if let Ok(t) = Address::from_str(addr_part) {
            if seen.insert(t) {
                out.push((t, symbol));
            }
        }
    }
    out
}

/// 1 BNB quy đổi sang USDT qua reserve V2 WBNB/USDT (không oracle).
pub fn probe_usdt_from_bnb(probe_bnb_wei: U256, reserve_wbnb: U256, reserve_usdt: U256) -> Option<U256> {
    if reserve_wbnb.is_zero() {
        return None;
    }
    probe_bnb_wei.checked_mul(reserve_usdt)?.checked_div(reserve_wbnb)
}

pub fn candidates_line(p: &TokenProbe) -> String {
    let sym = p.symbol.as_deref().unwrap_or("?");
    let v2q: Vec<&str> = p.v2.iter().filter(|x| x.ok).map(|x| quote_name(x.quote)).collect();
    let v3t: Vec<String> = p
        .v3
        .iter()
        .filter(|x| x.ok)
        .map(|x| format!("{}@{}", quote_name(x.quote), x.fee))
        .collect();
    let unit: Vec<String> = p
        .uni_v3
        .iter()
        .filter(|x| x.ok || p.from_pairs)
        .map(|x| format!("{}@{}", quote_name(x.quote), x.fee))
        .collect();
    let src = if p.from_pairs { "pairs" } else { "vol" };
    let note = format!(
        "discover_multivenue src={src} vol={:.4} v2={} v3={} uni={}",
        p.vol24h_bnb,
        v2q.join("+"),
        v3t.join("+"),
        unit.join("+")
    );
    // Quote mặc định WBNB; nếu chỉ có V2 USDT (kể cả dưới ngưỡng, pairs) thì cột 2 USDT.
    let only_usdt = p.v2.iter().any(|x| x.quote == usdt()) && !p.v2.iter().any(|x| x.quote == wbnb());
    let addr = if only_usdt {
        format!("{:#x},{:#x}", p.token, usdt())
    } else {
        format!("{:#x}", p.token)
    };
    format!("{addr} # {sym} | vetted  | tax ?/? | owner unknown | {note}")
}

pub fn report_header() -> &'static str {
    "token\tsymbol\tvol24h_bnb\tv2_ok\tv3_ok\tboth_ok\tv2_quotes\tv3_ok_tiers\tv3_min_impact_pct\tuni_v3_tiers\tverified\tproxy"
}

pub fn report_row(p: &TokenProbe) -> String {
    let v2q: Vec<String> = p
        .v2
        .iter()
        .map(|x| format!("{}:{}:ok={}", quote_name(x.quote), x.reserve_quote, x.ok))
        .collect();
    let v3t: Vec<String> = p
        .v3
        .iter()
        .filter(|x| x.ok)
        .map(|x| format!("{}@{}", quote_name(x.quote), x.fee))
        .collect();
    let min_imp = p
        .v3
        .iter()
        .filter_map(|x| x.impact_pct)
        .fold(None, |acc: Option<f64>, v| Some(acc.map(|a| a.min(v)).unwrap_or(v)));
    let uni: Vec<String> = p.uni_v3.iter().map(|x| format!("{}@{}", quote_name(x.quote), x.fee)).collect();
    format!(
        "{:#x}\t{}\t{:.6}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        p.token,
        p.symbol.as_deref().unwrap_or(""),
        p.vol24h_bnb,
        p.v2_ok(),
        p.v3_ok(),
        p.both_ok(),
        v2q.join("|"),
        v3t.join("|"),
        min_imp.map(|v| format!("{v:.4}")).unwrap_or_else(|| "-".into()),
        uni.join("|"),
        p.verified.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
        p.proxy.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
    )
}

/// 5 blue-chip + 5 mid-cap BSC (địa chỉ well-known, không lấy từ pairs.txt).
pub fn probe_ten_tokens() -> &'static [(&'static str, &'static str, &'static str)] {
    &[
        ("blue", "CAKE", "0x0E09FaBB73Bd3Ade0a17ECC321fD13a19e81cE82"),
        ("blue", "ETH", "0x2170Ed0880ac9A755fd29B2688956BD959F933F8"),
        ("blue", "BTCB", "0x7130d2A12B9BCbFAe4f2634d864A1Ee1Ce3Ead9c"),
        ("blue", "USDC", "0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d"),
        ("blue", "DOGE", "0xbA2aE424d960c26247Dd6c32edC70B295c744C43"),
        ("mid", "TWT", "0x4B0F1812e5Df2A09796481Ff14017e6005508003"),
        ("mid", "LINK", "0xF8A0BF9cF54Bb92F17374d9e9A321E6a111a51bD"),
        ("mid", "DOT", "0x7083609fCE4d1d8Dc0C979AAb8c869Ea2C873402"),
        ("mid", "UNI", "0xBf5140A22578168FD562DCcF235E5D43A02ce9B1"),
        ("mid", "XRP", "0x1D2F0da169ceB9fC7B3144628dB156f3F6c60dBE"),
    ]
}

/// Topic0 Swap V2: `Swap(address,uint256,uint256,uint256,uint256,address)`.
pub const V2_SWAP_EVENT_SIG: &str = "Swap(address,uint256,uint256,uint256,uint256,address)";
/// Topic0 Swap V3: `Swap(address,address,int256,int256,uint160,uint128,int24)`.
pub const V3_SWAP_EVENT_SIG: &str = "Swap(address,address,int256,int256,uint160,uint128,int24)";
pub const TRANSFER_EVENT_SIG: &str = "Transfer(address,address,uint256)";
pub const V2_PAIR_CREATED_SIG: &str = "PairCreated(address,address,address,uint256)";
pub const V3_POOL_CREATED_SIG: &str = "PoolCreated(address,address,uint24,int24,address)";

/// Giải mã abs(int256) ABI (word 32 byte, two's complement).
pub fn abs_i256_word(word: &[u8]) -> Option<U256> {
    if word.len() < 32 {
        return None;
    }
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&word[..32]);
    if bytes[0] & 0x80 == 0 {
        Some(U256::from_be_bytes(bytes))
    } else {
        let v = U256::from_be_bytes(bytes);
        Some(!v + U256::from(1u64))
    }
}

pub fn decode_v2_swap_quote_volume(data: &[u8], token0_is_quote: bool) -> Option<U256> {
    if data.len() < 128 {
        return None;
    }
    let a0_in = U256::from_be_slice(&data[0..32]);
    let a1_in = U256::from_be_slice(&data[32..64]);
    let a0_out = U256::from_be_slice(&data[64..96]);
    let a1_out = U256::from_be_slice(&data[96..128]);
    if token0_is_quote {
        Some(a0_in.saturating_add(a0_out))
    } else {
        Some(a1_in.saturating_add(a1_out))
    }
}

pub fn decode_v3_swap_quote_volume(data: &[u8], token0_is_quote: bool) -> Option<U256> {
    if data.len() < 64 {
        return None;
    }
    let a0 = abs_i256_word(&data[0..32])?;
    let a1 = abs_i256_word(&data[32..64])?;
    Some(if token0_is_quote { a0 } else { a1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(n: u8) -> Address {
        let mut b = [0u8; 20];
        b[19] = n;
        Address::from(b)
    }

    fn th() -> DiscoverThresholds {
        DiscoverThresholds::ship()
    }

    fn v2_ok(quote: Address, reserve: U256) -> V2Obs {
        let t = th();
        V2Obs { quote, pair: a(1), reserve_quote: reserve, reserve_token: U256::from(1u64), ok: v2_meets_min(reserve, quote, &t) }
    }
    fn v3_ok(quote: Address, impact: f64) -> V3Obs {
        let t = th();
        let imp = Some(impact);
        V3Obs { quote, pool: a(2), fee: 2500, impact_pct: imp, ok: v3_meets_impact(imp, &t) }
    }

    #[test]
    fn v2_threshold_wbnb_50() {
        let t = th();
        let just = f64_to_wei(50.0);
        let under = f64_to_wei(49.999);
        assert!(v2_meets_min(just, wbnb(), &t));
        assert!(!v2_meets_min(under, wbnb(), &t));
        assert!(!v2_meets_min(just, usdt(), &t)); // 50 USDT << 35000
    }

    #[test]
    fn v2_threshold_usdt_35000() {
        let t = th();
        let just = f64_to_wei(35_000.0);
        let under = f64_to_wei(34_999.0);
        assert!(v2_meets_min(just, usdt(), &t));
        assert!(!v2_meets_min(under, usdt(), &t));
    }

    #[test]
    fn v3_impact_at_most_2_pct() {
        let t = th();
        assert!(v3_meets_impact(Some(0.0), &t));
        assert!(v3_meets_impact(Some(2.0), &t));
        assert!(!v3_meets_impact(Some(2.0001), &t));
        assert!(!v3_meets_impact(Some(-0.1), &t));
        assert!(!v3_meets_impact(None, &t));
        assert!(!v3_meets_impact(Some(f64::NAN), &t));
    }

    #[test]
    fn both_ok_requires_same_quote() {
        let v2 = vec![v2_ok(wbnb(), f64_to_wei(100.0))];
        let v3_same = vec![v3_ok(wbnb(), 1.0)];
        let v3_other = vec![v3_ok(usdt(), 0.5)];
        assert!(qualifies_v2_v3(&v2, &v3_same, &[]));
        assert!(!qualifies_v2_v3(&v2, &v3_other, &[]));
    }

    #[test]
    fn token_only_v2_rejected() {
        let v2 = vec![v2_ok(wbnb(), f64_to_wei(100.0))];
        assert!(!qualifies_v2_v3(&v2, &[], &[]));
    }

    #[test]
    fn token_only_v3_rejected() {
        let v3 = vec![v3_ok(wbnb(), 0.1)];
        assert!(!qualifies_v2_v3(&[], &v3, &[]));
    }

    #[test]
    fn v2_below_threshold_rejected_even_if_v3_ok() {
        let v2 = vec![v2_ok(wbnb(), f64_to_wei(10.0))];
        let v3 = vec![v3_ok(wbnb(), 0.1)];
        assert!(!v2[0].ok);
        assert!(!qualifies_v2_v3(&v2, &v3, &[]));
    }

    #[test]
    fn v3_above_impact_rejected_even_if_v2_ok() {
        let v2 = vec![v2_ok(wbnb(), f64_to_wei(100.0))];
        let v3 = vec![v3_ok(wbnb(), 5.0)];
        assert!(!v3[0].ok);
        assert!(!qualifies_v2_v3(&v2, &v3, &[]));
    }

    #[test]
    fn uniswap_does_not_replace_pcs_v3() {
        // TokenProbe.both_ok chỉ nhìn v3 (PCS), không nhìn uni_v3.
        let p = TokenProbe {
            token: a(9),
            symbol: Some("X".into()),
            vol24h_bnb: 999.0,
            v2: vec![v2_ok(wbnb(), f64_to_wei(200.0))],
            v3: vec![],
            uni_v3: vec![UniV3Obs { quote: wbnb(), pool: a(3), fee: 3000, impact_pct: Some(0.1), ok: true }],
            verified: None,
            proxy: None,
            from_pairs: false,
        };
        assert!(p.v2_ok());
        assert!(p.v3_ok());
        assert!(p.both_ok(), "Uniswap V3 impact ok + cung quote duoc tinh");
    }

    #[test]
    fn uniswap_impact_over_threshold_does_not_qualify_volume() {
        let p = TokenProbe {
            token: a(9),
            symbol: Some("X".into()),
            vol24h_bnb: 1.0,
            v2: vec![v2_ok(wbnb(), f64_to_wei(200.0))],
            v3: vec![],
            uni_v3: vec![UniV3Obs { quote: wbnb(), pool: a(3), fee: 3000, impact_pct: Some(5.0), ok: false }],
            verified: None,
            proxy: None,
            from_pairs: false,
        };
        assert!(!p.both_ok());
        assert!(!p.keep_in_list());
    }

    #[test]
    fn pairs_txt_with_any_v3_is_kept_even_if_impact_fails() {
        let p = TokenProbe {
            token: a(9),
            symbol: Some("OLD".into()),
            vol24h_bnb: 0.0,
            v2: vec![v2_ok(wbnb(), f64_to_wei(10.0))], // duoi nguong 50
            v3: vec![],
            uni_v3: vec![UniV3Obs { quote: wbnb(), pool: a(3), fee: 3000, impact_pct: Some(9.0), ok: false }],
            verified: None,
            proxy: None,
            from_pairs: true,
        };
        assert!(!p.both_ok());
        assert!(p.has_any_v3_pool());
        assert!(p.keep_in_list());
    }

    #[test]
    fn pairs_txt_without_v3_not_kept() {
        let p = TokenProbe {
            token: a(8),
            symbol: Some("V2ONLY".into()),
            vol24h_bnb: 0.0,
            v2: vec![v2_ok(wbnb(), f64_to_wei(80.0))],
            v3: vec![],
            uni_v3: vec![],
            verified: None,
            proxy: None,
            from_pairs: true,
        };
        assert!(!p.keep_in_list());
    }

    #[test]
    fn uniswap_present_does_not_break_pcs_qualify() {
        let p = TokenProbe {
            token: a(9),
            symbol: Some("Y".into()),
            vol24h_bnb: 1.0,
            v2: vec![v2_ok(wbnb(), f64_to_wei(80.0))],
            v3: vec![v3_ok(wbnb(), 1.5)],
            uni_v3: vec![],
            verified: None,
            proxy: None,
            from_pairs: false,
        };
        assert!(p.both_ok());
    }

    #[test]
    fn impact_zero_when_probe_matches_spot_scale() {
        // spot 0.001 -> out 1; probe 1.0 -> out 1000  => impact 0
        let spot_in = f64_to_wei(0.001);
        let probe_in = f64_to_wei(1.0);
        let out_spot = U256::from(1_000u64);
        let out_probe = U256::from(1_000_000u64);
        let imp = impact_pct_from_quotes(out_spot, spot_in, out_probe, probe_in).unwrap();
        assert!(imp.abs() < 1e-6, "imp={imp}");
    }

    #[test]
    fn impact_two_pct_fixture() {
        // expected out_probe = out_spot * 1000; give 98% of that → 2% impact
        let spot_in = U256::from(1_000u64);
        let probe_in = U256::from(1_000_000u64);
        let out_spot = U256::from(1_000_000u64);
        let out_probe = U256::from(980_000_000u64); // 98% of 1_000_000 * 1000
        let imp = impact_pct_from_quotes(out_spot, spot_in, out_probe, probe_in).unwrap();
        assert!((imp - 2.0).abs() < 0.01, "imp={imp}");
        assert!(v3_meets_impact(Some(imp), &th()));
    }

    #[test]
    fn abs_i256_positive_and_negative() {
        let pos = U256::from(42u64).to_be_bytes::<32>();
        assert_eq!(abs_i256_word(&pos), Some(U256::from(42u64)));
        // -1 = 0xff..ff
        let neg = [0xffu8; 32];
        assert_eq!(abs_i256_word(&neg), Some(U256::from(1u64)));
    }

    #[test]
    fn candidates_line_leaves_vetted_blank() {
        let p = TokenProbe {
            token: a(9),
            symbol: Some("CAKE".into()),
            vol24h_bnb: 12.5,
            v2: vec![v2_ok(wbnb(), f64_to_wei(80.0))],
            v3: vec![v3_ok(wbnb(), 0.4)],
            uni_v3: vec![],
            verified: None,
            proxy: None,
            from_pairs: false,
        };
        let line = candidates_line(&p);
        assert!(line.contains("vetted  |"), "vetted phai de trong: {line}");
        assert!(!line.contains("vetted 20"), "{line}");
        assert!(line.starts_with("0x"));
    }

    #[test]
    fn probe_ten_is_five_blue_five_mid() {
        let t = probe_ten_tokens();
        assert_eq!(t.len(), 10);
        assert_eq!(t.iter().filter(|x| x.0 == "blue").count(), 5);
        assert_eq!(t.iter().filter(|x| x.0 == "mid").count(), 5);
        for (_, _, addr) in t {
            assert_eq!(addr.len(), 42);
            assert!(addr.starts_with("0x"));
        }
    }

    #[test]
    fn probe_usdt_from_bnb_uses_reserve_ratio() {
        // 1 BNB, pool 1 WBNB : 600 USDT → 600 USDT
        let usdt_out = probe_usdt_from_bnb(f64_to_wei(1.0), f64_to_wei(1.0), f64_to_wei(600.0)).unwrap();
        assert_eq!(usdt_out, f64_to_wei(600.0));
    }

    #[test]
    fn parse_pairs_tokens_skips_comments_unique() {
        let s = "# cmt\n0x00000000000000000000000000000000000000aa # AA | vetted 2026-09-16\n0x00000000000000000000000000000000000000aa # dup\n0x00000000000000000000000000000000000000bb,0x55d398326f99059ff775485246999027b3197955 # BB\n";
        let v = parse_pairs_tokens(s);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].1.as_deref(), Some("AA"));
    }

    /// RPC thật: 5 blue-chip + 5 mid-cap. Chạy:
    /// `cargo test --lib real_rpc_discover_mv_ten_tokens -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn real_rpc_discover_mv_ten_tokens() {
        use crate::pool;
        use crate::sim_v3;
        use crate::venues::{
            UNI_V3_FACTORY_ADDRESS, UNI_V3_FEE_TIERS, V2_FACTORY_ADDRESS, V3_FACTORY_ADDRESS,
            V3_QUOTER_V2_ADDRESS,
        };
        use alloy::providers::{Provider, ProviderBuilder};

        let provider = ProviderBuilder::new()
            .connect("https://bsc-dataseed.binance.org/")
            .await
            .expect("rpc");
        let chain = provider.get_chain_id().await.expect("chainId");
        assert_eq!(chain, 56, "eth_chainId phai 0x38");
        let block = provider.get_block_number().await.expect("block");
        println!("real_rpc_discover_mv_ten_tokens chain=56 block={block}");

        let v2f = Address::from_str(V2_FACTORY_ADDRESS).unwrap();
        let v3f = Address::from_str(V3_FACTORY_ADDRESS).unwrap();
        let unif = Address::from_str(UNI_V3_FACTORY_ADDRESS).unwrap();
        let quoter = Address::from_str(V3_QUOTER_V2_ADDRESS).unwrap();
        let t = th();
        let probe = t.probe_bnb_wei;

        for (kind, sym, hex) in probe_ten_tokens() {
            let token = Address::from_str(hex).unwrap();
            let mut v2_ok = false;
            let mut v3_ok = false;
            let mut uni_n = 0usize;
            let mut v2_note = String::new();
            let mut v3_note = String::new();
            for quote in [wbnb(), usdt()] {
                if let Ok(Ok(pair)) = pool::resolve_v2_pair_for_quote(&provider, v2f, token, quote).await {
                    if let Ok((rq, _)) = pool::get_reserves_vs_quote(&provider, pair, quote).await {
                        let ok = v2_meets_min(rq, quote, &t);
                        v2_ok |= ok;
                        v2_note.push_str(&format!("{} res={} ok={ok}; ", quote_name(quote), rq));
                    }
                }
                if let Ok(found) = pool::resolve_v3_pools_for_quote_tiers(&provider, v3f, token, quote, &pool::V3_FEE_TIERS).await {
                    for (pool_addr, fee) in found {
                        let spot = probe / U256::from(1000u64);
                        let out_s = sim_v3::quote_exact_input_single(&provider, quoter, quote, token, fee, spot).await.ok();
                        let out_p = sim_v3::quote_exact_input_single(&provider, quoter, quote, token, fee, probe).await.ok();
                        let imp = match (out_s, out_p) {
                            (Some(a), Some(b)) => impact_pct_from_quotes(a, spot, b, probe),
                            _ => None,
                        };
                        let ok = v3_meets_impact(imp, &t);
                        v3_ok |= ok;
                        v3_note.push_str(&format!("{}@{} {pool_addr:#x} impact={imp:?} ok={ok}; ", quote_name(quote), fee));
                    }
                }
                if let Ok(found) = pool::resolve_v3_pools_for_quote_tiers(&provider, unif, token, quote, &UNI_V3_FEE_TIERS).await {
                    uni_n += found.len();
                }
            }
            let both = v2_ok && v3_ok;
            println!("{kind} {sym} {hex} v2_ok={v2_ok} v3_ok={v3_ok} both={both} uni_pools={uni_n} | v2[{v2_note}] v3[{v3_note}]");
        }
    }
}

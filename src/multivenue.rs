//! Cụm `planB-B0-complete` — bản đồ venue thứ 2 (`state/multi_venue.json`).
//!
//! File do `cargo run --release --bin build_multi_venue` sinh: mỗi token
//! trong `pairs.txt` kèm pool V2 (WBNB + USDT) + reserve thật, ứng viên V3
//! (fee tier) và Infinity CL/Bin (nếu `eth_getLogs Initialize` trong cửa sổ
//! ≤ 5000 block tìm được). Đường nóng CHỈ đọc file này (0 RPC enumerate).
//! Thiếu file / token không có ≥2 pool V2 đủ sâu → skip `arb_no_second_venue`.

use alloy::primitives::{Address, U256};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use crate::sim_arb::ArbPool;
use crate::venues::{USDT_ADDRESS, WBNB_ADDRESS};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MultiVenueFile {
    pub generated_at_unix: u64,
    pub block: u64,
    pub infinity_from_block: u64,
    pub infinity_to_block: u64,
    pub min_reserve_wbnb_wei: String,
    pub min_reserve_usdt_wei: String,
    pub bridge_wbnb_usdt_pair: Option<String>,
    pub bridge_reserve_wbnb: Option<String>,
    pub bridge_reserve_usdt: Option<String>,
    pub tokens: Vec<TokenVenues>,
    /// Cụm `planB-B4-multivenue-tool` — nguồn sinh file (`discover_multivenue`
    /// vs `build_multi_venue` cũ từ pairs.txt).
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub hours: Option<u64>,
    #[serde(default)]
    pub scanned_tokens: Option<u64>,
    #[serde(default)]
    pub v2_ok_count: Option<u64>,
    #[serde(default)]
    pub v3_ok_count: Option<u64>,
    #[serde(default)]
    pub both_ok_count: Option<u64>,
    #[serde(default)]
    pub volume_method: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenVenues {
    pub token: String,
    pub symbol: Option<String>,
    /// Cũ (B0): ≥2 pool V2 đủ sâu. Mới (B4 discover): `both_ok` (PCS V2 + PCS V3).
    pub arb_ready: bool,
    pub v2_pools: Vec<V2PoolRec>,
    pub v3_pools: Vec<V3PoolRec>,
    #[serde(default)]
    pub infinity_pools: Vec<InfinityPoolRec>,
    #[serde(default)]
    pub uni_v3_pools: Vec<UniV3PoolRec>,
    #[serde(default)]
    pub vol24h_bnb: Option<f64>,
    #[serde(default)]
    pub v2_ok: bool,
    #[serde(default)]
    pub v3_ok: bool,
    #[serde(default)]
    pub both_ok: bool,
    #[serde(default)]
    pub verified: Option<bool>,
    #[serde(default)]
    pub proxy: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V2PoolRec {
    pub pair: String,
    pub quote: String,
    pub quote_name: String,
    pub reserve_quote: String,
    pub reserve_token: String,
    pub meets_min: bool,
    #[serde(default)]
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V3PoolRec {
    pub pool: String,
    pub quote: String,
    pub quote_name: String,
    pub fee: u32,
    #[serde(default)]
    pub impact_pct: Option<f64>,
    #[serde(default)]
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniV3PoolRec {
    pub pool: String,
    pub quote: String,
    pub quote_name: String,
    pub fee: u32,
    #[serde(default)]
    pub impact_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfinityPoolRec {
    pub family: String,
    pub pool_id: String,
    pub hooks: String,
    pub fee: u32,
    pub quote_name: String,
    pub currency0: String,
    pub currency1: String,
}

#[derive(Debug, Clone, Default)]
pub struct MultiVenueMap {
    pub block: u64,
    pub generated_at_unix: u64,
    pub infinity_from_block: u64,
    pub infinity_to_block: u64,
    by_token: HashMap<Address, TokenVenues>,
    pub bridge_pair: Option<Address>,
    pub bridge_reserve_wbnb: U256,
    pub bridge_reserve_usdt: U256,
}

impl MultiVenueMap {
    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        let raw = std::fs::read_to_string(path).map_err(|e| format!("doc {path:?} that bai: {e}"))?;
        Self::from_json_str(&raw)
    }

    pub fn from_json_str(s: &str) -> Result<Self, String> {
        let file: MultiVenueFile =
            serde_json::from_str(s).map_err(|e| format!("parse multi_venue.json that bai: {e}"))?;
        let mut by_token = HashMap::new();
        for t in file.tokens {
            if let Ok(addr) = Address::from_str(&t.token) {
                by_token.insert(addr, t);
            }
        }
        Ok(Self {
            block: file.block,
            generated_at_unix: file.generated_at_unix,
            infinity_from_block: file.infinity_from_block,
            infinity_to_block: file.infinity_to_block,
            by_token,
            bridge_pair: file.bridge_wbnb_usdt_pair.as_deref().and_then(|s| Address::from_str(s).ok()),
            bridge_reserve_wbnb: file.bridge_reserve_wbnb.as_deref().and_then(|s| U256::from_str(s).ok()).unwrap_or(U256::ZERO),
            bridge_reserve_usdt: file.bridge_reserve_usdt.as_deref().and_then(|s| U256::from_str(s).ok()).unwrap_or(U256::ZERO),
        })
    }

    pub fn len(&self) -> usize {
        self.by_token.len()
    }

    pub fn arb_ready_count(&self) -> usize {
        self.by_token.values().filter(|t| t.arb_ready).count()
    }

    pub fn get(&self, token: Address) -> Option<&TokenVenues> {
        self.by_token.get(&token)
    }

    /// Pool V2 đủ sâu (≥2) của token — `None` nếu không arb_ready.
    pub fn arb_v2_pools(&self, token: Address) -> Option<Vec<ArbPool>> {
        let rec = self.by_token.get(&token)?;
        if !rec.arb_ready {
            return None;
        }
        let mut out = Vec::new();
        for p in &rec.v2_pools {
            if !p.meets_min {
                continue;
            }
            let pair = Address::from_str(&p.pair).ok()?;
            let quote = Address::from_str(&p.quote).ok()?;
            let reserve_quote = U256::from_str(&p.reserve_quote).ok()?;
            let reserve_token = U256::from_str(&p.reserve_token).ok()?;
            out.push(ArbPool { pair, quote, reserve_quote, reserve_token });
        }
        if out.len() < 2 {
            return None;
        }
        Some(out)
    }

    pub fn tokens_missing_second_venue(&self) -> Vec<&TokenVenues> {
        let mut v: Vec<&TokenVenues> = self.by_token.values().filter(|t| !t.arb_ready).collect();
        v.sort_by(|a, b| a.token.cmp(&b.token));
        v
    }
}

pub fn wbnb() -> Address {
    Address::from_str(WBNB_ADDRESS).expect("WBNB pin")
}
pub fn usdt() -> Address {
    Address::from_str(USDT_ADDRESS).expect("USDT pin")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_file_ok() {
        let s = r#"{"generated_at_unix":1,"block":2,"infinity_from_block":0,"infinity_to_block":0,"min_reserve_wbnb_wei":"0","min_reserve_usdt_wei":"0","tokens":[]}"#;
        let m = MultiVenueMap::from_json_str(s).unwrap();
        assert_eq!(m.len(), 0);
        assert_eq!(m.arb_ready_count(), 0);
    }

    #[test]
    fn arb_ready_can_be_looked_up() {
        let tok = "0x00000000000000000000000000000000000000aa";
        let s = format!(
            r#"{{"generated_at_unix":1,"block":2,"infinity_from_block":0,"infinity_to_block":0,"min_reserve_wbnb_wei":"0","min_reserve_usdt_wei":"0","tokens":[{{"token":"{tok}","symbol":"X","arb_ready":true,"v2_pools":[{{"pair":"0x0000000000000000000000000000000000000001","quote":"{WBNB_ADDRESS}","quote_name":"WBNB","reserve_quote":"100000000000000000000","reserve_token":"1","meets_min":true}},{{"pair":"0x0000000000000000000000000000000000000002","quote":"{USDT_ADDRESS}","quote_name":"USDT","reserve_quote":"20000000000000000000000","reserve_token":"1","meets_min":true}}],"v3_pools":[],"infinity_pools":[]}}]}}"#
        );
        let m = MultiVenueMap::from_json_str(&s).unwrap();
        let pools = m.arb_v2_pools(Address::from_str(tok).unwrap()).unwrap();
        assert_eq!(pools.len(), 2);
    }
}

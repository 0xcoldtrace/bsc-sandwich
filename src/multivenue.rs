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

use crate::sim_arb::{ArbPool, ArbV3Pool, ArbVenue, V3Family};
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
    #[serde(default)]
    pub from_pairs: bool,
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
    #[serde(default)]
    pub ok: bool,
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

    /// Cụm `planB-B5-simarb-v3-measure` — V2 (meets_min) + V3/Uni (`ok` =
    /// impact ≤ 2 %). `None` nếu <2 venue. Không đòi ≥2 V2.
    pub fn arb_mixed_venues(&self, token: Address) -> Option<Vec<ArbVenue>> {
        let rec = self.by_token.get(&token)?;
        let mut out = Vec::new();
        for p in &rec.v2_pools {
            if !p.meets_min && !p.ok {
                continue;
            }
            let pair = Address::from_str(&p.pair).ok()?;
            let quote = Address::from_str(&p.quote).ok()?;
            let reserve_quote = U256::from_str(&p.reserve_quote).ok().unwrap_or(U256::ZERO);
            let reserve_token = U256::from_str(&p.reserve_token).ok().unwrap_or(U256::ZERO);
            out.push(ArbVenue::V2(ArbPool { pair, quote, reserve_quote, reserve_token }));
        }
        for p in &rec.v3_pools {
            if !p.ok {
                continue;
            }
            let pool = match Address::from_str(&p.pool) {
                Ok(a) => a,
                Err(_) => continue,
            };
            let quote = match Address::from_str(&p.quote) {
                Ok(a) => a,
                Err(_) => continue,
            };
            out.push(ArbVenue::V3(ArbV3Pool {
                pool,
                quote,
                fee: p.fee,
                family: V3Family::Pcs,
                reserve_quote: U256::ZERO,
                reserve_token: U256::ZERO,
                ok: p.ok,
            }));
        }
        for p in &rec.uni_v3_pools {
            if !p.ok {
                continue;
            }
            let pool = match Address::from_str(&p.pool) {
                Ok(a) => a,
                Err(_) => continue,
            };
            let quote = match Address::from_str(&p.quote) {
                Ok(a) => a,
                Err(_) => continue,
            };
            out.push(ArbVenue::V3(ArbV3Pool {
                pool,
                quote,
                fee: p.fee,
                family: V3Family::Uni,
                reserve_quote: U256::ZERO,
                reserve_token: U256::ZERO,
                ok: p.ok,
            }));
        }
        if out.len() < 2 {
            return None;
        }
        Some(out)
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

    #[test]
    fn mixed_venues_v2_plus_v3_ok() {
        let tok = "0x00000000000000000000000000000000000000aa";
        let s = format!(
            r#"{{"generated_at_unix":1,"block":2,"infinity_from_block":0,"infinity_to_block":0,"min_reserve_wbnb_wei":"0","min_reserve_usdt_wei":"0","tokens":[{{"token":"{tok}","symbol":"X","arb_ready":false,"v2_ok":true,"v3_ok":true,"both_ok":true,"v2_pools":[{{"pair":"0x0000000000000000000000000000000000000001","quote":"{WBNB_ADDRESS}","quote_name":"WBNB","reserve_quote":"100000000000000000000","reserve_token":"1","meets_min":true,"ok":true}}],"v3_pools":[{{"pool":"0x0000000000000000000000000000000000000003","quote":"{WBNB_ADDRESS}","quote_name":"WBNB","fee":2500,"impact_pct":0.1,"ok":true}}],"uni_v3_pools":[],"infinity_pools":[]}}]}}"#
        );
        let m = MultiVenueMap::from_json_str(&s).unwrap();
        let v = m.arb_mixed_venues(Address::from_str(tok).unwrap()).unwrap();
        assert_eq!(v.len(), 2);
        assert!(m.arb_v2_pools(Address::from_str(tok).unwrap()).is_none());
    }

    /// CASE_CAKE: pool bán 1% `ok=false` (impact 32.7 %) không vào search.
    /// Token `0x0e09FaBB…cE82`, pool mua `0x7f51c8aa…` fee 2500 ok, pool bán
    /// `0x55fe5567…` fee 10000 không ok. Hash
    /// `0xefabc7bfd29e81ed7897df19d18defaa89840f780a7307f5aec021c8d50ff779`.
    #[test]
    fn case_cake_bo_pool_v3_ok_false() {
        let tok = "0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82";
        let buy = "0x7f51c8aaa6b0599abd16674e2b17fec7a9f674a1";
        let sell_thin = "0x55fe55677e8398ef5c92f01b780403c9a771a1c0";
        let v2 = "0x0ed7e52944161450477ee417de9cd3a859b14fd0";
        let s = format!(
            r#"{{"generated_at_unix":1,"block":2,"infinity_from_block":0,"infinity_to_block":0,"min_reserve_wbnb_wei":"0","min_reserve_usdt_wei":"0","tokens":[{{"token":"{tok}","symbol":"Cake","arb_ready":true,"v2_ok":true,"v3_ok":true,"both_ok":true,"v2_pools":[{{"pair":"{v2}","quote":"{WBNB_ADDRESS}","quote_name":"WBNB","reserve_quote":"100000000000000000000","reserve_token":"1","meets_min":true,"ok":true}}],"v3_pools":[{{"pool":"{buy}","quote":"{USDT_ADDRESS}","quote_name":"USDT","fee":2500,"impact_pct":0.006642,"ok":true}},{{"pool":"{sell_thin}","quote":"{USDT_ADDRESS}","quote_name":"USDT","fee":10000,"impact_pct":32.735278,"ok":false}}],"uni_v3_pools":[],"infinity_pools":[]}}]}}"#
        );
        let m = MultiVenueMap::from_json_str(&s).unwrap();
        let v = m.arb_mixed_venues(Address::from_str(tok).unwrap()).unwrap();
        let ids: Vec<String> = v.iter().map(|x| format!("{:#x}", x.id())).collect();
        assert!(ids.iter().any(|x| x == buy), "pool 0.25% ok=true phai con");
        assert!(
            !ids.iter().any(|x| x == sell_thin),
            "pool 1% ok=false CASE_CAKE khong duoc search"
        );
    }

    /// CASE_LINK: pool mua Uni fee 3000 `ok=false` (impact 52 %) không vào search.
    /// Hash `0x0f9be5357e3c820ae8a9decebc786a7fd2c660008334f977bac347d5c148408a`.
    #[test]
    fn case_link_bo_pool_uni_ok_false() {
        let tok = "0x924fa68a0fc644485b8df8abfa0a41c2e7744444";
        let buy_thin = "0x75c5fbf77c1cd517544487aca4cc41e1ad95aced";
        let sell = "0xa1ff9406219ffa6bcc3d89c2719dd91d231d4cee";
        let v2 = "0x66f289de31eef70d52186729d2637ac978cfc56b";
        let s = format!(
            r#"{{"generated_at_unix":1,"block":2,"infinity_from_block":0,"infinity_to_block":0,"min_reserve_wbnb_wei":"0","min_reserve_usdt_wei":"0","tokens":[{{"token":"{tok}","symbol":"X","arb_ready":true,"v2_ok":true,"v3_ok":true,"both_ok":true,"v2_pools":[{{"pair":"{v2}","quote":"{WBNB_ADDRESS}","quote_name":"WBNB","reserve_quote":"100000000000000000000","reserve_token":"1","meets_min":true,"ok":true}}],"v3_pools":[{{"pool":"{sell}","quote":"{USDT_ADDRESS}","quote_name":"USDT","fee":10000,"impact_pct":0.224565,"ok":true}}],"uni_v3_pools":[{{"pool":"{buy_thin}","quote":"{USDT_ADDRESS}","quote_name":"USDT","fee":3000,"impact_pct":52.393932,"ok":false}}],"infinity_pools":[]}}]}}"#
        );
        let m = MultiVenueMap::from_json_str(&s).unwrap();
        let v = m.arb_mixed_venues(Address::from_str(tok).unwrap()).unwrap();
        let ids: Vec<String> = v.iter().map(|x| format!("{:#x}", x.id())).collect();
        assert!(ids.iter().any(|x| x == sell));
        assert!(
            !ids.iter().any(|x| x == buy_thin),
            "Uni 0.3% ok=false CASE_LINK khong duoc search"
        );
    }
}

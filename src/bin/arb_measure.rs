//! Cụm `planB-B5-simarb-v3-measure` — replay sim_arb đa venue (V2+V3) trên
//! mẫu swap lớn (log VPS / WSL) × `pairs_arb.txt` (list A both_ok đã vet).
//! Fit V3 1 lần / pool tại block hiện tại, search closed-form. Không ký, không gửi.

use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use bsc_sandwich::config::Config;
use bsc_sandwich::discover_mv;
use bsc_sandwich::flash::{FlashSnapshot, FlashSource, FlashSourceState};
use bsc_sandwich::multivenue::MultiVenueMap;
use bsc_sandwich::sim_arb::{self, ArbVenue};
use bsc_sandwich::sim_v3;
use bsc_sandwich::transport;
use bsc_sandwich::venues;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::str::FromStr;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut jsonl = None;
    let mut mv_path = "state/multi_venue.json".to_string();
    let mut pairs_arb = "pairs_arb.txt".to_string();
    let mut max_n: usize = 50_000;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--jsonl" => {
                jsonl = Some(args[i + 1].clone());
                i += 2;
            }
            "--multi-venue" => {
                mv_path = args[i + 1].clone();
                i += 2;
            }
            "--pairs-arb" => {
                pairs_arb = args[i + 1].clone();
                i += 2;
            }
            "--max" => {
                max_n = args[i + 1].parse().unwrap_or(50_000);
                i += 2;
            }
            _ => i += 1,
        }
    }
    let jsonl = match jsonl {
        Some(p) => p,
        None => {
            eprintln!("usage: arb_measure --jsonl FILE [--multi-venue PATH] [--pairs-arb PATH] [--max N]");
            std::process::exit(1);
        }
    };
    let list_a: HashSet<Address> = match std::fs::read_to_string(&pairs_arb) {
        Ok(s) => discover_mv::parse_pairs_tokens(&s).into_iter().map(|(a, _)| a).collect(),
        Err(e) => {
            eprintln!("FAIL doc {pairs_arb}: {e}");
            std::process::exit(1);
        }
    };
    let mv = match MultiVenueMap::load_from_path(Path::new(&mv_path)) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("FAIL load multi_venue: {e}");
            std::process::exit(1);
        }
    };
    let cfg = Config::load(Path::new("config.toml")).expect("config");
    let snap = infinite_free_snapshot();
    let wbnb = venues::wbnb_addr();

    let _ = transport::load_dotenv_defaults(Path::new(".env"));
    let urls = transport::filter_read_urls(transport::collect_rpc_urls_from_env("BSC_HTTP"));
    let provider = if urls.is_empty() {
        None
    } else {
        ProviderBuilder::new().connect(&urls[0]).await.ok()
    };
    let block = if let Some(p) = provider.as_ref() {
        p.get_block_number().await.unwrap_or(0)
    } else {
        0
    };

    let mut fitted: HashMap<Address, Vec<ArbVenue>> = HashMap::new();
    if let Some(p) = provider.as_ref() {
        let probe = U256::from(1_000_000_000_000_000_000u64);
        for &tok in &list_a {
            let Some(mut venues) = mv.arb_mixed_venues(tok) else {
                continue;
            };
            for v in &mut venues {
                if let ArbVenue::V3(vp) = v {
                    let pr = if vp.quote == wbnb {
                        probe
                    } else if !mv.bridge_reserve_wbnb.is_zero() {
                        probe.saturating_mul(mv.bridge_reserve_usdt)
                            / mv.bridge_reserve_wbnb.max(U256::from(1u64))
                    } else {
                        probe.saturating_mul(U256::from(600u64))
                    };
                    let _ = sim_v3::fit_arb_v3_pool(p, tok, vp, Some(block), pr).await;
                }
            }
            venues.retain(|v| v.is_fitted());
            if venues.len() >= 2 {
                fitted.insert(tok, venues);
            }
        }
        eprintln!("FIT tokens={} block={block} list_a={}", fitted.len(), list_a.len());
    } else {
        eprintln!("MISSING BSC_HTTP — chi dung V2 da fit trong file (V3 skip)");
        for &tok in &list_a {
            if let Some(venues) = mv.arb_mixed_venues(tok) {
                let v2: Vec<_> = venues.into_iter().filter(|v| matches!(v, ArbVenue::V2(_)) && v.is_fitted()).collect();
                if v2.len() >= 2 {
                    fitted.insert(tok, v2);
                }
            }
        }
    }

    println!("token\tsymbol\thash\tnet_usdt\tborrow_usdt\tsource\troute_kind\tbuy\tsell\tsanity\tcluster\tts");
    let mut n_opp = 0u64;
    let mut nets: Vec<f64> = Vec::new();
    let mut borrows: Vec<f64> = Vec::new();
    let mut cluster_n = 0u64;
    let mut n_in = 0u64;
    let mut by_key: HashMap<(String, String, String), Vec<f64>> = HashMap::new();
    let min_bnb = U256::from(500_000_000_000_000_000u64); // 0.5

    for line in std::fs::read_to_string(&jsonl).unwrap_or_default().lines() {
        if n_in >= max_n as u64 {
            break;
        }
        let d: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let ev = d.get("event").and_then(|v| v.as_str()).unwrap_or("");
        if ev != "tx.skip" && ev != "sim.result" && ev != "sim.arb" && ev != "tx.seen" {
            continue;
        }
        let token = match d.get("token").and_then(|v| v.as_str()).and_then(|s| Address::from_str(s).ok()) {
            Some(t) => t,
            None => continue,
        };
        if !list_a.contains(&token) {
            continue;
        }
        let Some(venues0) = fitted.get(&token) else {
            continue;
        };
        let amount_in = d
            .get("amount_in")
            .and_then(|v| v.as_str())
            .and_then(|s| U256::from_str(s).ok())
            .unwrap_or(U256::ZERO);
        if amount_in.is_zero() {
            continue;
        }
        let quote = d.get("quote").and_then(|v| v.as_str()).unwrap_or("wbnb");
        let amount_bnb = if quote == "usdt" && !mv.bridge_reserve_wbnb.is_zero() {
            amount_in.saturating_mul(mv.bridge_reserve_wbnb)
                / mv.bridge_reserve_usdt.max(U256::from(1u64))
        } else {
            amount_in
        };
        if amount_bnb < min_bnb {
            continue;
        }
        n_in += 1;
        let pair = d.get("pair").and_then(|v| v.as_str()).and_then(|s| Address::from_str(s).ok());
        let mut venues = venues0.clone();
        let victim_idx = pair
            .and_then(|p| venues.iter().position(|v| v.id() == p))
            .or_else(|| {
                venues.iter().position(|v| match v {
                    ArbVenue::V2(x) => x.quote == if quote == "usdt" { venues::usdt_addr() } else { wbnb },
                    ArbVenue::V3(x) => x.quote == if quote == "usdt" { venues::usdt_addr() } else { wbnb },
                })
            })
            .unwrap_or(0);
        if let Some(after) = sim_arb::apply_victim_to_venue(venues[victim_idx], amount_in, true) {
            venues[victim_idx] = after;
        } else {
            continue;
        }
        let max_for = |q: Address| cfg.arb_max_borrow_wei_for(q);
        let gas_for = |_q: Address| 0u128;
        let Some((route, q)) = sim_arb::best_arb_for_venues(
            token,
            &venues,
            mv.bridge_pair,
            mv.bridge_reserve_wbnb,
            mv.bridge_reserve_usdt,
            wbnb,
            &max_for,
            &snap,
            &gas_for,
            40.0,
            Some((cfg.bribe_min_wei(), cfg.bribe_max_wei())),
        ) else {
            continue;
        };
        if !sim_arb::arb_sanity_ok_mixed(&route, &q) || q.net_wei <= 0 {
            continue;
        }
        n_opp += 1;
        let net_usdt = net_as_usdt(q.net_wei, route.borrow_quote, &mv);
        nets.push(net_usdt);
        let borrow_usdt = borrow_as_usdt(&q.borrow, route.borrow_quote, &mv);
        borrows.push(borrow_usdt);
        let cluster = d.get("victim_in_competitor_cluster").and_then(|v| v.as_bool()).unwrap_or(false);
        if cluster {
            cluster_n += 1;
        }
        let ts = d.get("ts").and_then(|v| v.as_str()).unwrap_or("");
        let hour = ts.get(0..13).unwrap_or("?").to_string();
        let kind = sim_arb::route_kind(route.buy, route.sell).to_string();
        let tok_s = format!("{token:#x}");
        by_key.entry((tok_s.clone(), kind.clone(), hour)).or_default().push(net_usdt);
        println!(
            "{:#x}\t-\t{}\t{:.4}\t{:.4}\t{}\t{}\t{:#x}\t{:#x}\t1\t{}\t{}",
            token,
            d.get("hash").and_then(|v| v.as_str()).unwrap_or(""),
            net_usdt,
            borrow_usdt,
            q.flash_source.as_str(),
            kind,
            route.buy.id(),
            route.sell.id(),
            cluster as u8,
            ts,
        );
    }
    nets.sort_by(|a, b| a.partial_cmp(b).unwrap());
    borrows.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p = |v: &[f64], q: f64| -> f64 {
        if v.is_empty() {
            return 0.0;
        }
        v[(((v.len() - 1) as f64) * q).round() as usize]
    };
    println!(
        "SUMMARY n_in_listA={n_in} n_opp={n_opp} p50_usdt={:.4} p90_usdt={:.4} sum_usdt={:.2} borrow_p80={:.2} cluster_pct={:.1} fitted_tokens={}",
        p(&nets, 0.5),
        p(&nets, 0.9),
        nets.iter().sum::<f64>(),
        p(&borrows, 0.8),
        if n_opp == 0 { 0.0 } else { 100.0 * cluster_n as f64 / n_opp as f64 },
        fitted.len(),
    );
    println!("TABLE token\troute\thour\tn\tp50\tp90\tsum");
    let mut rows: Vec<_> = by_key.into_iter().collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    for ((tok, kind, hour), mut vs) in rows {
        vs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "{tok}\t{kind}\t{hour}\t{}\t{:.4}\t{:.4}\t{:.2}",
            vs.len(),
            p(&vs, 0.5),
            p(&vs, 0.9),
            vs.iter().sum::<f64>()
        );
    }
}

fn infinite_free_snapshot() -> FlashSnapshot {
    let wbnb = venues::wbnb_addr();
    let usdt = venues::usdt_addr();
    FlashSnapshot {
        block: 0,
        measured_at_unix: 0,
        states: vec![FlashSourceState {
            source: FlashSource::InfinityVault,
            fee_bps: Some(0),
            available: vec![(wbnb, U256::from(u128::MAX / 4)), (usdt, U256::from(u128::MAX / 4))],
            error: None,
        }],
    }
}

fn net_as_usdt(net: i128, quote: Address, mv: &MultiVenueMap) -> f64 {
    if net <= 0 {
        return 0.0;
    }
    let n = net as f64 / 1e18;
    if quote == venues::usdt_addr() {
        n
    } else if !mv.bridge_reserve_wbnb.is_zero() {
        let rw: f64 = u128::try_from(mv.bridge_reserve_wbnb).unwrap_or(0) as f64;
        let ru: f64 = u128::try_from(mv.bridge_reserve_usdt).unwrap_or(0) as f64;
        if rw > 0.0 {
            n * (ru / rw)
        } else {
            n * 600.0
        }
    } else {
        n * 600.0
    }
}

fn borrow_as_usdt(borrow: &U256, quote: Address, mv: &MultiVenueMap) -> f64 {
    let n: f64 = u128::try_from(*borrow).unwrap_or(0) as f64 / 1e18;
    if quote == venues::usdt_addr() {
        n
    } else if !mv.bridge_reserve_wbnb.is_zero() {
        let rw: f64 = u128::try_from(mv.bridge_reserve_wbnb).unwrap_or(0) as f64;
        let ru: f64 = u128::try_from(mv.bridge_reserve_usdt).unwrap_or(0) as f64;
        if rw > 0.0 {
            n * (ru / rw)
        } else {
            n * 600.0
        }
    } else {
        n * 600.0
    }
}

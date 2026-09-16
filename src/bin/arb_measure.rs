//! Cụm `planB-B0-complete` mục 4+5 — replay sim_arb trên mẫu swap lớn (log VPS)
//! và kiểm chéo revm (fork block−1) khi RPC còn state.
//! Không ký, không gửi.

use alloy::primitives::{Address, U256};
use bsc_sandwich::config::Config;
use bsc_sandwich::flash::{FlashSnapshot, FlashSource, FlashSourceState};
use bsc_sandwich::multivenue::MultiVenueMap;
use bsc_sandwich::sim_arb;
use bsc_sandwich::venues;
use serde_json::Value;
use std::path::Path;
use std::str::FromStr;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut jsonl = None;
    let mut mv_path = "state/multi_venue.json".to_string();
    let mut max_n: usize = 10_000;
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
            "--max" => {
                max_n = args[i + 1].parse().unwrap_or(10_000);
                i += 2;
            }
            _ => i += 1,
        }
    }
    let jsonl = match jsonl {
        Some(p) => p,
        None => {
            eprintln!("usage: arb_measure --jsonl FILE [--multi-venue PATH] [--max N]");
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
    println!("token\tpair\thash\tnet_usdt\tborrow\tsource\tsanity\tcluster\tts");
    let mut n_opp = 0u64;
    let mut nets: Vec<f64> = Vec::new();
    let mut borrows: Vec<f64> = Vec::new();
    let mut cluster_n = 0u64;
    let mut n_in = 0u64;
    for line in std::fs::read_to_string(&jsonl).unwrap_or_default().lines() {
        if n_in >= max_n as u64 {
            break;
        }
        let d: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let token = match d.get("token").and_then(|v| v.as_str()).and_then(|s| Address::from_str(s).ok()) {
            Some(t) => t,
            None => continue,
        };
        let Some(pools) = mv.arb_v2_pools(token) else {
            continue;
        };
        n_in += 1;
        let pair = d.get("pair").and_then(|v| v.as_str()).and_then(|s| Address::from_str(s).ok());
        let amount_in = d
            .get("amount_in")
            .and_then(|v| v.as_str())
            .and_then(|s| U256::from_str(s).ok())
            .unwrap_or(U256::ZERO);
        if amount_in.is_zero() {
            continue;
        }
        let victim_pool = match pair.and_then(|p| pools.iter().find(|x| x.pair == p).copied()) {
            Some(p) => p,
            None => pools[0],
        };
        let other = match pools.iter().find(|x| x.pair != victim_pool.pair).copied() {
            Some(p) => p,
            None => continue,
        };
        let after = match sim_arb::apply_victim_to_pool(victim_pool, amount_in, true) {
            Some(p) => p,
            None => continue,
        };
        let gas_wei = 0u128; // bang replay: tru gas o buoc sau neu can; day la so GROSS-vs-net cua math
        let max_for = |q: Address| cfg.arb_max_borrow_wei_for(q);
        let gas_for = |_q: Address| gas_wei;
        let Some((route, q)) = sim_arb::best_arb_for_pools(
            token,
            after,
            other,
            mv.bridge_pair,
            mv.bridge_reserve_wbnb,
            mv.bridge_reserve_usdt,
            wbnb,
            &max_for,
            &snap,
            &gas_for,
            cfg.bribe_pct_of_profit,
            Some((cfg.bribe_min_wei(), cfg.bribe_max_wei())),
        ) else {
            continue;
        };
        if !sim_arb::arb_sanity_ok(&route, &q) || q.net_wei <= 0 {
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
        println!(
            "{:#x}\t{:#x}\t{}\t{:.4}\t{:.4}\t{}\t1\t{}\t{}",
            token,
            victim_pool.pair,
            d.get("hash").and_then(|v| v.as_str()).unwrap_or(""),
            net_usdt,
            borrow_usdt,
            q.flash_source.as_str(),
            cluster as u8,
            d.get("ts").and_then(|v| v.as_str()).unwrap_or(""),
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
        "SUMMARY n_in_arb_ready={n_in} n_opp={n_opp} p50_usdt={:.4} p90_usdt={:.4} sum_usdt={:.2} borrow_p80={:.2} cluster_pct={:.1}",
        p(&nets, 0.5),
        p(&nets, 0.9),
        nets.iter().sum::<f64>(),
        p(&borrows, 0.8),
        if n_opp == 0 { 0.0 } else { 100.0 * cluster_n as f64 / n_opp as f64 },
    );
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

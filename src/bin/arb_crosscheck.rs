//! Cụm `planB-B0-complete` mục 4 — kiểm chéo sim_arb vs revm ≥20 case.
//! Fork block hiện tại, áp victim tổng hợp, 2 swap V2, so `route_out` vs
//! số token thật. Không ký, không gửi.

use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use bsc_sandwich::multivenue::MultiVenueMap;
use bsc_sandwich::sim_arb::{self, ArbRoute};
use bsc_sandwich::sim_evm;
use bsc_sandwich::transport;
use bsc_sandwich::venues;
use std::path::Path;
use std::str::FromStr;

#[tokio::main]
async fn main() {
    let _ = transport::load_dotenv_defaults(Path::new(".env"));
    let urls = transport::filter_read_urls(transport::collect_rpc_urls_from_env("BSC_HTTP_SIM"));
    let urls = if urls.is_empty() {
        transport::filter_read_urls(transport::collect_rpc_urls_from_env("BSC_HTTP"))
    } else {
        urls
    };
    if urls.is_empty() {
        eprintln!("MISSING BSC_HTTP");
        std::process::exit(1);
    }
    let provider = match ProviderBuilder::new().connect(&urls[0]).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("FAIL connect: {e}");
            std::process::exit(1);
        }
    };
    let block = provider.get_block_number().await.unwrap_or(1);
    let fork = block.saturating_sub(1);
    println!("arb_crosscheck chain_ok block={block} fork={fork}");
    let mv = MultiVenueMap::load_from_path(Path::new("state/multi_venue.json")).expect("multi_venue");
    let file: serde_json::Value = serde_json::from_str(&std::fs::read_to_string("state/multi_venue.json").unwrap()).unwrap();
    let mut n_ok = 0u32;
    let mut n_fail = 0u32;
    let mut n_skip = 0u32;
    println!("token\tdir\tborrow_bnb\tmath_token_out\trevm_buy_gas\trevm_sell_gas\tpct_diff\tok");
    for t in file["tokens"].as_array().unwrap() {
        if t["arb_ready"] != true {
            continue;
        }
        let token = Address::from_str(t["token"].as_str().unwrap()).unwrap();
        let Some(pools) = mv.arb_v2_pools(token) else {
            continue;
        };
        if pools.len() < 2 {
            continue;
        }
        // 2 huong x 2 co vay = 4 / token; 9 token = 36, lay 20 dau.
        // Uu tien vay WBNB (storage WBNB doc duoc tren RPC free; USDT getStorageAt
        // hay tra -32000 not supported — xem BAOCAO47).
        let ordered = if pools[0].quote == venues::wbnb_addr() {
            [(pools[0], pools[1]), (pools[1], pools[0])]
        } else {
            [(pools[1], pools[0]), (pools[0], pools[1])]
        };
        for (buy, sell) in ordered {
            if buy.quote != venues::wbnb_addr() {
                continue;
            }
            for borrow_bnb in [1u128, 5] {
                if n_ok + n_fail + n_skip >= 20 {
                    break;
                }
                let borrow = U256::from(borrow_bnb) * U256::from(10u64).pow(U256::from(17u64)); // 0.1 / 0.5
                let bridge = if buy.quote == sell.quote {
                    None
                } else if !mv.bridge_reserve_wbnb.is_zero() {
                    let (rin, rout) = if sell.quote == venues::wbnb_addr() {
                        (mv.bridge_reserve_wbnb, mv.bridge_reserve_usdt)
                    } else {
                        (mv.bridge_reserve_usdt, mv.bridge_reserve_wbnb)
                    };
                    Some(sim_arb::BridgeLeg { pair: mv.bridge_pair.unwrap_or(Address::ZERO), reserve_in: rin, reserve_out: rout })
                } else {
                    None
                };
                if buy.quote != sell.quote && bridge.is_none() {
                    n_skip += 1;
                    continue;
                }
                let route = ArbRoute {
                    token,
                    borrow_quote: buy.quote,
                    buy,
                    sell,
                    bridge,
                };
                let Some((_, _, math_final)) = sim_arb::route_out(&route, borrow) else {
                    n_skip += 1;
                    continue;
                };
                let p = provider.clone().erased();
                match sim_evm::simulate_arb_hops_evm(p, fork, buy.quote, token, sell.quote, borrow).await {
                    Ok((evm_final, gas)) => {
                        let math_u = u128::try_from(math_final).unwrap_or(0) as f64;
                        let evm_u = u128::try_from(evm_final).unwrap_or(0) as f64;
                        let pct = if math_u > 0.0 { (evm_u - math_u).abs() / math_u * 100.0 } else { 999.0 };
                        let ok = pct <= 2.0;
                        println!(
                            "{:#x}\tbuy={:#x}\t{}\t{}\t{}\t{}\t{:.4}\t{}",
                            token, buy.pair, borrow, math_final, evm_final, gas, pct, ok
                        );
                        if ok {
                            n_ok += 1;
                        } else {
                            n_fail += 1;
                        }
                    }
                    Err(e) => {
                        println!("{:#x}\tFAIL\t{borrow}\t{e}", token);
                        n_fail += 1;
                    }
                }
            }
        }
        if n_ok + n_fail >= 20 {
            break;
        }
    }
    println!("SUMMARY n_ok={n_ok} n_fail={n_fail} n_skip={n_skip} fork={fork}");
}

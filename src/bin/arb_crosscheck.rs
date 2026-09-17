//! Cụm `planB-B5-simarb-v3-measure` — kiểm chéo sim_arb mixed (V2+V3) vs revm
//! ≥15 case. Fork block−1, chạy hop SwapRouter V3 + pair V2, lệch ≤2%.
//! Không ký, không gửi.

use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use bsc_sandwich::discover_mv;
use bsc_sandwich::multivenue::MultiVenueMap;
use bsc_sandwich::sim_arb::{self, ArbVenue, MixedRoute};
use bsc_sandwich::sim_evm;
use bsc_sandwich::sim_v3;
use bsc_sandwich::transport;
use bsc_sandwich::venues;
use std::collections::HashSet;
use std::path::Path;

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
    let list_a: HashSet<Address> = std::fs::read_to_string("pairs_arb.txt")
        .map(|s| discover_mv::parse_pairs_tokens(&s).into_iter().map(|(a, _)| a).collect())
        .unwrap_or_default();
    let probe = U256::from(1_000_000_000_000_000_000u64);
    let mut n_ok = 0u32;
    let mut n_fail = 0u32;
    let mut n_skip = 0u32;
    let mut gases: Vec<u64> = Vec::new();
    println!("token\tkind\tborrow\tmath_out\trevm_out\tgas\tpct_diff\tok");

    for &token in &list_a {
        if n_ok + n_fail >= 15 {
            break;
        }
        let Some(mut venues) = mv.arb_mixed_venues(token) else {
            continue;
        };
        let p = provider.clone();
        for v in &mut venues {
            if let ArbVenue::V3(vp) = v {
                if vp.quote != venues::wbnb_addr() {
                    continue;
                }
                // Quote tai head (khong ghim fork): node public thuong khong
                // giu eth_call archive; lech 1 block vs revm fork-1.
                if let Err(e) = sim_v3::fit_arb_v3_pool(&p, token, vp, None, probe).await {
                    eprintln!("fit_fail token={token:#x} pool={:#x} fee={} {e}", vp.pool, vp.fee);
                }
            }
        }
        let v2s: Vec<_> = venues
            .iter()
            .copied()
            .filter(|v| matches!(v, ArbVenue::V2(p) if p.quote == venues::wbnb_addr() && v.is_fitted()))
            .collect();
        let v3s: Vec<_> = venues
            .iter()
            .copied()
            .filter(|v| matches!(v, ArbVenue::V3(p) if p.quote == venues::wbnb_addr() && v.is_fitted()))
            .collect();
        if v2s.is_empty() || v3s.is_empty() {
            n_skip += 1;
            continue;
        }
        let pairs = [(v2s[0], v3s[0]), (v3s[0], v2s[0])];
        for (buy, sell) in pairs {
            if n_ok + n_fail >= 15 {
                break;
            }
            let route = MixedRoute {
                token,
                borrow_quote: venues::wbnb_addr(),
                buy,
                sell,
                bridge: None,
            };
            let borrow = U256::from(100_000_000_000_000_000u64); // 0.1 BNB
            let Some((_, _, math_final)) = sim_arb::route_out_mixed(&route, borrow) else {
                n_skip += 1;
                continue;
            };
            let pp = provider.clone().erased();
            match sim_evm::simulate_arb_mixed_hops_evm(
                pp,
                fork,
                token,
                venues::wbnb_addr(),
                venues::wbnb_addr(),
                borrow,
                buy,
                sell,
            )
            .await
            {
                Ok((evm_final, gas)) => {
                    let math_u = u128::try_from(math_final).unwrap_or(0) as f64;
                    let evm_u = u128::try_from(evm_final).unwrap_or(0) as f64;
                    let pct = if math_u > 0.0 {
                        (evm_u - math_u).abs() / math_u * 100.0
                    } else {
                        999.0
                    };
                    let ok = pct <= 2.0;
                    println!(
                        "{:#x}\t{}\t{}\t{}\t{}\t{}\t{:.4}\t{}",
                        token,
                        sim_arb::route_kind(buy, sell),
                        borrow,
                        math_final,
                        evm_final,
                        gas,
                        pct,
                        ok
                    );
                    gases.push(gas);
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
    gases.sort();
    let p50 = if gases.is_empty() {
        0
    } else {
        gases[gases.len() / 2]
    };
    println!("SUMMARY n_ok={n_ok} n_fail={n_fail} n_skip={n_skip} fork={fork} gas_p50={p50}");
    if n_ok + n_fail > 0 {
        println!("gas_units_arb_v3_suggest={}", p50.saturating_add(80_000));
    }
}

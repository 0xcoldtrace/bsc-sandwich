//! Cụm `planB-B0-complete` mục 2 — sinh `state/multi_venue.json`.
//! Chỉ đọc chain (getPair/getReserves/getPool/getLogs). Không ký, không gửi.

use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder};
use bsc_sandwich::config::Config;
use bsc_sandwich::multivenue::{
    quote_name, InfinityPoolRec, MultiVenueFile, TokenVenues, V2PoolRec, V3PoolRec,
};
use bsc_sandwich::pool;
use bsc_sandwich::transport;
use bsc_sandwich::venues::{USDT_ADDRESS, V2_FACTORY_ADDRESS, V3_FACTORY_ADDRESS, WBNB_ADDRESS};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::main]
async fn main() {
    let _ = transport::load_dotenv_defaults(Path::new(".env"));
    let cfg = match Config::load(Path::new("config.toml")) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("FAIL load config.toml: {e}");
            std::process::exit(1);
        }
    };
    let urls = transport::filter_read_urls(transport::collect_rpc_urls_from_env("BSC_HTTP"));
    if urls.is_empty() {
        eprintln!("MISSING: BSC_HTTP rong");
        std::process::exit(1);
    }
    let provider = match ProviderBuilder::new().connect(&urls[0]).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("FAIL connect RPC: {e}");
            std::process::exit(1);
        }
    };
    let chain = provider.get_chain_id().await.unwrap_or(0);
    if chain != 56 {
        eprintln!("FAIL chain_id={chain} (can 56)");
        std::process::exit(1);
    }
    let block = provider.get_block_number().await.unwrap_or(0);
    println!("build_multi_venue chain=56 block={block} rpc_ok");

    let pairs_raw = std::fs::read_to_string(&cfg.pairs_path).unwrap_or_default();
    let tokens = parse_pairs_tokens(&pairs_raw);
    println!("pairs.txt tokens_parsed={}", tokens.len());

    let factory = Address::from_str(V2_FACTORY_ADDRESS).unwrap();
    let v3_factory = Address::from_str(V3_FACTORY_ADDRESS).unwrap();
    let wbnb = Address::from_str(WBNB_ADDRESS).unwrap();
    let usdt = Address::from_str(USDT_ADDRESS).unwrap();
    let min_w = cfg.min_reserve_wei();
    let min_u = cfg.min_reserve_usdt_wei();

    let mut token_recs: Vec<TokenVenues> = Vec::new();
    for (i, (token, symbol)) in tokens.iter().enumerate() {
        let mut v2_pools = Vec::new();
        for quote in [wbnb, usdt] {
            match pool::resolve_v2_pair_for_quote(&provider, factory, *token, quote).await {
                Ok(Ok(pair)) => match pool::get_reserves_vs_quote(&provider, pair, quote).await {
                    Ok((rq, rt)) => {
                        let min = if quote == wbnb { min_w } else { min_u };
                        v2_pools.push(V2PoolRec {
                            pair: format!("{pair:#x}"),
                            quote: format!("{quote:#x}"),
                            quote_name: quote_name(quote).to_string(),
                            reserve_quote: rq.to_string(),
                            reserve_token: rt.to_string(),
                            meets_min: rq >= min,
                            ok: rq >= min,
                        });
                    }
                    Err(e) => eprintln!("getReserves fail token={token:#x} quote={quote:#x}: {e}"),
                },
                Ok(Err(_)) => {}
                Err(e) => eprintln!("getPair fail token={token:#x} quote={quote:#x}: {e}"),
            }
        }
        let mut v3_pools = Vec::new();
        for quote in [wbnb, usdt] {
            match pool::resolve_v3_pools_for_quote(&provider, v3_factory, *token, quote).await {
                Ok(found) => {
                    for (pool_addr, fee) in found {
                        v3_pools.push(V3PoolRec {
                            pool: format!("{pool_addr:#x}"),
                            quote: format!("{quote:#x}"),
                            quote_name: quote_name(quote).to_string(),
                            fee,
                            impact_pct: None,
                            ok: false,
                        });
                    }
                }
                Err(e) => eprintln!("getPool v3 fail token={token:#x}: {e}"),
            }
        }
        let v2_ok = v2_pools.iter().any(|p| p.meets_min);
        let deep_n = v2_pools.iter().filter(|p| p.meets_min).count();
        let rec = TokenVenues {
            token: format!("{token:#x}"),
            symbol: symbol.clone(),
            arb_ready: deep_n >= 2,
            v2_pools,
            v3_pools,
            infinity_pools: Vec::new(),
            uni_v3_pools: Vec::new(),
            vol24h_bnb: None,
            v2_ok,
            v3_ok: false,
            both_ok: false,
            verified: None,
            proxy: None,
            from_pairs: true,
        };
        if (i + 1) % 20 == 0 || i + 1 == tokens.len() {
            println!("  progress {}/{} last={} v2={} arb_ready={}", i + 1, tokens.len(), rec.token, rec.v2_pools.len(), rec.arb_ready);
        }
        token_recs.push(rec);
    }

    // Infinity: 1 cửa sổ 5000 block, 2 lời gọi (CL + Bin), khớp token sau.
    let inf_to = block;
    let inf_from = block.saturating_sub(4_999);
    let mut inf_err = None;
    match pool::scan_infinity_initializes_window(&provider, inf_from, inf_to).await {
        Ok(matches) => {
            println!("infinity Initialize logs in {inf_from}-{inf_to}: {}", matches.len());
            let token_set: HashSet<Address> = tokens.iter().map(|(t, _)| *t).collect();
            let mut by_token: HashMap<Address, Vec<InfinityPoolRec>> = HashMap::new();
            for m in matches {
                let qname = if m.currency0 == wbnb || m.currency1 == wbnb {
                    "WBNB"
                } else if m.currency0 == usdt || m.currency1 == usdt {
                    "USDT"
                } else {
                    continue;
                };
                let other = if m.currency0 == wbnb || m.currency0 == usdt {
                    m.currency1
                } else {
                    m.currency0
                };
                if !token_set.contains(&other) {
                    continue;
                }
                by_token.entry(other).or_default().push(InfinityPoolRec {
                    family: m.family.as_str().to_string(),
                    pool_id: format!("{:#x}", m.pool_id),
                    hooks: format!("{:#x}", m.hooks),
                    fee: m.fee,
                    quote_name: qname.to_string(),
                    currency0: format!("{:#x}", m.currency0),
                    currency1: format!("{:#x}", m.currency1),
                });
            }
            for rec in &mut token_recs {
                if let Ok(addr) = Address::from_str(&rec.token) {
                    if let Some(v) = by_token.remove(&addr) {
                        rec.infinity_pools = v;
                    }
                }
            }
        }
        Err(e) => {
            inf_err = Some(e.clone());
            eprintln!("infinity scan FAIL (ghi nhan, khong crash): {e}");
        }
    }

    let (bridge_pair, br_w, br_u) = match pool::resolve_v2_pair_for_quote(&provider, factory, usdt, wbnb).await {
        Ok(Ok(p)) => match pool::get_reserves_vs_quote(&provider, p, wbnb).await {
            Ok((rw, ru)) => (Some(format!("{p:#x}")), Some(rw.to_string()), Some(ru.to_string())),
            Err(_) => (Some(format!("{p:#x}")), None, None),
        },
        _ => (None, None, None),
    };

    let ready = token_recs.iter().filter(|t| t.arb_ready).count();
    let file = MultiVenueFile {
        generated_at_unix: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        block,
        infinity_from_block: inf_from,
        infinity_to_block: inf_to,
        min_reserve_wbnb_wei: min_w.to_string(),
        min_reserve_usdt_wei: min_u.to_string(),
        bridge_wbnb_usdt_pair: bridge_pair,
        bridge_reserve_wbnb: br_w,
        bridge_reserve_usdt: br_u,
        tokens: token_recs,
        source: Some("build_multi_venue".into()),
        hours: None,
        scanned_tokens: None,
        v2_ok_count: None,
        v3_ok_count: None,
        both_ok_count: None,
        volume_method: None,
    };
    let out_path = Path::new(&cfg.multi_venue_path);
    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(&file).expect("serialize");
    std::fs::write(out_path, json).expect("write multi_venue.json");
    println!(
        "WROTE {} tokens={} arb_ready={} infinity_err={:?} machine=WSL",
        cfg.multi_venue_path,
        file.tokens.len(),
        ready,
        inf_err
    );
}

fn parse_pairs_tokens(content: &str) -> Vec<(Address, Option<String>)> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for raw in content.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let comment = raw.splitn(2, '#').nth(1).unwrap_or("");
        let symbol = comment.split('|').next().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let addr_part = line.split(',').next().unwrap_or("").trim();
        if let Ok(t) = Address::from_str(addr_part) {
            if seen.insert(t) {
                out.push((t, symbol));
            }
        }
    }
    out
}

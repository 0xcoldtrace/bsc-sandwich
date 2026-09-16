//! Cụm `planB-B4-multivenue-tool` — TOOL sinh list đa venue MỚI.
//!
//!   cargo run --release --bin discover_multivenue -- \
//!     --hours 24 --top 500 --min-v2-bnb 50 --min-v3-impact-pct 2 --probe-bnb 1 \
//!     --out state/multi_venue.json
//!
//! Chỉ ĐỌC chain. Không ký, không gửi.
//! Nguồn chính: Swap log PCS V2 + PCS V3. Bổ sung `pairs.txt` nếu token có V3
//! (PCS V3 hoặc Uniswap V3). Venue: PCS V2, PCS V3, Uniswap V3.

use alloy::eips::BlockNumberOrTag;
use alloy::primitives::{keccak256, Address, B256, U256};
use alloy::providers::{DynProvider, Provider, ProviderBuilder};
use alloy::rpc::types::eth::Filter;
use bsc_sandwich::config::Config;
use bsc_sandwich::discover_mv::{
    abs_i256_word, candidates_line, decode_v2_swap_quote_volume, impact_pct_from_quotes,
    parse_pairs_tokens, probe_ten_tokens, probe_usdt_from_bnb, quote_name, report_header, report_row,
    DiscoverThresholds, TokenProbe, UniV3Obs, V2Obs, V3Obs, TRANSFER_EVENT_SIG, V2_SWAP_EVENT_SIG,
    V3_SWAP_EVENT_SIG,
};
use bsc_sandwich::multivenue::{
    MultiVenueFile, TokenVenues, UniV3PoolRec, V2PoolRec, V3PoolRec,
};
use bsc_sandwich::pool;
use bsc_sandwich::sim_v3;
use bsc_sandwich::transport;
use bsc_sandwich::venues::{
    USDT_ADDRESS, UNI_V3_FACTORY_ADDRESS, UNI_V3_FEE_TIERS, UNI_V3_QUOTER_V2_ADDRESS,
    V2_FACTORY_ADDRESS, V3_FACTORY_ADDRESS, V3_QUOTER_V2_ADDRESS, WBNB_ADDRESS,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone)]
struct Args {
    hours: u64,
    top: usize,
    min_v2_bnb: f64,
    min_v2_usdt: f64,
    min_v3_impact_pct: f64,
    probe_bnb: f64,
    out: String,
    report: String,
    candidates: String,
    chunk: u64,
    sleep_ms: u64,
    probe_ten: bool,
    skip_volume: bool,
    pairs: String,
}

fn parse_args() -> Args {
    let raw: Vec<String> = std::env::args().collect();
    let mut a = Args {
        hours: 24,
        top: 500,
        min_v2_bnb: 50.0,
        min_v2_usdt: 35_000.0,
        min_v3_impact_pct: 2.0,
        probe_bnb: 1.0,
        out: "state/multi_venue.json".into(),
        report: "state/multi_venue_report.tsv".into(),
        candidates: "state/multi_venue_candidates.txt".into(),
        chunk: 1000,
        sleep_ms: 80,
        probe_ten: false,
        skip_volume: false,
        pairs: String::new(),
    };
    let mut i = 1;
    while i < raw.len() {
        match raw[i].as_str() {
            "--hours" => {
                a.hours = raw[i + 1].parse().unwrap_or(a.hours);
                i += 2;
            }
            "--top" => {
                a.top = raw[i + 1].parse().unwrap_or(a.top);
                i += 2;
            }
            "--min-v2-bnb" => {
                a.min_v2_bnb = raw[i + 1].parse().unwrap_or(a.min_v2_bnb);
                i += 2;
            }
            "--min-v2-usdt" => {
                a.min_v2_usdt = raw[i + 1].parse().unwrap_or(a.min_v2_usdt);
                i += 2;
            }
            "--min-v3-impact-pct" => {
                a.min_v3_impact_pct = raw[i + 1].parse().unwrap_or(a.min_v3_impact_pct);
                i += 2;
            }
            "--probe-bnb" => {
                a.probe_bnb = raw[i + 1].parse().unwrap_or(a.probe_bnb);
                i += 2;
            }
            "--out" => {
                a.out = raw[i + 1].clone();
                i += 2;
            }
            "--report" => {
                a.report = raw[i + 1].clone();
                i += 2;
            }
            "--candidates" => {
                a.candidates = raw[i + 1].clone();
                i += 2;
            }
            "--chunk" => {
                a.chunk = raw[i + 1].parse().unwrap_or(a.chunk);
                i += 2;
            }
            "--sleep-ms" => {
                a.sleep_ms = raw[i + 1].parse().unwrap_or(a.sleep_ms);
                i += 2;
            }
            "--probe-ten" => {
                a.probe_ten = true;
                i += 1;
            }
            "--skip-volume" => {
                a.skip_volume = true;
                i += 1;
            }
            "--pairs" => {
                a.pairs = raw[i + 1].clone();
                i += 2;
            }
            "-h" | "--help" => {
                eprintln!(
                    "discover_multivenue --hours 24 --top 500 --min-v2-bnb 50 --min-v3-impact-pct 2 --probe-bnb 1 --out state/multi_venue.json"
                );
                std::process::exit(0);
            }
            _ => i += 1,
        }
    }
    a
}

fn addr(s: &str) -> Address {
    Address::from_str(s).expect("address pin")
}

async fn connect(urls: &[String]) -> Option<(String, DynProvider)> {
    for u in urls {
        match ProviderBuilder::new().connect(u).await {
            Ok(p) => match p.get_chain_id().await {
                Ok(56) => return Some((transport::redact_rpc_url(u), p.erased())),
                Ok(id) => eprintln!("skip rpc chain_id={id} url={}", transport::redact_rpc_url(u)),
                Err(e) => eprintln!("skip rpc chainId err={e}"),
            },
            Err(e) => eprintln!("skip rpc connect err={e} url={}", transport::redact_rpc_url(u)),
        }
    }
    None
}

async fn get_logs_retry(provider: &DynProvider, filter: &Filter, sleep_ms: u64) -> Result<Vec<alloy::rpc::types::eth::Log>, String> {
    let mut last = String::new();
    for attempt in 0..5u32 {
        match tokio::time::timeout(Duration::from_secs(20), provider.get_logs(filter)).await {
            Ok(Ok(logs)) => return Ok(logs),
            Ok(Err(e)) => {
                last = e.to_string();
            }
            Err(_) => {
                return Err("timeout 20s".into());
            }
        }
        let rl = last.contains("-32005")
            || last.contains("limit exceeded")
            || last.contains("429")
            || last.contains("too many")
            || last.contains("query returned more")
            || last.contains("timeout")
            || last.contains("range is too large");
        if rl {
            let wait = sleep_ms.saturating_mul(2u64.pow(attempt)).min(8_000);
            tokio::time::sleep(Duration::from_millis(wait)).await;
            continue;
        }
        return Err(last);
    }
    Err(format!("het retry getLogs: {last}"))
}

fn topic0(sig: &str) -> B256 {
    keccak256(sig.as_bytes())
}

fn topic_to_addr(t: &B256) -> Address {
    Address::from_slice(&t.as_slice()[12..32])
}

async fn block_timestamp(provider: &DynProvider, n: u64) -> Result<u64, String> {
    let b = provider
        .get_block_by_number(BlockNumberOrTag::Number(n))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("block {n} missing"))?;
    Ok(b.header.timestamp)
}

async fn from_block_hours_ago(provider: &DynProvider, latest: u64, hours: u64) -> u64 {
    let ts = match block_timestamp(provider, latest).await {
        Ok(t) => t,
        Err(_) => return latest.saturating_sub(hours.saturating_mul(3_200)), // ~0.45s fallback ~8000 blk/h? 3600/0.45=8000
    };
    let sample_n = latest.saturating_sub(2_000);
    let sample_ts = block_timestamp(provider, sample_n).await.unwrap_or(ts.saturating_sub(900));
    let dt = ts.saturating_sub(sample_ts).max(1) as f64;
    let secs_per = dt / 2_000.0_f64.max(1.0);
    let need = ((hours as f64) * 3600.0 / secs_per.max(0.2)) as u64;
    println!("block_time_est={secs_per:.3}s/block hours={hours} blocks_back={need}");
    latest.saturating_sub(need)
}

#[derive(Clone)]
struct PoolCache {
    token0: Address,
    token1: Address,
    factory: Address,
}

async fn resolve_one(provider: DynProvider, pool: Address, wbnb_a: Address, usdt_a: Address) -> (Address, Option<PoolCache>) {
    let Ok((t0, t1)) = pool::get_pair_tokens(&provider, pool).await else {
        return (pool, None);
    };
    if t0 != wbnb_a && t1 != wbnb_a && t0 != usdt_a && t1 != usdt_a {
        return (pool, Some(PoolCache { token0: t0, token1: t1, factory: Address::ZERO }));
    }
    let Ok(factory) = pool::get_factory(&provider, pool).await else {
        return (pool, None);
    };
    (pool, Some(PoolCache { token0: t0, token1: t1, factory }))
}

async fn warm_pool_cache(
    provider: &DynProvider,
    pools: impl IntoIterator<Item = Address>,
    cache: &mut HashMap<Address, Option<PoolCache>>,
    wbnb_a: Address,
    usdt_a: Address,
) {
    let mut seen = HashSet::new();
    let unknown: Vec<Address> = pools
        .into_iter()
        .filter(|p| seen.insert(*p) && !cache.contains_key(p))
        .collect();
    if unknown.is_empty() {
        return;
    }
    eprintln!("    warm {} pool meta (cache already {})", unknown.len(), cache.len());
    let mut set = tokio::task::JoinSet::new();
    let mut inflight = 0usize;
    let mut it = unknown.into_iter();
    loop {
        while inflight < 10 {
            let Some(p) = it.next() else { break };
            let prov = provider.clone();
            set.spawn(async move { resolve_one(prov, p, wbnb_a, usdt_a).await });
            inflight += 1;
        }
        if inflight == 0 {
            break;
        }
        match set.join_next().await {
            Some(Ok((p, rec))) => {
                cache.insert(p, rec);
                inflight -= 1;
            }
            Some(Err(_)) => inflight = inflight.saturating_sub(1),
            None => break,
        }
    }
}

/// Volume 24h theo token (wei quote quy đổi WBNB). Trả (map, method, unique_pools, logs).
async fn scan_volume(
    provider: &DynProvider,
    from_block: u64,
    to_block: u64,
    chunk0: u64,
    sleep_ms: u64,
    v2_factory: Address,
    v3_factory: Address,
    wbnb_a: Address,
    usdt_a: Address,
    bridge_wbnb: U256,
    bridge_usdt: U256,
) -> (HashMap<Address, U256>, String, u64, u64) {
    let mut vol: HashMap<Address, U256> = HashMap::new();
    let mut cache: HashMap<Address, Option<PoolCache>> = HashMap::new();
    let mut n_logs = 0u64;
    let n_pools;

    let v2_topic = topic0(V2_SWAP_EVENT_SIG);
    let v3_topic = topic0(V3_SWAP_EVENT_SIG);

    // Probe: 1 chunk Swap V2 không lọc address.
    let probe_hi = to_block;
    let probe_lo = to_block.saturating_sub(chunk0.saturating_sub(1).min(50));
    let fprobe = Filter::new().event_signature(v2_topic).from_block(probe_lo).to_block(probe_hi);
    let swap_ok = match get_logs_retry(provider, &fprobe, sleep_ms).await {
        Ok(logs) => {
            println!("swap_unfiltered_probe logs={} range={probe_lo}-{probe_hi} -> DUNG Swap log", logs.len());
            true
        }
        Err(e) => {
            println!("swap_unfiltered_probe FAIL ({e}) -> fallback Transfer WBNB+USDT (van loc factory PCS)");
            false
        }
    };

    if swap_ok {
        let mut method = "swap_logs_v2_v3".to_string();
        for (label, topic, is_v3) in [("v2", v2_topic, false), ("v3", v3_topic, true)] {
            let mut cursor = to_block;
            let mut chunk = chunk0;
            while cursor >= from_block {
                let lo = cursor.saturating_sub(chunk.saturating_sub(1)).max(from_block);
                eprintln!("  {label} getLogs [{lo}-{cursor}] chunk={chunk} vol_tokens={} cache={}", vol.len(), cache.len());
                let f = Filter::new().event_signature(topic).from_block(lo).to_block(cursor);
                match get_logs_retry(provider, &f, sleep_ms).await {
                    Ok(logs) => {
                        n_logs += logs.len() as u64;
                        eprintln!("    -> {} logs (total {n_logs})", logs.len());
                        let mut uniq = HashSet::new();
                        let pools: Vec<Address> = logs.iter().map(|l| l.address()).filter(|a| uniq.insert(*a)).collect();
                        warm_pool_cache(provider, pools, &mut cache, wbnb_a, usdt_a).await;
                        for l in &logs {
                            let pool = l.address();
                            let Some(Some(meta)) = cache.get(&pool) else { continue };
                            let pcs = meta.factory == v2_factory || meta.factory == v3_factory;
                            if !pcs {
                                continue;
                            }
                            let token0_is_wbnb = meta.token0 == wbnb_a;
                            let token1_is_wbnb = meta.token1 == wbnb_a;
                            let token0_is_usdt = meta.token0 == usdt_a;
                            let token1_is_usdt = meta.token1 == usdt_a;
                            let (token, quote_is_wbnb, token0_is_quote) = if token0_is_wbnb {
                                (meta.token1, true, true)
                            } else if token1_is_wbnb {
                                (meta.token0, true, false)
                            } else if token0_is_usdt {
                                (meta.token1, false, true)
                            } else if token1_is_usdt {
                                (meta.token0, false, false)
                            } else {
                                continue;
                            };
                            if token == wbnb_a || token == usdt_a {
                                continue;
                            }
                            let data = l.data().data.as_ref();
                            let qvol = if is_v3 {
                                if data.len() >= 64 {
                                    let a0 = abs_i256_word(&data[0..32]).unwrap_or(U256::ZERO);
                                    let a1 = abs_i256_word(&data[32..64]).unwrap_or(U256::ZERO);
                                    if token0_is_quote { a0 } else { a1 }
                                } else {
                                    U256::ZERO
                                }
                            } else {
                                decode_v2_swap_quote_volume(data, token0_is_quote).unwrap_or(U256::ZERO)
                            };
                            let as_wbnb = if quote_is_wbnb {
                                qvol
                            } else if !bridge_usdt.is_zero() {
                                qvol.checked_mul(bridge_wbnb).and_then(|x| x.checked_div(bridge_usdt)).unwrap_or(U256::ZERO)
                            } else {
                                U256::ZERO
                            };
                            *vol.entry(token).or_insert(U256::ZERO) = vol.get(&token).copied().unwrap_or(U256::ZERO).saturating_add(as_wbnb);
                        }
                        if lo == from_block {
                            break;
                        }
                        cursor = lo.saturating_sub(1);
                        chunk = chunk0;
                    }
                    Err(e) => {
                        if chunk <= 20 {
                            eprintln!("  {label} getLogs [{lo}-{cursor}] FAIL chunk nho: {e}");
                            if lo == from_block {
                                break;
                            }
                            cursor = lo.saturating_sub(1);
                        } else {
                            chunk = (chunk / 2).max(20);
                            eprintln!("  {label} split chunk -> {chunk} ({e})");
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
            }
            println!("  scanned {label} swap logs_total={n_logs} vol_tokens={}", vol.len());
        }
        n_pools = cache.values().filter(|v| v.as_ref().map(|m| m.factory == v2_factory || m.factory == v3_factory).unwrap_or(false)).count() as u64;
        if vol.is_empty() {
            method = "swap_logs_empty_then_transfer".into();
            let (v2, _m, p, l) = scan_volume_transfer(provider, from_block, to_block, chunk0, sleep_ms, v2_factory, v3_factory, wbnb_a, usdt_a, bridge_wbnb, bridge_usdt).await;
            return (v2, method, p, n_logs + l);
        }
        return (vol, method, n_pools, n_logs);
    }

    scan_volume_transfer(provider, from_block, to_block, chunk0, sleep_ms, v2_factory, v3_factory, wbnb_a, usdt_a, bridge_wbnb, bridge_usdt).await
}

async fn scan_volume_transfer(
    provider: &DynProvider,
    from_block: u64,
    to_block: u64,
    chunk0: u64,
    sleep_ms: u64,
    v2_factory: Address,
    v3_factory: Address,
    wbnb_a: Address,
    usdt_a: Address,
    bridge_wbnb: U256,
    bridge_usdt: U256,
) -> (HashMap<Address, U256>, String, u64, u64) {
    let t0 = topic0(TRANSFER_EVENT_SIG);
    let mut counterparty: HashMap<Address, U256> = HashMap::new();
    let mut n_logs = 0u64;

    for token in [wbnb_a, usdt_a] {
        let mut cursor = to_block;
        let mut chunk = chunk0;
        while cursor >= from_block {
            let lo = cursor.saturating_sub(chunk.saturating_sub(1)).max(from_block);
            let f = Filter::new().address(token).event_signature(t0).from_block(lo).to_block(cursor);
            match get_logs_retry(provider, &f, sleep_ms).await {
                Ok(logs) => {
                    n_logs += logs.len() as u64;
                    for l in &logs {
                        let topics = l.topics();
                        if topics.len() < 3 {
                            continue;
                        }
                        let from = topic_to_addr(&topics[1]);
                        let to = topic_to_addr(&topics[2]);
                        let amt = U256::from_be_slice(l.data().data.as_ref());
                        let as_wbnb = if token == wbnb_a {
                            amt
                        } else if !bridge_usdt.is_zero() {
                            amt.checked_mul(bridge_wbnb).and_then(|x| x.checked_div(bridge_usdt)).unwrap_or(U256::ZERO)
                        } else {
                            U256::ZERO
                        };
                        *counterparty.entry(from).or_insert(U256::ZERO) = counterparty.get(&from).copied().unwrap_or(U256::ZERO).saturating_add(as_wbnb);
                        *counterparty.entry(to).or_insert(U256::ZERO) = counterparty.get(&to).copied().unwrap_or(U256::ZERO).saturating_add(as_wbnb);
                    }
                    if lo == from_block {
                        break;
                    }
                    cursor = lo.saturating_sub(1);
                    chunk = chunk0;
                }
                Err(e) => {
                    if chunk <= 20 {
                        eprintln!("  Transfer {token:#x} [{lo}-{cursor}] FAIL: {e}");
                        if lo == from_block {
                            break;
                        }
                        cursor = lo.saturating_sub(1);
                    } else {
                        chunk = (chunk / 2).max(20);
                        eprintln!("  Transfer split chunk -> {chunk} ({e})");
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        }
        println!("  Transfer {:#x} logs_so_far={n_logs} counterparties={}", token, counterparty.len());
    }

    let mut ranked: Vec<(Address, U256)> = counterparty.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1));
    ranked.truncate(4_000);
    println!("  factory() tren top {} counterparties", ranked.len());

    let mut vol: HashMap<Address, U256> = HashMap::new();
    let mut n_pools = 0u64;
    let mut cache: HashMap<Address, Option<PoolCache>> = HashMap::new();
    warm_pool_cache(provider, ranked.iter().map(|(a, _)| *a), &mut cache, wbnb_a, usdt_a).await;
    for (i, (cp, amt)) in ranked.iter().enumerate() {
        if *cp == wbnb_a || *cp == usdt_a {
            continue;
        }
        let Some(Some(meta)) = cache.get(cp) else { continue };
        if meta.factory != v2_factory && meta.factory != v3_factory {
            continue;
        }
        n_pools += 1;
        let token = if meta.token0 == wbnb_a || meta.token0 == usdt_a {
            meta.token1
        } else if meta.token1 == wbnb_a || meta.token1 == usdt_a {
            meta.token0
        } else {
            continue;
        };
        if token == wbnb_a || token == usdt_a {
            continue;
        }
        *vol.entry(token).or_insert(U256::ZERO) = vol.get(&token).copied().unwrap_or(U256::ZERO).saturating_add(*amt);
        if (i + 1) % 200 == 0 {
            println!("    meta {}/{} pcs_pools={n_pools} tokens={}", i + 1, ranked.len(), vol.len());
        }
    }
    (vol, "transfer_wbnb_usdt_then_factory_pcs".into(), n_pools, n_logs)
}

async fn quote_impact(
    provider: &DynProvider,
    quoter: Address,
    quote: Address,
    token: Address,
    fee: u32,
    probe_in: U256,
) -> Option<f64> {
    let spot_in = (probe_in / U256::from(1_000u64)).max(U256::from(1_000_000_000_000u64)); // ≥ 1e12
    let out_spot = sim_v3::quote_exact_input_single(provider, quoter, quote, token, fee, spot_in).await.ok()?;
    let out_probe = sim_v3::quote_exact_input_single(provider, quoter, quote, token, fee, probe_in).await.ok()?;
    impact_pct_from_quotes(out_spot, spot_in, out_probe, probe_in)
}

async fn probe_token(
    provider: &DynProvider,
    token: Address,
    vol24h_bnb: f64,
    th: &DiscoverThresholds,
    v2_factory: Address,
    v3_factory: Address,
    uni_factory: Address,
    pcs_quoter: Address,
    uni_quoter: Address,
    wbnb_a: Address,
    usdt_a: Address,
    probe_usdt: U256,
    sleep_ms: u64,
) -> TokenProbe {
    let mut v2 = Vec::new();
    let mut v3 = Vec::new();
    let mut uni_v3 = Vec::new();
    let symbol = pool::erc20_symbol(provider, token).await.ok();

    for quote in [wbnb_a, usdt_a] {
        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        match pool::resolve_v2_pair_for_quote(provider, v2_factory, token, quote).await {
            Ok(Ok(pair)) => match pool::get_reserves_vs_quote(provider, pair, quote).await {
                Ok((rq, rt)) => {
                    let ok = bsc_sandwich::discover_mv::v2_meets_min(rq, quote, th);
                    v2.push(V2Obs { quote, pair, reserve_quote: rq, reserve_token: rt, ok });
                }
                Err(e) => eprintln!("  getReserves fail token={token:#x} quote={quote:#x}: {e}"),
            },
            Ok(Err(_)) => {}
            Err(e) => eprintln!("  getPair fail token={token:#x}: {e}"),
        }
    }

    for quote in [wbnb_a, usdt_a] {
        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        match pool::resolve_v3_pools_for_quote(provider, v3_factory, token, quote).await {
            Ok(found) => {
                let probe_in = if quote == wbnb_a { th.probe_bnb_wei } else { probe_usdt };
                for (pool_addr, fee) in found {
                    tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
                    let impact = quote_impact(provider, pcs_quoter, quote, token, fee, probe_in).await;
                    let ok_impact = bsc_sandwich::discover_mv::v3_meets_impact(impact, th);
                    v3.push(V3Obs { quote, pool: pool_addr, fee, impact_pct: impact, ok: ok_impact });
                }
            }
            Err(e) => eprintln!("  getPool pcs v3 fail token={token:#x}: {e}"),
        }
    }

    for quote in [wbnb_a, usdt_a] {
        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        match pool::resolve_v3_pools_for_quote_tiers(provider, uni_factory, token, quote, &UNI_V3_FEE_TIERS).await {
            Ok(found) => {
                let probe_in = if quote == wbnb_a { th.probe_bnb_wei } else { probe_usdt };
                for (pool_addr, fee) in found {
                    tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
                    let impact = quote_impact(provider, uni_quoter, quote, token, fee, probe_in).await;
                    let ok = bsc_sandwich::discover_mv::v3_meets_impact(impact, th);
                    uni_v3.push(UniV3Obs { quote, pool: pool_addr, fee, impact_pct: impact, ok });
                }
            }
            Err(e) => eprintln!("  getPool uni v3 fail token={token:#x}: {e}"),
        }
    }

    TokenProbe {
        token,
        symbol,
        vol24h_bnb,
        v2,
        v3,
        uni_v3,
        verified: None,
        proxy: None,
        from_pairs: false,
    }
}

fn etherscan_source(addr: Address, key: &str) -> Option<(bool, bool)> {
    let url = format!(
        "https://api.etherscan.io/v2/api?chainid=56&module=contract&action=getsourcecode&address={addr:#x}&apikey={key}"
    );
    let out = std::process::Command::new("curl")
        .args(["-sS", "--max-time", "20", &url])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let item = v.get("result")?.as_array()?.first()?;
    let src = item.get("SourceCode").and_then(|x| x.as_str()).unwrap_or("");
    let proxy = item.get("Proxy").and_then(|x| x.as_str()).unwrap_or("0");
    Some((!src.is_empty() && src != "0x", proxy == "1"))
}

fn wei_to_bnb_f64(w: U256) -> f64 {
    let s = w.to_string();
    let v: f64 = s.parse().unwrap_or(0.0);
    v / 1e18
}

fn to_file_rec(p: &TokenProbe) -> TokenVenues {
    TokenVenues {
        token: format!("{:#x}", p.token),
        symbol: p.symbol.clone(),
        arb_ready: p.keep_in_list(),
        v2_pools: p
            .v2
            .iter()
            .map(|x| V2PoolRec {
                pair: format!("{:#x}", x.pair),
                quote: format!("{:#x}", x.quote),
                quote_name: quote_name(x.quote).to_string(),
                reserve_quote: x.reserve_quote.to_string(),
                reserve_token: x.reserve_token.to_string(),
                meets_min: x.ok,
                ok: x.ok,
            })
            .collect(),
        v3_pools: p
            .v3
            .iter()
            .map(|x| V3PoolRec {
                pool: format!("{:#x}", x.pool),
                quote: format!("{:#x}", x.quote),
                quote_name: quote_name(x.quote).to_string(),
                fee: x.fee,
                impact_pct: x.impact_pct,
                ok: x.ok,
            })
            .collect(),
        infinity_pools: Vec::new(),
        uni_v3_pools: p
            .uni_v3
            .iter()
            .map(|x| UniV3PoolRec {
                pool: format!("{:#x}", x.pool),
                quote: format!("{:#x}", x.quote),
                quote_name: quote_name(x.quote).to_string(),
                fee: x.fee,
                impact_pct: x.impact_pct,
                ok: x.ok,
            })
            .collect(),
        vol24h_bnb: Some(p.vol24h_bnb),
        v2_ok: p.v2_ok(),
        v3_ok: p.v3_ok(),
        both_ok: p.both_ok(),
        verified: p.verified,
        proxy: p.proxy,
        from_pairs: p.from_pairs,
    }
}

#[tokio::main]
async fn main() {
    let args = parse_args();
    let _ = transport::load_dotenv_defaults(Path::new(".env"));
    let cfg = match Config::load(Path::new("config.toml")) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("FAIL load config.toml: {e}");
            std::process::exit(1);
        }
    };
    let min_v2_bnb = if std::env::args().any(|x| x == "--min-v2-bnb") { args.min_v2_bnb } else { cfg.multivenue_min_v2_bnb };
    let min_v2_usdt = if std::env::args().any(|x| x == "--min-v2-usdt") { args.min_v2_usdt } else { cfg.multivenue_min_v2_usdt };
    let min_v3 = if std::env::args().any(|x| x == "--min-v3-impact-pct") { args.min_v3_impact_pct } else { cfg.multivenue_min_v3_impact_pct };
    let probe_bnb = if std::env::args().any(|x| x == "--probe-bnb") { args.probe_bnb } else { cfg.multivenue_probe_bnb };
    let th = DiscoverThresholds::from_cli(min_v2_bnb, min_v2_usdt, min_v3, probe_bnb);

    let urls = transport::filter_read_urls(transport::collect_rpc_urls_from_env("BSC_HTTP"));
    if urls.is_empty() {
        eprintln!("MISSING: BSC_HTTP rong");
        std::process::exit(1);
    }
    let Some((rpc_redact, provider)) = connect(&urls).await else {
        eprintln!("FAIL connect RPC chain 56");
        std::process::exit(1);
    };
    let chain = provider.get_chain_id().await.unwrap_or(0);
    if chain != 56 {
        eprintln!("FAIL chain_id={chain}");
        std::process::exit(1);
    }
    let latest = provider.get_block_number().await.unwrap_or(0);
    println!("discover_multivenue chain=56 block={latest} rpc={rpc_redact} hours={} top={} min_v2_bnb={min_v2_bnb} min_v3_impact={min_v3} probe_bnb={probe_bnb}", args.hours, args.top);

    let v2_factory = addr(V2_FACTORY_ADDRESS);
    let v3_factory = addr(V3_FACTORY_ADDRESS);
    let uni_factory = addr(UNI_V3_FACTORY_ADDRESS);
    let pcs_quoter = addr(V3_QUOTER_V2_ADDRESS);
    let uni_quoter = addr(UNI_V3_QUOTER_V2_ADDRESS);
    let wbnb_a = addr(WBNB_ADDRESS);
    let usdt_a = addr(USDT_ADDRESS);

    let (bridge_pair, br_w, br_u) = match pool::resolve_v2_pair_for_quote(&provider, v2_factory, usdt_a, wbnb_a).await {
        Ok(Ok(p)) => match pool::get_reserves_vs_quote(&provider, p, wbnb_a).await {
            Ok((rw, ru)) => (Some(p), rw, ru),
            Err(_) => (Some(p), U256::ZERO, U256::ZERO),
        },
        _ => (None, U256::ZERO, U256::ZERO),
    };
    println!("bridge WBNB/USDT pair={:?} reserve_wbnb={} reserve_usdt={}", bridge_pair.map(|p| format!("{p:#x}")), br_w, br_u);
    let probe_usdt = probe_usdt_from_bnb(th.probe_bnb_wei, br_w, br_u).unwrap_or(U256::ZERO);
    println!("probe_usdt_wei={probe_usdt} (1 BNB quy doi)");

    let started = Instant::now();

    if args.probe_ten {
        println!("=== probe 5 blue-chip + 5 mid-cap (RPC that, khong dung pairs.txt) ===");
        for (kind, sym, hex) in probe_ten_tokens() {
            let token = addr(hex);
            let p = probe_token(&provider, token, 0.0, &th, v2_factory, v3_factory, uni_factory, pcs_quoter, uni_quoter, wbnb_a, usdt_a, probe_usdt, args.sleep_ms).await;
            let v2s: Vec<String> = p.v2.iter().map(|x| format!("{} res={} ok={}", quote_name(x.quote), x.reserve_quote, x.ok)).collect();
            let v3s: Vec<String> = p.v3.iter().map(|x| format!("{}@{} impact={:?} ok={}", quote_name(x.quote), x.fee, x.impact_pct, x.ok)).collect();
            let unis: Vec<String> = p.uni_v3.iter().map(|x| format!("{}@{} impact={:?}", quote_name(x.quote), x.fee, x.impact_pct)).collect();
            println!(
                "{kind} {sym} {hex} symbol={:?} v2_ok={} v3_ok={} both_ok={} v2=[{}] v3=[{}] uni=[{}]",
                p.symbol.as_deref().unwrap_or("?"),
                p.v2_ok(),
                p.v3_ok(),
                p.both_ok(),
                v2s.join("; "),
                v3s.join("; "),
                unis.join("; ")
            );
        }
        if args.skip_volume {
            println!("--skip-volume: dung sau probe-ten. elapsed={:.1}s", started.elapsed().as_secs_f64());
            return;
        }
    }

    let from_block = from_block_hours_ago(&provider, latest, args.hours).await;
    println!("scan blocks {from_block}..{latest} ({} blocks)", latest.saturating_sub(from_block).saturating_add(1));

    let (vol_map, volume_method, n_pools, n_logs) = scan_volume(
        &provider, from_block, latest, args.chunk, args.sleep_ms, v2_factory, v3_factory, wbnb_a, usdt_a, br_w, br_u,
    )
    .await;
    println!("volume_method={volume_method} unique_tokens={} pcs_pools_seen={n_pools} logs={n_logs}", vol_map.len());

    let mut ranked: Vec<(Address, U256)> = vol_map.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1));
    let scanned_tokens = ranked.len();
    ranked.truncate(args.top);
    println!("probe top {} tokens (volume)", ranked.len());

    let mut pairs_set: HashSet<Address> = HashSet::new();
    let mut pairs_sym: HashMap<Address, String> = HashMap::new();
    if !args.pairs.is_empty() {
        match std::fs::read_to_string(&args.pairs) {
            Ok(s) => {
                let parsed = parse_pairs_tokens(&s);
                println!("pairs.txt {} unique tokens (bo sung neu co V3)", parsed.len());
                for (t, sym) in parsed {
                    pairs_set.insert(t);
                    if let Some(sy) = sym {
                        pairs_sym.insert(t, sy);
                    }
                }
            }
            Err(e) => eprintln!("FAIL doc {}: {e}", args.pairs),
        }
    }

    let mut probes: Vec<TokenProbe> = Vec::new();
    let mut seen_probe: HashSet<Address> = HashSet::new();
    for (i, (token, vol_wei)) in ranked.iter().enumerate() {
        let vol_bnb = wei_to_bnb_f64(*vol_wei);
        let mut p = probe_token(&provider, *token, vol_bnb, &th, v2_factory, v3_factory, uni_factory, pcs_quoter, uni_quoter, wbnb_a, usdt_a, probe_usdt, args.sleep_ms).await;
        p.from_pairs = pairs_set.contains(token);
        if p.symbol.is_none() {
            p.symbol = pairs_sym.get(token).cloned();
        }
        seen_probe.insert(*token);
        if (i + 1) % 10 == 0 || i + 1 == ranked.len() {
            println!(
                "  probe {}/{} last={:#x} sym={:?} v2_ok={} v3_ok={} both={} keep={}",
                i + 1,
                ranked.len(),
                p.token,
                p.symbol,
                p.v2_ok(),
                p.v3_ok(),
                p.both_ok(),
                p.keep_in_list()
            );
        }
        probes.push(p);
    }

    let extra: Vec<Address> = pairs_set.iter().copied().filter(|t| !seen_probe.contains(t)).collect();
    println!("bo sung probe pairs.txt chua nam trong top volume: {}", extra.len());
    for (i, token) in extra.iter().enumerate() {
        let mut p = probe_token(&provider, *token, 0.0, &th, v2_factory, v3_factory, uni_factory, pcs_quoter, uni_quoter, wbnb_a, usdt_a, probe_usdt, args.sleep_ms).await;
        p.from_pairs = true;
        if p.symbol.is_none() {
            p.symbol = pairs_sym.get(token).cloned();
        }
        if (i + 1) % 10 == 0 || i + 1 == extra.len() {
            println!(
                "  pairs {}/{} last={:#x} sym={:?} has_v3={} keep={}",
                i + 1,
                extra.len(),
                p.token,
                p.symbol,
                p.has_any_v3_pool(),
                p.keep_in_list()
            );
        }
        probes.push(p);
    }

    if let Ok(key) = std::env::var("ETHERSCAN_API_KEY").or_else(|_| std::env::var("BSCSCAN_API_KEY")) {
        if !key.is_empty() {
            println!("etherscan v2 getsourcecode cho token both_ok (khong in key)");
            for p in probes.iter_mut().filter(|p| p.keep_in_list()) {
                tokio::time::sleep(Duration::from_millis(250)).await;
                if let Some((ver, prox)) = etherscan_source(p.token, &key) {
                    p.verified = Some(ver);
                    p.proxy = Some(prox);
                }
            }
        }
    } else {
        println!("ETHERSCAN_API_KEY/BSCSCAN_API_KEY rong -> verified/proxy = null");
    }

    let v2_ok_n = probes.iter().filter(|p| p.v2_ok()).count();
    let v3_ok_n = probes.iter().filter(|p| p.v3_ok()).count();
    let both_n = probes.iter().filter(|p| p.both_ok()).count();
    let keep_n = probes.iter().filter(|p| p.keep_in_list()).count();
    let pairs_n = probes.iter().filter(|p| p.from_pairs).count();
    let pairs_v3_n = probes.iter().filter(|p| p.from_pairs && p.has_any_v3_pool()).count();
    let mut tier_dist: HashMap<u32, u64> = HashMap::new();
    let mut uni_tier: HashMap<u32, u64> = HashMap::new();
    for p in &probes {
        for v in p.v3.iter().filter(|x| x.ok) {
            *tier_dist.entry(v.fee).or_insert(0) += 1;
        }
        for v in p.uni_v3.iter().filter(|x| x.ok) {
            *uni_tier.entry(v.fee).or_insert(0) += 1;
        }
    }

    println!("=== TOM TAT ===");
    println!("scanned_tokens_vol={scanned_tokens} probed={}", probes.len());
    println!("v2_ok={v2_ok_n} v3_ok={v3_ok_n} both_ok={both_n} keep_in_list={keep_n}");
    println!("pairs_probed={pairs_n} pairs_with_v3={pairs_v3_n}");
    println!("tier_dist_pcs_v3_ok={tier_dist:?}");
    println!("tier_dist_uni_v3_ok={uni_tier:?}");
    println!("volume_method={volume_method}");

    let mut both: Vec<&TokenProbe> = probes.iter().filter(|p| p.keep_in_list()).collect();
    both.sort_by(|a, b| b.vol24h_bnb.partial_cmp(&a.vol24h_bnb).unwrap_or(std::cmp::Ordering::Equal));

    println!("=== TOP 30 keep_in_list theo vol (hoac top probed neu rong) ===");
    let top_print: Vec<&TokenProbe> = if both.is_empty() {
        let mut all: Vec<&TokenProbe> = probes.iter().collect();
        all.sort_by(|a, b| b.vol24h_bnb.partial_cmp(&a.vol24h_bnb).unwrap_or(std::cmp::Ordering::Equal));
        all.into_iter().take(30).collect()
    } else {
        both.iter().copied().take(30).collect()
    };
    for (i, p) in top_print.iter().enumerate() {
        let min_imp = p.v3.iter().filter(|x| x.ok).filter_map(|x| x.impact_pct).fold(None, |acc: Option<f64>, v| Some(acc.map(|a| a.min(v)).unwrap_or(v)));
        println!(
            "{:2} {:7} {:#x} vol={:.2} v2_ok={} v3_ok={} both={} keep={} pairs={} v3_impact_min={:?} uni={}",
            i + 1,
            p.symbol.as_deref().unwrap_or("?"),
            p.token,
            p.vol24h_bnb,
            p.v2_ok(),
            p.v3_ok(),
            p.both_ok(),
            p.keep_in_list(),
            p.from_pairs,
            min_imp,
            p.uni_v3.len()
        );
    }

    let recs: Vec<TokenVenues> = probes.iter().map(to_file_rec).collect();
    let file = MultiVenueFile {
        generated_at_unix: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        block: latest,
        infinity_from_block: 0,
        infinity_to_block: 0,
        min_reserve_wbnb_wei: th.min_v2_bnb_wei.to_string(),
        min_reserve_usdt_wei: th.min_v2_usdt_wei.to_string(),
        bridge_wbnb_usdt_pair: bridge_pair.map(|p| format!("{p:#x}")),
        bridge_reserve_wbnb: Some(br_w.to_string()),
        bridge_reserve_usdt: Some(br_u.to_string()),
        tokens: recs,
        source: Some("discover_multivenue".into()),
        hours: Some(args.hours),
        scanned_tokens: Some(scanned_tokens as u64),
        v2_ok_count: Some(v2_ok_n as u64),
        v3_ok_count: Some(v3_ok_n as u64),
        both_ok_count: Some(both_n as u64),
        volume_method: Some(volume_method),
    };

    for path in [&args.out, &args.report, &args.candidates] {
        if let Some(parent) = Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    std::fs::write(&args.out, serde_json::to_string_pretty(&file).expect("json")).expect("write json");

    let mut tsv = String::from(report_header());
    tsv.push('\n');
    let mut sorted = probes.clone();
    sorted.sort_by(|a, b| b.vol24h_bnb.partial_cmp(&a.vol24h_bnb).unwrap_or(std::cmp::Ordering::Equal));
    for p in &sorted {
        tsv.push_str(&report_row(p));
        tsv.push('\n');
    }
    std::fs::write(&args.report, tsv).expect("write tsv");

    let mut cand = String::from("# discover_multivenue — CHUA VET. Chu chay vet_bsc_token roi dien vetted YYYY-MM-DD.\n");
    for p in both {
        cand.push_str(&candidates_line(p));
        cand.push('\n');
    }
    std::fs::write(&args.candidates, cand).expect("write candidates");

    println!(
        "WROTE {} tokens_probed={} both_ok={both_n} keep={keep_n} pairs_v3={pairs_v3_n} report={} candidates={} elapsed={:.1}s machine=WSL",
        args.out,
        probes.len(),
        args.report,
        args.candidates,
        started.elapsed().as_secs_f64()
    );
}

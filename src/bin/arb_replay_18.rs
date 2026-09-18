//! Cụm `planB-B8e-revm-14-after-gate` — replay đúng 14 dòng `sim.arb simulated`
//! (BAOCAO57 paper 60' sau cổng B8c) trên revm, cùng borrow / route_kind /
//! flash `infinity_vault`, fork tại block victim. Không ký, không gửi.
//!
//! `--jsonl` mặc định file B8d. Kỳ vọng 14 simulated. Hỗ trợ V2 + V3
//! (`v2_v3` / `v3_v3`). `lệch_pct` so `size_quote_net_wei` (quoter), không
//! so paper CPMM. Panic/hàng → MISSING, chạy hết 14.
//!
//! Thiếu archive (`eth_getStorageAt` `-32000` / missing trie) → hàng đó
//! `profit_revm=MISSING`, không đoán lãi.

use alloy::primitives::{Address, B256, U256};
use alloy::providers::{Provider, ProviderBuilder};
use bsc_sandwich::multivenue::MultiVenueMap;
use bsc_sandwich::sim_arb::{self, ArbPool, ArbV3Pool, ArbVenue, BridgeLeg, MixedRoute, V3Family};
use bsc_sandwich::sim_evm;
use bsc_sandwich::sim_v3;
use bsc_sandwich::transport;
use serde_json::Value;
use std::path::Path;
use std::str::FromStr;

const DEFAULT_JSONL: &str = "baocao/evidence/baocao57_paper60_simarb.jsonl";
const DEFAULT_MV: &str = "state/multi_venue.json";
const CAKE: &str = "0x0e09FaBB73Bd3Ade0a17ECC321fD13a19e81cE82";

fn host_only(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "parse_fail".into())
}

fn redact(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            if w.starts_with("http://") || w.starts_with("https://") {
                "[url]"
            } else {
                w
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_addr(s: &str) -> Option<Address> {
    Address::from_str(s).ok()
}

fn parse_u256(s: &str) -> Option<U256> {
    U256::from_str(s).ok()
}

fn parse_i128(s: &str) -> Option<i128> {
    s.parse().ok()
}

fn family_of(kind: &str) -> Option<V3Family> {
    match kind {
        "pcs_v3" => Some(V3Family::Pcs),
        "uni_v3" => Some(V3Family::Uni),
        _ => None,
    }
}

/// dotenv first-wins bỏ dòng `BSC_HTTP_SIM` sau (NodeReal archive B8b).
/// Đọc MỌI occurrence, không echo giá trị.
fn load_all_env_key_urls(path: &Path, key: &str) -> Vec<String> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((k, v)) = line.split_once('=') else { continue };
        if k.trim() != key {
            continue;
        }
        let v = v.trim().trim_matches('"').trim_matches('\'');
        for part in v.split(',') {
            let u = part.trim();
            if !u.is_empty() && !out.iter().any(|x: &String| x == u) {
                out.push(u.to_string());
            }
        }
    }
    out
}

fn is_getblock_host(url: &str) -> bool {
    host_only(url).to_lowercase().contains("getblock")
}

/// `lệch_pct = (profit_revm - net_quoter) / |net_quoter| * 100` (có dấu).
fn lech_pct_vs_quoter(quoter: i128, revm: i128) -> f64 {
    if quoter == 0 {
        if revm == 0 {
            0.0
        } else {
            999.0
        }
    } else {
        (revm - quoter) as f64 / (quoter.unsigned_abs() as f64) * 100.0
    }
}

fn i128_units(v: i128) -> f64 {
    (v as f64) / 1e18
}

fn u256_units(v: U256) -> f64 {
    let u = u128::try_from(v).unwrap_or(0);
    (u as f64) / 1e18
}

fn is_archive_err(s: &str) -> bool {
    matches!(
        transport::classify_vet_error(s),
        "missing_trie_node" | "unsupported_method"
    ) || s.to_lowercase().contains("historical")
}

fn is_rate_limit(s: &str) -> bool {
    let l = s.to_lowercase();
    l.contains("-32005")
        || l.contains("limit exceeded")
        || l.contains("api usage limit")
        || l.contains("too many requests")
        || l.contains("429")
}

async fn fetch_block(
    providers: &[(String, alloy::providers::DynProvider)],
    hash: B256,
) -> Result<u64, String> {
    let mut last = "no_provider".to_string();
    for attempt in 0..6u32 {
        let mut saw_rate = false;
        for (id, p) in providers {
            match p.get_transaction_receipt(hash).await {
                Ok(Some(rcpt)) => match rcpt.block_number {
                    Some(b) => return Ok(b),
                    None => last = format!("host={id} receipt.block_number=None"),
                },
                Ok(None) => last = format!("host={id} receipt=None"),
                Err(e) => {
                    let msg = redact(&e.to_string());
                    last = format!("host={id} {msg}");
                    if is_rate_limit(&msg) {
                        saw_rate = true;
                    }
                }
            }
        }
        if saw_rate {
            tokio::time::sleep(std::time::Duration::from_millis(1500 * (attempt as u64 + 1))).await;
            continue;
        }
        break;
    }
    Err(last)
}

fn lookup_v3(mv: &MultiVenueMap, token: Address, pool: Address, kind: &str) -> Option<ArbVenue> {
    let rec = mv.get(token)?;
    let family = family_of(kind)?;
    let scan = |pools: &[bsc_sandwich::multivenue::V3PoolRec], fam: V3Family| -> Option<ArbVenue> {
        for p in pools {
            let Ok(a) = Address::from_str(&p.pool) else { continue };
            if a != pool {
                continue;
            }
            let Ok(quote) = Address::from_str(&p.quote) else { continue };
            return Some(ArbVenue::V3(ArbV3Pool {
                pool,
                quote,
                fee: p.fee,
                family: fam,
                reserve_quote: U256::ZERO,
                reserve_token: U256::ZERO,
                ok: p.ok,
            }));
        }
        None
    };
    match family {
        V3Family::Pcs => scan(&rec.v3_pools, V3Family::Pcs),
        V3Family::Uni => {
            for p in &rec.uni_v3_pools {
                let Ok(a) = Address::from_str(&p.pool) else { continue };
                if a != pool {
                    continue;
                }
                let Ok(quote) = Address::from_str(&p.quote) else { continue };
                return Some(ArbVenue::V3(ArbV3Pool {
                    pool,
                    quote,
                    fee: p.fee,
                    family: V3Family::Uni,
                    reserve_quote: U256::ZERO,
                    reserve_token: U256::ZERO,
                    ok: p.ok,
                }));
            }
            None
        }
    }
}

fn lookup_v2(mv: &MultiVenueMap, token: Address, pair: Address) -> Option<ArbVenue> {
    let rec = mv.get(token)?;
    for p in &rec.v2_pools {
        let Ok(a) = Address::from_str(&p.pair) else { continue };
        if a != pair {
            continue;
        }
        let Ok(quote) = Address::from_str(&p.quote) else { continue };
        let reserve_quote = U256::from_str(&p.reserve_quote).ok().unwrap_or(U256::ZERO);
        let reserve_token = U256::from_str(&p.reserve_token).ok().unwrap_or(U256::ZERO);
        return Some(ArbVenue::V2(ArbPool {
            pair,
            quote,
            reserve_quote,
            reserve_token,
        }));
    }
    None
}

fn lookup_venue(mv: &MultiVenueMap, token: Address, pool: Address, kind: &str) -> Option<ArbVenue> {
    if kind == "v2" {
        lookup_v2(mv, token, pool)
    } else {
        lookup_v3(mv, token, pool, kind)
    }
}

struct Row {
    hash: String,
    token: Address,
    route_kind: String,
    buy_kind: String,
    sell_kind: String,
    pair_buy: Address,
    pair_sell: Address,
    borrow: U256,
    borrow_quote: Address,
    flash_source: String,
    net_wei: i128,
    size_quote_net_wei: i128,
    gas_wei: i128,
    bribe_wei: i128,
    flash_fee_wei: U256,
}

fn load_simulated(path: &Path) -> Result<Vec<Row>, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("doc {path:?}: {e}"))?;
    let mut out = Vec::new();
    for line in raw.lines() {
        if line.is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(line).map_err(|e| format!("json: {e}"))?;
        if v.get("decision").and_then(|x| x.as_str()) != Some("simulated") {
            continue;
        }
        let gs = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        out.push(Row {
            hash: gs("hash"),
            token: parse_addr(&gs("token")).ok_or_else(|| "token".to_string())?,
            route_kind: gs("route_kind"),
            buy_kind: gs("buy_kind"),
            sell_kind: gs("sell_kind"),
            pair_buy: parse_addr(&gs("pair_buy")).ok_or_else(|| "pair_buy".to_string())?,
            pair_sell: parse_addr(&gs("pair_sell")).ok_or_else(|| "pair_sell".to_string())?,
            borrow: parse_u256(&gs("borrow")).ok_or_else(|| "borrow".to_string())?,
            borrow_quote: parse_addr(&gs("borrow_quote")).ok_or_else(|| "borrow_quote".to_string())?,
            flash_source: gs("flash_source"),
            net_wei: parse_i128(&gs("net_wei")).ok_or_else(|| "net_wei".to_string())?,
            size_quote_net_wei: parse_i128(&gs("size_quote_net_wei"))
                .ok_or_else(|| "size_quote_net_wei".to_string())?,
            gas_wei: parse_i128(&gs("gas_wei")).unwrap_or(0),
            bribe_wei: parse_i128(&gs("bribe_wei")).unwrap_or(0),
            flash_fee_wei: parse_u256(&gs("flash_fee_wei")).unwrap_or(U256::ZERO),
        });
    }
    Ok(out)
}

fn symbol_of(mv: &MultiVenueMap, token: Address) -> String {
    mv.get(token)
        .and_then(|t| t.symbol.clone())
        .unwrap_or_else(|| format!("{token:#x}"))
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut jsonl = DEFAULT_JSONL.to_string();
    let mut mv_path = DEFAULT_MV.to_string();
    let mut hash_filter: Option<std::collections::HashSet<String>> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--jsonl" => {
                jsonl = args[i + 1].clone();
                i += 2;
            }
            "--multi-venue" => {
                mv_path = args[i + 1].clone();
                i += 2;
            }
            "--hashes" => {
                let set = args[i + 1]
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();
                hash_filter = Some(set);
                i += 2;
            }
            _ => i += 1,
        }
    }

    let _ = transport::load_dotenv_defaults(Path::new(".env"));
    let mut sim = transport::filter_read_urls(transport::collect_rpc_urls_from_env("BSC_HTTP_SIM"));
    for u in load_all_env_key_urls(Path::new(".env"), "BSC_HTTP_SIM") {
        if !sim.contains(&u) {
            sim.push(u);
        }
    }
    sim.sort_by_key(|u| {
        let h = host_only(u).to_lowercase();
        if h.contains("nodereal") {
            0u8
        } else if is_getblock_host(u) {
            9
        } else {
            1
        }
    });
    let http = transport::filter_read_urls(transport::collect_rpc_urls_from_env("BSC_HTTP"));
    let mut urls = Vec::new();
    for u in sim.iter().chain(http.iter()) {
        if !urls.contains(u) {
            urls.push(u.clone());
        }
    }
    if urls.is_empty() {
        eprintln!("MISSING BSC_HTTP_SIM/BSC_HTTP");
        std::process::exit(1);
    }
    println!(
        "arb_replay_18 n_sim={} n_http={} n_url={} hosts={}",
        sim.len(),
        http.len(),
        urls.len(),
        urls.iter().map(|u| host_only(u)).collect::<Vec<_>>().join(",")
    );

    let mut providers: Vec<(String, alloy::providers::DynProvider)> = Vec::new();
    for (i, u) in urls.iter().enumerate() {
        match ProviderBuilder::new().connect(u).await {
            Ok(p) => match p.get_block_number().await {
                Ok(bn) => {
                    let h = format!("{}#{}", host_only(u), i);
                    println!("connect_ok host={h} head={bn}");
                    providers.push((h, p.erased()));
                }
                Err(e) => eprintln!("connect_head_fail host={} {}", host_only(u), redact(&e.to_string())),
            },
            Err(e) => eprintln!("connect_fail host={} {}", host_only(u), redact(&e.to_string())),
        }
    }
    if providers.is_empty() {
        eprintln!("FAIL khong connect duoc RPC nao");
        std::process::exit(1);
    }
    let used_host = providers.iter().map(|(h, _)| h.as_str()).collect::<Vec<_>>().join(",");
    let mv = match MultiVenueMap::load_from_path(Path::new(&mv_path)) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("FAIL load multi_venue: {e}");
            std::process::exit(1);
        }
    };
    let mut rows = match load_simulated(Path::new(&jsonl)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("FAIL load jsonl: {e}");
            std::process::exit(1);
        }
    };
    if let Some(ref want) = hash_filter {
        rows.retain(|r| want.contains(&r.hash.to_lowercase()));
    }
    if rows.is_empty() {
        eprintln!("FAIL n_simulated=0");
        std::process::exit(1);
    }
    let cake = Address::from_str(CAKE).expect("cake");
    rows.sort_by_key(|r| if r.token == cake { 0u8 } else { 1u8 });
    let n_unique = rows
        .iter()
        .map(|r| r.hash.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len();
    println!("n_simulated={} n_unique_hash={}", rows.len(), n_unique);
    println!("fork=BlockId::number(victim_receipt_block) post-state apply_victim_raw=no");
    println!(
        "token\tsymbol\troute\tborrow_BNB\tnet_paper\tnet_quoter_log\tnet_quoter_now\tprofit_revm\tlech_pct_vs_quoter_now\thop1\thop2\thop3\trevert\tblock\ttx_hash\tflash\terr"
    );

    let mut n_ok = 0u32;
    let mut n_fail_lech = 0u32;
    let mut n_revert = 0u32;
    let mut n_missing = 0u32;
    let mut archive_dead: std::collections::HashSet<String> = std::collections::HashSet::new();

    for row in &rows {
        let symbol = symbol_of(&mv, row.token);
        let borrow_u = u256_units(row.borrow);
        let paper_u = i128_units(row.net_wei);
        let quoter_u = i128_units(row.size_quote_net_wei);
        let miss = |blk: &str, why: String| {
            println!(
                "{:#x}\t{symbol}\t{}\t{borrow_u:.6}\t{paper_u:.6}\t{quoter_u:.6}\tMISSING\tMISSING\tMISSING\tMISSING\tMISSING\tMISSING\tMISSING\t{blk}\t{}\t{}\t{why}",
                row.token, row.route_kind, row.hash, row.flash_source
            );
        };
        let hash = match B256::from_str(&row.hash) {
            Ok(h) => h,
            Err(e) => {
                miss("", format!("hash_parse:{e}"));
                n_missing += 1;
                continue;
            }
        };
        let block = match fetch_block(&providers, hash).await {
            Ok(b) => b,
            Err(err) => {
                miss("", err);
                n_missing += 1;
                continue;
            }
        };

        let Some(buy) = lookup_venue(&mv, row.token, row.pair_buy, &row.buy_kind) else {
            miss(&block.to_string(), "no_buy_venue".into());
            n_missing += 1;
            continue;
        };
        let Some(sell) = lookup_venue(&mv, row.token, row.pair_sell, &row.sell_kind) else {
            miss(&block.to_string(), "no_sell_venue".into());
            n_missing += 1;
            continue;
        };
        let got_kind = sim_arb::route_kind(buy, sell);
        if got_kind != row.route_kind {
            miss(&block.to_string(), format!("route_mismatch:{got_kind}"));
            n_missing += 1;
            continue;
        }
        if row.flash_source != "infinity_vault" {
            miss(&block.to_string(), "flash_mismatch".into());
            n_missing += 1;
            continue;
        }

        let sell_quote = sell.quote();
        let mut done = false;
        let mut last_sim_err = String::new();
        'hosts: for (host, p) in &providers {
            if archive_dead.contains(host) {
                continue;
            }
            if host.to_lowercase().contains("getblock") {
                continue;
            }
            for attempt in 0..8u32 {
                let p2 = p.clone();
                let token = row.token;
                let borrow_quote = row.borrow_quote;
                let borrow = row.borrow;
                let join = tokio::spawn(async move {
                    sim_evm::simulate_arb_mixed_hops_evm(
                        p2,
                        block,
                        token,
                        borrow_quote,
                        sell_quote,
                        borrow,
                        buy,
                        sell,
                    )
                    .await
                });
                match join.await {
                    Err(_) => {
                        n_missing += 1;
                        miss(&block.to_string(), format!("panic host={host}"));
                        done = true;
                        break 'hosts;
                    }
                    Ok(Ok((final_out, _gas))) => {
                    let final_i = i128::try_from(u128::try_from(final_out).unwrap_or(0)).unwrap_or(0);
                    let borrow_i = i128::try_from(u128::try_from(row.borrow).unwrap_or(0)).unwrap_or(0);
                    let flash_i = i128::try_from(u128::try_from(row.flash_fee_wei).unwrap_or(0)).unwrap_or(0);
                    let net_revm = final_i - borrow_i - flash_i - row.gas_wei - row.bribe_wei;
                    let bridge = if buy.quote() == sell.quote() {
                        None
                    } else {
                        Some(BridgeLeg {
                            pair: mv.bridge_pair.unwrap_or(alloy::primitives::Address::ZERO),
                            reserve_in: mv.bridge_reserve_usdt,
                            reserve_out: mv.bridge_reserve_wbnb,
                        })
                    };
                    let route = MixedRoute {
                        token: row.token,
                        borrow_quote: row.borrow_quote,
                        buy,
                        sell,
                        bridge,
                    };
                    let (net_now, hop1_s, hop2_s, hop3_s) =
                        match sim_v3::quote_mixed_hops_at(p, &route, row.borrow, Some(block)).await {
                            Ok((h1, h2, h3)) => {
                                let h3i = i128::try_from(u128::try_from(h3).unwrap_or(0)).unwrap_or(0);
                                let n = h3i - borrow_i - flash_i - row.gas_wei - row.bribe_wei;
                                (Some(n), format!("{h1}"), format!("{h2}"), format!("{h3}"))
                            }
                            Err(e) => (
                                None,
                                "MISSING".into(),
                                "MISSING".into(),
                                format!("quote_fail:{}", redact(&e)),
                            ),
                        };
                    let quoter_now = net_now.unwrap_or(row.size_quote_net_wei);
                    let lech = lech_pct_vs_quoter(quoter_now, net_revm);
                    let fail = net_now.map(|n| n > 0 && lech.abs() > 20.0).unwrap_or(true);
                    if fail {
                        n_fail_lech += 1;
                    } else {
                        n_ok += 1;
                    }
                    let now_s = net_now
                        .map(|n| format!("{:.6}", i128_units(n)))
                        .unwrap_or_else(|| "MISSING".into());
                    let gate = if net_now.map(|n| n > 0).unwrap_or(false) {
                        if fail {
                            "FAIL_LECH>20"
                        } else {
                            "ok"
                        }
                    } else {
                        "GATE_NO_SIM"
                    };
                    println!(
                        "{:#x}\t{symbol}\t{}\t{borrow_u:.6}\t{paper_u:.6}\t{quoter_u:.6}\t{now_s}\t{:.6}\t{lech:.4}\t{hop1_s}\t{hop2_s}\t{hop3_s}\tno\t{block}\t{}\t{}\t{gate} host={host}",
                        row.token,
                        row.route_kind,
                        i128_units(net_revm),
                        row.hash,
                        row.flash_source,
                    );
                    done = true;
                    break 'hosts;
                    }
                    Ok(Err(e)) => {
                    let msg = redact(&e.to_string());
                    last_sim_err = format!("host={host} {msg}");
                    if e.is_revert() {
                        n_revert += 1;
                        println!(
                            "{:#x}\t{symbol}\t{}\t{borrow_u:.6}\t{paper_u:.6}\t{quoter_u:.6}\tMISSING\tMISSING\tMISSING\tMISSING\tMISSING\tMISSING\tyes\t{block}\t{}\t{}\thost={host} {msg}",
                            row.token, row.route_kind, row.hash, row.flash_source
                        );
                        done = true;
                        break 'hosts;
                    } else if is_archive_err(&msg) {
                        eprintln!("archive_fail host={host} block={block} {}", msg);
                        archive_dead.insert(host.clone());
                        continue 'hosts;
                    } else if is_rate_limit(&msg) {
                        eprintln!(
                            "rate_limit host={host} attempt={} block={block} {}",
                            attempt + 1,
                            msg
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(
                            5000 * (attempt as u64 + 1),
                        ))
                        .await;
                        continue;
                    } else {
                        n_missing += 1;
                        miss(&block.to_string(), format!("host={host} {msg}"));
                        done = true;
                        break 'hosts;
                    }
                    }
                }
            }
        }
        if !done {
            n_missing += 1;
            let err = if last_sim_err.is_empty() {
                format!("ARCHIVE all_rpc getStorageAt -32000/missing_trie hosts={used_host}")
            } else {
                format!("RETRY_EXHAUST {last_sim_err}")
            };
            miss(&block.to_string(), err);
        }
        tokio::time::sleep(std::time::Duration::from_millis(8000)).await;
    }

    println!(
        "SUMMARY n={} n_unique_hash={n_unique} n_ok={n_ok} n_fail_lech={n_fail_lech} n_revert={n_revert} n_missing={n_missing} rpc_host={used_host}",
        rows.len()
    );
    println!(
        "FAIL_rule: |lech_vs_quoter|>20% n={n_fail_lech} revert n={n_revert} (archive/MISSING n={n_missing} khong doan lai)"
    );
}

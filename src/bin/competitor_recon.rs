//! Cụm `competitor-recon-and-strategy` (F1) — trinh sát đối thủ MEV THẬT qua
//! RPC BSC công khai (`BSC_HTTP`). CHỈ ĐỌC — không có `send_raw_transaction`/
//! `Signer` nào trong file này. Chạy:
//!
//!   set -a; . .env; set +a; cargo run --release --bin competitor_recon
//!
//! PHẦN A: contract executor `0xa739Dfab40ef6585f1174fcE90EC96330669758c`
//! (selector nghi vấn `0x5aab2274`) + EOA `0xB406021E07b31E1f7850FCcCD7076094f18d07eF`
//! — lấy log `Swap` THẬT nơi 2 địa chỉ này là `sender`/`to` (2 topic indexed
//! của event `Swap(address indexed sender,uint,uint,uint,uint,address indexed to)`),
//! phân loại pool áp đảo, ước tính profit/tx, dấu hiệu bundle (gas ưu tiên +
//! thử `debug_traceTransaction` để tìm coinbase bribe — ghi MISSING nếu RPC
//! không hỗ trợ, KHÔNG suy diễn).
//!
//! PHẦN B: quét TOÀN BỘ 126 pool đã vet trong `pairs.txt` (qua `PairBook`
//! thật, cùng logic resolve production) trong 3000 block gần nhất — tìm
//! victim ≥0.05 BNB, kiểm ±3 vị trí liền kề CÙNG pool, gộp bảng "đối thủ
//! theo pool" + cột đối chiếu "bị 0xB406 phủ" (dùng tập pool từ Phần A).
//!
//! Không có test on-chain "pass rỗng" — mọi số liệu chỉ hiện khi RPC thật
//! trả về được; RPC lỗi/rỗng in `MISSING`/số 0 kèm lý do, không suy diễn.

use alloy::consensus::Transaction as _;
use alloy::network::TransactionResponse as _;
use alloy::primitives::{keccak256, Address, B256, U256};
use alloy::providers::{DynProvider, Provider, ProviderBuilder};
use alloy::rpc::types::eth::Filter;
use bsc_sandwich::logger::BotLogger;
use bsc_sandwich::pairbook::{PairBook, RpcPairResolver};
use bsc_sandwich::{pool, transport, venues};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::time::{Duration, Instant};

const CONTRACT_EXECUTOR: &str = "0xa739Dfab40ef6585f1174fcE90EC96330669758c";
const EOA_TRADER: &str = "0xB406021E07b31E1f7850FCcCD7076094f18d07eF";
const SUSPECT_SELECTOR: &str = "0x5aab2274";

const SWAP_EVENT_SIG: &str = "Swap(address,uint256,uint256,uint256,uint256,address)";
const TRANSFER_EVENT_SIG: &str = "Transfer(address,address,uint256)";

/// Ngân sách gọi RPC / thời gian cho MỖI địa chỉ ở Phần A — chặn treo phiên
/// nếu RPC công khai chậm/rate-limit (ghi rõ số block/tx thật quét được,
/// không giả vờ "quét hết" nếu hết ngân sách giữa chừng).
const PART_A_BLOCK_BUDGET: u64 = 150_000; // ~5 ngay BSC (~0.75-3s/block, node public that thay doi) - giam tu 400k sau khi thay 400k gay 429 rate-limit (run1) lam Phan B mat du lieu
const PART_A_WALL_BUDGET: Duration = Duration::from_secs(240);
const PART_A_TARGET_TX: usize = 500;
const PART_A_ANALYZE_CAP: usize = 150; // sau khi gom hash, phan tich sau toi da 150 tx/dia chi (bound RPC/thoi gian)

const PART_B_BLOCK_WINDOW: u64 = 3000;
const PART_B_VICTIM_MIN_WEI: u128 = 50_000_000_000_000_000; // 0.05 BNB
const PART_B_ADDR_BATCH: usize = 30;

fn swap_topic() -> B256 {
    keccak256(SWAP_EVENT_SIG.as_bytes())
}
fn transfer_topic() -> B256 {
    keccak256(TRANSFER_EVENT_SIG.as_bytes())
}

fn wbnb() -> Address {
    Address::from_str(venues::WBNB_ADDRESS).unwrap()
}
fn usdt() -> Address {
    Address::from_str(venues::USDT_ADDRESS).unwrap()
}

#[derive(Debug, Clone, Copy)]
struct SwapAmounts {
    amount0_in: U256,
    amount1_in: U256,
    amount0_out: U256,
    amount1_out: U256,
}

fn decode_swap_data(data: &[u8]) -> Option<SwapAmounts> {
    if data.len() < 128 {
        return None;
    }
    Some(SwapAmounts {
        amount0_in: U256::from_be_slice(&data[0..32]),
        amount1_in: U256::from_be_slice(&data[32..64]),
        amount0_out: U256::from_be_slice(&data[64..96]),
        amount1_out: U256::from_be_slice(&data[96..128]),
    })
}

/// Dò kích thước chunk block lớn nhất mà URL đang dùng chấp nhận cho
/// `eth_getLogs` không lọc address (cùng kỹ thuật đã verify thật ở
/// `sim_evm.rs::real_rpc_replay_real_sandwich_triplets`, tránh đoán mò).
/// `eth_getLogs` với retry khi gặp rate-limit thật (`429`/`-32005`/"limit
/// exceeded") — tối đa 4 lần thử, backoff tăng dần (500ms/1s/2s/4s). Phát
/// hiện thật ở run1 (`baocao/evidence/competitor_recon_run1.txt`): RPC
/// `rpc-bsc.48.club` trả `429` sau khi Phần A dùng hết ngân sách gọi liên
/// tục — Phần B khi đó BỊ MẤT TOÀN BỘ dữ liệu (0 log) vì code cũ chỉ in lỗi
/// rồi đi tiếp, không retry. Trả `Err` rõ ràng nếu hết lượt retry (không bịa
/// "0 log = không có hoạt động" khi thực ra là rate-limit).
async fn get_logs_retry(provider: &DynProvider, filter: &Filter) -> Result<Vec<alloy::rpc::types::eth::Log>, String> {
    let mut last_err = String::new();
    for attempt in 0..4u32 {
        match provider.get_logs(filter).await {
            Ok(logs) => return Ok(logs),
            Err(e) => {
                last_err = e.to_string();
                let is_rate_limit = last_err.contains("-32005") || last_err.contains("limit exceeded") || last_err.contains("429");
                if is_rate_limit {
                    tokio::time::sleep(Duration::from_millis(500 * 2u64.pow(attempt))).await;
                    continue;
                }
                return Err(last_err);
            }
        }
    }
    Err(format!("het luot retry (4 lan), loi cuoi: {last_err}"))
}

/// Quét `Transfer` (token bất kỳ) nơi `from` = `from_addr` — dùng để tìm CÁC
/// ĐỊA CHỈ được cấp vốn (dòng tiền) từ 1 địa chỉ gốc, KHÔNG phụ thuộc việc
/// địa chỉ nhận có tự gọi `pair.swap()` trực tiếp hay không (khác
/// `scan_swap_logs_for_role` — hàm đó chỉ bắt được Swap.sender/to, bỏ sót
/// hoàn toàn kiểu vận hành "ví A chuyển quote cho ví B, ví B tự swap qua
/// router" — đúng lỗi đã SỬA GIỮA PHIÊN sau khi Chủ chỉ ra bằng chứng thật
/// block `122076185`).
async fn scan_transfer_from_role(
    provider: &DynProvider,
    latest: u64,
    chunk: u64,
    token: Address,
    from_addr: Address,
    block_budget: u64,
    wall_budget: Duration,
) -> Vec<(Address, U256, u64, B256)> {
    let topic0 = transfer_topic();
    let started = Instant::now();
    let mut out = Vec::new();
    let mut cursor = latest;
    let mut scanned = 0u64;
    while scanned < block_budget && started.elapsed() < wall_budget {
        let lo = cursor.saturating_sub(chunk.saturating_sub(1));
        let f = Filter::new().address(token).event_signature(topic0).topic1(from_addr).from_block(lo).to_block(cursor);
        if let Ok(logs) = get_logs_retry(provider, &f).await {
            for l in &logs {
                let (Some(bn), Some(th)) = (l.block_number, l.transaction_hash) else { continue };
                let topics = l.topics();
                if topics.len() < 3 {
                    continue;
                }
                let to = topic_to_address(&topics[2]);
                let amount = U256::from_be_slice(l.data().data.as_ref());
                out.push((to, amount, bn, th));
            }
        }
        scanned += cursor - lo + 1;
        if lo == 0 {
            break;
        }
        cursor = lo - 1;
        tokio::time::sleep(Duration::from_millis(80)).await;
    }
    out
}

async fn probe_chunk_size(provider: &DynProvider, latest: u64, topic0: B256) -> u64 {
    for try_chunk in [2000u64, 1000, 500, 100, 25, 10] {
        let lo = latest.saturating_sub(try_chunk.saturating_sub(1));
        let f = Filter::new().event_signature(topic0).from_block(lo).to_block(latest);
        if provider.get_logs(&f).await.is_ok() {
            return try_chunk;
        }
    }
    5
}

async fn connect_first_working(urls: &[String]) -> Option<(String, DynProvider)> {
    for u in urls {
        if let Ok(p) = ProviderBuilder::new().connect(u).await {
            if let Ok(chain_id) = p.get_chain_id().await {
                if chain_id == 56 {
                    return Some((transport::redact_rpc_url(u), p.erased()));
                }
            }
        }
    }
    None
}

/// Log Swap THẬT thu được từ `eth_getLogs` — giữ đủ field cần cho cả Phần A
/// lẫn Phần B (tránh gọi lại RPC 2 lần cho cùng dữ liệu).
#[derive(Debug, Clone)]
struct SwapRow {
    block: u64,
    tx_index: u64,
    log_index: u64,
    tx_hash: B256,
    pool: Address,
    sender: Address,
    to: Address,
    amounts: SwapAmounts,
}

fn topic_to_address(t: &B256) -> Address {
    Address::from_slice(&t.as_slice()[12..32])
}

async fn scan_swap_logs_for_role(
    provider: &DynProvider,
    latest: u64,
    chunk: u64,
    role_topic_is_sender: bool,
    addr: Address,
    block_budget: u64,
    wall_budget: Duration,
    target_rows: usize,
) -> (Vec<SwapRow>, u64, u64) {
    let topic0 = swap_topic();
    let started = Instant::now();
    let mut rows = Vec::new();
    let mut cursor = latest;
    let mut blocks_scanned = 0u64;
    let mut calls = 0u64;
    while blocks_scanned < block_budget && started.elapsed() < wall_budget && rows.len() < target_rows {
        let lo = cursor.saturating_sub(chunk.saturating_sub(1));
        let mut f = Filter::new().event_signature(topic0).from_block(lo).to_block(cursor);
        f = if role_topic_is_sender { f.topic1(addr) } else { f.topic2(addr) };
        calls += 1;
        match get_logs_retry(provider, &f).await {
            Ok(logs) => {
                for l in &logs {
                    let (Some(bn), Some(ti), Some(li), Some(th)) =
                        (l.block_number, l.transaction_index, l.log_index, l.transaction_hash)
                    else {
                        continue;
                    };
                    let topics = l.topics();
                    if topics.len() < 3 {
                        continue;
                    }
                    let sender = topic_to_address(&topics[1]);
                    let to = topic_to_address(&topics[2]);
                    let Some(amounts) = decode_swap_data(l.data().data.as_ref()) else { continue };
                    rows.push(SwapRow { block: bn, tx_index: ti, log_index: li, tx_hash: th, pool: l.address(), sender, to, amounts });
                }
            }
            Err(e) => {
                println!("  eth_getLogs [{lo}..{cursor}] role={} LOI SAU RETRY: {e}", if role_topic_is_sender { "sender" } else { "to" });
            }
        }
        blocks_scanned += cursor - lo + 1;
        if lo == 0 {
            break;
        }
        cursor = lo - 1;
        tokio::time::sleep(Duration::from_millis(90)).await;
    }
    (rows, blocks_scanned, calls)
}

/// Fallback khi `Swap.sender`/`Swap.to` không lộ địa chỉ (vd EOA gọi qua
/// router — `sender` khi đó là ROUTER, không phải EOA) — quét log
/// `Transfer` của WBNB nơi `from`/`to` = địa chỉ cần tìm, để ít nhất có
/// TẬP HASH TX thật đã chạm WBNB, phục vụ phân tích tay.
async fn scan_wbnb_transfer_hashes(
    provider: &DynProvider,
    latest: u64,
    chunk: u64,
    addr: Address,
    block_budget: u64,
    wall_budget: Duration,
    target: usize,
) -> HashSet<B256> {
    let topic0 = transfer_topic();
    let wbnb_addr = wbnb();
    let started = Instant::now();
    let mut hashes = HashSet::new();
    let mut cursor = latest;
    let mut scanned = 0u64;
    while scanned < block_budget && started.elapsed() < wall_budget && hashes.len() < target {
        let lo = cursor.saturating_sub(chunk.saturating_sub(1));
        for role in [1u8, 2u8] {
            let mut f = Filter::new().address(wbnb_addr).event_signature(topic0).from_block(lo).to_block(cursor);
            f = if role == 1 { f.topic1(addr) } else { f.topic2(addr) };
            if let Ok(logs) = get_logs_retry(provider, &f).await {
                for l in &logs {
                    if let Some(h) = l.transaction_hash {
                        hashes.insert(h);
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
        scanned += cursor - lo + 1;
        if lo == 0 {
            break;
        }
        cursor = lo - 1;
    }
    hashes
}

struct TxDetail {
    block: u64,
    tx_index: u64,
    from: Address,
    to: Option<Address>,
    gas_price_wei: u128,
    selector: Option<String>,
    status_ok: bool,
    pools_touched: Vec<(Address, SwapAmounts, bool /*is_token0_sender_role unused*/)>,
}

async fn fetch_tx_detail(provider: &DynProvider, hash: B256) -> Option<TxDetail> {
    let tx = provider.get_transaction_by_hash(hash).await.ok()??;
    let receipt = provider.get_transaction_receipt(hash).await.ok()??;
    let block = receipt.block_number?;
    let tx_index = receipt.transaction_index.unwrap_or(0);
    let from = tx.from();
    let to = tx.to();
    let gas_price_wei = <_ as alloy::consensus::Transaction>::gas_price(&tx)
        .unwrap_or_else(|| <_ as alloy::consensus::Transaction>::max_fee_per_gas(&tx));
    let input = <_ as alloy::consensus::Transaction>::input(&tx);
    let selector = if input.len() >= 4 { Some(format!("0x{}", hex_encode(&input[0..4]))) } else { None };
    let status_ok = receipt.status();
    let topic0 = swap_topic();
    let mut pools_touched = Vec::new();
    for l in receipt.inner.logs() {
        if l.topics().first() != Some(&topic0) {
            continue;
        }
        if let Some(amounts) = decode_swap_data(l.data().data.as_ref()) {
            pools_touched.push((l.address(), amounts, false));
        }
    }
    Some(TxDetail { block, tx_index, from, to, gas_price_wei, selector, status_ok, pools_touched })
}

fn hex_encode(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

async fn get_pool_token0(provider: &DynProvider, cache: &mut HashMap<Address, Address>, pool_addr: Address) -> Option<Address> {
    if let Some(t) = cache.get(&pool_addr) {
        return Some(*t);
    }
    let (t0, _t1) = pool::get_pair_tokens(provider, pool_addr).await.ok()?;
    cache.insert(pool_addr, t0);
    Some(t0)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("== competitor_recon (cum competitor-recon-and-strategy, F1) ==");
    println!("May chay: {}", if std::path::Path::new("/proc/version").exists()
        && std::fs::read_to_string("/proc/version").unwrap_or_default().to_lowercase().contains("microsoft")
    {
        "WSL"
    } else {
        "khong xac dinh qua /proc/version (co the VPS/Linux thuong)"
    });

    // Cum `bugfix-presign-and-contract-plan` (A5) - trinh sat la viec NEN:
    // uu tien `BSC_HTTP_BG`; rong thi lay 3 URL CUOI cua `BSC_HTTP` (khong
    // dung URL dau - do la duong nong cua bot neu bot dang chay song song).
    let bg = transport::filter_read_urls(transport::collect_rpc_urls_from_env("BSC_HTTP_BG"));
    let urls = if !bg.is_empty() {
        println!("(A5) dung BSC_HTTP_BG: {} URL", bg.len());
        bg
    } else {
        let all = transport::filter_read_urls(transport::collect_rpc_urls_from_env("BSC_HTTP"));
        if all.len() >= 2 {
            let start = all.len().saturating_sub(3).max(1);
            println!("(A5) BSC_HTTP_BG rong -> dung {} URL CUOI cua BSC_HTTP (bo qua URL dau = duong nong)", all.len() - start);
            all[start..].to_vec()
        } else {
            all
        }
    };
    if urls.is_empty() {
        println!("MISSING: BSC_HTTP rong trong .env — khong the trinh sat that. Dung: set -a; . .env; set +a; cargo run --release --bin competitor_recon");
        return Ok(());
    }
    let Some((url_label, provider)) = connect_first_working(&urls).await else {
        println!("MISSING: khong ket noi duoc RPC nao trong danh sach BSC_HTTP (da thu {} URL)", urls.len());
        return Ok(());
    };
    println!("RPC dung: {url_label}");
    let latest = provider.get_block_number().await?;
    println!("latest block THAT: {latest}");

    let chunk = probe_chunk_size(&provider, latest, swap_topic()).await;
    println!("chunk get_logs toi da URL nay chap nhan: {chunk} block/lan goi");

    let contract = Address::from_str(CONTRACT_EXECUTOR)?;
    let eoa = Address::from_str(EOA_TRADER)?;

    let skip_part_a = std::env::var("RECON_SKIP_PART_A").is_ok();
    let mut part_a_pools: HashMap<&'static str, HashSet<Address>> = HashMap::new();
    if skip_part_a {
        println!("\n(RECON_SKIP_PART_A dat - bo qua Phan A, chi chay Phan B de debug/lap lai nhanh)");
    }
    // ================= PHAN A =================
    for (label, addr) in [("contract", contract), ("eoa", eoa)] {
        if skip_part_a {
            break;
        }
        println!("\n================ PHAN A: contract {CONTRACT_EXECUTOR} + EOA {EOA_TRADER} ================");
        println!("selector nghi van: {SUSPECT_SELECTOR}");
        println!("\n--- {label} {addr:#x} ---");
        match provider.get_transaction_count(addr).await {
            Ok(n) => println!("  eth_getTransactionCount(latest) = {n} (nonce hien tai — tin hieu tho ve muc hoat dong tong)"),
            Err(e) => println!("  eth_getTransactionCount that bai: {e}"),
        }
        match provider.get_code_at(addr).await {
            Ok(code) => println!("  eth_getCode len = {} byte ({})", code.len(), if code.is_empty() { "EOA hoac contract rong" } else { "CO bytecode - la contract that" }),
            Err(e) => println!("  eth_getCode that bai: {e}"),
        }
        let (rows_sender, scanned1, calls1) =
            scan_swap_logs_for_role(&provider, latest, chunk, true, addr, PART_A_BLOCK_BUDGET, PART_A_WALL_BUDGET, PART_A_TARGET_TX).await;
        let (rows_to, scanned2, calls2) = scan_swap_logs_for_role(
            &provider,
            latest,
            chunk,
            false,
            addr,
            PART_A_BLOCK_BUDGET,
            PART_A_WALL_BUDGET.saturating_sub(PART_A_WALL_BUDGET.min(Duration::from_secs(0))),
            PART_A_TARGET_TX,
        )
        .await;
        let mut hash_set: HashSet<B256> = HashSet::new();
        for r in rows_sender.iter().chain(rows_to.iter()) {
            hash_set.insert(r.tx_hash);
        }
        println!(
            "  Swap-log truc tiep (sender hoac to = dia chi nay): {} tx duy nhat (quet {}+{} block, {}+{} lan goi getLogs)",
            hash_set.len(),
            scanned1,
            scanned2,
            calls1,
            calls2
        );

        if hash_set.len() < 10 {
            println!("  -> qua it (dia chi co the giao dich qua ROUTER, Swap.sender=router chu khong phai {label}) — fallback quet Transfer WBNB");
            let fb = scan_wbnb_transfer_hashes(&provider, latest, chunk, addr, PART_A_BLOCK_BUDGET, Duration::from_secs(90), PART_A_TARGET_TX).await;
            println!("  fallback Transfer WBNB: +{} tx hash", fb.len());
            hash_set.extend(fb);
        }

        let total_found = hash_set.len();
        let mut hashes: Vec<B256> = hash_set.into_iter().collect();
        hashes.truncate(PART_A_ANALYZE_CAP);
        println!("  Phan tich sau: {} / {} tx (gioi han PART_A_ANALYZE_CAP={})", hashes.len(), total_found, PART_A_ANALYZE_CAP);

        let mut pool_hits: HashMap<Address, u32> = HashMap::new();
        let mut multi_swap_same_pool_tx = 0u32;
        let mut selector_hist: HashMap<String, u32> = HashMap::new();
        let mut analyzed = 0u32;
        let mut pools_touched_set: HashSet<Address> = HashSet::new();
        let mut sample_gas_prices: Vec<u128> = Vec::new();
        let mut detail_cache: HashMap<B256, TxDetail> = HashMap::new();

        for h in &hashes {
            let Some(detail) = fetch_tx_detail(&provider, *h).await else { continue };
            analyzed += 1;
            if let Some(sel) = &detail.selector {
                *selector_hist.entry(sel.clone()).or_insert(0) += 1;
            }
            sample_gas_prices.push(detail.gas_price_wei);
            let mut seen_pools_this_tx: HashMap<Address, u32> = HashMap::new();
            for (p, _amt, _) in &detail.pools_touched {
                *pool_hits.entry(*p).or_insert(0) += 1;
                *seen_pools_this_tx.entry(*p).or_insert(0) += 1;
                pools_touched_set.insert(*p);
            }
            if seen_pools_this_tx.values().any(|c| *c >= 2) {
                multi_swap_same_pool_tx += 1;
            }
            detail_cache.insert(*h, detail);
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        println!("  tx phan tich thanh cong: {analyzed}/{}", hashes.len());
        println!("  selector histogram (top 5):");
        let mut sel_vec: Vec<_> = selector_hist.into_iter().collect();
        sel_vec.sort_by(|a, b| b.1.cmp(&a.1));
        for (sel, cnt) in sel_vec.iter().take(5) {
            println!("    {sel}: {cnt}");
        }
        println!("  tx co >=2 Swap CUNG 1 pool trong 1 tx (round-trip/atomic): {multi_swap_same_pool_tx}");
        println!("  so pool khac nhau da cham: {}", pools_touched_set.len());
        let mut pool_vec: Vec<_> = pool_hits.into_iter().collect();
        pool_vec.sort_by(|a, b| b.1.cmp(&a.1));
        println!("  top 10 pool ap dao:");
        for (p, c) in pool_vec.iter().take(10) {
            println!("    {p:#x}: {c} lan");
        }
        if !sample_gas_prices.is_empty() {
            let avg = sample_gas_prices.iter().sum::<u128>() as f64 / sample_gas_prices.len() as f64 / 1e9;
            let max = *sample_gas_prices.iter().max().unwrap() as f64 / 1e9;
            let min = *sample_gas_prices.iter().min().unwrap() as f64 / 1e9;
            println!("  gas_price (gwei): min={min:.3} avg={avg:.3} max={max:.3}");
        }

        // Thu debug_traceTransaction tren toi da 3 mau — ghi MISSING neu RPC khong ho tro.
        let mut trace_supported = false;
        let mut trace_examples = 0u32;
        for h in hashes.iter().take(3) {
            let res: Result<serde_json::Value, _> = provider
                .raw_request("debug_traceTransaction".into(), (format!("{h:#x}"), serde_json::json!({"tracer": "callTracer"})))
                .await;
            match res {
                Ok(_v) => {
                    trace_supported = true;
                    trace_examples += 1;
                }
                Err(e) => {
                    println!("  debug_traceTransaction({h:#x}) that bai: {e}");
                }
            }
        }
        if trace_supported {
            println!("  debug_traceTransaction: RPC nay HO TRO ({trace_examples}/3 mau thanh cong) — co the trich internal transfer sau (chua trich trong phien nay, ngoai pham vi thoi gian)");
        } else {
            println!("  debug_traceTransaction: MISSING (RPC public nay KHONG ho tro tracer) — khong the do internal-transfer coinbase bribe truc tiep, chi dung gas_price uu tien lam proxy");
        }

        part_a_pools.insert(label, pools_touched_set);
    }

    // ================= PHAN A2 (SUA GIUA PHIEN, sau bang chung that Chu dua) =================
    // Phat hien THAT (Chu xac nhan, da doi chieu lai bang RPC that o phien
    // nay): 0xa739 KHONG tu goi pair.swap() (dung Swap.sender/to tim khong
    // ra, dung ket luan cu "dormant" — SAI). Block 122070562: tx TU 0xB406
    // TOI 0xa739 (selector 0x5aab2274, KHOP) that su ton tai. Block
    // 122076185: cung tx do lam 2 viec — Transfer 2757.93 USDT tu 0xB406
    // toi 0xaaBae02D453823E0CE3C86f8A1d29d3Da0a3eaf7 + Approval(0xB406,
    // 0xa739) — RỒI tx KE TIEP CUNG BLOCK (0xaaBae -> Router V2, selector
    // 0x38ed1739) THUC HIEN SWAP that tren pool 0xcec13213c390d51121f82ba2ecafb8e11e0af7a3.
    // Ca 2 deu xac nhan DUNG bang eth_getBlockByNumber+eth_getTransactionReceipt
    // that trong phien nay (khong bia). Ket luan: day la 1 CUM nhieu dia chi
    // (vi goc cap von + executor cap quyen + vi giao dich thuc thi swap),
    // KHONG phai 1 dia chi don le — phuong phap Swap.sender/to cu BO SOT
    // hoan toan buoc "chuyen von" (khong phai Swap event) va vi giao dich
    // THAT (khac han 0xB406/0xa739).
    println!("\n================ PHAN A2 (SUA GIUA PHIEN): cluster theo dong tien cung block ================");
    let seed_addrs: [(&str, &str); 4] = [
        (EOA_TRADER, "vi goc (0xB406)"),
        (CONTRACT_EXECUTOR, "executor cap quyen/chuyen von (0xa739)"),
        ("0xaaBae02D453823E0CE3C86f8A1d29d3Da0a3eaf7", "vi giao dich (0xaaBae, doi chung block 122076185: nhan 2757.93 USDT tu 0xB406, swap ngay sau cung block)"),
        ("0xc412d20A348d91982192bA898756C16af790130b", "vi ban (0xc412, Chu cung cap dia chi tu log that)"),
    ];
    let mut cluster: HashSet<Address> = HashSet::new();
    for (a, label) in &seed_addrs {
        let addr = Address::from_str(a)?;
        cluster.insert(addr);
        println!("  seed: {addr:#x} — {label}");
    }

    // Quet Transfer USDT+WBNB TU 0xB406 (50.000 block gan nhat, ngan sach
    // nho hon Phan A vi chi 1 topic filter don, re hon nhieu so scan Swap
    // 2 chieu) — tim THEM cac vi duoc cap von (ung vien cung cum).
    const CLUSTER_BLOCK_BUDGET: u64 = 50_000;
    let cluster_wall_budget = Duration::from_secs(90);
    let usdt_transfers = scan_transfer_from_role(&provider, latest, chunk, usdt(), eoa, CLUSTER_BLOCK_BUDGET, cluster_wall_budget).await;
    let wbnb_transfers = scan_transfer_from_role(&provider, latest, chunk, wbnb(), eoa, CLUSTER_BLOCK_BUDGET, cluster_wall_budget).await;
    println!(
        "  Transfer USDT tu 0xB406 (50k block gan nhat): {} lan; Transfer WBNB: {} lan",
        usdt_transfers.len(),
        wbnb_transfers.len()
    );
    let mut recipient_counts: HashMap<Address, u32> = HashMap::new();
    for (to, _amt, _bn, _th) in usdt_transfers.iter().chain(wbnb_transfers.iter()) {
        *recipient_counts.entry(*to).or_insert(0) += 1;
    }
    let mut ranked: Vec<(Address, u32)> = recipient_counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1));
    println!("  Top dia chi nhan von THAT tu 0xB406 (ung vien cung cum, toi da 10 hien thi, toi da 8 dua vao cluster de bound RPC):");
    for (addr, cnt) in ranked.iter().take(10) {
        println!("    {addr:#x}: {cnt} lan nhan von");
    }
    for (addr, _cnt) in ranked.iter().take(8) {
        cluster.insert(*addr);
    }
    println!("  Tong so dia chi trong cluster (seed + phat hien): {}", cluster.len());

    // Voi MOI dia chi trong cluster, quet Swap-log (ngan sach nho hon Phan A
    // de bound RPC/thoi gian) de biet CLUSTER cham toi pool nao — hop nhat
    // vao cluster_pools.
    let mut cluster_pools: HashSet<Address> = HashSet::new();
    cluster_pools.extend(part_a_pools.get("contract").cloned().unwrap_or_default());
    cluster_pools.extend(part_a_pools.get("eoa").cloned().unwrap_or_default());
    for addr in cluster.iter().filter(|a| **a != contract && **a != eoa) {
        let (rows_sender, _s1, _c1) =
            scan_swap_logs_for_role(&provider, latest, chunk, true, *addr, CLUSTER_BLOCK_BUDGET, Duration::from_secs(30), 200).await;
        let (rows_to, _s2, _c2) =
            scan_swap_logs_for_role(&provider, latest, chunk, false, *addr, CLUSTER_BLOCK_BUDGET, Duration::from_secs(30), 200).await;
        for r in rows_sender.iter().chain(rows_to.iter()) {
            cluster_pools.insert(r.pool);
        }
    }
    println!("  Tong so pool cluster nay cham toi (hop nhat ca Phan A cu): {}", cluster_pools.len());
    for p in &cluster_pools {
        println!("    pool cluster: {p:#x}");
    }

    // ================= PHAN B =================
    println!("\n================ PHAN B: 126 pool pairs.txt, {PART_B_BLOCK_WINDOW} block gan nhat ================");
    let pairs_content = std::fs::read_to_string("pairs.txt").unwrap_or_default();
    let recon_logger = BotLogger::new("logs/recon.jsonl")?;
    let factory = Address::from_str(venues::V2_FACTORY_ADDRESS)?;
    let resolver = RpcPairResolver { provider: provider.clone(), factory, url_label: url_label.clone() };
    let mut book = PairBook::new();
    book.reload(&pairs_content, &resolver, &recon_logger, Instant::now(), true).await;
    let pool_list: Vec<(Address, Address, Option<String>)> =
        book.entries().map(|e| (e.pair_addr, e.quote, e.symbol.clone())).collect();
    println!("PairBook resolve thanh cong: {} pool (tu pairs.txt that)", pool_list.len());
    let wbnb_pools: Vec<(Address, Option<String>)> =
        pool_list.iter().filter(|(_, q, _)| *q == wbnb()).map(|(p, _, s)| (*p, s.clone())).collect();
    let usdt_pools_n = pool_list.iter().filter(|(_, q, _)| *q == usdt()).count();
    println!("  quote=WBNB: {} pool (chi cac pool nay dung nguong 0.05 BNB); quote=USDT: {} pool (bo qua nguong BNB, ngoai pham vi phan tich sau)", wbnb_pools.len(), usdt_pools_n);

    let lo_block = latest.saturating_sub(PART_B_BLOCK_WINDOW.saturating_sub(1));
    let topic0 = swap_topic();
    let mut all_rows: Vec<SwapRow> = Vec::new();
    let mut batch_start = 0usize;
    let addrs_all: Vec<Address> = wbnb_pools.iter().map(|(p, _)| *p).collect();
    let mut total_calls = 0u64;
    while batch_start < addrs_all.len() {
        let batch_end = (batch_start + PART_B_ADDR_BATCH).min(addrs_all.len());
        let batch = &addrs_all[batch_start..batch_end];
        let mut c = lo_block;
        while c <= latest {
            let c_hi = (c + chunk - 1).min(latest);
            let f = Filter::new().address(batch.to_vec()).event_signature(topic0).from_block(c).to_block(c_hi);
            total_calls += 1;
            match get_logs_retry(&provider, &f).await {
                Ok(logs) => {
                    for l in &logs {
                        let (Some(bn), Some(ti), Some(li), Some(th)) =
                            (l.block_number, l.transaction_index, l.log_index, l.transaction_hash)
                        else {
                            continue;
                        };
                        let topics = l.topics();
                        if topics.len() < 3 {
                            continue;
                        }
                        let sender = topic_to_address(&topics[1]);
                        let to = topic_to_address(&topics[2]);
                        if let Some(amounts) = decode_swap_data(l.data().data.as_ref()) {
                            all_rows.push(SwapRow { block: bn, tx_index: ti, log_index: li, tx_hash: th, pool: l.address(), sender, to, amounts });
                        }
                    }
                }
                Err(e) => {
                    println!("  eth_getLogs batch [{c}..{c_hi}] {} addr LOI SAU RETRY: {e}", batch.len());
                }
            }
            c = c_hi + 1;
            tokio::time::sleep(Duration::from_millis(90)).await;
        }
        batch_start = batch_end;
    }
    println!("eth_getLogs Swap tren {} pool WBNB, {PART_B_BLOCK_WINDOW} block [{lo_block}..{latest}]: {} log THAT ({} lan goi)", addrs_all.len(), all_rows.len(), total_calls);

    let mut token0_cache: HashMap<Address, Address> = HashMap::new();
    // Nhom theo (pool, block) de kiem +-3 vi tri KHONG can goi RPC them.
    let mut by_pool_block: HashMap<(Address, u64), Vec<SwapRow>> = HashMap::new();
    for r in &all_rows {
        by_pool_block.entry((r.pool, r.block)).or_default().push(r.clone());
    }
    for v in by_pool_block.values_mut() {
        v.sort_by_key(|r| r.tx_index);
    }

    struct PoolStat {
        symbol: Option<String>,
        victim_count: u32,
        bracketed_count: u32,
        /// Cụm SỬA GIỮA PHIÊN — victim có neighbor ±3 mà `sender`/`to` thuộc
        /// `cluster` (không chỉ 2 địa chỉ đơn lẻ cũ).
        cluster_bracket_count: u32,
        top_neighbor: HashMap<Address, u32>,
    }
    let mut pool_stats: HashMap<Address, PoolStat> = HashMap::new();
    for (p, s) in &wbnb_pools {
        pool_stats.insert(
            *p,
            PoolStat { symbol: s.clone(), victim_count: 0, bracketed_count: 0, cluster_bracket_count: 0, top_neighbor: HashMap::new() },
        );
    }

    let mut victim_neighbor_pairs: Vec<(SwapRow, SwapRow)> = Vec::new();
    for ((pool_addr, block), rows) in &by_pool_block {
        let Some(t0) = get_pool_token0(&provider, &mut token0_cache, *pool_addr).await else { continue };
        for (i, row) in rows.iter().enumerate() {
            // xac dinh phia WBNB cua Swap nay (in hoac out) de doi ra BNB.
            let wbnb_amount: U256 = if t0 == wbnb() {
                row.amounts.amount0_in.max(row.amounts.amount0_out)
            } else {
                row.amounts.amount1_in.max(row.amounts.amount1_out)
            };
            if wbnb_amount < U256::from(PART_B_VICTIM_MIN_WEI) {
                continue;
            }
            if let Some(stat) = pool_stats.get_mut(pool_addr) {
                stat.victim_count += 1;
            }
            let mut found_neighbor = false;
            let mut found_cluster_neighbor = false;
            for j in 0..rows.len() {
                if j == i {
                    continue;
                }
                let other = &rows[j];
                let dist = if other.tx_index >= row.tx_index { other.tx_index - row.tx_index } else { row.tx_index - other.tx_index };
                if dist >= 1 && dist <= 3 {
                    found_neighbor = true;
                    victim_neighbor_pairs.push((row.clone(), other.clone()));
                    if let Some(stat) = pool_stats.get_mut(pool_addr) {
                        *stat.top_neighbor.entry(other.sender).or_insert(0) += 1;
                    }
                    if cluster.contains(&other.sender) || cluster.contains(&other.to) {
                        found_cluster_neighbor = true;
                    }
                }
            }
            if found_neighbor {
                if let Some(stat) = pool_stats.get_mut(pool_addr) {
                    stat.bracketed_count += 1;
                }
            }
            if found_cluster_neighbor {
                if let Some(stat) = pool_stats.get_mut(pool_addr) {
                    stat.cluster_bracket_count += 1;
                }
            }
            let _ = block;
        }
    }

    println!("\nvictim >=0.05 BNB tim thay (toan bo pool WBNB, {PART_B_BLOCK_WINDOW} block): {}", pool_stats.values().map(|s| s.victim_count).sum::<u32>());
    println!("victim co it nhat 1 tx khac CUNG pool trong +-3 vi tri: {}", victim_neighbor_pairs.len());

    // Doi chieu voi tap pool Phan A cham toi (contract + eoa DON LE, PHUONG
    // PHAP CU) VA tap pool cluster (Phan A2, PHUONG PHAP MOI SAU SUA).
    let empty_set = HashSet::new();
    let contract_pools = part_a_pools.get("contract").unwrap_or(&empty_set);
    let eoa_pools = part_a_pools.get("eoa").unwrap_or(&empty_set);

    println!("\nBang doi thu theo pool — CU (dia chi don le, TRUOC khi sua) (chi pool co victim >=1, sap theo victim_count giam dan):");
    println!("{:<44} {:<10} {:>8} {:>10} {:>10} {:>8}", "pool", "symbol", "victim", "bracket%", "top_addr(lan)", "bi_0xB406_phu(CU)");
    let mut rows_out: Vec<(&Address, &PoolStat)> = pool_stats.iter().filter(|(_, s)| s.victim_count > 0).collect();
    rows_out.sort_by(|a, b| b.1.victim_count.cmp(&a.1.victim_count));
    for (p, s) in rows_out.iter().take(40) {
        let pct = if s.victim_count > 0 { s.bracketed_count as f64 / s.victim_count as f64 * 100.0 } else { 0.0 };
        let top = s.top_neighbor.iter().max_by_key(|(_, c)| **c);
        let top_str = match top {
            Some((addr, c)) => format!("{}...{} ({c})", &format!("{addr:#x}")[0..8], &format!("{addr:#x}")[36..42]),
            None => "-".to_string(),
        };
        let covered = if contract_pools.contains(*p) || eoa_pools.contains(*p) { "CO" } else { "khong" };
        println!(
            "{:<44} {:<10} {:>8} {:>9.1}% {:>18} {:>8}",
            format!("{p:#x}"),
            s.symbol.clone().unwrap_or_default(),
            s.victim_count,
            pct,
            top_str,
            covered
        );
    }
    let opportunity_pools_old: Vec<_> = rows_out
        .iter()
        .filter(|(p, s)| s.bracketed_count == 0 && !contract_pools.contains(*p) && !eoa_pools.contains(*p) && s.victim_count > 0)
        .collect();
    println!(
        "\nKET LUAN CU (PHUONG PHAP DIA CHI DON LE, TRUOC SUA — SAI/THIEU, CHI DE DOI CHIEU): {}/{} pool co victim NHUNG khong thay bracket (+-3) va khong bi contract/EOA don le cham toi.",
        opportunity_pools_old.len(),
        rows_out.len()
    );

    println!("\nBang doi thu theo pool — MOI (theo CUM dia chi, SAU KHI SUA giua phien) (chi pool co victim >=1, sap theo victim_count giam dan):");
    println!("{:<44} {:<10} {:>8} {:>14} {:>10}", "pool", "symbol", "victim", "cluster_bracket%", "bi_cum_phu");
    for (p, s) in rows_out.iter().take(40) {
        let cluster_pct = if s.victim_count > 0 { s.cluster_bracket_count as f64 / s.victim_count as f64 * 100.0 } else { 0.0 };
        let covered_cluster = if cluster_pools.contains(*p) { "CO" } else { "khong" };
        println!(
            "{:<44} {:<10} {:>8} {:>13.1}% {:>10}",
            format!("{p:#x}"),
            s.symbol.clone().unwrap_or_default(),
            s.victim_count,
            cluster_pct,
            covered_cluster
        );
    }
    let opportunity_pools_new: Vec<_> = rows_out
        .iter()
        .filter(|(p, s)| s.cluster_bracket_count == 0 && !cluster_pools.contains(*p) && s.victim_count > 0)
        .collect();
    println!(
        "\nKET LUAN MOI (THEO CUM, SAU KHI SUA): {}/{} pool co victim NHUNG khong thay cluster_bracket (+-3 khop dia chi cum) VA khong bi cluster cham toi.",
        opportunity_pools_new.len(),
        rows_out.len()
    );
    println!(
        "\nSO SANH TRUOC/SAU: opportunity_pools CU={}/{}  MOI={}/{}  (cluster co {} dia chi, cham {} pool)",
        opportunity_pools_old.len(),
        rows_out.len(),
        opportunity_pools_new.len(),
        rows_out.len(),
        cluster.len(),
        cluster_pools.len()
    );
    println!("KHONG ket luan backrun-only/sandwich o day — cho bang theo cum nay la du lieu chinh, Chu/Grok tu doc so sanh de quyet dinh.");

    println!("\n== HET PHAN A+A2+B (du lieu THAT qua RPC {url_label}, block latest={latest}) ==");
    Ok(())
}

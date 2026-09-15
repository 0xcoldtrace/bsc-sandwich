//! Cụm `rpc-probe` — đo RTT/chain_ok cho từng URL RPC lấy từ env, KHÔNG
//! gửi tx, KHÔNG sửa `.env`/`config.toml`/`victims.txt`. Dùng lại ĐÚNG parser
//! đa-URL của cụm `5.3` (`transport::collect_rpc_urls_from_env` +
//! `transport::filter_read_urls`) qua lib crate `bsc_sandwich` — không chép
//! lại logic parse CSV/`_2`.._16`/`_LIST` riêng cho binary này (tránh lệch
//! hành vi với bot chính nếu 1 trong 2 nơi sửa mà quên nơi kia).
//!
//! Không có `.env`/biến env RPC nào set trong shell hiện tại -> in `MISSING`,
//! không panic, exit code 0 (đây không phải lỗi chương trình, chỉ là chưa có
//! gì để đo — đúng CLAUDE.md mục 0.ANTI "Không chắc -> MISSING").
//!
//! Timeout mỗi cuộc gọi RPC: 2s (đúng lệnh). 5 mẫu `eth_chainId` +
//! `eth_blockNumber` mỗi URL (connect 1 lần/URL, 5 vòng gọi lại trên cùng
//! kết nối — mô phỏng đúng cách bot chính dùng 1 provider lâu dài, không
//! phải benchmark handshake TCP/TLS mỗi lần). Ghi `artifacts/rpc_probe.json`
//! (đã redact — chỉ scheme+host+port, không path/query/token).

use alloy::providers::{Provider, ProviderBuilder};
use bsc_sandwich::transport;
use std::time::{Duration, Instant};

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const SAMPLES: usize = 5;
const REQUIRED_CHAIN_ID: u64 = 56;

#[derive(Debug, Clone, serde::Serialize)]
struct UrlProbeResult {
    transport: String,
    host_redacted: String,
    samples_ok: usize,
    rtt_ms_min: Option<f64>,
    rtt_ms_p50: Option<f64>,
    rtt_ms_p95: Option<f64>,
    chain_ok: bool,
    err: Option<String>,
}

fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let idx = (((sorted.len() - 1) as f64) * p).round() as usize;
    sorted.get(idx).copied()
}

/// Kết nối 1 lần (timeout 2s) rồi lấy `SAMPLES` mẫu `eth_chainId` +
/// `eth_blockNumber` trên CÙNG kết nối đó (mỗi mẫu vẫn có timeout riêng 2s).
/// Connect lỗi/timeout -> trả ngay 0 mẫu, `chain_ok=false`, `err` = lý do
/// thật (không bịa). Không panic ở bất kỳ nhánh nào (URL chết là kỳ vọng
/// bình thường của công cụ đo, không phải lỗi chương trình).
async fn probe_url(url: &str, transport_label: &str) -> UrlProbeResult {
    let host_redacted = transport::redact_rpc_url(url);
    let connect_fut = ProviderBuilder::new().connect(url);
    let provider = match tokio::time::timeout(PROBE_TIMEOUT, connect_fut).await {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => {
            return UrlProbeResult {
                transport: transport_label.to_string(),
                host_redacted,
                samples_ok: 0,
                rtt_ms_min: None,
                rtt_ms_p50: None,
                rtt_ms_p95: None,
                chain_ok: false,
                err: Some(shorten_err(format!("connect loi: {e}"))),
            }
        }
        Err(_) => {
            return UrlProbeResult {
                transport: transport_label.to_string(),
                host_redacted,
                samples_ok: 0,
                rtt_ms_min: None,
                rtt_ms_p50: None,
                rtt_ms_p95: None,
                chain_ok: false,
                err: Some(shorten_err(format!("connect timeout {}s", PROBE_TIMEOUT.as_secs()))),
            }
        }
    };

    let mut rtts_ms: Vec<f64> = Vec::with_capacity(SAMPLES);
    let mut chain_ok = true;
    let mut last_err: Option<String> = None;

    for _ in 0..SAMPLES {
        let start = Instant::now();
        let chain_res = tokio::time::timeout(PROBE_TIMEOUT, provider.get_chain_id()).await;
        let block_res = tokio::time::timeout(PROBE_TIMEOUT, provider.get_block_number()).await;
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

        match (chain_res, block_res) {
            (Ok(Ok(chain_id)), Ok(Ok(_block))) => {
                if chain_id != REQUIRED_CHAIN_ID {
                    chain_ok = false;
                    last_err = Some(shorten_err(format!("sai chain: tra ve {chain_id}, can {REQUIRED_CHAIN_ID}")));
                }
                rtts_ms.push(elapsed_ms);
            }
            (Ok(Err(e)), _) => {
                chain_ok = false;
                last_err = Some(shorten_err(format!("eth_chainId loi: {e}")));
            }
            (_, Ok(Err(e))) => {
                chain_ok = false;
                last_err = Some(shorten_err(format!("eth_blockNumber loi: {e}")));
            }
            _ => {
                chain_ok = false;
                last_err = Some(shorten_err(format!("mau timeout {}s", PROBE_TIMEOUT.as_secs())));
            }
        }
    }

    rtts_ms.sort_by(|a, b| a.partial_cmp(b).expect("RTT ms khong bao gio NaN"));

    UrlProbeResult {
        transport: transport_label.to_string(),
        host_redacted,
        samples_ok: rtts_ms.len(),
        rtt_ms_min: rtts_ms.first().copied(),
        rtt_ms_p50: percentile(&rtts_ms, 0.50),
        rtt_ms_p95: percentile(&rtts_ms, 0.95),
        chain_ok: chain_ok && !rtts_ms.is_empty(),
        err: last_err,
    }
}

fn fmt_ms(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{x:.1}"),
        None => "-".to_string(),
    }
}

/// Vài RPC công khai trả lỗi rate-limit kèm NGUYÊN VĂN body HTML/JSON dài
/// (vd trang chặn Cloudflare, thông báo nâng gói) trong `Display` của lỗi —
/// không phải secret (không chứa URL/token của `.env` chủ, đã xác nhận bằng
/// mắt khi verify runtime), nhưng làm bảng/JSON khó đọc. Cắt về 1 dòng, tối
/// đa 160 ký tự, không đổi Ý NGHĨA lỗi (vẫn giữ đủ để biết vì sao URL đó
/// `chain_ok=false`).
fn shorten_err(e: String) -> String {
    let one_line: String = e.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() > 160 {
        let truncated: String = one_line.chars().take(157).collect();
        format!("{truncated}...")
    } else {
        one_line
    }
}

#[tokio::main]
async fn main() {
    // Cùng parser cụm `5.3`: uu tien `<base>_LIST` (phay), fallback
    // `<base>` + `<base>_2`..`<base>_16` (moi field tu tach dau phay).
    let http_urls = transport::filter_read_urls(transport::collect_rpc_urls_from_env("BSC_HTTP"));
    let ws_urls = transport::filter_read_urls(transport::collect_rpc_urls_from_env("BSC_WS"));

    if http_urls.is_empty() && ws_urls.is_empty() {
        println!("MISSING: khong co BSC_HTTP*/BSC_WS* nao trong bien moi truong hien tai.");
        println!("(File .env khong tu dong duoc nap boi binary nay - can 'source'/export truoc khi chay,");
        println!(" xem scripts/run_rpc_probe.sh hoac scripts/run_rpc_probe.ps1.)");
        return;
    }

    println!(
        "rpc_probe: {} URL HTTP + {} URL WSS (da loc maxbackrun/fullprivacy/privacy), {} mau/URL, timeout {}s/goi",
        http_urls.len(),
        ws_urls.len(),
        SAMPLES,
        PROBE_TIMEOUT.as_secs()
    );

    let mut results: Vec<UrlProbeResult> = Vec::new();
    for u in &http_urls {
        results.push(probe_url(u, "http").await);
    }
    for u in &ws_urls {
        results.push(probe_url(u, "ws").await);
    }

    println!(
        "{:<6} {:<38} {:>4} {:>9} {:>9} {:>9} {:<9} err",
        "tr", "host", "ok", "min_ms", "p50_ms", "p95_ms", "chain_ok"
    );
    for r in &results {
        println!(
            "{:<6} {:<38} {:>4} {:>9} {:>9} {:>9} {:<9} {}",
            r.transport,
            r.host_redacted,
            r.samples_ok,
            fmt_ms(r.rtt_ms_min),
            fmt_ms(r.rtt_ms_p50),
            fmt_ms(r.rtt_ms_p95),
            r.chain_ok,
            r.err.clone().unwrap_or_default(),
        );
    }

    if let Err(e) = std::fs::create_dir_all("artifacts") {
        eprintln!("khong tao duoc thu muc artifacts/: {e} (bang tren van la ket qua that)");
        return;
    }
    match serde_json::to_string_pretty(&results) {
        Ok(json) => match std::fs::write("artifacts/rpc_probe.json", &json) {
            Ok(()) => println!("da ghi artifacts/rpc_probe.json ({} URL, da redact)", results.len()),
            Err(e) => eprintln!("khong ghi duoc artifacts/rpc_probe.json: {e}"),
        },
        Err(e) => eprintln!("serialize loi (khong bao gio xay ra voi struct nay): {e}"),
    }
}

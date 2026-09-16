//! Cụm `verify-cluster-as-victim` (mục 3) — ĐẦU DÒ MEMPOOL THUẦN, CHỈ ĐỌC.
//!
//! # Vì sao cần binary riêng
//!
//! `logs/bot.jsonl` KHÔNG trả lời được câu hỏi "cụm đối thủ có gửi private
//! không": bot chỉ ghi `tx.seen` cho tx đã qua **gate router** (`tx.to` phải
//! là 1 trong 5 router Pancake đã pin — `main.rs::passes_router_gate`). Phần
//! lớn tx của cụm đi tới CONTRACT RIÊNG của họ (`0x8180…`, selector
//! `0xecb65f51`), nên chúng không bao giờ sinh dòng `tx.seen` **dù bot có
//! nhìn thấy chúng trong mempool**. Lấy sự vắng mặt của `tx.seen` làm bằng
//! chứng "họ gửi private" sẽ là kết luận SAI.
//!
//! Binary này subscribe ĐÚNG CÙNG `BSC_WS` mà bot dùng và ghi **mọi** hash
//! pending kèm dấu thời gian, không lọc gì cả. Đối chiếu danh sách đó với tx
//! của cụm đã lên chain mới cho ra tỉ lệ "lên chain mà chưa từng xuất hiện
//! trong mempool công khai" đúng nghĩa.
//!
//! KHÔNG ký, KHÔNG gửi tx, KHÔNG in URL RPC đầy đủ.
//!
//! Dùng: `cargo run --release --bin mempool_probe -- --minutes 60 --out logs/mempool_probe.jsonl`

use alloy::providers::Provider;
use std::io::Write;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = bsc_sandwich::transport::load_dotenv_defaults(std::path::Path::new(".env"));
    let mut minutes: u64 = 60;
    let mut out_path = "logs/mempool_probe.jsonl".to_string();
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--minutes" => {
                minutes = args[i + 1].parse()?;
                i += 2;
            }
            "--out" => {
                out_path = args[i + 1].clone();
                i += 2;
            }
            other => return Err(format!("tham so la: {other}").into()),
        }
    }
    let urls = bsc_sandwich::transport::collect_rpc_urls_from_env("BSC_WS");
    if urls.is_empty() {
        eprintln!("MISSING: BSC_WS rong trong .env");
        std::process::exit(2);
    }
    let url = urls[0].clone();
    eprintln!("mempool_probe: ws = {} ; {minutes} phut ; out = {out_path}", bsc_sandwich::transport::redact_rpc_url(&url));
    let provider = bsc_sandwich::transport::connect_and_verify(&url).await?;
    let chain = provider.get_chain_id().await?;
    eprintln!("eth_chainId = {chain} (0x{chain:x})");
    if chain != 56 {
        return Err(format!("chain_id != 56: {chain}").into());
    }
    // Kênh phải ĐỦ RỘNG: mempool BSC đẩy vài nghìn hash/giây, kênh mặc định
    // (nhỏ) làm `recv()` trả `Lagged` ngay lập tức và đầu dò chết sau vài
    // chục hash — đã quan sát thật ("channel lagged by 19").
    let mut sub = provider
        .subscribe_pending_transactions()
        .channel_size(bsc_sandwich::transport::PENDING_WS_CHANNEL_SIZE)
        .await?;
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out_path)?);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(minutes * 60);
    let mut n: u64 = 0;
    let mut lagged: u64 = 0;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            break;
        }
        match tokio::time::timeout(left, sub.recv()).await {
            Ok(Ok(hash)) => {
                let ts = chrono::Utc::now().to_rfc3339();
                writeln!(f, "{{\"hash\":\"{hash:#x}\",\"ts\":\"{ts}\"}}")?;
                n += 1;
                if n % 20_000 == 0 {
                    f.flush()?;
                    eprintln!("  {n} hash...");
                }
            }
            Ok(Err(e)) => {
                // `Lagged` = đầu dò ghi chậm hơn mempool đẩy; KHÔNG phải mất
                // kết nối. Đếm số hash bị bỏ (con số này phải dán vào báo cáo
                // — nó là sai số của phép đo "bot không thấy tx nào") rồi chạy
                // tiếp, thay vì chết im lặng.
                let msg = e.to_string();
                if msg.contains("lagged") {
                    lagged += 1;
                    continue;
                }
                eprintln!("subscription rot sau {n} hash: {e}");
                break;
            }
            Err(_) => break,
        }
    }
    f.flush()?;
    eprintln!("XONG: {n} hash pending ({lagged} lan lagged) -> {out_path}");
    Ok(())
}

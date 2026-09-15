//! Cụm 2.1 — HTTP/WSS thật qua `alloy-provider` (2.4.2). Paper-safe: file này
//! không có hàm gửi giao dịch nào (không `send_raw_transaction`).
//!
//! Nếu không kết nối được (chưa có `.env`/`BSC_HTTP` thật, hoặc placeholder
//! trong `vps.json`) thì trả `Err` cho caller tự log `rpc.skip` — boot vẫn
//! chạy, không panic, không halt (CLAUDE.md: "Không halt vì WSS im (ship)").
//!
//! `ProviderBuilder::connect(url)` (alias cũ `on_builtin`) tự nhận diện
//! scheme http(s)/ws(s) nên dùng chung một hàm cho cả `BSC_HTTP` và `BSC_WS`.

use alloy::primitives::{Address, Bytes, B256, U256};
use alloy::providers::{DynProvider, Provider, ProviderBuilder};
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::logger::BotLogger;

pub const REQUIRED_CHAIN_ID: u64 = 56;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Cụm `5.3` — số item buffer cho channel subscription pending-tx qua WSS
/// (mặc định `alloy` chỉ 16, xem `alloy-pubsub::PubSubFrontend::new`) — BAOCAO08
/// đã chứng minh runtime THẬT bị "channel lagged by 24" chỉ sau ~1s với mặc
/// định 16 (mempool BSC thật bắn tx nhanh hơn tốc độ xử lý mỗi tx qua
/// `eth_call` resolve reserve). Nâng lên để chịu được burst dài hơn trước khi
/// phải failover sang WSS khác/txpool — vẫn không đoán mò, `GetSubscription::channel_size`
/// là API alloy cho phép chỉnh thật (`alloy-provider::provider::subscription.rs`).
pub const PENDING_WS_CHANNEL_SIZE: usize = 4096;

#[derive(Debug)]
pub enum ConnectError {
    Transport(String),
    ChainMismatch(u64),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectError::Transport(e) => write!(f, "rpc khong ket noi duoc: {e}"),
            ConnectError::ChainMismatch(id) => {
                write!(f, "sai chain: rpc tra ve chain_id={id}, can {REQUIRED_CHAIN_ID}")
            }
        }
    }
}

impl std::error::Error for ConnectError {}

/// Kết nối (http/https/ws/wss tự nhận diện qua scheme) rồi xác nhận
/// `eth_chainId == 56`. Sai chain là lỗi rõ ràng (`ChainMismatch`), không
/// âm thầm coi như đã kết nối thành công theo đúng CLAUDE.md mục 2.1.
pub async fn connect_and_verify(url: &str) -> Result<DynProvider, ConnectError> {
    let connect_fut = ProviderBuilder::new().connect(url);
    let provider = tokio::time::timeout(CONNECT_TIMEOUT, connect_fut)
        .await
        .map_err(|_| ConnectError::Transport(format!("timeout {}s", CONNECT_TIMEOUT.as_secs())))?
        .map_err(|e| ConnectError::Transport(e.to_string()))?;

    let chain_id = tokio::time::timeout(CONNECT_TIMEOUT, provider.get_chain_id())
        .await
        .map_err(|_| ConnectError::Transport(format!("eth_chainId timeout {}s", CONNECT_TIMEOUT.as_secs())))?
        .map_err(|e| ConnectError::Transport(format!("eth_chainId that bai: {e}")))?;

    if chain_id != REQUIRED_CHAIN_ID {
        return Err(ConnectError::ChainMismatch(chain_id));
    }
    Ok(provider.erased())
}

/// Redact URL trước khi log — chỉ giữ scheme + host (+port), bỏ path/query
/// (nơi thường chứa API key của nhà cung cấp RPC). Không bao giờ log
/// `BSC_HTTP`/`BSC_WS` thật nguyên văn.
pub fn redact_rpc_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(u) => {
            let scheme = u.scheme();
            let host = u.host_str().unwrap_or("?");
            let port = u.port().map(|p| format!(":{p}")).unwrap_or_default();
            format!("{scheme}://{host}{port}/***")
        }
        Err(_) => "***invalid_url***".to_string(),
    }
}

/// Đọc `rpc_http`/`rpc_ws` fallback từ `vps.json` khi biến môi trường tương
/// ứng rỗng/không có. `vps.json` mặc định chứa placeholder
/// (`REPLACE_ME_..._RPC_URL`) — không phải RPC thật, chỉ để boot không
/// panic khi thiếu cả `.env` lẫn cấu hình thật.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct VpsFallback {
    #[serde(default)]
    pub rpc_http: String,
    #[serde(default)]
    pub rpc_ws: String,
}

impl VpsFallback {
    pub fn load(path: &std::path::Path) -> VpsFallback {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(VpsFallback { rpc_http: String::new(), rpc_ws: String::new() })
    }
}

/// Ưu tiên biến môi trường (từ `.env` chủ điền), rỗng thì fallback `vps.json`.
/// Không panic nếu cả hai đều rỗng — trả `None`, caller log `rpc.skip`.
pub fn pick_url(env_val: Option<String>, vps_fallback: &str) -> Option<String> {
    let env_val = env_val.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if let Some(v) = env_val {
        return Some(v);
    }
    let fb = vps_fallback.trim();
    if fb.is_empty() {
        None
    } else {
        Some(fb.to_string())
    }
}

fn split_csv_urls(v: &str) -> Vec<String> {
    v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// Cụm `5.3` — đọc danh sách nhiều URL RPC theo đúng thứ tự lệnh: ưu tiên
/// `<base>_LIST` (phẩy, trim, bỏ rỗng) NẾU có ít nhất 1 phần tử hợp lệ; không
/// thì gộp `<base>` (tương thích ngược với `pick_url`) + `<base>_2` ..
/// `<base>_16`. MỖI giá trị đọc được (kể cả `<base>` gốc) đều được tách theo
/// dấu phẩy trước khi thêm vào danh sách — phát hiện thật khi verify runtime
/// phiên này: `.env` chủ đã điền SẴN `BSC_HTTP=` là 1 chuỗi 34 URL nối bằng
/// dấu phẩy (không phải `BSC_HTTP_LIST`) — URL hợp lệ không bao giờ chứa dấu
/// phẩy chưa mã hoá, nên tách theo `,` ở MỌI field là an toàn tuyệt đối, không
/// làm hỏng trường hợp 1 URL đơn (không phẩy thì tách ra đúng 1 phần tử).
/// `get_var` tách khỏi `std::env::var` thật để test được không cần env thật
/// (tránh test song song ghi đè biến môi trường global).
pub fn parse_rpc_url_list(get_var: impl Fn(&str) -> Option<String>, base: &str) -> Vec<String> {
    let list_key = format!("{base}_LIST");
    if let Some(list) = get_var(&list_key) {
        let urls = split_csv_urls(&list);
        if !urls.is_empty() {
            return urls;
        }
    }
    let mut urls = Vec::new();
    if let Some(v) = get_var(base) {
        urls.extend(split_csv_urls(&v));
    }
    for i in 2..=16 {
        let key = format!("{base}_{i}");
        if let Some(v) = get_var(&key) {
            urls.extend(split_csv_urls(&v));
        }
    }
    urls
}

/// Wrapper dùng `std::env::var` thật — điểm gọi production duy nhất
/// (`main.rs`), tách riêng khỏi `parse_rpc_url_list` (thuần, test bằng closure
/// giả) để không phải mock biến môi trường thật trong test.
pub fn collect_rpc_urls_from_env(base: &str) -> Vec<String> {
    parse_rpc_url_list(|k| std::env::var(k).ok(), base)
}

/// Cụm `5.3` — URL kênh gửi/private (`maxbackrun`/`fullprivacy`/`privacy`
/// trong host, theo đúng lệnh) — KHÔNG dùng làm provider READ chính
/// (`eth_call`/`getBlock`/`txpool_content`), chỉ giữ lại để dùng cho kênh gửi
/// live sau này (`7.x`, chưa làm ở cụm này).
pub fn is_private_send_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("maxbackrun") || lower.contains("fullprivacy") || lower.contains("privacy")
}

/// Lọc bỏ URL kênh gửi/private khỏi danh sách dùng cho pool READ (HTTP).
pub fn filter_read_urls(urls: Vec<String>) -> Vec<String> {
    urls.into_iter().filter(|u| !is_private_send_url(u)).collect()
}

#[derive(Debug, Default)]
struct RpcPoolState {
    idx: usize,
    provider: Option<DynProvider>,
}

/// Cụm `5.3` — pool nhiều URL HTTP, failover round-robin khi 1 node chết/sai
/// chain/timeout. Danh sách URL cố định tại construction (`Vec<String>`,
/// không đổi sau khi tạo — chỉ `idx`/`provider` hiện tại đổi qua
/// `RwLock<RpcPoolState>`). Không bao giờ panic — hết danh sách mà không URL
/// nào connect được thì trả `None`, caller tự quyết định (log `rpc.skip`,
/// không halt bot).
pub struct RpcPool {
    urls: Vec<String>,
    state: RwLock<RpcPoolState>,
}

impl RpcPool {
    pub fn new(urls: Vec<String>) -> Self {
        RpcPool { urls, state: RwLock::new(RpcPoolState::default()) }
    }

    pub fn is_empty(&self) -> bool {
        self.urls.is_empty()
    }

    /// Thử kết nối, bắt đầu từ `idx` hiện tại, quay đúng 1 vòng qua toàn bộ
    /// danh sách (`quay vòng` — CLAUDE.md lệnh `5.3`). Mỗi URL lỗi (sai chain
    /// / timeout / transport) log `rpc.failover` (redact, không lộ token).
    /// URL đầu tiên connect được thì lưu lại (`idx` + `provider`), log
    /// `rpc.connect`, trả `Some`. Hết danh sách -> `None`, KHÔNG panic.
    pub async fn connect(&self, logger: &BotLogger, transport_label: &str) -> Option<DynProvider> {
        if self.urls.is_empty() {
            return None;
        }
        let start_idx = self.state.read().await.idx;
        for step in 0..self.urls.len() {
            let idx = (start_idx + step) % self.urls.len();
            let url = &self.urls[idx];
            let redacted = redact_rpc_url(url);
            match connect_and_verify(url).await {
                Ok(provider) => {
                    let mut st = self.state.write().await;
                    st.idx = idx;
                    st.provider = Some(provider.clone());
                    logger.log(
                        "rpc.connect",
                        serde_json::json!({ "transport": transport_label, "url": redacted, "pool_index": idx, "pool_size": self.urls.len() }),
                    );
                    return Some(provider);
                }
                Err(e) => {
                    logger.log(
                        "rpc.failover",
                        serde_json::json!({ "transport": transport_label, "url": redacted, "pool_index": idx, "pool_size": self.urls.len(), "reason": e.to_string() }),
                    );
                }
            }
        }
        None
    }

    /// Provider hiện tại (đã connect trước đó), KHÔNG thử kết nối lại —
    /// dùng cho call runtime bình thường (`eth_call`/`getBlock`/`txpool`).
    pub async fn current(&self) -> Option<DynProvider> {
        self.state.read().await.provider.clone()
    }

    /// Gọi khi 1 call runtime qua provider hiện tại lỗi transport — chuyển
    /// sang URL KẾ (idx+1, quay vòng), thử connect lại (log `rpc.failover`/
    /// `rpc.connect` qua `connect()`). Không halt vì 1 node chết.
    pub async fn advance_and_reconnect(&self, logger: &BotLogger, transport_label: &str) -> Option<DynProvider> {
        if self.urls.is_empty() {
            return None;
        }
        {
            let mut st = self.state.write().await;
            st.idx = (st.idx + 1) % self.urls.len();
            st.provider = None;
        }
        self.connect(logger, transport_label).await
    }
}

/// Cụm 5.1 — dữ liệu thô rút ra từ MỘT tx (pending thật qua WSS hoặc dòng
/// `state/inject_tx.jsonl`) đủ để đưa vào `pipeline::decide_paper` (không
/// phụ thuộc kiểu `alloy::rpc::types::eth::Transaction` cụ thể ở tầng gọi
/// trên, tránh phải kéo trait `alloy::consensus::Transaction`/
/// `alloy::network::TransactionResponse` vào `main.rs`/`pipeline.rs`).
#[derive(Debug, Clone)]
pub struct PendingTxRaw {
    pub from: Address,
    /// Cụm `foundation-fix-then-real-sim` (A1) — `None` chỉ cho tx bơm qua
    /// `state/inject_tx.jsonl` (định dạng cũ `from,value_wei,input_hex` không
    /// có cột `to`) — tx pending THẬT (WS hoặc `txpool_content`) LUÔN có
    /// `Some` (mọi tx thường, kể cả contract-creation không xảy ra ở đường
    /// swap này). `main.rs` coi `None` là "nguồn không rõ router" — bỏ qua
    /// gate `not_pancake_router` (không đủ dữ liệu để từ chối), vẫn chạy tiếp
    /// decode như trước đây (giữ khả năng test bằng inject cũ không bị vỡ).
    pub to: Option<Address>,
    pub value: U256,
    pub input: Vec<u8>,
    /// Hash tx thật (rỗng/`B256::ZERO` cho tx inject — file cũ không có cột
    /// hash, không bịa giá trị).
    pub hash: B256,
    pub gas: u64,
    pub gas_price: U256,
    pub nonce: u64,
}

/// Cụm `5.2` — nguồn tx pending ĐANG hoạt động, dùng cho `/api/status`
/// (`pending_source`). Mặc định `InjectOnly` lúc boot (chưa chứng minh được
/// WSS/txpool nào) — `main.rs::subscribe_pending_txs`/`poll_txpool_pending`
/// cập nhật thành `Ws`/`Txpool` ngay khi subscribe/poll THÀNH CÔNG lần đầu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingSource {
    Ws,
    Txpool,
    InjectOnly,
}

impl PendingSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            PendingSource::Ws => "ws",
            PendingSource::Txpool => "txpool",
            PendingSource::InjectOnly => "inject_only",
        }
    }
}

/// Rút `from`/`value`/`input` từ 1 tx pending thật (`eth_subscribe
/// newPendingTransactions` với `Params::Bool(true)`, xem
/// `Provider::subscribe_full_pending_transactions`). Generic theo 2 trait
/// (`TransactionResponse` cho `from()`, `consensus::Transaction` cho
/// `value()`/`input()`) thay vì đặt tên kiểu `alloy::rpc::types::eth::Transaction`
/// cụ thể — tránh phải bật thêm feature/đường dẫn module chỉ để gọi hàm này.
///
/// Cụm A1: điền ĐỦ `to`/`hash`/`gas`/`gas_price`/`nonce` từ tx thật — dùng
/// CHUNG cho cả nhánh WS (`subscribe_full_pending_transactions`) lẫn
/// `txpool_content` (cả hai truyền `T = alloy::rpc::types::eth::Transaction`,
/// cùng implement 2 trait trên) nên không cần code riêng cho từng nhánh.
/// `gas_price` bị 2 trait khai TRÙNG tên method
/// (`consensus::Transaction::gas_price` VÀ `network::TransactionResponse::gas_price`,
/// 2 định nghĩa/default khác nhau) — gọi UFCS tường minh qua
/// `consensus::Transaction` để tránh ambiguous method call, fallback
/// `max_fee_per_gas()` khi tx là dynamic-fee (EIP-1559, `gas_price()` trả
/// `None` theo đúng semantics `alloy-consensus`).
pub fn pending_tx_from_rpc<T>(tx: &T) -> PendingTxRaw
where
    T: alloy::network::TransactionResponse + alloy::consensus::Transaction,
{
    let gas_price_u128 = <T as alloy::consensus::Transaction>::gas_price(tx)
        .unwrap_or_else(|| <T as alloy::consensus::Transaction>::max_fee_per_gas(tx));
    PendingTxRaw {
        from: tx.from(),
        to: tx.to(),
        value: tx.value(),
        input: tx.input().to_vec(),
        hash: tx.tx_hash(),
        gas: tx.gas_limit(),
        gas_price: U256::from(gas_price_u128),
        nonce: tx.nonce(),
    }
}

/// Parse 1 dòng `state/inject_tx.jsonl`: `from,value_wei,input_hex` (input_hex
/// có/không tiền tố `0x` đều được). Dùng để bơm tx giả lập test paper pipeline
/// khi node không đẩy pending thật (`rpc.pending_unavailable`) — KHÔNG bịa
/// hash tx (file này không có trường hash, hàm gọi sau không cần tới nó).
/// Lỗi định dạng trả `Err(String)`, không panic, đúng quy ước `victims.rs`.
pub fn parse_inject_line(line: &str) -> Result<PendingTxRaw, String> {
    let line = line.trim();
    let parts: Vec<&str> = line.splitn(3, ',').collect();
    if parts.len() != 3 {
        return Err("dinh dang phai la from,value_wei,input_hex".to_string());
    }
    let from = Address::from_str(parts[0].trim()).map_err(|e| format!("from khong hop le: {e}"))?;
    let value = U256::from_str(parts[1].trim()).map_err(|e| format!("value_wei khong hop le: {e}"))?;
    let hex_field = parts[2].trim();
    let hex_field = hex_field.strip_prefix("0x").or_else(|| hex_field.strip_prefix("0X")).unwrap_or(hex_field);
    let input = Bytes::from_str(hex_field).map_err(|e| format!("input_hex khong hop le: {e}"))?.to_vec();
    // Dinh dang file cu (3 cot, tu BAOCAO ban dau) khong co cot `to`/hash/gas -
    // KHONG bia gia tri, de `to=None` (xem doc-comment PendingTxRaw::to) va
    // B256::ZERO/0 cho phan con lai (chi dung de bom test paper, khong phai
    // du lieu tx that).
    Ok(PendingTxRaw { from, to: None, value, input, hash: B256::ZERO, gas: 0, gas_price: U256::ZERO, nonce: 0 })
}

// ============================================================================
// Cụm `exec-path-traps` (F-13) — nonce THẬT của victim
// ============================================================================

/// Kết quả so `nonce` của 1 candidate (lấy từ tx pending thật/`inject`) với
/// nonce KỲ VỌNG (`eth_getTransactionCount(from, "latest")` — xem lý do CHỌN
/// `"latest"` thay vì `"pending"` ở `fetch_expected_nonce`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonceCheck {
    /// `actual == expected` — đúng nonce kế tiếp, sẽ thực thi ở block kế (giả
    /// định không có gì khác chen vào), an toàn để front-run.
    Ok,
    /// `actual < expected` — nonce này ĐÃ được dùng bởi 1 tx khác đã lên
    /// block (tx đang xét là bản sao/đã bị thay thế, không bao giờ chạy).
    Stale,
    /// `actual > expected` — còn khoảng trống nonce (tx khác của CÙNG ví
    /// phải lên block trước), tx đang xét CHƯA thể chạy ngay.
    Future,
}

/// So sánh THUẦN, không RPC — tách riêng khỏi `fetch_expected_nonce`/`NonceCache`
/// để test được không cần `Provider`.
pub fn compare_nonce(actual: u64, expected: u64) -> NonceCheck {
    match actual.cmp(&expected) {
        std::cmp::Ordering::Less => NonceCheck::Stale,
        std::cmp::Ordering::Equal => NonceCheck::Ok,
        std::cmp::Ordering::Greater => NonceCheck::Future,
    }
}

/// Nonce "kỳ vọng" (nonce mà 1 tx MỚI của `from` cần có để thực thi NGAY ở
/// block kế tiếp) — dùng `eth_getTransactionCount(from, "latest")` (nonce đã
/// XÁC NHẬN trên chain), KHÔNG PHẢI tag `"pending"` dù lệnh gốc nêu
/// `eth_getTransactionCount(from, pending)`.
///
/// **Lý do lệch khỏi chữ literal của lệnh (ghi rõ, không âm thầm đổi)**: tag
/// `"pending"` của hầu hết node (Geth và tương thích) trả `latest_count +
/// số tx PENDING LIÊN TỤC (không đứt quãng) đã thấy của địa chỉ đó` — vì
/// CHÍNH tx đang được đánh giá ở đây LUÔN nằm trong mempool node vừa thấy nó
/// (đó là lý do nó tới được `handle_paper_tx`), tag `"pending"` trong trường
/// hợp BÌNH THƯỜNG (tx hợp lệ, không có gì bất thường) sẽ LUÔN trả
/// `victim.nonce + 1` — nghĩa là so `victim.nonce == pending_count` sẽ LUÔN
/// `false` (`Stale`) kể cả ở trường hợp khoẻ mạnh nhất, làm gate này chặn
/// MỌI candidate, không chỉ candidate thật sự có bẫy. `"latest"` (nonce đã
/// xác nhận on-chain — chính là nonce BẮT BUỘC cho tx MỚI tiếp theo của địa
/// chỉ đó nếu không có gì khác chen vào) cho ra đúng ngữ nghĩa "nonce này có
/// phải cái TIẾP THEO sẽ chạy hay không" mà lệnh mô tả (`nonce victim <
/// expected → nonce_stale`, `> expected → nonce_future`) — chỉ khác Ở CHỌN
/// TAG RPC nào để hỏi, KHÔNG đổi hướng so sánh/2 reason mới.
pub async fn fetch_expected_nonce(provider: &dyn Provider, from: Address) -> Result<u64, String> {
    provider.get_transaction_count(from).latest().await.map_err(|e| e.to_string())
}

/// Cache `(from, block) -> nonce kỳ vọng` để không gọi lặp lại
/// `eth_getTransactionCount` cho CÙNG 1 victim trong CÙNG 1 block (nhiều
/// candidate cùng ví hiếm nhưng có thể xảy ra trong 1 block). KHÔNG tự khoá
/// (khác `RpcPool`) — theo đúng quy ước repo (`AppStateInner` bọc `RwLock`
/// từ NGOÀI, xem `risk_guard`/`tax_cache`), caller tự `.read()/.write()`.
#[derive(Debug, Default)]
pub struct NonceCache {
    entries: std::collections::HashMap<(Address, u64), u64>,
}

impl NonceCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cached(&self, from: Address, block: u64) -> Option<u64> {
        self.entries.get(&(from, block)).copied()
    }

    pub fn insert(&mut self, from: Address, block: u64, nonce: u64) {
        self.entries.insert((from, block), nonce);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_source_as_str_matches_api_status_contract() {
        assert_eq!(PendingSource::Ws.as_str(), "ws");
        assert_eq!(PendingSource::Txpool.as_str(), "txpool");
        assert_eq!(PendingSource::InjectOnly.as_str(), "inject_only");
    }

    #[test]
    fn redact_hides_path_and_query() {
        let redacted = redact_rpc_url("https://provider.example.com/v2/SECRET_KEY?x=1");
        assert!(!redacted.contains("SECRET_KEY"));
        assert!(redacted.starts_with("https://provider.example.com"));
    }

    #[test]
    fn redact_handles_garbage_without_panic() {
        assert_eq!(redact_rpc_url("not a url"), "***invalid_url***");
    }

    #[test]
    fn redact_keeps_port() {
        let redacted = redact_rpc_url("wss://example.com:8546/ws/APIKEY");
        assert_eq!(redacted, "wss://example.com:8546/***");
    }

    #[test]
    fn pick_url_prefers_env_over_vps_fallback() {
        assert_eq!(
            pick_url(Some("https://real.example.com".to_string()), "REPLACE_ME_HTTP_RPC_URL"),
            Some("https://real.example.com".to_string())
        );
    }

    #[test]
    fn pick_url_falls_back_when_env_empty() {
        assert_eq!(
            pick_url(Some("   ".to_string()), "https://fallback.example.com"),
            Some("https://fallback.example.com".to_string())
        );
        assert_eq!(pick_url(None, "https://fallback.example.com"), Some("https://fallback.example.com".to_string()));
    }

    #[test]
    fn pick_url_none_when_both_empty() {
        assert_eq!(pick_url(None, ""), None);
        assert_eq!(pick_url(Some("".to_string()), "   "), None);
    }

    #[test]
    fn vps_fallback_missing_file_is_empty_not_panic() {
        let fb = VpsFallback::load(std::path::Path::new("khong_ton_tai.json"));
        assert!(fb.rpc_http.is_empty());
        assert!(fb.rpc_ws.is_empty());
    }

    /// Placeholder trong `vps.json` phải fail connect nhanh, không panic —
    /// khớp "boot vẫn chạy, log rpc.skip" khi chưa có `.env` thật.
    #[tokio::test]
    async fn placeholder_url_fails_fast_no_panic() {
        let err = connect_and_verify("REPLACE_ME_HTTP_RPC_URL").await;
        assert!(err.is_err());
    }

    #[test]
    fn parse_inject_line_valid_with_0x_prefix() {
        let raw = parse_inject_line(
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,50000000000000000,0x7ff36ab5deadbeef",
        )
        .expect("dong hop le phai parse duoc");
        assert_eq!(format!("{:#x}", raw.from), "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(raw.value, U256::from(50_000_000_000_000_000u64));
        assert_eq!(raw.input, vec![0x7f, 0xf3, 0x6a, 0xb5, 0xde, 0xad, 0xbe, 0xef]);
    }

    /// input_hex KHÔNG có tiền tố `0x` vẫn phải parse được (lệnh không bắt
    /// buộc tiền tố, chỉ nói "input_hex").
    #[test]
    fn parse_inject_line_valid_without_0x_prefix() {
        let raw =
            parse_inject_line("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,1000000000000000000,deadbeef")
                .expect("dong khong tien to 0x van phai parse duoc");
        assert_eq!(raw.value, U256::from(1_000_000_000_000_000_000u64));
        assert_eq!(raw.input, vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn parse_inject_line_rejects_wrong_field_count() {
        let err = parse_inject_line("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,123").unwrap_err();
        assert!(err.contains("from,value_wei,input_hex"));
    }

    #[test]
    fn parse_inject_line_rejects_bad_address() {
        let err = parse_inject_line("not_an_address,123,0xdead").unwrap_err();
        assert!(err.contains("from khong hop le"));
    }

    #[test]
    fn parse_inject_line_rejects_bad_value() {
        let err =
            parse_inject_line("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,not_a_number,0xdead").unwrap_err();
        assert!(err.contains("value_wei khong hop le"));
    }

    #[test]
    fn parse_inject_line_rejects_bad_hex() {
        let err =
            parse_inject_line("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,123,0xzz_not_hex").unwrap_err();
        assert!(err.contains("input_hex khong hop le"));
    }

    #[test]
    fn parse_inject_line_ignores_leading_trailing_whitespace() {
        let raw = parse_inject_line("  0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa , 1 , 0xbeef  ")
            .expect("khoang trang quanh field phai duoc trim");
        assert_eq!(raw.value, U256::from(1u64));
        assert_eq!(raw.input, vec![0xbe, 0xef]);
    }

    // ===== Cụm `5.3` — parse_rpc_url_list / is_private_send_url / filter_read_urls =====

    fn env_map(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn parse_rpc_url_list_prefers_list_over_numbered() {
        let env = env_map(&[
            ("BSC_HTTP_LIST", "https://a.example.com, https://b.example.com ,https://c.example.com"),
            ("BSC_HTTP", "https://should-be-ignored.example.com"),
            ("BSC_HTTP_2", "https://also-ignored.example.com"),
        ]);
        let urls = parse_rpc_url_list(|k| env.get(k).cloned(), "BSC_HTTP");
        assert_eq!(
            urls,
            vec!["https://a.example.com", "https://b.example.com", "https://c.example.com"]
        );
    }

    #[test]
    fn parse_rpc_url_list_falls_back_to_base_plus_numbered_when_list_absent() {
        let env = env_map(&[
            ("BSC_HTTP", "https://one.example.com"),
            ("BSC_HTTP_2", "https://two.example.com"),
            ("BSC_HTTP_3", "  "), // rong sau trim -> bo qua
            ("BSC_HTTP_4", "https://four.example.com"),
        ]);
        let urls = parse_rpc_url_list(|k| env.get(k).cloned(), "BSC_HTTP");
        assert_eq!(
            urls,
            vec!["https://one.example.com", "https://two.example.com", "https://four.example.com"]
        );
    }

    #[test]
    fn parse_rpc_url_list_empty_when_nothing_set() {
        let env: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        assert!(parse_rpc_url_list(|k| env.get(k).cloned(), "BSC_HTTP").is_empty());
    }

    /// Phát hiện THẬT khi verify runtime phiên này: `.env` chủ đã điền `BSC_HTTP=`
    /// là 1 chuỗi NHIỀU URL nối bằng dấu phẩy (34 URL thật), không dùng
    /// `BSC_HTTP_LIST` — field `<base>` gốc PHẢI tự tách được theo dấu phẩy,
    /// không chỉ `<base>_LIST`, nếu không cả 33 URL sau bị bỏ sót hoàn toàn.
    #[test]
    fn parse_rpc_url_list_splits_commas_inside_base_var_itself() {
        let env = env_map(&[("BSC_HTTP", "https://a.example.com,https://b.example.com, https://c.example.com ")]);
        let urls = parse_rpc_url_list(|k| env.get(k).cloned(), "BSC_HTTP");
        assert_eq!(
            urls,
            vec!["https://a.example.com", "https://b.example.com", "https://c.example.com"]
        );
    }

    #[test]
    fn parse_rpc_url_list_ignores_empty_list_value_falls_back_to_numbered() {
        // BSC_HTTP_LIST co nhung toan chuoi rong sau split/trim -> phai fallback
        // xuong BSC_HTTP/BSC_HTTP_N, khong tra ve danh sach rong sai.
        let env = env_map(&[("BSC_HTTP_LIST", " , , "), ("BSC_HTTP", "https://fallback.example.com")]);
        let urls = parse_rpc_url_list(|k| env.get(k).cloned(), "BSC_HTTP");
        assert_eq!(urls, vec!["https://fallback.example.com"]);
    }

    #[test]
    fn is_private_send_url_detects_all_3_keywords_case_insensitive() {
        assert!(is_private_send_url("https://bsc.MaxBackRun.io/rpc"));
        assert!(is_private_send_url("https://bsc-fullprivacy.example.com"));
        assert!(is_private_send_url("https://PRIVACY-relay.example.com"));
        assert!(!is_private_send_url("https://bsc-dataseed1.bnbchain.org"));
    }

    #[test]
    fn filter_read_urls_drops_private_send_keeps_normal_dataseed() {
        let urls = vec![
            "https://bsc-dataseed1.bnbchain.org".to_string(),
            "https://relay.maxbackrun.io/submit".to_string(),
            "https://rpc.fullprivacy.example.com".to_string(),
            "https://bsc-rpc.publicnode.com".to_string(),
        ];
        let filtered = filter_read_urls(urls);
        assert_eq!(
            filtered,
            vec!["https://bsc-dataseed1.bnbchain.org".to_string(), "https://bsc-rpc.publicnode.com".to_string()]
        );
    }

    // ===== Cụm `5.3` — RpcPool (mock JSON-RPC server nội bộ, không cần mạng thật) =====

    /// Server JSON-RPC giả tối giản (chạy trên `127.0.0.1:0`, port random) —
    /// trả CỐ ĐỊNH `chain_id_hex` cho MỌI request (đủ cho `connect_and_verify`
    /// chỉ gọi `eth_chainId`). Cho phép test `ChainMismatch`/thành công mà
    /// KHÔNG cần mạng Internet thật (khác `#[ignore]` real_rpc_* ở sim_v3.rs).
    async fn mock_rpc_server(chain_id_hex: &'static str) -> String {
        use axum::extract::{Json as ReqJson, State as AxState};
        use axum::response::Json as RespJson;
        use axum::routing::post;

        async fn handle(
            AxState(chain_id_hex): AxState<&'static str>,
            ReqJson(body): ReqJson<serde_json::Value>,
        ) -> RespJson<serde_json::Value> {
            let id = body.get("id").cloned().unwrap_or(serde_json::json!(1));
            RespJson(serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": chain_id_hex }))
        }

        let app = axum::Router::new().route("/", post(handle)).with_state(chain_id_hex);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind mock rpc server");
        let addr = listener.local_addr().expect("local_addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{addr}/")
    }

    fn test_logger() -> (tempfile::TempDir, BotLogger) {
        let dir = tempfile::tempdir().unwrap();
        let logger = BotLogger::new(dir.path().join("logs").join("bot.jsonl")).unwrap();
        (dir, logger)
    }

    /// ĐẠT CẦN DÁN: "test failover URL đầu chết → URL sau". URL đầu là cổng
    /// TCP không ai lắng nghe (`127.0.0.1:1`, refused NGAY, không cần chờ
    /// timeout DNS) — mô phỏng "1 node chết". URL sau là mock server hợp lệ
    /// (chain 56 đúng) -> `connect()` phải bỏ qua URL 1 (log `rpc.failover`)
    /// rồi chọn URL 2 (log `rpc.connect`), KHÔNG panic.
    #[tokio::test]
    async fn rpc_pool_failover_when_first_url_dead_picks_next() {
        let (_dir, logger) = test_logger();
        let dead_url = "http://127.0.0.1:1/".to_string();
        let good_url = mock_rpc_server("0x38").await; // chain 56
        let pool = RpcPool::new(vec![dead_url, good_url.clone()]);

        let provider = pool.connect(&logger, "http").await;
        assert!(provider.is_some(), "URL 2 hop le phai duoc chon du URL 1 chet");

        let tail = logger.tail(10);
        assert!(tail.iter().any(|l| l["event"] == "rpc.failover" && l["pool_index"] == 0), "phai log failover cho URL 1");
        assert!(
            tail.iter().any(|l| l["event"] == "rpc.connect" && l["pool_index"] == 1),
            "phai log connect thanh cong cho URL 2"
        );
        assert_eq!(pool.current().await.is_some(), true);
    }

    /// Sai chain (`0x1` thay vì `0x38`) phải bị SKIP đúng như "1 node chết"
    /// (khác lỗi transport nhưng cùng hành động: thử URL kế) — không cần
    /// mạng Internet thật, dùng 2 mock server nội bộ trả chain khác nhau.
    #[tokio::test]
    async fn rpc_pool_skips_wrong_chain_url_then_picks_correct_one() {
        let (_dir, logger) = test_logger();
        let wrong_chain_url = mock_rpc_server("0x1").await; // chain 1, sai
        let right_chain_url = mock_rpc_server("0x38").await; // chain 56, dung
        let pool = RpcPool::new(vec![wrong_chain_url, right_chain_url]);

        let provider = pool.connect(&logger, "http").await;
        assert!(provider.is_some(), "URL sai chain phai bi bo qua, URL dung chain duoc chon");

        let tail = logger.tail(10);
        assert!(
            tail.iter().any(|l| l["event"] == "rpc.failover" && l["reason"].as_str().unwrap_or("").contains("sai chain")),
            "URL sai chain phai log rpc.failover voi ly do sai chain, tail={tail:?}"
        );
    }

    /// Redact PHẢI được áp dụng trong log `rpc.failover` — token trong query
    /// KHÔNG được lộ, kể cả khi URL đó chết (không connect được).
    #[tokio::test]
    async fn rpc_pool_failover_log_redacts_token_in_query() {
        let (_dir, logger) = test_logger();
        let dead_url_with_token = "http://127.0.0.1:1/v2/SUPER_SECRET_TOKEN?x=1".to_string();
        let pool = RpcPool::new(vec![dead_url_with_token]);

        let provider = pool.connect(&logger, "http").await;
        assert!(provider.is_none(), "URL chet duy nhat trong pool -> None, khong panic");

        let tail = logger.tail(10);
        let failover = tail.iter().find(|l| l["event"] == "rpc.failover").expect("phai co 1 dong rpc.failover");
        let logged_url = failover["url"].as_str().unwrap_or("");
        assert!(!logged_url.contains("SUPER_SECRET_TOKEN"), "khong duoc lo token trong log, logged_url={logged_url}");
        assert!(logged_url.starts_with("http://127.0.0.1:1"));
    }

    /// Hết cả danh sách (mọi URL đều chết) -> `None`, KHÔNG panic, KHÔNG halt
    /// (caller tự quyết định log rpc.skip, xem main.rs::connect_rpc).
    #[tokio::test]
    async fn rpc_pool_all_urls_dead_returns_none_no_panic() {
        let (_dir, logger) = test_logger();
        let pool = RpcPool::new(vec!["http://127.0.0.1:1/".to_string(), "http://127.0.0.1:2/".to_string()]);
        let provider = pool.connect(&logger, "http").await;
        assert!(provider.is_none());
        let tail = logger.tail(10);
        let failover_count = tail.iter().filter(|l| l["event"] == "rpc.failover").count();
        assert_eq!(failover_count, 2, "ca 2 URL deu phai duoc thu va log failover");
    }

    #[tokio::test]
    async fn rpc_pool_empty_list_returns_none_no_panic() {
        let (_dir, logger) = test_logger();
        let pool = RpcPool::new(vec![]);
        assert!(pool.is_empty());
        assert!(pool.connect(&logger, "http").await.is_none());
        assert!(pool.current().await.is_none());
    }

    /// `advance_and_reconnect` phải QUAY VÒNG đúng nghĩa: từ idx cuối cùng
    /// (2 URL, đang ở idx 1) quay lại idx 0 chứ không panic index-out-of-range.
    #[tokio::test]
    async fn rpc_pool_advance_and_reconnect_wraps_around() {
        let (_dir, logger) = test_logger();
        let url_a = mock_rpc_server("0x38").await;
        let url_b = mock_rpc_server("0x38").await;
        let pool = RpcPool::new(vec![url_a, url_b]);

        pool.connect(&logger, "http").await.expect("connect dau tien phai OK (idx 0)");
        let after_first_advance = pool.advance_and_reconnect(&logger, "http").await;
        assert!(after_first_advance.is_some(), "idx 1 van la mock hop le");
        let after_second_advance = pool.advance_and_reconnect(&logger, "http").await;
        assert!(after_second_advance.is_some(), "quay vong ve idx 0, khong panic index");
    }

    // ===== Cụm `exec-path-traps` (F-13) — compare_nonce / NonceCache =====

    #[test]
    fn compare_nonce_equal_is_ok() {
        assert_eq!(compare_nonce(5, 5), NonceCheck::Ok);
    }

    #[test]
    fn compare_nonce_lower_is_stale() {
        assert_eq!(compare_nonce(4, 5), NonceCheck::Stale);
    }

    #[test]
    fn compare_nonce_higher_is_future() {
        assert_eq!(compare_nonce(6, 5), NonceCheck::Future);
    }

    #[test]
    fn nonce_cache_miss_then_hit_after_insert() {
        let mut cache = NonceCache::new();
        let from = Address::from_str("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        assert_eq!(cache.cached(from, 1000), None);
        cache.insert(from, 1000, 42);
        assert_eq!(cache.cached(from, 1000), Some(42));
        // Block khac -> van la cache miss (khoa theo (from, block), khong
        // phai chi theo from).
        assert_eq!(cache.cached(from, 1001), None);
    }

    /// `#[ignore]` — chỉ chạy thủ công khi có RPC sống (cùng khuôn
    /// `pool.rs::real_rpc_v2_get_pair_wbnb_usdt`, dùng RPC công khai, KHÔNG
    /// phải `.env` runtime của bot). Chứng minh `fetch_expected_nonce` gọi
    /// `eth_getTransactionCount(address, "latest")` THẬT thành công trên
    /// mainnet chain 56 — dùng địa chỉ WBNB (contract token thường KHÔNG tự
    /// gửi tx nào, nonce kỳ vọng thấp/ổn định, đủ để chứng minh đường RPC
    /// hoạt động mà không phụ thuộc trạng thái mempool biến động).
    #[tokio::test]
    #[ignore]
    async fn real_rpc_fetch_expected_nonce_for_wbnb_contract() {
        use alloy::providers::{Provider, ProviderBuilder};

        let wbnb = Address::from_str(crate::venues::WBNB_ADDRESS).unwrap();
        let provider =
            ProviderBuilder::new().connect("https://bsc-dataseed.binance.org/").await.expect("ket noi RPC cong khai that bai");
        let chain_id = provider.get_chain_id().await.expect("eth_chainId that bai");
        assert_eq!(chain_id, 56);

        let nonce = fetch_expected_nonce(&provider, wbnb).await.expect("eth_getTransactionCount that bai");
        println!("eth_getTransactionCount(WBNB, latest) THAT = {nonce}");
        // Contract token thuong khong tu gui tx nao - nonce thuc te == 0,
        // nhung khong assert cung 0 (khong bia bat bien on-chain vinh vien
        // chua tung tu verify se khong bao gio doi) - chi assert goi RPC
        // thanh cong va tra ve so hop le.
    }
}

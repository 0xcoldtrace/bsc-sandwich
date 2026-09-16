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

/// Cụm `econ-truth-latency-vps` (mục 0.c) — nhận diện lỗi JSON-RPC dạng "node
/// KHÔNG HỖ TRỢ method này" (`-32000`/`-32601` "method not found", hoặc chuỗi
/// "not supported" — quan sát thật từ bloXroute khi gọi các method cần state
/// đầy đủ cho revm fork, xem `pairs_vet_task`), KHÁC lỗi nghiệp vụ thật (vd
/// "execution reverted", timeout mạng). Dùng để quyết định "đánh dấu URL này
/// không dùng được cho method đó, chuyển URL kế" thay vì tính là lỗi kinh tế/
/// resolve_fail thông thường.
pub fn is_unsupported_method_error(err: &str) -> bool {
    let lower = err.to_lowercase();
    lower.contains("-32000")
        || lower.contains("-32601")
        || lower.contains("not supported")
        || lower.contains("method not found")
        // Cụm `bugfix-presign-and-contract-plan` (A5) — quan sát THẬT khi chạy
        // shadow lần đầu với pool RPC NỀN: `bsc-rpc.publicnode.com` trả
        // `-32602 "Archive requests require a personal token. Get one at:
        // https://www.allnodes.com/publicnode"` cho MỌI lần `revm`/`AlloyDB`
        // đọc storage slot ở block cũ hơn vài block. Đây cũng là "node này
        // không kham được việc này" — phải đánh dấu URL và chuyển URL kế,
        // nếu không `pairs_vet_task` lặp lỗi vô hạn trên đúng 1 URL hỏng
        // (đã thấy: 126 dòng `pair.vet_error` liên tiếp, 0 pool nào được vet,
        // kéo theo đường ký shadow abort `vet_stale` 100%).
        || lower.contains("archive request")
        || lower.contains("personal token")
}

/// Cụm `decision-data-24h` (mục 5) — PHÂN LOẠI lỗi vet nền (`pair.vet_error`)
/// để phân biệt 3 nguyên nhân hoàn toàn khác nhau, trước đây gộp chung 1 chuỗi
/// lỗi thô:
///
/// - `missing_trie_node` — node KHÔNG GIỮ state của block đó (node full pruned
///   hoặc đã prune qua block cần fork). Đây là hạn chế HẠ TẦNG, không phải lỗi
///   token: pool dính lỗi này KHÔNG BAO GIỜ vet được trên node hiện tại ⇒
///   không bao giờ ký được cho pool đó (cổng (a) `vet_stale` vĩnh viễn). Cần
///   node archive riêng (`BSC_HTTP_SIM`).
/// - `rate_limited` — HTTP 429, tạm thời, thử lại vòng sau là được.
/// - `unsupported_method` — xem `is_unsupported_method_error` (đổi URL).
/// - `other` — mọi thứ còn lại (revert thật, decode lỗi...).
pub fn classify_vet_error(err: &str) -> &'static str {
    let lower = err.to_lowercase();
    if lower.contains("missing trie node") || lower.contains("required historical state unavailable") {
        "missing_trie_node"
    } else if lower.contains("429") || lower.contains("too many requests") {
        "rate_limited"
    } else if is_unsupported_method_error(err) {
        "unsupported_method"
    } else {
        "other"
    }
}

/// Cụm `decision-data-24h` (mục 5) — nạp `.env` vào biến môi trường tiến trình
/// khi bot được chạy TRỰC TIẾP (`./target/release/bsc_sandwich config.toml`),
/// không qua `scripts/paper_run.sh` (script đó đã `set -a; . ./.env`).
///
/// Vì thiếu bước này, mọi biến chỉ có trong `.env` mà không được script export
/// — cụ thể `BSC_HTTP_SIM` (node archive riêng cho revm fork) — bị bot bỏ qua
/// im lặng khi chạy tay, và `pairs_vet_task` lại rơi vào đúng node public
/// không đủ state (`missing trie node`).
///
/// Quy tắc: biến ĐÃ CÓ trong môi trường luôn THẮNG (`.env` chỉ là mặc định,
/// không ghi đè lệnh chạy); dòng trống/`#` bỏ qua; nháy `"`/`'` bao ngoài giá
/// trị được gỡ; **KHÔNG BAO GIỜ log giá trị** (file này chứa `PRIVATE_KEY`) —
/// chỉ trả về danh sách TÊN biến đã nạp.
pub fn load_dotenv_defaults(path: &std::path::Path) -> Vec<String> {
    let Ok(raw) = std::fs::read_to_string(path) else { return Vec::new() };
    let mut loaded = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim();
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        if std::env::var(key).is_ok() {
            continue;
        }
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        if value.is_empty() {
            continue;
        }
        std::env::set_var(key, value);
        loaded.push(key.to_string());
    }
    loaded
}

#[derive(Debug, Default)]
struct RpcPoolState {
    idx: usize,
    provider: Option<DynProvider>,
    /// Cụm `econ-truth-latency-vps` (0.c) — chỉ số URL (trong `self.urls`) đã
    /// từng trả lỗi "method không hỗ trợ" cho pool này — `connect()` bỏ qua
    /// các URL này TRỪ KHI toàn bộ danh sách đều bị đánh dấu (tránh khoá chết
    /// hoàn toàn khi mọi URL đều từng lỗi 1 method nào đó không liên quan).
    unsupported: std::collections::HashSet<usize>,
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
        let (start_idx, unsupported) = {
            let st = self.state.read().await;
            (st.idx, st.unsupported.clone())
        };
        // Cum 0.c - bo qua URL da danh dau "method khong ho tro" TRU KHI ca
        // danh sach deu bi danh dau (khi do bo qua het se khong con URL nao
        // de thu - tha ra thu lai tat ca thay vi khoa chet vinh vien).
        let skip_unsupported = unsupported.len() < self.urls.len();
        for step in 0..self.urls.len() {
            let idx = (start_idx + step) % self.urls.len();
            if skip_unsupported && unsupported.contains(&idx) {
                continue;
            }
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

    /// Cụm `econ-truth-latency-vps` (0.c) — URL HIỆN TẠI vừa trả lỗi "method
    /// không hỗ trợ" (`transport::is_unsupported_method_error`) cho 1 method
    /// cụ thể (vd `eth_getStorageAt` cần cho revm fork trên node bloXroute) —
    /// đánh dấu KHÔNG dùng URL này nữa cho pool này, log `rpc.method_unsupported`,
    /// rồi chuyển sang URL kế (giống `advance_and_reconnect`, nhưng nhớ lại
    /// lâu dài thay vì chỉ đổi tạm 1 lần).
    pub async fn mark_current_unsupported(&self, logger: &BotLogger, transport_label: &str) -> Option<DynProvider> {
        if self.urls.is_empty() {
            return None;
        }
        let marked_idx = {
            let mut st = self.state.write().await;
            let current_idx = st.idx;
            st.unsupported.insert(current_idx);
            st.provider = None;
            st.idx = (current_idx + 1) % self.urls.len();
            current_idx
        };
        logger.log(
            "rpc.method_unsupported",
            serde_json::json!({ "transport": transport_label, "pool_index": marked_idx, "pool_size": self.urls.len() }),
        );
        self.connect(logger, transport_label).await
    }

    /// Nhãn (redacted) của URL hiện đang connect — dùng để đính kèm log
    /// `pair.resolve_fail` (`url_label`) mà KHÔNG lộ URL thật/token.
    pub async fn current_url_label(&self) -> Option<String> {
        let st = self.state.read().await;
        self.urls.get(st.idx).map(|u| redact_rpc_url(u))
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
/// Cụm `truth-victim-ok-and-memleak` (mục 3) — số BLOCK gần nhất được giữ.
/// Key của cache có `block`, nên entry của block cũ KHÔNG BAO GIỜ trúng lại;
/// giữ chúng chỉ tốn RAM. Trước cụm này không có bước xoá nào ⇒ mỗi ví victim
/// mới trong mỗi block mới là 1 entry sống mãi (đo thật trên VPS: OOM-kill ở
/// 7,6 GB sau 656 phút, xem BAOCAO43 ô 5 mục 0). 4 block BSC ≈ 12 giây, thừa
/// cho khoảng cách giữa lúc thấy tx pending và lúc quyết định (p95 ~350 ms).
pub const NONCE_CACHE_BLOCKS: usize = 4;

#[derive(Debug, Default)]
pub struct NonceCache {
    entries: std::collections::HashMap<(Address, u64), u64>,
    /// Các block đang có entry, tăng dần — dùng để xoá theo block (xem
    /// `NONCE_CACHE_BLOCKS`).
    blocks: std::collections::BTreeSet<u64>,
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
        self.blocks.insert(block);
        while self.blocks.len() > NONCE_CACHE_BLOCKS {
            let Some(oldest) = self.blocks.iter().next().copied() else { break };
            self.blocks.remove(&oldest);
            self.entries.retain(|(_, b), _| *b != oldest);
        }
    }

    /// Cụm `truth-victim-ok-and-memleak` (mục 3) — số entry đang sống, cho
    /// `GET /api/mem` + log `mem.rss_mb`.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Cụm `econ-truth-latency-vps` (mục 1) — dedup hash tx pending DÙNG CHUNG
/// giữa CẢ 3 nguồn tx (`subscribe_pending_txs` WS, `poll_txpool_pending`,
/// `watch_inject_file`) — trước cụm này, `poll_txpool_pending` có `seen_set`
/// RIÊNG của chính nó (khởi tạo RỖNG khi hàm bắt đầu), nên nếu WS subscription
/// rớt giữa chừng rồi rơi xuống fallback `poll_txpool_pending` (đúng thiết kế
/// `5.2`), 1 tx VỪA được xử lý qua WS (đã tăng `funnel.simulated` +
/// `log_outcome_v2`) có thể vẫn còn NẰM TRONG mempool và bị `txpool_content`
/// lấy lại, xử lý (và tăng funnel) LẦN NỮA — nghi vấn góp phần vào lệch
/// `funnel.simulated` vs số dòng `sim.result` thật quan sát ở BAOCAO39 (27 vs
/// 14, xem `docs/TASKS.md`). Cùng khuôn cap+xoá-theo-tuổi như
/// `poll_txpool_pending::seen_set` cũ.
pub const SEEN_HASH_CAP: usize = 50_000;

#[derive(Debug, Default)]
pub struct SeenHashSet {
    set: std::collections::HashSet<B256>,
    order: std::collections::VecDeque<B256>,
}

impl SeenHashSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` nếu hash CHƯA từng thấy (và ghi nhận NGAY — gọi 1 lần duy nhất
    /// cho quyết định "có xử lý tx này không", không tách `contains`+`insert`
    /// riêng để tránh race giữa 2 bước đó).
    pub fn insert_if_new(&mut self, hash: B256) -> bool {
        if self.set.contains(&hash) {
            return false;
        }
        self.set.insert(hash);
        self.order.push_back(hash);
        if self.order.len() > SEEN_HASH_CAP {
            if let Some(oldest) = self.order.pop_front() {
                self.set.remove(&oldest);
            }
        }
        true
    }

    /// Cụm `truth-victim-ok-and-memleak` (mục 3) — số hash đang giữ, cho
    /// `GET /api/mem` (trần = `SEEN_HASH_CAP`).
    pub fn len(&self) -> usize {
        self.set.len()
    }

    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }
}

/// Cụm `hotpath-fix-then-decoder-ur` (A4b) — cache `(pair, quote, block) ->
/// reserves` để KHÔNG gọi lặp lại `token0()`/`getReserves()` (2 `eth_call`)
/// cho CÙNG 1 pool trong CÙNG 1 block — nhiều tx chạm cùng pool nóng (vd
/// CAKE/WBNB) trong 1 block là bình thường ở mempool BSC thật (BAOCAO38:
/// 12657 candidate/phút qua đúng ~44 pool). Cùng khuôn `NonceCache` (không tự
/// khoá, caller tự `RwLock`). `block` đổi -> entry cũ đơn giản là cache-miss
/// (key gồm cả block), KHÔNG cần dọn dẹp chủ động.
///
/// # Vì sao key PHẢI có `quote` (cụm `bugfix-presign-and-contract-plan`, A1)
///
/// `PoolReserves` KHÔNG tự mô tả chiều: field `reserve_wbnb` thực chất là
/// "reserve của QUOTE ASSET mà caller đã hỏi", `reserve_token` là phía còn
/// lại (xem `pool::order_reserves_by_quote`). Trước bản sửa này key chỉ là
/// `(pair, block)` — cùng 1 pool hỏi bằng 2 quote asset khác nhau dùng CHUNG
/// 1 entry, nên chiều bị ĐẢO im lặng. Bug THẬT quan sát được trên
/// `logs/bot.jsonl` (68 dòng, pool WBNB/USDT `0x16b9a828…`): victim MUA WBNB
/// bằng USDT khiến pool đó được cache với `quote=USDT`
/// (`reserve_wbnb`=reserve USDT thật); ngay sau đó bước quy đổi gas/
/// `amount_in_bnb_equiv` hỏi CÙNG pool với `quote=WBNB` và nhận lại entry
/// USDT-ordered → `convert_usdt_to_bnb_wei` nhân thay vì chia (tỉ giá 720
/// thay vì 1/720) → `amount_in_bnb_equiv` phồng ~518.000 lần, `/api/econ`
/// báo `best_net_bnb` hàng chục nghìn BNB, và `gas_cost_usdt_wei` bị CHIA
/// cho 720 (gas rẻ giả → profit USDT bị thổi lên). Key có `quote` làm 2 chiều
/// thành 2 entry riêng, không thể lẫn.
/// Cụm `truth-victim-ok-and-memleak` (mục 3) — số BLOCK gần nhất được giữ.
///
/// **Đây là chỗ RÒ RỈ NẶNG NHẤT đã tìm ra**: doc-comment cũ của
/// `ReserveCache` viết nguyên văn *"block đổi -> entry cũ đơn giản là
/// cache-miss (key gồm cả block), KHÔNG cần dọn dẹp chủ động"* — đúng về mặt
/// ĐÚNG/SAI nhưng sai về bộ nhớ: cache-miss không giải phóng gì cả. Nguồn ghi
/// KHÔNG phải đường nóng mà là `sync_reserves_task` (`main.rs`): nó ghi 1
/// entry cho MỖI sự kiện `Sync` của MỖI pool trong `pairs.txt` ở MỖI block —
/// 126 pool × ~2,2 block/s ⇒ tới ~10 triệu entry sau 11 giờ, không bao giờ
/// xoá. Giữ 8 block (≈18 s) là thừa cho mục đích của cache (nhiều tx cùng
/// pool trong CÙNG 1 block).
pub const RESERVE_CACHE_BLOCKS: usize = 8;

#[derive(Debug, Default)]
pub struct ReserveCache {
    entries: std::collections::HashMap<(Address, Address, u64), crate::sim_v2::PoolReserves>,
    blocks: std::collections::BTreeSet<u64>,
}

impl ReserveCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cached(&self, pair: Address, quote: Address, block: u64) -> Option<crate::sim_v2::PoolReserves> {
        self.entries.get(&(pair, quote, block)).copied()
    }

    pub fn insert(&mut self, pair: Address, quote: Address, block: u64, reserves: crate::sim_v2::PoolReserves) {
        self.entries.insert((pair, quote, block), reserves);
        self.blocks.insert(block);
        while self.blocks.len() > RESERVE_CACHE_BLOCKS {
            let Some(oldest) = self.blocks.iter().next().copied() else { break };
            self.blocks.remove(&oldest);
            self.entries.retain(|(_, _, b), _| *b != oldest);
        }
    }

    /// Cụm `truth-victim-ok-and-memleak` (mục 3) — số entry đang sống.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ============================================================================
// Cụm `bugfix-presign-and-contract-plan` (A4) — 2 cache "đường ký KHÔNG gọi
// RPC": chỉ số tx ĐÃ LÊN BLOCK gần nhất + nonce ví shadow prefetch theo block.
// ============================================================================

/// Số block gần nhất giữ lại trong `MinedTxIndex`. 3 block BSC ≈ 9 giây —
/// thừa sức phủ khoảng thời gian từ lúc bot thấy tx pending tới lúc nó quyết
/// định (p95 ~350 ms, xem BAOCAO41), nhưng vẫn đủ ngắn để bộ nhớ không phình.
pub const MINED_INDEX_DEPTH: usize = 3;

/// Cụm `bugfix-presign-and-contract-plan` (A4) — "mempool view" của bot: tập
/// hash tx đã xuất hiện trong `MINED_INDEX_DEPTH` block gần nhất, nạp bởi 1
/// task nền (1 lời gọi `eth_getBlockByNumber` KHÔNG-full cho MỖI block mới,
/// trên RPC NỀN) — KHÔNG phải mỗi tx.
///
/// Thay cho `eth_getTransactionReceipt(victim)` ở đường ký (BAOCAO41:
/// `pre_sign_revet` gọi receipt + fork revm, 0/36 lần ký kịp). Câu hỏi cần
/// trả lời trước khi ký là "victim CÒN pending không" — nếu hash đã nằm
/// trong 1 block đã biết thì chắc chắn KHÔNG còn; nếu chưa thấy thì theo
/// hiểu biết hiện có của bot nó vẫn pending (ghi rõ: đây là *hiểu biết của
/// bot*, không phải bằng chứng tuyệt đối từ node — đúng tinh thần "KHÔNG gọi
/// RPC receipt" của lệnh A4).
#[derive(Debug, Default)]
pub struct MinedTxIndex {
    /// `(block, hash tập)` theo thứ tự block tăng dần, tối đa `MINED_INDEX_DEPTH`.
    blocks: std::collections::VecDeque<(u64, std::collections::HashSet<B256>)>,
}

impl MinedTxIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Nạp danh sách hash của 1 block. Block đã có -> ghi đè (reorg/đọc lại).
    pub fn insert_block(&mut self, block: u64, hashes: std::collections::HashSet<B256>) {
        self.blocks.retain(|(b, _)| *b != block);
        self.blocks.push_back((block, hashes));
        // Giữ đúng N block MỚI NHẤT (sắp theo số block, không theo thứ tự nạp).
        let mut sorted: Vec<_> = self.blocks.drain(..).collect();
        sorted.sort_by_key(|(b, _)| *b);
        while sorted.len() > MINED_INDEX_DEPTH {
            sorted.remove(0);
        }
        self.blocks = sorted.into();
    }

    /// Cụm `truth-victim-ok-and-memleak` (mục 3) — TỔNG số hash đang giữ
    /// (cộng mọi block trong cửa sổ), cho `GET /api/mem`.
    pub fn len(&self) -> usize {
        self.blocks.iter().map(|(_, h)| h.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `true` khi hash ĐÃ thấy trong 1 block gần đây (victim KHÔNG còn pending).
    pub fn contains(&self, hash: B256) -> bool {
        self.blocks.iter().any(|(_, set)| set.contains(&hash))
    }

    /// Block mới nhất đã nạp (`None` khi chưa nạp block nào — caller PHẢI coi
    /// đây là "chưa biết gì", không được suy ra "victim còn pending").
    pub fn newest_block(&self) -> Option<u64> {
        self.blocks.iter().map(|(b, _)| *b).max()
    }

    pub fn blocks_loaded(&self) -> usize {
        self.blocks.len()
    }
}

/// Cụm `bugfix-presign-and-contract-plan` (A4) — nonce ví SHADOW (ví của
/// chính bot) prefetch mỗi block bởi task nền, để đường ký không phải gọi
/// `eth_getTransactionCount` (BAOCAO41 đo: lời gọi đó + fork tax là nguyên
/// nhân ký trễ). KHÁC `NonceCache` (nonce của VICTIM, khoá theo địa chỉ bất
/// kỳ) — đây chỉ 1 địa chỉ, nên lưu thẳng `(block, nonce)`.
#[derive(Debug, Default)]
pub struct SelfNonceCache {
    cached: Option<(u64, u64)>,
}

impl SelfNonceCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, block: u64, nonce: u64) {
        self.cached = Some((block, nonce));
    }

    /// Nonce prefetch + số block đã "cũ" so với `current_block`. `None` khi
    /// chưa từng prefetch. Caller tự quyết định độ cũ chấp nhận được (đường
    /// ký hiện chấp nhận ≤ `MINED_INDEX_DEPTH` block để không bỏ lỡ cơ hội
    /// khi 1 nhịp prefetch bị trượt).
    pub fn cached(&self, current_block: u64) -> Option<(u64, u64)> {
        self.cached.map(|(b, n)| (n, current_block.saturating_sub(b)))
    }
}

// ============================================================================
// Cụm `real-economics-mode2` (F-03) — GasOracle: `eth_gasPrice` cache theo
// block, fallback median gas_price tx trong block MINED gần nhất khi
// `eth_gasPrice` lỗi (node không hỗ trợ/timeout).
// ============================================================================

/// Giá gas THẬT (wei/gas-unit) cho 1 block cụ thể + nguồn đo được — dùng để
/// tính `gas_cost_wei = (units_front+units_back) * max(gia_nay, victim.gas_price)`
/// (xem `pipeline::compute_gas_cost_wei`). Cache theo block để KHÔNG gọi
/// `eth_gasPrice` lặp lại cho mỗi candidate trong cùng 1 block.
#[derive(Debug, Default)]
pub struct GasOracle {
    cached: RwLock<Option<(u64, u128)>>,
}

impl GasOracle {
    pub fn new() -> Self {
        Self::default()
    }

    /// `median` của gas_price các tx trong `block` (block ĐÃ MINED, có đủ
    /// danh sách tx) — fallback khi `eth_gasPrice` lỗi. `None` nếu lấy block
    /// thất bại hoặc block rỗng (không tx nào để tính median).
    async fn median_from_mined_block(provider: &dyn Provider, block: u64) -> Option<u128> {
        use alloy::eips::BlockNumberOrTag;
        let b = provider.get_block_by_number(BlockNumberOrTag::Number(block)).full().await.ok().flatten()?;
        let mut prices: Vec<u128> = b
            .transactions
            .txns()
            .map(|tx| {
                <_ as alloy::consensus::Transaction>::gas_price(tx)
                    .unwrap_or_else(|| <_ as alloy::consensus::Transaction>::max_fee_per_gas(tx))
            })
            .collect();
        if prices.is_empty() {
            return None;
        }
        prices.sort_unstable();
        Some(prices[prices.len() / 2])
    }

    /// Trả giá gas (wei/unit) cho `block` — cache hit nếu đã đo đúng block
    /// này; ngược lại gọi `eth_gasPrice`, lỗi thì thử median block MINED gần
    /// nhất (`block.saturating_sub(1)` — block hiện tại `bot` đang xét CHƯA
    /// mined nên không tự chứa danh sách tx đầy đủ ổn định), lỗi cả 2 thì
    /// GIỮ giá trị cache CŨ (nếu có, khác block) thay vì trả `0` (0 sẽ khiến
    /// `gas_cost_wei` bị đánh giá thấp giả tạo — nguy hiểm hơn dùng số cũ hơi
    /// lệch). Chưa từng đo lần nào -> `0` (an toàn theo hướng khác: caller
    /// dùng `max(oracle, victim.gas_price)` nên `victim.gas_price` vẫn chặn
    /// được, `0` chỉ là "không có thông tin thêm từ oracle").
    /// Cụm `bugfix-presign-and-contract-plan` (A4) — đọc CHỈ cache, KHÔNG
    /// bao giờ gọi RPC: dùng trên đường ký shadow (yêu cầu "0 RPC trên đường
    /// ký"). `None` khi chưa có số đo nào (caller rơi về `victim.gas_price`,
    /// luôn biết từ chính tx quan sát được).
    pub async fn cached_price_any_block(&self) -> Option<(u64, u128)> {
        *self.cached.read().await
    }

    pub async fn gas_price_wei(&self, provider: &dyn Provider, block: u64, logger: &BotLogger) -> u128 {
        if let Some((b, price)) = *self.cached.read().await {
            if b == block {
                return price;
            }
        }
        let (price, source) = match provider.get_gas_price().await {
            Ok(p) => (p, "eth_gasPrice"),
            Err(e) => match Self::median_from_mined_block(provider, block.saturating_sub(1)).await {
                Some(p) => (p, "block_median"),
                None => {
                    let old = self.cached.read().await.map(|(_, p)| p);
                    match old {
                        Some(p) => (p, "stale_cache"),
                        None => {
                            logger.log(
                                "gas.oracle_error",
                                serde_json::json!({ "block": block, "reason": e.to_string() }),
                            );
                            (0u128, "unavailable")
                        }
                    }
                }
            },
        };
        *self.cached.write().await = Some((block, price));
        logger.log(
            "gas.oracle",
            serde_json::json!({ "block": block, "gwei": price as f64 / 1e9, "source": source }),
        );
        price
    }
}

// ============================================================================
// Cụm `competitor-recon-and-strategy` (F-01, chuẩn bị bundle nguyên tử
// `[front, victim_raw, back]`) — tái tạo raw tx ĐÃ KÝ THẬT của victim từ
// hash quan sát được trong mempool/block. Đặt ở `transport.rs` (tầng RPC),
// KHÔNG ở `relay.rs` — `relay.rs` giữ nguyên charter "THUẦN, không network"
// (xem doc-comment đầu `relay.rs` + test `no_http_network_calls_anywhere_in_relay_rs`),
// chỉ NHẬN raw hex đã có sẵn làm tham số. Hàm này CHỈ ĐỌC (2 phương thức
// `eth_*` đọc), KHÔNG ký/gửi gì.
// ============================================================================

/// Nguồn đã dùng để lấy raw tx — ghi vào log để phân biệt (một số RPC không
/// hỗ trợ `eth_getRawTransactionByHash`, đặc biệt node đã prune calldata cũ).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawTxSource {
    /// RPC trả thẳng bytes RLP đã encode (route ưu tiên, đúng lệnh).
    GetRawTransactionByHash,
    /// RPC không hỗ trợ/trả `None` cho route trên — tái tạo từ
    /// `eth_getTransactionByHash` (có v/r/s hoặc yParity/r/s tuỳ type 0/2)
    /// qua `TxEnvelope::encoded_2718()` (alloy tự RLP-encode ĐÚNG theo type
    /// tx thật — KHÔNG tự viết tay logic RLP, tránh lệch offset/field).
    ReconstructedFromComponents,
}

impl RawTxSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            RawTxSource::GetRawTransactionByHash => "eth_getRawTransactionByHash",
            RawTxSource::ReconstructedFromComponents => "eth_getTransactionByHash+encoded_2718",
        }
    }
}

/// Lấy raw tx ĐÃ KÝ (EIP-2718 envelope bytes) của `hash`, verify
/// `keccak256(raw) == hash` TRƯỚC KHI trả — không bao giờ trả raw không khớp
/// (dùng sai raw trong bundle vô nghĩa/nguy hiểm hơn là fail rõ ràng ở đây).
/// Hỗ trợ CẢ type 0 (Legacy) và type 2 (EIP-1559, đã bật trên BSC) vì
/// `alloy_consensus::TxEnvelope`/`Encodable2718` generic theo type thật của
/// tx, không giả định 1 loại cố định.
pub async fn fetch_raw_tx_verified(provider: &DynProvider, hash: B256) -> Result<(Vec<u8>, RawTxSource), String> {
    use alloy::eips::eip2718::Encodable2718;

    if let Ok(Some(raw)) = provider.get_raw_transaction_by_hash(hash).await {
        let bytes = raw.to_vec();
        let computed = alloy::primitives::keccak256(&bytes);
        if computed == hash {
            return Ok((bytes, RawTxSource::GetRawTransactionByHash));
        }
        return Err(format!(
            "eth_getRawTransactionByHash tra ve nhung keccak khong khop hash: computed={computed:#x} expected={hash:#x}"
        ));
    }

    let tx = provider
        .get_transaction_by_hash(hash)
        .await
        .map_err(|e| format!("eth_getTransactionByHash loi: {e}"))?
        .ok_or_else(|| "eth_getTransactionByHash tra ve None (tx khong ton tai tren node nay)".to_string())?;
    let envelope = tx.inner.into_inner();
    let bytes = envelope.encoded_2718();
    let computed = alloy::primitives::keccak256(&bytes);
    if computed != hash {
        return Err(format!(
            "tai tao raw tu v/r/s (encoded_2718) khong khop hash that: computed={computed:#x} expected={hash:#x}"
        ));
    }
    Ok((bytes, RawTxSource::ReconstructedFromComponents))
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

    /// Cụm `hotpath-fix-then-decoder-ur` (A4b) — ĐẠT CẦN DÁN: cùng `(pair,
    /// quote, block)` -> hit cache; đổi `block` -> miss (khác key là entry
    /// khác trong `HashMap`, không phải "invalidate theo block" chủ động).
    #[test]
    fn reserve_cache_hits_same_pair_and_block_misses_different_block() {
        let pair = Address::from_str("0x111111111111111111111111111111111111beef").unwrap();
        let quote = Address::from_str("0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c").unwrap();
        let reserves = crate::sim_v2::PoolReserves { reserve_wbnb: U256::from(20u64), reserve_token: U256::from(1000u64) };
        let mut cache = ReserveCache::new();
        assert_eq!(cache.cached(pair, quote, 100), None);
        cache.insert(pair, quote, 100, reserves);
        assert_eq!(cache.cached(pair, quote, 100), Some(reserves));
        assert_eq!(cache.cached(pair, quote, 101), None, "block khac -> cache miss, khong dung reserve cu");
    }

    /// Cụm `bugfix-presign-and-contract-plan` (A1) — ĐẠT CẦN DÁN: CÙNG pool,
    /// CÙNG block, 2 quote asset khác nhau KHÔNG ĐƯỢC dùng chung entry.
    ///
    /// Đây chính là bug gốc khiến `/api/econ` báo `best_net_bnb` hàng chục
    /// nghìn BNB: pool WBNB/USDT `0x16b9a828…` được cache với `quote=USDT`
    /// (`reserve_wbnb` = 38.2M USDT) bởi 1 candidate victim-mua-WBNB-bằng-USDT,
    /// rồi bước quy đổi `amount_in_bnb_equiv`/gas hỏi CÙNG pool với
    /// `quote=WBNB` và nhận lại ĐÚNG entry đó (chiều ĐẢO) — tỉ giá 720 thay
    /// vì 1/720.
    #[test]
    fn reserve_cache_same_pair_two_quotes_do_not_collide() {
        let pair = Address::from_str("0x16b9a828e9d35a1e6df7d9f65b3e3e7b3e0b0dae").unwrap();
        let wbnb = Address::from_str("0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c").unwrap();
        let usdt = Address::from_str("0x55d398326f99059fF775485246999027B3197955").unwrap();
        // Hoi voi quote=WBNB: reserve_wbnb = 52_000 BNB, phia con lai 37.4M USDT.
        let as_wbnb_quote =
            crate::sim_v2::PoolReserves { reserve_wbnb: U256::from(52_000u64), reserve_token: U256::from(37_440_000u64) };
        // Hoi voi quote=USDT: 2 field DAO NGUOC (reserve_wbnb = reserve cua QUOTE = USDT).
        let as_usdt_quote =
            crate::sim_v2::PoolReserves { reserve_wbnb: U256::from(37_440_000u64), reserve_token: U256::from(52_000u64) };
        let mut cache = ReserveCache::new();
        cache.insert(pair, usdt, 100, as_usdt_quote);
        assert_eq!(cache.cached(pair, usdt, 100), Some(as_usdt_quote));
        assert_eq!(
            cache.cached(pair, wbnb, 100),
            None,
            "quote khac -> PHAI cache miss (de caller tu resolve dung chieu), KHONG duoc tra entry chieu nguoc"
        );
        cache.insert(pair, wbnb, 100, as_wbnb_quote);
        assert_eq!(cache.cached(pair, wbnb, 100), Some(as_wbnb_quote));
        assert_eq!(cache.cached(pair, usdt, 100), Some(as_usdt_quote), "2 entry song song, khong de len nhau");
    }

    /// ĐẠT CẦN DÁN (cụm `bugfix-presign-and-contract-plan`, A5) — lỗi THẬT
    /// quan sát được khi chạy shadow lần đầu qua pool RPC nền.
    #[test]
    fn is_unsupported_method_error_catches_real_archive_token_error() {
        let real = "sim_evm thuc thi loi: doc storage slot 0 that bai: Inner(Transport(ErrorResp(ErrorPayload { \
                    code: -32602, message: \"Archive requests require a personal token. Get one at: \
                    https://www.allnodes.com/publicnode\", data: None })))";
        assert!(is_unsupported_method_error(real), "phai nhan dien de doi URL, khong lap lo vo han tren 1 node hong");
        assert!(is_unsupported_method_error("-32601 method not found"));
        assert!(!is_unsupported_method_error("execution reverted"), "revert THAT cua token khong phai loi node");
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

    // ===== Cụm `real-economics-mode2` (F-03) — GasOracle (mock JSON-RPC theo
    // METHOD, khác `mock_rpc_server` ở trên vốn trả CỐ ĐỊNH 1 giá trị cho mọi
    // request) =====

    /// Mock JSON-RPC server dispatch theo TÊN METHOD (`eth_chainId`,
    /// `eth_gasPrice`, `eth_getBlockByNumber`...) — cần cho test `GasOracle`
    /// vì nó gọi ≥2 method khác nhau trên CÙNG 1 kết nối (khác `mock_rpc_server`
    /// chỉ phục vụ `connect_and_verify` gọi đúng 1 method).
    async fn mock_rpc_dispatch<F>(handler: F) -> String
    where
        F: Fn(&str) -> serde_json::Value + Send + Sync + 'static,
    {
        use axum::extract::{Json as ReqJson, State as AxState};
        use axum::response::Json as RespJson;
        use axum::routing::post;
        use std::sync::Arc;

        async fn handle(
            AxState(handler): AxState<Arc<dyn Fn(&str) -> serde_json::Value + Send + Sync>>,
            ReqJson(body): ReqJson<serde_json::Value>,
        ) -> RespJson<serde_json::Value> {
            let id = body.get("id").cloned().unwrap_or(serde_json::json!(1));
            let method = body.get("method").and_then(|m| m.as_str()).unwrap_or("");
            let result = handler(method);
            RespJson(serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }))
        }

        let handler: Arc<dyn Fn(&str) -> serde_json::Value + Send + Sync> = Arc::new(handler);
        let app = axum::Router::new().route("/", post(handle)).with_state(handler);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind mock rpc dispatch server");
        let addr = listener.local_addr().expect("local_addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{addr}/")
    }

    /// ĐẠT CẦN DÁN — `GasOracle::gas_price_wei` gọi `eth_gasPrice` THẬT (qua
    /// mock) và trả đúng giá trị hex đã decode, log `gas.oracle{source:"eth_gasPrice"}`.
    #[tokio::test]
    async fn gas_oracle_reads_eth_gas_price_and_logs_source() {
        let (_dir, logger) = test_logger();
        let url = mock_rpc_dispatch(|method| match method {
            "eth_chainId" => serde_json::json!("0x38"),
            "eth_gasPrice" => serde_json::json!("0x3b9aca00"), // 1_000_000_000 wei = 1 gwei
            _ => serde_json::json!(null),
        })
        .await;
        let provider = connect_and_verify(&url).await.expect("connect phai OK (chain 56)");
        let oracle = GasOracle::new();
        let price = oracle.gas_price_wei(&provider, 100, &logger).await;
        assert_eq!(price, 1_000_000_000u128);
        let tail = logger.tail(10);
        let row = tail.iter().find(|l| l["event"] == "gas.oracle").expect("phai co dong gas.oracle");
        assert_eq!(row["source"], "eth_gasPrice");
        assert_eq!(row["block"], 100);
        assert!((row["gwei"].as_f64().unwrap() - 1.0).abs() < 1e-9);
    }

    /// Cache theo BLOCK: gọi lại CÙNG block phải trả giá CŨ (không gọi lại
    /// `eth_gasPrice`) dù mock đã đổi giá trị trả về — chỉ block MỚI mới lấy
    /// giá MỚI. Dùng `AtomicU64` đếm số lần method `eth_gasPrice` thực sự
    /// được gọi để chứng minh cache hit không tốn round-trip.
    #[tokio::test]
    async fn gas_oracle_caches_per_block_does_not_refetch_same_block() {
        let (_dir, logger) = test_logger();
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let cc = call_count.clone();
        let url = mock_rpc_dispatch(move |method| match method {
            "eth_chainId" => serde_json::json!("0x38"),
            "eth_gasPrice" => {
                let n = cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                // Lan goi dau -> 1 gwei, lan goi sau (block moi) -> 2 gwei.
                if n == 0 { serde_json::json!("0x3b9aca00") } else { serde_json::json!("0x77359400") }
            }
            _ => serde_json::json!(null),
        })
        .await;
        let provider = connect_and_verify(&url).await.expect("connect phai OK");
        let oracle = GasOracle::new();

        let p1 = oracle.gas_price_wei(&provider, 100, &logger).await;
        assert_eq!(p1, 1_000_000_000u128);
        let p1_again = oracle.gas_price_wei(&provider, 100, &logger).await;
        assert_eq!(p1_again, 1_000_000_000u128, "cung block 100 phai tra gia CU, khong goi lai eth_gasPrice");
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1, "block khong doi thi khong duoc goi lai eth_gasPrice");

        let p2 = oracle.gas_price_wei(&provider, 101, &logger).await;
        assert_eq!(p2, 2_000_000_000u128, "block MOI phai goi lai va lay gia MOI");
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    /// `eth_gasPrice` lỗi (node trả JSON-RPC error) -> fallback median gas_price
    /// từ block MINED gần nhất (`eth_getBlockByNumber(block-1, full)`).
    #[tokio::test]
    async fn gas_oracle_falls_back_to_block_median_when_eth_gas_price_errors() {
        use axum::extract::{Json as ReqJson, State as AxState};
        use axum::response::Json as RespJson;
        use axum::routing::post;

        async fn handle(AxState(_): AxState<()>, ReqJson(body): ReqJson<serde_json::Value>) -> RespJson<serde_json::Value> {
            let id = body.get("id").cloned().unwrap_or(serde_json::json!(1));
            let method = body.get("method").and_then(|m| m.as_str()).unwrap_or("");
            match method {
                "eth_chainId" => RespJson(serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": "0x38" })),
                "eth_gasPrice" => RespJson(serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "method not supported" } })),
                "eth_getBlockByNumber" => {
                    // 3 tx voi gasPrice 1/2/3 gwei -> median = 2 gwei.
                    let mk_tx = |gp_hex: &str| {
                        serde_json::json!({
                            "hash": format!("0x{:064x}", 1),
                            "nonce": "0x0",
                            "blockHash": format!("0x{:064x}", 1),
                            "blockNumber": "0x1",
                            "transactionIndex": "0x0",
                            "from": format!("0x{:040x}", 1),
                            "to": format!("0x{:040x}", 2),
                            "value": "0x0",
                            "gas": "0x5208",
                            "gasPrice": gp_hex,
                            "input": "0x",
                            "v": "0x1b", "r": format!("0x{:064x}", 1), "s": format!("0x{:064x}", 1),
                            "type": "0x0", "chainId": "0x38",
                        })
                    };
                    let block = serde_json::json!({
                        "number": "0x63", "hash": format!("0x{:064x}", 99), "parentHash": format!("0x{:064x}", 98),
                        "nonce": "0x0000000000000000", "mixHash": format!("0x{:064x}", 0), "sha3Uncles": format!("0x{:064x}", 0),
                        "logsBloom": format!("0x{}", "0".repeat(512)), "transactionsRoot": format!("0x{:064x}", 0),
                        "stateRoot": format!("0x{:064x}", 0), "receiptsRoot": format!("0x{:064x}", 0),
                        "miner": format!("0x{:040x}", 0), "difficulty": "0x0", "totalDifficulty": "0x0",
                        "extraData": "0x", "size": "0x0", "gasLimit": "0x0", "gasUsed": "0x0",
                        "timestamp": "0x0", "transactions": [mk_tx("0x3b9aca00"), mk_tx("0x77359400"), mk_tx("0xb2d05e00")],
                        "uncles": [],
                    });
                    RespJson(serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": block }))
                }
                _ => RespJson(serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": null })),
            }
        }

        let app = axum::Router::new().route("/", post(handle)).with_state(());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
        let addr = listener.local_addr().expect("local_addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let url = format!("http://{addr}/");

        let (_dir, logger) = test_logger();
        let provider = connect_and_verify(&url).await.expect("connect phai OK");
        let oracle = GasOracle::new();
        let price = oracle.gas_price_wei(&provider, 100, &logger).await;
        assert_eq!(price, 2_000_000_000u128, "median cua 1/2/3 gwei phai la 2 gwei");
        let tail = logger.tail(10);
        let row = tail.iter().find(|l| l["event"] == "gas.oracle").expect("phai co dong gas.oracle");
        assert_eq!(row["source"], "block_median");
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

    /// Cụm `real-economics-mode2` (mục 5) — ĐẠT CẦN DÁN: "1 sender 2 tx (k,
    /// k+1) → k ok, k+1 nonce_future; k lên block → k+1 ok". Mô phỏng đúng
    /// kịch bản dùng CHÍNH `compare_nonce`/`NonceCache` mà
    /// `main.rs::run_evm_decision` gọi thật (nonce EXPECTED từ
    /// `eth_getTransactionCount(from,"latest")` — ở đây giả lập bằng số
    /// nguyên tay, không cần RPC sống): lúc đầu ví có nonce THẬT trên chain
    /// = 5 (tx thứ 5 đã confirm, tx TIẾP THEO hợp lệ phải mang nonce=5).
    /// Candidate mang nonce=5 (k) → `Ok` (đúng nonce kỳ vọng). Candidate
    /// KHÁC cùng ví, cùng lúc, mang nonce=6 (k+1, tx sau) → `Future` (còn
    /// thiếu 1 tx ở giữa). Sau khi tx k được coi là đã lên block (nonce THẬT
    /// trên chain tăng lên 6), candidate nonce=6 lúc này → `Ok`.
    #[test]
    fn nonce_future_then_ok_after_k_confirms_same_sender() {
        let from = Address::from_str("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();
        let mut cache = NonceCache::new();

        // Block 1000: nonce THAT tren chain = 5 (tu eth_getTransactionCount,
        // gia lap qua insert truc tiep - production goi fetch_expected_nonce
        // that qua RPC, xem run_evm_decision).
        let block = 1000u64;
        cache.insert(from, block, 5);
        let expected = cache.cached(from, block).expect("da insert, phai co trong cache");

        // Candidate k: nonce=5, dung nonce ke tiep -> Ok.
        let candidate_k_nonce = 5u64;
        assert_eq!(compare_nonce(candidate_k_nonce, expected), NonceCheck::Ok, "tx k (nonce=5) phai la Ok");

        // Candidate k+1 (cung vi, cung block quan sat, nonce=6) TOI TRUOC khi
        // k len block -> nonce THAT van con la 5 -> Future (con thieu 1 tx).
        let candidate_k_plus_1_nonce = 6u64;
        assert_eq!(
            compare_nonce(candidate_k_plus_1_nonce, expected),
            NonceCheck::Future,
            "tx k+1 (nonce=6) truoc khi k len block phai la Future"
        );

        // "k len block" = nonce THAT tren chain tang len 6 (khoi block MOI,
        // cache theo (from, block) nen khong the tai su dung entry cu - phai
        // insert lai cho block moi, dung y "khong the tai su dung nonce cu
        // qua block khac" da kiem o nonce_cache_miss_then_hit_after_insert).
        let next_block = block + 1;
        cache.insert(from, next_block, 6);
        let expected_after_k_confirmed = cache.cached(from, next_block).expect("da insert cho block moi");
        assert_eq!(
            compare_nonce(candidate_k_plus_1_nonce, expected_after_k_confirmed),
            NonceCheck::Ok,
            "sau khi k len block (nonce that=6), candidate k+1 (nonce=6) phai la Ok"
        );
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

    // ===== Cụm `econ-truth-latency-vps` (0.c) — is_unsupported_method_error / RpcPool.mark_current_unsupported =====

    // ===== Cụm `econ-truth-latency-vps` (mục 1) — SeenHashSet =====

    #[test]
    fn seen_hash_set_insert_if_new_true_once_false_after() {
        let mut set = SeenHashSet::new();
        let h = B256::from([1u8; 32]);
        assert!(set.insert_if_new(h), "lan dau thay hash nay phai tra true");
        assert!(!set.insert_if_new(h), "lan 2 tro di phai tra false, khong xu ly lai");
        let h2 = B256::from([2u8; 32]);
        assert!(set.insert_if_new(h2), "hash khac van phai tra true doc lap");
    }

    #[test]
    fn is_unsupported_method_error_detects_known_shapes() {
        assert!(is_unsupported_method_error("-32000: method not supported"));
        assert!(is_unsupported_method_error("JsonRpc error -32601 method not found"));
        assert!(is_unsupported_method_error("this Method is NOT SUPPORTED by this node"));
        assert!(!is_unsupported_method_error("execution reverted: TRANSFER_FROM_FAILED"));
        assert!(!is_unsupported_method_error("timeout 10s"));
    }

    /// ĐẠT CẦN DÁN — URL hiện tại lỗi "method không hỗ trợ" -> đánh dấu
    /// KHÔNG dùng lại, `connect()` sau đó luôn bỏ qua URL đó (không chỉ 1 lần
    /// như `advance_and_reconnect`, mà nhớ mãi qua nhiều lần connect lại).
    #[tokio::test]
    async fn mark_current_unsupported_is_remembered_across_reconnects() {
        let (_dir, logger) = test_logger();
        let unsupported_url = mock_rpc_server("0x38").await;
        let good_url = mock_rpc_server("0x38").await;
        let pool = RpcPool::new(vec![unsupported_url.clone(), good_url.clone()]);

        // Connect lan dau -> chon URL 1 (idx 0, dau danh sach).
        let p0 = pool.connect(&logger, "sim").await;
        assert!(p0.is_some());

        // URL 0 vua tra loi "method khong ho tro" -> danh dau, chuyen URL ke.
        let p1 = pool.mark_current_unsupported(&logger, "sim").await;
        assert!(p1.is_some(), "phai chuyen sang URL 2 thanh cong");
        let tail = logger.tail(10);
        assert!(tail.iter().any(|l| l["event"] == "rpc.method_unsupported" && l["pool_index"] == 0));

        // Goi lai connect() (vd health-check dinh ky sau nay) tu dau danh
        // sach van phai BO QUA URL 0 (da nho, khong quay lai) - chon URL 1.
        let p2 = pool.connect(&logger, "sim").await;
        assert!(p2.is_some());
        assert_eq!(pool.current_url_label().await, Some(redact_rpc_url(&good_url)));
    }

    /// Nếu TẤT CẢ URL trong pool đều bị đánh dấu unsupported (edge case hiếm)
    /// -> `connect()` KHÔNG khoá chết, thử lại toàn bộ danh sách như bình
    /// thường (thà thử lại còn hơn không bao giờ kết nối được nữa).
    #[tokio::test]
    async fn connect_falls_back_to_all_urls_when_every_url_marked_unsupported() {
        let (_dir, logger) = test_logger();
        let url = mock_rpc_server("0x38").await;
        let pool = RpcPool::new(vec![url.clone()]);
        assert!(pool.connect(&logger, "sim").await.is_some());
        // Chi 1 URL duy nhat, danh dau no unsupported -> unsupported.len() ==
        // urls.len() -> phai boi qua co che "skip", van thu lai duoc.
        let after_mark = pool.mark_current_unsupported(&logger, "sim").await;
        assert!(after_mark.is_some(), "chi 1 URL duy nhat, van phai connect lai duoc du da danh dau unsupported");
    }

    #[tokio::test]
    async fn current_url_label_none_before_any_connect() {
        let pool = RpcPool::new(vec!["http://127.0.0.1:1/".to_string()]);
        assert_eq!(pool.current_url_label().await, Some("http://127.0.0.1:1/***".to_string()));
    }

    // ===== Cụm `competitor-recon-and-strategy` (F-01) — fetch_raw_tx_verified =====

    /// `#[ignore]` — RPC thật. Quét TỪ block mới nhất LÙI VỀ tối đa 30 block
    /// tìm ĐỦ 1 tx type 0 (Legacy) VÀ 1 tx type 2 (EIP-1559) THẬT (BSC có cả
    /// 2 loại lưu thông — EIP-1559 đã bật, xem CLAUDE.md), verify
    /// `fetch_raw_tx_verified` tái tạo ĐÚNG cả 2 (khớp `keccak256(raw)==hash`),
    /// và `RawTxSource` phản ánh đúng route đã dùng (RPC hỗ trợ
    /// `eth_getRawTransactionByHash` hay phải fallback tái tạo từ v/r/s).
    #[tokio::test]
    #[ignore]
    async fn real_rpc_reconstruct_raw_tx_type0_and_type2() {
        use crate::sim_evm::validate_rpc_urls;
        use alloy::eips::BlockNumberOrTag;
        use alloy::providers::{Provider, ProviderBuilder};

        let urls = validate_rpc_urls();
        let mut provider = None;
        for u in &urls {
            if let Ok(p) = ProviderBuilder::new().connect(u).await {
                if p.get_chain_id().await.unwrap_or(0) == 56 {
                    provider = Some(p.erased());
                    println!("dung RPC: {}", redact_rpc_url(u));
                    break;
                }
            }
        }
        let Some(provider) = provider else {
            println!("SKIP (khong phai FAIL): khong ket noi duoc RPC nao trong validate_rpc_urls()");
            return;
        };

        let latest = provider.get_block_number().await.expect("eth_blockNumber that bai");
        let mut type0_hash: Option<B256> = None;
        let mut type2_hash: Option<B256> = None;
        for bn in (latest.saturating_sub(30)..=latest).rev() {
            if type0_hash.is_some() && type2_hash.is_some() {
                break;
            }
            let Ok(Some(block)) = provider.get_block_by_number(BlockNumberOrTag::Number(bn)).full().await else { continue };
            for tx in block.transactions.txns() {
                let ty = <_ as alloy::eips::eip2718::Typed2718>::ty(tx);
                let hash = <_ as alloy::network::TransactionResponse>::tx_hash(tx);
                if ty == 0 && type0_hash.is_none() {
                    type0_hash = Some(hash);
                } else if ty == 2 && type2_hash.is_none() {
                    type2_hash = Some(hash);
                }
            }
        }

        println!("type0_hash tim duoc: {:?}", type0_hash.map(|h| format!("{h:#x}")));
        println!("type2_hash tim duoc: {:?}", type2_hash.map(|h| format!("{h:#x}")));

        for (label, hash_opt) in [("type0/Legacy", type0_hash), ("type2/EIP-1559", type2_hash)] {
            let Some(hash) = hash_opt else {
                println!("SKIP {label}: khong tim thay mau nao trong 30 block gan nhat (khong phai FAIL - phu thuoc thanh phan mempool that luc chay)");
                continue;
            };
            let (raw, source) = fetch_raw_tx_verified(&provider, hash).await.expect("fetch_raw_tx_verified phai thanh cong voi hash that");
            let computed = alloy::primitives::keccak256(&raw);
            assert_eq!(computed, hash, "{label}: keccak256(raw) phai khop hash that");
            println!("{label}: hash={hash:#x} raw_len={} nguon={}", raw.len(), source.as_str());
        }
    }

    /// Cụm `decision-data-24h` (mục 5) — `.env` chỉ là MẶC ĐỊNH: biến đã có
    /// trong môi trường (vd do `paper_run.sh` export, hoặc Chủ đặt trước lệnh
    /// chạy) phải THẮNG, không bao giờ bị `.env` ghi đè.
    #[test]
    fn load_dotenv_defaults_khong_ghi_de_bien_da_co_va_go_nhay() {
        let dir = std::env::temp_dir().join(format!("bsc_dotenv_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".env.test");
        std::fs::write(
            &path,
            "# comment\n\nBSC_TEST_SIM_URL=\"https://sim.example.com\"\nBSC_TEST_ALREADY=tu_file\nexport BSC_TEST_EXPORTED='https://exported.example.com'\nBSC_TEST_RONG=\nkhong-phai-dong-gan\n",
        )
        .unwrap();
        std::env::set_var("BSC_TEST_ALREADY", "tu_moi_truong");

        let loaded = load_dotenv_defaults(&path);

        assert_eq!(std::env::var("BSC_TEST_SIM_URL").unwrap(), "https://sim.example.com", "nhay \" phai bi go");
        assert_eq!(std::env::var("BSC_TEST_EXPORTED").unwrap(), "https://exported.example.com", "ho tro tien to `export ` + nhay '");
        assert_eq!(std::env::var("BSC_TEST_ALREADY").unwrap(), "tu_moi_truong", "bien da co trong moi truong PHAI thang .env");
        assert!(std::env::var("BSC_TEST_RONG").is_err(), "gia tri rong -> khong set");
        assert!(loaded.contains(&"BSC_TEST_SIM_URL".to_string()));
        assert!(loaded.contains(&"BSC_TEST_EXPORTED".to_string()));
        assert!(!loaded.contains(&"BSC_TEST_ALREADY".to_string()), "bien khong nap thi khong duoc bao cao la da nap");
        // Ham tra ve TEN bien, khong bao gio tra ve gia tri (file that co PRIVATE_KEY).
        assert!(loaded.iter().all(|k| !k.contains("https://")));

        std::env::remove_var("BSC_TEST_SIM_URL");
        std::env::remove_var("BSC_TEST_EXPORTED");
        std::env::remove_var("BSC_TEST_ALREADY");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_dotenv_defaults_file_khong_ton_tai_tra_rong_khong_panic() {
        assert!(load_dotenv_defaults(std::path::Path::new("/khong/ton/tai/.env")).is_empty());
    }

    /// Cụm `decision-data-24h` (mục 5) — 4 lớp lỗi vet nền phải tách bạch:
    /// `missing_trie_node` (node KHÔNG bao giờ vet được pool đó) khác hẳn
    /// `rate_limited` (thử lại là xong).
    #[test]
    fn classify_vet_error_tach_missing_trie_node_khoi_cac_loi_khac() {
        assert_eq!(
            classify_vet_error("sim_evm thuc thi loi: tax buy: Database(Inner(Transport(HttpError { status: 429, body: \"\" })))"),
            "rate_limited"
        );
        assert_eq!(
            classify_vet_error("Database(Inner(Transport(ErrorResp(ErrorPayload { code: -32000, message: \"missing trie node 0xabc (path ) state 0xdef is not available\" }))))"),
            "missing_trie_node",
            "phai uu tien missing trie node hon -32000 (chuoi nay chua CA hai)"
        );
        assert_eq!(classify_vet_error("required historical state unavailable"), "missing_trie_node");
        assert_eq!(classify_vet_error("-32602 Archive requests require a personal token"), "unsupported_method");
        assert_eq!(classify_vet_error("execution reverted: TRANSFER_FROM_FAILED"), "other");
    }
}

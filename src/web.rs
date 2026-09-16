use alloy::primitives::Address;
use alloy::providers::DynProvider;
use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{RwLock, Semaphore};
use tower_http::services::ServeDir;

use crate::config::Config;
use crate::logger::BotLogger;
use crate::pairbook::PairBook;
use crate::state::{BotState, ControlAction, StateFiles};
use crate::tax::TaxCache;
use crate::transport::PendingSource;
use crate::venues::{registry_snapshot, SKIP_REASONS};
use crate::victims::VictimBook;

/// Trạng thái dùng chung cho toàn bộ web dashboard. Chỉ đọc file/memory bot;
/// nút Halt/Disarm/Reset chỉ ghi state/*.req, KHÔNG gọi signer.
///
/// `config` là `RwLock` (khác các cụm trước) — cụm config-hot-reload thêm
/// task nền gọi `Config::reload_if_due` mỗi `config_reload_sec`, đọc bằng
/// `.read().await` ở mọi nơi (chỉ `venues()`/`status()` dùng tới) — cùng quy
/// ước `victims: RwLock<VictimBook>` đã có từ trước.
pub struct AppStateInner {
    pub config: RwLock<Config>,
    pub victims: RwLock<VictimBook>,
    /// Cụm pair-mode — `PairBook` theo dõi POOL (token/WBNB) chỉ định qua
    /// `pairs.txt`, chạy SONG SONG `victims` (không thay thế).
    pub pairbook: RwLock<PairBook>,
    pub state_files: Arc<StateFiles>,
    pub logger: Arc<BotLogger>,
    pub bot_state: RwLock<BotState>,
    pub start_time: Instant,
    /// Cụm `real-economics-mode2` — mốc thời gian THỰC (wall-clock, khớp định
    /// dạng `ts` mà `BotLogger::log` ghi — `Utc::now().to_rfc3339()`) lúc boot.
    /// `GET /api/econ` dùng field này để CHỈ tính dòng của LẦN CHẠY HIỆN TẠI —
    /// `logs/bot.jsonl` là file DÙNG CHUNG, KHÔNG bị xoá giữa các lần chạy
    /// (khác `skip_counts`/`FunnelCounters`, 2 bộ đếm trong RAM tự reset mỗi
    /// lần boot) — thiếu mốc này, `/api/econ` sẽ lẫn lộn số liệu của MỌI lần
    /// chạy trước đó trong lịch sử file, không phải chỉ lần chạy đang xem.
    pub boot_wall_clock: chrono::DateTime<chrono::Utc>,
    pub skip_counts: RwLock<HashMap<String, u64>>,
    pub last_block: RwLock<Option<u64>>,
    /// Cụm `5.1` — provider HTTP đã kết nối (nếu `.env`/`BSC_HTTP` hợp lệ),
    /// dùng lại cho `pipeline::resolve_v2_reserves` trong live paper loop
    /// (`main.rs`) thay vì mở kết nối riêng mỗi tx. `None` khi chưa kết nối
    /// được — loop paper skip `no_pool` cho tới khi có provider.
    pub provider: RwLock<Option<DynProvider>>,
    /// Cache tax roundtrip dùng chung toàn bot — CHỈ điền thủ công/test
    /// (xem `docs/STATE.md`/`src/tax.rs`), live loop `5.1` KHÔNG tự động gọi
    /// `tax::measure_roundtrip_via_router` để điền cache này (quyết định từ
    /// phiên `3.3`, chưa đổi ở đây).
    pub tax_cache: RwLock<TaxCache>,
    /// Cụm B3.4 — 50 dòng `validate.victim` mới nhất + tỉ lệ tích luỹ.
    ///
    /// **Vì sao KHÔNG có `evm_fork` dùng chung trong `AppStateInner`** (giới
    /// hạn kỹ thuật THẬT, ghi rõ thay vì im lặng): `revm`'s `Evm`/`Context`
    /// chứa `Rc<RefCell<...>>` (LocalContext) + con trỏ thô trong interpreter
    /// nên `!Send`/`!Sync` — KHÔNG thể đặt trong `AppStateInner` (phải `Sync`
    /// để `Arc<AppStateInner>: Send` cho axum/tokio) lẫn giữ qua `.await`
    /// trong task đã `spawn` (yêu cầu `Send`). Vì vậy fork EVM được mở LẠI mỗi
    /// candidate tới bước sim (số này rất ít sau các gate rẻ), và tái dùng
    /// trong CÙNG lời gọi `refine_front_in_on_fork` (5 lần thử `front_in`
    /// dùng chung 1 fork đã warm — đúng phần "≤50ms/tx sau tx đầu" đã chứng
    /// minh ở test B4''.4). Chia sẻ fork xuyên tx cần 1 worker-thread riêng
    /// (actor) — ngoài phạm vi cụm này, ghi CÒN NỢ.
    pub validate_log: RwLock<ValidateStats>,
    /// Giới hạn xử lý tx paper đồng thời ≤ 4 (CLAUDE.md lệnh `5.1`).
    pub pending_semaphore: Arc<Semaphore>,
    /// Cụm `5.2` — nguồn pending-tx ĐANG hoạt động (`ws`/`txpool`/
    /// `inject_only`), cập nhật bởi `main.rs::subscribe_pending_txs`/
    /// `poll_txpool_pending` ngay khi subscribe/poll thành công lần đầu. Đọc
    /// bởi `GET /api/status` (`pending_source`) — KHÔNG bịa, mặc định
    /// `InjectOnly` cho tới khi có bằng chứng thật.
    pub pending_source: RwLock<PendingSource>,
    /// Cụm `7.1`/pair-mode — `RiskGuard` (đếm lỗ liên tiếp + trần front sau
    /// gas reserve, xem `config.rs::RiskGuard`), dùng chung cho
    /// `pipeline::decide_paper_v2` trong live loop.
    pub risk_guard: RwLock<crate::config::RiskGuard>,
    /// Cụm `exec-path-traps` (F-13) — cache `(from, block) -> nonce kỳ vọng`
    /// (`eth_getTransactionCount`, xem `transport::NonceCache`), dùng bởi
    /// `main.rs::run_evm_decision` TRƯỚC khi mở fork EVM cho mỗi candidate.
    pub nonce_cache: RwLock<crate::transport::NonceCache>,
    /// Cụm `foundation-fix-then-real-sim` (A6) — bộ đếm funnel theo gate
    /// order THẬT (A4): `seen -> not_pancake_router -> decode_fail ->
    /// not_wbnb_pair -> venue_v3|venue_v2 -> no_pool/rpc_error -> below_min ->
    /// thin_liq -> honeypot_or_tax -> unprofitable -> victim_would_revert ->
    /// simulated`. Trước đây (BAOCAO30) sống rời trong `main.rs` vì `web.rs`
    /// ngoài phạm vi lệnh khi đó — nay trong phạm vi, đưa vào
    /// `AppStateInner` để `GET /api/funnel` đọc trực tiếp (không cần `Arc`
    /// bọc thêm, `AppStateInner` đã nằm sau `Arc` — xem `AppState`).
    pub funnel: FunnelCounters,
    /// Cụm `real-economics-mode2` (F-03) — cache `eth_gasPrice` theo block
    /// (+ fallback median block MINED gần nhất khi lỗi), dùng bởi
    /// `main.rs::handle_paper_tx` để tính `gas_cost_wei` THẬT thay vì trần
    /// cấu hình cố định.
    pub gas_oracle: crate::transport::GasOracle,
    /// F-03 — `(gas_units_front, gas_units_back)` hiện dùng: khởi tạo bằng
    /// giá trị FALLBACK từ `config.toml` (`gas_units_front`/`gas_units_back`),
    /// được `main.rs::gas_units_boot_task` ghi đè bằng số ĐO THẬT (revm, 1
    /// lần lúc boot trên 1 pair đã vet) nếu đo thành công.
    pub gas_units: RwLock<(u64, u64)>,
    /// Cụm `hotpath-fix-then-decoder-ur` (A4b) — cache `(pair, block) ->
    /// reserves` để KHÔNG gọi lặp lại `eth_call getReserves`/`token0` cho
    /// CÙNG 1 pool trong CÙNG 1 block (nhiều candidate cùng pool nóng rất phổ
    /// biến, xem `transport::ReserveCache`).
    pub reserve_cache: RwLock<crate::transport::ReserveCache>,
    /// Cụm `hotpath-fix-then-decoder-ur` (B5) — báo hiệu `pairbook` reload
    /// LẦN ĐẦU (có provider, thực sự chạy `PairBook::reload`) đã xong —
    /// `gas_units_boot_task` chờ tín hiệu này (thay vì đoán 1 khoảng thời
    /// gian cố định) trước khi bắt đầu đo gas thật, tránh race đã ghi nhận ở
    /// BAOCAO38 (giveup sớm hơn reload thật chỉ 700ms dù có 60s ngân sách).
    pub pairs_first_reload_done: Arc<tokio::sync::Notify>,
    /// Cụm `econ-truth-latency-vps` (0.d) — provider RPC RIÊNG cho revm/vet/
    /// validator (`pairs_vet_task`/`gas_units_boot_task`), KHÁC `provider`
    /// (đường nóng `handle_paper_tx`) — cho phép Chủ trỏ `BSC_HTTP_SIM` sang
    /// node hỗ trợ đầy đủ state cho revm fork (một số node như bloXroute trả
    /// `-32000 not supported` cho vài method cần cho fork). `None` khi chưa
    /// kết nối được URL nào trong `BSC_HTTP_SIM`/fallback `BSC_HTTP`.
    pub sim_provider: RwLock<Option<DynProvider>>,
    /// Cụm `bugfix-presign-and-contract-plan` (A5) — provider RPC **NỀN**
    /// (`BSC_HTTP_BG`, mặc định = 3 URL CUỐI của `BSC_HTTP`): dùng bởi
    /// `spawn_shadow_sign_task` (pre-sign/nonce/gas), `spawn_post_simulated_tracker`
    /// (`compete.check` + `decision_vs_mined_block`) và `spawn_victim_validator`.
    /// Tách khỏi `provider` (đường nóng `handle_paper_tx`, luôn ưu tiên URL
    /// ĐẦU danh sách) sau khi BAOCAO41 đo được p95 `seen_to_decision` tăng
    /// 321→356 ms khi thêm task nền dùng chung pool. `None` khi chưa kết nối
    /// được URL nền nào (caller rơi về `provider`, có nhãn `hot_fallback`).
    pub bg_provider: RwLock<Option<DynProvider>>,
    /// Cụm `econ-truth-latency-vps` (mục 1) — dedup hash tx DÙNG CHUNG giữa 3
    /// nguồn tx (WS/txpool/inject), đóng khoảng hở khi WS fallback sang
    /// txpool giữa chừng (xem `transport::SeenHashSet`).
    pub seen_hashes: RwLock<crate::transport::SeenHashSet>,
    /// Cụm `econ-truth-latency-vps` (mục 2) — thống kê `compete.check`.
    pub compete_stats: CompeteStats,
    /// Cụm `competitor-recon-and-strategy` (mục 4, shadow mode) — `(địa chỉ
    /// THẬT suy từ `PRIVATE_KEY`, `EthereumWallet` để ký) khi
    /// `live_mode="shadow"` VÀ load key thành công lúc boot; `None` mọi
    /// trường hợp khác (`live_mode` khác `"shadow"`, hoặc thiếu/sai key —
    /// KHÔNG panic, bot vẫn chạy paper bình thường, chỉ mất khả năng ký
    /// shadow). Nạp 1 LẦN lúc boot (không hot-reload — đổi `PRIVATE_KEY`
    /// cần khởi động lại bot, khác các field `config.toml` khác).
    pub shadow_wallet: Option<(Address, alloy::network::EthereumWallet)>,
    /// Cụm `bugfix-presign-and-contract-plan` (A4) — "mempool view" của bot:
    /// hash tx của 3 block gần nhất, nạp bởi `main.rs::mined_and_nonce_prefetch_task`
    /// (1 `eth_getBlockByNumber` không-full mỗi block, RPC NỀN). Đường ký
    /// shadow dùng cache này thay cho `eth_getTransactionReceipt(victim)`.
    pub mined_index: RwLock<crate::transport::MinedTxIndex>,
    /// Cụm `bugfix-presign-and-contract-plan` (A4) — nonce ví shadow prefetch
    /// mỗi block (cùng task trên), để đường ký không gọi
    /// `eth_getTransactionCount`.
    pub self_nonce: RwLock<crate::transport::SelfNonceCache>,
    /// Cụm `bugfix-presign-and-contract-plan` (A3) — cụm địa chỉ ĐỐI THỦ đã
    /// nhận diện (3 seed + ví nhận Transfer quote từ seed trong block hiện
    /// tại/trước), xem `competitor::ClusterIndex`.
    pub competitor_cluster: RwLock<crate::competitor::ClusterIndex>,
    /// Cụm `bugfix-presign-and-contract-plan` (A4) — `pair_addr -> lần cuối
    /// pool đó thực sự có candidate đi tới bước sim`. `pairs_vet_task` dùng
    /// để rút chu kỳ vet xuống `HOT_VET_INTERVAL_SEC` (300 s) cho ĐÚNG các
    /// pool đang nóng, thay vì vet đều mọi pool theo `pairs_vet_interval_sec`
    /// (600 s) — kết quả vet TƯƠI là điều kiện (a) của đường ký shadow, nên
    /// pool đang có cơ hội phải được đo lại thường xuyên hơn.
    pub candidate_seen: RwLock<std::collections::HashMap<Address, std::time::Instant>>,
}

/// Cụm `evm-validate-fixed-then-wire` (B3.4) — VALIDATOR NHÚNG, chỉ số SỐNG.
///
/// Đây là bản THAY THẾ LÂU DÀI cho gate B4''.3(b) (replay sandwich lịch sử):
/// thay vì đi tìm sandwich của người khác trong quá khứ (phụ thuộc mật độ dữ
/// liệu + cửa sổ state ~128 block của RPC công khai, xem `docs/STATE.md`), bot
/// TỰ ĐO ĐỘ CHÍNH XÁC của chính mình trên mọi tx nó thấy: mỗi candidate có
/// `sim.evm` sẽ được một task nền theo dõi tới khi tx lên block, rồi so
/// `victim_out` DỰ ĐOÁN với `victim_out` THẬT trong receipt.
///
/// Chạy ở CẢ paper lẫn live — nếu tỉ lệ `≤1%` tụt xuống, đó là tín hiệu sớm
/// rằng sim đang lệch khỏi thực tế (RPC sai block, token đổi hành vi, v.v.).
/// Cụm `real-economics-mode2` (F-27) — thống kê 1 NHÓM (`isolated` hoặc
/// `non_isolated`) riêng biệt: `n` (tổng dòng), `within_1pct` (số dòng
/// `lech_pct <= 1.0`), + p50/p95 CỦA CHÍNH `lech_pct` (không phải chỉ đếm nhị
/// phân đạt/không đạt — đúng lệnh "n, within_1pct, p50, p95 lệch"). Giữ tối
/// đa 200 mẫu gần nhất/nhóm để tính percentile (đủ ổn định, không phình vô
/// hạn qua thời gian chạy dài).
#[derive(Debug, Default)]
struct ValidateGroupStats {
    n: u64,
    within_1pct: u64,
    lech_pct_samples: std::collections::VecDeque<f64>,
}

const VALIDATE_GROUP_SAMPLE_CAP: usize = 200;
/// Cụm `truth-victim-ok-and-memleak` (mục 3) — trần `rows` của
/// `ValidateStats`/`CompeteStats`, tách thành hằng số có TÊN để `GET /api/mem`
/// báo cáo được đúng con số đang áp dụng (trước đó là số `50` viết thẳng
/// trong vòng `while`, không ai ngoài hàm đó biết).
pub const VALIDATE_ROWS_CAP: usize = 50;
pub const COMPETE_ROWS_CAP: usize = 50;

impl ValidateGroupStats {
    fn push(&mut self, lech_pct: f64) {
        self.n += 1;
        if lech_pct <= 1.0 {
            self.within_1pct += 1;
        }
        self.lech_pct_samples.push_back(lech_pct);
        while self.lech_pct_samples.len() > VALIDATE_GROUP_SAMPLE_CAP {
            self.lech_pct_samples.pop_front();
        }
    }

    /// Percentile THUẦN (nearest-rank trên mẫu ĐÃ SẮP XẾP) của `lech_pct` —
    /// `None` khi chưa có mẫu nào (tránh chia 0/hiện `0.0` giả).
    fn percentile(&self, p: f64) -> Option<f64> {
        if self.lech_pct_samples.is_empty() {
            return None;
        }
        let mut v: Vec<f64> = self.lech_pct_samples.iter().copied().collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = (((p / 100.0) * (v.len() - 1) as f64).round() as usize).min(v.len() - 1);
        Some(v[idx])
    }

    fn snapshot(&self) -> Value {
        json!({
            "n": self.n,
            "within_1pct": self.within_1pct,
            "within_1pct_ratio": if self.n > 0 { self.within_1pct as f64 / self.n as f64 } else { 0.0 },
            "p50_lech_pct": self.percentile(50.0),
            "p95_lech_pct": self.percentile(95.0),
        })
    }
}

/// F-27 — fix mâu thuẫn audit (`/api/validate` trả `within_1pct=2` trong khi
/// 12/18 dòng thật có `lech_pct<=1.0` — bộ đếm CŨ chỉ tính dòng `isolated:true`
/// vào `within_1pct` dù tên field không nói rõ điều đó). Giờ tách RÕ 2 nhóm
/// (`isolated`/`non_isolated`), MỖI nhóm có bộ đếm riêng đúng nghĩa — tổng
/// `total`/`within_1pct` ở gốc JSON là CỘNG DỒN cả 2 nhóm (đúng trực giác tên
/// field, không còn ngầm định chỉ tính `isolated`).
#[derive(Debug, Default)]
pub struct ValidateStats {
    rows: std::collections::VecDeque<Value>,
    isolated: ValidateGroupStats,
    non_isolated: ValidateGroupStats,
}

impl ValidateStats {
    /// `is_isolated`/`lech_pct` tách riêng khỏi `ok` (cũ) — caller
    /// (`main.rs::spawn_victim_validator`) đã tính cả 2 giá trị này trước khi
    /// gọi, `push` giờ tự phân nhóm và tự tính `within_1pct` ĐÚNG cho MỖI
    /// nhóm thay vì 1 cờ `ok` gộp sẵn dễ lẫn ý nghĩa (nguồn gốc bug F-27).
    /// Cụm `truth-victim-ok-and-memleak` (mục 3) — `(rows, mẫu isolated, mẫu
    /// non_isolated)` đang giữ, cho `GET /api/mem`.
    pub fn sizes(&self) -> (usize, usize, usize) {
        (self.rows.len(), self.isolated.lech_pct_samples.len(), self.non_isolated.lech_pct_samples.len())
    }

    pub fn push(&mut self, row: Value, is_isolated: bool, lech_pct: f64) {
        if is_isolated {
            self.isolated.push(lech_pct);
        } else {
            self.non_isolated.push(lech_pct);
        }
        self.rows.push_back(row);
        while self.rows.len() > VALIDATE_ROWS_CAP {
            self.rows.pop_front();
        }
    }

    pub fn snapshot(&self) -> Value {
        let total = self.isolated.n + self.non_isolated.n;
        let within_1pct = self.isolated.within_1pct + self.non_isolated.within_1pct;
        json!({
            "total": total,
            "within_1pct": within_1pct,
            "within_1pct_ratio": if total > 0 { within_1pct as f64 / total as f64 } else { 0.0 },
            "isolated": self.isolated.snapshot(),
            "non_isolated": self.non_isolated.snapshot(),
            "rows": self.rows.iter().cloned().collect::<Vec<_>>(),
        })
    }
}

async fn validate_list(State(state): State<AppState>) -> Json<Value> {
    Json(state.validate_log.read().await.snapshot())
}

async fn compete(State(state): State<AppState>) -> Json<Value> {
    let mut out = state.compete_stats.snapshot();
    // Cụm `decision-data-24h` (mục 2) — thêm bảng THEO POOL: trong số cơ hội
    // CÓ LÃI trên pool đó, bao nhiêu phần trăm victim chính là ví của cụm đối
    // thủ MEV (`victim_in_competitor_cluster`). Đây là con số quyết định "pool
    // này có đáng làm không": pool 100% victim-cụm nghĩa là đối thủ đang tự
    // swap token của họ, front-run nhóm đó là đối đầu trực diện với hệ thống
    // có hạ tầng bundle riêng, KHÔNG phải nạn nhân bình thường.
    //
    // Nguồn số liệu giống hệt `/api/econ` (cùng `compute_econ_from_rows`, cùng
    // bộ lọc `boot_wall_clock`) để 2 endpoint không bao giờ lệch nhau.
    let log_path = state.logger.path().to_path_buf();
    let content = read_log_tail(&log_path).await;
    let all_lines: Vec<&str> = content.lines().collect();
    let start = all_lines.len().saturating_sub(ECON_MAX_LINES);
    let rows: Vec<Value> = all_lines[start..].iter().filter_map(|l| serde_json::from_str::<Value>(l).ok()).collect();
    let boot_ts = state.boot_wall_clock.to_rfc3339();
    let econ = compute_econ_from_rows(&rows, Some(&boot_ts));
    out["by_pool"] = econ["top_pools_by_net"].clone();
    out["net_pos_total"] = econ["net_pos_total"].clone();
    out["net_pos_total_non_cluster"] = econ["net_pos_total_non_cluster"].clone();
    out["pct_net_pos_la_vi_cum"] = {
        let np = econ["net_pos_total"].as_u64().unwrap_or(0);
        let nc = econ["net_pos_total_non_cluster"].as_u64().unwrap_or(0);
        json!(if np > 0 { (np - nc) as f64 * 100.0 / np as f64 } else { 0.0 })
    };
    out["competitor"] = econ["competitor"].clone();
    Json(out)
}

/// Cụm `competitor-recon-and-strategy` (mục 4, shadow mode) — dashboard khối
/// "Live": `live_mode`, ví REDACT (không bao giờ khoá riêng), số bundle ĐÃ
/// KÝ (KHÔNG gửi) + số lần bị từ chối re-vet, xem CHI TIẾT các dòng gần
/// nhất. KHÔNG có "PnL thật" (shadow mode không broadcast nên không có kết
/// quả on-chain nào để tính lãi/lỗ thật — ghi rõ trong `note`, không bịa số).
async fn shadow_status(State(state): State<AppState>) -> Json<Value> {
    let cfg = state.config.read().await;
    let live_mode = cfg.live_mode.clone();
    drop(cfg);
    let shadow_self_address = state.shadow_wallet.as_ref().map(|(addr, _)| redact_address(&format!("{addr:#x}")));

    let log_path = state.logger.path().to_path_buf();
    let content = read_log_tail(&log_path).await;
    let all_lines: Vec<&str> = content.lines().collect();
    let start = all_lines.len().saturating_sub(ECON_MAX_LINES);
    let boot_ts = state.boot_wall_clock.to_rfc3339();

    let mut bundles: Vec<Value> = Vec::new();
    let mut aborts: Vec<Value> = Vec::new();
    // Cụm `bugfix-presign-and-contract-plan` (A4) — `tx.abort` giờ mang lý do
    // CỤ THỂ (`vet_stale`/`reserve_stale`/`victim_already_mined`/
    // `mined_index_cold`/`nonce_not_prefetched`/`sign_failed_*`) thay cho 1
    // chữ `pre_sign_revet_failed` chung. Lọc theo tên cũ sẽ trả 0 abort dù
    // thực tế có — đếm THEO TỪNG lý do.
    let mut abort_by_reason: HashMap<String, u64> = HashMap::new();
    for line in &all_lines[start..] {
        let Ok(row) = serde_json::from_str::<Value>(line) else { continue };
        if row["ts"].as_str().map(|t| t < boot_ts.as_str()).unwrap_or(true) {
            continue; // chi tinh dong CUA LAN CHAY HIEN TAI, cung luat /api/econ
        }
        match row["event"].as_str() {
            Some("bundle.shadow") => bundles.push(row),
            Some("tx.abort") => {
                if let Some(r) = row["reason"].as_str() {
                    *abort_by_reason.entry(r.to_string()).or_insert(0) += 1;
                }
                aborts.push(row);
            }
            _ => {}
        }
    }
    let bundle_count = bundles.len();
    let abort_count = aborts.len();
    // Tỉ lệ "ký kịp" — chính là điều kiện go/no-go #1 của
    // `docs/CONTRACT_DESIGN.md` B7 (≥ 50%).
    let sign_rate_pct = if bundle_count + abort_count > 0 {
        bundle_count as f64 / (bundle_count + abort_count) as f64 * 100.0
    } else {
        0.0
    };
    bundles.reverse();
    aborts.reverse();
    bundles.truncate(20);
    aborts.truncate(20);

    Json(json!({
        "live_mode": live_mode,
        "shadow_armed": shadow_self_address.is_some(),
        "shadow_self_address": shadow_self_address,
        "bundle_shadow_count": bundle_count,
        "abort_count": abort_count,
        "abort_by_reason": abort_by_reason,
        "sign_rate_pct": sign_rate_pct,
        "recent_bundles": bundles,
        "recent_aborts": aborts,
        "note": "shadow mode KHONG gui/broadcast - khong co PnL THAT (chua co ket qua on-chain nao de biet lai/lo that), day chi la hoat dong KY duoc",
    }))
}

/// Cụm A6 — `AtomicU64` (không `RwLock<HashMap>`) vì đây là hot path tăng
/// theo MỖI tx pending thật, tránh khoá ghi tranh chấp. Ý nghĩa từng field —
/// GHI RÕ vì đây KHÔNG phải "đếm tích luỹ đơn thuần cùng nghĩa" như cũ:
/// - `seen`/`not_pancake_router`: đếm MỌI tx quan sát được (bất kể có spawn
///   xử lý tiếp hay không) — xem `main.rs` 3 hàm nguồn tx.
/// - `decode_fail`/`not_wbnb_pair` (gộp CẢ `not_quote_pair`/`sell_direction`
///   — không tách riêng phiên này, cùng ý nghĩa "sai hướng/sai cặp quote"):
///   lý do skip TẬN CÙNG (terminal), đúng enum `PipelineSkip`.
/// - `venue_v3`: decode OK, đúng hướng MUA, nhưng hàm/command là V3
///   (`decoder::SwapVenue::V3`) — CHƯA pin quoter/sim ở TẦNG PIPELINE này
///   (`PipelineSkip::VenueUnpinned`), KHÔNG đưa vào `resolve_v2_reserves`.
/// - `venue_v2`: decode OK, đúng hướng MUA, venue V2 — MILESTONE (đã vào
///   nhánh `resolve_v2_reserves`), KHÔNG loại trừ lẫn với các bucket sau (1
///   tx V2 luôn cộng `venue_v2` VÀ đúng 1 trong số {no_pool, below_min,
///   thin_liq, honeypot_or_tax, unprofitable, victim_would_revert,
///   simulated} — khác các bucket khác vốn loại trừ lẫn nhau).
/// - `rpc_error`: cụm `hotpath-fix-then-decoder-ur` (A3) — trước đó DÀNH SẴN,
///   LUÔN `0` (`pipeline::resolve_v2_reserves` gộp "không có pool" VÀ
///   "eth_call lỗi mạng" thành CÙNG 1 `PipelineSkip::NoPool`, quyết định có
///   chủ đích từ `5.1`). Từ cụm này, `resolve_v2_reserves`/
///   `resolve_reserves_for_quote`/`resolve_v2_reserves_known_pair` tách RIÊNG
///   `PipelineSkip::RpcError` (timeout/lỗi mạng) khỏi `NoPool` (Factory trả
///   `address(0)`, chắc chắn không pool) — field này giờ có số THẬT.
/// - `below_min`/`thin_liq`/`honeypot_or_tax`/`unprofitable`/`victim_would_revert`:
///   terminal, khớp `PipelineSkip` cùng tên (áp dụng chung cho CẢ nhánh V2
///   wallet/pair/universal LẪN nhánh USDT fallback — 2 nhánh dùng chung ý
///   nghĩa reason, chỉ khác nguồn resolve pool).
/// - `simulated`: `PipelineOutcome::Simulated` (CẢ 2 nhánh).
#[derive(Debug, Default)]
pub struct FunnelCounters {
    seen: AtomicU64,
    not_pancake_router: AtomicU64,
    decode_fail: AtomicU64,
    not_wbnb_pair: AtomicU64,
    venue_v3: AtomicU64,
    venue_v2: AtomicU64,
    no_pool: AtomicU64,
    rpc_error: AtomicU64,
    below_min: AtomicU64,
    thin_liq: AtomicU64,
    honeypot_or_tax: AtomicU64,
    unprofitable: AtomicU64,
    victim_would_revert: AtomicU64,
    simulated: AtomicU64,
    /// Cụm `evm-validate-fixed-then-wire` (B3.2) — đếm riêng số tx bị bỏ vì
    /// `sim_engine="evm"` mà revm/RPC không chạy được. Tách khỏi `rpc_error`
    /// (field đó vẫn LUÔN 0, xem BAOCAO31 A6) để Chủ phân biệt được "RPC
    /// không kham nổi tải EVM" với các lý do skip kinh tế.
    sim_error: AtomicU64,
    /// Cụm `exec-path-traps` (F-14) — `PipelineSkip::Deadline` (reason đã
    /// khai báo từ trước trong `venues::SKIP_REASONS` nhưng CHƯA từng phát
    /// sinh, xem audit F-14).
    deadline: AtomicU64,
    /// Cụm `exec-path-traps` (F-13) — `PipelineSkip::NonceStale`.
    nonce_stale: AtomicU64,
    /// Cụm `exec-path-traps` (F-13) — `PipelineSkip::NonceFuture`.
    nonce_future: AtomicU64,
    /// Cụm `real-economics-mode2` (F-03) — `PipelineSkip::GasCap`.
    gas_cap: AtomicU64,
    /// Cụm `bugfix-presign-and-contract-plan` (A2) — `PipelineSkip::SanityReject`
    /// (cổng tỉnh táo trước `Simulated`, xem `pipeline::sanity_check`).
    sanity_reject: AtomicU64,
    /// Cụm `bugfix-presign-and-contract-plan` (A3) — candidate bị chặn vì
    /// `from` (hoặc ví vừa nhận quote từ) thuộc CỤM ĐỐI THỦ đã nhận diện
    /// (`allow_competitor_victims=false` + `live_mode != "off"`).
    competitor_victim: AtomicU64,
}

impl FunnelCounters {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_seen(&self) {
        self.seen.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_not_pancake_router(&self) {
        self.not_pancake_router.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_decode_fail(&self) {
        self.decode_fail.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_not_wbnb_pair(&self) {
        self.not_wbnb_pair.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_venue_v3(&self) {
        self.venue_v3.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_venue_v2(&self) {
        self.venue_v2.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_no_pool(&self) {
        self.no_pool.fetch_add(1, Ordering::Relaxed);
    }
    /// Cụm `hotpath-fix-then-decoder-ur` (A3) — field đã DÀNH SẴN từ
    /// `foundation-fix-then-real-sim` (luôn 0 tới giờ), lần đầu có số thật khi
    /// `PipelineSkip::RpcError` phát sinh.
    pub fn record_rpc_error(&self) {
        self.rpc_error.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_below_min(&self) {
        self.below_min.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_thin_liq(&self) {
        self.thin_liq.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_honeypot_or_tax(&self) {
        self.honeypot_or_tax.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_unprofitable(&self) {
        self.unprofitable.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_victim_would_revert(&self) {
        self.victim_would_revert.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_simulated(&self) {
        self.simulated.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_sim_error(&self) {
        self.sim_error.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_deadline(&self) {
        self.deadline.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_nonce_stale(&self) {
        self.nonce_stale.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_nonce_future(&self) {
        self.nonce_future.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_gas_cap(&self) {
        self.gas_cap.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_sanity_reject(&self) {
        self.sanity_reject.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_competitor_victim(&self) {
        self.competitor_victim.fetch_add(1, Ordering::Relaxed);
    }

    /// Đọc snapshot HIỆN TẠI (không reset) — dùng cho `GET /api/funnel`.
    pub fn snapshot(&self) -> Value {
        json!({
            "seen": self.seen.load(Ordering::Relaxed),
            "not_pancake_router": self.not_pancake_router.load(Ordering::Relaxed),
            "decode_fail": self.decode_fail.load(Ordering::Relaxed),
            "not_wbnb_pair": self.not_wbnb_pair.load(Ordering::Relaxed),
            "venue_v3": self.venue_v3.load(Ordering::Relaxed),
            "venue_v2": self.venue_v2.load(Ordering::Relaxed),
            "no_pool": self.no_pool.load(Ordering::Relaxed),
            "rpc_error": self.rpc_error.load(Ordering::Relaxed),
            "below_min": self.below_min.load(Ordering::Relaxed),
            "thin_liq": self.thin_liq.load(Ordering::Relaxed),
            "honeypot_or_tax": self.honeypot_or_tax.load(Ordering::Relaxed),
            "unprofitable": self.unprofitable.load(Ordering::Relaxed),
            "victim_would_revert": self.victim_would_revert.load(Ordering::Relaxed),
            "simulated": self.simulated.load(Ordering::Relaxed),
            "sim_error": self.sim_error.load(Ordering::Relaxed),
            "deadline": self.deadline.load(Ordering::Relaxed),
            "nonce_stale": self.nonce_stale.load(Ordering::Relaxed),
            "nonce_future": self.nonce_future.load(Ordering::Relaxed),
            "gas_cap": self.gas_cap.load(Ordering::Relaxed),
            "sanity_reject": self.sanity_reject.load(Ordering::Relaxed),
            "competitor_victim": self.competitor_victim.load(Ordering::Relaxed),
        })
    }

    /// Đọc VÀ reset về 0 — dùng cho log `funnel.minute` mỗi 60s (`main.rs`),
    /// "cộng dồn TRONG phút đó", không phải tổng tích luỹ từ lúc boot.
    pub fn snapshot_and_reset(&self) -> Value {
        let out = json!({
            "seen": self.seen.swap(0, Ordering::Relaxed),
            "not_pancake_router": self.not_pancake_router.swap(0, Ordering::Relaxed),
            "decode_fail": self.decode_fail.swap(0, Ordering::Relaxed),
            "not_wbnb_pair": self.not_wbnb_pair.swap(0, Ordering::Relaxed),
            "venue_v3": self.venue_v3.swap(0, Ordering::Relaxed),
            "venue_v2": self.venue_v2.swap(0, Ordering::Relaxed),
            "no_pool": self.no_pool.swap(0, Ordering::Relaxed),
            "rpc_error": self.rpc_error.swap(0, Ordering::Relaxed),
            "below_min": self.below_min.swap(0, Ordering::Relaxed),
            "thin_liq": self.thin_liq.swap(0, Ordering::Relaxed),
            "honeypot_or_tax": self.honeypot_or_tax.swap(0, Ordering::Relaxed),
            "unprofitable": self.unprofitable.swap(0, Ordering::Relaxed),
            "victim_would_revert": self.victim_would_revert.swap(0, Ordering::Relaxed),
            "simulated": self.simulated.swap(0, Ordering::Relaxed),
            "sim_error": self.sim_error.swap(0, Ordering::Relaxed),
            "deadline": self.deadline.swap(0, Ordering::Relaxed),
            "nonce_stale": self.nonce_stale.swap(0, Ordering::Relaxed),
            "nonce_future": self.nonce_future.swap(0, Ordering::Relaxed),
            "gas_cap": self.gas_cap.swap(0, Ordering::Relaxed),
            "sanity_reject": self.sanity_reject.swap(0, Ordering::Relaxed),
            "competitor_victim": self.competitor_victim.swap(0, Ordering::Relaxed),
        });
        out
    }
}

/// Cụm `econ-truth-latency-vps` (mục 2) — thống kê `compete.check` (task nền
/// `main.rs::spawn_post_simulated_tracker`): với MỖI candidate `Simulated`,
/// chờ victim lên block rồi soi tx NGAY TRƯỚC/SAU trong CÙNG block xem có
/// chạm cùng pool không (dấu hiệu có bot khác cũng đang giao dịch pool đó
/// quanh thời điểm victim, khả năng cạnh tranh) — kiểm tra Ở MỨC 1 vị trí kề
/// (không quét toàn block), ghi rõ giới hạn này (không phải "không có bot
/// cạnh tranh nào khác trong block", chỉ là "không thấy ở vị trí LIỀN KỀ").
/// Cụm `truth-victim-ok-and-memleak` (mục 3) — trần THẬT cho 2 container của
/// `CompeteStats` (trước cụm này chỉ có trong doc-comment, không có trong code).
const TOP_BOTS_CAP: usize = 20;
const COMPETITOR_GAS_SAMPLE_CAP: usize = 500;

#[derive(Debug, Default)]
pub struct CompeteStats {
    checked: AtomicU64,
    possible_competitor: AtomicU64,
    /// Địa chỉ bot nghi cạnh tranh + số lần thấy (giữ tối đa 20 địa chỉ khác
    /// nhau gần nhất, đủ cho "top bot" hiển thị dashboard).
    top_bots: std::sync::Mutex<HashMap<String, u64>>,
    /// `gwei` của bot cạnh tranh mỗi lần thấy — dùng tính median hiển thị.
    competitor_gas_gwei_samples: std::sync::Mutex<Vec<f64>>,
    rows: std::sync::Mutex<std::collections::VecDeque<Value>>,
}

impl CompeteStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cụm `truth-victim-ok-and-memleak` (mục 3) — `(rows, top_bots, mẫu gas)`
    /// đang giữ, cho `GET /api/mem`.
    pub fn sizes(&self) -> (usize, usize, usize) {
        (
            self.rows.lock().map(|r| r.len()).unwrap_or(0),
            self.top_bots.lock().map(|b| b.len()).unwrap_or(0),
            self.competitor_gas_gwei_samples.lock().map(|v| v.len()).unwrap_or(0),
        )
    }

    pub fn record(&self, row: Value, competitor_addr: Option<&str>, competitor_gas_gwei: Option<f64>) {
        self.checked.fetch_add(1, Ordering::Relaxed);
        if let Some(addr) = competitor_addr {
            self.possible_competitor.fetch_add(1, Ordering::Relaxed);
            let mut bots = self.top_bots.lock().unwrap();
            *bots.entry(addr.to_string()).or_insert(0) += 1;
            // Cum `truth-victim-ok-and-memleak` (muc 3): doc-comment cu noi
            // "giu toi da 20 dia chi" nhung KHONG he co buoc cat -> map nay
            // lon dan theo so dia chi bot KHAC NHAU gap trong ca doi bot.
            // Cat that: giu 20 dia chi DEM CAO NHAT (con so hien thi la
            // "top bot", nen cat theo count la dung y nghia).
            if bots.len() > TOP_BOTS_CAP {
                let mut v: Vec<(String, u64)> = bots.drain().collect();
                v.sort_by(|a, b| b.1.cmp(&a.1));
                v.truncate(TOP_BOTS_CAP);
                *bots = v.into_iter().collect();
            }
            if let Some(g) = competitor_gas_gwei {
                // Vec nay truoc day push VO HAN (chi dung de tinh trung binh).
                let mut samples = self.competitor_gas_gwei_samples.lock().unwrap();
                samples.push(g);
                if samples.len() > COMPETITOR_GAS_SAMPLE_CAP {
                    let extra = samples.len() - COMPETITOR_GAS_SAMPLE_CAP;
                    samples.drain(0..extra);
                }
            }
        }
        let mut rows = self.rows.lock().unwrap();
        rows.push_back(row);
        while rows.len() > COMPETE_ROWS_CAP {
            rows.pop_front();
        }
    }

    pub fn snapshot(&self) -> Value {
        let checked = self.checked.load(Ordering::Relaxed);
        let possible = self.possible_competitor.load(Ordering::Relaxed);
        let bots = self.top_bots.lock().unwrap();
        let mut top: Vec<(String, u64)> = bots.iter().map(|(k, v)| (k.clone(), *v)).collect();
        top.sort_by(|a, b| b.1.cmp(&a.1));
        top.truncate(10);
        let mut samples = self.competitor_gas_gwei_samples.lock().unwrap().clone();
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let avg_gas = if samples.is_empty() { None } else { Some(samples.iter().sum::<f64>() / samples.len() as f64) };
        json!({
            "checked": checked,
            "possible_competitor": possible,
            "possible_competitor_pct": if checked > 0 { possible as f64 / checked as f64 * 100.0 } else { 0.0 },
            "top_bots": top.into_iter().map(|(a, c)| json!({"address": a, "count": c})).collect::<Vec<_>>(),
            "avg_competitor_gas_gwei": avg_gas,
            "rows": self.rows.lock().unwrap().iter().cloned().collect::<Vec<_>>(),
        })
    }
}

/// Cụm `truth-victim-ok-and-memleak` (mục 3) — LIỆT KÊ MỌI container sống lâu
/// của bot kèm kích thước hiện tại và TRẦN của nó, ở MỘT chỗ duy nhất.
///
/// Hàm này là nguồn sự thật dùng chung cho cả `GET /api/mem` và task log
/// `mem.rss_mb` mỗi phút (`main.rs::mem_watch_task`) — cố ý KHÔNG viết 2 bản,
/// vì hai bản sẽ lệch nhau ngay lần thêm container tiếp theo và khi đó số
/// trong log lại không khớp số trên dashboard.
///
/// `cap = None` nghĩa là container đó **chưa có trần** và phải bị coi là nghi
/// phạm rò rỉ cho tới khi có trần — không được đọc thành "an toàn".
pub fn container_sizes(state: &AppState) -> Vec<crate::mem::ContainerSize> {
    use crate::mem::ContainerSize as C;
    // MỌI lần đọc dùng `try_read()` — xem doc-comment `ContainerSize::len` để
    // biết vì sao (task đo bộ nhớ KHÔNG được phép xếp hàng sau khoá ghi của
    // `pairs_vet_task`, vòng vet giữ khoá 3–4 phút).
    let (val_rows, val_iso, val_non) = match state.validate_log.try_read() {
        Ok(g) => {
            let (a, b, c) = g.sizes();
            (Some(a), Some(b), Some(c))
        }
        Err(_) => (None, None, None),
    };
    let (cmp_rows, cmp_bots, cmp_gas) = {
        let (a, b, c) = state.compete_stats.sizes();
        (Some(a), Some(b), Some(c))
    };
    vec![
        C {
            name: "transport::ReserveCache.entries",
            len: state.reserve_cache.try_read().ok().map(|g| g.len()),
            cap: Some(crate::transport::RESERVE_CACHE_BLOCKS),
            cap_unit: "block",
        },
        C {
            name: "transport::NonceCache.entries",
            len: state.nonce_cache.try_read().ok().map(|g| g.len()),
            cap: Some(crate::transport::NONCE_CACHE_BLOCKS),
            cap_unit: "block",
        },
        C {
            name: "transport::SeenHashSet",
            len: state.seen_hashes.try_read().ok().map(|g| g.len()),
            cap: Some(crate::transport::SEEN_HASH_CAP),
            cap_unit: "hash",
        },
        C {
            name: "transport::MinedTxIndex",
            len: state.mined_index.try_read().ok().map(|g| g.len()),
            cap: Some(crate::transport::MINED_INDEX_DEPTH),
            cap_unit: "block",
        },
        C {
            name: "competitor::ClusterIndex.funded",
            len: state.competitor_cluster.try_read().ok().map(|g| g.funded_now()),
            cap: Some(crate::competitor::FUNDED_WINDOW_BLOCKS),
            cap_unit: "block",
        },
        C { name: "web::ValidateStats.rows", len: val_rows, cap: Some(VALIDATE_ROWS_CAP), cap_unit: "dong" },
        C {
            name: "web::ValidateStats.isolated.samples",
            len: val_iso,
            cap: Some(VALIDATE_GROUP_SAMPLE_CAP),
            cap_unit: "mau",
        },
        C {
            name: "web::ValidateStats.non_isolated.samples",
            len: val_non,
            cap: Some(VALIDATE_GROUP_SAMPLE_CAP),
            cap_unit: "mau",
        },
        C { name: "web::CompeteStats.rows", len: cmp_rows, cap: Some(COMPETE_ROWS_CAP), cap_unit: "dong" },
        C { name: "web::CompeteStats.top_bots", len: cmp_bots, cap: Some(TOP_BOTS_CAP), cap_unit: "dia chi" },
        C {
            name: "web::CompeteStats.gas_samples",
            len: cmp_gas,
            cap: Some(COMPETITOR_GAS_SAMPLE_CAP),
            cap_unit: "mau",
        },
        C {
            name: "web::skip_counts",
            len: state.skip_counts.try_read().ok().map(|g| g.len()),
            cap: None,
            cap_unit: "enum skip (chan boi so enum, khong phai thoi gian)",
        },
        C {
            name: "main::candidate_seen",
            len: state.candidate_seen.try_read().ok().map(|g| g.len()),
            cap: None,
            cap_unit: "pool (chan boi pairs.txt o mode 2)",
        },
        C {
            name: "tax::TaxCache",
            len: state.tax_cache.try_read().ok().map(|g| g.len()),
            cap: None,
            cap_unit: "(token,quote)",
        },
        C {
            name: "pairbook::PairBook",
            len: state.pairbook.try_read().ok().map(|g| g.len()),
            cap: None,
            cap_unit: "dong pairs.txt",
        },
        C {
            name: "victims::VictimBook",
            len: state.victims.try_read().ok().map(|g| g.len()),
            cap: None,
            cap_unit: "dong victims.txt",
        },
    ]
}

/// `GET /api/mem` — cụm `truth-victim-ok-and-memleak` (mục 3). Trả `VmRSS`/
/// `VmHWM` THẬT của tiến trình + bảng container ở trên. `rss_mb = null` nghĩa
/// là không đọc được `/proc/self/status` (không phải Linux) — KHÔNG trả 0 giả.
async fn mem_status(State(state): State<AppState>) -> Json<Value> {
    let containers = container_sizes(&state);
    Json(json!({
        "rss_mb": crate::mem::rss_mb(),
        "rss_peak_mb": crate::mem::rss_peak_mb(),
        "uptime_sec": state.start_time.elapsed().as_secs(),
        "containers": containers
            .iter()
            .map(|c| json!({"name": c.name, "len": c.len, "cap": c.cap, "cap_unit": c.cap_unit}))
            .collect::<Vec<_>>(),
        "khong_co_tran": containers.iter().filter(|c| c.cap.is_none()).map(|c| c.name).collect::<Vec<_>>(),
    }))
}

pub type AppState = Arc<AppStateInner>;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/status", get(status))
        .route("/api/victims", get(victims))
        .route("/api/pairs", get(pairs))
        .route("/api/venues", get(venues))
        .route("/api/hits", get(hits))
        .route("/api/skips", get(skips))
        .route("/api/funnel", get(funnel))
        .route("/api/econ", get(econ))
        .route("/api/validate", get(validate_list))
        .route("/api/compete", get(compete))
        .route("/api/mem", get(mem_status))
        .route("/api/shadow", get(shadow_status))
        .route("/api/tax", get(tax_cache_list).post(tax_inject))
        .route("/api/control", post(control))
        .with_state(state)
        .fallback_service(ServeDir::new("web"))
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn status(State(state): State<AppState>) -> Json<Value> {
    let cfg = state.config.read().await;
    let halted = state.state_files.is_halted();
    let bot_state = *state.bot_state.read().await;
    let last_block = *state.last_block.read().await;

    let gate = json!({
        "allow_live": cfg.allow_live,
        "not_dry_run": !cfg.dry_run,
        "bot_armed": cfg.bot_armed,
        "not_halted": !halted,
        "chain_id_56": cfg.chain_id == 56,
    });

    // Cum `5.2` — nguon pending-tx dang hoat dong + 3 nguong chu chinh trong
    // config.toml (doc qua Config lock, khong hardcode), phuc vu dashboard
    // "Sim cuoi"/"Bot" xem du thong tin.
    let pending_source = state.pending_source.read().await.as_str();

    // Cum `exec-path-traps` (F-04) — RiskGuard con SONG (record_result gio
    // co call site that, xem main.rs::spawn_victim_validator) - phoi ra
    // /api/status de Chu/Grok thay dem lo lien tiep + co bi vuot nguong
    // max_consecutive_loss hay khong, khong phai doan qua log.
    let risk_guard = state.risk_guard.read().await;
    let consecutive_loss = risk_guard.consecutive_loss();
    let risk_guard_json = json!({
        "consecutive_loss": consecutive_loss,
        "max_consecutive_loss": cfg.max_consecutive_loss,
        "exceeded": risk_guard.consecutive_loss_exceeded(cfg.max_consecutive_loss),
    });
    drop(risk_guard);

    // Cụm `competitor-recon-and-strategy` (mục 4, shadow mode) — CHỈ phơi
    // địa chỉ ví đã REDACT (không bao giờ khoá riêng) + có nạp được signer
    // hay không. `live_mode="off"` (ship) -> `shadow_self_address=None`,
    // `shadow_armed=false`, không đổi gì so trước cụm này.
    let shadow_self_address = state.shadow_wallet.as_ref().map(|(addr, _)| redact_address(&format!("{addr:#x}")));

    Json(json!({
        "state": bot_state.as_str(),
        "uptime_sec": state.start_time.elapsed().as_secs(),
        "last_block": last_block,
        "chain_id": cfg.chain_id,
        "dry_run": cfg.dry_run,
        "allow_live": cfg.allow_live,
        "bot_armed": cfg.bot_armed,
        "halt_lock": halted,
        "live_gate": gate,
        "pending_source": pending_source,
        "max_front_bnb": cfg.max_front_bnb,
        "max_exposure_bnb": cfg.max_exposure_bnb,
        "min_profit_bnb": cfg.min_profit_bnb,
        "risk_guard": risk_guard_json,
        "live_mode": cfg.live_mode,
        "shadow_armed": state.shadow_wallet.is_some(),
        "shadow_self_address": shadow_self_address,
        "bribe_pct_of_profit": cfg.bribe_pct_of_profit,
        "bribe_mode": cfg.bribe_mode,
    }))
}

fn redact_address(addr: &str) -> String {
    if addr.len() <= 10 {
        return addr.to_string();
    }
    format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
}

async fn victims(State(state): State<AppState>) -> Json<Value> {
    let book = state.victims.read().await;
    let entries: Vec<Value> = book
        .entries()
        .map(|(addr, wei)| {
            json!({
                "address": redact_address(addr),
                "min_swap_bnb": (*wei as f64) / 1e18,
            })
        })
        .collect();
    Json(json!({
        "count": book.len(),
        "error_lines": book.error_lines,
        "last_reload_sec_ago": book.last_reload.map(|t| t.elapsed().as_secs()),
        "victims": entries,
    }))
}

/// Cụm pair-mode — `GET /api/pairs`: đủ `count`/`error_lines`/
/// `last_reload_sec_ago`/danh sách entry (`pair_addr`/`source_line`/
/// `resolved_from`) đúng schema CLAUDE.md lệnh pair-mode. Chỉ đọc, không sửa
/// (sửa qua file `pairs.txt`, hot-reload theo `pairs_reload_sec`).
///
/// Cụm `strategy-lock-mode2` — thêm 5 cột: `vetted_at` (ngày Chủ vet tay,
/// `null` = chưa vet), `candidate` (`PairBook::contains` thật — `false` khi
/// bị vet nền loại, xem `vet_failed`), `buy_bps`/`sell_bps`/`honeypot`/
/// `last_vet_sec_ago` (kết quả `pairs_vet_task` gần nhất, `null` khi chưa
/// từng vet nền lần nào).
async fn pairs(State(state): State<AppState>) -> Json<Value> {
    let book = state.pairbook.read().await;
    let entries: Vec<Value> = book
        .entries()
        .map(|e| {
            let vet = book.vet_result(&e.pair_addr);
            json!({
                "pair_addr": format!("{:#x}", e.pair_addr),
                "source_line": e.source_line,
                "symbol": e.symbol,
                "resolved_from": e.resolved_from.as_str(),
                "vetted_at": e.vetted_at.map(|d| d.to_string()),
                "candidate": book.contains(&e.pair_addr),
                "buy_bps": vet.map(|(r, _)| r.buy_bps),
                "sell_bps": vet.map(|(r, _)| r.sell_bps),
                "honeypot": vet.map(|(r, _)| r.honeypot),
                "last_vet_sec_ago": vet.map(|(_, secs)| secs),
            })
        })
        .collect();
    // Cum `econ-truth-latency-vps` (0.a) — dòng ĐANG chờ resolve (RPC lỗi/
    // "no pool" tạm thời, retry backoff 5s/15s/60s) — KHÔNG rớt khỏi
    // `pairs.txt`/candidate list nếu đã từng resolve trước đó (chỉ dòng MỚI/
    // chưa từng resolve mới xuất hiện ở đây).
    let pending: Vec<Value> = book
        .pending_entries()
        .into_iter()
        .map(|(token_or_addr, quote, attempts, last_error, last_attempt_sec_ago)| {
            json!({
                "token": format!("{:#x}", token_or_addr),
                "quote": format!("{:#x}", quote),
                "attempts": attempts,
                "last_error": last_error,
                "last_attempt_sec_ago": last_attempt_sec_ago,
            })
        })
        .collect();
    Json(json!({
        "count": book.len(),
        "error_lines": book.error_lines,
        "pending_count": pending.len(),
        "pending": pending,
        "last_reload_sec_ago": book.last_reload_sec_ago(),
        "pairs": entries,
    }))
}

async fn venues(State(state): State<AppState>) -> Json<Value> {
    let cfg = state.config.read().await;
    let v = registry_snapshot(
        cfg.scan_v2,
        cfg.scan_v3,
        cfg.scan_v4,
        cfg.live_v2,
        cfg.live_v3,
        cfg.live_v4,
    );
    Json(json!({ "venues": v }))
}

#[derive(Deserialize)]
struct HitsQuery {
    limit: Option<usize>,
}

async fn hits(State(state): State<AppState>, Query(q): Query<HitsQuery>) -> Json<Value> {
    let limit = q.limit.unwrap_or(50).min(500);
    let lines = state.logger.tail(limit);
    Json(json!({ "hits": lines }))
}

async fn skips(State(state): State<AppState>) -> Json<Value> {
    let counts = state.skip_counts.read().await;
    let mut out = serde_json::Map::new();
    for reason in SKIP_REASONS {
        out.insert(
            (*reason).to_string(),
            json!(counts.get(*reason).copied().unwrap_or(0)),
        );
    }
    Json(Value::Object(out))
}

/// Cụm A6 — `GET /api/funnel`: snapshot HIỆN TẠI (không reset, không ảnh
/// hưởng tới log `funnel.minute` mỗi 60s ở `main.rs` — 2 nguồn đọc độc lập
/// trên CÙNG bộ đếm atomic).
async fn funnel(State(state): State<AppState>) -> Json<Value> {
    Json(state.funnel.snapshot())
}

// ============================================================================
// Cụm `real-economics-mode2` (mục 3) — GET /api/econ: đọc trực tiếp
// `logs/bot.jsonl` (dòng `tx.skip`/`sim.result`, dùng field mới ở mục 2:
// `amount_in`/`quote`/`gas_cost_wei`/`profit_gross_wei`/`profit_net_wei`/
// `seen_to_decision_ms`) rồi tổng hợp bucket/latency/top token — KHÔNG cần
// state riêng trong `AppStateInner` (đọc file trực tiếp mỗi lần gọi, đơn giản
// hơn và luôn phản ánh log THẬT, chấp nhận chi phí đọc file mỗi request —
// dashboard/paper_run.sh gọi endpoint này không thường xuyên).
// ============================================================================

/// Trần số dòng CUỐI đọc từ `logs/bot.jsonl` — tránh phình bộ nhớ với log
/// chạy rất lâu (VPS nhiều ngày); đủ dư cho 1 lần `paper_run.sh` (60 phút).
/// Cụm `truth-victim-ok-and-memleak` (mục 3) — TRẦN BYTE khi đọc
/// `logs/bot.jsonl` trong các handler HTTP.
///
/// # Vì sao cần
///
/// 3 handler (`/api/econ`, `/api/compete`, `/api/shadow`) gọi
/// `tokio::fs::read_to_string` trên TOÀN BỘ `bot.jsonl` rồi mới cắt
/// `ECON_MAX_LINES` dòng cuối. Trên VPS file đó đã lên **214 MB** trước khi
/// bot bị OOM-kill ⇒ mỗi lời gọi API cấp phát 214 MB (cộng thêm `Vec<&str>`
/// và `Vec<Value>` parse ra). Bộ nhớ đó được giải phóng về allocator nhưng
/// glibc KHÔNG trả lại cho hệ điều hành các arena lớn đã dùng, nên `VmRSS`
/// chỉ có lên chứ không xuống — dashboard tự refresh là đủ để đẩy RSS lên
/// hàng GB mà không có "container" nào phình ra cả. Đây là lý do vì sao chỉ
/// nhìn các map/vec sống lâu thì không giải thích hết 7,6 GB.
///
/// 256 MiB là dư cho `ECON_MAX_LINES` dòng ở mọi kích thước dòng thực tế;
/// quan trọng là nó là TRẦN CỐ ĐỊNH, không tăng theo tuổi của file log.
const LOG_TAIL_MAX_BYTES: u64 = 256 * 1024 * 1024;

/// Đọc **phần đuôi** của file log (tối đa `LOG_TAIL_MAX_BYTES`) thay vì cả
/// file. Cắt bỏ dòng đầu tiên khi đã bỏ qua phần đầu file, vì lát cắt theo
/// byte gần như chắc chắn rơi vào GIỮA một dòng JSON và dòng cụt đó sẽ parse
/// lỗi (im lặng) — cắt hẳn cho sạch.
async fn read_log_tail(path: &std::path::Path) -> String {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    let Ok(mut f) = tokio::fs::File::open(path).await else { return String::new() };
    let len = match f.metadata().await {
        Ok(m) => m.len(),
        Err(_) => return String::new(),
    };
    let truncated = len > LOG_TAIL_MAX_BYTES;
    if truncated && f.seek(std::io::SeekFrom::End(-(LOG_TAIL_MAX_BYTES as i64))).await.is_err() {
        return String::new();
    }
    let mut buf = Vec::with_capacity(len.min(LOG_TAIL_MAX_BYTES) as usize);
    if f.read_to_end(&mut buf).await.is_err() {
        return String::new();
    }
    let content = String::from_utf8_lossy(&buf).into_owned();
    if truncated {
        match content.find('\n') {
            Some(i) => content[i + 1..].to_string(),
            None => String::new(),
        }
    } else {
        content
    }
}

const ECON_MAX_LINES: usize = 2_000_000;

/// 5 bucket `victim_in` (BNB) đúng CLAUDE.md mục 3.a.
const BNB_BUCKETS: [(&str, f64, f64); 5] = [
    ("<0.01", 0.0, 0.01),
    ("0.01-0.05", 0.01, 0.05),
    ("0.05-0.2", 0.05, 0.2),
    ("0.2-1", 0.2, 1.0),
    (">=1", 1.0, f64::INFINITY),
];

/// 5 router đã pin, tên hiển thị khớp CLAUDE.md mục 3.c ("V2 Router /
/// SmartRouter / UR v3 / UR Infinity / SwapRouter") — dùng ĐÚNG địa chỉ đã
/// pin trong `venues::PANCAKE_ROUTERS`, không lặp lại hằng số riêng.
fn router_display_name(to: &str) -> &'static str {
    let to_lower = to.to_lowercase();
    for (addr, venue) in crate::venues::PANCAKE_ROUTERS.iter() {
        if addr.to_lowercase() == to_lower {
            return match *venue {
                crate::venues::Venue::V2 => "V2 Router",
                crate::venues::Venue::V3 => "SwapRouter",
                crate::venues::Venue::SmartRouter => "SmartRouter",
                // 2 dia chi UR (v3-cu / Infinity) deu map Venue::UniversalRouter -
                // phan biet lai bang dia chi THAT de dat dung 2 nhan rieng.
                crate::venues::Venue::UniversalRouter => {
                    if addr.eq_ignore_ascii_case("0x1A0A18AC4BECDDbd6389559687d1A73d8927E416") {
                        "UR v3 (cu)"
                    } else {
                        "UR Infinity"
                    }
                }
            };
        }
    }
    "other"
}

/// Cụm `econ-truth-latency-vps` (mục 1) — `profit_wei`/`profit_gross_wei`/
/// `profit_net_wei` giờ ghi dạng `String` (fix bug `serde_json::json!` panic
/// với `i128` vượt `i64::MAX`, xem `pipeline::log_outcome_v2`) — đọc lại qua
/// `as_str().parse()`. Fallback `as_i64()` giữ khả năng đọc được dòng log CŨ
/// (trước fix, chỉ tồn tại cho profit NHỎ hơn `i64::MAX` — dòng lớn hơn
/// trước đây chưa từng được ghi thành công nên không cần lo tương thích).
fn parse_profit_wei(v: &Value) -> Option<i128> {
    v.as_str().and_then(|s| s.parse::<i128>().ok()).or_else(|| v.as_i64().map(|n| n as i128))
}

fn parse_wei_str_to_bnb(s: &str) -> Option<f64> {
    s.parse::<f64>().ok().map(|w| w / 1e18)
}

fn percentile_f64(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = (((p / 100.0) * (v.len() - 1) as f64).round() as usize).min(v.len() - 1);
    Some(v[idx])
}

/// Cụm `econ-truth-latency-vps` (mục 1) — tích luỹ theo POOL (`pair`
/// address), thay `by_token`/`top_tokens` cũ (đếm theo TOKEN, không phân
/// biệt được 2 pool cùng token khác quote asset).
#[derive(Default)]
struct PoolAcc {
    count: u64,
    net_pos: u64,
    sum_net_bnb: f64,
    /// Cụm `bugfix-presign-and-contract-plan` (A6) — pool này có bị CỤM ĐỐI
    /// THỦ chạm tới không: `true` khi có ít nhất 1 dòng trên pool có
    /// `victim_in_competitor_cluster=true` (victim chính là ví của cụm), HOẶC
    /// 1 dòng `compete.result` trên pool đó tìm thấy tx liền kề của địa chỉ
    /// khác chạm ĐÚNG pool (`competitor` khác null).
    competitor_touched: bool,
    /// Cụm `decision-data-24h` (mục 2) — `net_pos` SAU KHI LOẠI victim thuộc
    /// cụm đối thủ. Đo 10.92 h thật trên VPS cho thấy 481/497 cơ hội có lãi
    /// (96.8%) là ví burner của chính cụm đối thủ đang tự swap token của họ —
    /// `net_pos` trần trụi vì vậy KHÔNG dùng để kết luận kinh tế được; mọi nơi
    /// hiện `net_pos` phải hiện kèm con số đã loại cụm này.
    net_pos_non_cluster: u64,
    sum_net_bnb_non_cluster: f64,
}

#[derive(Default)]
struct BucketAcc {
    count: u64,
    gross_pos: u64,
    net_pos: u64,
    /// Cụm `decision-data-24h` (mục 2) — xem `PoolAcc::net_pos_non_cluster`.
    net_pos_non_cluster: u64,
    sum_net_pos_bnb: f64,
    sum_net_pos_bnb_non_cluster: f64,
    best_net_bnb: Option<f64>,
    gas_cost_bnb_samples: Vec<f64>,
}

impl BucketAcc {
    fn snapshot(&self, label: &str) -> Value {
        let mut gas_samples = self.gas_cost_bnb_samples.clone();
        gas_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median_gas = if gas_samples.is_empty() { None } else { Some(gas_samples[gas_samples.len() / 2]) };
        json!({
            "bucket": label,
            "count": self.count,
            "gross_pos": self.gross_pos,
            "net_pos": self.net_pos,
            "net_pos_non_cluster": self.net_pos_non_cluster,
            "sum_net_pos_bnb": self.sum_net_pos_bnb,
            "sum_net_pos_bnb_non_cluster": self.sum_net_pos_bnb_non_cluster,
            "best_net_bnb": self.best_net_bnb,
            "median_gas_cost_bnb": median_gas,
        })
    }
}

async fn econ(State(state): State<AppState>) -> Json<Value> {
    let log_path = state.logger.path().to_path_buf();
    let content = read_log_tail(&log_path).await;
    let all_lines: Vec<&str> = content.lines().collect();
    let start = all_lines.len().saturating_sub(ECON_MAX_LINES);
    let rows: Vec<Value> = all_lines[start..].iter().filter_map(|l| serde_json::from_str::<Value>(l).ok()).collect();
    // Cụm `real-economics-mode2` — CHỈ tính dòng có `ts >= boot_wall_clock`
    // (`logs/bot.jsonl` dùng CHUNG qua mọi lần chạy, không bị xoá) — thiếu
    // lọc này, /api/econ sẽ lẫn số liệu MỌI lần chạy trước đó (phát hiện
    // thật khi verify 60 phút, BAOCAO38: `lines_scanned` gấp ~4 lần số dòng
    // thật của riêng lần chạy đó).
    let boot_ts = state.boot_wall_clock.to_rfc3339();
    let mut out = compute_econ_from_rows(&rows, Some(&boot_ts));
    // Cụm `econ-truth-latency-vps` (mục 1) — đính kèm `symbol` (đọc từ
    // `pairs.txt`, xem `PairBook::entries`/`PairEntry::symbol`) vào từng
    // `top_pools` — `compute_econ_from_rows` THUẦN (không truy cập PairBook),
    // nên bước enrich này làm Ở ĐÂY (handler duy nhất có `state.pairbook`).
    if let Some(top_pools) = out.get_mut("top_pools").and_then(|v| v.as_array_mut()) {
        let book = state.pairbook.read().await;
        for entry in top_pools.iter_mut() {
            let symbol = entry["pair"]
                .as_str()
                .and_then(|p| Address::from_str(p).ok())
                .and_then(|addr| book.entries().find(|e| e.pair_addr == addr))
                .and_then(|e| e.symbol.clone());
            entry["symbol"] = json!(symbol);
        }
    }
    Json(out)
}

/// Lõi THUẦN (không I/O) của `GET /api/econ` — tách riêng để test được bằng
/// dòng JSON dựng tay, không cần dựng `AppState`/ghi file thật. `since_ts`
/// (RFC3339, `None` = không lọc — dùng bởi test cũ/muốn xem TOÀN BỘ lịch sử
/// file) — so sánh CHUỖI trực tiếp với `row["ts"]`: an toàn vì
/// `BotLogger::log` LUÔN ghi `Utc::now().to_rfc3339()` (cùng định dạng,
/// cùng múi giờ UTC) nên thứ tự chuỗi khớp thứ tự thời gian thật.
fn compute_econ_from_rows(rows: &[Value], since_ts: Option<&str>) -> Value {
    let filtered: Vec<Value>;
    let rows: &[Value] = match since_ts {
        Some(cutoff) => {
            filtered = rows.iter().filter(|r| r["ts"].as_str().map(|t| t >= cutoff).unwrap_or(false)).cloned().collect();
            &filtered
        }
        None => rows,
    };
    let mut buckets: [BucketAcc; 5] = Default::default();
    let mut by_quote: HashMap<String, u64> = HashMap::new();
    let mut by_pool: HashMap<String, PoolAcc> = HashMap::new();
    let mut decode_fail_by_router: HashMap<&'static str, u64> = HashMap::new();
    let mut latency_ms_samples: Vec<f64> = Vec::new();
    let mut nonce_stale_count: u64 = 0;
    let mut candidate_count: u64 = 0;
    let mut net_pos_total: u64 = 0;
    // Cụm `decision-data-24h` (mục 2).
    let mut net_pos_total_non_cluster: u64 = 0;
    let mut sum_net_bnb_total_non_cluster: f64 = 0.0;
    let mut best_net_bnb_total: Option<f64> = None;
    // Cụm `competitor-recon-and-strategy` (F-02) — tổng bribe MÔ PHỎNG đã
    // trừ vào các dòng `sim.result` (mọi dòng `Simulated` ĐÃ vượt gate
    // profit-sau-bribe, xem `pipeline::evaluate_candidate*`) — chỉ cộng khi
    // quy đổi được sang BNB (`quote_to_bnb_rate`, cùng cơ chế `sum_net_bnb`).
    let mut sum_bribe_bnb: f64 = 0.0;
    let mut bribe_samples: u64 = 0;
    // Cụm `bugfix-presign-and-contract-plan` (A1) — số dòng bị loại vì tỉ giá
    // quote→BNB bất khả thi (xem trong vòng lặp).
    let mut rate_inverted_rejected: u64 = 0;
    let mut rate_unavailable: u64 = 0;
    // Cụm `bugfix-presign-and-contract-plan` (A6) — bucket theo VỐN CẦN
    // (`front_in`, quy về BNB-equivalent bằng đúng tỉ giá của dòng đó), song
    // song với bucket theo `victim_in` đã có. Trả lời câu "muốn ăn nhóm cơ
    // hội này thì phải có bao nhiêu vốn", khác hẳn câu "victim to cỡ nào".
    let mut front_buckets: [BucketAcc; 5] = Default::default();
    // (quote, front_in native, net profit native) cho mỗi dòng Simulated có
    // lãi — dùng tính "vốn để lấy 80% tổng lãi" THEO QUOTE.
    let mut capital_rows: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
    // Cụm A3/A6 — nhóm victim thuộc CỤM ĐỐI THỦ đếm RIÊNG.
    let mut competitor_candidate: u64 = 0;
    let mut competitor_simulated: u64 = 0;
    let mut competitor_sum_net_bnb: f64 = 0.0;
    let mut competitor_pools: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Cụm `truth-victim-ok-and-memleak` (mục 6) — LƯỢT QUÉT TRƯỚC: bản đồ
    // `victim_hash -> victim_ok` do EVM (revm 3 chân, `shadow.sim`) trả về.
    //
    // Phải quét trước vì `shadow.sim` chạy trong task NỀN nên dòng của nó nằm
    // SAU dòng `sim.result` tương ứng trong file log (đo thật RUN 4: cách
    // nhau 4–9 giây). Một lượt quét tuyến tính sẽ luôn "chưa biết" tại thời
    // điểm gặp `sim.result`.
    //
    // Ý nghĩa vận hành (lệnh mục 6): `net_pos` là số cơ hội CÓ LÃI theo công
    // thức đóng V2. Nhưng RUN 4 cho thấy 4/4 bundle có `profit_net` dương lại
    // `victim_ok=false` trong EVM — tức khoản lãi đó KHÔNG TỒN TẠI. Vì vậy
    // mọi `net_pos` phải đi kèm 2 cổng, và số tiền chỉ được tính khi CẢ HAI
    // `true`.
    let mut evm_victim_ok: HashMap<String, bool> = HashMap::new();
    for row in rows {
        if row["event"].as_str() == Some("shadow.sim") {
            if let (Some(h), Some(ok)) = (row["victim_hash"].as_str(), row["victim_ok"].as_bool()) {
                evm_victim_ok.insert(h.to_string(), ok);
            }
        }
    }
    // `net_pos` phân rã theo 2 cổng victim-ok (mục 6).
    let mut net_pos_total_v2_ok: u64 = 0;
    let mut net_pos_total_v2_false: u64 = 0;
    let mut net_pos_total_v2_unknown: u64 = 0;
    let mut net_pos_evm_ok: u64 = 0;
    let mut net_pos_evm_false: u64 = 0;
    let mut net_pos_evm_unknown: u64 = 0;
    let mut sum_net_bnb_ca_hai_cong_ok: f64 = 0.0;
    let mut net_pos_ca_hai_cong_ok: u64 = 0;

    for row in rows {
        let event = row["event"].as_str().unwrap_or("");
        // Cụm A6 — `compete.result` (task nền `spawn_post_simulated_tracker`)
        // là nguồn THỨ 2 đánh dấu pool bị đối thủ chạm: tx liền kề victim
        // trong cùng block có chạm ĐÚNG pool đó.
        if event == "compete.result" {
            if let (Some(pair), true) = (row["pair"].as_str(), !row["competitor"].is_null()) {
                by_pool.entry(pair.to_string()).or_default().competitor_touched = true;
                competitor_pools.insert(pair.to_string());
            }
            continue;
        }
        if event != "tx.skip" && event != "sim.result" {
            continue;
        }
        candidate_count += 1;
        let in_cluster = row["victim_in_competitor_cluster"].as_bool().unwrap_or(false);
        if in_cluster {
            competitor_candidate += 1;
            if let Some(pair) = row["pair"].as_str() {
                competitor_pools.insert(pair.to_string());
            }
        }

        if let Some(q) = row["quote"].as_str() {
            *by_quote.entry(q.to_string()).or_insert(0) += 1;
        }
        if let Some(ms) = row["seen_to_decision_ms"].as_f64() {
            latency_ms_samples.push(ms);
        }
        if event == "tx.skip" {
            if row["reason"].as_str() == Some("nonce_stale") {
                nonce_stale_count += 1;
            }
            if row["reason"].as_str() == Some("decode_fail") {
                if let Some(to) = row["to"].as_str() {
                    let name = router_display_name(to);
                    *decode_fail_by_router.entry(name).or_insert(0) += 1;
                }
            }
        }

        // Cụm `econ-truth-latency-vps` (mục 1) — tỉ giá quote-asset -> BNB
        // NGẦM ĐỊNH cho CHÍNH dòng này, suy từ 2 field đã log sẵn
        // (`amount_in` đơn vị quote gốc, `amount_in_bnb_equiv` đã quy đổi ở
        // `main.rs`/`pipeline::convert_usdt_to_bnb_wei`) — KHÔNG phải price
        // oracle (CLAUDE.md cấm), chỉ tái dùng đúng tỉ giá reserve THẬT bot
        // đã tính lúc quyết định. `quote="wbnb"` cho tỉ giá 1.0 tự nhiên
        // (amount_in_bnb_equiv == amount_in, xem `main.rs`).
        // Cụm 1 - CẢ 2 giá trị phải quy về đơn vị BNB/USDT thật (chia 1e18,
        // `parse_wei_str_to_bnb`) TRƯỚC khi tính tỉ giá/so bucket — dùng
        // thẳng wei thô (chưa chia) sẽ không bao giờ khớp khoảng bucket nhỏ
        // (0-1) VÀ sai đơn vị hiển thị.
        let native_amount = row["amount_in"].as_str().and_then(parse_wei_str_to_bnb);
        // Fallback tương thích ngược: dòng log CŨ (trước cụm này) chưa có
        // `amount_in_bnb_equiv` — nếu quote đã là "wbnb" thì amount_in TỰ NÓ
        // đã là BNB, dùng thẳng (rate=1.0) thay vì rớt mất dòng đó.
        let bnb_equiv_amount = row["amount_in_bnb_equiv"]
            .as_str()
            .and_then(parse_wei_str_to_bnb)
            .or_else(|| if row["quote"].as_str() == Some("wbnb") { native_amount } else { None });
        let quote_to_bnb_rate = match (native_amount, bnb_equiv_amount) {
            (Some(n), Some(b)) if n > 0.0 => Some(b / n),
            (None, Some(_)) if row["quote"].as_str() == Some("wbnb") => Some(1.0),
            _ => None,
        };
        // Cụm `bugfix-presign-and-contract-plan` (A1) — LỚP PHÒNG THỦ THỨ 2
        // (lớp 1 là fix chiều cache ở `transport::ReserveCache`): 1 đơn vị
        // USDT KHÔNG THỂ đáng giá ≥ 1 BNB, nên tỉ giá quote→BNB > 1.0 cho
        // quote khác WBNB là bằng chứng dòng log đó có `amount_in_bnb_equiv`
        // ĐẢO CHIỀU (ghi bởi binary TRƯỚC bản sửa). Không đoán/không tự đảo
        // ngược lại (không biết chắc chiều nào đúng cho dòng cũ) — loại dòng
        // đó khỏi mọi phép cộng BNB và đếm riêng `rate_rejected` để Chủ thấy
        // ngay còn bao nhiêu dòng lịch sử bị nhiễm.
        let quote_is_wbnb = row["quote"].as_str() == Some("wbnb");
        let quote_to_bnb_rate = match quote_to_bnb_rate {
            // Tỉ giá 0/âm/NaN = KHÔNG quy đổi được (quan sát thật: dòng
            // `no_pool`/`rpc_error` có `amount_in_bnb_equiv="0"` vì pool
            // WBNB/USDT chưa resolve được lúc đó) — khác hẳn "tỉ giá đảo
            // chiều" của bug A1, nên đếm ở bộ đếm RIÊNG để 2 hiện tượng
            // không lẫn vào nhau.
            Some(r) if !r.is_finite() || r <= 0.0 => {
                rate_unavailable += 1;
                None
            }
            Some(r) if !quote_is_wbnb && r > 1.0 => {
                rate_inverted_rejected += 1;
                None
            }
            other => other,
        };
        // Tỉ giá bị loại -> `amount_in_bnb_equiv` của CHÍNH dòng đó cũng sai
        // (cùng một phép quy đổi sai sinh ra cả 2) -> không bucket dòng này.
        let bnb_equiv_amount = if quote_to_bnb_rate.is_none() && !quote_is_wbnb { None } else { bnb_equiv_amount };

        let pair = row["pair"].as_str().map(|s| s.to_string());
        if let Some(pair) = &pair {
            let acc = by_pool.entry(pair.clone()).or_default();
            acc.count += 1;
            if in_cluster {
                acc.competitor_touched = true;
            }
            if event == "sim.result" {
                if let (Some(net_wei), Some(rate)) = (parse_profit_wei(&row["profit_net_wei"]), quote_to_bnb_rate) {
                    if net_wei > 0 {
                        acc.net_pos += 1;
                        acc.sum_net_bnb += (net_wei as f64) * rate / 1e18;
                        if !in_cluster {
                            acc.net_pos_non_cluster += 1;
                            acc.sum_net_bnb_non_cluster += (net_wei as f64) * rate / 1e18;
                        }
                    }
                }
            }
        }

        // Cum mục 1 (`econ-truth-latency-vps`) — bucket theo victim_in QUY
        // VỀ BNB cho CẢ 2 quote asset (trước cụm này chỉ `quote="wbnb"` được
        // bucket — USDT hoàn toàn vắng mặt khỏi `buckets_bnb`, xem
        // `docs/TASKS.md` mục nợ `hotpath-fix-then-decoder-ur`). Quy đổi qua
        // `amount_in_bnb_equiv` (đã tính sẵn bằng reserve THẬT, không price
        // oracle) — KHÔNG còn đọc thẳng `amount_in` (đơn vị quote gốc, sai
        // đơn vị cho USDT).
        if let Some(amount_bnb) = bnb_equiv_amount {
            if let Some(bucket_idx) = BNB_BUCKETS.iter().position(|(_, lo, hi)| amount_bnb >= *lo && amount_bnb < *hi) {
                let acc = &mut buckets[bucket_idx];
                acc.count += 1;
                if let Some(gas_bnb) = row["gas_cost_wei"].as_str().and_then(parse_wei_str_to_bnb) {
                    acc.gas_cost_bnb_samples.push(gas_bnb);
                }
                if event == "sim.result" {
                    let gross = parse_profit_wei(&row["profit_gross_wei"]);
                    let net = parse_profit_wei(&row["profit_net_wei"]);
                    if gross.map(|g| g > 0).unwrap_or(false) {
                        acc.gross_pos += 1;
                    }
                    if let (Some(net_wei), Some(rate)) = (net, quote_to_bnb_rate) {
                        if net_wei > 0 {
                            acc.net_pos += 1;
                            net_pos_total += 1;
                            let net_bnb = (net_wei as f64) * rate / 1e18;
                            if !in_cluster {
                                acc.net_pos_non_cluster += 1;
                                acc.sum_net_pos_bnb_non_cluster += net_bnb;
                                net_pos_total_non_cluster += 1;
                                sum_net_bnb_total_non_cluster += net_bnb;
                            }
                            acc.sum_net_pos_bnb += net_bnb;
                            acc.best_net_bnb = Some(acc.best_net_bnb.map_or(net_bnb, |b: f64| b.max(net_bnb)));
                            best_net_bnb_total = Some(best_net_bnb_total.map_or(net_bnb, |b: f64| b.max(net_bnb)));
                        }
                        if let Some(bribe_wei) = parse_profit_wei(&row["bribe_wei"]) {
                            if bribe_wei > 0 {
                                sum_bribe_bnb += (bribe_wei as f64) * rate / 1e18;
                                bribe_samples += 1;
                            }
                        }
                    }
                }
            }
        }

        // ===== Cụm `bugfix-presign-and-contract-plan` (A6) =====
        // (1) bucket theo VỐN CẦN (`front_in`, quy về BNB-equivalent).
        // (2) mẫu (front_in, net) THEO QUOTE cho "vốn lấy 80% tổng lãi".
        // (3) tổng riêng cho nhóm victim thuộc cụm đối thủ.
        if event == "sim.result" {
            let front_native = row["front_in_wei"].as_str().and_then(parse_wei_str_to_bnb);
            let net_native = parse_profit_wei(&row["profit_net_wei"]).map(|w| w as f64 / 1e18);
            if let (Some(front), Some(rate)) = (front_native, quote_to_bnb_rate) {
                let front_bnb = front * rate;
                if let Some(idx) = BNB_BUCKETS.iter().position(|(_, lo, hi)| front_bnb >= *lo && front_bnb < *hi) {
                    let acc = &mut front_buckets[idx];
                    acc.count += 1;
                    if let Some(net) = net_native {
                        if net > 0.0 {
                            acc.net_pos += 1;
                            acc.sum_net_pos_bnb += net * rate;
                            // Cụm `decision-data-24h` (mục 2) — bucket VỐN CẦN
                            // cũng phải tách nhóm cụm đối thủ, nếu không bảng
                            // "cần bao nhiêu vốn" sẽ tính cả vốn cho những cơ
                            // hội mà ta KHÔNG định lấy.
                            if !in_cluster {
                                acc.net_pos_non_cluster += 1;
                                acc.sum_net_pos_bnb_non_cluster += net * rate;
                            }
                            acc.best_net_bnb = Some(acc.best_net_bnb.map_or(net * rate, |b: f64| b.max(net * rate)));
                        }
                    }
                }
            }
            // Cụm `truth-victim-ok-and-memleak` (mục 6) — phân rã `net_pos`
            // theo 2 cổng victim-ok. `victim_ok_v2` là `None` với log sinh ra
            // TRƯỚC cụm này (field chưa tồn tại) — đếm vào `unknown`, KHÔNG
            // suy diễn thành `true`.
            if net_native.map(|n| n > 0.0).unwrap_or(false) {
                match row["victim_ok_v2"].as_bool() {
                    Some(true) => net_pos_total_v2_ok += 1,
                    Some(false) => net_pos_total_v2_false += 1,
                    None => net_pos_total_v2_unknown += 1,
                }
                let evm = row["hash"].as_str().and_then(|h| evm_victim_ok.get(h).copied());
                match evm {
                    Some(true) => net_pos_evm_ok += 1,
                    Some(false) => net_pos_evm_false += 1,
                    None => net_pos_evm_unknown += 1,
                }
                if row["victim_ok_v2"].as_bool() == Some(true) && evm == Some(true) {
                    net_pos_ca_hai_cong_ok += 1;
                    if let (Some(net), Some(rate)) = (net_native, quote_to_bnb_rate) {
                        sum_net_bnb_ca_hai_cong_ok += net * rate;
                    }
                }
            }
            if let (Some(front), Some(net)) = (front_native, net_native) {
                if net > 0.0 && front > 0.0 {
                    let q = row["quote"].as_str().unwrap_or("unknown").to_string();
                    capital_rows.entry(q).or_default().push((front, net));
                }
            }
            if in_cluster {
                competitor_simulated += 1;
                if let (Some(net), Some(rate)) = (net_native, quote_to_bnb_rate) {
                    if net > 0.0 {
                        competitor_sum_net_bnb += net * rate;
                    }
                }
            }
        }
    }

    // Cụm A6 — "vốn để lấy 80% tổng lãi" THEO QUOTE: sắp các cơ hội có lãi
    // theo `front_in` TĂNG DẦN, cộng dồn lãi tới khi đạt ≥80% tổng lãi; con
    // số trả về là `front_in` của cơ hội CUỐI CÙNG phải lấy — tức mức vốn tối
    // thiểu (đơn vị quote gốc) đủ để với tới 80% lợi nhuận quan sát được.
    // Không quy đổi sang BNB ở đây (mỗi quote báo bằng chính đơn vị của nó,
    // tránh mọi khả năng sai tỉ giá — đúng bài học A1).
    let capital_80: Value = {
        let mut m = serde_json::Map::new();
        for (q, mut rows_q) in capital_rows {
            rows_q.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            let total: f64 = rows_q.iter().map(|(_, n)| n).sum();
            let target = total * 0.8;
            let mut acc = 0.0;
            let mut needed_front = 0.0;
            let mut taken = 0u64;
            for (front, net) in &rows_q {
                acc += net;
                taken += 1;
                needed_front = *front;
                if acc >= target {
                    break;
                }
            }
            m.insert(
                q,
                json!({
                    "opportunities": rows_q.len(),
                    "taken_for_80pct": taken,
                    "capital_needed_native": needed_front,
                    "total_net_native": total,
                    "captured_native": acc,
                }),
            );
        }
        Value::Object(m)
    };

    // Cụm `econ-truth-latency-vps` (mục 1) — `top_pools` (pair/count/net_pos/
    // sum_net_bnb) THAY `top_tokens` cũ — `symbol` được đính kèm SAU (ở
    // `econ()`, hàm THUẦN này không có quyền truy cập `PairBook`).
    let mut top_pools: Vec<(String, PoolAcc)> = by_pool.into_iter().collect();
    // Cụm `bugfix-presign-and-contract-plan` (A6) — `top_pools` sắp theo
    // COUNT (pool bận nhất). Đo thật 30 phút cho thấy như vậy CHƯA ĐỦ để
    // trả lời go/no-go: 2 pool DUY NHẤT có lãi (`0xdfe23efb…`, `0xcec13213…`)
    // đều KHÔNG lọt top-10 theo count, nên bảng đó toàn `net_pos=0`. Thêm
    // danh sách THỨ 2 sắp theo LÃI — đây mới là bảng trả lời "pool nào đáng
    // làm, và pool đó có bị cụm đối thủ chạm không".
    let mut top_pools_by_net: Vec<(String, u64, f64, bool, u64, f64)> = top_pools
        .iter()
        .filter(|(_, acc)| acc.net_pos > 0)
        .map(|(p, acc)| {
            (p.clone(), acc.net_pos, acc.sum_net_bnb, acc.competitor_touched, acc.net_pos_non_cluster, acc.sum_net_bnb_non_cluster)
        })
        .collect();
    // Cụm `decision-data-24h` (mục 2) — sắp theo LÃI ĐÃ LOẠI CỤM ĐỐI THỦ
    // (`sum_net_bnb_non_cluster`), không phải lãi thô: 2 pool "lãi nhất" đo
    // được trên VPS (`0xdfe23efb…`, `0xcec13213…`) có 100% victim là ví của
    // cụm đối thủ, tức đứng đầu bảng cũ nhưng KHÔNG đáng làm.
    top_pools_by_net.sort_by(|a, b| b.5.partial_cmp(&a.5).unwrap_or(std::cmp::Ordering::Equal));
    top_pools_by_net.truncate(10);
    top_pools.sort_by(|a, b| b.1.count.cmp(&a.1.count));
    top_pools.truncate(10);

    let p50 = percentile_f64(&latency_ms_samples, 50.0);
    let p95 = percentile_f64(&latency_ms_samples, 95.0);
    let stale_pct = if candidate_count > 0 { nonce_stale_count as f64 / candidate_count as f64 * 100.0 } else { 0.0 };
    let decode_fail_smartrouter = decode_fail_by_router.get("SmartRouter").copied().unwrap_or(0);

    let summary_line = format!(
        "candidate={} net_pos={} net_pos_non_cluster={} net_pos_v2ok={} net_pos_evmok={} net_pos_ca_hai_cong_ok={} best_net_bnb={} p50_ms={} p95_ms={} stale_pct={:.2} decode_fail_smartrouter={}",
        candidate_count,
        net_pos_total,
        net_pos_total_non_cluster,
        net_pos_total_v2_ok,
        net_pos_evm_ok,
        net_pos_ca_hai_cong_ok,
        best_net_bnb_total.map(|b| format!("{b:.6}")).unwrap_or_else(|| "-".to_string()),
        p50.map(|v| format!("{v:.2}")).unwrap_or_else(|| "-".to_string()),
        p95.map(|v| format!("{v:.2}")).unwrap_or_else(|| "-".to_string()),
        stale_pct,
        decode_fail_smartrouter,
    );

    json!({
        "lines_scanned": rows.len(),
        "candidate": candidate_count,
        "buckets_bnb": buckets.iter().zip(BNB_BUCKETS.iter()).map(|(acc, (label, _, _))| acc.snapshot(label)).collect::<Vec<_>>(),
        "by_quote": by_quote,
        "top_pools": top_pools.into_iter().map(|(pair, acc)| json!({
            "pair": pair,
            "count": acc.count,
            "net_pos": acc.net_pos,
            "net_pos_non_cluster": acc.net_pos_non_cluster,
            "sum_net_bnb": acc.sum_net_bnb,
            "sum_net_bnb_non_cluster": acc.sum_net_bnb_non_cluster,
            "pct_net_pos_la_vi_cum": if acc.net_pos > 0 { (acc.net_pos - acc.net_pos_non_cluster) as f64 * 100.0 / acc.net_pos as f64 } else { 0.0 },
            // Cụm A6 — "cụm đối thủ chạm: có/không" (2 nguồn: victim CHÍNH LÀ
            // ví của cụm, hoặc `compete.result` thấy tx liền kề chạm cùng pool).
            "competitor_touched": acc.competitor_touched,
        })).collect::<Vec<_>>(),
        // A6 — pool ĐÁNG LÀM (có lãi), kèm cờ cụm đối thủ chạm: đây là bảng
        // dùng cho điều kiện go/no-go #2 (`docs/CONTRACT_DESIGN.md` B7).
        "top_pools_by_net": top_pools_by_net.into_iter().map(|(pair, net_pos, sum_net_bnb, competitor_touched, net_pos_nc, sum_net_bnb_nc)| json!({
            "pair": pair,
            "net_pos": net_pos,
            "net_pos_non_cluster": net_pos_nc,
            "sum_net_bnb": sum_net_bnb,
            "sum_net_bnb_non_cluster": sum_net_bnb_nc,
            "pct_net_pos_la_vi_cum": if net_pos > 0 { (net_pos - net_pos_nc) as f64 * 100.0 / net_pos as f64 } else { 0.0 },
            "competitor_touched": competitor_touched,
        })).collect::<Vec<_>>(),
        "decode_fail_by_router": decode_fail_by_router,
        "latency_ms": { "p50": p50, "p95": p95, "samples": latency_ms_samples.len() },
        "nonce_stale_pct_of_candidate": stale_pct,
        "net_pos_total": net_pos_total,
        // Cụm `decision-data-24h` (mục 2) — con số DUY NHẤT nên dùng để kết
        // luận kinh tế (xem `PoolAcc::net_pos_non_cluster`).
        "net_pos_total_non_cluster": net_pos_total_non_cluster,
        "sum_net_bnb_total_non_cluster": sum_net_bnb_total_non_cluster,
        // Cụm `truth-victim-ok-and-memleak` (mục 6) — MỖI `net_pos` phải kèm
        // 2 cổng victim-ok, và số tiền chỉ được tính khi CẢ HAI `true`.
        // `unknown` = dữ liệu không có (log cũ chưa có field, hoặc
        // `shadow.sim` chưa chạy cho hash đó) — KHÔNG được đọc thành `true`.
        "victim_ok": {
            "v2_ok": net_pos_total_v2_ok,
            "v2_false": net_pos_total_v2_false,
            "v2_unknown": net_pos_total_v2_unknown,
            "evm_ok": net_pos_evm_ok,
            "evm_false": net_pos_evm_false,
            "evm_unknown": net_pos_evm_unknown,
            "ca_hai_cong_ok": net_pos_ca_hai_cong_ok,
            "sum_net_bnb_ca_hai_cong_ok": sum_net_bnb_ca_hai_cong_ok,
            "ghi_chu": "So tien DUY NHAT duoc phep ket luan kinh te la sum_net_bnb_ca_hai_cong_ok. net_pos tran trui va net_pos_non_cluster KHONG kiem tra victim co thuc thi duoc khong.",
        },
        "best_net_bnb": best_net_bnb_total,
        "summary_line": summary_line,
        // Cụm `competitor-recon-and-strategy` (F-02) — bribe MÔ PHỎNG đã trừ
        // vào các dòng Simulated (gate đã áp ở pipeline.rs). "Simulated" ở
        // đây LUÔN net_pos_after_bribe>0 (đúng định nghĩa gate mới) — bucket
        // "lãi trước bribe nhưng KHÔNG còn lãi sau bribe" CHƯA tách được từ
        // log hiện có (`tx.skip{reason:unprofitable}` không mang profit_wei
        // thô để so sánh riêng — xem docs/TASKS.md mục nợ).
        "bribe": {
            "sum_bribe_bnb": sum_bribe_bnb,
            "samples": bribe_samples,
        },
        // Cụm `bugfix-presign-and-contract-plan` (A1) — dòng log có tỉ giá
        // quote→BNB bất khả thi (ghi bởi binary trước bản sửa chiều cache
        // reserve) đã bị LOẠI khỏi mọi phép cộng BNB, không phải bị bỏ quên.
        // `rate_inverted_rejected`: dòng có tỉ giá quote→BNB **> 1.0** (bất
        // khả thi) — dấu vết bug A1 (ghi bởi binary trước bản sửa
        // `ReserveCache`). Kỳ vọng = 0 với mọi dòng ghi SAU bản sửa.
        "rate_inverted_rejected": rate_inverted_rejected,
        // `rate_unavailable`: dòng KHÔNG quy đổi được (tỉ giá 0/NaN, thường
        // do `no_pool`/`rpc_error` khiến `amount_in_bnb_equiv="0"`) — bình
        // thường, KHÔNG phải bug.
        "rate_unavailable": rate_unavailable,
        // Cụm `bugfix-presign-and-contract-plan` (A6) — bucket theo VỐN CẦN
        // (`front_in` quy về BNB-equivalent), song song `buckets_bnb` (theo
        // `victim_in`).
        "buckets_front_in_bnb": front_buckets.iter().zip(BNB_BUCKETS.iter()).map(|(acc, (label, _, _))| acc.snapshot(label)).collect::<Vec<_>>(),
        // A6 — vốn tối thiểu (ĐƠN VỊ QUOTE GỐC, không quy đổi) để với tới 80%
        // tổng lãi quan sát được, tách theo quote asset.
        "capital_for_80pct_profit": capital_80,
        // A3/A6 — nhóm victim thuộc CỤM ĐỐI THỦ, đếm RIÊNG.
        "competitor": {
            "candidate": competitor_candidate,
            "simulated": competitor_simulated,
            "sum_net_bnb": competitor_sum_net_bnb,
            "pools_touched": competitor_pools.len(),
            "pct_of_candidate": if candidate_count > 0 { competitor_candidate as f64 / candidate_count as f64 * 100.0 } else { 0.0 },
        },
    })
}

/// Cụm tax-cache-inject — bảng cache tax hiện có (token/bps/block/fresh?)
/// cho web dashboard, đúng "Web: bảng cache token + bps + block".
async fn tax_cache_list(State(state): State<AppState>) -> Json<Value> {
    let cache = state.tax_cache.read().await;
    let cfg = state.config.read().await;
    let current_block = state.last_block.read().await.unwrap_or(0);
    let entries: Vec<Value> = cache
        .entries()
        .map(|(k, m)| {
            json!({
                "token": format!("{:#x}", k.token),
                // Cụm `evm-validate-fixed-then-wire` (C2) — cache khoá theo CẶP
                // (token, quote); field `quote` mới, mặc định WBNB cho bản ghi
                // điền tay qua đường `inject_from_buy_sell_bps` (không có quote).
                "quote": format!("{:#x}", k.quote),
                "roundtrip_tax_bps": m.roundtrip_tax_bps,
                "buy_bps": m.buy_bps,
                "sell_bps": m.sell_bps,
                "honeypot": m.honeypot,
                "measured_at_block": m.measured_at_block,
                "fresh": cache.get_fresh_ttl(k.token, k.quote, cfg.tax_cache_ttl()).is_some(),
            })
        })
        .collect();
    Json(json!({
        "allow_tax_inject": cfg.allow_tax_inject,
        "current_block": current_block,
        // `tax_cache_blocks` GIỮ LẠI để không phá client cũ, nhưng KHÔNG còn
        // quyết định tươi/cũ — `tax_cache_ttl_sec` mới là ngưỡng thật (C2).
        "tax_cache_blocks": cfg.tax_cache_blocks,
        "tax_cache_ttl_sec": cfg.tax_cache_ttl_sec,
        "entries": entries,
    }))
}

#[derive(Deserialize)]
struct TaxInjectBody {
    token: String,
    buy_tax: u32,
    sell_tax: u32,
}

/// Cụm tax-cache-inject — `POST /api/tax {token, buy_tax, sell_tax}` (đơn vị
/// basis point, cùng đơn vị `buy_bps`/`sell_bps` trong `state/tax_inject.jsonl`
/// để chỉ có 1 cách hiểu số — xem `tax::combine_roundtrip_bps`). `allow_tax_inject=false`
/// (`config.toml`) -> BỎ QUA, trả `ok:false` rõ ràng, KHÔNG ghi cache, KHÔNG
/// panic — đúng lệnh "false thì API/file bỏ qua".
async fn tax_inject(State(state): State<AppState>, Json(body): Json<TaxInjectBody>) -> Json<Value> {
    let allow = state.config.read().await.allow_tax_inject;
    if !allow {
        return Json(json!({ "ok": false, "error": "allow_tax_inject=false trong config.toml, API bi bo qua" }));
    }
    let token = match Address::from_str(body.token.trim()) {
        Ok(t) => t,
        Err(e) => return Json(json!({ "ok": false, "error": format!("token khong hop le: {e}") })),
    };
    let current_block = state.last_block.read().await.unwrap_or(0);
    {
        let mut cache = state.tax_cache.write().await;
        cache.inject_from_buy_sell_bps(token, body.buy_tax, body.sell_tax, current_block);
    }
    state.logger.log(
        "tax.inject",
        json!({
            "token": format!("{:#x}", token),
            "buy_bps": body.buy_tax,
            "sell_bps": body.sell_tax,
            "measured_at_block": current_block,
            "source": "api",
        }),
    );
    Json(json!({ "ok": true, "token": format!("{:#x}", token), "measured_at_block": current_block }))
}

#[derive(Deserialize)]
struct ControlBody {
    action: String,
}

async fn control(State(state): State<AppState>, Json(body): Json<ControlBody>) -> Json<Value> {
    match ControlAction::parse(&body.action) {
        Some(action) => match action.apply(&state.state_files) {
            Ok(()) => {
                state
                    .logger
                    .log("control.request", json!({ "action": body.action }));
                Json(json!({ "ok": true, "action": body.action }))
            }
            Err(e) => Json(json!({ "ok": false, "error": e.to_string() })),
        },
        None => Json(json!({ "ok": false, "error": "unknown action" })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ĐẠT CẦN DÁN (F-27) — tái tạo ĐÚNG bộ số audit (`BAOCAO_AUDIT_2026-09-15.md`
    /// mục 5.4): 18 dòng, `isolated:true` 2 dòng (cả 2 đều 0.0% — trong
    /// `within_1pct`), `isolated:false` 16 dòng (10 ≤1%, 6 >1%: 1.11, 1.19,
    /// 1.61, 2.40, 2.40, 3.10). Bug cũ: `within_1pct` toàn cục chỉ đếm
    /// `isolated:true` (=2, tỉ lệ 11.1%) — SAU fix, tổng `within_1pct` phải
    /// là 12 (2 isolated + 10 non_isolated), khớp đúng số đếm tay của audit.
    #[test]
    fn validate_stats_matches_audit_manual_recount_after_f27_fix() {
        let mut stats = ValidateStats::default();
        // 2 dong isolated, ca 2 deu 0.0%.
        stats.push(json!({"isolated": true, "lech_pct": 0.0}), true, 0.0);
        stats.push(json!({"isolated": true, "lech_pct": 0.0}), true, 0.0);
        // 16 dong non_isolated: 10 <=1%, 6 >1% (dung 6 gia tri audit liet ke).
        for _ in 0..10 {
            stats.push(json!({"isolated": false, "lech_pct": 0.5}), false, 0.5);
        }
        for pct in [1.11, 1.19, 1.61, 2.40, 2.40, 3.10] {
            stats.push(json!({"isolated": false, "lech_pct": pct}), false, pct);
        }

        let snap = stats.snapshot();
        assert_eq!(snap["total"], 18);
        assert_eq!(snap["within_1pct"], 12, "F-27: tong within_1pct phai la 12 (2 isolated + 10 non_isolated), khong con la 2");
        assert_eq!(snap["isolated"]["n"], 2);
        assert_eq!(snap["isolated"]["within_1pct"], 2);
        assert_eq!(snap["non_isolated"]["n"], 16);
        assert_eq!(snap["non_isolated"]["within_1pct"], 10);
        let ratio = snap["within_1pct_ratio"].as_f64().unwrap();
        assert!((ratio - (12.0 / 18.0)).abs() < 1e-9);
    }

    #[test]
    fn validate_group_stats_percentile_matches_manual_sort() {
        let mut g = ValidateGroupStats::default();
        for pct in [0.0, 0.5, 1.11, 1.19, 1.61, 2.40, 2.40, 3.10] {
            g.push(pct);
        }
        // 8 mau, sap xep: 0.0 0.5 1.11 1.19 1.61 2.40 2.40 3.10
        // p50 (nearest-rank tren idx round(0.5*7)=round(3.5)=4 -> gia tri idx4=1.61)
        assert_eq!(g.percentile(50.0), Some(1.61));
        // p95: idx = round(0.95*7)=round(6.65)=7 -> gia tri cuoi 3.10
        assert_eq!(g.percentile(95.0), Some(3.10));
    }

    #[test]
    fn validate_group_stats_percentile_none_when_empty() {
        let g = ValidateGroupStats::default();
        assert_eq!(g.percentile(50.0), None);
        assert_eq!(g.snapshot()["p50_lech_pct"], Value::Null);
    }

    #[test]
    fn validate_stats_snapshot_empty_has_zero_ratio_not_panic() {
        let stats = ValidateStats::default();
        let snap = stats.snapshot();
        assert_eq!(snap["total"], 0);
        assert_eq!(snap["within_1pct_ratio"], 0.0);
    }

    // ===== Cụm `real-economics-mode2` (mục 3) — /api/econ =====

    fn wei(bnb: f64) -> String {
        ((bnb * 1e18) as u128).to_string()
    }

    /// Cụm `decision-data-24h` (mục 2) — `net_pos` KHÔNG còn đứng một mình ở
    /// bất kỳ đâu: bucket, pool, tổng, và `summary_line` đều phải kèm con số
    /// ĐÃ LOẠI victim thuộc cụm đối thủ. Dữ liệu thật 10.92 h trên VPS: 481/497
    /// cơ hội có lãi là ví burner của chính cụm đối thủ.
    #[test]
    /// Cụm `truth-victim-ok-and-memleak` (mục 6) — `net_pos` phải tách theo
    /// 2 cổng victim-ok, và TIỀN chỉ được cộng khi CẢ HAI `true`.
    ///
    /// Fixture tái hiện đúng tình huống RUN 4 (BAOCAO43): dòng có
    /// `profit_net` DƯƠNG nhưng `shadow.sim` sau đó báo `victim_ok=false` —
    /// khoản lãi đó không tồn tại và không được phép lọt vào con số kết luận.
    /// `shadow.sim` cố ý đặt SAU `sim.result` trong danh sách để test luôn
    /// việc phải quét trước (task nền ghi log sau, đo thật cách 4–9 giây).
    #[test]
    fn compute_econ_net_pos_luon_kem_2_cong_victim_ok() {
        let sim = |hash: &str, v2ok: Option<bool>, profit: i64| {
            let mut v = json!({
                "event": "sim.result",
                "hash": hash,
                "pair": "0xpool",
                "quote": "wbnb",
                "amount_in": wei(0.06),
                "amount_in_bnb_equiv": wei(0.06),
                "front_in_wei": wei(0.05),
                "profit_net_wei": profit,
            });
            if let Some(b) = v2ok {
                v["victim_ok_v2"] = json!(b);
            }
            v
        };
        let shadow = |hash: &str, ok: bool| json!({"event": "shadow.sim", "victim_hash": hash, "victim_ok": ok});
        let rows = vec![
            sim("0xaa", Some(true), 4_000_000_000_000_000i64),  // ca 2 cong OK
            sim("0xbb", Some(true), 6_000_000_000_000_000i64),  // EVM noi victim CHET
            sim("0xcc", Some(true), 1_000_000_000_000_000i64),  // chua co shadow.sim
            sim("0xdd", None, 2_000_000_000_000_000i64),        // log CU, khong co field
            shadow("0xaa", true),
            shadow("0xbb", false),
        ];
        let econ = compute_econ_from_rows(&rows, None);
        let v = &econ["victim_ok"];

        assert_eq!(econ["net_pos_total"], 4, "4 dong deu co profit duong");
        assert_eq!(v["v2_ok"], 3);
        assert_eq!(v["v2_unknown"], 1, "log cu khong co field -> unknown, KHONG suy dien thanh true");
        assert_eq!(v["evm_ok"], 1);
        assert_eq!(v["evm_false"], 1);
        assert_eq!(v["evm_unknown"], 2);
        assert_eq!(v["ca_hai_cong_ok"], 1, "chi 0xaa qua duoc CA HAI cong");
        assert!(
            (v["sum_net_bnb_ca_hai_cong_ok"].as_f64().unwrap() - 0.004).abs() < 1e-9,
            "chi duoc cong tien cua 0xaa (0.004 BNB), khong duoc cong 0xbb du profit_net cao hon"
        );
        assert!(econ["summary_line"].as_str().unwrap().contains("net_pos_ca_hai_cong_ok=1"));
    }

    #[test]
    fn compute_econ_tach_net_pos_non_cluster_moi_cho_co_net_pos() {
        let row = |pair: &str, cluster: bool, profit: i64| {
            json!({
                "event": "sim.result",
                "token": "0xtoken1",
                "pair": pair,
                "quote": "wbnb",
                "amount_in": wei(0.06),
                "amount_in_bnb_equiv": wei(0.06),
                "front_in_wei": wei(0.05),
                "profit_net_wei": profit,
                "victim_in_competitor_cluster": cluster,
            })
        };
        let rows = vec![
            row("0xpool_cum", true, 4_000_000_000_000_000i64),
            row("0xpool_cum", true, 6_000_000_000_000_000i64),
            row("0xpool_that", false, 3_000_000_000_000_000i64),
        ];
        let econ = compute_econ_from_rows(&rows, None);

        assert_eq!(econ["net_pos_total"], 3);
        assert_eq!(econ["net_pos_total_non_cluster"], 1, "chi 1 co hoi khong thuoc cum doi thu");
        assert!((econ["sum_net_bnb_total_non_cluster"].as_f64().unwrap() - 0.003).abs() < 1e-9);

        let bucket = econ["buckets_bnb"].as_array().unwrap().iter().find(|b| b["bucket"] == "0.05-0.2").unwrap();
        assert_eq!(bucket["net_pos"], 3);
        assert_eq!(bucket["net_pos_non_cluster"], 1);

        // `top_pools_by_net` sap theo LAI DA LOAI CUM -> pool that dung dau,
        // du pool cum co tong lai lon gap 3.3 lan.
        let by_net = econ["top_pools_by_net"].as_array().unwrap();
        assert_eq!(by_net[0]["pair"], "0xpool_that");
        assert_eq!(by_net[0]["net_pos_non_cluster"], 1);
        assert!((by_net[0]["pct_net_pos_la_vi_cum"].as_f64().unwrap() - 0.0).abs() < 1e-9);
        let pool_cum = by_net.iter().find(|p| p["pair"] == "0xpool_cum").unwrap();
        assert_eq!(pool_cum["net_pos"], 2);
        assert_eq!(pool_cum["net_pos_non_cluster"], 0);
        assert!((pool_cum["pct_net_pos_la_vi_cum"].as_f64().unwrap() - 100.0).abs() < 1e-9);

        assert!(
            econ["summary_line"].as_str().unwrap().contains("net_pos=3 net_pos_non_cluster=1"),
            "summary_line phai hien ca 2 so: {}",
            econ["summary_line"]
        );
    }

    /// ĐẠT CẦN DÁN — 1 dòng `sim.result` quote=wbnb, `amount_in`=0.06 BNB
    /// (rơi vào bucket "0.05-0.2") với `profit_net_wei` DƯƠNG phải: tăng
    /// `count`/`net_pos` đúng bucket, cộng vào `net_pos_total`/`best_net_bnb`,
    /// và xuất hiện trong `by_quote`/`top_tokens`.
    #[test]
    fn compute_econ_buckets_a_positive_sim_result_row_correctly() {
        let rows = vec![json!({
            "event": "sim.result",
            "token": "0xtoken1",
            "pair": "0xpair1",
            "quote": "wbnb",
            "amount_in": wei(0.06),
            "amount_in_bnb_equiv": wei(0.06),
            "gas_cost_wei": wei(0.002),
            "profit_gross_wei": 5_000_000_000_000_000i64,
            "profit_net_wei": 3_000_000_000_000_000i64,
            "seen_to_decision_ms": 12.5,
        })];
        let econ = compute_econ_from_rows(&rows, None);
        assert_eq!(econ["candidate"], 1);
        let buckets = econ["buckets_bnb"].as_array().unwrap();
        let bucket_005_02 = buckets.iter().find(|b| b["bucket"] == "0.05-0.2").unwrap();
        assert_eq!(bucket_005_02["count"], 1);
        assert_eq!(bucket_005_02["gross_pos"], 1);
        assert_eq!(bucket_005_02["net_pos"], 1);
        assert!((bucket_005_02["sum_net_pos_bnb"].as_f64().unwrap() - 0.003).abs() < 1e-9);
        assert!((bucket_005_02["best_net_bnb"].as_f64().unwrap() - 0.003).abs() < 1e-9);
        assert!((bucket_005_02["median_gas_cost_bnb"].as_f64().unwrap() - 0.002).abs() < 1e-9);
        assert_eq!(econ["net_pos_total"], 1);
        assert!((econ["best_net_bnb"].as_f64().unwrap() - 0.003).abs() < 1e-9);
        assert_eq!(econ["by_quote"]["wbnb"], 1);
        assert_eq!(econ["top_pools"][0]["pair"], "0xpair1");
        assert_eq!(econ["top_pools"][0]["count"], 1);
        assert_eq!(econ["top_pools"][0]["net_pos"], 1);
        assert!((econ["top_pools"][0]["sum_net_bnb"].as_f64().unwrap() - 0.003).abs() < 1e-9);
        assert!(econ["summary_line"].as_str().unwrap().contains("candidate=1"));
    }

    /// Dòng CŨ (trước cụm `econ-truth-latency-vps`) không có
    /// `amount_in_bnb_equiv` — vẫn phải bucket ĐÚNG cho quote wbnb (fallback
    /// coi `amount_in` tự nó là BNB, rate=1.0), không rớt mất dòng lịch sử.
    #[test]
    fn compute_econ_legacy_wbnb_row_without_bnb_equiv_field_still_buckets() {
        let rows = vec![json!({
            "event": "sim.result", "quote": "wbnb", "amount_in": wei(0.06),
            "profit_gross_wei": 1i64, "profit_net_wei": 1i64,
        })];
        let econ = compute_econ_from_rows(&rows, None);
        let total: i64 = econ["buckets_bnb"].as_array().unwrap().iter().map(|b| b["count"].as_i64().unwrap()).sum();
        assert_eq!(total, 1, "dong wbnb cu (khong co amount_in_bnb_equiv) van phai bucket duoc");
    }

    /// `decode_fail` với `to` = SmartRouter đã pin phải đếm đúng tên hiển thị
    /// "SmartRouter" (mục 3.c), phản ánh vào `decode_fail_smartrouter` ở
    /// dòng tổng.
    #[test]
    fn compute_econ_decode_fail_grouped_by_router_display_name() {
        let smart_router = "0x13f4EA83D0bd40E75C8222255bc855a974568Dd4";
        let rows = vec![
            json!({"event": "tx.skip", "reason": "decode_fail", "to": smart_router}),
            json!({"event": "tx.skip", "reason": "decode_fail", "to": smart_router}),
            json!({"event": "tx.skip", "reason": "decode_fail", "to": "0x0000000000000000000000000000000000dead"}),
        ];
        let econ = compute_econ_from_rows(&rows, None);
        assert_eq!(econ["decode_fail_by_router"]["SmartRouter"], 2);
        assert_eq!(econ["decode_fail_by_router"]["other"], 1);
        assert!(econ["summary_line"].as_str().unwrap().contains("decode_fail_smartrouter=2"));
    }

    /// `nonce_stale` chia trên tổng `candidate` (mọi dòng tx.skip/sim.result)
    /// -> `nonce_stale_pct_of_candidate` đúng tỉ lệ.
    #[test]
    fn compute_econ_nonce_stale_percentage_of_candidate() {
        let rows = vec![
            json!({"event": "tx.skip", "reason": "nonce_stale"}),
            json!({"event": "tx.skip", "reason": "below_min"}),
            json!({"event": "tx.skip", "reason": "below_min"}),
            json!({"event": "tx.skip", "reason": "below_min"}),
        ];
        let econ = compute_econ_from_rows(&rows, None);
        assert_eq!(econ["candidate"], 4);
        assert!((econ["nonce_stale_pct_of_candidate"].as_f64().unwrap() - 25.0).abs() < 1e-9);
    }

    /// `seen_to_decision_ms` p50/p95 tính đúng trên mẫu THẬT (không phải chỉ
    /// assert `Some`) — 4 mẫu [10, 20, 30, 40].
    #[test]
    fn compute_econ_latency_percentiles_match_manual_sort() {
        let rows: Vec<Value> = [10.0, 20.0, 30.0, 40.0]
            .iter()
            .map(|ms| json!({"event": "tx.skip", "reason": "below_min", "seen_to_decision_ms": ms}))
            .collect();
        let econ = compute_econ_from_rows(&rows, None);
        // nearest-rank: p50 idx=round(0.5*3)=2 -> gia tri 30; p95 idx=round(0.95*3)=3 -> 40.
        assert_eq!(econ["latency_ms"]["p50"], 30.0);
        assert_eq!(econ["latency_ms"]["p95"], 40.0);
        assert_eq!(econ["latency_ms"]["samples"], 4);
    }

    /// USDT quote KHÔNG được quy vào bucket BNB (CLAUDE.md cấm price oracle
    /// quy đổi) — vẫn đếm vào `by_quote`/`candidate` nhưng không rơi vào bất
    /// kỳ bucket BNB nào.
    #[test]
    fn compute_econ_usdt_quote_without_bnb_equiv_field_is_not_bucketed() {
        // Dong USDT KHONG co amount_in_bnb_equiv (vd loi quy doi wbnb_usdt
        // reserve luc do) -> khong the bucket (khac wbnb, khong co fallback
        // hop le vi USDT amount_in KHONG phai don vi BNB).
        let rows = vec![json!({
            "event": "sim.result", "token": "0xtoken2", "quote": "usdt",
            "profit_gross_wei": 1i64, "profit_net_wei": 1i64,
        })];
        let econ = compute_econ_from_rows(&rows, None);
        assert_eq!(econ["by_quote"]["usdt"], 1);
        let total_bucket_count: i64 = econ["buckets_bnb"].as_array().unwrap().iter().map(|b| b["count"].as_i64().unwrap()).sum();
        assert_eq!(total_bucket_count, 0, "thieu amount_in_bnb_equiv thi khong bucket duoc, khong doan mo");
    }

    /// ĐẠT CẦN DÁN (cụm `econ-truth-latency-vps`, mục 1) — dòng USDT CÓ
    /// `amount_in_bnb_equiv` (đã quy đổi qua reserve WBNB/USDT thật ở
    /// `main.rs`) PHẢI bucket được vào `buckets_bnb`, và `sum_net_pos_bnb`
    /// phải dùng ĐÚNG tỉ giá suy ra từ `amount_in`/`amount_in_bnb_equiv` của
    /// chính dòng đó (không phải price oracle) — trước cụm này, MỌI candidate
    /// quote USDT (BAOCAO39: 3279 candidate trong 60 phút) hoàn toàn vắng
    /// mặt khỏi `buckets_bnb`.
    #[test]
    fn compute_econ_usdt_quote_with_bnb_equiv_field_is_bucketed_with_correct_rate() {
        // 500 USDT ~= 0.8 BNB (ty gia 625 USDT/BNB gia du) -> rate = 0.8/500 = 0.0016.
        let amount_in_usdt = 500.0 * 1e18;
        let amount_in_bnb = 0.8 * 1e18;
        let profit_net_usdt_wei: i64 = 5_000_000_000_000_000_000; // 5 USDT loi
        let rows = vec![json!({
            "event": "sim.result", "token": "0xtoken3", "pair": "0xpair3", "quote": "usdt",
            "amount_in": format!("{amount_in_usdt}"),
            "amount_in_bnb_equiv": format!("{amount_in_bnb}"),
            "profit_gross_wei": profit_net_usdt_wei,
            "profit_net_wei": profit_net_usdt_wei,
        })];
        let econ = compute_econ_from_rows(&rows, None);
        let bucket_02_1 = econ["buckets_bnb"].as_array().unwrap().iter().find(|b| b["bucket"] == "0.2-1").unwrap();
        assert_eq!(bucket_02_1["count"], 1, "0.8 BNB-equivalent phai roi dung bucket 0.2-1");
        assert_eq!(bucket_02_1["net_pos"], 1);
        // sum_net_pos_bnb = 5 USDT * rate(0.0016) = 0.008 BNB-equivalent.
        assert!((bucket_02_1["sum_net_pos_bnb"].as_f64().unwrap() - 0.008).abs() < 1e-6);
        assert_eq!(econ["top_pools"][0]["pair"], "0xpair3");
        assert!((econ["top_pools"][0]["sum_net_bnb"].as_f64().unwrap() - 0.008).abs() < 1e-6);
    }

    /// ĐẠT CẦN DÁN (cụm `econ-truth-latency-vps`, mục 1) — `profit_net_wei`/
    /// `profit_gross_wei` dạng String VƯỢT `i64::MAX` (định dạng THẬT sau
    /// fix bug panic `serde_json::json!`, xem `pipeline::log_outcome_v2`)
    /// vẫn phải bucket ĐÚNG, không rơi mất qua `as_i64()` (sẽ trả `None` cho
    /// giá trị này nếu code cũ chưa đổi sang `parse_profit_wei`).
    #[test]
    fn compute_econ_profit_over_i64_max_as_string_still_buckets_correctly() {
        let big_profit = "17579175023944993805"; // that, > i64::MAX, dang String
        let rows = vec![json!({
            "event": "sim.result", "pair": "0xpairbig", "quote": "wbnb",
            "amount_in": wei(1.0), "amount_in_bnb_equiv": wei(1.0),
            "profit_gross_wei": big_profit, "profit_net_wei": big_profit,
        })];
        let econ = compute_econ_from_rows(&rows, None);
        let bucket = econ["buckets_bnb"].as_array().unwrap().iter().find(|b| b["bucket"] == ">=1").unwrap();
        assert_eq!(bucket["net_pos"], 1, "profit lon (String, vuot i64::MAX) phai duoc dem, khong roi mat");
        assert!((bucket["sum_net_pos_bnb"].as_f64().unwrap() - 17.579175023944993805).abs() < 1e-6);
    }

    /// ĐẠT CẦN DÁN (cụm `bugfix-presign-and-contract-plan`, A1) — FIXTURE
    /// dựng từ CHÍNH dòng `sim.result` THẬT Chủ chỉ ra (VPS, paper 24h port
    /// 18910): hash `0x9a248ea3a185744c1d6bf61f36fe1c42eb044e59a56838ad18089b20eff47922`,
    /// `quote=usdt`, `profit_net_wei=219550516598821986047` (= 219.55 USDT).
    /// Ở tỉ giá THẬT quan sát được trong `logs/bot.jsonl` cùng khung giờ
    /// (~720–732 USDT/BNB, suy từ chính cặp `amount_in`/`amount_in_bnb_equiv`
    /// của các dòng USDT khác), 219.55 USDT ≈ **0.3 BNB** — KHÔNG PHẢI 55803
    /// BNB như `/api/econ` báo trước bản sửa.
    #[test]
    fn compute_econ_real_vps_usdt_row_converts_to_about_0_3_bnb() {
        // Ty gia dung: 1000 USDT = 1.366 BNB (~732 USDT/BNB).
        let rows = vec![json!({
            "event": "sim.result",
            "hash": "0x9a248ea3a185744c1d6bf61f36fe1c42eb044e59a56838ad18089b20eff47922",
            "from": "0xaabae02d453823e0ce3c86f8a1d29d3da0a3eaf7",
            "pair": "0xcec13213c390d51121f82ba2ecafb8e11e0af7a3",
            "quote": "usdt",
            "amount_in": wei(1000.0),
            "amount_in_bnb_equiv": wei(1.366),
            "profit_gross_wei": "219550516598821986047",
            "profit_net_wei": "219550516598821986047",
        })];
        let econ = compute_econ_from_rows(&rows, None);
        let best = econ["best_net_bnb"].as_f64().expect("phai bucket duoc, khong duoc null");
        assert!(
            (best - 0.3).abs() < 0.005,
            "219.55 USDT phai ra ~0.3 BNB (ty gia ~732 USDT/BNB), nhan duoc {best} — chieu quy doi USDT->BNB sai"
        );
        assert_eq!(econ["rate_inverted_rejected"], 0, "dong nay ty gia HOP LE, khong duoc bi loai");
        assert!((econ["top_pools"][0]["sum_net_bnb"].as_f64().unwrap() - 0.3).abs() < 0.005);
    }

    /// ĐẠT CẦN DÁN (A1) — CÙNG dòng trên nhưng `amount_in_bnb_equiv` ĐẢO
    /// CHIỀU (dạng THẬT do binary trước bản sửa ghi ra: 1000 USDT ->
    /// "732 BNB"). Tỉ giá suy ra = 732 > 1.0, bất khả thi cho quote USDT ->
    /// PHẢI bị loại (`rate_rejected=1`), KHÔNG được cộng vào `best_net_bnb`
    /// (trước bản sửa: 219.55 × 732 = **160.700 BNB** báo lên dashboard).
    #[test]
    fn compute_econ_inverted_usdt_rate_row_is_rejected_not_astronomical() {
        let rows = vec![json!({
            "event": "sim.result",
            "pair": "0xcec13213c390d51121f82ba2ecafb8e11e0af7a3",
            "quote": "usdt",
            "amount_in": wei(1000.0),
            "amount_in_bnb_equiv": wei(732_000.0), // DAO CHIEU: nhan thay vi chia
            "profit_gross_wei": "219550516598821986047",
            "profit_net_wei": "219550516598821986047",
        })];
        let econ = compute_econ_from_rows(&rows, None);
        assert_eq!(econ["rate_inverted_rejected"], 1, "ty gia USDT->BNB > 1.0 la bat kha thi, phai bi loai");
        assert!(econ["best_net_bnb"].is_null(), "khong duoc bao so lai thien van tu dong log hong");
        let total_bucket_count: i64 = econ["buckets_bnb"].as_array().unwrap().iter().map(|b| b["count"].as_i64().unwrap()).sum();
        assert_eq!(total_bucket_count, 0, "dong ty gia hong khong duoc bucket");
        assert_eq!(econ["candidate"], 1, "van dem la candidate (khong giau dong do khoi tong)");
    }

    /// ĐẠT CẦN DÁN (A6) — bucket theo VỐN CẦN (`front_in`) + "vốn để lấy 80%
    /// tổng lãi" + đếm riêng nhóm cụm đối thủ + cột `competitor_touched`.
    #[test]
    fn compute_econ_front_in_buckets_capital80_and_competitor_split() {
        let rows = vec![
            // 3 co hoi WBNB: von 0.02/0.3/2.0 BNB, lai 0.01/0.05/0.004 BNB.
            json!({"event":"sim.result","quote":"wbnb","pair":"0xp1",
                   "amount_in": wei(0.5), "amount_in_bnb_equiv": wei(0.5),
                   "front_in_wei": wei(0.02), "profit_gross_wei": wei(0.01), "profit_net_wei": wei(0.01)}),
            json!({"event":"sim.result","quote":"wbnb","pair":"0xp2",
                   "amount_in": wei(3.0), "amount_in_bnb_equiv": wei(3.0),
                   "front_in_wei": wei(0.3), "profit_gross_wei": wei(0.05), "profit_net_wei": wei(0.05)}),
            json!({"event":"sim.result","quote":"wbnb","pair":"0xp3",
                   "amount_in": wei(9.0), "amount_in_bnb_equiv": wei(9.0),
                   "front_in_wei": wei(2.0), "profit_gross_wei": wei(0.004), "profit_net_wei": wei(0.004),
                   "victim_in_competitor_cluster": true}),
            // compete.result: pool p1 co tx lien ke cua dia chi khac cham cung pool
            json!({"event":"compete.result","pair":"0xp1","competitor":"0xbeef"}),
        ];
        let econ = compute_econ_from_rows(&rows, None);

        let fb = econ["buckets_front_in_bnb"].as_array().unwrap();
        let get = |label: &str| fb.iter().find(|b| b["bucket"] == label).unwrap().clone();
        assert_eq!(get("0.01-0.05")["count"], 1, "von 0.02 BNB -> bucket 0.01-0.05");
        assert_eq!(get("0.2-1")["count"], 1, "von 0.3 BNB -> bucket 0.2-1");
        assert_eq!(get(">=1")["count"], 1, "von 2.0 BNB -> bucket >=1");

        // Tong lai = 0.064; 80% = 0.0512. Sap theo von tang dan:
        // 0.02 (lai 0.01, cong don 0.010) -> 0.3 (lai 0.05, cong don 0.060 >= 0.0512, DUNG).
        let cap = &econ["capital_for_80pct_profit"]["wbnb"];
        assert_eq!(cap["opportunities"], 3);
        assert_eq!(cap["taken_for_80pct"], 2);
        assert!((cap["capital_needed_native"].as_f64().unwrap() - 0.3).abs() < 1e-9, "von can = 0.3 BNB");

        let comp = &econ["competitor"];
        assert_eq!(comp["candidate"], 1);
        assert_eq!(comp["simulated"], 1);
        assert!((comp["sum_net_bnb"].as_f64().unwrap() - 0.004).abs() < 1e-9);

        // A6 — pool CO LAI phai xuat hien o `top_pools_by_net` (bang dung cho
        // go/no-go), ke ca khi khong lot top-10 theo count.
        let by_net = econ["top_pools_by_net"].as_array().unwrap();
        assert_eq!(by_net.len(), 3, "3 pool deu co net_pos>0");
        assert_eq!(by_net[0]["pair"], "0xp2", "pool lai nhieu nhat (0.05) dung dau");
        assert_eq!(by_net[0]["competitor_touched"], false);
        assert_eq!(by_net.iter().find(|p| p["pair"] == "0xp3").unwrap()["competitor_touched"], true);

        let pools = econ["top_pools"].as_array().unwrap();
        let p1 = pools.iter().find(|p| p["pair"] == "0xp1").unwrap();
        let p2 = pools.iter().find(|p| p["pair"] == "0xp2").unwrap();
        let p3 = pools.iter().find(|p| p["pair"] == "0xp3").unwrap();
        assert_eq!(p1["competitor_touched"], true, "compete.result danh dau pool p1");
        assert_eq!(p2["competitor_touched"], false);
        assert_eq!(p3["competitor_touched"], true, "victim chinh la vi cua cum");
    }

    #[test]
    fn compute_econ_empty_rows_no_panic() {
        let econ = compute_econ_from_rows(&[], None);
        assert_eq!(econ["candidate"], 0);
        assert_eq!(econ["net_pos_total"], 0);
        assert!(econ["best_net_bnb"].is_null());
        assert!(econ["latency_ms"]["p50"].is_null());
    }

    /// ĐẠT CẦN DÁN — phát hiện THẬT lúc verify 60 phút (BAOCAO38):
    /// `logs/bot.jsonl` dùng CHUNG qua mọi lần boot (không bị xoá), nên
    /// `/api/econ` PHẢI lọc theo `since_ts` (`boot_wall_clock`) để không lẫn
    /// số liệu của các lần chạy TRƯỚC — thiếu lọc, `candidate` bị thổi phồng
    /// (quan sát thật: 87225 thay vì 49787 của riêng 60 phút đó).
    #[test]
    fn compute_econ_since_ts_excludes_rows_from_previous_runs() {
        let rows = vec![
            json!({"event": "tx.skip", "reason": "decode_fail", "ts": "2026-09-15T09:00:00+00:00"}), // lan chay TRUOC
            json!({"event": "tx.skip", "reason": "decode_fail", "ts": "2026-09-15T10:16:09+00:00"}), // lan chay HIEN TAI
            json!({"event": "tx.skip", "reason": "unprofitable", "ts": "2026-09-15T10:20:00+00:00"}), // lan chay HIEN TAI
        ];
        let boot_ts = "2026-09-15T10:16:08+00:00";
        let econ_filtered = compute_econ_from_rows(&rows, Some(boot_ts));
        assert_eq!(econ_filtered["candidate"], 2, "chi 2 dong co ts >= boot_wall_clock");

        let econ_unfiltered = compute_econ_from_rows(&rows, None);
        assert_eq!(econ_unfiltered["candidate"], 3, "khong loc thi tinh ca dong lan chay truoc");
    }

    #[test]
    fn router_display_name_matches_all_5_pinned_routers() {
        assert_eq!(router_display_name(crate::venues::V2_ROUTER_ADDRESS), "V2 Router");
        assert_eq!(router_display_name("0x1b81D678ffb9C0263b24A97847620C99d213eB14"), "SwapRouter");
        assert_eq!(router_display_name("0x13f4EA83D0bd40E75C8222255bc855a974568Dd4"), "SmartRouter");
        assert_eq!(router_display_name("0x1A0A18AC4BECDDbd6389559687d1A73d8927E416"), "UR v3 (cu)");
        assert_eq!(router_display_name("0xd9C500DfF816a1Da21A48A732d3498Bf09dc9AEB"), "UR Infinity");
        assert_eq!(router_display_name("0x0000000000000000000000000000000000dead"), "other");
    }
}

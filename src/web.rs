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
#[derive(Debug, Default)]
pub struct ValidateStats {
    rows: std::collections::VecDeque<Value>,
    pub total: u64,
    pub within_1pct: u64,
}

impl ValidateStats {
    pub fn push(&mut self, row: Value, ok: bool) {
        self.total += 1;
        if ok {
            self.within_1pct += 1;
        }
        self.rows.push_back(row);
        while self.rows.len() > 50 {
            self.rows.pop_front();
        }
    }
    pub fn snapshot(&self) -> Value {
        json!({
            "total": self.total,
            "within_1pct": self.within_1pct,
            "within_1pct_ratio": if self.total > 0 { self.within_1pct as f64 / self.total as f64 } else { 0.0 },
            "rows": self.rows.iter().cloned().collect::<Vec<_>>(),
        })
    }
}

async fn validate_list(State(state): State<AppState>) -> Json<Value> {
    Json(state.validate_log.read().await.snapshot())
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
/// - `rpc_error`: DÀNH SẴN, LUÔN `0` phiên này — `pipeline::resolve_v2_reserves`
///   gộp "không có pool" VÀ "eth_call lỗi mạng" thành CÙNG 1 `PipelineSkip::NoPool`
///   (quyết định có chủ đích từ `5.1`, xem `docs/STATE.md`) nên chưa có tín
///   hiệu nào tách riêng lỗi RPC khỏi "chắc chắn không pool" — không bịa số,
///   để `0` + ghi rõ lý do trong BAOCAO thay vì giả vờ đã đo được.
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
        });
        out
    }
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
        .route("/api/validate", get(validate_list))
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
async fn pairs(State(state): State<AppState>) -> Json<Value> {
    let book = state.pairbook.read().await;
    let entries: Vec<Value> = book
        .entries()
        .map(|e| {
            json!({
                "pair_addr": format!("{:#x}", e.pair_addr),
                "source_line": e.source_line,
                "resolved_from": e.resolved_from.as_str(),
            })
        })
        .collect();
    Json(json!({
        "count": book.len(),
        "error_lines": book.error_lines,
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

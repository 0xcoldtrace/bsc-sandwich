//! Cụm pair-mode — `PairBook` theo dõi các POOL (token/WBNB) chỉ định trực
//! tiếp qua `pairs.txt`, KHÁC `VictimBook` (theo dõi ĐỊA CHỈ VÍ). Bất kỳ tx
//! nào chạm đúng pool đã liệt kê ở đây đều là candidate "pair mode" — không
//! cần biết trước địa chỉ ví nào sẽ giao dịch trên pool đó (khác victims.txt).
//!
//! Định dạng 1 dòng `pairs.txt`:
//! - `0xAddress` (không dấu phẩy) — CHƯA rõ đây là địa chỉ TOKEN hay PAIR
//!   luôn. Resolve bằng `Factory.getPair(addr, WBNB)`: trả về != 0x0 ->
//!   `addr` là TOKEN (pair = kết quả); trả về 0x0 -> `addr` TỰ NÓ là pair
//!   address (không cần resolve thêm bước nào khác — tránh phải phân biệt
//!   "pair vs token" bằng heuristic ngoài chuỗi, đúng ý lệnh).
//! - `tokenAddr,WBNB` hoặc `tokenAddr,USDT` — resolve thẳng `getPair(tokenAddr,
//!   quote)`. Địa chỉ thứ 2 PHẢI đúng WBNB HOẶC USDT đã pin
//!   (`venues::WBNB_ADDRESS`/`venues::USDT_ADDRESS` — cụm `hotpath-fix-then-decoder-ur`,
//!   A1: trước đó chỉ nhận WBNB, khiến 7 dòng `pairs.txt` dùng quote USDT rơi
//!   vào `error_lines`), sai địa chỉ khác -> lỗi dòng, skip.
//!
//! Global `min_swap` (`config.toml::pairs_min_swap_bnb`) áp dụng cho MỌI
//! entry — không có `min_swap` riêng theo từng pool (khác `victims.txt`, nơi
//! mỗi ví có ngưỡng riêng).
//!
//! Cụm `strategy-lock-mode2` (Chủ chốt 2026-09-15) — mỗi dòng còn mang 1
//! comment VET TAY sau `#`: `SYMBOL | vetted YYYY-MM-DD | tax b/s | owner
//! renounced|active | note`. Field `vetted YYYY-MM-DD` là điều kiện DUY NHẤT
//! `PairBook` đọc (các field còn lại chỉ để Chủ tự đối chiếu bằng mắt, không
//! parse). Thiếu `vetted`/ngày không hợp lệ -> `vetted_at = None` -> khi
//! `pairs_require_vetted=true` (ship) entry đó KHÔNG vào map, không bao giờ
//! thành candidate (`not_in_list`), xem `reload`.

use alloy::primitives::Address;
use alloy::providers::DynProvider;
use chrono::NaiveDate;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

use crate::logger::BotLogger;
use crate::pool;
use crate::venues::{USDT_ADDRESS, WBNB_ADDRESS};

/// Resolve tối đa 10 dòng đồng thời (CLAUDE.md lệnh pair-mode), timeout 3s/dòng.
const MAX_CONCURRENT_RESOLVE: usize = 10;
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(3);

fn wbnb() -> Address {
    Address::from_str(WBNB_ADDRESS).expect("WBNB_ADDRESS da pin phai hop le")
}

/// Cụm `hotpath-fix-then-decoder-ur` (A1) — USDT parse ngang hàng WBNB.
fn usdt() -> Address {
    Address::from_str(USDT_ADDRESS).expect("USDT_ADDRESS da pin phai hop le")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedFrom {
    /// Dòng "0xAddress" mà `getPair(addr, WBNB)` trả `0x0` -> addr TỰ NÓ là pair.
    Direct,
    /// Dòng là (hoặc resolve ra) địa chỉ TOKEN, pair lấy từ `getPair`.
    Token,
}

impl ResolvedFrom {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResolvedFrom::Direct => "direct",
            ResolvedFrom::Token => "token",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PairEntry {
    pub pair_addr: Address,
    pub source_line: String,
    pub resolved_from: ResolvedFrom,
    /// Cụm `strategy-lock-mode2` — ngày Chủ vet tay (`vetted YYYY-MM-DD`
    /// trong comment), `None` = CHƯA VET. Xem doc-comment đầu file.
    pub vetted_at: Option<NaiveDate>,
    /// Cụm `hotpath-fix-then-decoder-ur` (A1) — quote asset THẬT của pool này
    /// (WBNB hoặc USDT). Dòng "0xAddress" trần (không dấu phẩy, cả `Token` lẫn
    /// `Direct`) LUÔN `wbnb()` (giữ đúng hành vi cũ — chỉ dòng `token,quote`
    /// tường minh mới có thể là USDT). `is_tax_ok`/`knows_pool`/`is_vet_failed`
    /// vẫn khoá theo `pair_addr` (không đổi) — field này chỉ phục vụ
    /// `tokens_to_vet`/`known_pair` cần biết ĐÚNG quote để gọi RPC.
    pub quote: Address,
    /// Cụm `econ-truth-latency-vps` (mục 1) — SYMBOL đọc từ comment
    /// (`# SYMBOL | vetted ...`, field ĐẦU TIÊN trước dấu `|`), chỉ phục vụ
    /// hiển thị (`GET /api/econ` `top_pools`, `GET /api/pairs`) — KHÔNG dùng
    /// cho bất kỳ quyết định nào. `None` khi comment rỗng/không có field nào
    /// trước `|` đầu tiên (không bịa symbol).
    pub symbol: Option<String>,
}

/// Cụm `econ-truth-latency-vps` (mục 0.a) — trạng thái 1 dòng `pairs.txt`
/// CHƯA resolve xong (RPC lỗi/timeout, hoặc "no pool" tạm thời) — giữ qua
/// NHIỀU lần `reload()` với backoff tăng dần, KHÔNG rớt khỏi
/// `PairBook`/candidate list nếu trước đó ĐÃ TỪNG resolve thành công (xem
/// `LineState`/`reload`).
#[derive(Debug, Clone)]
pub struct PendingRetry {
    pub attempts: u32,
    pub last_attempt: Instant,
    pub last_error: String,
}

/// Backoff 5s/15s/60s (CLAUDE.md lệnh mục 0.a) theo số lần lỗi LIÊN TIẾP đã
/// có (`attempts`, 0 = chưa từng thử) — THUẦN, test được không cần `Instant`
/// thật trôi qua.
fn retry_backoff(attempts: u32) -> Duration {
    match attempts {
        0 => Duration::ZERO,
        1 => Duration::from_secs(5),
        2 => Duration::from_secs(15),
        _ => Duration::from_secs(60),
    }
}

#[derive(Debug, Clone)]
enum LineState {
    Resolved(PairEntry),
    Pending(PendingRetry),
}

/// Khoá 1 dòng `pairs.txt` xuyên suốt nhiều lần `reload()` — dòng "0xAddress"
/// trần dùng `(addr, wbnb())` (quote ngầm định), dòng "token,quote" dùng
/// chính `(token, quote)` đã khai báo. Đây là khoá DUY NHẤT quyết định "dòng
/// này đã từng resolve/đang chờ retry" — KHÁC `token_quote_to_pair` (chỉ
/// index cho entry ĐÃ resolve xong, phục vụ tra cứu nhanh ở hot path).
type LineKey = (Address, Address);

fn parse_symbol_from_comment(comment: &str) -> Option<String> {
    let first = comment.split('|').next()?.trim();
    if first.is_empty() {
        None
    } else {
        Some(first.to_string())
    }
}

/// Cụm `strategy-lock-mode2` — tách field `vetted YYYY-MM-DD` khỏi comment
/// cuối dòng `pairs.txt` (sau dấu `#`), định dạng `SYMBOL | vetted
/// YYYY-MM-DD | tax b/s | owner ... | note` (các field khác `|` không được
/// parse, chỉ để Chủ đối chiếu bằng mắt). Không có field bắt đầu bằng
/// "vetted", hoặc có nhưng KHÔNG kèm ngày hợp lệ (`vetted` để trống, hoặc gõ
/// sai định dạng ngày) -> `None`, đúng nghĩa "CHƯA VET" (an toàn, không đoán).
fn parse_vetted_from_comment(comment: &str) -> Option<NaiveDate> {
    for field in comment.split('|') {
        let field = field.trim();
        if let Some(rest) = field.strip_prefix("vetted") {
            let date_str = rest.trim();
            if date_str.is_empty() {
                return None;
            }
            return NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok();
        }
    }
    None
}

/// Cụm `strategy-lock-mode2` — kết quả đo lại bằng `sim_evm::measure_tax_evm`
/// (task nền `pairs_vet_task`, `src/main.rs`), KHÁC `vetted_at` (ngày Chủ tự
/// vet tay ghi trong `pairs.txt`). Đây là lớp XÁC NHẬN THỨ 2, độc lập —
/// `vetted_at` cho phép entry vào map, `VetResult` (qua `set_vet_result`) có
/// thể loại nó khỏi candidate LẠI nếu revm đo ra tax/honeypot thật, KHÔNG
/// đụng `pairs.txt` của Chủ.
#[derive(Debug, Clone, Copy)]
pub struct VetResult {
    pub buy_bps: u32,
    pub sell_bps: u32,
    pub honeypot: bool,
    pub block: u64,
}

#[derive(Debug, Default)]
pub struct PairBook {
    /// Cụm `econ-truth-latency-vps` (0.a) — trạng thái BỀN qua nhiều lần
    /// `reload()`, khoá theo `LineKey` (xem doc-comment type đó): nguồn sự
    /// thật DUY NHẤT quyết định dòng nào đã resolve/đang chờ. `pairs`/
    /// `token_quote_to_pair` bên dưới được TÁI DỰNG từ đây mỗi lần `reload()`
    /// (chỉ chứa các entry `Resolved`), giữ để không phải sửa mọi call site
    /// đang dùng 2 map đó.
    line_state: HashMap<LineKey, LineState>,
    pairs: HashMap<Address, PairEntry>,
    /// Cụm `strategy-lock-mode2` — kết quả vet nền gần nhất/thời điểm đo,
    /// khoá theo `pair_addr` (cùng khoá với `pairs`).
    vet_results: HashMap<Address, (VetResult, Instant)>,
    /// Cụm `strategy-lock-mode2` — tập `pair_addr` bị `pairs_vet_task` loại
    /// khỏi candidate (tax > ngưỡng hoặc honeypot đo bằng EVM thật). Entry
    /// vẫn nằm trong `pairs` (vẫn hiện trên `/api/pairs`) nhưng `contains()`
    /// trả `false` — không thành candidate cho tới lần vet PASS kế tiếp.
    vet_failed: std::collections::HashSet<Address>,
    /// Cụm `hotpath-fix-then-decoder-ur` (A4a) — reverse index `(token, quote)
    /// -> pair_addr` cho MỌI entry `resolved_from=Token` (token biết được từ
    /// `source_line`) — cho phép `main.rs` bỏ qua `Factory.getPair` (1
    /// `eth_call`) khi token đã có sẵn trong `pairs.txt`, chỉ còn cần
    /// `getReserves` (xem `known_pair`). Khoá theo CẢ `quote` (không chỉ
    /// `token`) vì 1 token có thể xuất hiện 2 lần với 2 quote asset khác nhau
    /// (vd CAKE có cả pool WBNB lẫn USDT trong `pairs.txt`).
    token_quote_to_pair: HashMap<(Address, Address), Address>,
    pub error_lines: u64,
    pub last_reload: Option<Instant>,
}

enum ParsedLine {
    /// "0xAddress" trần — chưa biết token hay pair, quote LUÔN WBNB.
    Direct(Address),
    /// "tokenAddr,quote" — đã biết chắc là token, quote = WBNB hoặc USDT.
    TokenQuote(Address, Address),
}

fn parse_pairs_line(line: &str) -> Result<ParsedLine, String> {
    if let Some(comma) = line.find(',') {
        let (addr_str, rest) = line.split_at(comma);
        let rest = &rest[1..];
        let token = Address::from_str(addr_str.trim())
            .map_err(|e| format!("token address khong hop le '{addr_str}': {e}"))?;
        let quote_candidate = Address::from_str(rest.trim())
            .map_err(|e| format!("dia chi thu 2 khong hop le '{rest}': {e}"))?;
        if quote_candidate != wbnb() && quote_candidate != usdt() {
            return Err(format!(
                "dia chi thu 2 '{rest}' khong phai WBNB ({WBNB_ADDRESS}) hoac USDT ({USDT_ADDRESS}) da pin"
            ));
        }
        Ok(ParsedLine::TokenQuote(token, quote_candidate))
    } else {
        let addr = Address::from_str(line.trim()).map_err(|e| format!("address khong hop le '{line}': {e}"))?;
        Ok(ParsedLine::Direct(addr))
    }
}

/// Trait tách nguồn resolve `getPair(token, WBNB)` khỏi `PairBook` — sản xuất
/// dùng `RpcPairResolver` (gọi `pool::resolve_v2_pair` thật qua RPC), test
/// dùng 1 resolver giả lập (mock, không cần RPC sống) để `pairbook_load_*`
/// chạy được trong `cargo test` mặc định (đúng quy ước repo: test không bắt
/// RPC sống trừ khi `#[ignore]`).
pub trait PairResolver: Clone + Send + Sync + 'static {
    /// Cụm `hotpath-fix-then-decoder-ur` (A1) — thêm tham số `quote` (WBNB
    /// hoặc USDT) — trước đó hardcode WBNB, khiến mọi dòng `token,USDT` không
    /// thể resolve đúng pool. Mọi implementor/test caller cũ phải truyền
    /// `quote` tường minh (dòng "0xAddress" trần luôn dùng `wbnb()`, xem
    /// `parse_pairs_line`/`reload`).
    fn get_pair(&self, token: Address, quote: Address) -> impl std::future::Future<Output = Result<Address, String>> + Send;

    /// Cụm `econ-truth-latency-vps` (0.b) — nhãn URL (đã redact) đang dùng để
    /// resolve, đính kèm vào log `pair.resolve_fail`. Mặc định rỗng (test
    /// resolver không cần override).
    fn url_label(&self) -> String {
        String::new()
    }
}

/// Resolver PRODUCTION — bọc `DynProvider` (owned, `Clone` rẻ vì bên trong là
/// `Arc`) + factory V2 đã pin, dùng lại `pool::resolve_v2_pair` (không tự
/// viết lại `eth_call`). `Address::ZERO` nghĩa là "không có pool" (khớp
/// `PoolSkipReason::NoPool`), KHÁC lỗi RPC thật (`Err`).
#[derive(Clone)]
pub struct RpcPairResolver {
    pub provider: DynProvider,
    pub factory: Address,
    /// Cụm `econ-truth-latency-vps` (0.b) — nhãn URL (đã redact, xem
    /// `transport::RpcPool::current_url_label`) ĐANG dùng cho `provider` này
    /// — chỉ phục vụ đính kèm log `pair.resolve_fail`, không ảnh hưởng logic
    /// resolve. Rỗng khi không xác định được (vd test dùng resolver giả).
    pub url_label: String,
}

impl PairResolver for RpcPairResolver {
    async fn get_pair(&self, token: Address, quote: Address) -> Result<Address, String> {
        match pool::resolve_v2_pair_for_quote(&self.provider, self.factory, token, quote).await {
            Ok(Ok(pair)) => Ok(pair),
            Ok(Err(_)) => Ok(Address::ZERO),
            Err(e) => Err(e),
        }
    }

    fn url_label(&self) -> String {
        self.url_label.clone()
    }
}

impl PairBook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Tra O(1) trên hot path (`pipeline::decide_paper_v2`) — tx chạm đúng
    /// pool nào trong danh sách này là candidate "pair mode". Cụm
    /// `strategy-lock-mode2` — entry bị `pairs_vet_task` đánh dấu
    /// `vet_failed` (tax/honeypot đo lại bằng EVM thật) KHÔNG còn là
    /// candidate cho tới lần vet PASS kế tiếp, dù vẫn còn trong `pairs.txt`.
    pub fn contains(&self, pair: &Address) -> bool {
        self.pairs.contains_key(pair) && !self.vet_failed.contains(pair)
    }

    /// Cụm `real-economics-mode2` — mục 0 (fix bug BAOCAO37:
    /// `honeypot_or_tax=95/phút` với `sim_engine="v2"`): token có trong
    /// `PairBook` VÀ ĐÃ vet tay (`vetted_at=Some`, đọc thẳng field đó — độc
    /// lập với `pairs_require_vetted`, không chỉ dựa `contains()` vì cờ đó
    /// có thể tắt) VÀ KHÔNG nằm trong `vet_failed` (vet nền vẫn có thể loại
    /// pool bất kỳ lúc nào) → coi là "tax OK", đường nóng KHÔNG cần tra
    /// `TaxCache` (cache đó chỉ còn ý nghĩa khi `pairs_require_vetted=false`,
    /// xem CLAUDE.md mục "Chiến lược đã chốt"/"BUG cổng tax"). `pair` KHÔNG
    /// có trong map (vd wallet-mode/universal-mode) → `false`, giữ nguyên
    /// hành vi tra `TaxCache` cũ cho 2 nhánh đó.
    pub fn is_tax_ok(&self, pair: &Address) -> bool {
        self.pairs.get(pair).map(|e| e.vetted_at.is_some()).unwrap_or(false) && !self.vet_failed.contains(pair)
    }

    /// Cụm `real-economics-mode2` — `true` khi `pair` CÓ trong `pairs.txt`
    /// (bất kể `vet_failed` hay không) — KHÁC `contains()` (loại trừ
    /// `vet_failed`). Dùng để pipeline VẪN route pool vet_failed vào nhánh
    /// "pair" (trả `honeypot_or_tax` rõ ràng, giữ visibility) thay vì để nó
    /// rơi im lặng xuống `not_in_list`/`universal`.
    pub fn knows_pool(&self, pair: &Address) -> bool {
        self.pairs.contains_key(pair)
    }

    /// Cụm `real-economics-mode2` — `true` khi vet nền (`pairs_vet_task`) đã
    /// loại `pair` khỏi candidate (tax/honeypot đo bằng EVM thật).
    pub fn is_vet_failed(&self, pair: &Address) -> bool {
        self.vet_failed.contains(pair)
    }

    /// Cụm `strategy-lock-mode2` — kết quả vet nền gần nhất cho 1 pool
    /// (`buy_bps`/`sell_bps`/`honeypot`/`block`) + số giây từ lúc đo, dùng cho
    /// `GET /api/pairs`. `None` = chưa từng vet nền lần nào.
    pub fn vet_result(&self, pair: &Address) -> Option<(VetResult, u64)> {
        self.vet_results.get(pair).map(|(r, t)| (*r, t.elapsed().as_secs()))
    }

    /// Cụm `strategy-lock-mode2` — ghi kết quả `pairs_vet_task` vừa đo được
    /// cho 1 pool. `ok=false` (tax > ngưỡng HOẶC honeypot) -> thêm vào
    /// `vet_failed` (loại khỏi candidate ngay từ lần đọc `contains()` kế
    /// tiếp); `ok=true` -> xoá khỏi `vet_failed` nếu trước đó có (cho phép
    /// quay lại candidate ở lần vet sau nếu đã hết vấn đề).
    pub fn set_vet_result(&mut self, pair_addr: Address, result: VetResult, ok: bool) {
        self.vet_results.insert(pair_addr, (result, Instant::now()));
        if ok {
            self.vet_failed.remove(&pair_addr);
        } else {
            self.vet_failed.insert(pair_addr);
        }
    }

    /// Cụm `strategy-lock-mode2` — danh sách `(pair_addr, token_addr)` cần
    /// `pairs_vet_task` đo lại: MỌI entry đã có `vetted_at` (Chủ đã vet tay)
    /// VÀ biết được địa chỉ TOKEN riêng (`resolved_from=Token` — dòng gốc là
    /// `tokenAddr` hoặc `tokenAddr,WBNB`, `source_line` bắt đầu bằng đúng địa
    /// chỉ token đó). Entry `resolved_from=Direct` (dòng gốc TỰ NÓ là địa chỉ
    /// PAIR, không phải token) bị bỏ qua ở đây — không đủ thông tin để gọi
    /// `measure_tax_evm(token, ...)` mà không thêm 1 `eth_call`
    /// `token0()/token1()` (ngoài phạm vi cụm này, ghi rõ thay vì đoán).
    /// Cụm `hotpath-fix-then-decoder-ur` (A1) — trả kèm `quote` (WBNB hoặc
    /// USDT, từ `PairEntry::quote`) — trước đó chỉ trả `(pair_addr, token)`,
    /// khiến `pairs_vet_task` hardcode `quote=WBNB` cho MỌI token kể cả 7
    /// pool USDT (đo tax sai pool/luôn lỗi vì pool đó không có phía WBNB).
    pub fn tokens_to_vet(&self) -> Vec<(Address, Address, Address)> {
        self.pairs
            .values()
            .filter(|e| e.vetted_at.is_some() && e.resolved_from == ResolvedFrom::Token)
            .filter_map(|e| {
                let token_str = e.source_line.split(',').next().unwrap_or("").trim();
                Address::from_str(token_str).ok().map(|t| (e.pair_addr, t, e.quote))
            })
            .collect()
    }

    /// Cụm `hotpath-fix-then-decoder-ur` (A4a) — `pair_addr` ĐÃ BIẾT cho
    /// `(token, quote)` từ `pairs.txt` (đã resolve khi `reload`), tránh caller
    /// (`main.rs`) phải gọi lại `Factory.getPair` cho tx chạm token đã có
    /// trong danh sách. `None` khi token/quote này chưa từng resolve thành
    /// công (main.rs vẫn phải tự resolve qua RPC như cũ).
    pub fn known_pair(&self, token: Address, quote: Address) -> Option<Address> {
        self.token_quote_to_pair.get(&(token, quote)).copied()
    }

    pub fn entries(&self) -> impl Iterator<Item = &PairEntry> {
        self.pairs.values()
    }

    pub fn last_reload_sec_ago(&self) -> Option<u64> {
        self.last_reload.map(|t| t.elapsed().as_secs())
    }

    /// Cụm `econ-truth-latency-vps` (0.a) — số dòng ĐANG chờ retry (resolve
    /// chưa xong, đang chờ backoff hoặc chờ lượt resolve tiếp theo) — KHÁC
    /// `error_lines` (parse lỗi thật, không retry).
    pub fn pending_count(&self) -> u64 {
        self.line_state.values().filter(|s| matches!(s, LineState::Pending(_))).count() as u64
    }

    /// Chi tiết từng dòng đang pending: `(token_hoac_addr, quote, attempts,
    /// last_error, last_attempt_sec_ago)` — dùng cho `GET /api/pairs`
    /// ("4 dòng đang lỗi: ghi token + lý do", CLAUDE.md lệnh mục 0 DoD).
    pub fn pending_entries(&self) -> Vec<(Address, Address, u32, String, u64)> {
        self.line_state
            .iter()
            .filter_map(|(key, state)| match state {
                LineState::Pending(p) => Some((key.0, key.1, p.attempts, p.last_error.clone(), p.last_attempt.elapsed().as_secs())),
                _ => None,
            })
            .collect()
    }

    /// Nạp lại toàn bộ `pairs.txt` — resolve tối đa `MAX_CONCURRENT_RESOLVE`
    /// dòng đồng thời qua `tokio::spawn` (resolver `Clone + Send + Sync +
    /// 'static` nên mỗi task giữ 1 bản clone riêng, không tranh chấp
    /// lifetime), timeout `RESOLVE_TIMEOUT`/dòng. Lỗi resolve (timeout/RPC
    /// lỗi) -> log `pair.resolve_fail`, BỎ QUA dòng đó, KHÔNG crash. `now`
    /// truyền từ ngoài để test được (cùng quy ước `VictimBook`/`Config`).
    ///
    /// Cụm `strategy-lock-mode2` — `require_vetted` (từ
    /// `config.toml::pairs_require_vetted`): `true` -> dòng KHÔNG có `vetted
    /// YYYY-MM-DD` hợp lệ trong comment bị loại NGAY (không tốn 1 lần resolve
    /// RPC nào), đếm gộp, log `pair.unvetted` ĐÚNG 1 LẦN/reload (không phải
    /// 1 lần/dòng — tránh spam log khi `pairs.txt` có hàng trăm dòng chưa
    /// vet). `false` -> giữ hành vi cũ (mọi dòng resolve được đều thành
    /// candidate, bất kể `vetted_at`).
    pub async fn reload<R: PairResolver>(
        &mut self,
        content: &str,
        resolver: &R,
        logger: &BotLogger,
        now: Instant,
        require_vetted: bool,
    ) {
        let started = Instant::now();
        // Cum `econ-truth-latency-vps` (0.a) - khac ban cu: gio giu THEM
        // LineKey de doi chieu voi `self.line_state` (trang thai BEN qua
        // nhieu lan reload) - dong nao DA resolve xong tu truoc thi KHONG
        // resolve lai (0 eth_call), dong dang Pending thi cho backoff, chi
        // dong MOI/den han retry moi thuc su goi RPC.
        let mut parsed_lines: Vec<(LineKey, String, ParsedLine, Option<NaiveDate>, Option<String>)> = Vec::new();
        let mut errors = 0u64;
        let mut unvetted = 0u64;
        for raw_line in content.lines() {
            // Cum `foundation-fix-then-real-sim` (A5) - FIX BUG: file that
            // (`pairs.txt`, phien `pairs-discovery`) dung dinh dang
            // "0xAddr # SYMBOL ghi chu" (comment CUOI DONG, khong phai dong
            // rieng) - truoc day `line.trim()` KHONG cat comment nay truoc
            // khi goi `Address::from_str`, khien MOI dong that fail parse
            // (BAOCAO27: `pair.parse_error=900`, BAOCAO26: PHA A "0
            // candidate"). Cat comment cuoi dong TRUOC parse, dung
            // `parse_pairs_line` chi con nhan phan dia chi thuan tuy.
            let line = raw_line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            // Cum `strategy-lock-mode2` - phan COMMENT (sau '#') mang field
            // `vetted YYYY-MM-DD` rieng, tach doc lap voi phan dia chi.
            let comment = raw_line.splitn(2, '#').nth(1).unwrap_or("");
            let vetted_at = parse_vetted_from_comment(comment);
            let symbol = parse_symbol_from_comment(comment);
            match parse_pairs_line(line) {
                Ok(p) => {
                    if require_vetted && vetted_at.is_none() {
                        unvetted += 1;
                        continue;
                    }
                    let key: LineKey = match p {
                        ParsedLine::TokenQuote(t, q) => (t, q),
                        ParsedLine::Direct(a) => (a, wbnb()),
                    };
                    parsed_lines.push((key, line.to_string(), p, vetted_at, symbol));
                }
                Err(e) => {
                    errors += 1;
                    logger.log("pair.parse_error", serde_json::json!({ "error": e, "line": line }));
                }
            }
        }
        if unvetted > 0 {
            logger.log("pair.unvetted", serde_json::json!({ "count": unvetted, "require_vetted": require_vetted }));
        }

        let mut new_line_state: HashMap<LineKey, LineState> = HashMap::new();
        let mut to_resolve: Vec<(LineKey, String, ParsedLine, Option<NaiveDate>, Option<String>, u32)> = Vec::new();

        for (key, line, parsed, vetted_at, symbol) in parsed_lines {
            match self.line_state.get(&key) {
                Some(LineState::Resolved(old_entry)) => {
                    // Da resolve tu truoc - GIU pair_addr/resolved_from/quote
                    // CU, chi cap nhat phan doc tu file MOI (vetted_at/symbol/
                    // source_line co the doi) - KHONG goi RPC lai.
                    let mut updated = old_entry.clone();
                    updated.source_line = line;
                    updated.vetted_at = vetted_at;
                    updated.symbol = symbol;
                    new_line_state.insert(key, LineState::Resolved(updated));
                }
                Some(LineState::Pending(p)) => {
                    if now.saturating_duration_since(p.last_attempt) >= retry_backoff(p.attempts) {
                        to_resolve.push((key, line, parsed, vetted_at, symbol, p.attempts));
                    } else {
                        new_line_state.insert(key, LineState::Pending(p.clone()));
                    }
                }
                None => {
                    to_resolve.push((key, line, parsed, vetted_at, symbol, 0));
                }
            }
        }

        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_RESOLVE));
        let mut handles = Vec::new();
        for (key, line, parsed, vetted_at, symbol, prior_attempts) in to_resolve {
            let sem = semaphore.clone();
            let resolver = resolver.clone();
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire_owned().await.ok();
                let url_label = resolver.url_label();
                // Cum 0.a - phan biet RO 3 tinh huong (truoc day gop chung
                // thanh 1 `None` roi mat het thong tin):
                // (1) resolve THANH CONG (Ok(pair_addr,...))
                // (2) "khong co pool" that (Factory tra 0x0 cho dong
                //     token,quote tuong minh) - VAN cho retry (co the do
                //     factory chua index kip/RPC tra thieu, khong coi la loi
                //     vinh vien) - KHAC ban cu "Direct" fallback (dong tran
                //     tu resolve ra 0x0 la THANH CONG, tu no la pair).
                // (3) loi RPC that (timeout/transport) - retry.
                let outcome: Result<(Address, ResolvedFrom, Address), String> = match parsed {
                    ParsedLine::TokenQuote(token, quote) => {
                        match tokio::time::timeout(RESOLVE_TIMEOUT, resolver.get_pair(token, quote)).await {
                            Ok(Ok(pair)) if pair != Address::ZERO => Ok((pair, ResolvedFrom::Token, quote)),
                            Ok(Ok(_zero)) => Err("factory tra 0x0 (chua co pool cho token,quote nay)".to_string()),
                            Ok(Err(e)) => Err(e),
                            Err(_) => Err(format!("timeout sau {}s", RESOLVE_TIMEOUT.as_secs())),
                        }
                    }
                    ParsedLine::Direct(addr) => {
                        match tokio::time::timeout(RESOLVE_TIMEOUT, resolver.get_pair(addr, wbnb())).await {
                            Ok(Ok(pair)) if pair != Address::ZERO => Ok((pair, ResolvedFrom::Token, wbnb())),
                            Ok(Ok(_zero)) => Ok((addr, ResolvedFrom::Direct, wbnb())),
                            Ok(Err(e)) => Err(e),
                            Err(_) => Err(format!("timeout sau {}s", RESOLVE_TIMEOUT.as_secs())),
                        }
                    }
                };
                (key, line, vetted_at, symbol, prior_attempts, outcome, url_label)
            }));
        }

        for h in handles {
            match h.await {
                Ok((key, line, vetted_at, symbol, prior_attempts, outcome, url_label)) => match outcome {
                    Ok((pair_addr, resolved_from, quote)) => {
                        new_line_state.insert(
                            key,
                            LineState::Resolved(PairEntry { pair_addr, source_line: line, resolved_from, vetted_at, quote, symbol }),
                        );
                    }
                    Err(err) => {
                        // Cum 0.b - log day du token/quote/error/url_label (truoc
                        // day log rong `{}`, mat het thong tin de doi chieu).
                        logger.log(
                            "pair.resolve_fail",
                            serde_json::json!({
                                "token": format!("{:#x}", key.0),
                                "quote": format!("{:#x}", key.1),
                                "error": err,
                                "url_label": url_label,
                                "attempt": prior_attempts + 1,
                            }),
                        );
                        new_line_state
                            .insert(key, LineState::Pending(PendingRetry { attempts: prior_attempts + 1, last_attempt: now, last_error: err }));
                    }
                },
                Err(e) => {
                    // Task panic that (bug thuc su, khong phai loi RPC) -
                    // khong co key de giu pending, dem vao error_lines that.
                    errors += 1;
                    logger.log("pair.resolve_task_panic", serde_json::json!({ "error": e.to_string() }));
                }
            }
        }

        let mut new_map = HashMap::new();
        let mut new_token_quote_index = HashMap::new();
        let mut pending_count = 0u64;
        for (key, state) in &new_line_state {
            match state {
                LineState::Resolved(entry) => {
                    if entry.resolved_from == ResolvedFrom::Token {
                        new_token_quote_index.insert(*key, entry.pair_addr);
                    }
                    new_map.insert(entry.pair_addr, entry.clone());
                }
                LineState::Pending(_) => pending_count += 1,
            }
        }

        let count = new_map.len();
        self.line_state = new_line_state;
        self.pairs = new_map;
        self.token_quote_to_pair = new_token_quote_index;
        self.error_lines = errors;
        self.last_reload = Some(now);
        logger.log(
            "pair.reload",
            serde_json::json!({
                "count": count,
                "error_lines": errors,
                "pending": pending_count,
                "unvetted": unvetted,
                "elapsed_ms": started.elapsed().as_millis() as u64,
            }),
        );
    }

    /// Hot-reload có điều kiện thời gian, cùng khuôn `VictimBook::reload_if_due`
    /// / `Config::reload_if_due`. Đọc file lỗi (chưa tồn tại/permission) ->
    /// log + giữ `last_reload=now` (tránh retry storm mỗi tick), KHÔNG panic.
    pub async fn reload_if_due<R: PairResolver>(
        &mut self,
        path: &Path,
        resolver: &R,
        logger: &BotLogger,
        reload_interval: Duration,
        now: Instant,
        require_vetted: bool,
    ) -> bool {
        let due = match self.last_reload {
            None => true,
            Some(last) => now.saturating_duration_since(last) >= reload_interval,
        };
        if due {
            match tokio::fs::read_to_string(path).await {
                Ok(content) => self.reload(&content, resolver, logger, now, require_vetted).await,
                Err(e) => {
                    logger.log("pair.reload_error", serde_json::json!({ "error": e.to_string() }));
                    self.last_reload = Some(now);
                }
            }
        }
        due
    }
}

/// Helper CHỈ DÙNG TRONG TEST (cả trong crate này lẫn `pipeline.rs`) — chèn
/// thẳng 1 entry đã resolve sẵn, tránh phải dựng `PairResolver` giả lập ở
/// những nơi chỉ cần `PairBook` đã có sẵn dữ liệu để test `decide_paper_v2`.
/// `vetted_at` luôn `None` ở đây (các test dùng helper này KHÔNG kiểm tra
/// gate vet — gate đó nằm trong `reload`, bị bỏ qua hoàn toàn bởi helper
/// insert thẳng này, đúng ý "helper chèn sẵn dữ liệu").
#[cfg(test)]
impl PairBook {
    pub fn insert_test_entry(&mut self, pair_addr: Address, source_line: &str, resolved_from: ResolvedFrom) {
        self.pairs.insert(
            pair_addr,
            PairEntry { pair_addr, source_line: source_line.to_string(), resolved_from, vetted_at: None, quote: wbnb(), symbol: None },
        );
        self.last_reload = Some(Instant::now());
    }

    /// Cụm `hotpath-fix-then-decoder-ur` (A2) — helper test CHỈ DÙNG bởi
    /// `pipeline.rs` (test `decide_paper_quote` nhánh USDT): chèn thẳng 1 pool
    /// ĐÃ VET (`vetted_at=Some`, `is_tax_ok=true` ngay), khác `insert_test_entry`
    /// (luôn `vetted_at=None`) — mô phỏng đúng trạng thái `pairs.txt` sau khi
    /// Chủ vet tay + `reload` xong.
    pub fn insert_test_vetted_pair(&mut self, pair_addr: Address, quote: Address) {
        self.pairs.insert(
            pair_addr,
            PairEntry {
                pair_addr,
                source_line: "0xtest".to_string(),
                resolved_from: ResolvedFrom::Token,
                vetted_at: NaiveDate::from_ymd_opt(2026, 9, 16),
                quote,
                symbol: None,
            },
        );
        self.last_reload = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(hex: &str) -> Address {
        Address::from_str(hex).unwrap()
    }

    fn test_logger() -> (tempfile::TempDir, BotLogger) {
        let dir = tempfile::tempdir().unwrap();
        let logger = BotLogger::new(dir.path().join("logs").join("bot.jsonl")).unwrap();
        (dir, logger)
    }

    /// Resolver giả lập KHÔNG cần RPC sống — `map` mô phỏng đúng ngữ nghĩa
    /// `Factory.getPair`: token có trong map -> pair tương ứng; không có ->
    /// `Address::ZERO` (giống "không có pool"). `should_err` mô phỏng lỗi RPC
    /// thật cho 1 vài token cụ thể (test resolve fail).
    #[derive(Clone, Default)]
    struct MockResolver {
        map: HashMap<Address, Address>,
        err_for: Vec<Address>,
    }

    impl PairResolver for MockResolver {
        async fn get_pair(&self, token: Address, _quote: Address) -> Result<Address, String> {
            if self.err_for.contains(&token) {
                return Err("mock rpc loi".to_string());
            }
            Ok(self.map.get(&token).copied().unwrap_or(Address::ZERO))
        }
    }

    /// Cụm `econ-truth-latency-vps` (0.a) — bọc `MockResolver`, đếm số lần
    /// `get_pair` THẬT được gọi — chứng minh dòng đã resolve xong không bị
    /// gọi RPC lại ở các lần `reload()` sau.
    #[derive(Clone)]
    struct CountingResolver {
        inner: MockResolver,
        calls: Arc<std::sync::atomic::AtomicU64>,
    }

    impl PairResolver for CountingResolver {
        async fn get_pair(&self, token: Address, quote: Address) -> Result<Address, String> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.inner.get_pair(token, quote).await
        }
    }

    /// ĐẠT CẦN DÁN: `pairbook_load_direct_addr` — dòng "0xAddress" trần mà
    /// `getPair(addr, WBNB)` trả về `0x0` (addr không có trong map) -> addr
    /// TỰ NÓ là pair address, `resolved_from=Direct`.
    #[tokio::test]
    async fn pairbook_load_direct_addr() {
        let (_dir, logger) = test_logger();
        let pair_itself = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let resolver = MockResolver::default(); // map rong -> moi getPair tra ZERO
        let mut book = PairBook::new();
        book.reload(
            &format!("{pair_itself:#x}\n"),
            &resolver,
            &logger,
            Instant::now(),
            false,
        )
        .await;
        assert_eq!(book.error_lines, 0);
        assert_eq!(book.len(), 1);
        assert!(book.contains(&pair_itself));
        let entry = book.entries().next().unwrap();
        assert_eq!(entry.resolved_from, ResolvedFrom::Direct);
        assert_eq!(entry.pair_addr, pair_itself);
    }

    /// ĐẠT CẦN DÁN: `pairbook_load_token_resolve_mock` — dòng "0xAddress" trần
    /// LÀ token thật (mock map trả pair != 0) -> `resolved_from=Token`, entry
    /// key theo PAIR (không phải token). Cả 2 định dạng dòng đều test (trần +
    /// "token,WBNB").
    #[tokio::test]
    async fn pairbook_load_token_resolve_mock() {
        let (_dir, logger) = test_logger();
        let token_bare = addr("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let pair_bare = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let token_csv = addr("0xdddddddddddddddddddddddddddddddddddddddd");
        let pair_csv = addr("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        let mut map = HashMap::new();
        map.insert(token_bare, pair_bare);
        map.insert(token_csv, pair_csv);
        let resolver = MockResolver { map, err_for: vec![] };

        let content = format!("{token_bare:#x}\n{token_csv:#x},{WBNB_ADDRESS}\n");
        let mut book = PairBook::new();
        book.reload(&content, &resolver, &logger, Instant::now(), false).await;

        assert_eq!(book.error_lines, 0);
        assert_eq!(book.len(), 2);
        assert!(book.contains(&pair_bare));
        assert!(book.contains(&pair_csv));
        for entry in book.entries() {
            assert_eq!(entry.resolved_from, ResolvedFrom::Token);
        }
    }

    #[tokio::test]
    async fn pairbook_wrong_second_column_is_parse_error_skipped() {
        let (_dir, logger) = test_logger();
        let token = addr("0xffffffffffffffffffffffffffffffffffffffff");
        let not_wbnb = addr("0x1111111111111111111111111111111111111111");
        let content = format!("{token:#x},{not_wbnb:#x}\n");
        let resolver = MockResolver::default();
        let mut book = PairBook::new();
        book.reload(&content, &resolver, &logger, Instant::now(), false).await;
        assert_eq!(book.len(), 0);
        assert_eq!(book.error_lines, 1);
    }

    /// ĐẠT CẦN DÁN (cụm `hotpath-fix-then-decoder-ur`, A1) — 3 dòng
    /// `token,WBNB` / `token,USDT` / `token,dia_chi_sai` -> ĐÚNG 2 entry
    /// (WBNB + USDT) + 1 `error_lines` (địa chỉ thứ 2 không phải WBNB/USDT).
    /// Trước fix: dòng USDT bị coi lỗi (chỉ nhận WBNB) -> 1 entry + 2 error.
    #[tokio::test]
    async fn pairbook_load_wbnb_and_usdt_quote_lines_two_entries_one_error() {
        let (_dir, logger) = test_logger();
        let token_wbnb = addr("0xaaaa00000000000000000000000000000000aaaa");
        let pair_wbnb = addr("0xbbbb00000000000000000000000000000000bbbb");
        let token_usdt = addr("0xcccc00000000000000000000000000000000cccc");
        let pair_usdt = addr("0xdddd00000000000000000000000000000000dddd");
        let token_bad = addr("0xeeee00000000000000000000000000000000eeee");
        let bad_quote = addr("0x1111111111111111111111111111111111111111");

        let mut map = HashMap::new();
        map.insert(token_wbnb, pair_wbnb);
        map.insert(token_usdt, pair_usdt);
        let resolver = MockResolver { map, err_for: vec![] };

        let content = format!(
            "{token_wbnb:#x},{WBNB_ADDRESS}\n{token_usdt:#x},{USDT_ADDRESS}\n{token_bad:#x},{bad_quote:#x}\n"
        );
        let mut book = PairBook::new();
        book.reload(&content, &resolver, &logger, Instant::now(), false).await;

        assert_eq!(book.len(), 2, "dung 2 entry (WBNB + USDT), dong sai dia chi bi loai");
        assert_eq!(book.error_lines, 1, "chi dong dia chi thu 2 sai moi tinh loi");
        assert!(book.contains(&pair_wbnb));
        assert!(book.contains(&pair_usdt));
        let entry_wbnb = book.entries().find(|e| e.pair_addr == pair_wbnb).unwrap();
        let entry_usdt = book.entries().find(|e| e.pair_addr == pair_usdt).unwrap();
        assert_eq!(entry_wbnb.quote, wbnb());
        assert_eq!(entry_usdt.quote, usdt());
        assert_eq!(book.known_pair(token_wbnb, wbnb()), Some(pair_wbnb));
        assert_eq!(book.known_pair(token_usdt, usdt()), Some(pair_usdt));
        assert_eq!(book.known_pair(token_usdt, wbnb()), None, "khac quote -> khong khop");
    }

    /// `tokens_to_vet` phải trả ĐÚNG quote cho từng entry (không hardcode
    /// WBNB) — mấu chốt để `pairs_vet_task` gọi `measure_tax_evm` đúng pool.
    #[tokio::test]
    async fn tokens_to_vet_carries_correct_quote_per_entry() {
        let (_dir, logger) = test_logger();
        let token_wbnb = addr("0xaaaa00000000000000000000000000000000aaaa");
        let pair_wbnb = addr("0xbbbb00000000000000000000000000000000bbbb");
        let token_usdt = addr("0xcccc00000000000000000000000000000000cccc");
        let pair_usdt = addr("0xdddd00000000000000000000000000000000dddd");
        let mut map = HashMap::new();
        map.insert(token_wbnb, pair_wbnb);
        map.insert(token_usdt, pair_usdt);
        let resolver = MockResolver { map, err_for: vec![] };
        let content = format!(
            "{token_wbnb:#x},{WBNB_ADDRESS} # A | vetted 2026-09-16 |\n{token_usdt:#x},{USDT_ADDRESS} # B | vetted 2026-09-16 |\n"
        );
        let mut book = PairBook::new();
        book.reload(&content, &resolver, &logger, Instant::now(), true).await;

        let mut vet = book.tokens_to_vet();
        vet.sort_by_key(|(pair, _, _)| *pair);
        assert_eq!(vet.len(), 2);
        let wbnb_entry = vet.iter().find(|(p, _, _)| *p == pair_wbnb).unwrap();
        assert_eq!(wbnb_entry.2, wbnb());
        let usdt_entry = vet.iter().find(|(p, _, _)| *p == pair_usdt).unwrap();
        assert_eq!(usdt_entry.2, usdt());
    }

    #[tokio::test]
    async fn pairbook_resolve_rpc_error_is_skipped_not_crash() {
        // Cum `econ-truth-latency-vps` (0.a) - THAY DOI HANH VI co chu dich:
        // loi RPC (timeout/transport) khi resolve KHONG con tinh la
        // `error_lines` (loi vinh vien) nua - gio la `pending` (cho retry
        // backoff 5s/15s/60s), khong crash, khong bao gio bi mat khoi
        // PairBook truoc khi tung resolve thanh cong (khac ban cu coi day la
        // 1 "error" giong loi parse dia chi that).
        let (_dir, logger) = test_logger();
        let token = addr("0x2222222222222222222222222222222222222222");
        let resolver = MockResolver { map: HashMap::new(), err_for: vec![token] };
        let mut book = PairBook::new();
        book.reload(&format!("{token:#x}\n"), &resolver, &logger, Instant::now(), false).await;
        assert_eq!(book.len(), 0);
        assert_eq!(book.error_lines, 0, "loi RPC khong con tinh vao error_lines (chi loi parse that moi tinh)");
        assert_eq!(book.pending_count(), 1, "dong loi RPC phai roi vao pending, cho retry");
        let pending = book.pending_entries();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, token);
        assert!(pending[0].3.contains("mock rpc loi"), "last_error phai giu nguyen van thong bao loi that");

        let tail = logger.tail(10);
        let row = tail.iter().find(|l| l["event"] == "pair.resolve_fail").expect("phai co dong pair.resolve_fail");
        assert_eq!(row["token"], format!("{token:#x}"));
        assert!(row["error"].as_str().unwrap_or("").contains("mock rpc loi"));
    }

    /// ĐẠT CẦN DÁN (0.a) — dòng ĐÃ resolve thành công ở lần reload trước
    /// KHÔNG bị resolve lại (0 lần gọi `get_pair` thêm) ở các lần reload sau,
    /// dù resolver có đổi map/lỗi — chứng minh cache bền qua nhiều lần
    /// reload, đúng yêu cầu "chỉ resolve dòng MỚI/đổi".
    #[tokio::test]
    async fn reload_does_not_reresolve_already_resolved_lines() {
        let (_dir, logger) = test_logger();
        let token = addr("0x00000000000000000000000000000000cafe1234");
        let pair = addr("0x00000000000000000000000000000000cafeaaaa");
        let mut map = HashMap::new();
        map.insert(token, pair);
        let counting = CountingResolver { inner: MockResolver { map, err_for: vec![] }, calls: Arc::new(std::sync::atomic::AtomicU64::new(0)) };
        let mut book = PairBook::new();
        let content = format!("{token:#x}\n");
        book.reload(&content, &counting, &logger, Instant::now(), false).await;
        assert_eq!(book.len(), 1);
        assert_eq!(counting.calls.load(std::sync::atomic::Ordering::SeqCst), 1);

        // Lan reload SAU, cung content - resolver gio se LOI cho MOI token
        // (mo phong RPC chet) nhung dong nay DA resolve tu truoc nen KHONG
        // duoc goi lai - van con nguyen trong book, KHONG rot xuong pending.
        let failing = CountingResolver {
            inner: MockResolver { map: HashMap::new(), err_for: vec![token] },
            calls: counting.calls.clone(),
        };
        book.reload(&content, &failing, &logger, Instant::now() + Duration::from_secs(1), false).await;
        assert_eq!(book.len(), 1, "dong da resolve khong duoc resolve lai, khong bi mat");
        assert!(book.contains(&pair));
        assert_eq!(book.pending_count(), 0);
        assert_eq!(counting.calls.load(std::sync::atomic::Ordering::SeqCst), 1, "khong co eth_call THEM nao cho dong da resolve xong");
    }

    /// Backoff: dòng pending mới thất bại lần 1 (attempts=1) KHÔNG được retry
    /// ngay ở lần reload kế tiếp (chưa đủ 5s) — retry đúng sau khi đủ thời
    /// gian backoff.
    #[tokio::test]
    async fn pending_line_respects_backoff_before_retrying() {
        let (_dir, logger) = test_logger();
        let token = addr("0x00000000000000000000000000000000cafe999a");
        let counting =
            CountingResolver { inner: MockResolver { map: HashMap::new(), err_for: vec![token] }, calls: Arc::new(std::sync::atomic::AtomicU64::new(0)) };
        let mut book = PairBook::new();
        let content = format!("{token:#x}\n");
        let t0 = Instant::now();
        book.reload(&content, &counting, &logger, t0, false).await;
        assert_eq!(counting.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(book.pending_entries()[0].2, 1, "attempts phai la 1 sau lan loi dau tien");

        // Reload lai chi 2s sau (chua du backoff 5s cho attempts=1) - KHONG
        // duoc goi RPC them.
        book.reload(&content, &counting, &logger, t0 + Duration::from_secs(2), false).await;
        assert_eq!(counting.calls.load(std::sync::atomic::Ordering::SeqCst), 1, "chua du 5s backoff, khong duoc retry");

        // Reload lai 6s sau lan dau (du 5s backoff) - PHAI goi RPC lai.
        book.reload(&content, &counting, &logger, t0 + Duration::from_secs(6), false).await;
        assert_eq!(counting.calls.load(std::sync::atomic::Ordering::SeqCst), 2, "du 5s backoff, phai retry");
        assert_eq!(book.pending_entries()[0].2, 2, "attempts tang len 2 sau lan retry that bai thu 2");
    }

    /// ĐẠT CẦN DÁN (cụm A5, fix bug parse) — định dạng file THẬT
    /// `pairs.txt` (`0xAddr # SYMBOL reserve_wbnb~=... BNB`, comment CUỐI
    /// DÒNG) phải parse ĐÚNG 1 pool, không rơi vào `error_lines` như trước
    /// khi fix (BAOCAO27: `pair.parse_error=900` vì thiếu bước cắt comment).
    #[tokio::test]
    async fn pairbook_real_file_format_with_trailing_comment_parses_exactly_1_pool() {
        let (_dir, logger) = test_logger();
        let token = addr("0x0E09FaBB73Bd3Ade0a17ECC321fD13a19e81cE82"); // CAKE, dung format that
        let pair = addr("0x111111111111111111111111111111111111beef");
        let mut map = HashMap::new();
        map.insert(token, pair);
        let resolver = MockResolver { map, err_for: vec![] };

        let content = format!(
            "# pairs.txt header comment\n{token:#x} # CAKE reserve_wbnb~=14002 BNB\n\n# blank/comment lines xen ke\n"
        );
        let mut book = PairBook::new();
        book.reload(&content, &resolver, &logger, Instant::now(), false).await;

        assert_eq!(book.error_lines, 0, "dinh dang that (comment cuoi dong) khong duoc tinh la loi");
        assert_eq!(book.len(), 1);
        assert!(book.contains(&pair));
    }

    #[tokio::test]
    async fn pairbook_garbage_line_and_comment_skipped_no_panic() {
        let (_dir, logger) = test_logger();
        let content = "# comment\nnot_an_address\n\n";
        let resolver = MockResolver::default();
        let mut book = PairBook::new();
        book.reload(content, &resolver, &logger, Instant::now(), false).await;
        assert_eq!(book.len(), 0);
        assert_eq!(book.error_lines, 1); // chi "not_an_address" tinh loi, comment/blank bi bo qua truoc
    }

    #[tokio::test]
    async fn reload_if_due_respects_interval_with_injected_clock() {
        let dir = tempfile::tempdir().unwrap();
        let logger_dir = tempfile::tempdir().unwrap();
        let logger = BotLogger::new(logger_dir.path().join("logs").join("bot.jsonl")).unwrap();
        let path = dir.path().join("pairs.txt");
        let pair_itself = addr(&format!("0x{}", "3".repeat(40)));
        std::fs::write(&path, format!("{pair_itself:#x}\n")).unwrap();

        let resolver = MockResolver::default();
        let mut book = PairBook::new();
        let t0 = Instant::now();
        assert!(book.reload_if_due(&path, &resolver, &logger, Duration::from_secs(15), t0, false).await);
        assert_eq!(book.len(), 1);

        // Ghi file khac ngay (chua du interval) -> khong reload lai.
        let pair_other = addr(&format!("0x{}", "4".repeat(40)));
        std::fs::write(&path, format!("{pair_other:#x}\n")).unwrap();
        let t1 = t0 + Duration::from_secs(5);
        assert!(!book.reload_if_due(&path, &resolver, &logger, Duration::from_secs(15), t1, false).await);
        assert!(book.contains(&pair_itself));

        // Du interval -> doc lai file moi.
        let t2 = t0 + Duration::from_secs(16);
        assert!(book.reload_if_due(&path, &resolver, &logger, Duration::from_secs(15), t2, false).await);
        assert!(book.contains(&pair_other));
        assert!(!book.contains(&pair_itself));
    }

    // ===== Cụm `strategy-lock-mode2` — parse `vetted YYYY-MM-DD` + gate =====

    #[test]
    fn parse_vetted_from_comment_valid_date() {
        let d = parse_vetted_from_comment(" CAKE | vetted 2026-09-15 | tax 0/0 | owner renounced | note ");
        assert_eq!(d, Some(NaiveDate::from_ymd_opt(2026, 9, 15).unwrap()));
    }

    #[test]
    fn parse_symbol_from_comment_reads_first_field() {
        assert_eq!(
            parse_symbol_from_comment(" CAKE | vetted 2026-09-15 | tax 0/0 | owner renounced | note "),
            Some("CAKE".to_string())
        );
        assert_eq!(parse_symbol_from_comment(""), None);
        assert_eq!(parse_symbol_from_comment("   "), None);
        assert_eq!(parse_symbol_from_comment("BORT reserve_wbnb~=100 BNB"), Some("BORT reserve_wbnb~=100 BNB".to_string()));
    }

    #[test]
    fn parse_vetted_from_comment_empty_vetted_field_is_none() {
        // "vetted" co mat nhung KHONG kem ngay (template chua dien) -> None.
        assert_eq!(parse_vetted_from_comment(" CAKE | vetted | tax  | owner  | "), None);
    }

    #[test]
    fn parse_vetted_from_comment_old_format_no_pipe_is_none() {
        // Dinh dang cu (BAOCAO25/27): "# SYMBOL reserve_wbnb~=X BNB", khong
        // co "|", khong co "vetted" -> None (CHUA VET), khong panic/khong loi.
        assert_eq!(parse_vetted_from_comment(" CAKE reserve_wbnb~=14002 BNB"), None);
    }

    #[test]
    fn parse_vetted_from_comment_bad_date_format_is_none() {
        assert_eq!(parse_vetted_from_comment(" CAKE | vetted 15/09/2026 | "), None);
    }

    /// ĐẠT CẦN DÁN (lệnh `strategy-lock-mode2`, mục 3): 3 dòng (vetted /
    /// không vetted / comment cũ) -> ĐÚNG 1 candidate khi `require_vetted=true`.
    #[tokio::test]
    async fn reload_require_vetted_true_keeps_only_dated_entry() {
        let (_dir, logger) = test_logger();
        let vetted_token = addr("0x1111111111111111111111111111111111111111");
        let unvetted_token = addr("0x2222222222222222222222222222222222222222");
        let old_format_token = addr("0x3333333333333333333333333333333333333333");
        let pair_vetted = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let pair_unvetted = addr("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let pair_old = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let mut map = HashMap::new();
        map.insert(vetted_token, pair_vetted);
        map.insert(unvetted_token, pair_unvetted);
        map.insert(old_format_token, pair_old);
        let resolver = MockResolver { map, err_for: vec![] };

        let content = format!(
            "{vetted_token:#x} # VET | vetted 2026-09-15 | tax 0/0 | owner renounced | ok\n\
             {unvetted_token:#x} # NOVET | vetted | tax  | owner  | chua dien\n\
             {old_format_token:#x} # OLD reserve_wbnb~=100 BNB\n"
        );
        let mut book = PairBook::new();
        book.reload(&content, &resolver, &logger, Instant::now(), true).await;

        assert_eq!(book.len(), 1, "chi dong co vetted YYYY-MM-DD hop le duoc vao map");
        assert!(book.contains(&pair_vetted));
        assert!(!book.contains(&pair_unvetted));
        assert!(!book.contains(&pair_old));
        assert_eq!(book.error_lines, 0, "2 dong con lai la UNVETTED, khong phai loi parse dia chi");
        let entry = book.entries().next().unwrap();
        assert_eq!(entry.vetted_at, Some(NaiveDate::from_ymd_opt(2026, 9, 15).unwrap()));
    }

    /// Cùng 3 dòng trên nhưng `require_vetted=false` -> giữ hành vi cũ, cả 3
    /// đều thành candidate (không phân biệt đã vet hay chưa).
    #[tokio::test]
    async fn reload_require_vetted_false_keeps_all_resolved_entries() {
        let (_dir, logger) = test_logger();
        let vetted_token = addr("0x4444444444444444444444444444444444444444");
        let unvetted_token = addr("0x5555555555555555555555555555555555555555");
        let pair_vetted = addr("0xdddddddddddddddddddddddddddddddddddddddd");
        let pair_unvetted = addr("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        let mut map = HashMap::new();
        map.insert(vetted_token, pair_vetted);
        map.insert(unvetted_token, pair_unvetted);
        let resolver = MockResolver { map, err_for: vec![] };

        let content =
            format!("{vetted_token:#x} # VET | vetted 2026-09-15 |\n{unvetted_token:#x} # NOVET | vetted |\n");
        let mut book = PairBook::new();
        book.reload(&content, &resolver, &logger, Instant::now(), false).await;

        assert_eq!(book.len(), 2);
        assert!(book.contains(&pair_vetted));
        assert!(book.contains(&pair_unvetted));
    }

    /// ĐẠT CẦN DÁN (lệnh `strategy-lock-mode2`, mục 4) — `set_vet_result`
    /// `ok=false` (tax/honeypot đo bằng EVM thật) loại pool khỏi `contains()`
    /// NGAY, dù entry vẫn còn trong `pairs.txt`/`entries()`; `ok=true` sau đó
    /// trả candidate về lại.
    #[test]
    fn set_vet_result_false_removes_from_contains_true_restores() {
        let mut book = PairBook::new();
        let pair = addr("0x6666666666666666666666666666666666666666");
        book.insert_test_entry(pair, "0xtoken", ResolvedFrom::Token);
        assert!(book.contains(&pair));

        book.set_vet_result(pair, VetResult { buy_bps: 0, sell_bps: 2000, honeypot: false, block: 100 }, false);
        assert!(!book.contains(&pair), "tax cao phai loai khoi candidate");
        assert!(book.entries().any(|e| e.pair_addr == pair), "entry van con trong pairs.txt/list");

        book.set_vet_result(pair, VetResult { buy_bps: 0, sell_bps: 0, honeypot: false, block: 101 }, true);
        assert!(book.contains(&pair), "vet PASS lan sau phai tra lai candidate");
    }

    /// Cụm `real-economics-mode2` — ĐẠT CẦN DÁN mục 0: token vetted (từ
    /// `reload`, `vetted_at=Some`) VÀ chưa từng `vet_fail` → `is_tax_ok=true`.
    /// Token chưa vet (`insert_test_entry`, `vetted_at=None`) → `false`.
    /// Token vetted nhưng vừa bị `set_vet_result(ok=false)` → `false`.
    #[tokio::test]
    async fn is_tax_ok_true_only_for_vetted_and_not_vet_failed() {
        let (_dir, logger) = test_logger();
        let vetted_token = addr("0x00000000000000000000000000000000000000aa");
        let pair_vetted = addr("0x00000000000000000000000000000000000000bb");
        let mut map = HashMap::new();
        map.insert(vetted_token, pair_vetted);
        let resolver = MockResolver { map, err_for: vec![] };
        let content = format!("{vetted_token:#x} # VET | vetted 2026-09-16 | tax 0/0 | owner renounced | note\n");
        let mut book = PairBook::new();
        book.reload(&content, &resolver, &logger, Instant::now(), true).await;
        assert!(book.is_tax_ok(&pair_vetted), "token da vet, chua tung vet_fail -> tax OK");

        let unknown_pair = addr("0x00000000000000000000000000000000000000cc");
        assert!(!book.is_tax_ok(&unknown_pair), "pair khong co trong PairBook -> khong duoc coi tax OK");

        book.set_vet_result(pair_vetted, VetResult { buy_bps: 0, sell_bps: 2000, honeypot: false, block: 1 }, false);
        assert!(!book.is_tax_ok(&pair_vetted), "vet nen vua loai (vet_fail) -> khong con tax OK");
    }

    /// Entry CHƯA vet (helper `insert_test_entry`, `vetted_at=None` cố ý) →
    /// `is_tax_ok=false` dù đã `contains()==true` — hai hàm KHÁC ý nghĩa
    /// nhau (`contains` chỉ cần có trong map + không vet_fail, `is_tax_ok`
    /// đòi thêm `vetted_at=Some`).
    #[test]
    fn is_tax_ok_false_for_unvetted_entry_even_if_contains_true() {
        let mut book = PairBook::new();
        let pair = addr("0x00000000000000000000000000000000000000dd");
        book.insert_test_entry(pair, "0xtoken", ResolvedFrom::Token);
        assert!(book.contains(&pair), "helper insert truc tiep vao map -> contains=true");
        assert!(!book.is_tax_ok(&pair), "insert_test_entry luon vetted_at=None -> is_tax_ok phai false");
    }

    #[test]
    fn tokens_to_vet_only_includes_vetted_token_entries_not_direct() {
        let (_dir, _logger) = test_logger();
        let mut book = PairBook::new();
        let token = addr("0x7777777777777777777777777777777777777777");
        let pair_token = addr("0x8888888888888888888888888888888888888888");
        let pair_direct = addr("0x9999999999999999999999999999999999999999");
        book.insert_test_entry(pair_token, &format!("{token:#x}"), ResolvedFrom::Token);
        book.insert_test_entry(pair_direct, &format!("{pair_direct:#x}"), ResolvedFrom::Direct);
        // insert_test_entry luon dat vetted_at=None -> khong entry nao vao
        // tokens_to_vet(), dung du de test filter resolved_from hoat dong khi
        // ket hop voi mot entry co vetted_at (kiem qua reload() o test tren).
        assert!(book.tokens_to_vet().is_empty());
    }
}

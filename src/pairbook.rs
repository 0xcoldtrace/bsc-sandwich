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
//! - `tokenAddr,WBNB` — resolve thẳng `getPair(tokenAddr, WBNB)`. Địa chỉ thứ
//!   2 PHẢI đúng WBNB đã pin (`venues::WBNB_ADDRESS`), sai -> lỗi dòng, skip.
//!
//! Global `min_swap` (`config.toml::pairs_min_swap_bnb`) áp dụng cho MỌI
//! entry — không có `min_swap` riêng theo từng pool (khác `victims.txt`, nơi
//! mỗi ví có ngưỡng riêng).

use alloy::primitives::Address;
use alloy::providers::DynProvider;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

use crate::logger::BotLogger;
use crate::pool;
use crate::venues::WBNB_ADDRESS;

/// Resolve tối đa 10 dòng đồng thời (CLAUDE.md lệnh pair-mode), timeout 3s/dòng.
const MAX_CONCURRENT_RESOLVE: usize = 10;
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(3);

fn wbnb() -> Address {
    Address::from_str(WBNB_ADDRESS).expect("WBNB_ADDRESS da pin phai hop le")
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
}

#[derive(Debug, Default)]
pub struct PairBook {
    pairs: HashMap<Address, PairEntry>,
    pub error_lines: u64,
    pub last_reload: Option<Instant>,
}

enum ParsedLine {
    /// "0xAddress" trần — chưa biết token hay pair.
    Direct(Address),
    /// "tokenAddr,WBNB" — đã biết chắc là token.
    TokenWbnb(Address),
}

fn parse_pairs_line(line: &str) -> Result<ParsedLine, String> {
    if let Some(comma) = line.find(',') {
        let (addr_str, rest) = line.split_at(comma);
        let rest = &rest[1..];
        let token = Address::from_str(addr_str.trim())
            .map_err(|e| format!("token address khong hop le '{addr_str}': {e}"))?;
        let wbnb_candidate = Address::from_str(rest.trim())
            .map_err(|e| format!("dia chi thu 2 khong hop le '{rest}': {e}"))?;
        if wbnb_candidate != wbnb() {
            return Err(format!(
                "dia chi thu 2 '{rest}' khong phai WBNB da pin ({WBNB_ADDRESS})"
            ));
        }
        Ok(ParsedLine::TokenWbnb(token))
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
    fn get_pair(&self, token: Address) -> impl std::future::Future<Output = Result<Address, String>> + Send;
}

/// Resolver PRODUCTION — bọc `DynProvider` (owned, `Clone` rẻ vì bên trong là
/// `Arc`) + factory V2 đã pin, dùng lại `pool::resolve_v2_pair` (không tự
/// viết lại `eth_call`). `Address::ZERO` nghĩa là "không có pool" (khớp
/// `PoolSkipReason::NoPool`), KHÁC lỗi RPC thật (`Err`).
#[derive(Clone)]
pub struct RpcPairResolver {
    pub provider: DynProvider,
    pub factory: Address,
}

impl PairResolver for RpcPairResolver {
    async fn get_pair(&self, token: Address) -> Result<Address, String> {
        match pool::resolve_v2_pair(&self.provider, self.factory, token).await {
            Ok(Ok(pair)) => Ok(pair),
            Ok(Err(_)) => Ok(Address::ZERO),
            Err(e) => Err(e),
        }
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
    /// pool nào trong danh sách này là candidate "pair mode".
    pub fn contains(&self, pair: &Address) -> bool {
        self.pairs.contains_key(pair)
    }

    pub fn entries(&self) -> impl Iterator<Item = &PairEntry> {
        self.pairs.values()
    }

    pub fn last_reload_sec_ago(&self) -> Option<u64> {
        self.last_reload.map(|t| t.elapsed().as_secs())
    }

    /// Nạp lại toàn bộ `pairs.txt` — resolve tối đa `MAX_CONCURRENT_RESOLVE`
    /// dòng đồng thời qua `tokio::spawn` (resolver `Clone + Send + Sync +
    /// 'static` nên mỗi task giữ 1 bản clone riêng, không tranh chấp
    /// lifetime), timeout `RESOLVE_TIMEOUT`/dòng. Lỗi resolve (timeout/RPC
    /// lỗi) -> log `pair.resolve_fail`, BỎ QUA dòng đó, KHÔNG crash. `now`
    /// truyền từ ngoài để test được (cùng quy ước `VictimBook`/`Config`).
    pub async fn reload<R: PairResolver>(&mut self, content: &str, resolver: &R, logger: &BotLogger, now: Instant) {
        let started = Instant::now();
        let mut parsed_lines: Vec<(String, ParsedLine)> = Vec::new();
        let mut errors = 0u64;
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
            match parse_pairs_line(line) {
                Ok(p) => parsed_lines.push((line.to_string(), p)),
                Err(e) => {
                    errors += 1;
                    logger.log("pair.parse_error", serde_json::json!({ "error": e, "line": line }));
                }
            }
        }

        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_RESOLVE));
        let mut handles = Vec::new();
        for (line, parsed) in parsed_lines {
            let sem = semaphore.clone();
            let resolver = resolver.clone();
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire_owned().await.ok()?;
                let resolved = match parsed {
                    ParsedLine::TokenWbnb(token) => {
                        match tokio::time::timeout(RESOLVE_TIMEOUT, resolver.get_pair(token)).await {
                            Ok(Ok(pair)) if pair != Address::ZERO => Some((pair, ResolvedFrom::Token)),
                            _ => None,
                        }
                    }
                    ParsedLine::Direct(addr) => {
                        match tokio::time::timeout(RESOLVE_TIMEOUT, resolver.get_pair(addr)).await {
                            Ok(Ok(pair)) if pair != Address::ZERO => Some((pair, ResolvedFrom::Token)),
                            Ok(Ok(_zero)) => Some((addr, ResolvedFrom::Direct)),
                            _ => None,
                        }
                    }
                };
                resolved.map(|(pair_addr, resolved_from)| PairEntry { pair_addr, source_line: line, resolved_from })
            }));
        }

        let mut new_map = HashMap::new();
        for h in handles {
            match h.await {
                Ok(Some(entry)) => {
                    new_map.insert(entry.pair_addr, entry);
                }
                Ok(None) => {
                    errors += 1;
                    logger.log("pair.resolve_fail", serde_json::json!({}));
                }
                Err(_) => {
                    errors += 1;
                }
            }
        }

        let count = new_map.len();
        self.pairs = new_map;
        self.error_lines = errors;
        self.last_reload = Some(now);
        logger.log(
            "pair.reload",
            serde_json::json!({ "count": count, "error_lines": errors, "elapsed_ms": started.elapsed().as_millis() as u64 }),
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
    ) -> bool {
        let due = match self.last_reload {
            None => true,
            Some(last) => now.saturating_duration_since(last) >= reload_interval,
        };
        if due {
            match tokio::fs::read_to_string(path).await {
                Ok(content) => self.reload(&content, resolver, logger, now).await,
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
#[cfg(test)]
impl PairBook {
    pub fn insert_test_entry(&mut self, pair_addr: Address, source_line: &str, resolved_from: ResolvedFrom) {
        self.pairs.insert(
            pair_addr,
            PairEntry { pair_addr, source_line: source_line.to_string(), resolved_from },
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
        async fn get_pair(&self, token: Address) -> Result<Address, String> {
            if self.err_for.contains(&token) {
                return Err("mock rpc loi".to_string());
            }
            Ok(self.map.get(&token).copied().unwrap_or(Address::ZERO))
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
        book.reload(&content, &resolver, &logger, Instant::now()).await;

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
        book.reload(&content, &resolver, &logger, Instant::now()).await;
        assert_eq!(book.len(), 0);
        assert_eq!(book.error_lines, 1);
    }

    #[tokio::test]
    async fn pairbook_resolve_rpc_error_is_skipped_not_crash() {
        let (_dir, logger) = test_logger();
        let token = addr("0x2222222222222222222222222222222222222222");
        let resolver = MockResolver { map: HashMap::new(), err_for: vec![token] };
        let mut book = PairBook::new();
        book.reload(&format!("{token:#x}\n"), &resolver, &logger, Instant::now()).await;
        assert_eq!(book.len(), 0);
        assert_eq!(book.error_lines, 1);
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
        book.reload(&content, &resolver, &logger, Instant::now()).await;

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
        book.reload(content, &resolver, &logger, Instant::now()).await;
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
        assert!(book.reload_if_due(&path, &resolver, &logger, Duration::from_secs(15), t0).await);
        assert_eq!(book.len(), 1);

        // Ghi file khac ngay (chua du interval) -> khong reload lai.
        let pair_other = addr(&format!("0x{}", "4".repeat(40)));
        std::fs::write(&path, format!("{pair_other:#x}\n")).unwrap();
        let t1 = t0 + Duration::from_secs(5);
        assert!(!book.reload_if_due(&path, &resolver, &logger, Duration::from_secs(15), t1).await);
        assert!(book.contains(&pair_itself));

        // Du interval -> doc lai file moi.
        let t2 = t0 + Duration::from_secs(16);
        assert!(book.reload_if_due(&path, &resolver, &logger, Duration::from_secs(15), t2).await);
        assert!(book.contains(&pair_other));
        assert!(!book.contains(&pair_itself));
    }
}

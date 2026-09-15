//! Cụm 4.1 — Nối `decoder` + `VictimBook` + `TaxCache` + `sim_v2` thành 1
//! quyết định "paper" cho một tx giấy. CHƯA subscribe pending-tx thật (thuộc
//! `5.1+`, cần `eth_subscribe newPendingTransactions` — transport.rs mới có
//! `subscribe_blocks`) — hàm `decide_paper` nhận tx giấy qua fixture
//! (`PaperDecision`), đúng "Cho phép gọi hàm bằng fixture tx" trong lệnh.
//!
//! Tách phần LÕI thuần (`decide_paper`, không cần `Provider`) khỏi phần
//! resolve on-chain thật (reserve pool `pool::get_reserves_vs_wbnb`, tax đo
//! thật `tax::measure_roundtrip_via_router`) — đúng quy ước repo (`pool.rs`/
//! `transport.rs`): test mặc định KHÔNG bắt RPC sống, chỉ phần lõi quyết
//! định được test đầy đủ ở đây; wiring RPC thật + vòng lặp pending-tx là
//! CÒN NỢ của `5.1+`.
//!
//! PHẠM VI: chỉ chiều victim MUA token bằng WBNB (`decoded.path.token_a ==
//! WBNB`, khớp `swapExactETHForTokens`/UR `V2_SWAP_EXACT_IN` từ WBNB) — đây
//! là chiều sandwich cổ điển (`sim_v2.rs`). Chiều victim BÁN token lấy WBNB
//! đã BỊ BỎ HẲN khỏi pipeline (quyết định phiên BAOCAO14, xem `docs/STATE.md`)
//! — thứ tự lệnh gốc (front mua trước, back bán sau victim bán) chứng minh
//! toán học luôn lỗ; hướng fix đúng cần đảo front/back + giữ tồn kho
//! token/flashloan, ngoài scope "1 signer, không flashloan" của CLAUDE.md.
//! Decoder vẫn decode đúng chiều bán đó nhưng pipeline coi `not_wbnb_pair`
//! (không sim, không code chiều này dưới bất kỳ hình thức nào).

use alloy::primitives::{Address, U256};
use alloy::providers::Provider;

use crate::config::{Config, RiskGuard};
use crate::decoder::{self, DecodedSwap, SkipReason as DecodeSkip};
use crate::executor;
use crate::logger::BotLogger;
use crate::pairbook::PairBook;
use crate::pool;
use crate::sim_v2::{self, PoolReserves, SandwichQuote};
use crate::tax::TaxCache;
use crate::venues::{V2_FACTORY_ADDRESS, USDT_ADDRESS, WBNB_ADDRESS};
use crate::victims::VictimBook;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineSkip {
    NotInList,
    BelowMin,
    DecodeFail,
    NotWbnbPair,
    /// Cụm `usdt-quote-asset` (BAOCAO29) — dùng bởi `decode_and_classify_quote`/
    /// `decide_paper_quote` (entrypoint quote-aware MỚI, song song
    /// `decode_and_classify`/`decide_paper_v2` — 2 hàm đó KHÔNG đổi, vẫn trả
    /// `NotWbnbPair` y hệt cũ) khi path không khớp WBNB lẫn USDT (hoặc USDT
    /// bị tắt qua `scan_quote_usdt=false`).
    NotQuotePair,
    /// Cụm `quote-live-wiring-funnel-diagnostics` — path CÓ chứa quote asset
    /// đang bật (WBNB luôn, USDT khi `scan_quote_usdt=true`) nhưng ở SAI
    /// PHÍA (`token_b`, không phải `token_a`) — nghĩa là victim đang BÁN
    /// token lấy quote asset, không phải MUA. Chỉ dùng bởi
    /// `decode_and_classify_quote` (entrypoint quote-aware MỚI) — hàm cũ
    /// `decode_and_classify` (WBNB-only, `decide_paper_v2`) GIỮ NGUYÊN, vẫn
    /// trả `NotWbnbPair` y hệt trước cho cùng tình huống này (không đổi hành
    /// vi WBNB hiện có). Chưa có model sandwich cho chiều bán (BAOCAO14) —
    /// đây chỉ là NHÃN chính xác hơn cho lý do skip, không bật lại chiều bán.
    SellDirection,
    /// Cụm `foundation-fix-then-real-sim` (A4) — `tx.to` KHÔNG nằm trong 5
    /// router Pancake đã pin (`venues::PANCAKE_ROUTERS`) — gate rẻ tiền, 0
    /// RPC, chạy TRƯỚC decode ở `main.rs`.
    NotPancakeRouter,
    /// Cụm A4 — decode thành công nhưng `decoder::SwapVenue` là `V3` (chưa
    /// pin quoter/sim cho nhánh nào bị SmartRouter/UR gộp gọi qua
    /// `exactInputSingle`/`exactInput`/UR `V3_SWAP_EXACT_IN` ở TẦNG PIPELINE
    /// này) — KHÔNG được đưa vào `resolve_v2_reserves` (bug cũ: V3 bị sim
    /// nhầm bằng pool V2, xem CLAUDE.md lệnh A4).
    VenueUnpinned,
    ThinLiq,
    NoPool,
    /// Cụm `hotpath-fix-then-decoder-ur` (A3) — `eth_call` `getPair`/
    /// `getReserves` LỖI THẬT (timeout/RPC mạng) — KHÁC `NoPool` (Factory trả
    /// `address(0)`, CHẮC CHẮN không có pool). Trước fix, cả 2 tình huống bị
    /// gộp chung vào `NoPool` ở `resolve_v2_reserves`/`resolve_reserves_for_quote`
    /// (quyết định cũ từ `5.1`) — paper run 5 phút phát hiện USDC/WBNB (pool
    /// CÓ THẬT, 294 BNB) bị `no_pool` 419 lần vì lỗi RPC thoáng qua, che mất
    /// tín hiệu "RPC quá tải/cần retry" khỏi "token không có pool V2 thật".
    /// `FunnelCounters::rpc_error` (đã có sẵn field DÀNH SẴN từ `foundation-fix-then-real-sim`,
    /// luôn 0 tới giờ) bắt đầu có số thật từ cụm này.
    RpcError,
    /// Cụm `exec-path-traps` (F-14) — `decoded.deadline` (khi có, `None` cho
    /// command Universal Router `V2_SWAP_EXACT_IN`/`V3_SWAP_EXACT_IN` — UR
    /// không mang tham số `deadline` per-swap, xem `decoder.rs`) đã hết hạn
    /// hoặc còn dưới `2 * BSC_BLOCK_TIME_SEC` giây so wall-clock hiện tại —
    /// front-run 1 tx sắp hết hạn là bẫy thực thi thật: nếu victim revert vì
    /// deadline trước khi front/back kịp chạy, bot tự sandwich chính mình.
    Deadline,
    VictimWouldRevert,
    Unprofitable,
    HoneypotOrTax,
    /// Cụm `evm-validate-fixed-then-wire` (B3.2) — `sim_engine="evm"` nhưng
    /// `revm`/`AlloyDB` KHÔNG chạy được (lỗi RPC khi fork, state ngoài cửa sổ
    /// non-archive, hoặc EVM halt bất thường). **KHÔNG rơi về `sim_v2` âm
    /// thầm**: công thức đóng không thấy fee-on-transfer/honeypot nên một
    /// fallback im lặng sẽ biến "không biết" thành "có lãi" — đúng loại lỗi
    /// nguy hiểm nhất cho bot thật. Skip rõ ràng, đếm riêng trong funnel, để
    /// Chủ thấy ngay khi RPC không kham nổi tải EVM.
    SimError,
    /// Cụm `exec-path-traps` (F-13) — nonce THẬT của victim (`eth_getTransactionCount`,
    /// xem `transport::NonceCache`) THẤP HƠN nonce của candidate đang xét —
    /// tx này đã bị thay thế/đã lên block với nonce khác, KHÔNG còn khả năng
    /// thực thi ở vị trí quan sát được, front-run nó là vô nghĩa (bot tự
    /// sandwich chính mình vì victim không bao giờ tới).
    NonceStale,
    /// F-13 — nonce THẬT của victim CAO HƠN nonce của candidate — còn khoảng
    /// trống nonce (tx khác của CÙNG ví phải lên block trước), tx này CHƯA
    /// thể thực thi ngay ở block kế tiếp — front-run giả định "chạy ngay sau"
    /// là sai, cùng loại bẫy với `Deadline`.
    NonceFuture,
    /// Cụm `real-economics-mode2` (F-03) — `gas_cost_wei` đo THẬT
    /// (`compute_gas_cost_wei`: gas unit đo bằng revm × max(`eth_gasPrice`,
    /// `victim.gas_price`)) VƯỢT trần cấu hình
    /// (`front_max_gas_bnb_wei + back_max_gas_bnb_wei`, nay CHỈ còn là trần —
    /// không còn dùng thẳng làm chi phí gas nữa), HOẶC `eth_gasPrice` đo được
    /// vượt `gas_price_max_gwei` (mạng đang tắc nghẽn bất thường). Khác
    /// `Unprofitable` (profit dương nhưng dưới `min_profit_bnb`) — đây là gas
    /// tự nó đã quá đắt, chưa cần tính tới profit.
    GasCap,
}

impl PipelineSkip {
    pub fn as_str(&self) -> &'static str {
        match self {
            PipelineSkip::NotInList => "not_in_list",
            PipelineSkip::BelowMin => "below_min",
            PipelineSkip::DecodeFail => "decode_fail",
            PipelineSkip::NotWbnbPair => "not_wbnb_pair",
            PipelineSkip::NotQuotePair => "not_quote_pair",
            PipelineSkip::SellDirection => "sell_direction",
            PipelineSkip::NotPancakeRouter => "not_pancake_router",
            PipelineSkip::VenueUnpinned => "venue_unpinned",
            PipelineSkip::ThinLiq => "thin_liq",
            PipelineSkip::NoPool => "no_pool",
            PipelineSkip::RpcError => "rpc_error",
            PipelineSkip::Deadline => "deadline",
            PipelineSkip::VictimWouldRevert => "victim_would_revert",
            PipelineSkip::Unprofitable => "unprofitable",
            PipelineSkip::HoneypotOrTax => "honeypot_or_tax",
            PipelineSkip::SimError => "sim_error",
            PipelineSkip::NonceStale => "nonce_stale",
            PipelineSkip::NonceFuture => "nonce_future",
            PipelineSkip::GasCap => "gas_cap",
        }
    }
}

/// Cụm `real-economics-mode2` (F-03) — chi phí gas THẬT (wei BNB) cho 1 cặp
/// front+back, thuần/sync/test-được (không gọi RPC ở đây, caller đã tự fetch
/// `oracle_gas_price_wei` qua `transport::GasOracle` + `victim_gas_price_wei`
/// từ chính tx quan sát được). `oracle_gas_price_wei > gas_price_max_wei`
/// (mạng tắc nghẽn bất thường) → trả `u128::MAX` (sentinel CHẮC CHẮN vượt bất
/// kỳ trần `cfg.gas_wei()` nào) để gate `gas_cap` ở `evaluate_candidate`/
/// `evaluate_candidate_quote` tự động kích hoạt, không cần thêm nhánh so sánh
/// riêng cho lý do (e) trong CLAUDE.md — gộp cả 2 điều kiện (c)+(e) vào ĐÚNG 1
/// chỗ trả `gas_cap`.
pub fn compute_gas_cost_wei(
    gas_units_front: u64,
    gas_units_back: u64,
    oracle_gas_price_wei: u128,
    victim_gas_price_wei: u128,
    gas_price_max_wei: u128,
) -> u128 {
    if gas_price_max_wei > 0 && oracle_gas_price_wei > gas_price_max_wei {
        return u128::MAX;
    }
    let effective_price = oracle_gas_price_wei.max(victim_gas_price_wei);
    (gas_units_front as u128 + gas_units_back as u128).saturating_mul(effective_price)
}

/// Cụm `usdt-quote-asset` (BAOCAO29) — quote asset của 1 candidate: WBNB
/// (hành vi cổ điển, không đổi) hoặc USDT (mới, chỉ khi `scan_quote_usdt=true`).
/// KHÔNG lẫn với `PoolReserves` — struct đó vẫn giữ tên field `reserve_wbnb`/
/// `reserve_token` (không đổi, tránh phá `sim_v2.rs`/mọi test cũ) nhưng được
/// TÁI DÙNG generic cho cả 2 quote asset ở entrypoint mới (`decide_paper_quote`):
/// `reserve_wbnb` mang nghĩa "reserve của quote asset hiện tại" (WBNB hoặc
/// USDT tuỳ `QuoteAsset`), vì math AMM constant-product của `sim_v2.rs` không
/// quan tâm định danh token, chỉ cần đúng cặp reserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteAsset {
    Wbnb,
    Usdt,
}

impl QuoteAsset {
    pub fn as_str(&self) -> &'static str {
        match self {
            QuoteAsset::Wbnb => "wbnb",
            QuoteAsset::Usdt => "usdt",
        }
    }
}

#[derive(Debug, Clone)]
pub enum PipelineOutcome {
    Skip(PipelineSkip),
    Simulated(SandwichQuote),
}

fn wbnb() -> Address {
    Address::from_str(WBNB_ADDRESS).expect("WBNB_ADDRESS phai hop le")
}

/// Cụm `usdt-quote-asset` (BAOCAO29).
fn usdt() -> Address {
    Address::from_str(USDT_ADDRESS).expect("USDT_ADDRESS phai hop le")
}

/// Input đã có đủ dữ liệu on-chain cần thiết (reserve pool V2 hiện tại +
/// block hiện tại) — phần FETCH thật (RPC) tách riêng, không nằm trong struct
/// này (giữ `decide_paper` thuần/sync/test-được-ngay).
pub struct PaperDecision<'a> {
    pub from: Address,
    pub calldata: &'a [u8],
    pub tx_value: U256,
    pub reserves: PoolReserves,
    pub current_block: u64,
}

/// Cụm 4.1 lõi (mở rộng cụm config-hot-reload) — thuần, sync, test bằng
/// fixture tay (reserve + tx), không gọi RPC. MỌI ngưỡng so sánh đọc thẳng
/// từ `cfg: &Config` (`min_profit_wei`/`effective_front_cap_wei` [cụm `5.2`,
/// gộp `max_front_wei` + `max_exposure_wei`, xem `config.rs`]/`min_reserve_wei`/
/// `max_roundtrip_tax_bps`/`gas_wei`/`tax_cache_blocks`) — không có literal
/// ngưỡng nào hardcode ở đây; chủ đổi `config.toml` (kể cả hot-reload giữa
/// 2 lần gọi) là lần `decide_paper` SAU dùng số mới ngay, không cần build
/// lại. Thứ tự kiểm tra khớp CLAUDE.md mục "Decode được phép"/"Skip":
/// decode -> path WBNB -> victims.txt (not_in_list/below_min) -> thin_liq
/// (pool quá mỏng so `min_reserve_wbnb`) -> tax cache (honeypot_or_tax, cả
/// khi CHƯA đo lẫn khi đo được nhưng vượt `max_roundtrip_tax`) -> sim
/// (victim_would_revert/unprofitable, kể cả lãi dương nhưng dưới
/// `min_profit_bnb`).
/// Phần LÕI thuần của `decide_paper` KHÔNG cần biết reserve pool: decode +
/// path phải là chiều victim MUA (token_a==WBNB) + tra `victims.txt`
/// (not_in_list/below_min). Tách riêng để `5.1` (`precheck_without_reserves`)
/// gọi được TRƯỚC khi tốn 1 vòng RPC resolve pool/reserve cho tx rõ ràng
/// không phải candidate — decode + lookup victims.txt đều thuần/rẻ, không
/// đáng để trì hoãn tới sau khi có reserve thật.
fn decode_and_prefilter(
    victims: &VictimBook,
    from: Address,
    calldata: &[u8],
    tx_value: U256,
) -> Result<(DecodedSwap, Address), PipelineSkip> {
    let decoded = match decoder::decode_swap_calldata(calldata, tx_value) {
        Ok(d) => d,
        Err(DecodeSkip::DecodeFail) => return Err(PipelineSkip::DecodeFail),
        Err(DecodeSkip::NotWbnbPair) => return Err(PipelineSkip::NotWbnbPair),
    };

    let token = match decoded.token() {
        Ok(t) => t,
        Err(_) => return Err(PipelineSkip::NotWbnbPair),
    };

    if decoded.path.token_a != wbnb() {
        // Chieu victim BAN token lay WBNB - ngoai pham vi math sandwich phien
        // nay (xem doc-comment dau file) - khong bia model, khong tinh loi.
        return Err(PipelineSkip::NotWbnbPair);
    }

    let from_hex = format!("{:#x}", from);
    let min_wei = match victims.min_for(&from_hex) {
        Some(m) => m,
        None => return Err(PipelineSkip::NotInList),
    };

    let amount_in_u128: u128 = match decoded.amount_in.try_into() {
        Ok(v) => v,
        Err(_) => return Err(PipelineSkip::Unprofitable), // vuot pham vi BNB thuc te
    };
    if amount_in_u128 < min_wei {
        return Err(PipelineSkip::BelowMin);
    }

    Ok((decoded, token))
}

/// Cụm `5.1` — chạy trước khi gọi RPC resolve pool/reserve trong live loop
/// (`main.rs`): trả `token` cần resolve nếu tx là candidate thật (qua decode
/// + not_in_list/below_min), hoặc lý do skip luôn nếu rõ ràng không cần RPC.
pub fn precheck_without_reserves(
    victims: &VictimBook,
    from: Address,
    calldata: &[u8],
    tx_value: U256,
) -> Result<Address, PipelineSkip> {
    decode_and_prefilter(victims, from, calldata, tx_value).map(|(_, token)| token)
}

/// Cụm `5.1` — resolve pool V2 (factory đã pin) + reserve THẬT qua RPC cho
/// live loop. Gộp chung mọi lý do thất bại (không có pool / `eth_call` lỗi)
/// thành `no_pool` ở tầng gọi này — khác `pool::resolve_v2_pair` (vẫn tách
/// `Err(String)` lỗi RPC khỏi `Ok(Err(NoPool))` "chắc chắn không có pool" cho
/// caller thấp hơn); ở live loop, cả hai đều dẫn tới cùng hành động: không đủ
/// dữ liệu để sim, skip đúng enum `no_pool` đã có sẵn trong CLAUDE.md.
///
/// Cụm pair-mode: trả THÊM `pair_addr` (khác `5.1` chỉ trả `PoolReserves`) —
/// `decide_paper_v2` cần biết đúng địa chỉ pool để tra `PairBook::contains`
/// (pair-mode theo dõi POOL, không theo dõi ví). Đây là điểm DUY NHẤT gọi
/// `pool::resolve_v2_pair` trong live loop nên không tốn thêm round-trip RPC
/// so với `5.1` — chỉ đổi kiểu trả về.
pub async fn resolve_v2_reserves(provider: &dyn Provider, token: Address) -> Result<(Address, PoolReserves), PipelineSkip> {
    let factory = Address::from_str(V2_FACTORY_ADDRESS).expect("V2_FACTORY_ADDRESS da pin phai la address hop le");
    // Cum `hotpath-fix-then-decoder-ur` (A3) - tach RIENG "Factory tra ve
    // address(0), CHAC CHAN khong co pool" (Ok(Err)) khoi "eth_call THAT su
    // loi mang/timeout" (Err) - truoc fix ca 2 gop chung thanh NoPool, khien
    // USDC/WBNB (pool co that, 294 BNB) bi dem nham no_pool khi RPC choang tai.
    let pair = match pool::resolve_v2_pair(provider, factory, token).await {
        Ok(Ok(p)) => p,
        Ok(Err(_)) => return Err(PipelineSkip::NoPool),
        Err(_) => return Err(PipelineSkip::RpcError),
    };
    match pool::get_reserves_vs_wbnb(provider, pair).await {
        Ok((reserve_wbnb, reserve_token)) => Ok((pair, PoolReserves { reserve_wbnb, reserve_token })),
        // pair != address(0) (da qua nhanh tren) -> pool CHAC CHAN ton tai,
        // getReserves that bai o day CHI co the la loi RPC that.
        Err(_) => Err(PipelineSkip::RpcError),
    }
}

/// Cụm `hotpath-fix-then-decoder-ur` (A4a) — như `resolve_v2_reserves` nhưng
/// `pair_addr` ĐÃ BIẾT TRƯỚC (từ `PairBook::known_pair`) — bỏ hẳn `eth_call
/// getPair` (1 round-trip RPC/candidate), chỉ còn `getReserves`. Lỗi RPC ở
/// đây LUÔN `RpcError` (không có nhánh `NoPool` — pool đã biết chắc tồn tại
/// vì từng resolve thành công lúc `PairBook::reload`).
pub async fn resolve_v2_reserves_known_pair(provider: &dyn Provider, pair: Address, quote: Address) -> Result<PoolReserves, PipelineSkip> {
    match pool::get_reserves_vs_quote(provider, pair, quote).await {
        Ok((reserve_quote, reserve_token)) => Ok(PoolReserves { reserve_wbnb: reserve_quote, reserve_token }),
        Err(_) => Err(PipelineSkip::RpcError),
    }
}

/// Decode + xác nhận chiều victim MUA token (`token_a == WBNB`), KHÔNG tra
/// `victims.txt`/`PairBook` (khác `decode_and_prefilter`) — dùng cho
/// `precheck_token_only`/`decide_paper_v2` vì pair-mode cần biết `token` để
/// resolve `pair_addr` TRƯỚC khi biết tx này thuộc wallet-mode hay pair-mode
/// (không thể lọc theo victims.txt sớm như `decode_and_prefilter` cũ, vì
/// pair-mode không quan tâm địa chỉ `from`). Chiều victim BÁN token đã BỊ BỎ
/// HẲN (xem doc-comment đầu file/`docs/STATE.md`) — `token_a != WBNB` luôn
/// `not_wbnb_pair`, không còn nhánh phân loại chiều nào khác.
fn decode_and_classify(calldata: &[u8], tx_value: U256) -> Result<(DecodedSwap, Address), PipelineSkip> {
    let decoded = match decoder::decode_swap_calldata(calldata, tx_value) {
        Ok(d) => d,
        Err(DecodeSkip::DecodeFail) => return Err(PipelineSkip::DecodeFail),
        Err(DecodeSkip::NotWbnbPair) => return Err(PipelineSkip::NotWbnbPair),
    };
    let token = match decoded.token() {
        Ok(t) => t,
        Err(_) => return Err(PipelineSkip::NotWbnbPair),
    };
    if decoded.path.token_a != wbnb() {
        return Err(PipelineSkip::NotWbnbPair);
    }
    Ok((decoded, token))
}

/// Cụm pair-mode — chạy TRƯỚC khi tốn `eth_call` resolve pool trong live loop
/// (`main.rs`): trả `token` nếu decode được (bất kể có trong victims.txt hay
/// không — pair-mode không cần biết `from` sớm), hoặc skip ngay nếu decode
/// thất bại/không phải cặp WBNB. KHÁC `precheck_without_reserves` (`5.1`, vẫn
/// giữ nguyên cho `decide_paper` cũ): hàm đó lọc `not_in_list`/`below_min`
/// SỚM vì chỉ phục vụ wallet-mode; hàm này phải hoãn quyết định wallet-hay-pair
/// tới sau khi biết `pair_addr` (từ `resolve_v2_reserves`).
pub fn precheck_token_only(calldata: &[u8], tx_value: U256) -> Result<Address, PipelineSkip> {
    decode_and_classify(calldata, tx_value).map(|(_, token)| token)
}

/// Cụm `foundation-fix-then-real-sim` (A4) — như `precheck_token_only`
/// nhưng trả kèm `decoder::SwapVenue` thật (V2 hay V3+fee) — `main.rs` dùng
/// để GATE `venue==V3` TRƯỚC khi gọi `resolve_v2_reserves` (bug cũ: V3 bị
/// đưa nhầm vào pool V2, xem doc-comment `PipelineSkip::VenueUnpinned`).
///
/// Cụm `exec-path-traps` (F-16) — trả THÊM `selector_name` (so với bản cũ
/// chỉ trả `(Address, SwapVenue)`) để `main.rs` cross-check được với
/// `venues::Venue(tx.to)` qua `decoder::venue_matches_router` — selector V2
/// cổ điển gửi tới router V3/UR (hoặc ngược lại) là dấu hiệu calldata bất
/// thường/decode nhầm, không nên coi là candidate hợp lệ.
pub fn precheck_token_and_venue(calldata: &[u8], tx_value: U256) -> Result<(Address, decoder::SwapVenue, &'static str), PipelineSkip> {
    decode_and_classify(calldata, tx_value).map(|(decoded, token)| (token, decoded.venue, decoded.selector_name))
}

/// Input đầy đủ cho `decide_paper_v2` — thêm `pair_addr` (đã resolve qua
/// `resolve_v2_reserves`) so với `PaperDecision` gốc, để tra `PairBook`.
pub struct PaperDecisionV2<'a> {
    pub from: Address,
    pub calldata: &'a [u8],
    pub tx_value: U256,
    pub reserves: PoolReserves,
    pub pair_addr: Address,
    pub current_block: u64,
    /// Cụm `real-economics-mode2` (F-03) — chi phí gas THẬT (wei BNB), đã
    /// tính sẵn bởi caller (`main.rs::handle_paper_tx`, qua
    /// `compute_gas_cost_wei`) — `pipeline.rs` không gọi RPC nên không tự
    /// tính được `eth_gasPrice`/gas unit đo bằng revm.
    pub gas_cost_wei: u128,
}

/// Lõi đánh giá 1 candidate (đã biết chắc `token`/ngưỡng min-size, luôn
/// chiều victim MUA — xem doc-comment đầu file) — DÙNG CHUNG cho cả
/// wallet-mode và pair-mode trong
/// `decide_paper_v2` (chỉ khác nguồn `min_threshold_wei`: `victims.txt` theo
/// từng ví, hay `pairs_min_swap_bnb` GLOBAL). Thứ tự kiểm tra khớp
/// `decide_paper` gốc + thêm cổng `RiskGuard` (7.1/pair-mode wire
/// `max_consecutive_loss`/`gas_reserve_bnb_wei`, xem `config.rs::RiskGuard`)
/// ngay trước sim.
/// Cụm `exec-path-traps` (F-14) — ước lượng thời gian block BSC (giây),
/// KHÔNG phải số đo thật per-block (không có `Provider` ở tầng thuần này) —
/// dùng để tính buffer `2 * BSC_BLOCK_TIME_SEC` giây trước khi coi 1
/// `deadline` là "sắp hết hạn" đủ để từ chối candidate. Khớp CLAUDE.md mục
/// Math "BSC ~3s/block".
const BSC_BLOCK_TIME_SEC: u64 = 3;

/// F-14 — `true` khi `deadline` (nếu có) đã hết hạn hoặc còn dưới
/// `2 * BSC_BLOCK_TIME_SEC` giây so `now_unix`. `deadline=None` (command
/// Universal Router `V2_SWAP_EXACT_IN`/`V3_SWAP_EXACT_IN` — UR không mang
/// tham số `deadline` riêng cho từng swap, xem `decoder.rs`) -> KHÔNG áp
/// dụng gate này, luôn `false` (đúng lệnh "UR tx không có deadline → không
/// áp"). `deadline` không ép được về `u64` (quá lớn, thực tế = "rất xa
/// tương lai") -> `false`, không panic.
fn deadline_expired(deadline: Option<U256>, now_unix: u64) -> bool {
    let Some(deadline) = deadline else { return false };
    match u64::try_from(deadline) {
        Ok(d) => d < now_unix.saturating_add(2 * BSC_BLOCK_TIME_SEC),
        Err(_) => false,
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn evaluate_candidate(
    token: Address,
    amount_in: U256,
    amount_out_min: U256,
    deadline: Option<U256>,
    min_threshold_wei: u128,
    reserves: PoolReserves,
    current_block: u64,
    tax_cache: &TaxCache,
    cfg: &Config,
    risk: &RiskGuard,
    gas_cost_wei: u128,
    skip_tax_gate: bool,
) -> PipelineOutcome {
    // F-14 - gate deadline TRUOC TIEN (thuan, 0 RPC, khong phu thuoc
    // amount/reserve) - front-run 1 tx sap het han la bay thuc thi that.
    if deadline_expired(deadline, now_unix()) {
        return PipelineOutcome::Skip(PipelineSkip::Deadline);
    }
    let amount_in_u128: u128 = match amount_in.try_into() {
        Ok(v) => v,
        Err(_) => return PipelineOutcome::Skip(PipelineSkip::Unprofitable), // vuot pham vi BNB thuc te
    };
    if amount_in_u128 < min_threshold_wei {
        return PipelineOutcome::Skip(PipelineSkip::BelowMin);
    }
    if reserves.reserve_wbnb < cfg.min_reserve_wei() {
        return PipelineOutcome::Skip(PipelineSkip::ThinLiq);
    }
    // Cụm `evm-validate-fixed-then-wire` (B3.2/C3) — khi `sim_engine="evm"`,
    // BỎ QUA cổng tax công thức-đóng ở đây: EVM (`run_evm_decision`) sẽ TỰ đo
    // tax bằng revm và tự gate honeypot_or_tax. Nếu vẫn gate ở đây (cache
    // rỗng -> honeypot_or_tax) thì EVM KHÔNG BAO GIỜ chạy (deadlock: cache
    // không đầy vì EVM không chạy vì cache rỗng). `decide_paper_v2` lúc này
    // chỉ còn tính ước lượng `front_in` bằng `sim_v2` để đưa vào EVM.
    //
    // Cụm `real-economics-mode2` (mục 0, fix BUG BAOCAO37) — `skip_tax_gate`
    // (`true` CHỈ cho nhánh pair-mode khi `PairBook::is_tax_ok` xác nhận token
    // đã vet tay VÀ chưa bị vet nền loại) bỏ HẲN qua tra `TaxCache` — token
    // trong `pairs.txt` đã được Chủ vet tay + vet nền định kỳ, tra thêm
    // `TaxCache` (nguồn khác, luôn rỗng trên đường nóng vì không ai tự động
    // điền) chỉ khiến MỌI candidate pair-mode rơi vào `honeypot_or_tax` giả,
    // che mất chuỗi gate kinh tế thật phía dưới (đúng phát hiện BAOCAO37:
    // `honeypot_or_tax=95/phút`, `unprofitable=0`).
    if !cfg.sim_engine_is_evm() && !skip_tax_gate {
        let measurement = match tax_cache.get_fresh(token, current_block, cfg.tax_cache_blocks) {
            Some(m) => m,
            None => return PipelineOutcome::Skip(PipelineSkip::HoneypotOrTax),
        };
        if measurement.roundtrip_tax_bps > cfg.max_roundtrip_tax_bps() {
            return PipelineOutcome::Skip(PipelineSkip::HoneypotOrTax);
        }
    }
    // Cong RiskGuard (7.1/pair-mode) - da lo lien tiep du nguong (do TANG EXECUTOR
    // THAT 7.x goi record_result, xem doc-comment RiskGuard) -> coi nhu unprofitable,
    // dung lai enum san co, khong them skip reason moi ngoai CLAUDE.md.
    if risk.consecutive_loss_exceeded(cfg.max_consecutive_loss) {
        return PipelineOutcome::Skip(PipelineSkip::Unprofitable);
    }
    // Cụm `real-economics-mode2` (F-03) — `gas_cost_wei` (đo THẬT, xem
    // `compute_gas_cost_wei`) so với TRẦN cấu hình (`cfg.gas_wei()`, không
    // còn dùng thẳng làm chi phí gas nữa) — vượt trần thì `gas_cap`, KHÔNG
    // tính sim (tránh phí `search_max_front_in` cho candidate chắc chắn gas
    // quá đắt).
    if gas_cost_wei > cfg.gas_wei() {
        return PipelineOutcome::Skip(PipelineSkip::GasCap);
    }
    let front_cap = RiskGuard::front_cap_after_gas_reserve(cfg.effective_front_cap_wei(), cfg.gas_reserve_bnb_wei);
    let quote = match sim_v2::search_max_front_in(reserves, amount_in, front_cap, gas_cost_wei) {
        Some(q) => q,
        None => return PipelineOutcome::Skip(PipelineSkip::Unprofitable),
    };
    if !sim_v2::victim_still_ok(quote.victim_out, amount_out_min) {
        return PipelineOutcome::Skip(PipelineSkip::VictimWouldRevert);
    }
    if quote.profit_wei <= 0 {
        return PipelineOutcome::Skip(PipelineSkip::Unprofitable);
    }
    if U256::from(quote.profit_wei as u128) < cfg.min_profit_wei() {
        return PipelineOutcome::Skip(PipelineSkip::Unprofitable);
    }
    PipelineOutcome::Simulated(quote)
}

/// Cụm pair-mode — entry point MỚI, dual-branch (wallet|pair), chạy SONG SONG
/// với `decide_paper` gốc (KHÔNG thay thế — `decide_paper` vẫn nguyên vẹn,
/// mọi test cũ vẫn pass). Ưu tiên: `cfg.wallet_scan_enabled == true` (ship
/// `true`) VÀ `tx.from` khớp `victims.txt` -> wallet mode (win, KHÔNG check
/// pair, đúng lệnh); `cfg.pair_scan_enabled == true` (ship `true`) VÀ không
/// khớp wallet mà `pair_addr` khớp `PairBook` -> pair mode (ngưỡng
/// `pairs_min_swap_bnb` GLOBAL); không khớp cả hai (hoặc mode đó đang bị tắt
/// qua `wallet_scan_enabled`/`pair_scan_enabled`) NHƯNG `cfg.pair_scan_universal
/// == true` (cụm `universal-pair-scan`, ship `false`) -> universal mode
/// (CÙNG ngưỡng GLOBAL `pairs_min_swap_bnb`, không thêm ngưỡng riêng); không
/// khớp gì (hoặc universal đang tắt) -> `not_in_list`. Trả kèm `source`
/// ("wallet"/"pair"/"universal"/"none") để log field `source` (CLAUDE.md lệnh
/// pair-mode/universal-pair-scan). Thứ tự ưu tiên GIỮ NGUYÊN khi nhiều hơn 1
/// mode bật cùng lúc: wallet > pair (đã liệt kê) > universal (mới) >
/// not_in_list — `wallet_scan_enabled`/`pair_scan_enabled` (cụm
/// `explicit-mode-flags`) chỉ TẮT HẲN 1 nhánh khỏi việc xét, KHÔNG đổi thứ tự
/// ưu tiên giữa các nhánh còn bật. Code KHÔNG ép buộc "chỉ 1 mode true" — đó
/// là Chủ tự quản lý qua `config.toml`, không phải validate/fail-load.
pub fn decide_paper_v2(
    victims: &VictimBook,
    pairbook: &PairBook,
    tax_cache: &TaxCache,
    cfg: &Config,
    risk: &RiskGuard,
    input: &PaperDecisionV2,
) -> (PipelineOutcome, &'static str) {
    let (decoded, token) = match decode_and_classify(input.calldata, input.tx_value) {
        Ok(v) => v,
        Err(skip) => return (PipelineOutcome::Skip(skip), "none"),
    };

    let from_hex = format!("{:#x}", input.from);
    if cfg.wallet_scan_enabled {
        if let Some(min_wei) = victims.min_for(&from_hex) {
            let outcome = evaluate_candidate(
                token,
                decoded.amount_in,
                decoded.amount_out_min,
                decoded.deadline,
                min_wei,
                input.reserves,
                input.current_block,
                tax_cache,
                cfg,
                risk,
                input.gas_cost_wei,
                false, // wallet-mode khong co co che vet PairBook - luon tra TaxCache
            );
            return (outcome, "wallet");
        }
    }

    if cfg.pair_scan_enabled && pairbook.knows_pool(&input.pair_addr) {
        // Cum `real-economics-mode2` (muc 0) - pool DA BIET nhung vet NEN vua
        // loai (tax/honeypot do bang EVM that) -> tra honeypot_or_tax NGAY,
        // KHONG de roi xuong not_in_list/universal (mat visibility) - khac
        // `contains()` (loai vet_failed hoan toan khoi candidate).
        if pairbook.is_vet_failed(&input.pair_addr) {
            return (PipelineOutcome::Skip(PipelineSkip::HoneypotOrTax), "pair");
        }
        let min_wei: u128 = cfg.pairs_min_swap_wei().try_into().unwrap_or(u128::MAX);
        // Cum `real-economics-mode2` (muc 0) - fix bug BAOCAO37: token da vet
        // tay + chua bi vet nen loai -> bo qua TaxCache (xem
        // PairBook::is_tax_ok / doc-comment evaluate_candidate).
        let skip_tax_gate = pairbook.is_tax_ok(&input.pair_addr);
        let outcome = evaluate_candidate(
            token,
            decoded.amount_in,
            decoded.amount_out_min,
            decoded.deadline,
            min_wei,
            input.reserves,
            input.current_block,
            tax_cache,
            cfg,
            risk,
            input.gas_cost_wei,
            skip_tax_gate,
        );
        return (outcome, "pair");
    }

    if cfg.pair_scan_universal {
        let min_wei: u128 = cfg.pairs_min_swap_wei().try_into().unwrap_or(u128::MAX);
        let outcome = evaluate_candidate(
            token,
            decoded.amount_in,
            decoded.amount_out_min,
            decoded.deadline,
            min_wei,
            input.reserves,
            input.current_block,
            tax_cache,
            cfg,
            risk,
            input.gas_cost_wei,
            false, // universal-mode quet MOI pool WBNB, khong co vet tay -> tra TaxCache
        );
        return (outcome, "universal");
    }

    (PipelineOutcome::Skip(PipelineSkip::NotInList), "none")
}

/// Cụm `7.3` (BAOCAO16) — bọc `decide_paper_v2` (GIỮ NGUYÊN, không đổi chữ
/// ký/hành vi — mọi call site cũ, kể cả `main.rs::handle_paper_tx`, vẫn biên
/// dịch/chạy y hệt): khi kết quả là `Simulated` (đã qua MỌI cổng, kể cả
/// `min_profit_bnb`), build THÊM 2 tx paper (front-buy/back-sell) qua
/// `executor::build_and_log_paper_sandwich` rồi GHI LOG (`tx.build`) — CHỈ
/// LOG, không ký/gửi gì (xem doc-comment đầu `executor.rs`, không có
/// `Provider`/`Signer` nào trong đường đi này).
///
/// Hàm MỚI, tách biệt hoàn toàn — `main.rs::handle_paper_tx` CHƯA đổi sang
/// gọi hàm này (`main.rs` không nằm trong ĐƯỢC ĐỤNG phiên `BAOCAO16`, xem ô 10
/// BAOCAO16): hàm này SẴN SÀNG, phiên sau chỉ cần đổi 1 lời gọi
/// `decide_paper_v2(...)` thành `decide_and_build_paper_v2(..., &app_state.logger, ...)`
/// ở `main.rs` (cùng kiểu trả về `(PipelineOutcome, &'static str)`, không đổi
/// logic đọc kết quả phía sau).
///
/// `token` cần cho calldata front-buy/back-sell nhưng `decide_paper_v2` không
/// trả trong tuple kết quả (giữ nguyên chữ ký cũ) — gọi lại `decode_and_classify`
/// (thuần, rẻ, cùng calldata/tx_value đã decode 1 lần bên trong `decide_paper_v2`,
/// nên PHẢI trả `Ok` y hệt lần trước, không có nhánh lỗi mới nào phát sinh).
///
/// Cụm `exec-path-traps` (F-26) — CHỈ build NGAY ở đây khi `cfg.sim_engine ==
/// "v2"` (đường lùi, hành vi build-ngay-khi-Simulated GIỮ NGUYÊN như trước —
/// đúng lệnh "sim_engine=v2 vẫn build theo v2"). Khi `cfg.sim_engine ==
/// "evm"` (ship mặc định), hàm này KHÔNG build gì nữa dù `decide_paper_v2` ra
/// `Simulated` — `Simulated` lúc đó CHỈ là ước lượng công thức đóng, CHƯA
/// được EVM thật xác nhận (xem `docs/STATE.md`/`CLAUDE.md` mục Math). Build
/// thật cho đường `evm` dời sang `build_paper_txs_from_evm_decision` (dưới),
/// gọi SAU khi `decide_with_evm` đã chạy — đo THẬT phiên trước sửa: 501
/// `tx.build` / 1 `simulated` (audit F-26), vì hàm này build TRƯỚC khi EVM
/// phán quyết.
pub fn decide_and_build_paper_v2(
    victims: &VictimBook,
    pairbook: &PairBook,
    tax_cache: &TaxCache,
    cfg: &Config,
    risk: &RiskGuard,
    logger: &BotLogger,
    input: &PaperDecisionV2,
) -> (PipelineOutcome, &'static str) {
    let (outcome, source) = decide_paper_v2(victims, pairbook, tax_cache, cfg, risk, input);
    if !cfg.sim_engine_is_evm() {
        if let PipelineOutcome::Simulated(quote) = &outcome {
            if let Ok((_, token)) = decode_and_classify(input.calldata, input.tx_value) {
                executor::build_and_log_paper_sandwich(logger, cfg, input.from, token, quote, "v2");
            }
        }
    }
    (outcome, source)
}

/// Cụm `exec-path-traps` (F-26 + F-07) — điểm build DUY NHẤT cho đường
/// `sim_engine="evm"`: CHỈ build khi `decision.outcome == Simulated` VÀ
/// `decision.evm.is_some()` (EVM thật ĐÃ chạy và xác nhận, không phải chỉ
/// công thức đóng). `decision.outcome`'s `SandwichQuote.front_out` lúc này
/// ĐÃ được `decide_with_evm` thay bằng `evm.token_received` (số token THẬT
/// attacker nhận sau front-buy, đã trừ tax mua nếu có) — nên
/// `build_back_sell_paper_tx` (gọi bên trong `build_and_log_paper_sandwich`)
/// tự động dùng ĐÚNG số đó làm `amountIn` back-sell, không phải ước lượng
/// `sim_v2` (F-07: "back-sell amountIn = balanceOf(self) tại thời điểm build,
/// LẤY TỪ evm.token_received CHỨ KHÔNG PHẢI front_out của sim_v2" — đã đúng
/// nhờ thay thế này, không cần đọc `balanceOf` riêng lần nữa).
pub fn build_paper_txs_from_evm_decision(
    logger: &BotLogger,
    cfg: &Config,
    victim_from: Address,
    token: Address,
    decision: &EvmDecision,
) -> Option<(crate::executor::PaperTxLog, crate::executor::PaperTxLog)> {
    match (&decision.outcome, &decision.evm) {
        (PipelineOutcome::Simulated(quote), Some(_)) => {
            executor::build_and_log_paper_sandwich(logger, cfg, victim_from, token, quote, "evm")
        }
        _ => None,
    }
}

// ===== Cụm `usdt-quote-asset` (BAOCAO29) — quote asset thứ 2 (USDT), SONG
// SONG toàn bộ code WBNB phía trên (KHÔNG đổi 1 dòng nào của
// `decode_and_classify`/`decide_paper_v2`/`decide_and_build_paper_v2`/
// `decide_paper` — vẫn dùng "not_wbnb_pair", vẫn WBNB-only y hệt trước phiên
// này). Entrypoint mới `decide_paper_quote` CHƯA được nối vào `main.rs` (nằm
// ngoài `ĐƯỢC ĐỤNG` phiên này, xem `docs/STATE.md` mục `usdt-quote-asset`) —
// hàm SẴN SÀNG, phiên sau nối dây nếu Grok ra lệnh, cùng khuôn tiền lệ `7.2`/
// `7.3`/`relay-bundle-builder`.

/// Decode + phân loại quote asset (WBNB HOẶC USDT) — chiều victim MUA
/// (`token_a` = quote asset, khớp hướng "input trước" của mọi selector đã
/// decode, xem `decode_and_classify`). Thử WBNB TRƯỚC (luôn bật, không phụ
/// thuộc `scan_quote_usdt`); không khớp mà `scan_quote_usdt=true` thì thử
/// USDT. Không khớp cái nào (hoặc USDT đang tắt) -> `NotQuotePair`.
fn decode_and_classify_quote(
    calldata: &[u8],
    tx_value: U256,
    scan_quote_usdt: bool,
) -> Result<(DecodedSwap, Address, QuoteAsset), PipelineSkip> {
    let decoded = match decoder::decode_swap_calldata(calldata, tx_value) {
        Ok(d) => d,
        Err(DecodeSkip::DecodeFail) => return Err(PipelineSkip::DecodeFail),
        Err(DecodeSkip::NotWbnbPair) => return Err(PipelineSkip::NotQuotePair),
    };
    if decoded.path.token_a == wbnb() {
        if let Some(token) = decoded.path.token_vs(wbnb()) {
            return Ok((decoded, token, QuoteAsset::Wbnb));
        }
    }
    if scan_quote_usdt && decoded.path.token_a == usdt() {
        if let Some(token) = decoded.path.token_vs(usdt()) {
            return Ok((decoded, token, QuoteAsset::Usdt));
        }
    }
    // Cụm `quote-live-wiring-funnel-diagnostics` — token_a khong phai quote
    // nao (nhanh tren khong khop), nhung mot quote asset dang bat VAN co mat
    // trong path o vi tri token_b -> day la chieu victim BAN (khong phai
    // MUA) mot cap quote/token da biet, khong phai "khong lien quan gi toi
    // quote asset nao ca". `token_vs` tra Some khi MOT trong 2 dau path
    // trung quote (bat ke thu tu) nen day la kiem tra CHINH XAC, khong doan.
    let wbnb_side_present = decoded.path.token_vs(wbnb()).is_some();
    let usdt_side_present = scan_quote_usdt && decoded.path.token_vs(usdt()).is_some();
    if wbnb_side_present || usdt_side_present {
        return Err(PipelineSkip::SellDirection);
    }
    Err(PipelineSkip::NotQuotePair)
}

/// Cụm `usdt-quote-asset` — chạy TRƯỚC khi tốn `eth_call` resolve pool, cùng
/// vai trò `precheck_without_reserves`/`precheck_token_only` nhưng quote-aware:
/// trả `(token, quote)` nếu decode được, hoặc skip sớm nếu rõ ràng không phải
/// candidate (decode_fail/not_quote_pair).
pub fn precheck_quote_only(calldata: &[u8], tx_value: U256, scan_quote_usdt: bool) -> Result<(Address, QuoteAsset), PipelineSkip> {
    decode_and_classify_quote(calldata, tx_value, scan_quote_usdt).map(|(_, token, quote)| (token, quote))
}

/// Cụm `usdt-quote-asset` — resolve pool V2 (factory đã pin) + reserve THẬT
/// qua RPC, ĐÚNG quote asset (`pool::resolve_v2_pair_for_quote`/
/// `pool::get_reserves_vs_quote`, tổng quát hoá mới) — song song
/// `resolve_v2_reserves` (WBNB-only, KHÔNG đổi). Gộp mọi lỗi (không có pool /
/// `eth_call` lỗi) thành `NoPool`, cùng quy ước `resolve_v2_reserves`.
pub async fn resolve_reserves_for_quote(
    provider: &dyn Provider,
    token: Address,
    quote: QuoteAsset,
) -> Result<(Address, PoolReserves), PipelineSkip> {
    let factory = Address::from_str(V2_FACTORY_ADDRESS).expect("V2_FACTORY_ADDRESS da pin phai la address hop le");
    let quote_addr = match quote {
        QuoteAsset::Wbnb => wbnb(),
        QuoteAsset::Usdt => usdt(),
    };
    // Cum A3 - tach no_pool/rpc_error, cung nguyen tac resolve_v2_reserves.
    let pair = match pool::resolve_v2_pair_for_quote(provider, factory, token, quote_addr).await {
        Ok(Ok(p)) => p,
        Ok(Err(_)) => return Err(PipelineSkip::NoPool),
        Err(_) => return Err(PipelineSkip::RpcError),
    };
    match pool::get_reserves_vs_quote(provider, pair, quote_addr).await {
        Ok((reserve_quote, reserve_token)) => Ok((pair, PoolReserves { reserve_wbnb: reserve_quote, reserve_token })),
        Err(_) => Err(PipelineSkip::RpcError),
    }
}

/// Cụm `usdt-quote-asset` — lõi đánh giá 1 candidate quote-aware, KHÔNG có
/// wallet-mode (đúng CLAUDE.md diff "KHÔNG thêm cột cho wallet-mode ở cụm
/// này" — không có `min_threshold_wei`/`victims.txt` nào ở nhánh này, mọi
/// candidate qua được decode+thin_liq+tax đều được sim, giống kiểu
/// "universal"). Ngưỡng/gas theo ĐÚNG quote asset:
/// - `Wbnb`: y hệt `evaluate_candidate` cũ — `min_reserve_wei`/
///   `effective_front_cap_wei`/`gas_wei()` trừ thẳng vào `profit_wei`/
///   `min_profit_wei`.
/// - `Usdt`: `min_reserve_usdt_wei`/`max_front_usdt_wei` (KHÔNG gộp
///   `max_exposure_bnb` — field đó là BNB, ngoài phạm vi lệnh USDT lần này),
///   `gas_wei=0` khi tính `profit_wei` (ĐÚNG CLAUDE.md "profit_usdt =
///   backUSDT - frontUSDT THUẦN, không trừ gas vào số này"), so với
///   `min_profit_usdt_wei`. Gas VẪN được chặn riêng qua
///   `RiskGuard::front_cap_after_gas_reserve` (trừ `gas_reserve_bnb_wei` khỏi
///   trần front — field BNB có sẵn, KHÔNG quy đổi/không price oracle, đúng
///   CLAUDE.md "gate độc lập, y hệt cơ chế hiện tại").
fn evaluate_candidate_quote(
    quote: QuoteAsset,
    token: Address,
    amount_in: U256,
    amount_out_min: U256,
    deadline: Option<U256>,
    reserves: PoolReserves,
    current_block: u64,
    tax_cache: &TaxCache,
    cfg: &Config,
    gas_cost_bnb_wei: u128,
    gas_cost_in_quote_wei: u128,
    skip_tax_gate: bool,
) -> PipelineOutcome {
    // F-14 - cung gate deadline nhu evaluate_candidate, ap dung ca 2 quote asset.
    if deadline_expired(deadline, now_unix()) {
        return PipelineOutcome::Skip(PipelineSkip::Deadline);
    }
    let (min_reserve_wei, front_cap_raw, min_profit_wei) = match quote {
        QuoteAsset::Wbnb => (cfg.min_reserve_wei(), cfg.effective_front_cap_wei(), cfg.min_profit_wei()),
        QuoteAsset::Usdt => (cfg.min_reserve_usdt_wei(), cfg.max_front_usdt_wei(), cfg.min_profit_usdt_wei()),
    };

    if reserves.reserve_wbnb < min_reserve_wei {
        return PipelineOutcome::Skip(PipelineSkip::ThinLiq);
    }

    // Cụm `evm-validate-fixed-then-wire` — cùng lý do `evaluate_candidate`:
    // sim_engine=evm thì để EVM tự đo tax (tránh deadlock cache rỗng).
    // Cụm `hotpath-fix-then-decoder-ur` (A2) — `skip_tax_gate` (true CHỈ khi
    // `PairBook::is_tax_ok` xác nhận pool USDT đã vet tay + chưa bị vet nền
    // loại, xem `decide_paper_quote`) bỏ qua TaxCache, ÁP DỤNG ĐÚNG luật
    // BAOCAO38 mục 0 cho nhánh USDT — trước fix, nhánh này KHÔNG hề tra
    // PairBook nên MỌI candidate USDT rơi vào honeypot_or_tax giả (TaxCache
    // luôn rỗng, không ai tự động điền).
    if !cfg.sim_engine_is_evm() && !skip_tax_gate {
        let measurement = match tax_cache.get_fresh(token, current_block, cfg.tax_cache_blocks) {
            Some(m) => m,
            None => return PipelineOutcome::Skip(PipelineSkip::HoneypotOrTax),
        };
        if measurement.roundtrip_tax_bps > cfg.max_roundtrip_tax_bps() {
            return PipelineOutcome::Skip(PipelineSkip::HoneypotOrTax);
        }
    }

    // Cụm `real-economics-mode2` (mục 1.d, sửa CLAUDE.md Math "profit_usdt
    // không trừ gas") — trần gas LUÔN so bằng ĐƠN VỊ BNB (gas trả bằng BNB
    // bất kể quote asset nào của pool), `gas_cost_in_quote_wei` (đã quy đổi
    // sẵn bởi caller cho USDT qua reserve WBNB/USDT thật tại block, xem
    // `main.rs`) mới là số THỰC SỰ trừ vào profit bên dưới.
    if gas_cost_bnb_wei > cfg.gas_wei() {
        return PipelineOutcome::Skip(PipelineSkip::GasCap);
    }

    let front_cap = RiskGuard::front_cap_after_gas_reserve(front_cap_raw, cfg.gas_reserve_bnb_wei);
    let quote_result = match sim_v2::search_max_front_in(reserves, amount_in, front_cap, gas_cost_in_quote_wei) {
        Some(q) => q,
        None => return PipelineOutcome::Skip(PipelineSkip::Unprofitable),
    };

    if !sim_v2::victim_still_ok(quote_result.victim_out, amount_out_min) {
        return PipelineOutcome::Skip(PipelineSkip::VictimWouldRevert);
    }
    if quote_result.profit_wei <= 0 {
        return PipelineOutcome::Skip(PipelineSkip::Unprofitable);
    }
    if U256::from(quote_result.profit_wei as u128) < min_profit_wei {
        return PipelineOutcome::Skip(PipelineSkip::Unprofitable);
    }

    PipelineOutcome::Simulated(quote_result)
}

/// Cụm `usdt-quote-asset` — entrypoint MỚI, quote-aware (WBNB hoặc USDT),
/// SONG SONG `decide_paper`/`decide_paper_v2` (2 hàm đó KHÔNG đổi). Trả kèm
/// `&'static str` = `quote.as_str()` ("wbnb"/"usdt") khi decode được, hoặc
/// `"none"` khi skip sớm (decode_fail/not_quote_pair) — dùng cho log (khác ý
/// nghĩa field `source` của `decide_paper_v2`, đây là QUOTE ASSET chứ không
/// phải wallet|pair|universal).
/// Cụm `real-economics-mode2` — `gas_cost_bnb_wei` luôn là chi phí gas THẬT
/// tính bằng BNB (gas trả bằng BNB, không phụ thuộc quote asset của pool).
/// `gas_cost_in_quote_wei` là số THỰC SỰ trừ vào `profit`, ĐÃ quy đổi sẵn bởi
/// caller sang đúng đơn vị quote asset (bằng chính `gas_cost_bnb_wei` khi
/// `quote=Wbnb` — không cần quy đổi; quy đổi qua reserve WBNB/USDT thật khi
/// `quote=Usdt`, xem `main.rs::handle_paper_tx`).
/// Cụm `hotpath-fix-then-decoder-ur` (A2) — thêm `pairbook`/`pair_addr` (so
/// với bản gốc `usdt-quote-asset` không hề biết `PairBook`). Nhánh `Usdt` áp
/// ĐÚNG luật MODE 2 ONLY/BAOCAO38 mục 0 (giống nhánh "pair" của
/// `decide_paper_v2`): pool KHÔNG có trong `pairs.txt` -> `not_in_list` (KHÔNG
/// còn hành vi "universal" ngầm định cho USDT — trái `strategy-lock-mode2`);
/// vet nền vừa loại -> `honeypot_or_tax` (giữ visibility, không rơi im lặng);
/// đã vet tay + chưa vet_fail -> bỏ qua TaxCache. Nhánh `Wbnb` (KHÔNG dùng
/// trong production qua hàm này — `main.rs` luôn xử lý WBNB qua
/// `decide_paper_v2` trước, hàm này chỉ còn nhánh WBNB cho mục đích đối
/// chứng/test) GIỮ NGUYÊN hành vi cũ (luôn tra TaxCache), không đụng
/// `pair_addr`/`pairbook`.
pub fn decide_paper_quote(
    pairbook: &PairBook,
    pair_addr: Address,
    tax_cache: &TaxCache,
    cfg: &Config,
    calldata: &[u8],
    tx_value: U256,
    reserves: PoolReserves,
    current_block: u64,
    gas_cost_bnb_wei: u128,
    gas_cost_in_quote_wei: u128,
) -> (PipelineOutcome, &'static str) {
    let (decoded, token, quote) = match decode_and_classify_quote(calldata, tx_value, cfg.scan_quote_usdt) {
        Ok(v) => v,
        Err(skip) => return (PipelineOutcome::Skip(skip), "none"),
    };

    let skip_tax_gate = if quote == QuoteAsset::Usdt {
        if !pairbook.knows_pool(&pair_addr) {
            return (PipelineOutcome::Skip(PipelineSkip::NotInList), "none");
        }
        if pairbook.is_vet_failed(&pair_addr) {
            return (PipelineOutcome::Skip(PipelineSkip::HoneypotOrTax), "usdt");
        }
        pairbook.is_tax_ok(&pair_addr)
    } else {
        false
    };

    let outcome = evaluate_candidate_quote(
        quote,
        token,
        decoded.amount_in,
        decoded.amount_out_min,
        decoded.deadline,
        reserves,
        current_block,
        tax_cache,
        cfg,
        gas_cost_bnb_wei,
        gas_cost_in_quote_wei,
        skip_tax_gate,
    );
    (outcome, quote.as_str())
}

/// Cụm `real-economics-mode2` (mục 1.d) — quy đổi `gas_cost_bnb_wei` (BNB)
/// sang đơn vị USDT qua tỉ giá reserve THẬT của pool WBNB/USDT tại block hiện
/// tại (`reserve_wbnb`/`reserve_usdt` của pool đó — KHÔNG phải price oracle,
/// không phải pool token/USDT đang xét). `reserve_wbnb=0` (không thể xảy ra
/// với pool USDT thật đã pin, nhưng tự vệ input rác) → trả `u128::MAX` (sentinel
/// "không quy đổi được, coi như gas vô cùng đắt" — AN TOÀN hơn trả 0).
pub fn convert_gas_cost_bnb_to_usdt(gas_cost_bnb_wei: u128, reserve_wbnb: U256, reserve_usdt: U256) -> u128 {
    if reserve_wbnb.is_zero() {
        return u128::MAX;
    }
    let gas_u256 = U256::from(gas_cost_bnb_wei);
    match gas_u256.checked_mul(reserve_usdt).map(|n| n / reserve_wbnb) {
        Some(v) => u128::try_from(v).unwrap_or(u128::MAX),
        None => u128::MAX,
    }
}

pub fn decide_paper(victims: &VictimBook, tax_cache: &TaxCache, cfg: &Config, input: &PaperDecision) -> PipelineOutcome {
    let (decoded, token) = match decode_and_prefilter(victims, input.from, input.calldata, input.tx_value) {
        Ok(v) => v,
        Err(skip) => return PipelineOutcome::Skip(skip),
    };

    if input.reserves.reserve_wbnb < cfg.min_reserve_wei() {
        return PipelineOutcome::Skip(PipelineSkip::ThinLiq);
    }

    let measurement = match tax_cache.get_fresh(token, input.current_block, cfg.tax_cache_blocks) {
        Some(m) => m,
        None => return PipelineOutcome::Skip(PipelineSkip::HoneypotOrTax),
    };
    if measurement.roundtrip_tax_bps > cfg.max_roundtrip_tax_bps() {
        return PipelineOutcome::Skip(PipelineSkip::HoneypotOrTax);
    }

    let quote = match sim_v2::search_max_front_in(input.reserves, decoded.amount_in, cfg.effective_front_cap_wei(), cfg.gas_wei()) {
        Some(q) => q,
        None => return PipelineOutcome::Skip(PipelineSkip::Unprofitable),
    };

    if !sim_v2::victim_still_ok(quote.victim_out, decoded.amount_out_min) {
        return PipelineOutcome::Skip(PipelineSkip::VictimWouldRevert);
    }

    if quote.profit_wei <= 0 {
        return PipelineOutcome::Skip(PipelineSkip::Unprofitable);
    }
    // profit_wei > 0 da xac nhan o tren -> ep u128 an toan (khong tran, bien
    // do nam trong pham vi BNB thuc te, xem doc-comment dau sim_v2.rs).
    if U256::from(quote.profit_wei as u128) < cfg.min_profit_wei() {
        return PipelineOutcome::Skip(PipelineSkip::Unprofitable);
    }

    PipelineOutcome::Simulated(quote)
}

// ===== Cụm `evm-validate-fixed-then-wire` (B3.2) — EVM THẬT là động cơ
// quyết định cuối cùng =====

/// Kết quả nối `sim_evm` vào pipeline — giữ kèm số liệu EVM để log `sim.evm`.
pub struct EvmDecision {
    pub outcome: PipelineOutcome,
    /// `None` khi không chạy EVM (engine = "v2", hoặc bước trước đã skip).
    pub evm: Option<crate::sim_evm::EvmSandwichOutcome>,
    pub tried: u32,
    pub total_ms: f64,
}

/// B3.2 — chạy SAU `decide_paper_v2`/`decide_paper_quote`: nếu bước đó đã skip
/// thì giữ nguyên skip (không tốn EVM); nếu ra `Simulated(quote_v2)` thì
/// `quote_v2` chỉ còn là **ước lượng khoảng `front_in`**, và mọi kết luận
/// `Simulated`/`victim_would_revert`/`unprofitable` được QUYẾT ĐỊNH LẠI bằng
/// EVM thật trên `fork` (fork tại block hiện tại, dùng chung cho mọi tx cùng
/// block).
///
/// Quy tắc (đúng CLAUDE.md — công thức đóng không thấy tax/honeypot):
/// - EVM lỗi hoàn toàn -> `sim_error` (KHÔNG rơi về `sim_v2` âm thầm).
/// - `victim_success == false` -> `victim_would_revert` (victim thật sẽ revert
///   nếu bị front-run ở mức này -> không được phép sandwich).
/// - `profit_evm <= 0` hoặc `< min_profit` -> `unprofitable`.
/// - còn lại -> `Simulated` với `SandwichQuote` đã THAY `front_in`/`back_out`/
///   `profit_wei` bằng SỐ ĐO EVM THẬT (trừ gas theo đúng quote asset), để mọi
///   bước sau (`executor` build calldata, log, web) dùng số thật chứ không
///   dùng số công thức đóng.
pub fn decide_with_evm(
    fork: &mut crate::sim_evm::BlockForkCache,
    cfg: &Config,
    token: Address,
    victim: &crate::transport::PendingTxRaw,
    prior: PipelineOutcome,
    quote_asset: QuoteAsset,
) -> EvmDecision {
    let quote_v2 = match &prior {
        PipelineOutcome::Simulated(q) => q.clone(),
        PipelineOutcome::Skip(_) => {
            return EvmDecision { outcome: prior, evm: None, tried: 0, total_ms: 0.0 };
        }
    };

    let (max_front, gas_wei, min_profit) = match quote_asset {
        QuoteAsset::Wbnb => (cfg.effective_front_cap_wei(), cfg.gas_wei(), cfg.min_profit_wei()),
        // Quote USDT: profit THUAN USDT, KHONG tru gas (CLAUDE.md muc Math).
        QuoteAsset::Usdt => (cfg.max_front_usdt_wei(), 0u128, cfg.min_profit_usdt_wei()),
    };
    let front_cap = RiskGuard::front_cap_after_gas_reserve(max_front, cfg.gas_reserve_bnb_wei);

    let (evm, tried, total_ms) =
        match crate::sim_evm::refine_front_in_on_fork(fork, token, victim, quote_v2.front_in, front_cap) {
            Ok(v) => v,
            // B3.5 — revert EVM (front/back/approve `!is_success`) là tín hiệu
            // honeypot/anti-bot THẬT, KHÔNG phải RPC lỗi -> `honeypot_or_tax`
            // (an toàn). Chỉ lỗi fork/RPC thật mới là `sim_error`.
            Err(e) => {
                let skip = if e.is_revert() { PipelineSkip::HoneypotOrTax } else { PipelineSkip::SimError };
                return EvmDecision { outcome: PipelineOutcome::Skip(skip), evm: None, tried: 0, total_ms: 0.0 };
            }
        };

    if !evm.victim_success {
        return EvmDecision {
            outcome: PipelineOutcome::Skip(PipelineSkip::VictimWouldRevert),
            evm: Some(evm),
            tried,
            total_ms,
        };
    }

    // `profit_wei` cua sim_evm KHONG tru gas (gas_price=0 cho moi tx attacker,
    // xem doc-comment dau sim_evm.rs) - tru gas O DAY dung y nghia quote asset.
    let profit_after_gas = evm.profit_wei - (gas_wei as i128);
    if profit_after_gas <= 0 || U256::from(profit_after_gas as u128) < min_profit {
        return EvmDecision {
            outcome: PipelineOutcome::Skip(PipelineSkip::Unprofitable),
            evm: Some(evm),
            tried,
            total_ms,
        };
    }

    let quote = sim_v2::SandwichQuote {
        front_in: evm.front_in,
        front_out: evm.token_received,
        // `victim_out` KHONG do duoc truc tiep tu `EvmSandwichOutcome` (hien
        // chi tra so cua ATTACKER) - giu so uoc luong `sim_v2` va ghi ro o
        // day thay vi bia. Cong dung duy nhat cua field nay sau buoc nay la
        // hien thi/log; cong `victim_would_revert` DA duoc quyet dinh bang
        // `evm.victim_success` THAT o tren.
        victim_out: quote_v2.victim_out,
        back_out: evm.back_out,
        profit_wei: profit_after_gas,
    };
    EvmDecision { outcome: PipelineOutcome::Simulated(quote), evm: Some(evm), tried, total_ms }
}

/// B3.2 — ghi `sim.evm` (số EVM thật + block fork đã pin + thời gian) vào
/// `logs/bot.jsonl`. Gọi cho MỌI candidate đã chạy EVM (kể cả khi kết quả là
/// skip) — đó là điểm soi chính khi Chủ muốn biết vì sao 1 tx không thành
/// `Simulated`.
pub fn log_sim_evm(
    logger: &BotLogger,
    from: Address,
    token: Address,
    fork_block: u64,
    d: &EvmDecision,
    quote_asset: QuoteAsset,
) {
    let (front_in, token_received, back_out, profit_raw, victim_success, buy_bps, sell_bps) = match &d.evm {
        Some(e) => (
            e.front_in.to_string(),
            e.token_received.to_string(),
            e.back_out.to_string(),
            e.profit_wei.to_string(),
            Some(e.victim_success),
            e.buy_tax_bps,
            e.sell_tax_bps,
        ),
        None => ("0".into(), "0".into(), "0".into(), "0".into(), None, None, None),
    };
    let decision = match &d.outcome {
        PipelineOutcome::Simulated(_) => "simulated",
        PipelineOutcome::Skip(s) => s.as_str(),
    };
    let profit_after_gas = match &d.outcome {
        PipelineOutcome::Simulated(q) => Some(q.profit_wei.to_string()),
        _ => None,
    };
    logger.log(
        "sim.evm",
        serde_json::json!({
            "from": format!("{:#x}", from),
            "token": format!("{:#x}", token),
            "quote": quote_asset.as_str(),
            "fork_block": fork_block,
            "front_in": front_in,
            "token_received": token_received,
            "back_out": back_out,
            "profit_raw_no_gas": profit_raw,
            "profit_after_gas": profit_after_gas,
            "victim_success": victim_success,
            "buy_tax_bps": buy_bps,
            "sell_tax_bps": sell_bps,
            "attempts": d.tried,
            "ms": (d.total_ms * 100.0).round() / 100.0,
            "decision": decision,
        }),
    );
}

/// Ghi `tx.skip`/`sim.result` vào `logs/bot.jsonl` (CLAUDE.md mục "State /
/// log"). CHƯA wire vào vòng lặp pending-tx thật (không tồn tại ở phiên này)
/// — hàm này chỉ được gọi thủ công/test, chứng minh log hoạt động đúng
/// schema cho khi `5.1` nối pending-tx thật.
pub fn log_outcome(logger: &BotLogger, from: Address, token_hint: Option<Address>, outcome: &PipelineOutcome) {
    match outcome {
        PipelineOutcome::Skip(reason) => {
            logger.log(
                "tx.skip",
                serde_json::json!({
                    "from": format!("{:#x}", from),
                    "token": token_hint.map(|t| format!("{:#x}", t)),
                    "reason": reason.as_str(),
                }),
            );
        }
        PipelineOutcome::Simulated(q) => {
            logger.log(
                "sim.result",
                serde_json::json!({
                    "from": format!("{:#x}", from),
                    "token": token_hint.map(|t| format!("{:#x}", t)),
                    "front_in_wei": q.front_in.to_string(),
                    "back_out_wei": q.back_out.to_string(),
                    "profit_wei": q.profit_wei,
                }),
            );
        }
    }
}

/// Cụm `foundation-fix-then-real-sim` (A4) — thông tin thô kèm log
/// `tx.skip`/`sim.result` (hash/to/venue/selector/fee, CLAUDE.md lệnh A4 mục
/// "log thêm hash,to,venue,selector") — hoàn toàn phục vụ quan sát/debug,
/// KHÔNG ảnh hưởng quyết định pipeline. `fee` chỉ `Some` khi venue là V3
/// (đọc thật từ `decoder::SwapVenue::V3{fee}`, dùng cho log `venue_unpinned`
/// theo đúng lệnh "log field venue,fee").
#[derive(Debug, Clone, Default)]
pub struct TxLogMeta {
    pub hash: String,
    pub to: Option<String>,
    pub venue: Option<String>,
    pub selector: Option<String>,
    pub fee: Option<u32>,
    /// Cụm `exec-path-traps` (F-16) — chi tiết thêm cho `reason=decode_fail`
    /// khi lý do CỤ THỂ là `venue_mismatch` (selector đã decode không khớp
    /// router `tx.to` thật, xem `decoder::venue_matches_router`), hoặc
    /// (cụm `real-economics-mode2`, mục 0) `"vet_fail"` khi pool pair-mode
    /// đã biết nhưng vet nền vừa loại — `None` cho MỌI trường hợp khác
    /// (không bịa lý do).
    pub detail: Option<String>,
    /// Cụm `real-economics-mode2` (mục 2) — số lượng victim đưa vào (wei,
    /// đơn vị QUOTE ASSET của candidate — WBNB hoặc USDT). `None` khi chưa
    /// decode được (`decode_fail`/`not_pancake_router`...).
    pub amount_in: Option<String>,
    /// Quote asset của candidate: `"wbnb"`/`"usdt"` — KHÁC `source` (mang
    /// nghĩa wallet/pair/universal/none ở nhánh WBNB, hoặc wbnb/usdt/none ở
    /// nhánh quote-aware). `None` khi chưa xác định được (skip quá sớm).
    pub quote: Option<String>,
    /// Địa chỉ pool V2 đã resolve (`None` khi chưa resolve được — skip trước
    /// bước đó, vd `decode_fail`/`no_pool`).
    pub pair: Option<String>,
    /// Reserve của quote asset trong pool đã resolve (wei) — `None` khi chưa
    /// resolve được pool.
    pub reserve_quote: Option<String>,
    /// Cụm F-03 — chi phí gas THẬT đã tính (wei, đơn vị BNB — gas luôn trả
    /// bằng BNB) tại thời điểm quyết định. `None` khi chưa tính tới bước đó.
    pub gas_cost_wei: Option<String>,
    /// F-03 — giá gas (gwei) đọc từ `GasOracle` tại block quyết định.
    pub gas_price_gwei: Option<f64>,
    /// Số mili-giây từ lúc `handle_paper_tx` NHẬN tx (proxy cho lúc `tx.seen`
    /// được log — độ trễ giữa 2 mốc này là chi phí `tokio::spawn`, không
    /// đáng kể ở mức ms) tới lúc quyết định CUỐI CÙNG (outcome terminal).
    pub seen_to_decision_ms: Option<f64>,
}

/// Cụm pair-mode — bản `log_outcome` có thêm field `source` ("wallet"/"pair"/
/// "none", xem `decide_paper_v2`) — `log_outcome` gốc GIỮ NGUYÊN (không thêm
/// tham số, tránh phá test/call site cũ), hàm này DÙNG THÊM cho `main.rs` khi
/// gọi `decide_paper_v2`. Cụm A4 — thêm tham số `meta: &TxLogMeta` (hash/to/
/// venue/selector/fee), CHỈ 1 call site (`main.rs::handle_paper_tx`) nên đổi
/// chữ ký trực tiếp, không cần hàm song song mới.
pub fn log_outcome_v2(
    logger: &BotLogger,
    from: Address,
    token_hint: Option<Address>,
    source: &str,
    meta: &TxLogMeta,
    outcome: &PipelineOutcome,
) {
    match outcome {
        PipelineOutcome::Skip(reason) => {
            logger.log(
                "tx.skip",
                serde_json::json!({
                    "from": format!("{:#x}", from),
                    "token": token_hint.map(|t| format!("{:#x}", t)),
                    "reason": reason.as_str(),
                    "source": source,
                    "hash": meta.hash,
                    "to": meta.to,
                    "venue": meta.venue,
                    "selector": meta.selector,
                    "fee": meta.fee,
                    "detail": meta.detail,
                    // Cum `real-economics-mode2` (muc 2)
                    "amount_in": meta.amount_in,
                    "quote": meta.quote,
                    "pair": meta.pair,
                    "reserve_quote": meta.reserve_quote,
                    "gas_cost_wei": meta.gas_cost_wei,
                    "gas_price_gwei": meta.gas_price_gwei,
                    "seen_to_decision_ms": meta.seen_to_decision_ms,
                }),
            );
        }
        PipelineOutcome::Simulated(q) => {
            // Cum `real-economics-mode2` (muc 2) - profit_gross_wei = profit
            // TRUOC khi tru gas (SandwichQuote.profit_wei DA la net, tru san
            // gas_cost_wei ben trong sim_v2::quote_at) - cong nguoc lai gas
            // da biet de co so gross doi chieu, KHONG tinh lai tu dau.
            let gas_cost_i128: i128 = meta
                .gas_cost_wei
                .as_deref()
                .and_then(|s| s.parse::<u128>().ok())
                .map(|g| g as i128)
                .unwrap_or(0);
            let profit_gross_wei = q.profit_wei + gas_cost_i128;
            logger.log(
                "sim.result",
                serde_json::json!({
                    "from": format!("{:#x}", from),
                    "token": token_hint.map(|t| format!("{:#x}", t)),
                    "front_in_wei": q.front_in.to_string(),
                    "back_out_wei": q.back_out.to_string(),
                    "profit_wei": q.profit_wei,
                    "source": source,
                    "hash": meta.hash,
                    "to": meta.to,
                    "venue": meta.venue,
                    "selector": meta.selector,
                    // Cum `real-economics-mode2` (muc 2)
                    "amount_in": meta.amount_in,
                    "quote": meta.quote,
                    "pair": meta.pair,
                    "reserve_quote": meta.reserve_quote,
                    "gas_cost_wei": meta.gas_cost_wei,
                    "gas_price_gwei": meta.gas_price_gwei,
                    "profit_gross_wei": profit_gross_wei,
                    "profit_net_wei": q.profit_wei,
                    "seen_to_decision_ms": meta.seen_to_decision_ms,
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pairbook::{PairBook, ResolvedFrom};
    use crate::tax::TaxMeasurement;
    use alloy::primitives::keccak256;

    fn addr(hex: &str) -> Address {
        Address::from_str(hex).unwrap()
    }

    fn pad_addr(a: Address) -> [u8; 32] {
        let mut w = [0u8; 32];
        w[12..32].copy_from_slice(a.as_slice());
        w
    }

    fn pad_u256(v: u64) -> [u8; 32] {
        U256::from(v).to_be_bytes::<32>()
    }

    fn sel(sig: &str) -> [u8; 4] {
        let h = keccak256(sig.as_bytes());
        [h[0], h[1], h[2], h[3]]
    }

    /// Fixture calldata `swapExactETHForTokens(amountOutMin, path, to,
    /// deadline)` — path=[WBNB, token], amountIn lấy từ `tx.value` (không
    /// nằm trong calldata, đúng quy ước `decoder.rs`).
    fn build_eth_for_tokens(token: Address, amount_out_min: u64) -> Vec<u8> {
        build_eth_for_tokens_with_deadline(token, amount_out_min, 9_999_999_999)
    }

    /// Cụm `exec-path-traps` (F-14) — như `build_eth_for_tokens` nhưng cho
    /// phép chỉnh `deadline` (test gate deadline cần deadline gần/đã qua wall-clock).
    fn build_eth_for_tokens_with_deadline(token: Address, amount_out_min: u64, deadline: u64) -> Vec<u8> {
        let mut out = sel("swapExactETHForTokens(uint256,address[],address,uint256)").to_vec();
        out.extend_from_slice(&pad_u256(amount_out_min));
        out.extend_from_slice(&pad_u256(0x80)); // offset path
        out.extend_from_slice(&pad_addr(addr("0x999999999999999999999999999999999999beef"))); // to
        out.extend_from_slice(&pad_u256(deadline));
        out.extend_from_slice(&pad_u256(2)); // path.length
        out.extend_from_slice(&pad_addr(wbnb()));
        out.extend_from_slice(&pad_addr(token));
        out
    }

    fn victims_ab() -> VictimBook {
        let mut book = VictimBook::new();
        book.load_from_str(
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,0.01\n0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,0.5\n",
        );
        book
    }

    fn fixture_reserves() -> PoolReserves {
        // Pool NHỎ có chủ đích (1 WBNB / 1_000_000 token) để victim 0.05 BNB
        // (5% pool) tạo đủ price impact cho sandwich có lãi rõ ràng vượt gas
        // config ship (~0.006 BNB) — pool sâu hơn (vd 1000 WBNB) sẽ khiến
        // price impact quá nhỏ, profit < gas, không minh hoạ được "ra sim"
        // theo đúng lệnh.
        PoolReserves {
            reserve_wbnb: U256::from(1_000_000_000_000_000_000u128), // 1 WBNB
            reserve_token: U256::from(1_000_000u64) * U256::from(1_000_000_000_000_000_000u128),
        }
    }

    const MAX_FRONT_WEI: u64 = 1_500_000_000_000_000_000; // 1.5 BNB, khop config.toml ship

    /// Config test khớp `config.toml` ship, TRỪ `min_reserve_wbnb` hạ về `0`
    /// — các fixture pool ở file này cố ý rất nông (1 WBNB, xem
    /// `fixture_reserves()`) để minh hoạ price impact rõ ràng, khác pool thật
    /// (`min_reserve_wbnb=20` ship sẽ chặn `thin_liq` mọi fixture ở đây; test
    /// `thin_liq_skip_when_pool_reserve_below_min_reserve_config` dùng ship
    /// value thật 20 để chứng minh wiring, tách riêng khỏi các test khác).
    fn test_config_toml(overrides: &[(&str, &str)]) -> String {
        let mut s = String::from(
            "chain_id = 56\ndry_run = true\nallow_live = false\nbot_armed = false\n\
             scan_v2 = true\nscan_v3 = true\nscan_v4 = true\n\
             live_v2 = false\nlive_v3 = false\nlive_v4 = false\n\
             min_profit_bnb = 0.01\nmax_front_bnb = 1.5\nmin_reserve_wbnb = 0\n\
             victims_path = \"victims.txt\"\nvictims_reload_sec = 15\nconfig_reload_sec = 15\n\
             pending_poll_ms = 400\npending_txpool_max_per_poll = 32\n\
             gas_reserve_bnb_wei = 5000000000000000\n\
             front_max_gas_bnb_wei = 3000000000000000\nback_max_gas_bnb_wei = 3000000000000000\n\
             tx_timeout_sec = 30\nws_silence_sec = 60\nmax_consecutive_loss = 3\nmax_exposure_bnb = 5.0\n\
             web_bind = \"127.0.0.1\"\nweb_port = 8787\n\
             max_roundtrip_tax = 0.005\ntax_cache_blocks = 30\n\
             allow_tax_inject = true\n\
             executor_deadline_buffer_sec = 120\n\
             pairs_path = \"pairs.txt\"\npairs_reload_sec = 30\npairs_min_swap_bnb = 0.05\n\
             pair_scan_universal = false\n\
             wallet_scan_enabled = true\npair_scan_enabled = true\n\
             scan_quote_usdt = false\nmin_profit_usdt = 3.0\n\
             max_front_usdt = 3000.0\nmin_reserve_usdt = 15000.0\n\
             sim_engine = \"evm\"\ntax_cache_ttl_sec = 600\n\
             front_slippage_bps = 10\nback_slippage_bps = 50\n\
             pairs_vet_interval_sec = 600\npairs_require_vetted = false\n\
             gas_units_front = 160000\ngas_units_back = 140000\ngas_price_max_gwei = 10\n",
        );
        for (needle, replacement) in overrides {
            s = s.replace(needle, replacement);
        }
        s
    }

    fn test_config() -> crate::config::Config {
        crate::config::Config::from_str(&test_config_toml(&[])).expect("test config phai load duoc")
    }

    /// Case B: ví có trong list nhưng amount (0.05 BNB) < min (0.5 BNB) ->
    /// `below_min`.
    #[test]
    fn victim_b_below_min_is_skipped_with_clear_reason() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"); // min 0.5
        let tx_value = U256::from(50_000_000_000_000_000u64); // 0.05 BNB < 0.5 min
        let victims = victims_ab();
        let cache = TaxCache::new();
        let input = PaperDecision {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(),
            current_block: 1000,
        };
        let outcome = decide_paper(&victims, &cache, &test_config(), &input);
        match outcome {
            PipelineOutcome::Skip(PipelineSkip::BelowMin) => {}
            other => panic!("expect below_min, got {other:?}"),
        }
    }

    /// Case A: ví min 0.01, amount 0.05 BNB đủ điều kiện min-size, NHƯNG
    /// token chưa đo tax -> `honeypot_or_tax` (reason rõ ràng).
    #[test]
    fn victim_a_passes_min_size_but_skips_honeypot_or_tax_when_unmeasured() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); // min 0.01
        let tx_value = U256::from(50_000_000_000_000_000u64); // 0.05 BNB >= 0.01
        let victims = victims_ab();
        let cache = TaxCache::new(); // chua do token nao
        let input = PaperDecision {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(),
            current_block: 1000,
        };
        let outcome = decide_paper(&victims, &cache, &test_config(), &input);
        match outcome {
            PipelineOutcome::Skip(PipelineSkip::HoneypotOrTax) => {}
            other => panic!("expect honeypot_or_tax, got {other:?}"),
        }
    }

    /// Cùng case A, nhưng test inject cache tax=0 (thủ công, đúng lệnh "1
    /// test inject cache tax=0 để A ra sim") -> phải đi tới `Simulated`.
    #[test]
    fn victim_a_reaches_sim_when_tax_cache_injected_zero() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995)); // trong tax_cache_blocks=30
        let input = PaperDecision {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(),
            current_block: 1000,
        };
        let outcome = decide_paper(&victims, &cache, &test_config(), &input);
        match outcome {
            PipelineOutcome::Simulated(q) => {
                println!(
                    "pipeline sim OK: front_in={} back_out={} profit_wei={}",
                    q.front_in, q.back_out, q.profit_wei
                );
                assert!(q.front_in <= U256::from(MAX_FRONT_WEI));
                assert!(q.profit_wei > 0);
            }
            other => panic!("expect Simulated, got {other:?}"),
        }
    }

    /// Cụm tax-cache-inject — ĐẠT CẦN DÁN: "1 inject tax 0,0 → reason khác
    /// honeypot_or_tax". Dùng đúng điểm ghi `TaxCache::inject_from_buy_sell_bps`
    /// (không `insert` tay như test `victim_a_reaches_sim_when_tax_cache_injected_zero`
    /// ở trên) để chứng minh đường đi THẬT `POST /api/tax`/`tax_inject.jsonl`
    /// sẽ dùng — cùng fixture Case A nên kết quả phải là `Simulated`, KHÔNG
    /// phải `honeypot_or_tax`.
    #[test]
    fn victim_a_reaches_sim_after_inject_from_buy_sell_bps_zero_zero() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let mut cache = TaxCache::new();
        cache.inject_from_buy_sell_bps(token, 0, 0, 995); // "0,0 = zero-tax da do tay"
        let input = PaperDecision {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(),
            current_block: 1000,
        };
        let outcome = decide_paper(&victims, &cache, &test_config(), &input);
        match outcome {
            PipelineOutcome::Simulated(_) => {}
            PipelineOutcome::Skip(PipelineSkip::HoneypotOrTax) => {
                panic!("inject tax 0,0 khong duoc dung lai o honeypot_or_tax")
            }
            other => panic!("expect Simulated (hoac unprofitable, khong phai honeypot_or_tax), got {other:?}"),
        }
    }

    /// Cụm `5.3`, ĐẠT CẦN DÁN: "test precheck không gọi RPC". Bảo đảm ở MỨC
    /// KIỂU (compile-time), không chỉ hành vi runtime — `precheck_without_reserves`
    /// KHÔNG nhận tham số `Provider` nào (khác `resolve_v2_reserves` bên
    /// dưới nó nhận `provider: &dyn Provider`), nên hàm này KHÔNG THỂ tự nó
    /// gây ra `eth_call` dù calldata thế nào. Test xác nhận 3 nhánh skip sớm
    /// (`decode_fail`/`not_in_list`/`below_min`) đều dừng ĐÚNG ở bước thuần
    /// này — `main.rs::handle_paper_tx` chỉ gọi `resolve_v2_reserves` (nơi
    /// DUY NHẤT có `eth_call`) SAU khi hàm này trả `Ok(token)`.
    #[test]
    fn precheck_without_reserves_never_touches_rpc_for_every_early_skip_reason() {
        let victims = victims_ab();
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let amount_50_bnb = U256::from(50_000_000_000_000_000u64);

        let garbage_calldata = vec![0xde, 0xad, 0xbe, 0xef];
        assert_eq!(
            precheck_without_reserves(&victims, addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"), &garbage_calldata, amount_50_bnb),
            Err(PipelineSkip::DecodeFail)
        );
        assert_eq!(
            precheck_without_reserves(&victims, addr("0xdddddddddddddddddddddddddddddddddddddddd"), &calldata, amount_50_bnb),
            Err(PipelineSkip::NotInList)
        );
        assert_eq!(
            precheck_without_reserves(&victims, addr("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"), &calldata, amount_50_bnb),
            Err(PipelineSkip::BelowMin)
        );
    }

    fn build_exact_input_single(token_in: Address, token_out: Address, fee: u64, amount_in: u64) -> Vec<u8> {
        let mut out = sel("exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))").to_vec();
        out.extend_from_slice(&pad_addr(token_in));
        out.extend_from_slice(&pad_addr(token_out));
        out.extend_from_slice(&pad_u256(fee));
        out.extend_from_slice(&pad_addr(addr("0x666666666666666666666666666666666666e0e0"))); // recipient
        out.extend_from_slice(&pad_u256(9_999_999_999)); // deadline
        out.extend_from_slice(&pad_u256(amount_in));
        out.extend_from_slice(&pad_u256(0)); // amountOutMinimum
        out.extend_from_slice(&pad_u256(0)); // sqrtPriceLimitX96
        out
    }

    /// ĐẠT CẦN DÁN (lệnh A4, test bắt buộc) — `exactInputSingle` (hàm SwapRouter
    /// V3 đã pin, cũng là chữ ký SmartRouter dùng lại) phải bị
    /// `precheck_token_and_venue` phân loại `SwapVenue::V3{fee}` — CHỨNG MINH
    /// `main.rs::handle_paper_tx` rẽ vào nhánh `VenueUnpinned` (gate c) thay vì
    /// gọi `resolve_v2_reserves` (bug cũ đã sửa: V3 từng bị sim nhầm bằng pool
    /// V2). Test ở MỨC KIỂU: `evaluate_candidate`/`resolve_v2_reserves` không
    /// hề xuất hiện trong test này, đúng nghĩa "không gọi" — hành vi thật của
    /// `main.rs` được đảm bảo bởi cấu trúc `match` (xem `handle_paper_tx`,
    /// nhánh `SwapVenue::V3` return sớm trước khi đọc `app_state.provider`).
    #[test]
    fn precheck_token_and_venue_exact_input_single_is_v3_with_fee() {
        let token_out = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_exact_input_single(wbnb(), token_out, 500, 777);
        let (token, venue, selector_name) = precheck_token_and_venue(&calldata, U256::ZERO).expect("phai decode duoc");
        assert_eq!(token, token_out);
        assert_eq!(venue, decoder::SwapVenue::V3 { fee: 500 });
        assert_eq!(selector_name, "exactInputSingle");
    }

    #[test]
    fn precheck_token_and_venue_v2_functions_are_v2() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let (t, venue, selector_name) = precheck_token_and_venue(&calldata, U256::from(1u64)).expect("phai decode duoc");
        assert_eq!(t, token);
        assert_eq!(venue, decoder::SwapVenue::V2);
        assert_eq!(selector_name, "swapExactETHForTokens");
    }

    #[test]
    fn address_not_in_victims_txt_is_not_in_list() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xdddddddddddddddddddddddddddddddddddddddd"); // khong co trong victims_ab()
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let cache = TaxCache::new();
        let input = PaperDecision {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(),
            current_block: 1000,
        };
        let outcome = decide_paper(&victims, &cache, &test_config(), &input);
        match outcome {
            PipelineOutcome::Skip(PipelineSkip::NotInList) => {}
            other => panic!("expect not_in_list, got {other:?}"),
        }
    }

    #[test]
    fn log_outcome_writes_expected_events() {
        let dir = tempfile::tempdir().unwrap();
        let logger = BotLogger::new(dir.path().join("logs").join("bot.jsonl")).unwrap();
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        log_outcome(&logger, from, Some(token), &PipelineOutcome::Skip(PipelineSkip::HoneypotOrTax));
        let tail = logger.tail(10);
        assert_eq!(tail.len(), 1);
        assert_eq!(tail[0]["event"], "tx.skip");
        assert_eq!(tail[0]["reason"], "honeypot_or_tax");
    }

    /// `min_reserve_wbnb` (Config) đọc thẳng vào `decide_paper` — pool
    /// `fixture_reserves()` chỉ có 1 WBNB, dưới ngưỡng ship thật `20` ->
    /// `thin_liq`, KHÔNG hardcode trong `pipeline.rs`/`sim_v2.rs`.
    #[test]
    fn thin_liq_skip_when_pool_reserve_below_min_reserve_config() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); // min 0.01, du pass below_min
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let cache = TaxCache::new();
        let cfg = crate::config::Config::from_str(&test_config_toml(&[("min_reserve_wbnb = 0", "min_reserve_wbnb = 20")]))
            .expect("cfg phai load duoc");
        let input = PaperDecision {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(), // chi 1 WBNB, duoi 20
            current_block: 1000,
        };
        let outcome = decide_paper(&victims, &cache, &cfg, &input);
        match outcome {
            PipelineOutcome::Skip(PipelineSkip::ThinLiq) => {}
            other => panic!("expect thin_liq, got {other:?}"),
        }
    }

    /// Lãi mô phỏng DƯƠNG (~0.033 BNB, xem `victim_a_reaches_sim_when_tax_cache_injected_zero`)
    /// nhưng dưới `min_profit_bnb=0.05` (Config chỉnh tay) -> `unprofitable`,
    /// không phải `Simulated` — chứng minh `min_profit_bnb` không phải chỉ
    /// "positive check" cứng mà đọc thật từ Config.
    #[test]
    fn unprofitable_skip_when_profit_positive_but_below_min_profit_bnb_config() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let cfg = crate::config::Config::from_str(&test_config_toml(&[("min_profit_bnb = 0.01", "min_profit_bnb = 0.05")]))
            .expect("cfg phai load duoc");
        let input = PaperDecision {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(),
            current_block: 1000,
        };
        let outcome = decide_paper(&victims, &cache, &cfg, &input);
        match outcome {
            PipelineOutcome::Skip(PipelineSkip::Unprofitable) => {}
            other => panic!("expect unprofitable (loi duong nhung duoi min_profit_bnb=0.05), got {other:?}"),
        }
    }

    /// Cache "đã đo" (fresh) nhưng `roundtrip_tax_bps` (100 = 1%) vượt
    /// `max_roundtrip_tax` ship (0.005 = 50 bps) -> vẫn `honeypot_or_tax`,
    /// không được sim tiếp dù cache không rỗng/không hết hạn.
    #[test]
    fn honeypot_or_tax_skip_when_measured_tax_exceeds_max_roundtrip_tax_config() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(100, 995)); // 1% > 0.5% max
        let cfg = test_config(); // max_roundtrip_tax=0.005 (50 bps) ship
        let input = PaperDecision {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(),
            current_block: 1000,
        };
        let outcome = decide_paper(&victims, &cache, &cfg, &input);
        match outcome {
            PipelineOutcome::Skip(PipelineSkip::HoneypotOrTax) => {}
            other => panic!("expect honeypot_or_tax (tax do duoc nhung vuot max_roundtrip_tax), got {other:?}"),
        }
    }

    /// ĐẠT lát config-hot-reload: đổi `max_front_bnb` GIỮA 2 lần gọi
    /// `decide_paper` (mô phỏng hot-reload — `Config` mới thay `Config` cũ,
    /// đúng những gì `Config::reload_if_due` làm) -> trần `front_in` đổi
    /// theo số MỚI ngay lần gọi sau, không cần build lại/restart.
    #[test]
    fn changing_config_between_two_decide_paper_calls_changes_front_cap() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let input = PaperDecision {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(),
            current_block: 1000,
        };

        let cfg_before = test_config(); // max_front_bnb=1.5
        let outcome_before = decide_paper(&victims, &cache, &cfg_before, &input);
        let front_before = match outcome_before {
            PipelineOutcome::Simulated(q) => q.front_in,
            other => panic!("expect Simulated truoc reload, got {other:?}"),
        };
        // Ternary search hoi tu GAN tran 1.5 BNB (khong luon dung tuyet doi
        // bang tran, xem sim_v2::search_max_front_in) — chi can du lon de lam
        // baseline so sanh voi tran moi rat thap (0.001 BNB) o duoi.
        assert!(front_before > cfg_before.max_front_wei() * U256::from(99u64) / U256::from(100u64));

        // "Reload" = Config moi thay Config cu (dung y nghia hot-reload, xem
        // Config::reload_if_due) — tran front giam manh con 0.001 BNB.
        let cfg_after =
            crate::config::Config::from_str(&test_config_toml(&[("max_front_bnb = 1.5", "max_front_bnb = 0.001")]))
                .expect("cfg sau reload phai load duoc");
        let outcome_after = decide_paper(&victims, &cache, &cfg_after, &input);
        let front_after = match outcome_after {
            PipelineOutcome::Simulated(q) => q.front_in,
            PipelineOutcome::Skip(PipelineSkip::Unprofitable) => {
                // Tran qua thap co the khien loi nhuan < min_profit_bnb —
                // van chap nhan duoc, miem la KHONG con dung tran CU (1.5).
                cfg_after.max_front_wei()
            }
            other => panic!("khong mong doi {other:?}"),
        };
        assert!(front_after <= cfg_after.max_front_wei(), "front sau reload phai theo tran MOI (0.001 BNB)");
        assert!(front_after < front_before, "tran moi thap hon phai lam front_in giam theo, khong con dung so cu");
    }

    /// Pool SÂU (1000 WBNB) + victim RẤT LỚN (500 WBNB, 50% reserve) — dùng
    /// riêng cho 2 test exposure-cap dưới đây. Tối ưu lý thuyết (bỏ qua gas)
    /// của công thức sandwich 2 bước trên cặp số này nằm quanh
    /// `sqrt(reserve_in*(reserve_in+victim_in*0.9975)) - reserve_in` ≈ 224
    /// BNB — RẤT xa cả 2 trần đang test (5 và 10 BNB), nên `profit(front_in)`
    /// vẫn đang tăng dần trong toàn bộ khoảng `[0, 10 BNB]` -> ternary search
    /// hội tụ CHÍNH XÁC tại biên trần (giống thiết kế
    /// `sim_v2::tests::search_never_exceeds_max_front_bnb`), khác
    /// `fixture_reserves()` (pool 1 WBNB, quá nông, tối ưu thật nằm dưới cả
    /// 1.5 BNB nên không phù hợp để test 2 trần 5/10 BNB).
    fn deep_pool_reserves() -> PoolReserves {
        PoolReserves {
            reserve_wbnb: U256::from(1_000u64) * U256::from(1_000_000_000_000_000_000u128), // 1000 WBNB
            reserve_token: U256::from(1_000_000_000u64) * U256::from(1_000_000_000_000_000_000u128), // 1e9 token
        }
    }

    const FIVE_BNB_WEI: u128 = 5_000_000_000_000_000_000;
    const TEN_BNB_WEI: u128 = 10_000_000_000_000_000_000;
    const VICTIM_500_BNB_WEI: u128 = 500_000_000_000_000_000_000;

    /// Cụm `5.2`, ĐẠT CẦN DÁN mục 1: "max_front=10, max_exposure=5 -> front_in
    /// <= 5 BNB wei". `test_config_toml()` ship sẵn `max_exposure_bnb = 5.0`
    /// — chỉ nới `max_front_bnb` lên 10 (rất rộng) để chứng minh
    /// `max_exposure_bnb` mới là cái CHẶN THỰC SỰ qua `decide_paper` đầy đủ
    /// (không chỉ gọi thẳng `sim_v2::search_max_front_in` như test đơn vị ở
    /// `config.rs`).
    #[test]
    fn max_exposure_bnb_caps_front_in_tighter_than_max_front_bnb() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); // min 0.01, du pass below_min voi 500 BNB
        let tx_value = U256::from(VICTIM_500_BNB_WEI);
        let victims = victims_ab();
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let cfg = crate::config::Config::from_str(&test_config_toml(&[("max_front_bnb = 1.5", "max_front_bnb = 10")]))
            .expect("cfg phai load duoc");
        assert_eq!(cfg.max_front_wei(), U256::from(TEN_BNB_WEI));
        assert_eq!(cfg.max_exposure_wei(), Some(U256::from(FIVE_BNB_WEI)));
        let input = PaperDecision {
            from,
            calldata: &calldata,
            tx_value,
            reserves: deep_pool_reserves(),
            current_block: 1000,
        };
        let outcome = decide_paper(&victims, &cache, &cfg, &input);
        match outcome {
            PipelineOutcome::Simulated(q) => {
                let five_bnb_wei = U256::from(FIVE_BNB_WEI);
                println!(
                    "exposure cap test: front_in={} (tran max_exposure_bnb=5 BNB={}), max_front_bnb=10",
                    q.front_in, five_bnb_wei
                );
                assert!(
                    q.front_in <= five_bnb_wei,
                    "front_in phai bi max_exposure_bnb=5 chan, khong duoc dung toi max_front_bnb=10"
                );
                // Pool sau (xem doc-comment deep_pool_reserves) -> loi nhuan van
                // dang tang tai bien 5 BNB, phai hoi tu DUNG tai bien (giong
                // sim_v2::tests::search_never_exceeds_max_front_bnb), khong phai
                // trung hop roi vao gan bien.
                assert_eq!(q.front_in, five_bnb_wei, "loi nhuan con tang toi bien -> phai hoi tu dung tai max_exposure_bnb");
            }
            other => panic!("expect Simulated, got {other:?}"),
        }
    }

    /// Đối chứng: `max_exposure_bnb = 0` phải TẮT cap này — cùng
    /// `max_front_bnb = 10`, front_in được vượt quá 5 BNB (hội tụ đúng tại
    /// 10 BNB), chứng minh `0` không phải "trần 0 BNB" mà là "không giới hạn
    /// thêm" (chỉ còn `max_front_bnb` chặn).
    #[test]
    fn max_exposure_bnb_zero_disables_cap_front_can_exceed_5_bnb() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let tx_value = U256::from(VICTIM_500_BNB_WEI);
        let victims = victims_ab();
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let cfg = crate::config::Config::from_str(&test_config_toml(&[
            ("max_front_bnb = 1.5", "max_front_bnb = 10"),
            ("max_exposure_bnb = 5.0", "max_exposure_bnb = 0"),
        ]))
        .expect("cfg phai load duoc (max_exposure_bnb=0 hop le)");
        assert_eq!(cfg.max_exposure_wei(), None);
        let input = PaperDecision {
            from,
            calldata: &calldata,
            tx_value,
            reserves: deep_pool_reserves(),
            current_block: 1000,
        };
        let outcome = decide_paper(&victims, &cache, &cfg, &input);
        match outcome {
            PipelineOutcome::Simulated(q) => {
                let five_bnb_wei = U256::from(FIVE_BNB_WEI);
                let ten_bnb_wei = U256::from(TEN_BNB_WEI);
                println!("exposure off test: front_in={} (phai duoc vuot 5 BNB={}, hoi tu tai 10 BNB={})", q.front_in, five_bnb_wei, ten_bnb_wei);
                assert!(q.front_in > five_bnb_wei, "max_exposure_bnb=0 phai TAT cap, front_in duoc vuot 5 BNB");
                assert_eq!(q.front_in, ten_bnb_wei, "khong con exposure cap -> phai hoi tu dung tai max_front_bnb=10");
            }
            other => panic!("expect Simulated, got {other:?}"),
        }
    }

    // ===== Cụm pair-mode / 7.1 — decide_paper_v2 =====

    /// ĐẠT CẦN DÁN: `decide_paper_wallet_source` — `tx.from` khớp
    /// `victims.txt` -> `source="wallet"`, kết quả giống hệt `decide_paper`
    /// gốc (không đổi hành vi wallet-mode).
    #[test]
    fn decide_paper_wallet_source() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); // trong victims_ab, min 0.01
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let pairbook = PairBook::new(); // rong - khong lien quan wallet mode
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let risk = RiskGuard::new();
        let cfg = test_config();
        let pair_addr = addr("0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead"); // pool bat ky, khong trong PairBook
        let input = PaperDecisionV2 {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(),
            pair_addr,
            current_block: 1000,
            gas_cost_wei: cfg.gas_wei(),
        };
        let (outcome, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "wallet");
        match outcome {
            PipelineOutcome::Simulated(q) => assert!(q.profit_wei > 0),
            other => panic!("expect Simulated (wallet mode), got {other:?}"),
        }
    }

    /// ĐẠT CẦN DÁN: `decide_paper_pair_source` — `tx.from` KHÔNG trong
    /// `victims.txt`, nhưng `pair_addr` khớp `PairBook` -> `source="pair"`,
    /// dùng ngưỡng `pairs_min_swap_bnb` GLOBAL (0.05 BNB, khớp
    /// `test_config_toml`) thay vì min riêng từng ví.
    #[test]
    fn decide_paper_pair_source() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0x9999999999999999999999999999999999999999"); // KHONG trong victims_ab
        let tx_value = U256::from(60_000_000_000_000_000u64); // 0.06 BNB >= pairs_min_swap_bnb 0.05
        let victims = victims_ab();
        let mut pairbook = PairBook::new();
        let pair_addr = addr("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        pairbook.insert_test_entry(pair_addr, "0xcccc...,WBNB", ResolvedFrom::Token);
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let risk = RiskGuard::new();
        let cfg = test_config();
        let input = PaperDecisionV2 {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(),
            pair_addr,
            current_block: 1000,
            gas_cost_wei: cfg.gas_wei(),
        };
        let (outcome, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "pair");
        match outcome {
            PipelineOutcome::Simulated(q) => assert!(q.profit_wei > 0),
            other => panic!("expect Simulated (pair mode), got {other:?}"),
        }
    }

    // ===== Cụm `real-economics-mode2` (mục 0) — fix BUG BAOCAO37: pool
    // pair-mode ĐÃ VET (qua `PairBook::reload` thật, không phải
    // `insert_test_entry`) không còn bị chặn bởi `TaxCache` rỗng =====

    /// Resolver giả trả `Address::ZERO` cho MỌI token -> dòng `pairs.txt`
    /// dạng "0xAddress" trần TỰ NÓ trở thành `pair_addr` (`ResolvedFrom::Direct`,
    /// xem `pairbook.rs::reload`) — đủ để dựng 1 `PairBook` đã vet THẬT (qua
    /// `reload()`, khác `insert_test_entry` luôn `vetted_at=None`) mà không
    /// cần factory/RPC thật.
    #[derive(Clone, Default)]
    struct DirectPairResolver;
    impl crate::pairbook::PairResolver for DirectPairResolver {
        async fn get_pair(&self, _token: Address, _quote: Address) -> Result<Address, String> {
            Ok(Address::ZERO)
        }
    }

    fn pairbook_test_logger() -> (tempfile::TempDir, BotLogger) {
        let dir = tempfile::tempdir().unwrap();
        let logger = BotLogger::new(dir.path().join("logs").join("bot.jsonl")).unwrap();
        (dir, logger)
    }

    /// ĐẠT CẦN DÁN (lệnh `real-economics-mode2`, mục 0): pool ĐÃ VET THẬT
    /// (`vetted_at=Some`, qua `reload()`) + `TaxCache` RỖNG HOÀN TOÀN vẫn ra
    /// `Simulated` — TRƯỚC fix này, mọi candidate pair-mode rơi vào
    /// `honeypot_or_tax` vì đường nóng tra nhầm `TaxCache` (luôn rỗng vì
    /// không ai tự động điền) thay vì tin `pairs.txt` đã vet tay (BAOCAO37:
    /// `honeypot_or_tax=95/phút`, `unprofitable=0`).
    #[tokio::test]
    async fn decide_paper_v2_pair_mode_vetted_pool_reaches_sim_with_empty_tax_cache() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0x9999999999999999999999999999999999999999"); // khong trong victims_ab
        let tx_value = U256::from(60_000_000_000_000_000u64); // 0.06 BNB >= pairs_min_swap_bnb 0.05
        let victims = victims_ab();
        let pair_addr = addr("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        let (_dir, logger) = pairbook_test_logger();
        let mut pairbook = PairBook::new();
        let content = format!("{pair_addr:#x} # SYM | vetted 2026-09-16 | tax 0/0 | owner renounced | note\n");
        pairbook.reload(&content, &DirectPairResolver, &logger, std::time::Instant::now(), true).await;
        assert!(pairbook.is_tax_ok(&pair_addr), "setup: pool phai da vet va chua vet_fail");

        let cache = TaxCache::new(); // RONG HOAN TOAN - trong tam bug fix
        let risk = RiskGuard::new();
        let cfg = test_config();
        let input = PaperDecisionV2 {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(),
            pair_addr,
            current_block: 1000,
            gas_cost_wei: cfg.gas_wei(),
        };
        let (outcome, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "pair");
        match outcome {
            PipelineOutcome::Simulated(q) => assert!(q.profit_wei > 0),
            other => panic!("expect Simulated (pool da vet, TaxCache rong khong duoc chan) - day la bug BAOCAO37, got {other:?}"),
        }
    }

    /// ĐẠT CẦN DÁN (lệnh `real-economics-mode2`, mục 0): pool đã vet nhưng
    /// vet NỀN (`pairs_vet_task`) vừa loại (`set_vet_result(ok=false)`) ->
    /// `honeypot_or_tax`, `source="pair"` (KHÔNG rơi xuống `not_in_list` —
    /// giữ visibility đúng lý do kinh tế thật, xem `PairBook::knows_pool`/
    /// `is_vet_failed`).
    #[tokio::test]
    async fn decide_paper_v2_pair_mode_vet_failed_pool_is_honeypot_or_tax() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0x9999999999999999999999999999999999999999");
        let tx_value = U256::from(60_000_000_000_000_000u64);
        let victims = victims_ab();
        let pair_addr = addr("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        let (_dir, logger) = pairbook_test_logger();
        let mut pairbook = PairBook::new();
        let content = format!("{pair_addr:#x} # SYM | vetted 2026-09-16 | tax 0/0 | owner renounced | note\n");
        pairbook.reload(&content, &DirectPairResolver, &logger, std::time::Instant::now(), true).await;
        pairbook.set_vet_result(
            pair_addr,
            crate::pairbook::VetResult { buy_bps: 0, sell_bps: 2000, honeypot: false, block: 1 },
            false,
        );
        assert!(!pairbook.is_tax_ok(&pair_addr));
        assert!(pairbook.is_vet_failed(&pair_addr));

        let cache = TaxCache::new();
        let risk = RiskGuard::new();
        let cfg = test_config();
        let input = PaperDecisionV2 {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(),
            pair_addr,
            current_block: 1000,
            gas_cost_wei: cfg.gas_wei(),
        };
        let (outcome, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "pair", "pool DA BIET (vet_failed) phai van route vao nhanh pair, khong roi xuong not_in_list");
        assert!(matches!(outcome, PipelineOutcome::Skip(PipelineSkip::HoneypotOrTax)), "got {outcome:?}");
    }

    /// Wallet khớp `victims.txt` PHẢI THẮNG, không check `PairBook` dù
    /// `pair_addr` cũng có trong đó — đúng lệnh "Hai mode KHÔNG xung đột".
    #[test]
    fn decide_paper_v2_wallet_wins_over_pair_when_both_match() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); // trong victims_ab
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let mut pairbook = PairBook::new();
        let pair_addr = addr("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        pairbook.insert_test_entry(pair_addr, "line", ResolvedFrom::Token); // pair_addr CUNG khop
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let risk = RiskGuard::new();
        let cfg = test_config();
        let input = PaperDecisionV2 { from, calldata: &calldata, tx_value, reserves: fixture_reserves(), pair_addr, current_block: 1000, gas_cost_wei: cfg.gas_wei() };
        let (_, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "wallet", "wallet mode phai thang du pair_addr cung khop PairBook");
    }

    /// Không khớp cả wallet lẫn pair -> `not_in_list`, `source="none"`.
    #[test]
    fn decide_paper_v2_neither_match_is_not_in_list() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0x9999999999999999999999999999999999999999");
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let pairbook = PairBook::new(); // rong
        let cache = TaxCache::new();
        let risk = RiskGuard::new();
        let cfg = test_config();
        let pair_addr = addr("0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead");
        let input = PaperDecisionV2 { from, calldata: &calldata, tx_value, reserves: fixture_reserves(), pair_addr, current_block: 1000, gas_cost_wei: cfg.gas_wei() };
        let (outcome, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "none");
        assert!(matches!(outcome, PipelineOutcome::Skip(PipelineSkip::NotInList)));
    }

    // ===== Cụm `universal-pair-scan` — decide_paper_v2 nhánh universal =====

    /// ĐẠT CẦN DÁN: `pair_scan_universal=false` (ship mặc định) phải giữ
    /// NGUYÊN hành vi cũ y hệt `decide_paper_v2_neither_match_is_not_in_list`
    /// — không khớp wallet/pair -> `not_in_list`, `source="none"`, KHÔNG trở
    /// thành `universal` dù pool hoàn toàn hợp lệ (đủ ngưỡng
    /// `pairs_min_swap_bnb`).
    #[test]
    fn universal_scan_disabled_by_default_still_not_in_list() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0x9999999999999999999999999999999999999999"); // khong trong victims_ab
        let tx_value = U256::from(60_000_000_000_000_000u64); // 0.06 BNB >= pairs_min_swap_bnb 0.05
        let victims = victims_ab();
        let pairbook = PairBook::new(); // rong - pair_addr khong khop
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let risk = RiskGuard::new();
        let cfg = test_config(); // pair_scan_universal = false (ship mac dinh)
        assert!(!cfg.pair_scan_universal, "test_config() phai ship pair_scan_universal=false");
        let pair_addr = addr("0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead");
        let input = PaperDecisionV2 { from, calldata: &calldata, tx_value, reserves: fixture_reserves(), pair_addr, current_block: 1000, gas_cost_wei: cfg.gas_wei() };
        let (outcome, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "none");
        assert!(matches!(outcome, PipelineOutcome::Skip(PipelineSkip::NotInList)));
    }

    /// ĐẠT CẦN DÁN: `pair_scan_universal=true` + pool LẠ (không trong
    /// `PairBook`, `from` không trong `victims.txt`) + đủ ngưỡng
    /// `pairs_min_swap_bnb` GLOBAL -> candidate `source="universal"`,
    /// `Simulated`.
    #[test]
    fn universal_scan_enabled_unlisted_pool_becomes_candidate() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0x9999999999999999999999999999999999999999"); // khong trong victims_ab
        let tx_value = U256::from(60_000_000_000_000_000u64); // 0.06 BNB >= pairs_min_swap_bnb 0.05
        let victims = victims_ab();
        let pairbook = PairBook::new(); // rong - pair_addr khong khop, chi con nhanh universal
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let risk = RiskGuard::new();
        let cfg = crate::config::Config::from_str(&test_config_toml(&[("pair_scan_universal = false", "pair_scan_universal = true")]))
            .expect("cfg universal=true phai load duoc");
        let pair_addr = addr("0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead"); // pool la, khong trong pairs.txt
        let input = PaperDecisionV2 { from, calldata: &calldata, tx_value, reserves: fixture_reserves(), pair_addr, current_block: 1000, gas_cost_wei: cfg.gas_wei() };
        let (outcome, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "universal");
        match outcome {
            PipelineOutcome::Simulated(q) => assert!(q.profit_wei > 0),
            other => panic!("expect Simulated (universal mode), got {other:?}"),
        }
    }

    /// ĐẠT CẦN DÁN: `pair_scan_universal=true` nhưng amount DƯỚI ngưỡng
    /// `pairs_min_swap_bnb` GLOBAL -> `source="universal"`, `Skip(below_min)`
    /// (KHÔNG có ngưỡng riêng cho universal, dùng đúng ngưỡng pair-mode).
    #[test]
    fn universal_scan_enabled_below_threshold_is_below_min() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0x9999999999999999999999999999999999999999");
        let tx_value = U256::from(10_000_000_000_000_000u64); // 0.01 BNB < pairs_min_swap_bnb 0.05
        let victims = victims_ab();
        let pairbook = PairBook::new();
        let cache = TaxCache::new();
        let risk = RiskGuard::new();
        let cfg = crate::config::Config::from_str(&test_config_toml(&[("pair_scan_universal = false", "pair_scan_universal = true")]))
            .expect("cfg universal=true phai load duoc");
        let pair_addr = addr("0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead");
        let input = PaperDecisionV2 { from, calldata: &calldata, tx_value, reserves: fixture_reserves(), pair_addr, current_block: 1000, gas_cost_wei: cfg.gas_wei() };
        let (outcome, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "universal");
        assert!(matches!(outcome, PipelineOutcome::Skip(PipelineSkip::BelowMin)));
    }

    /// Wallet mode PHẢI THẮNG universal khi cả 2 khớp (giữ đúng thứ tự ưu
    /// tiên wallet > pair > universal > not_in_list).
    #[test]
    fn universal_scan_enabled_wallet_still_wins() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); // trong victims_ab, min 0.01
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let pairbook = PairBook::new(); // rong - pair_addr khong khop, chi wallet vs universal
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let risk = RiskGuard::new();
        let cfg = crate::config::Config::from_str(&test_config_toml(&[("pair_scan_universal = false", "pair_scan_universal = true")]))
            .expect("cfg universal=true phai load duoc");
        let pair_addr = addr("0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead");
        let input = PaperDecisionV2 { from, calldata: &calldata, tx_value, reserves: fixture_reserves(), pair_addr, current_block: 1000, gas_cost_wei: cfg.gas_wei() };
        let (_, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "wallet", "wallet mode phai thang universal du universal dang bat");
    }

    // ===== Cụm `explicit-mode-flags` — wallet_scan_enabled/pair_scan_enabled =====

    /// `test_config()` (mọi test cũ ở file này, KHÔNG sửa 1 dòng nào) ship
    /// `wallet_scan_enabled=true`/`pair_scan_enabled=true` (mặc định giống
    /// hệt hành vi trước cụm này) — chứng minh `test_config_toml()` thêm 2
    /// field mới KHÔNG đổi hành vi mặc định (toàn bộ 187 test cũ ở file này
    /// vẫn pass nguyên, xem ĐẠT CẦN DÁN).
    #[test]
    fn explicit_mode_flags_default_ship_true_unchanged_behavior() {
        let cfg = test_config();
        assert!(cfg.wallet_scan_enabled, "test_config() phai ship wallet_scan_enabled=true (mac dinh)");
        assert!(cfg.pair_scan_enabled, "test_config() phai ship pair_scan_enabled=true (mac dinh)");
    }

    /// ĐẠT CẦN DÁN: `wallet_scan_enabled=false` — ví trong `victims.txt`
    /// KHÔNG còn ưu tiên, rơi xuống pair mode dù `pair_addr` CŨNG khớp
    /// `PairBook` (đối xứng với `decide_paper_v2_wallet_wins_over_pair_when_both_match`
    /// nhưng đảo kết quả vì wallet bị tắt tường minh).
    #[test]
    fn wallet_scan_disabled_falls_through_to_pair_when_both_match() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); // trong victims_ab, min 0.01
        let tx_value = U256::from(60_000_000_000_000_000u64); // >= pairs_min_swap_bnb 0.05
        let victims = victims_ab();
        let mut pairbook = PairBook::new();
        let pair_addr = addr("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        pairbook.insert_test_entry(pair_addr, "line", ResolvedFrom::Token); // pair_addr CUNG khop
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let risk = RiskGuard::new();
        let cfg = crate::config::Config::from_str(&test_config_toml(&[("wallet_scan_enabled = true", "wallet_scan_enabled = false")]))
            .expect("cfg wallet_scan_enabled=false phai load duoc");
        let input = PaperDecisionV2 { from, calldata: &calldata, tx_value, reserves: fixture_reserves(), pair_addr, current_block: 1000, gas_cost_wei: cfg.gas_wei() };
        let (outcome, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "pair", "wallet_scan_enabled=false phai khien tx roi xuong pair mode");
        match outcome {
            PipelineOutcome::Simulated(q) => assert!(q.profit_wei > 0),
            other => panic!("expect Simulated (pair mode), got {other:?}"),
        }
    }

    /// `wallet_scan_enabled=false` + KHÔNG có pair/universal nào khớp -> rơi
    /// hẳn xuống `not_in_list` dù `from` nằm trong `victims.txt`.
    #[test]
    fn wallet_scan_disabled_and_no_other_match_is_not_in_list() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); // trong victims_ab
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let pairbook = PairBook::new(); // rong
        let cache = TaxCache::new();
        let risk = RiskGuard::new();
        let cfg = crate::config::Config::from_str(&test_config_toml(&[("wallet_scan_enabled = true", "wallet_scan_enabled = false")]))
            .expect("cfg wallet_scan_enabled=false phai load duoc");
        let pair_addr = addr("0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead");
        let input = PaperDecisionV2 { from, calldata: &calldata, tx_value, reserves: fixture_reserves(), pair_addr, current_block: 1000, gas_cost_wei: cfg.gas_wei() };
        let (outcome, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "none");
        assert!(matches!(outcome, PipelineOutcome::Skip(PipelineSkip::NotInList)));
    }

    /// ĐẠT CẦN DÁN: `pair_scan_enabled=false` — pool trong `pairs.txt` KHÔNG
    /// còn match, rơi xuống universal mode (bật riêng) dù `pair_addr` khớp
    /// `PairBook`.
    #[test]
    fn pair_scan_disabled_falls_through_to_universal_when_pool_listed() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0x9999999999999999999999999999999999999999"); // khong trong victims_ab
        let tx_value = U256::from(60_000_000_000_000_000u64); // >= pairs_min_swap_bnb 0.05
        let victims = victims_ab();
        let mut pairbook = PairBook::new();
        let pair_addr = addr("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        pairbook.insert_test_entry(pair_addr, "line", ResolvedFrom::Token); // pair_addr khop PairBook
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let risk = RiskGuard::new();
        let cfg = crate::config::Config::from_str(&test_config_toml(&[
            ("pair_scan_enabled = true", "pair_scan_enabled = false"),
            ("pair_scan_universal = false", "pair_scan_universal = true"),
        ]))
        .expect("cfg pair_scan_enabled=false + universal=true phai load duoc");
        let input = PaperDecisionV2 { from, calldata: &calldata, tx_value, reserves: fixture_reserves(), pair_addr, current_block: 1000, gas_cost_wei: cfg.gas_wei() };
        let (outcome, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "universal", "pair_scan_enabled=false phai khien tx roi xuong universal mode");
        match outcome {
            PipelineOutcome::Simulated(q) => assert!(q.profit_wei > 0),
            other => panic!("expect Simulated (universal mode), got {other:?}"),
        }
    }

    /// `pair_scan_enabled=false` + universal cũng tắt (ship mặc định) -> rơi
    /// hẳn xuống `not_in_list` dù `pair_addr` khớp `PairBook`.
    #[test]
    fn pair_scan_disabled_and_universal_off_is_not_in_list() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0x9999999999999999999999999999999999999999");
        let tx_value = U256::from(60_000_000_000_000_000u64);
        let victims = victims_ab();
        let mut pairbook = PairBook::new();
        let pair_addr = addr("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        pairbook.insert_test_entry(pair_addr, "line", ResolvedFrom::Token);
        let cache = TaxCache::new();
        let risk = RiskGuard::new();
        let cfg = crate::config::Config::from_str(&test_config_toml(&[("pair_scan_enabled = true", "pair_scan_enabled = false")]))
            .expect("cfg pair_scan_enabled=false phai load duoc");
        let input = PaperDecisionV2 { from, calldata: &calldata, tx_value, reserves: fixture_reserves(), pair_addr, current_block: 1000, gas_cost_wei: cfg.gas_wei() };
        let (outcome, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "none");
        assert!(matches!(outcome, PipelineOutcome::Skip(PipelineSkip::NotInList)));
    }

    /// ĐẠT CẦN DÁN — kịch bản Chủ mô tả đúng nguyên văn: đúng 1 cờ `true`, 2
    /// cờ còn lại `false`, lần lượt cho cả 3 tổ hợp (chỉ wallet / chỉ pair /
    /// chỉ universal). CÙNG 1 tx (`from` khớp `victims.txt` VÀ `pair_addr`
    /// khớp `PairBook` — cả 3 mode ĐỀU CÓ THỂ match nếu bật) để chứng minh
    /// đúng NGUỒN match theo field tương ứng, không phải trùng hợp vì chỉ 1
    /// nhánh có dữ liệu khớp.
    #[test]
    fn explicit_mode_flags_exactly_one_true_matches_correct_source() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); // trong victims_ab, min 0.01
        let tx_value = U256::from(60_000_000_000_000_000u64); // du ca min victims (0.01) lan pairs_min_swap_bnb (0.05)
        let pair_addr = addr("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");

        let build_pairbook = || {
            let mut pb = PairBook::new();
            pb.insert_test_entry(pair_addr, "line", ResolvedFrom::Token);
            pb
        };
        let build_cache = || {
            let mut c = TaxCache::new();
            c.insert(token, TaxMeasurement::manual(0, 995));
            c
        };
        let victims = victims_ab();
        let risk = RiskGuard::new();

        // To hop 1: chi wallet true.
        let cfg_wallet_only = crate::config::Config::from_str(&test_config_toml(&[
            ("wallet_scan_enabled = true", "wallet_scan_enabled = true"), // giu true (mac dinh)
            ("pair_scan_enabled = true", "pair_scan_enabled = false"),
            ("pair_scan_universal = false", "pair_scan_universal = false"), // giu false (mac dinh)
        ]))
        .expect("to hop chi-wallet phai load duoc");
        let input = PaperDecisionV2 {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(),
            pair_addr,
            current_block: 1000,
            gas_cost_wei: cfg_wallet_only.gas_wei(),
        };
        let (outcome, source) = decide_paper_v2(&victims, &build_pairbook(), &build_cache(), &cfg_wallet_only, &risk, &input);
        assert_eq!(source, "wallet", "to hop chi-wallet=true phai match source=wallet");
        assert!(matches!(outcome, PipelineOutcome::Simulated(_)));

        // To hop 2: chi pair true.
        let cfg_pair_only = crate::config::Config::from_str(&test_config_toml(&[
            ("wallet_scan_enabled = true", "wallet_scan_enabled = false"),
            ("pair_scan_enabled = true", "pair_scan_enabled = true"), // giu true (mac dinh)
            ("pair_scan_universal = false", "pair_scan_universal = false"),
        ]))
        .expect("to hop chi-pair phai load duoc");
        let (outcome, source) = decide_paper_v2(&victims, &build_pairbook(), &build_cache(), &cfg_pair_only, &risk, &input);
        assert_eq!(source, "pair", "to hop chi-pair=true phai match source=pair");
        assert!(matches!(outcome, PipelineOutcome::Simulated(_)));

        // To hop 3: chi universal true.
        let cfg_universal_only = crate::config::Config::from_str(&test_config_toml(&[
            ("wallet_scan_enabled = true", "wallet_scan_enabled = false"),
            ("pair_scan_enabled = true", "pair_scan_enabled = false"),
            ("pair_scan_universal = false", "pair_scan_universal = true"),
        ]))
        .expect("to hop chi-universal phai load duoc");
        let (outcome, source) = decide_paper_v2(&victims, &build_pairbook(), &build_cache(), &cfg_universal_only, &risk, &input);
        assert_eq!(source, "universal", "to hop chi-universal=true phai match source=universal");
        assert!(matches!(outcome, PipelineOutcome::Simulated(_)));
    }

    /// Cụm BAOCAO14 — chiều victim BÁN token đã BỊ BỎ HẲN khỏi
    /// `decide_paper_v2` (xem doc-comment đầu file/`docs/STATE.md`):
    /// `swapExactTokensForETH` (path=[token,WBNB], `token_a != WBNB`) PHẢI
    /// luôn `not_wbnb_pair`, `source="none"` — dù `from` có trong
    /// `victims.txt` (khác hành vi cũ trước BAOCAO14 từng route hàm này vào
    /// `sim_v2::search_max_front_in_sell`, giờ hàm đó không còn tồn tại).
    #[test]
    fn decide_paper_v2_sell_direction_is_not_wbnb_pair() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let mut calldata = sel("swapExactTokensForETH(uint256,uint256,address[],address,uint256)").to_vec();
        calldata.extend_from_slice(&pad_u256(50_000_000_000_000_000u64)); // amountIn
        calldata.extend_from_slice(&pad_u256(0)); // amountOutMin
        calldata.extend_from_slice(&pad_u256(0xa0)); // offset path
        calldata.extend_from_slice(&pad_addr(addr("0x999999999999999999999999999999999999beef"))); // to
        calldata.extend_from_slice(&pad_u256(9_999_999_999)); // deadline
        calldata.extend_from_slice(&pad_u256(2)); // path.length
        calldata.extend_from_slice(&pad_addr(token));
        calldata.extend_from_slice(&pad_addr(wbnb()));
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); // trong victims_ab, min 0.01
        let victims = victims_ab();
        let pairbook = PairBook::new();
        let cache = TaxCache::new();
        let risk = RiskGuard::new();
        let cfg = test_config();
        let pair_addr = addr("0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead");
        let input = PaperDecisionV2 {
            from,
            calldata: &calldata,
            tx_value: U256::ZERO,
            reserves: fixture_reserves(),
            pair_addr,
            current_block: 1000,
            gas_cost_wei: cfg.gas_wei(),
        };
        let (outcome, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "none");
        assert!(
            matches!(outcome, PipelineOutcome::Skip(PipelineSkip::NotWbnbPair)),
            "chieu ban phai luon not_wbnb_pair, got {outcome:?}"
        );
    }

    // ===== Cụm `7.3` (BAOCAO16) + `exec-path-traps` (F-26) — decide_and_build_paper_v2 =====

    /// Cụm `exec-path-traps` (F-26) — sửa lại theo hành vi MỚI: `test_config()`
    /// ship `sim_engine="evm"`, nên `decide_and_build_paper_v2` KHÔNG còn
    /// build ngay khi `Simulated` nữa (build dời sang SAU khi EVM xác nhận,
    /// qua `build_paper_txs_from_evm_decision`) — outcome/source vẫn ĐÚNG
    /// (không đổi so `decide_paper_v2`), nhưng KHÔNG có `tx.build` LẪN
    /// `build.refused` nào (chưa từng gọi tới `executor::build_and_log_paper_sandwich`).
    /// Tên test đổi từ "logs_tx_build_when_simulated" để phản ánh đúng hành
    /// vi engine=evm (case engine=v2 build-ngay xem
    /// `decide_and_build_paper_v2_still_builds_immediately_for_v2_engine` bên dưới).
    #[test]
    fn decide_and_build_paper_v2_defers_build_to_evm_step_when_engine_is_evm() {
        let dir = tempfile::tempdir().unwrap();
        let logger = BotLogger::new(dir.path().join("logs").join("bot.jsonl")).unwrap();
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); // trong victims_ab, min 0.01
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let pairbook = PairBook::new();
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let risk = RiskGuard::new();
        let cfg = test_config();
        assert!(cfg.dry_run, "test_config phai dry_run=true (khop config ship)");
        assert!(cfg.sim_engine_is_evm(), "test_config ship sim_engine=evm");
        let pair_addr = addr("0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead");
        let input = PaperDecisionV2 { from, calldata: &calldata, tx_value, reserves: fixture_reserves(), pair_addr, current_block: 1000, gas_cost_wei: cfg.gas_wei() };

        let (outcome, source) = decide_and_build_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &logger, &input);
        assert_eq!(source, "wallet");
        match &outcome {
            PipelineOutcome::Simulated(q) => assert!(q.profit_wei > 0),
            other => panic!("expect Simulated, got {other:?}"),
        }

        let tail = logger.tail(10);
        assert!(tail.iter().all(|v| v["event"] != "tx.build"), "engine=evm khong duoc build NGAY, du outcome=Simulated");
        assert!(tail.iter().all(|v| v["event"] != "build.refused"), "engine=evm chua tung goi build_and_log_paper_sandwich o buoc nay");
    }

    /// F-26 mục "Khi sim_engine=v2 vẫn build theo v2 (giữ hành vi cũ cho
    /// engine cũ)" — override `sim_engine="v2"` trong `test_config_toml`, xác
    /// nhận `decide_and_build_paper_v2` VẪN thử build ngay (thấy `build.refused`
    /// engine="v2", vì `executor_self_address()` luôn `None` ở paper mode —
    /// KHÔNG thấy `tx.build` thật cho tới khi `7.3` nối signer, nhưng bằng
    /// chứng "có thử build ngay lập tức" là dòng `build.refused` xuất hiện
    /// NGAY tại bước này, khác engine=evm ở test trên).
    #[test]
    fn decide_and_build_paper_v2_still_attempts_build_immediately_for_v2_engine() {
        let dir = tempfile::tempdir().unwrap();
        let logger = BotLogger::new(dir.path().join("logs").join("bot.jsonl")).unwrap();
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let pairbook = PairBook::new();
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let risk = RiskGuard::new();
        let cfg = Config::from_str(&test_config_toml(&[("sim_engine = \"evm\"", "sim_engine = \"v2\"")])).unwrap();
        assert!(!cfg.sim_engine_is_evm());
        let pair_addr = addr("0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead");
        let input = PaperDecisionV2 { from, calldata: &calldata, tx_value, reserves: fixture_reserves(), pair_addr, current_block: 1000, gas_cost_wei: cfg.gas_wei() };

        let (outcome, _source) = decide_and_build_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &logger, &input);
        assert!(matches!(outcome, PipelineOutcome::Simulated(_)));

        let tail = logger.tail(10);
        assert!(tail.iter().all(|v| v["event"] != "tx.build"));
        let refused: Vec<_> = tail.iter().filter(|v| v["event"] == "build.refused").collect();
        assert_eq!(refused.len(), 1, "engine=v2 phai THU build NGAY (bi tu choi vi chua co signer, nhung co thu)");
        assert_eq!(refused[0]["engine"], "v2");
    }

    /// Đối chứng: candidate bị SKIP (không tới `Simulated`) -> KHÔNG có dòng
    /// `tx.build`/`build.refused` nào — `executor::build_and_log_paper_sandwich`
    /// không được gọi ngoài nhánh `Simulated`.
    #[test]
    fn decide_and_build_paper_v2_no_tx_build_when_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let logger = BotLogger::new(dir.path().join("logs").join("bot.jsonl")).unwrap();
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0x9999999999999999999999999999999999999999"); // khong trong victims.txt/pairs.txt
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let pairbook = PairBook::new();
        let cache = TaxCache::new();
        let risk = RiskGuard::new();
        let cfg = test_config();
        let pair_addr = addr("0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead");
        let input = PaperDecisionV2 { from, calldata: &calldata, tx_value, reserves: fixture_reserves(), pair_addr, current_block: 1000, gas_cost_wei: cfg.gas_wei() };

        let (outcome, source) = decide_and_build_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &logger, &input);
        assert_eq!(source, "none");
        assert!(matches!(outcome, PipelineOutcome::Skip(PipelineSkip::NotInList)));

        let tail = logger.tail(10);
        assert!(tail.iter().all(|v| v["event"] != "tx.build"), "khong duoc log tx.build khi bi skip");
        assert!(tail.iter().all(|v| v["event"] != "build.refused"), "khong duoc thu build khi bi skip");
    }

    // ===== Cụm `exec-path-traps` (F-26 + F-07) — build_paper_txs_from_evm_decision =====

    fn fixture_evm_outcome(front_in: U256, token_received: U256, back_out: U256, profit_wei: i128, victim_success: bool) -> crate::sim_evm::EvmSandwichOutcome {
        crate::sim_evm::EvmSandwichOutcome {
            front_in,
            token_received,
            back_out,
            profit_wei,
            victim_success,
            buy_tax_bps: None,
            sell_tax_bps: None,
        }
    }

    /// ĐẠT CẦN DÁN (mục 1, "Test: outcome v2=Simulated nhưng evm=None →
    /// không build"): `EvmDecision{outcome: Simulated(_), evm: None}` (đúng
    /// nhánh `PipelineOutcome::Skip(_) => evm:None` KHÔNG áp dụng ở đây — đây
    /// là trường hợp GIẢ ĐỊNH có Simulated nhưng evm chưa chạy, dựng tay để
    /// chứng minh guard `evm.is_some()` hoạt động độc lập với `outcome`) ->
    /// `build_paper_txs_from_evm_decision` PHẢI trả `None`, KHÔNG log gì cả
    /// (không gọi `executor::build_and_log_paper_sandwich`).
    #[test]
    fn build_paper_txs_from_evm_decision_no_build_when_evm_is_none_even_if_outcome_simulated() {
        let dir = tempfile::tempdir().unwrap();
        let logger = BotLogger::new(dir.path().join("logs").join("bot.jsonl")).unwrap();
        let cfg = test_config();
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let quote = crate::sim_v2::SandwichQuote {
            front_in: U256::from(1u64),
            front_out: U256::from(1u64),
            victim_out: U256::ZERO,
            back_out: U256::from(2u64),
            profit_wei: 1,
        };
        let decision = EvmDecision { outcome: PipelineOutcome::Simulated(quote), evm: None, tried: 0, total_ms: 0.0 };

        let result = build_paper_txs_from_evm_decision(&logger, &cfg, from, token, &decision);
        assert!(result.is_none());
        let tail = logger.tail(10);
        assert!(tail.is_empty(), "evm=None thi khong duoc goi build_and_log_paper_sandwich, khong log gi ca");
    }

    /// Đối chứng: `outcome == Skip(_)` (evm cũng luôn `None` trong nhánh này
    /// theo `decide_with_evm`) -> không build.
    #[test]
    fn build_paper_txs_from_evm_decision_no_build_when_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let logger = BotLogger::new(dir.path().join("logs").join("bot.jsonl")).unwrap();
        let cfg = test_config();
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let decision = EvmDecision { outcome: PipelineOutcome::Skip(PipelineSkip::Unprofitable), evm: None, tried: 1, total_ms: 5.0 };

        let result = build_paper_txs_from_evm_decision(&logger, &cfg, from, token, &decision);
        assert!(result.is_none());
    }

    /// ĐẠT CẦN DÁN (mục 3, F-07): `evm.is_some()` VÀ `outcome==Simulated` ->
    /// build được THỬ (bị `build.refused` vì paper mode chưa có signer, xem
    /// F-06) — quan trọng: `EvmDecision.outcome`'s `quote.front_out` (đã được
    /// `decide_with_evm` thay bằng `evm.token_received`, KHÔNG PHẢI `front_out`
    /// ước lượng của `sim_v2`) là số ĐÚNG mà `build_and_log_paper_sandwich`
    /// sẽ dùng làm `amountIn` back-sell nếu build thành công — test này dựng
    /// `token_received` NHỎ HƠN 1 `front_out` giả định (mô phỏng tax mua) để
    /// chứng minh guard `evm.is_some()` cho qua đúng candidate, và đường build
    /// nhận đúng struct đã thay số EVM (không phải struct v2 gốc).
    #[test]
    fn build_paper_txs_from_evm_decision_attempts_build_using_evm_adjusted_quote() {
        let dir = tempfile::tempdir().unwrap();
        let logger = BotLogger::new(dir.path().join("logs").join("bot.jsonl")).unwrap();
        let cfg = test_config();
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        // token_received (45 token, da tru tax) < front_out uoc luong sim_v2
        // gia dinh se lon hon (vd 50 token) - decide_with_evm da THAY the so
        // nay TRUOC khi toi day (test o day dung thang gia tri da thay).
        let token_received = U256::from(45_000_000_000_000_000_000u128);
        let quote_with_evm_numbers = crate::sim_v2::SandwichQuote {
            front_in: U256::from(50_000_000_000_000_000u64),
            front_out: token_received, // DA la evm.token_received, khong phai uoc luong sim_v2
            victim_out: U256::ZERO,
            back_out: U256::from(57_000_000_000_000_000u64),
            profit_wei: 7_000_000_000_000_000i128,
        };
        let evm = fixture_evm_outcome(
            U256::from(50_000_000_000_000_000u64),
            token_received,
            U256::from(57_000_000_000_000_000u64),
            7_000_000_000_000_000i128,
            true,
        );
        let decision = EvmDecision { outcome: PipelineOutcome::Simulated(quote_with_evm_numbers), evm: Some(evm), tried: 1, total_ms: 12.3 };

        let result = build_paper_txs_from_evm_decision(&logger, &cfg, from, token, &decision);
        assert!(result.is_none(), "paper mode chua co signer -> build.refused, khong phai Some");
        let tail = logger.tail(10);
        let refused: Vec<_> = tail.iter().filter(|v| v["event"] == "build.refused").collect();
        assert_eq!(refused.len(), 1, "phai co dung 1 lan THU build (bi tu choi)");
        assert_eq!(refused[0]["engine"], "evm");
    }

    // ===== Cụm `exec-path-traps` (F-14) — deadline gate =====

    #[test]
    fn deadline_expired_none_never_applies_ur_tx() {
        assert!(!deadline_expired(None, 1_000_000));
    }

    #[test]
    fn deadline_expired_true_when_deadline_in_the_past() {
        assert!(deadline_expired(Some(U256::from(999u64)), 1_000_000));
    }

    #[test]
    fn deadline_expired_true_when_deadline_within_2_blocks_ahead() {
        // now=1000, buffer=2*3=6s -> deadline=1005 (< 1006) van coi la sap het han.
        assert!(deadline_expired(Some(U256::from(1005u64)), 1_000));
        assert!(!deadline_expired(Some(U256::from(1006u64)), 1_000));
    }

    #[test]
    fn deadline_expired_false_when_far_in_future_or_unrepresentable() {
        assert!(!deadline_expired(Some(U256::from(9_999_999_999u64)), 1_000_000));
        assert!(!deadline_expired(Some(U256::MAX), 1_000_000));
    }

    /// ĐẠT CẦN DÁN (mục 8, F-14) — candidate hợp lệ về mọi mặt khác (wallet
    /// mode, đủ min-size, pool đủ sâu, `sim_engine="v2"` để khỏi cần fork
    /// EVM) nhưng `deadline` đã QUA (1 giây Unix epoch, chắc chắn ở quá khứ)
    /// -> `PipelineSkip::Deadline`, KHÔNG lọt tới bước sim.
    #[test]
    fn decide_paper_v2_expired_deadline_is_skipped_with_deadline_reason() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens_with_deadline(token, 0, 1);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); // trong victims_ab, min 0.01
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let pairbook = PairBook::new();
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let risk = RiskGuard::new();
        let cfg = Config::from_str(&test_config_toml(&[("sim_engine = \"evm\"", "sim_engine = \"v2\"")])).unwrap();
        let pair_addr = addr("0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead");
        let input = PaperDecisionV2 { from, calldata: &calldata, tx_value, reserves: fixture_reserves(), pair_addr, current_block: 1000, gas_cost_wei: cfg.gas_wei() };

        let (outcome, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "wallet");
        assert!(matches!(outcome, PipelineOutcome::Skip(PipelineSkip::Deadline)), "expect Deadline, got {outcome:?}");
    }

    /// Đối chứng: deadline xa tương lai -> KHÔNG bị chặn bởi gate này (vẫn
    /// `Simulated`, cùng fixture với test `Simulated` cũ, chỉ đổi engine="v2"
    /// để không cần fork EVM thật).
    #[test]
    fn decide_paper_v2_far_future_deadline_not_blocked() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0); // deadline=9_999_999_999
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let pairbook = PairBook::new();
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let risk = RiskGuard::new();
        let cfg = Config::from_str(&test_config_toml(&[("sim_engine = \"evm\"", "sim_engine = \"v2\"")])).unwrap();
        let pair_addr = addr("0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead");
        let input = PaperDecisionV2 { from, calldata: &calldata, tx_value, reserves: fixture_reserves(), pair_addr, current_block: 1000, gas_cost_wei: cfg.gas_wei() };

        let (outcome, _source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert!(matches!(outcome, PipelineOutcome::Simulated(_)), "expect Simulated, got {outcome:?}");
    }

    // ===== Cụm `usdt-quote-asset` (BAOCAO29) — decide_paper_quote =====

    /// Fixture `swapExactTokensForTokens(amountIn, amountOutMin, path, to,
    /// deadline)` — path=[quote_in, token_out], amount_in truyền thẳng (khác
    /// `build_eth_for_tokens` lấy amount từ `tx.value`). Dùng `u128` (không
    /// phải `u64`) vì số USDT-wei test cần lớn hơn `u64::MAX` (vd 1000 USDT =
    /// `1000 * 1e18`).
    fn build_tokens_for_tokens(amount_in: u128, amount_out_min: u64, quote_in: Address, token_out: Address) -> Vec<u8> {
        let mut out = sel("swapExactTokensForTokens(uint256,uint256,address[],address,uint256)").to_vec();
        out.extend_from_slice(&U256::from(amount_in).to_be_bytes::<32>());
        out.extend_from_slice(&pad_u256(amount_out_min));
        out.extend_from_slice(&pad_u256(0xa0)); // offset path = 5 head words * 32
        out.extend_from_slice(&pad_addr(addr("0x999999999999999999999999999999999999beef"))); // to
        out.extend_from_slice(&pad_u256(9_999_999_999)); // deadline
        out.extend_from_slice(&pad_u256(2)); // path.length
        out.extend_from_slice(&pad_addr(quote_in));
        out.extend_from_slice(&pad_addr(token_out));
        out
    }

    /// Pool quote USDT SÂU (20.000 USDT — TRÊN `min_reserve_usdt=15000` ship),
    /// cùng tỉ lệ token như `fixture_reserves()` (chỉ scale phần quote lên
    /// 20.000x) — dùng cho mọi test USDT cần qua khỏi `thin_liq`.
    fn usdt_deep_reserves() -> PoolReserves {
        PoolReserves {
            reserve_wbnb: U256::from(20_000u64) * U256::from(1_000_000_000_000_000_000u128), // 20.000 USDT (field tai dung, xem doc-comment QuoteAsset)
            reserve_token: U256::from(1_000_000u64) * U256::from(1_000_000_000_000_000_000u128),
        }
    }

    const USDT_VICTIM_1000_WEI: u128 = 1_000_000_000_000_000_000_000; // 1000 USDT (5% cua usdt_deep_reserves, giong ty le fixture_reserves)

    fn cfg_usdt_enabled(overrides: &[(&str, &str)]) -> crate::config::Config {
        let mut all = vec![("scan_quote_usdt = false", "scan_quote_usdt = true")];
        all.extend_from_slice(overrides);
        crate::config::Config::from_str(&test_config_toml(&all)).expect("cfg usdt-enabled phai load duoc")
    }

    /// `scan_quote_usdt=false` (ship mac dinh) -> swap quote USDT phai
    /// `not_quote_pair`, KHONG duoc coi la candidate (dung CLAUDE.md
    /// "scan_quote_usdt=false -> hanh vi WBNB khong doi gi", ngam y USDT
    /// KHONG duoc bat len khi cha chua bat co).
    #[test]
    fn usdt_quote_disabled_by_default_is_not_quote_pair() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_tokens_for_tokens(50_000_000_000_000_000, 0, usdt(), token);
        let cache = TaxCache::new();
        let cfg = test_config();
        let pairbook = PairBook::new();
        let pair_addr = addr("0xf00df00df00df00df00df00df00df00df00df00d");
        assert!(!cfg.scan_quote_usdt, "ship default phai la false");
        let (outcome, tag) =
            decide_paper_quote(&pairbook, pair_addr, &cache, &cfg, &calldata, U256::ZERO, fixture_reserves(), 1000, cfg.gas_wei(), cfg.gas_wei());
        assert_eq!(tag, "none");
        assert!(matches!(outcome, PipelineOutcome::Skip(PipelineSkip::NotQuotePair)), "got {outcome:?}");
    }

    /// Cụm `hotpath-fix-then-decoder-ur` (A2) — ĐẠT CẦN DÁN: pool USDT KHÔNG
    /// có trong `pairs.txt` (`PairBook` rỗng) -> `not_in_list`, KHÔNG còn hành
    /// vi "universal" ngầm định như trước fix (mọi pool USDT đủ sâu đều thành
    /// candidate). Đúng `strategy-lock-mode2`: pairs.txt là nguồn candidate
    /// DUY NHẤT, áp dụng CẢ 2 quote asset.
    #[test]
    fn usdt_quote_pool_not_in_pairs_txt_is_not_in_list() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_tokens_for_tokens(USDT_VICTIM_1000_WEI, 0, usdt(), token);
        let cache = TaxCache::new();
        let cfg = cfg_usdt_enabled(&[]);
        let pairbook = PairBook::new(); // rong - khong biet pool nao
        let pair_addr = addr("0xf00df00df00df00df00df00df00df00df00df00d");
        let (outcome, tag) =
            decide_paper_quote(&pairbook, pair_addr, &cache, &cfg, &calldata, U256::ZERO, usdt_deep_reserves(), 1000, cfg.gas_wei(), cfg.gas_wei());
        assert_eq!(tag, "none");
        assert!(matches!(outcome, PipelineOutcome::Skip(PipelineSkip::NotInList)), "got {outcome:?}");
    }

    /// Bật `scan_quote_usdt=true`, pool ĐÃ VET trong `pairs.txt` nhưng quá
    /// mỏng (`fixture_reserves()` = 1 WBNB-tương-đương, dưới
    /// `min_reserve_usdt=15000` ship) -> `thin_liq`, chứng minh ngưỡng USDT
    /// ĐÚNG field (`min_reserve_usdt`, không lẫn `min_reserve_wbnb`).
    #[test]
    fn usdt_quote_enabled_thin_liq_when_pool_reserve_below_min_reserve_usdt() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_tokens_for_tokens(50_000_000_000_000_000, 0, usdt(), token);
        let cache = TaxCache::new();
        let cfg = cfg_usdt_enabled(&[]);
        let pair_addr = addr("0xf00df00df00df00df00df00df00df00df00df00d");
        let mut pairbook = PairBook::new();
        pairbook.insert_test_vetted_pair(pair_addr, usdt());
        let (outcome, tag) =
            decide_paper_quote(&pairbook, pair_addr, &cache, &cfg, &calldata, U256::ZERO, fixture_reserves(), 1000, cfg.gas_wei(), cfg.gas_wei());
        assert_eq!(tag, "usdt");
        assert!(matches!(outcome, PipelineOutcome::Skip(PipelineSkip::ThinLiq)), "got {outcome:?}");
    }

    /// Cụm `hotpath-fix-then-decoder-ur` (A2) — ĐẠT CẦN DÁN, tái hiện ĐÚNG bug
    /// BAOCAO38 mục 0 cho nhánh USDT: pool ĐÃ VET (`is_tax_ok=true`) + pool
    /// đủ sâu, `TaxCache` RỖNG hoàn toàn (giống đường nóng thật, không ai tự
    /// động điền) -> PHẢI tới `Simulated`, KHÔNG được rơi vào `honeypot_or_tax`
    /// giả như trước fix (paper run 5 phút: `honeypot_or_tax≈240/phút` ≈ toàn
    /// bộ `by_quote.usdt`).
    #[test]
    fn usdt_quote_vetted_pair_skips_tax_cache_even_when_cache_empty() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_tokens_for_tokens(USDT_VICTIM_1000_WEI, 0, usdt(), token);
        let cache = TaxCache::new(); // RONG - dung y, mo phong duong nong that
        let cfg = cfg_usdt_enabled(&[("sim_engine = \"evm\"", "sim_engine = \"v2\"")]);
        let pair_addr = addr("0xf00df00df00df00df00df00df00df00df00df00d");
        let mut pairbook = PairBook::new();
        pairbook.insert_test_vetted_pair(pair_addr, usdt());
        let (outcome, tag) =
            decide_paper_quote(&pairbook, pair_addr, &cache, &cfg, &calldata, U256::ZERO, usdt_deep_reserves(), 1000, cfg.gas_wei(), cfg.gas_wei());
        assert_eq!(tag, "usdt");
        assert!(matches!(outcome, PipelineOutcome::Simulated(_)), "got {outcome:?} - pool da vet KHONG duoc tra TaxCache");
    }

    /// Đối chứng — pool ĐÃ BIẾT trong `pairs.txt` nhưng vet NỀN vừa loại
    /// (`set_vet_result(ok=false)`) -> `honeypot_or_tax` rõ ràng (KHÔNG rơi
    /// vào `not_in_list`, giữ visibility, cùng cơ chế nhánh "pair" của
    /// `decide_paper_v2`).
    #[test]
    fn usdt_quote_vet_failed_pool_is_honeypot_or_tax() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_tokens_for_tokens(USDT_VICTIM_1000_WEI, 0, usdt(), token);
        let cache = TaxCache::new();
        let cfg = cfg_usdt_enabled(&[("sim_engine = \"evm\"", "sim_engine = \"v2\"")]);
        let pair_addr = addr("0xf00df00df00df00df00df00df00df00df00df00d");
        let mut pairbook = PairBook::new();
        pairbook.insert_test_vetted_pair(pair_addr, usdt());
        pairbook.set_vet_result(
            pair_addr,
            crate::pairbook::VetResult { buy_bps: 0, sell_bps: 2000, honeypot: false, block: 1 },
            false,
        );
        let (outcome, tag) =
            decide_paper_quote(&pairbook, pair_addr, &cache, &cfg, &calldata, U256::ZERO, usdt_deep_reserves(), 1000, cfg.gas_wei(), cfg.gas_wei());
        assert_eq!(tag, "usdt");
        assert!(matches!(outcome, PipelineOutcome::Skip(PipelineSkip::HoneypotOrTax)), "got {outcome:?}");
    }

    /// Cụm `real-economics-mode2` (mục 1.d) — CLAUDE.md Math ĐÃ SỬA: bỏ luật
    /// cũ "profit_usdt không trừ gas" (test cũ
    /// `usdt_quote_profit_has_no_gas_subtracted_matches_gas_wei_zero_exactly`
    /// đã XOÁ, hành vi đó không còn đúng). Giờ `gas_cost_in_quote_wei` (đã
    /// quy đổi sẵn bởi caller — `main.rs` qua `convert_gas_cost_bnb_to_usdt`)
    /// được trừ THẲNG vào profit USDT, y hệt cơ chế WBNB — đối chiếu
    /// `profit_wei`/`front_in` trả về từ `decide_paper_quote` với
    /// `sim_v2::search_max_front_in` gọi TRỰC TIẾP cùng `gas_cost_in_quote_wei`.
    #[test]
    fn usdt_quote_profit_subtracts_gas_converted_to_usdt() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_tokens_for_tokens(USDT_VICTIM_1000_WEI, 0, usdt(), token);
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let cfg = cfg_usdt_enabled(&[]);
        let pair_addr = addr("0xf00df00df00df00df00df00df00df00df00df00d");
        let mut pairbook = PairBook::new();
        pairbook.insert_test_vetted_pair(pair_addr, usdt());
        let gas_cost_bnb_wei = cfg.gas_wei();
        // Gia tri gia lap "da quy doi sang USDT" (0.5 USDT, 18dp) - trong test
        // nay chi can KHAC 0 va nho hon loi nhuan du de van con Simulated.
        let gas_cost_usdt_wei: u128 = 500_000_000_000_000_000u128;

        let (outcome, tag) = decide_paper_quote(
            &pairbook,
            pair_addr,
            &cache,
            &cfg,
            &calldata,
            U256::ZERO,
            usdt_deep_reserves(),
            1000,
            gas_cost_bnb_wei,
            gas_cost_usdt_wei,
        );
        assert_eq!(tag, "usdt");
        let q = match outcome {
            PipelineOutcome::Simulated(q) => q,
            other => panic!("expect Simulated, got {other:?}"),
        };
        println!("usdt quote sim OK: front_in={} back_out={} profit_wei={}", q.front_in, q.back_out, q.profit_wei);
        assert!(q.profit_wei > 0);

        let front_cap = RiskGuard::front_cap_after_gas_reserve(cfg.max_front_usdt_wei(), cfg.gas_reserve_bnb_wei);
        let expected_with_gas = sim_v2::search_max_front_in(
            usdt_deep_reserves(),
            U256::from(USDT_VICTIM_1000_WEI),
            front_cap,
            gas_cost_usdt_wei,
        )
        .expect("phai co quote");
        let expected_no_gas =
            sim_v2::search_max_front_in(usdt_deep_reserves(), U256::from(USDT_VICTIM_1000_WEI), front_cap, 0).expect("phai co quote");

        assert_eq!(q.front_in, expected_with_gas.front_in);
        assert_eq!(q.profit_wei, expected_with_gas.profit_wei, "nhanh USDT phai khop CHINH XAC gas_cost_in_quote_wei da truyen vao");
        assert_ne!(q.profit_wei, expected_no_gas.profit_wei, "gas phai THUC SU duoc tru, khac luat cu 'khong tru gas'");
    }

    /// F-03 — `gas_cost_bnb_wei` (LUÔN đơn vị BNB, bất kể quote asset) vượt
    /// trần `cfg.gas_wei()` -> `gas_cap`, KHÔNG phụ thuộc `gas_cost_in_quote_wei`
    /// nhỏ tới đâu (trần gas so bằng BNB vì gas luôn trả bằng BNB).
    #[test]
    fn usdt_quote_gas_cap_when_gas_cost_bnb_exceeds_config_cap() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_tokens_for_tokens(USDT_VICTIM_1000_WEI, 0, usdt(), token);
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let cfg = cfg_usdt_enabled(&[]);
        let pair_addr = addr("0xf00df00df00df00df00df00df00df00df00df00d");
        let mut pairbook = PairBook::new();
        pairbook.insert_test_vetted_pair(pair_addr, usdt());
        let gas_cost_bnb_wei = cfg.gas_wei() + 1; // vuot tran dung 1 wei
        let (outcome, tag) =
            decide_paper_quote(&pairbook, pair_addr, &cache, &cfg, &calldata, U256::ZERO, usdt_deep_reserves(), 1000, gas_cost_bnb_wei, 1u128);
        assert_eq!(tag, "usdt");
        assert!(matches!(outcome, PipelineOutcome::Skip(PipelineSkip::GasCap)), "got {outcome:?}");
    }

    /// Lãi mô phỏng DƯƠNG nhưng dưới `min_profit_usdt` chỉnh tay rất cao ->
    /// `unprofitable`, chứng minh ngưỡng USDT đọc ĐÚNG field
    /// (`min_profit_usdt`, không lẫn `min_profit_bnb`).
    #[test]
    fn usdt_quote_unprofitable_when_profit_below_min_profit_usdt_config() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_tokens_for_tokens(USDT_VICTIM_1000_WEI, 0, usdt(), token);
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let cfg = cfg_usdt_enabled(&[("min_profit_usdt = 3.0", "min_profit_usdt = 1000000.0")]);
        let pair_addr = addr("0xf00df00df00df00df00df00df00df00df00df00d");
        let mut pairbook = PairBook::new();
        pairbook.insert_test_vetted_pair(pair_addr, usdt());
        let (outcome, tag) =
            decide_paper_quote(&pairbook, pair_addr, &cache, &cfg, &calldata, U256::ZERO, usdt_deep_reserves(), 1000, cfg.gas_wei(), cfg.gas_wei());
        assert_eq!(tag, "usdt");
        assert!(matches!(outcome, PipelineOutcome::Skip(PipelineSkip::Unprofitable)), "got {outcome:?}");
    }

    /// Đối chứng — quote WBNB đi qua ĐÚNG entrypoint MỚI (`decide_paper_quote`)
    /// vẫn hoạt động y hệt (không bị phá bởi việc thêm nhánh USDT song song).
    /// Nhánh WBNB không đụng `pairbook`/`pair_addr` (xem doc-comment
    /// `decide_paper_quote`) nên truyền `PairBook` rỗng + địa chỉ bất kỳ.
    #[test]
    fn decide_paper_quote_wbnb_branch_still_works_when_usdt_disabled() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let cfg = test_config(); // scan_quote_usdt=false ship
        let pairbook = PairBook::new();
        let pair_addr = addr("0xf00df00df00df00df00df00df00df00df00df00d");
        let (outcome, tag) =
            decide_paper_quote(&pairbook, pair_addr, &cache, &cfg, &calldata, tx_value, fixture_reserves(), 1000, cfg.gas_wei(), cfg.gas_wei());
        assert_eq!(tag, "wbnb");
        match outcome {
            PipelineOutcome::Simulated(q) => assert!(q.profit_wei > 0),
            other => panic!("expect Simulated (nhanh WBNB qua decide_paper_quote), got {other:?}"),
        }
    }

    /// `precheck_quote_only` KHÔNG nhận `Provider` (bảo đảm ở mức kiểu, cùng
    /// tinh thần `precheck_without_reserves_never_touches_rpc_for_every_early_skip_reason`)
    /// — 2 nhánh skip sớm (`decode_fail`/`not_quote_pair`) đều dừng đúng ở
    /// bước thuần này.
    #[test]
    fn precheck_quote_only_never_touches_rpc_for_early_skip_reasons() {
        let garbage = vec![0xde, 0xad, 0xbe, 0xef];
        assert_eq!(precheck_quote_only(&garbage, U256::ZERO, true), Err(PipelineSkip::DecodeFail));

        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_tokens_for_tokens(50_000_000_000_000_000, 0, usdt(), token);
        assert_eq!(precheck_quote_only(&calldata, U256::ZERO, false), Err(PipelineSkip::NotQuotePair));
        assert_eq!(precheck_quote_only(&calldata, U256::ZERO, true), Ok((token, QuoteAsset::Usdt)));
    }

    // ===== Cụm `quote-live-wiring-funnel-diagnostics` — 2 test bắt buộc theo
    // lệnh mục (4): decode calldata THẬT cấu trúc (không bịa selector, dùng
    // lại builder fixture đã có sẵn từ BAOCAO29) qua ĐÚNG hàm quote-aware
    // (`decode_and_classify_quote` — "quote" là khái niệm tầng pipeline, decoder.rs
    // không biết WBNB/USDT là gì, chỉ biết path 2 token).

    /// ĐẠT CẦN DÁN (lệnh mục 4, test 1): decode `swapExactETHForTokens` ->
    /// quote PHẢI ra WBNB, `amountIn = tx.value` (ETH gửi kèm, không nằm
    /// trong calldata — đúng quy ước `decoder.rs`).
    #[test]
    fn decode_and_classify_quote_swap_exact_eth_for_tokens_gives_wbnb_and_amount_in_eq_tx_value() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let tx_value = U256::from(37_000_000_000_000_000u64); // 0.037 BNB, gia tri ro rang khac 0
        let calldata = build_eth_for_tokens(token, 0);
        let (decoded, out_token, quote) =
            decode_and_classify_quote(&calldata, tx_value, false).expect("swapExactETHForTokens phai decode+quote duoc");
        assert_eq!(quote, QuoteAsset::Wbnb, "quote asset phai la WBNB");
        assert_eq!(out_token, token);
        assert_eq!(decoded.amount_in, tx_value, "amountIn cua swapExactETHForTokens phai = tx.value, khong phai tu calldata");
        assert_eq!(decoded.selector_name, "swapExactETHForTokens");
    }

    /// ĐẠT CẦN DÁN (lệnh mục 4, test 2): decode `swapExactTokensForTokens`
    /// path `[USDT, X]` -> quote PHẢI ra USDT (`scan_quote_usdt=true`).
    #[test]
    fn decode_and_classify_quote_swap_exact_tokens_for_tokens_usdt_path_gives_usdt() {
        let token = addr("0xdddddddddddddddddddddddddddddddddddddddd");
        let amount_in: u128 = 500_000_000_000_000_000_000; // 500 USDT (18 decimal, dung u128 vi vuot u64::MAX)
        let calldata = build_tokens_for_tokens(amount_in, 0, usdt(), token);
        let (decoded, out_token, quote) = decode_and_classify_quote(&calldata, U256::ZERO, true)
            .expect("swapExactTokensForTokens path [USDT,X] phai decode+quote duoc khi scan_quote_usdt=true");
        assert_eq!(quote, QuoteAsset::Usdt, "quote asset phai la USDT");
        assert_eq!(out_token, token);
        assert_eq!(decoded.amount_in, U256::from(amount_in));
        assert_eq!(decoded.selector_name, "swapExactTokensForTokens");
    }

    /// Đối chứng cho `PipelineSkip::SellDirection` mới: path `[X, WBNB]`
    /// (victim BÁN token lấy WBNB, không phải MUA) qua `decode_and_classify_quote`
    /// phải trả `sell_direction` — chính xác hơn `not_quote_pair` (WBNB CÓ
    /// mặt trong path, chỉ sai phía) — trong khi `decode_and_classify` cũ
    /// (WBNB-only, `decide_paper_v2`) KHÔNG bị đổi, vẫn trả `not_wbnb_pair`
    /// y hệt trước (verify ở test riêng dưới).
    #[test]
    fn decode_and_classify_quote_sell_direction_is_distinguished_from_not_quote_pair() {
        let token = addr("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        let calldata = build_tokens_for_tokens(123_456_789, 0, token, wbnb()); // path=[token, WBNB] = ban token lay WBNB
        let err = decode_and_classify_quote(&calldata, U256::ZERO, true).unwrap_err();
        assert_eq!(err, PipelineSkip::SellDirection);

        // decode_and_classify (WBNB-only cu, dung boi decide_paper_v2) KHONG
        // doi hanh vi - van la not_wbnb_pair y het truoc phien nay.
        let err_old = decode_and_classify(&calldata, U256::ZERO).unwrap_err();
        assert_eq!(err_old, PipelineSkip::NotWbnbPair);
    }

    // ===== Cụm `real-economics-mode2` (F-03) — compute_gas_cost_wei / gas_cap =====

    #[test]
    fn compute_gas_cost_wei_uses_max_of_oracle_and_victim_gas_price() {
        let cost_oracle_higher = compute_gas_cost_wei(100_000, 100_000, 5_000_000_000, 1_000_000_000, 10_000_000_000);
        assert_eq!(cost_oracle_higher, 200_000u128 * 5_000_000_000u128);
        let cost_victim_higher = compute_gas_cost_wei(100_000, 100_000, 1_000_000_000, 5_000_000_000, 10_000_000_000);
        assert_eq!(cost_victim_higher, 200_000u128 * 5_000_000_000u128, "victim.gas_price cao hon oracle phai duoc dung");
    }

    #[test]
    fn compute_gas_cost_wei_sentinel_when_oracle_price_exceeds_gas_price_max() {
        let cost = compute_gas_cost_wei(100_000, 100_000, 11_000_000_000, 0, 10_000_000_000);
        assert_eq!(cost, u128::MAX, "oracle gas price vuot gas_price_max_gwei -> sentinel gas_cap");
    }

    #[test]
    fn compute_gas_cost_wei_gas_price_max_zero_disables_ceiling_check() {
        // gas_price_max_wei=0 nghia la KHONG check tran gia (cung tinh than
        // "0 = tat" cua max_exposure_bnb/max_consecutive_loss).
        let cost = compute_gas_cost_wei(100_000, 100_000, 999_000_000_000, 0, 0);
        assert_eq!(cost, 200_000u128 * 999_000_000_000u128);
    }

    /// ĐẠT CẦN DÁN (F-03) — `gas_cost_wei` vượt trần `cfg.gas_wei()` (đúng 1
    /// wei) -> `gas_cap`, KHÔNG rơi vào `unprofitable`/`Simulated` — chứng
    /// minh gate mới hoạt động ở tầng `decide_paper_v2` đầy đủ (không chỉ
    /// đơn vị `evaluate_candidate`).
    #[test]
    fn decide_paper_v2_wallet_mode_gas_cap_when_gas_cost_exceeds_config_cap() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); // trong victims_ab
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let pairbook = PairBook::new();
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let risk = RiskGuard::new();
        let cfg = test_config();
        let pair_addr = addr("0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead");
        let input = PaperDecisionV2 {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(),
            pair_addr,
            current_block: 1000,
            gas_cost_wei: cfg.gas_wei() + 1, // vuot tran dung 1 wei
        };
        let (outcome, source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert_eq!(source, "wallet");
        assert!(matches!(outcome, PipelineOutcome::Skip(PipelineSkip::GasCap)), "got {outcome:?}");
    }

    /// Đối chứng: `gas_cost_wei == cfg.gas_wei()` (đúng bằng trần, KHÔNG vượt)
    /// vẫn phải đi tới sim bình thường — trần chỉ chặn khi VƯỢT, không chặn
    /// khi bằng.
    #[test]
    fn decide_paper_v2_gas_cost_equal_to_cap_still_reaches_sim() {
        let token = addr("0xcccccccccccccccccccccccccccccccccccccccc");
        let calldata = build_eth_for_tokens(token, 0);
        let from = addr("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let tx_value = U256::from(50_000_000_000_000_000u64);
        let victims = victims_ab();
        let pairbook = PairBook::new();
        let mut cache = TaxCache::new();
        cache.insert(token, TaxMeasurement::manual(0, 995));
        let risk = RiskGuard::new();
        let cfg = test_config();
        let pair_addr = addr("0xdeaddeaddeaddeaddeaddeaddeaddeaddeaddead");
        let input = PaperDecisionV2 {
            from,
            calldata: &calldata,
            tx_value,
            reserves: fixture_reserves(),
            pair_addr,
            current_block: 1000,
            gas_cost_wei: cfg.gas_wei(),
        };
        let (outcome, _source) = decide_paper_v2(&victims, &pairbook, &cache, &cfg, &risk, &input);
        assert!(matches!(outcome, PipelineOutcome::Simulated(_)), "gas_cost_wei == tran (khong vuot) khong duoc gas_cap, got {outcome:?}");
    }
}

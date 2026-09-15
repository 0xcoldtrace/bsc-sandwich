//! Cụm 7.3 (executor gửi tx thật) CHƯA làm trong phiên này. File này chứa
//! cổng kiểm tra điều kiện live (đọc, không gửi gì) + load signer từ `.env`
//! (đọc private key, KHÔNG ký/gửi gì) — đúng CLAUDE.md mục "Live". Không có
//! hàm gửi giao dịch (`send_raw_transaction` hay tương đương) nào tồn tại
//! trong repo — cấm bịa executor trước 7.3.
//!
//! **`load_signer` trả `B256` (32 byte thô), KHÔNG PHẢI
//! `alloy::signers::local::PrivateKeySigner`** — lý do kỹ thuật (không phải
//! tuỳ tiện đổi thiết kế): bật feature `signer-local` của crate `alloy` làm
//! `cargo check` FAIL NGAY ở bước resolve dependency, CÙNG LỚP LỖI đã ghi ở
//! `docs/STATE.md` mục "5.2" (`alloy-rpc-types-txpool`) — `alloy-provider`/
//! `alloy 2.4.2` (bản đang pin) đòi `alloy-signer-local = "^2.4.2"` nhưng
//! crate đó CHƯA có bản `2.4.2` phát hành trên crates.io (chỉ tới `2.4.1`)
//! tại ngày phiên này (`cargo check` in rõ danh sách candidate versions
//! `2.4.1, 2.4.0, 2.3.0, ...`, không có `2.4.2`). Theo đúng quyết định đã
//! chốt ở `5.2` ("KHÔNG hạ version alloy xuống 2.4.1" — tránh phá pin +
//! rủi ro breaking API khác), phiên này KHÔNG bật `signer-local`. `B256` (từ
//! `alloy_primitives`, ĐÃ có sẵn trong dependency, không cần feature mới) đủ
//! để "đọc private key từ `.env`, validate đúng 32 byte hex, KHÔNG ký/gửi gì"
//! — đúng phạm vi `7.1` (live gate + đọc signer). Việc ký/gửi tx thật (`7.3`,
//! cần `Signer`/`TxSigner` đầy đủ) sẽ cần Grok quyết định lệnh sau: chờ
//! `alloy-signer-local` bắt kịp version, hoặc bơm `alloy` lên bản mới hơn khi
//! đến `7.3` (không phải phạm vi phiên này).

use crate::calldata::{encode_back_sell, encode_front_buy};
use crate::config::Config;
/// Cụm `exec-path-traps` (F-05) — `gate_check` giờ SỐNG DUY NHẤT ở
/// `config.rs` như method `Config::gate_check` (không còn hàm rời trong
/// module này) — `LiveGateStatus` (kiểu trả về) re-export lại đây để code cũ
/// `use crate::executor::LiveGateStatus` không phải đổi đường import.
pub use crate::config::LiveGateStatus;
use crate::logger::BotLogger;
use crate::sim_v2::SandwichQuote;
use crate::venues::{V2_ROUTER_ADDRESS, WBNB_ADDRESS};
use alloy::primitives::{Address, Bytes, B256, U256};
use serde_json::json;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

/// Trả true chỉ khi TẤT CẢ điều kiện Live trong CLAUDE.md đều thoả. Vì
/// `dry_run=true` theo config ship mặc định, hàm này luôn false cho tới khi
/// chủ tự đổi cờ (không phải Claude Code tự bật).
///
/// Cụm `exec-path-traps` (F-05) — bỏ tham số `gas_cap_positive` (bản cũ nhận
/// rời, có thể lệch với `cfg.front_max_gas_bnb_wei`/`back_max_gas_bnb_wei`
/// thật): `Config::live_gate_ok` giờ tự đọc thẳng 2 field đó.
pub fn can_send_live(cfg: &Config, halt_locked: bool, version_live: bool, version_pinned: bool) -> bool {
    cfg.live_gate_ok(halt_locked, version_live, version_pinned)
}

// Cụm `exec-path-traps` (F-04, mục a) — CALL SITE DÀNH SẴN cho `7.3` (chưa
// tồn tại phiên này — KHÔNG có `sendRaw`/`Signer` nào trong repo, xem
// doc-comment đầu file). Khi `7.3` nối signer + gửi tx thật xong, điểm gọi
// `RiskGuard::record_result` PHẢI đặt NGAY SAU khi biết kết quả ON-CHAIN THẬT
// của 1 cặp front/back (receipt front+back đều về, biết chắc lãi/lỗ thật
// bằng BNB thật) — VÍ DỤ (giả định chữ ký hàm gửi tx tương lai, KHÔNG PHẢI
// code thật, chỉ đánh dấu vị trí):
//
//   let net_wei = back_receipt_bnb_out - front_receipt_bnb_spent - gas_that;
//   app_state.risk_guard.write().await.record_result(net_wei <= 0);
//
// Paper mode phiên này (mục b) dùng tín hiệu thay thế từ validator nhúng
// (`main.rs::spawn_victim_validator`) vì chưa có giao dịch thật nào để đo lỗ
// thật — xem doc-comment ở đó.

/// Đọc `PRIVATE_KEY` từ biến môi trường `env_key`, validate đúng 32 byte hex
/// (`B256`) — CHỈ ĐỌC/PARSE, KHÔNG `sign_transaction`/`send_raw_transaction`
/// nào ở đây hay bất kỳ đâu trong repo phiên này (để dành `7.2`/`7.3`, xem
/// doc-comment đầu file lý do trả `B256` thay vì `PrivateKeySigner`). Thất
/// bại (biến rỗng/không tồn tại/không phải hex 32 byte hợp lệ) trả `Err` rõ
/// ràng — KHÔNG panic, để tầng gọi (tương lai `7.x`) tự quyết định.
pub fn load_signer(env_key: &str) -> Result<B256, String> {
    let raw = std::env::var(env_key).map_err(|e| format!("{env_key} khong doc duoc tu env: {e}"))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{env_key} rong (chua dien private key that)"));
    }
    B256::from_str(trimmed).map_err(|e| format!("{env_key} khong phai private key hop le (can 32 byte hex): {e}"))
}

// ===== Cụm `7.3` (BAOCAO16) — build + LOG 2 tx paper (front-buy/back-sell) =====
//
// Nối `calldata.rs::encode_front_buy`/`encode_back_sell` (`7.2`, BAOCAO15,
// mới chỉ build `Vec<u8>` thuần) vào 1 điểm gọi thật: khi `pipeline.rs` (qua
// `decide_and_build_paper_v2`, xem `pipeline.rs`) xác nhận 1 candidate có lợi
// nhuận (`PipelineOutcome::Simulated`), hàm ở đây build đủ 2 tx (front-buy +
// back-sell) rồi CHỈ GHI LOG (`logs/bot.jsonl` event `tx.build`) — KHÔNG có
// `Provider`/`Signer` nào xuất hiện trong bất kỳ hàm nào dưới đây, nên KHÔNG
// CÓ ĐƯỜNG NÀO dẫn tới ký/gửi (test `no_send_raw_transaction_call_anywhere_in_src`
// dưới xác nhận cả repo không có lời gọi `send_raw_transaction` nào).
//
// **`to` (địa chỉ nhận token/WBNB) dùng PLACEHOLDER `Address::ZERO`**: `7.1`
// (`load_signer`) chỉ trả `B256` (khoá riêng thô 32 byte) — KHÔNG dẫn xuất
// được địa chỉ public (cần ECDSA point-mul qua `alloy-signer-local`/`k256`,
// chưa bật feature, xem doc-comment đầu file). Dùng địa chỉ 0 là CHỌN CÓ CHỦ
// Ý một placeholder RÕ RÀNG không phải địa chỉ thật (khác "bịa" 1 địa chỉ
// trông giống thật) — mọi log đánh dấu `self_address_placeholder: true`, `7.3`
// sau (khi có signer thật) phải thay bằng địa chỉ ví thật trước khi build tx
// cho live.
//
// **`deadline` = wall-clock hiện tại (Unix epoch giây) + `executor_deadline_buffer_sec`,
// KHÔNG PHẢI `block.timestamp` on-chain thật**: mọi hàm ở đây thuần/sync,
// không nhận `Provider` (đúng tinh thần tách lõi thuần của `pipeline.rs`) nên
// không gọi được `eth_getBlockByNumber` để lấy timestamp thật. Sai lệch giữa
// wall-clock và block.timestamp thật trên BSC (~3s/block) không đáng kể so
// buffer ship mặc định (120s = 40 lần block time) — xem `docs/STATE.md`.

fn v2_router() -> Address {
    Address::from_str(V2_ROUTER_ADDRESS).expect("V2_ROUTER_ADDRESS da pin phai la address hop le")
}

fn wbnb() -> Address {
    Address::from_str(WBNB_ADDRESS).expect("WBNB_ADDRESS da pin phai la address hop le")
}

/// Cụm `exec-path-traps` (F-06) — địa chỉ THẬT nhận token/WBNB của attacker,
/// suy ra từ signer khi có. **HIỆN TẠI (paper mode) LUÔN trả `None`**: `7.1`
/// (`load_signer`) chỉ trả `B256` (khoá riêng thô 32 byte, xem doc-comment
/// đầu file lý do kỹ thuật — `alloy-signer-local` chưa có bản khớp version
/// `alloy 2.4.2` đang pin) — KHÔNG có cách nào suy ra địa chỉ public (cần
/// ECDSA point-mul) mà không bật feature đó. Khi `7.3` nối signer thật
/// (`alloy-signer-local` bắt kịp version hoặc `alloy` được bơm lên bản mới),
/// hàm này đổi sang gọi `signer.address()` — mọi call site khác (`build_and_log_paper_sandwich`)
/// không cần đổi gì, chỉ hàm này đổi.
pub fn executor_self_address() -> Option<Address> {
    None
}

/// `deadline` = giờ hệ thống hiện tại (giây, Unix epoch) + `buffer_sec` — xem
/// giải thích ở khối comment trên vì sao KHÔNG phải `block.timestamp` on-chain
/// thật. `SystemTime::now()` lỗi (đồng hồ hệ thống trước 1970, thực tế không
/// xảy ra) -> fallback `0` thay vì panic (`unwrap_or`).
pub fn compute_deadline(buffer_sec: u64) -> U256 {
    let now_sec = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    U256::from(now_sec.saturating_add(buffer_sec))
}

/// `amount_out_min = expected * (10000 - slippage_bps) / 10000` — `slippage_bps`
/// bị clamp về tối đa `10000` (100%) trước khi tính (không panic với input rác
/// > 100%, giống `combine_roundtrip_bps` ở `tax.rs`). `checked_mul` tự vệ
/// tràn `U256` ở mức lý thuyết (số token cực lớn) — overflow thì fallback trả
/// thẳng `expected` (không slippage), KHÔNG panic; thực tế không xảy ra với số
/// lượng BNB/token trong phạm vi bot này.
pub fn apply_slippage(expected: U256, slippage_bps: u32) -> U256 {
    let bps = slippage_bps.min(10_000);
    let keep_bps = U256::from(10_000 - bps);
    match expected.checked_mul(keep_bps) {
        Some(product) => product / U256::from(10_000u64),
        None => expected,
    }
}

/// 1 tx paper đã build xong — đủ dữ liệu để LOG (`to`/`calldata`/`value`/`gas
/// ước tính`), KHÔNG phải tx đã ký (không có trường `nonce`/`chain_id`/
/// `signature` nào — đó là việc của `7.3` thật khi có `Signer`).
#[derive(Debug, Clone)]
pub struct PaperTxLog {
    pub label: &'static str,
    pub to: Address,
    pub calldata_hex: String,
    pub value_wei: U256,
    pub gas_est_wei: u64,
}

/// Front-run: mua `token` bằng WBNB (`quote.front_in`, nằm trong `msg.value`,
/// KHÔNG trong calldata — đúng ABI `swapExactETHForTokens`, xem `calldata.rs`).
/// `amount_out_min` tính từ `quote.front_out` (số token kỳ vọng mua được) trừ
/// slippage.
pub fn build_front_buy_paper_tx(
    quote: &SandwichQuote,
    token: Address,
    self_addr: Address,
    deadline: U256,
    slippage_bps: u32,
    front_gas_wei: u64,
) -> PaperTxLog {
    let amount_out_min = apply_slippage(quote.front_out, slippage_bps);
    let calldata = encode_front_buy(wbnb(), token, amount_out_min, self_addr, deadline);
    PaperTxLog {
        label: "front_buy",
        to: v2_router(),
        calldata_hex: Bytes::from(calldata).to_string(),
        value_wei: quote.front_in,
        gas_est_wei: front_gas_wei,
    }
}

/// Back-run: bán lại `quote.front_out` token (đúng số front-buy vừa mua, NẰM
/// TRONG calldata — đúng ABI `swapExactTokensForETH`) lấy WBNB. `amount_out_min`
/// tính từ `quote.back_out` (WBNB kỳ vọng nhận lại) trừ slippage.
pub fn build_back_sell_paper_tx(
    quote: &SandwichQuote,
    token: Address,
    self_addr: Address,
    deadline: U256,
    slippage_bps: u32,
    back_gas_wei: u64,
) -> PaperTxLog {
    let amount_out_min = apply_slippage(quote.back_out, slippage_bps);
    let calldata = encode_back_sell(token, wbnb(), quote.front_out, amount_out_min, self_addr, deadline);
    PaperTxLog {
        label: "back_sell",
        to: v2_router(),
        calldata_hex: Bytes::from(calldata).to_string(),
        value_wei: U256::ZERO,
        gas_est_wei: back_gas_wei,
    }
}

/// Điểm gọi DUY NHẤT ghép build + log 2 tx paper. Cổng `cfg.dry_run`: chỉ
/// build/log khi `dry_run=true` (đúng lệnh "chỉ log ra (paper/dry-run)") —
/// `dry_run=false` KHÔNG build gì, chỉ log `tx.build_skipped` rồi dừng, đúng
/// "không có code path nào dẫn tới ký/gửi" (module này vốn dĩ không có
/// `Provider`/`Signer` nên dù có build cũng không gửi được gì, nhưng vẫn chặn
/// tường minh thêm 1 lớp theo đúng lệnh).
///
/// Cụm `exec-path-traps`:
/// - (F-06) — sau cổng `dry_run`, kiểm `executor_self_address()`: `None`
///   hoặc `Some(Address::ZERO)` -> TỪ CHỐI build, log `build.refused
///   {reason:"self_address_zero"}`, trả `None`. Paper mode HIỆN TẠI (chưa nối
///   signer thật) LUÔN rơi vào nhánh này — đúng ý "chưa có signer thì không
///   build" (khác bản cũ dùng `Address::ZERO` làm placeholder ĐI THẲNG vào
///   calldata, đã quan sát thật trong log — nguồn gốc audit F-06).
/// - (F-08) — dùng ĐÚNG `cfg.front_slippage_bps`/`cfg.back_slippage_bps`
///   riêng cho mỗi chân (field gộp chung `executor_slippage_bps` đã XOÁ).
/// - `engine`: `"v2"` (build ngay sau công thức đóng, hành vi cũ) hoặc
///   `"evm"` (build CHỈ SAU khi EVM thật xác nhận — xem `pipeline::build_paper_txs_from_evm_decision`),
///   ghi vào log `tx.build`/`build.refused` để phân biệt nguồn quyết định.
pub fn build_and_log_paper_sandwich(
    logger: &BotLogger,
    cfg: &Config,
    victim_from: Address,
    token: Address,
    quote: &SandwichQuote,
    engine: &str,
) -> Option<(PaperTxLog, PaperTxLog)> {
    if !cfg.dry_run {
        logger.log(
            "tx.build_skipped",
            json!({
                "reason": "dry_run=false: executor 7.3 phien nay CHI build/log paper khi dry_run=true",
                "victim_from": format!("{:#x}", victim_from),
                "engine": engine,
            }),
        );
        return None;
    }

    let self_addr = match executor_self_address() {
        Some(a) if a != Address::ZERO => a,
        _ => {
            logger.log(
                "build.refused",
                json!({
                    "reason": "self_address_zero",
                    "victim_from": format!("{:#x}", victim_from),
                    "token": format!("{:#x}", token),
                    "engine": engine,
                }),
            );
            return None;
        }
    };

    let deadline = compute_deadline(cfg.executor_deadline_buffer_sec);
    let front = build_front_buy_paper_tx(quote, token, self_addr, deadline, cfg.front_slippage_bps, cfg.front_max_gas_bnb_wei);
    let back = build_back_sell_paper_tx(quote, token, self_addr, deadline, cfg.back_slippage_bps, cfg.back_max_gas_bnb_wei);

    logger.log(
        "tx.build",
        json!({
            "victim_from": format!("{:#x}", victim_from),
            "token": format!("{:#x}", token),
            "deadline": deadline.to_string(),
            "self_address_placeholder": false,
            "engine": engine,
            "front": {
                "label": front.label,
                "to": format!("{:#x}", front.to),
                "calldata": front.calldata_hex,
                "value_wei": front.value_wei.to_string(),
                "gas_est_wei": front.gas_est_wei,
            },
            "back": {
                "label": back.label,
                "to": format!("{:#x}", back.to),
                "calldata": back.calldata_hex,
                "value_wei": back.value_wei.to_string(),
                "gas_est_wei": back.gas_est_wei,
            },
        }),
    );

    Some((front, back))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn shipped_config() -> Config {
        Config::from_str(include_str!("../config.toml")).expect("config.toml phai load duoc")
    }

    #[test]
    fn dry_run_blocks_live_even_if_everything_else_is_green() {
        let cfg = shipped_config();
        assert!(cfg.dry_run, "config ship phai dry_run=true");
        assert!(!cfg.allow_live, "config ship phai allow_live=false");
        assert!(!cfg.bot_armed, "config ship phai bot_armed=false");
        // Giả định best-case moi dieu kien khac deu xanh: van phai false.
        assert!(!can_send_live(&cfg, false, true, true));
    }

    #[test]
    fn halt_lock_blocks_live() {
        let mut cfg = shipped_config();
        cfg.dry_run = false;
        cfg.allow_live = true;
        cfg.bot_armed = true;
        assert!(can_send_live(&cfg, false, true, true));
        assert!(!can_send_live(&cfg, true, true, true), "halt.lock phai chan live");
    }

    fn green_config() -> Config {
        let mut cfg = shipped_config();
        cfg.dry_run = false;
        cfg.allow_live = true;
        cfg.bot_armed = true;
        cfg
    }

    /// ĐẠT CẦN DÁN: `gate_check_fail` — `allow_live=false` (config ship gốc,
    /// mọi điều kiện khác giả định xanh) -> `LiveGateStatus { ok: false,
    /// failures: ["allow_live=false"] }` (ĐÚNG 1 lý do, không lẫn lý do khác).
    #[test]
    fn gate_check_fail() {
        let cfg = shipped_config(); // dry_run=true, allow_live=false, bot_armed=false (ship goc)
        let mut only_allow_live_off = green_config();
        only_allow_live_off.allow_live = false;
        let status = only_allow_live_off.gate_check(false, "live_v2", true, true);
        assert_eq!(
            status,
            LiveGateStatus { ok: false, failures: vec!["allow_live=false".to_string()] }
        );
        // Config ship goc (3 co live deu tat) -> nhieu ly do cung luc, khong panic.
        let status_ship = cfg.gate_check(false, "live_v2", cfg.live_v2, true);
        assert!(!status_ship.ok);
        assert!(status_ship.failures.contains(&"allow_live=false".to_string()));
        assert!(status_ship.failures.contains(&"dry_run=true".to_string()));
        assert!(status_ship.failures.contains(&"bot_armed=false".to_string()));
    }

    /// ĐẠT CẦN DÁN: `gate_check_ok` — mọi điều kiện xanh (kể cả gas cap > 0
    /// từ config ship, không halt, `live_v2=true` giả lập, version đã pin) ->
    /// `ok=true`, `failures` rỗng.
    #[test]
    fn gate_check_ok() {
        let cfg = green_config();
        assert!(cfg.front_max_gas_bnb_wei > 0 && cfg.back_max_gas_bnb_wei > 0, "config ship phai co gas cap > 0");
        let status = cfg.gate_check(false, "live_v2", true, true);
        assert_eq!(status, LiveGateStatus { ok: true, failures: vec![] });
    }

    #[test]
    fn gate_check_zero_gas_cap_is_a_failure_reason() {
        let mut cfg = green_config();
        cfg.front_max_gas_bnb_wei = 0;
        let status = cfg.gate_check(false, "live_v2", true, true);
        assert!(!status.ok);
        assert!(status.failures.contains(&"front_max_gas_bnb_wei=0".to_string()));
    }

    /// Cụm `exec-path-traps` (F-05) — `version_pinned=false` (mà mọi điều
    /// kiện khác xanh, kể cả `live_v2=true`) PHẢI là 1 lý do fail riêng —
    /// audit gốc: bản `executor::gate_check` CŨ thiếu hẳn check này.
    #[test]
    fn gate_check_version_not_pinned_is_a_failure_reason() {
        let cfg = green_config();
        let status = cfg.gate_check(false, "live_v2", true, false);
        assert!(!status.ok);
        assert!(status.failures.contains(&"live_v2_pinned=false".to_string()));
    }

    #[test]
    fn load_signer_missing_env_var_is_err_no_panic() {
        let key = "BSC_SANDWICH_TEST_PRIVATE_KEY_MISSING_VAR";
        std::env::remove_var(key);
        let err = load_signer(key).unwrap_err();
        assert!(err.contains("khong doc duoc tu env"));
    }

    #[test]
    fn load_signer_empty_env_var_is_err_no_panic() {
        let key = "BSC_SANDWICH_TEST_PRIVATE_KEY_EMPTY_VAR";
        std::env::set_var(key, "");
        let err = load_signer(key).unwrap_err();
        std::env::remove_var(key);
        assert!(err.contains("rong"));
    }

    /// Private key giả lập hợp lệ (32 byte hex, KHÔNG PHẢI key thật/không
    /// dùng cho ví thật nào) — chỉ verify `load_signer` PARSE đúng, KHÔNG ký/
    /// gửi gì (không có hàm sign/send nào tồn tại trong repo phiên này).
    #[test]
    fn load_signer_valid_hex_key_parses_ok() {
        let key = "BSC_SANDWICH_TEST_PRIVATE_KEY_VALID_VAR";
        // Xay chuoi hex 64 ky tu (32 byte) = 63 so "0" + "1" bang code, tranh
        // dem tay sai do dai chuoi hardcode (gia tri = 1, khoa hop le ve mat
        // ky thuat secp256k1, KHONG PHAI khoa/vi that nao).
        let fake_key_hex = format!("0x{}1", "0".repeat(63));
        std::env::set_var(key, &fake_key_hex);
        let signer = load_signer(key);
        std::env::remove_var(key);
        assert!(signer.is_ok(), "hex private key hop le 32 byte phai parse duoc: {signer:?}");
    }

    #[test]
    fn load_signer_garbage_hex_is_err_no_panic() {
        let key = "BSC_SANDWICH_TEST_PRIVATE_KEY_GARBAGE_VAR";
        std::env::set_var(key, "not_a_private_key");
        let err = load_signer(key).unwrap_err();
        std::env::remove_var(key);
        assert!(err.contains("khong phai private key hop le"));
    }

    // ===== Cụm `7.3` (BAOCAO16) — build + log tx paper =====

    /// Chỉ dùng trong test (F-06 đã xoá khỏi đường build production) — gọi
    /// thẳng `build_front_buy_paper_tx`/`build_back_sell_paper_tx` (hàm calldata
    /// thuần, KHÔNG qua cổng `executor_self_address`) vẫn cần 1 địa chỉ `to`
    /// bất kỳ để kiểm cấu trúc calldata.
    const TEST_PLACEHOLDER_SELF_ADDRESS: Address = Address::ZERO;

    fn test_token() -> Address {
        Address::from_str("0xcccccccccccccccccccccccccccccccccccccccc").unwrap()
    }

    fn test_victim() -> Address {
        Address::from_str("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap()
    }

    /// Số liệu KHÔNG phải kết quả sim thật (`sim_v2::search_max_front_in`) —
    /// chỉ là fixture tay nội bộ, đủ dùng để kiểm tra build calldata/slippage
    /// đúng cấu trúc, không claim đây là 1 quote thật trên chain.
    fn fixture_quote() -> SandwichQuote {
        SandwichQuote {
            front_in: U256::from(50_000_000_000_000_000u64), // 0.05 BNB
            front_out: U256::from(45_000_000_000_000_000_000u128), // 45 token (18dp)
            victim_out: U256::from(39_000_000_000_000_000_000u128),
            back_out: U256::from(57_000_000_000_000_000u64), // 0.057 BNB
            profit_wei: 7_000_000_000_000_000i128,
        }
    }

    #[test]
    fn apply_slippage_zero_bps_keeps_full_amount() {
        assert_eq!(apply_slippage(U256::from(1000u64), 0), U256::from(1000u64));
    }

    #[test]
    fn apply_slippage_100_bps_is_1_percent_off() {
        // 1000 * (10000-100)/10000 = 1000*9900/10000 = 990.
        assert_eq!(apply_slippage(U256::from(1000u64), 100), U256::from(990u64));
    }

    #[test]
    fn apply_slippage_10000_bps_is_zero_no_panic() {
        assert_eq!(apply_slippage(U256::from(1000u64), 10_000), U256::ZERO);
    }

    #[test]
    fn apply_slippage_over_10000_bps_clamped_no_panic() {
        // >100% bi clamp ve 10000 (0 con lai), khong tran/khong panic voi input rac.
        assert_eq!(apply_slippage(U256::from(1000u64), 50_000), U256::ZERO);
    }

    #[test]
    fn compute_deadline_is_now_plus_buffer_within_tolerance() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let deadline = compute_deadline(120);
        let deadline_u64: u64 = deadline.try_into().unwrap();
        // Dung sai vai giay cho thoi gian chay test that su (khong flaky).
        assert!(deadline_u64 >= now + 118, "deadline phai >= now+118, got {deadline_u64} (now={now})");
        assert!(deadline_u64 <= now + 130, "deadline phai <= now+130, got {deadline_u64} (now={now})");
    }

    /// ĐẠT CẦN DÁN: front-buy build ra calldata decode lại ĐÚNG bit-for-bit
    /// bằng chính `decoder::decode_swap_calldata` (giống roundtrip `calldata.rs`
    /// `7.2`, BAOCAO15) — chứng minh `amount_out_min` tính từ slippage đúng,
    /// `to` = placeholder, `deadline` truyền đúng.
    #[test]
    fn build_front_buy_paper_tx_roundtrips_through_decoder() {
        let token = test_token();
        let quote = fixture_quote();
        let deadline = U256::from(9_999_999_999u64);
        let front = build_front_buy_paper_tx(&quote, token, TEST_PLACEHOLDER_SELF_ADDRESS, deadline, 50, 3_000_000_000_000_000);
        assert_eq!(front.label, "front_buy");
        assert_eq!(front.to, v2_router());
        assert_eq!(front.value_wei, quote.front_in);
        assert_eq!(front.gas_est_wei, 3_000_000_000_000_000);

        let calldata_bytes = Bytes::from_str(&front.calldata_hex).unwrap().to_vec();
        let decoded = crate::decoder::decode_swap_calldata(&calldata_bytes, front.value_wei).expect("phai decode duoc calldata vua build");
        assert_eq!(decoded.selector_name, "swapExactETHForTokens");
        assert_eq!(decoded.amount_in, quote.front_in);
        assert_eq!(decoded.path.token_a, wbnb());
        assert_eq!(decoded.path.token_b, token);
        assert_eq!(decoded.to, TEST_PLACEHOLDER_SELF_ADDRESS);
        assert_eq!(decoded.deadline, Some(deadline));
        assert_eq!(decoded.amount_out_min, apply_slippage(quote.front_out, 50));
    }

    /// Back-sell: `amountIn` calldata PHẢI đúng `quote.front_out` (số token
    /// front-buy vừa mua, không phải `quote.back_out`/`victim_out`).
    #[test]
    fn build_back_sell_paper_tx_roundtrips_through_decoder() {
        let token = test_token();
        let quote = fixture_quote();
        let deadline = U256::from(9_999_999_999u64);
        let back = build_back_sell_paper_tx(&quote, token, TEST_PLACEHOLDER_SELF_ADDRESS, deadline, 50, 3_000_000_000_000_000);
        assert_eq!(back.label, "back_sell");
        assert_eq!(back.value_wei, U256::ZERO, "swapExactTokensForETH khong payable");

        let calldata_bytes = Bytes::from_str(&back.calldata_hex).unwrap().to_vec();
        let decoded = crate::decoder::decode_swap_calldata(&calldata_bytes, U256::ZERO).expect("phai decode duoc calldata vua build");
        assert_eq!(decoded.selector_name, "swapExactTokensForETH");
        assert_eq!(decoded.amount_in, quote.front_out, "amountIn back-sell phai la front_out (token vua mua)");
        assert_eq!(decoded.path.token_a, token);
        assert_eq!(decoded.path.token_b, wbnb());
        assert_eq!(decoded.to, TEST_PLACEHOLDER_SELF_ADDRESS);
        assert_eq!(decoded.amount_out_min, apply_slippage(quote.back_out, 50));
    }

    /// Cụm `exec-path-traps` (F-06) — sửa lại theo hành vi MỚI: KHÔNG còn
    /// build được khi `dry_run=true` (khác bản cũ, tên test đổi để phản ánh
    /// đúng) vì `executor_self_address()` LUÔN `None` ở paper mode (chưa nối
    /// signer thật) — `build_and_log_paper_sandwich` giờ TỪ CHỐI build, log
    /// `build.refused{reason:"self_address_zero"}` thay vì `tx.build` với
    /// `to=Address::ZERO` (đúng audit F-06: KHÔNG được để calldata thật mang
    /// `to=0x000...000`). Đây là thay đổi hành vi CÓ CHỦ Ý theo đúng mục 2
    /// của lệnh, không phải nới test để né fail.
    #[test]
    fn build_and_log_paper_sandwich_refuses_when_no_real_self_address() {
        let dir = tempfile::tempdir().unwrap();
        let logger = BotLogger::new(dir.path().join("logs").join("bot.jsonl")).unwrap();
        let cfg = shipped_config();
        assert!(cfg.dry_run, "config ship phai dry_run=true");
        assert_eq!(executor_self_address(), None, "paper mode chua noi signer that, phai None");
        let quote = fixture_quote();

        let result = build_and_log_paper_sandwich(&logger, &cfg, test_victim(), test_token(), &quote, "v2");
        assert!(result.is_none(), "khong co self address that thi khong duoc build");

        let tail = logger.tail(10);
        assert!(tail.iter().all(|v| v["event"] != "tx.build"), "khong co self address that khong duoc log tx.build");
        let refused: Vec<_> = tail.iter().filter(|v| v["event"] == "build.refused").collect();
        assert_eq!(refused.len(), 1, "phai co dung 1 dong build.refused");
        assert_eq!(refused[0]["reason"], "self_address_zero");
        assert_eq!(refused[0]["engine"], "v2");
        // Bang chung dong log THAT (cum exec-path-traps) - chay voi --nocapture.
        println!("build.refused log THAT: {}", serde_json::to_string(refused[0]).unwrap());
    }

    /// ĐẠT CẦN DÁN: `dry_run=false` -> KHÔNG build gì (`None`), chỉ log
    /// `tx.build_skipped`, KHÔNG có `tx.build` nào — đúng lệnh "chỉ log ra
    /// (paper/dry-run)". Cổng `dry_run` chạy TRƯỚC cổng self-address nên vẫn
    /// đúng dù `executor_self_address()` luôn `None`.
    #[test]
    fn build_and_log_paper_sandwich_skips_when_dry_run_false() {
        let dir = tempfile::tempdir().unwrap();
        let logger = BotLogger::new(dir.path().join("logs").join("bot.jsonl")).unwrap();
        let mut cfg = shipped_config();
        cfg.dry_run = false;
        let quote = fixture_quote();

        let result = build_and_log_paper_sandwich(&logger, &cfg, test_victim(), test_token(), &quote, "evm");
        assert!(result.is_none());

        let tail = logger.tail(10);
        assert!(tail.iter().all(|v| v["event"] != "tx.build"), "dry_run=false khong duoc log tx.build");
        assert!(tail.iter().any(|v| v["event"] == "tx.build_skipped"));
        assert!(tail.iter().all(|v| v["event"] != "build.refused"), "dry_run=false dung o cong dry_run, chua toi cong self-address");
    }

    /// ĐẠT CẦN DÁN: grep TOÀN BỘ `src/` xác nhận KHÔNG có bất kỳ lời gọi hàm
    /// `send_raw_transaction`/`sendRawTransaction(...)` nào tồn tại trong repo
    /// (kể cả `#[cfg(test)]`) — chứng minh "không có code path nào dẫn tới
    /// ký/gửi" ở MỨC TOÀN REPO, không chỉ riêng module `executor.rs`. Loại
    /// trừ dòng comment (bắt đầu `//`/`///`/`//!`) vì file này (và
    /// `docs/STATE.md`) có NHẮC TÊN hàm đó trong lời giải thích, không phải
    /// lời gọi thật — chỉ coi là vi phạm khi có dấu `(` NGAY sau tên hàm
    /// (đúng cú pháp gọi hàm) trên dòng KHÔNG PHẢI comment.
    #[test]
    fn no_send_raw_transaction_call_anywhere_in_src() {
        // Ghep chuoi tim kiem tu 2 manh TAI RUNTIME (khong xuat hien lien tuc
        // trong source) — de chinh dong code check nay khong tu bao no la
        // "vi pham" (chuoi "send_raw_transaction(" lien tuc chi ton tai o day
        // qua concat runtime, khong phai literal tinh trong file .rs nao).
        let needle_snake: String = format!("{}{}", "send_raw_transaction", "(");
        let needle_camel: String = format!("{}{}", "sendrawtransaction", "(");

        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stack = vec![src_dir];
        let mut offending = Vec::new();
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("doc duoc thu muc src") {
                let entry = entry.expect("doc duoc entry trong thu muc src");
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().map(|e| e == "rs").unwrap_or(false) {
                    let content = std::fs::read_to_string(&path).unwrap_or_default();
                    for (i, line) in content.lines().enumerate() {
                        let trimmed = line.trim_start();
                        let is_comment_line = trimmed.starts_with("//");
                        let lower = line.to_lowercase();
                        let looks_like_call = lower.contains(&needle_snake) || lower.contains(&needle_camel);
                        if looks_like_call && !is_comment_line {
                            offending.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
                        }
                    }
                }
            }
        }
        assert!(offending.is_empty(), "tim thay loi GOI send_raw_transaction trong code (cam tuyet doi phien nay): {offending:?}");
    }
}

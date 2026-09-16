//! Cụm `competitor-recon-and-strategy` (mục 4, shadow mode) — KÝ THẬT tx
//! front-buy/back-sell bằng `PRIVATE_KEY` (.env, ví Chủ tự điền — "ví
//! trắng"), dùng nonce/gas/bribe THẬT, nhưng KHÔNG BAO GIỜ broadcast/gửi đi
//! đâu (không có `eth_sendRawTransaction`/`send_raw_transaction` nào trong
//! file này hay bất kỳ đâu trong repo — xem test
//! `executor::tests::no_send_raw_transaction_call_anywhere_in_src`, quét
//! TOÀN `src/`, tự động bắt module này nếu ai đó thêm lời gọi gửi tx thật).
//!
//! Bật CHỈ khi `config.toml::live_mode = "shadow"` (ship `"off"` — không load
//! signer, không ký gì, hành vi giống 100% các cụm trước). `"live"` (gửi tx
//! thật) CHƯA implement — đọc `live_mode="live"` ở đây KHÔNG tự mở khoá gửi,
//! cổng gửi thật DUY NHẤT vẫn là `executor::can_send_live`/`Config::gate_check`
//! (chưa có hàm gửi nào tồn tại, xem `docs/STATE.md` mục `7.3`).
//!
//! ## Vì sao dùng `alloy_signer_local::PrivateKeySigner` (khác `executor::load_signer`)
//!
//! `executor::load_signer` (7.1, BAOCAO15) trả `B256` thô vì lúc đó
//! `alloy-signer-local` CHƯA có bản khớp `alloy 2.4.2` đang pin (xem
//! doc-comment `executor.rs`). Cụm này (2026-09-16) xác nhận
//! `alloy-signer-local = "2.4.2"` NAY ĐÃ CÓ trên crates.io — thêm dependency
//! thật (`Cargo.toml`), dùng `PrivateKeySigner` đầy đủ (ký ECDSA thật, tự suy
//! địa chỉ public). `executor::load_signer`/`executor_self_address` GIỮ
//! NGUYÊN (không đổi — đường paper-mode hiện có vẫn `None`/từ chối build,
//! đúng F-06), module này là ĐƯỜNG THỨ 2, độc lập, chỉ chạy khi
//! `live_mode="shadow"`.
//!
//! ## Không có endpoint simulate ở 2 relay (xác nhận THẬT, không đoán)
//!
//! Thử `eth_callBundle` (kiểu Flashbots) trên CẢ 2 relay đã pin
//! (`relay::CLUB48_RPC_URL`/`BLOCKRAZOR_RPC_URL`) qua `curl` thật
//! (2026-09-16): cả 2 trả `-32601 method eth_callBundle does not exist/is
//! not available` — GIỐNG HỆT lỗi method-not-found đã thấy ở
//! `relay-schema-verify` (BAOCAO23) cho `eth_sendBundle` trước khi tìm đúng
//! endpoint. Không có nguồn chính thức nào (docs 48 Club/BlockRazor đã đọc)
//! liệt kê method simulate riêng — kết luận: 2 relay này KHÔNG có endpoint
//! simulate công khai tại thời điểm cụm này. Theo đúng lệnh ("không có
//! endpoint sim → log bundle đã ký (hash), không gửi"), `build_shadow_bundle`
//! dưới đây CHỈ log — không gọi HTTP nào (dùng lại đúng charter "relay.rs
//! không network", xem `relay.rs`).

use alloy::consensus::TxEnvelope;
use alloy::eips::eip2718::Encodable2718;
use alloy::network::{EthereumWallet, NetworkTransactionBuilder, TransactionBuilder};
use alloy::primitives::{Address, Bytes, B256, U256};

use alloy::rpc::types::eth::TransactionRequest;
use alloy::signers::Signer as _;
use alloy_signer_local::PrivateKeySigner;
use std::str::FromStr;

use crate::logger::BotLogger;

/// Đọc `PRIVATE_KEY` từ biến môi trường, dựng `PrivateKeySigner` đầy đủ (khác
/// `executor::load_signer` chỉ trả `B256` thô). Lỗi rõ ràng (biến rỗng/không
/// tồn tại/không phải hex 32 byte hợp lệ) — KHÔNG panic.
pub fn load_shadow_signer(env_key: &str, chain_id: u64) -> Result<PrivateKeySigner, String> {
    let raw = std::env::var(env_key).map_err(|e| format!("{env_key} khong doc duoc tu env: {e}"))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{env_key} rong (chua dien private key that — can vi TRANG rieng cho shadow mode)"));
    }
    let mut signer = PrivateKeySigner::from_str(trimmed)
        .map_err(|e| format!("{env_key} khong phai private key hop le (can 32 byte hex): {e}"))?;
    signer.set_chain_id(Some(chain_id));
    Ok(signer)
}

pub fn self_address(signer: &PrivateKeySigner) -> Address {
    alloy::signers::Signer::address(signer)
}

/// 1 chân (front-buy hoặc back-sell) ĐÃ KÝ THẬT — `raw_hex`/`hash` là dữ liệu
/// PUBLIC-SAFE (chữ ký không lộ private key), an toàn để log/hiển thị.
#[derive(Debug, Clone)]
pub struct ShadowSignedLeg {
    pub label: &'static str,
    pub raw_hex: String,
    pub hash: B256,
    pub nonce: u64,
    pub gas_limit: u64,
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
}

fn to_hex(bytes: &[u8]) -> String {
    format!("0x{}", bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())
}

/// Ký 1 tx EIP-1559 (type 2, đã bật trên BSC) THẬT bằng `wallet` — KHÔNG gửi
/// đi đâu (không có `Provider::send_transaction`/`send_raw_transaction` nào
/// gọi ở đây). `max_priority_fee_per_gas` mang bribe MÔ PHỎNG (F-02,
/// `bribe_mode="gaspriority"`) hoặc `0` khi `bribe_mode="builder_transfer"`
/// (bribe kiểu transfer KHÔNG đi qua priority fee — nó là 1 lệnh chuyển BNB
/// tới VÍ EOA CỦA BUILDER đặt trong CHÂN BACK, cần contract executor mới
/// thực hiện được nguyên tử "kiểm lãi xong mới trả bribe" — xem
/// `docs/CONTRACT_DESIGN.md` mục B2/B3, CHƯA implement trong shadow mode).
#[allow(clippy::too_many_arguments)]
pub async fn sign_leg(
    wallet: &EthereumWallet,
    from: Address,
    to: Address,
    value: U256,
    calldata: Vec<u8>,
    nonce: u64,
    chain_id: u64,
    gas_limit: u64,
    max_fee_per_gas: u128,
    max_priority_fee_per_gas: u128,
    label: &'static str,
) -> Result<ShadowSignedLeg, String> {
    let tx_req = TransactionRequest::default()
        .with_chain_id(chain_id)
        .with_from(from)
        .with_to(to)
        .with_value(value)
        .with_input(Bytes::from(calldata))
        .with_nonce(nonce)
        .with_gas_limit(gas_limit)
        .with_max_fee_per_gas(max_fee_per_gas)
        .with_max_priority_fee_per_gas(max_priority_fee_per_gas);

    let envelope: TxEnvelope = tx_req.build(wallet).await.map_err(|e| format!("ky tx that bai ({label}): {e}"))?;
    let raw = envelope.encoded_2718();
    let hash = *envelope.tx_hash();
    Ok(ShadowSignedLeg {
        label,
        raw_hex: to_hex(&raw),
        hash,
        nonce,
        gas_limit,
        max_fee_per_gas,
        max_priority_fee_per_gas,
    })
}

/// Kết quả pre-sign re-vet — cụm `bugfix-presign-and-contract-plan` (A4) đã
/// THAY HẲN nội dung 4 kiểm tra (xem `pre_sign_revet_fast`): không còn đo tax
/// bằng fork EVM, không còn gọi `eth_getTransactionReceipt`.
#[derive(Debug, Clone)]
pub struct PreSignRevetResult {
    /// (a) token đã vet (`PairBook::is_tax_ok`) VÀ kết quả vet nền còn TƯƠI
    /// (`last_vet <= pairs_vet_interval_sec * 2`).
    pub vet_fresh: bool,
    /// Tuổi kết quả vet nền (giây); `None` = CHƯA từng vet nền lần nào.
    pub vet_age_sec: Option<u64>,
    /// (b) reserve lấy từ cache Sync-event ĐÚNG block hiện tại và ≥ `thin_liq`.
    pub reserve_still_ok: bool,
    /// (c) victim vẫn CHƯA thấy trong block nào bot đã biết (`MinedTxIndex`).
    pub victim_pending_confirmed: bool,
    /// (d) nonce ví bot lấy được từ cache prefetch (không gọi RPC).
    pub nonce_ready: bool,
    /// Lý do ABORT đầu tiên gặp phải (`None` = qua hết 4 cổng).
    pub abort_reason: Option<&'static str>,
}

impl PreSignRevetResult {
    pub fn all_ok(&self) -> bool {
        self.abort_reason.is_none()
    }
}

/// Cụm `bugfix-presign-and-contract-plan` (A4) — RE-VET TRƯỚC KHI KÝ,
/// **THUẦN, 0 RPC, 0 fork EVM**.
///
/// # Vì sao bỏ `measure_tax_evm` khỏi đường ký (bằng chứng BAOCAO41)
///
/// Bản trước gọi `measure_tax_evm` (fork `revm` + `AlloyDB`, cold-fetch state
/// qua RPC) NGAY TRÊN đường ký, rồi mới hỏi `eth_getTransactionReceipt` xem
/// victim còn pending không. Kết quả đo THẬT 30 phút: **0/36 lần ký kịp** —
/// 34/36 abort vì `victim_pending_confirmed=false`, tức victim ĐÃ LÊN BLOCK
/// trước khi fork chạy xong (block BSC ~3 s, fork lạnh lâu hơn thế). Cổng an
/// toàn hoạt động đúng, nhưng kiến trúc không kịp thời gian thực.
///
/// Thay thế bằng 4 cổng đọc TOÀN BỘ từ dữ liệu bot đã có sẵn trong bộ nhớ:
///
/// - (a) `vet_ok` (token có trong `pairs.txt`, đã vet tay, chưa bị vet nền
///   loại) + `vet_age_sec <= pairs_vet_interval_sec * 2` — cũ hơn thì
///   `vet_stale` (thà bỏ cơ hội còn hơn ký theo số đo quá hạn).
/// - (b) reserve từ cache Sync-event, **đúng block hiện tại**, ≥ ngưỡng
///   `thin_liq` — cache lệch block -> `reserve_stale`.
/// - (c) victim chưa nằm trong `MinedTxIndex` (3 block gần nhất bot đã đọc).
///   Chưa nạp được block nào -> KHÔNG đoán, trả `mined_index_cold`.
/// - (d) nonce ví bot có trong cache prefetch và không quá cũ.
///
/// `vet_result_age_sec = None` nghĩa là chưa có lần vet nền nào -> `vet_stale`
/// (không suy diễn "chắc vẫn sạch").
#[allow(clippy::too_many_arguments)]
pub fn pre_sign_revet_fast(
    vet_ok: bool,
    vet_age_sec: Option<u64>,
    vet_max_age_sec: u64,
    reserve_quote_wei: Option<U256>,
    reserve_cache_block: Option<u64>,
    current_block: u64,
    min_reserve_wei: U256,
    victim_seen_in_block: bool,
    mined_index_blocks_loaded: usize,
    nonce_age_blocks: Option<u64>,
    nonce_max_age_blocks: u64,
) -> PreSignRevetResult {
    let vet_fresh = vet_ok && vet_age_sec.map(|a| a <= vet_max_age_sec).unwrap_or(false);
    let reserve_still_ok =
        matches!((reserve_quote_wei, reserve_cache_block), (Some(r), Some(b)) if b == current_block && r >= min_reserve_wei);
    let victim_pending_confirmed = mined_index_blocks_loaded > 0 && !victim_seen_in_block;
    let nonce_ready = nonce_age_blocks.map(|a| a <= nonce_max_age_blocks).unwrap_or(false);

    let abort_reason = if !vet_fresh {
        Some("vet_stale")
    } else if !reserve_still_ok {
        Some("reserve_stale")
    } else if mined_index_blocks_loaded == 0 {
        Some("mined_index_cold")
    } else if !victim_pending_confirmed {
        Some("victim_already_mined")
    } else if !nonce_ready {
        Some("nonce_not_prefetched")
    } else {
        None
    };

    PreSignRevetResult { vet_fresh, vet_age_sec, reserve_still_ok, victim_pending_confirmed, nonce_ready, abort_reason }
}

/// Điểm gọi DUY NHẤT: ký front+back (nếu pre-sign re-vet OK), log
/// `bundle.shadow` — KHÔNG relay simulate (2 relay không có endpoint, xem
/// doc-comment module), KHÔNG gửi. Trả `None` + log `tx.abort` khi re-vet
/// fail (lý do cụ thể trong log, không bịa).
#[allow(clippy::too_many_arguments)]
pub async fn build_and_log_shadow_bundle(
    logger: &BotLogger,
    wallet: &EthereumWallet,
    self_addr: Address,
    chain_id: u64,
    revet: &PreSignRevetResult,
    front: (Address, U256, Vec<u8>, u64, u128, u128, u64), // to,value,calldata,nonce,max_fee,max_priority,gas_limit
    back: (Address, U256, Vec<u8>, u64, u128, u128, u64),
    victim_hash: B256,
    token: Address,
    presign_ms: serde_json::Value,
) -> Option<(ShadowSignedLeg, ShadowSignedLeg)> {
    if !revet.all_ok() {
        logger.log(
            "tx.abort",
            serde_json::json!({
                // Cụm A4 — lý do CỤ THỂ (vet_stale/reserve_stale/
                // mined_index_cold/victim_already_mined/nonce_not_prefetched)
                // thay cho 1 chữ `pre_sign_revet_failed` chung chung.
                "reason": revet.abort_reason.unwrap_or("pre_sign_revet_failed"),
                "victim_hash": format!("{victim_hash:#x}"),
                "token": format!("{token:#x}"),
                "vet_fresh": revet.vet_fresh,
                "vet_age_sec": revet.vet_age_sec,
                "reserve_still_ok": revet.reserve_still_ok,
                "victim_pending_confirmed": revet.victim_pending_confirmed,
                "nonce_ready": revet.nonce_ready,
                "presign_ms": presign_ms,
            }),
        );
        return None;
    }

    let (f_to, f_val, f_data, f_nonce, f_max_fee, f_prio, f_gas) = front;
    let (b_to, b_val, b_data, b_nonce, b_max_fee, b_prio, b_gas) = back;

    let front_signed = match sign_leg(wallet, self_addr, f_to, f_val, f_data, f_nonce, chain_id, f_gas, f_max_fee, f_prio, "front_buy").await {
        Ok(l) => l,
        Err(e) => {
            logger.log("tx.abort", serde_json::json!({"reason": "sign_failed_front", "detail": e, "victim_hash": format!("{victim_hash:#x}")}));
            return None;
        }
    };
    let back_signed = match sign_leg(wallet, self_addr, b_to, b_val, b_data, b_nonce, chain_id, b_gas, b_max_fee, b_prio, "back_sell").await {
        Ok(l) => l,
        Err(e) => {
            logger.log("tx.abort", serde_json::json!({"reason": "sign_failed_back", "detail": e, "victim_hash": format!("{victim_hash:#x}")}));
            return None;
        }
    };

    logger.log(
        "bundle.shadow",
        serde_json::json!({
            "note": "SHADOW MODE - da KY THAT bang PRIVATE_KEY that, KHONG gui/broadcast (khong co Provider::send_transaction nao trong duong nay)",
            "self_address": format!("{self_addr:#x}"),
            "victim_hash": format!("{victim_hash:#x}"),
            "token": format!("{token:#x}"),
            // Cụm A4 — thời gian TỪNG BƯỚC của đường ký (mục tiêu p95 < 20 ms).
            "presign_ms": presign_ms,
            "front": {
                "label": front_signed.label, "hash": format!("{:#x}", front_signed.hash), "nonce": front_signed.nonce,
                "gas_limit": front_signed.gas_limit, "max_fee_per_gas_gwei": front_signed.max_fee_per_gas as f64 / 1e9,
                "max_priority_fee_per_gas_gwei": front_signed.max_priority_fee_per_gas as f64 / 1e9,
                "raw_hex": front_signed.raw_hex,
            },
            "back": {
                "label": back_signed.label, "hash": format!("{:#x}", back_signed.hash), "nonce": back_signed.nonce,
                "gas_limit": back_signed.gas_limit, "max_fee_per_gas_gwei": back_signed.max_fee_per_gas as f64 / 1e9,
                "max_priority_fee_per_gas_gwei": back_signed.max_priority_fee_per_gas as f64 / 1e9,
                "raw_hex": back_signed.raw_hex,
            },
            "relay_simulate": {
                "club48": "MISSING - eth_callBundle khong ton tai tren endpoint da pin (xac nhan cURL that 2026-09-16, -32601)",
                "blockrazor": "MISSING - eth_callBundle khong ton tai tren endpoint da pin (xac nhan cURL that 2026-09-16, -32601)",
            },
        }),
    );

    Some((front_signed, back_signed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_logger() -> (tempfile::TempDir, BotLogger) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("logs").join("bot.jsonl");
        let logger = BotLogger::new(&path).unwrap();
        (dir, logger)
    }

    // Private key GIẢ LẬP hợp lệ về mặt kỹ thuật secp256k1 (KHÔNG PHẢI ví
    // thật, KHÔNG dùng cho bất kỳ mục đích nào ngoài test) — cùng khuôn
    // `executor::tests::load_signer_valid_hex_key_parses_ok`.
    fn fake_key_hex() -> String {
        format!("0x{}1", "0".repeat(63))
    }

    #[test]
    fn load_shadow_signer_missing_env_is_err_no_panic() {
        let key = "BSC_SANDWICH_TEST_SHADOW_KEY_MISSING";
        std::env::remove_var(key);
        let err = load_shadow_signer(key, 56).unwrap_err();
        assert!(err.contains("khong doc duoc"));
    }

    #[test]
    fn load_shadow_signer_empty_is_err_no_panic() {
        let key = "BSC_SANDWICH_TEST_SHADOW_KEY_EMPTY";
        std::env::set_var(key, "");
        let err = load_shadow_signer(key, 56).unwrap_err();
        std::env::remove_var(key);
        assert!(err.contains("rong"));
    }

    #[test]
    fn load_shadow_signer_garbage_is_err_no_panic() {
        let key = "BSC_SANDWICH_TEST_SHADOW_KEY_GARBAGE";
        std::env::set_var(key, "not_a_key");
        let err = load_shadow_signer(key, 56).unwrap_err();
        std::env::remove_var(key);
        assert!(err.contains("khong phai private key hop le"));
    }

    /// ĐẠT CẦN DÁN — khác `executor::load_signer` (chỉ trả `B256` thô),
    /// `load_shadow_signer` PHẢI tự suy ra được địa chỉ public THẬT (chứng
    /// minh `alloy-signer-local` hoạt động đúng, không chỉ parse hex).
    #[test]
    fn load_shadow_signer_valid_key_derives_real_address() {
        let key = "BSC_SANDWICH_TEST_SHADOW_KEY_VALID";
        std::env::set_var(key, fake_key_hex());
        let signer = load_shadow_signer(key, 56).expect("key hop le phai load duoc");
        std::env::remove_var(key);
        let addr = self_address(&signer);
        assert_ne!(addr, Address::ZERO, "phai suy ra dia chi THAT, khong phai placeholder zero (khac F-06 cua paper mode)");
        println!("dia chi suy tu fake key (KHONG PHAI vi that): {addr:#x}");
    }

    #[tokio::test]
    async fn sign_leg_produces_valid_envelope_with_matching_nonce_and_verifiable_hash() {
        let key = "BSC_SANDWICH_TEST_SHADOW_KEY_SIGN";
        std::env::set_var(key, fake_key_hex());
        let signer = load_shadow_signer(key, 56).unwrap();
        std::env::remove_var(key);
        let self_addr = self_address(&signer);
        let wallet = EthereumWallet::from(signer);

        let to = Address::from_str("0x10ED43C718714eb63d5aA57B78B54704E256024E").unwrap(); // V2 router
        let leg = sign_leg(&wallet, self_addr, to, U256::from(1u64), vec![0x12, 0x34], 7, 56, 300_000, 5_000_000_000, 1_000_000_000, "front_buy")
            .await
            .expect("ky phai thanh cong");
        assert_eq!(leg.nonce, 7);
        assert_eq!(leg.label, "front_buy");
        assert!(leg.raw_hex.starts_with("0x"));
        // raw tx type 2 (EIP-1559) bat dau bang byte 0x02 (EIP-2718 envelope prefix).
        assert!(leg.raw_hex.starts_with("0x02"), "BSC da bat EIP-1559, tx phai la type 2: {}", leg.raw_hex);
        // keccak(raw) phai khop hash tra ve (tu-nhat-quan, dung lai logic da verify o transport::fetch_raw_tx_verified).
        let raw_bytes = alloy::primitives::Bytes::from_str(&leg.raw_hex).unwrap();
        let computed = alloy::primitives::keccak256(raw_bytes.as_ref());
        assert_eq!(computed, leg.hash, "keccak256(raw) phai khop tx_hash tra ve tu envelope");
    }

    fn revet_all_ok() -> PreSignRevetResult {
        pre_sign_revet_fast(
            true,                    // vet_ok
            Some(10),                // vet_age_sec
            1200,                    // vet_max_age_sec (= pairs_vet_interval_sec 600 x2)
            Some(U256::from(100u64)),// reserve
            Some(50),                // reserve cache block
            50,                      // current block
            U256::from(20u64),       // min_reserve
            false,                   // victim chua thay trong block nao
            3,                       // mined index da nap 3 block
            Some(0),                 // nonce prefetch cung block
            3,                       // nonce_max_age_blocks
        )
    }

    /// ĐẠT CẦN DÁN (A4) — 5 lý do abort, mỗi lý do test RIÊNG, và thứ tự ưu
    /// tiên phải ổn định (vet -> reserve -> mined_index -> victim -> nonce).
    #[test]
    fn pre_sign_revet_fast_each_gate_has_its_own_abort_reason() {
        assert!(revet_all_ok().all_ok(), "cau hinh hop le phai qua het 4 cong");
        assert_eq!(revet_all_ok().abort_reason, None);

        // (a) chua vet nen lan nao
        let r = pre_sign_revet_fast(true, None, 1200, Some(U256::from(100u64)), Some(50), 50, U256::from(20u64), false, 3, Some(0), 3);
        assert_eq!(r.abort_reason, Some("vet_stale"));
        // (a) vet qua han (1201 > 1200)
        let r = pre_sign_revet_fast(true, Some(1201), 1200, Some(U256::from(100u64)), Some(50), 50, U256::from(20u64), false, 3, Some(0), 3);
        assert_eq!(r.abort_reason, Some("vet_stale"));
        // (a) token bi vet nen loai
        let r = pre_sign_revet_fast(false, Some(1), 1200, Some(U256::from(100u64)), Some(50), 50, U256::from(20u64), false, 3, Some(0), 3);
        assert_eq!(r.abort_reason, Some("vet_stale"));

        // (b) reserve cache lech block
        let r = pre_sign_revet_fast(true, Some(1), 1200, Some(U256::from(100u64)), Some(49), 50, U256::from(20u64), false, 3, Some(0), 3);
        assert_eq!(r.abort_reason, Some("reserve_stale"));
        // (b) reserve tut duoi nguong thin_liq
        let r = pre_sign_revet_fast(true, Some(1), 1200, Some(U256::from(19u64)), Some(50), 50, U256::from(20u64), false, 3, Some(0), 3);
        assert_eq!(r.abort_reason, Some("reserve_stale"));

        // (c) chua nap duoc block nao -> KHONG doan "victim con pending"
        let r = pre_sign_revet_fast(true, Some(1), 1200, Some(U256::from(100u64)), Some(50), 50, U256::from(20u64), false, 0, Some(0), 3);
        assert_eq!(r.abort_reason, Some("mined_index_cold"));
        // (c) victim DA len block
        let r = pre_sign_revet_fast(true, Some(1), 1200, Some(U256::from(100u64)), Some(50), 50, U256::from(20u64), true, 3, Some(0), 3);
        assert_eq!(r.abort_reason, Some("victim_already_mined"));

        // (d) nonce chua prefetch / qua cu
        let r = pre_sign_revet_fast(true, Some(1), 1200, Some(U256::from(100u64)), Some(50), 50, U256::from(20u64), false, 3, None, 3);
        assert_eq!(r.abort_reason, Some("nonce_not_prefetched"));
        let r = pre_sign_revet_fast(true, Some(1), 1200, Some(U256::from(100u64)), Some(50), 50, U256::from(20u64), false, 3, Some(4), 3);
        assert_eq!(r.abort_reason, Some("nonce_not_prefetched"));
    }

    /// ĐẠT CẦN DÁN (A4) — hàm re-vet mới phải THUẦN và nhanh: không `async`,
    /// không `provider`, không `measure_tax_evm`. 100.000 lần gọi phải xong
    /// dưới 20 ms (mục tiêu p95 presign < 20 ms cho TOÀN đường ký).
    #[test]
    fn pre_sign_revet_fast_is_pure_and_cheap() {
        let t0 = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = revet_all_ok();
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        println!("pre_sign_revet_fast x100.000 lan = {ms:.3} ms");
        assert!(ms < 20.0, "re-vet thuan phai re (do duoc {ms:.3} ms cho 100.000 lan)");
    }

    #[tokio::test]
    async fn build_and_log_shadow_bundle_aborts_and_logs_when_revet_fails() {
        let (_dir, logger) = tmp_logger();
        let key = "BSC_SANDWICH_TEST_SHADOW_KEY_ABORT";
        std::env::set_var(key, fake_key_hex());
        let signer = load_shadow_signer(key, 56).unwrap();
        std::env::remove_var(key);
        let self_addr = self_address(&signer);
        let wallet = EthereumWallet::from(signer);
        let bad_revet =
            pre_sign_revet_fast(true, Some(1), 1200, Some(U256::from(100u64)), Some(50), 50, U256::from(20u64), true, 3, Some(0), 3);
        let to = Address::from_str("0x10ED43C718714eb63d5aA57B78B54704E256024E").unwrap();
        let token = Address::from_str("0xcccccccccccccccccccccccccccccccccccccccc").unwrap();
        let result = build_and_log_shadow_bundle(
            &logger,
            &wallet,
            self_addr,
            56,
            &bad_revet,
            (to, U256::from(1u64), vec![], 0, 1, 1, 100_000),
            (to, U256::ZERO, vec![], 1, 1, 1, 100_000),
            B256::ZERO,
            token,
            serde_json::json!({"total": 0.1}),
        )
        .await;
        assert!(result.is_none(), "revet fail phai tu choi ky/log, tra None");
        let tail = logger.tail(5);
        assert!(tail.iter().any(|v| v["event"] == "tx.abort" && v["reason"] == "victim_already_mined"));
        assert!(tail.iter().all(|v| v["event"] != "bundle.shadow"), "revet fail khong duoc log bundle.shadow");
    }
}

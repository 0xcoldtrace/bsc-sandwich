//! Cụm "relay-bundle-builder" — module THUẦN (không mở socket/HTTP nào) build
//! request body JSON-RPC 2.0 cho 2 relay bundle-builder trên BSC, chuẩn bị cho
//! `7.3` khi có signer thật + HTTP client thật. Phiên này CHỈ build
//! `serde_json::Value` + LOG preview (`bundle.build_preview`) — KHÔNG gọi HTTP
//! POST tới bất kỳ URL relay nào (test `no_http_network_calls_anywhere_in_relay_rs`
//! dưới xác nhận không có `reqwest`/`hyper`/`TcpStream`/`http::Client` nào
//! xuất hiện dưới dạng code thật trong file này).
//!
//! ## SỬA F-01 (audit Critical, cụm `competitor-recon-and-strategy`, 2026-09-16)
//!
//! Bản gốc build bundle CHỈ 2 leg `[front, back]` — THIẾU victim tx ở giữa.
//! Nếu nối live y nguyên, đây là lỗ CHẮC CHẮN: bot tự mua (front) rồi tự bán
//! (back) mà KHÔNG có giao dịch victim nào chen giữa để tạo chênh lệch giá —
//! chỉ mất 2×0.25% phí pool + price impact tự gây ra, không có nguồn lợi
//! nhuận nào. `build_48club_send_bundle_request`/`build_blockrazor_send_mev_bundle_request`
//! giờ nhận THÊM `victim_raw_hex` (tham số thứ 2, giữa front/back) — bundle
//! LUÔN đúng 3 leg thứ tự `[front, victim_raw, back]`. `victim_raw_hex` PHẢI
//! là raw tx ĐÃ KÝ THẬT của chính victim (lấy qua
//! `transport::fetch_raw_tx_verified`, xem `transport.rs` — ưu tiên
//! `eth_getRawTransactionByHash`, fallback tái tạo từ `eth_getTransactionByHash`
//! qua `Encodable2718`, verify `keccak256(raw)==hash` trước khi dùng) — KHÔNG
//! PHẢI tx do bot tự ký (đó là front/back, 2 leg còn lại). Verify thật (RPC
//! thật, 1 victim raw tx trên chain) ở test `real_rpc_bundle_with_real_victim_raw_tx`.
//!
//! ## Nguồn field (KHÔNG bịa field ngoài danh sách đã research trong lệnh)
//!
//! ### 48 Club (Puissant Builder v2)
//! - Endpoint (chỉ ghi làm hằng số, KHÔNG gọi): `https://puissant-builder.48.club/`
//!   (chain 56) — SỬA ở `relay-finalize` (BAOCAO24, 2026-09-15), xem
//!   doc-comment tại `CLUB48_RPC_URL` bên dưới để biết vì sao endpoint cũ
//!   `https://rpc.48.club` bị thay. Nguồn: <https://docs.48.club/puissant-builder>
//!   (đọc 2026-09-15).
//! - Method `eth_sendBundle`. Nguồn:
//!   <https://docs.48.club/puissant-builder/send-bundle> (đọc 2026-09-15).
//! - Field: `txs` (bắt buộc), `backrunTarget`, `maxBlockNumber` (default
//!   `current_block + 100` khi `None`), `maxTimestamp`, `revertingTxHashes`,
//!   `noMerge`, `positionFirst`. `48spSign` KHÔNG được đưa vào struct field
//!   nào ở đây — bỏ qua đúng lệnh (không có 48SoulPoint key thật phiên này).
//!
//! ### BlockRazor
//! - Endpoint (chỉ ghi làm hằng số, KHÔNG gọi): `https://bsc.blockrazor.xyz`.
//!   Nguồn: <https://docs.blockrazor.io/transaction-submission/rpc/bsc/integration>
//!   (đọc 2026-09-15).
//! - Method `eth_sendMevBundle`. Nguồn:
//!   <https://docs.blockrazor.io/transaction-submission/rpc/bsc/eth_sendbundle.md>
//!   (đọc 2026-09-15).
//! - Field: `txs` (bắt buộc, tối đa 50 theo docs — hàm ở đây LUÔN đúng 2 phần
//!   tử front/back nên không thể vượt), `revertingTxHashes`, `maxBlockNumber`
//!   (default `current_block + 100` khi `None`).
//!
//! ## Quyết định kỹ thuật KHÔNG có sẵn trong lệnh gốc (ghi rõ để phiên sau đối chiếu)
//!
//! - `params` bọc trong 1 mảng 1 phần tử (`[bundle_object]`) — theo đúng quy
//!   ước `eth_sendBundle` kiểu Flashbots mà phần lớn relay BSC (48 Club/
//!   BlockRazor) mô phỏng lại. Lệnh gốc liệt kê CÁC FIELD của bundle nhưng
//!   không nói rõ `params` là mảng hay object trần — đây là 1 lựa chọn kỹ
//!   thuật hợp lý dựa trên quy ước ngành phổ biến, **CHƯA verify bằng cURL
//!   thật tới endpoint thật** (đúng đầu mục 4 của lệnh: "endpoint/method
//!   verify lại bằng cURL thật trước khi tin tưởng field nào cũ/lỗi thời").
//!   Ghi CÒN NỢ — phiên sau gửi 1 request thật (ví dụ bundle test an toàn)
//!   mới xác nhận được đúng/sai hình dạng `params` này.
//! - Field optional (`Option<...>` trong bảng lệnh) khi `None` → BỊ LOẠI khỏi
//!   JSON hoàn toàn (không ghi `null`) — payload gọn, tránh relay hiểu nhầm
//!   `null` tường minh khác với "không gửi field này".

use crate::logger::BotLogger;
use serde_json::{json, Map, Value};

/// Chỉ dùng làm hằng số tham chiếu (địa chỉ RPC relay) — KHÔNG được gọi tới
/// trong file này phiên này (xem CẤM trong lệnh: cấm HTTP thật kể cả "chỉ
/// test").
///
/// SỬA ở `relay-finalize` (BAOCAO24, 2026-09-15): `https://rpc.48.club` (cũ)
/// là node BSC JSON-RPC công khai hợp lệ (`eth_chainId` -> `0x38`) nhưng
/// KHÔNG hỗ trợ method bundle (`-32601 method eth_sendBundle does not
/// exist/is not available`, xác nhận cURL thật ở BAOCAO23). Endpoint ĐÚNG
/// của Puissant Builder là `https://puissant-builder.48.club/` — verify
/// cURL thật (BAOCAO24, payload rác `txs: ["0xdeadbeef"]`) trả lỗi
/// `-32000 rlp: value size exceeds available input length` (lỗi NỘI DUNG,
/// method/`params`/`txs` đã được parse đúng, khác hẳn `-32601` cũ). Nguồn:
/// <https://docs.48.club/puissant-builder> (đọc 2026-09-15).
pub const CLUB48_RPC_URL: &str = "https://puissant-builder.48.club/";
pub const BLOCKRAZOR_RPC_URL: &str = "https://bsc.blockrazor.xyz";

/// Cụm `bugfix-presign-and-contract-plan` (BỔ SUNG GIỮA PHIÊN, Chủ dán docs
/// chính thức BlockRazor **Block Builder**, 2026-09-16) — ĐƯỜNG 1 (ưu tiên):
/// endpoint builder riêng cho vùng, VPS đặt tại NJ/US nên dùng
/// `virginia.builder.blockrazor.io`. Khác `BLOCKRAZOR_RPC_URL` (đường 2,
/// `eth_sendMevBundle`, không auth) ở 3 điểm: (a) method `eth_sendBundle`,
/// (b) BẮT BUỘC header `Authorization: <BLOCKRAZOR_AUTH>`, (c) có thêm field
/// `noMerge`/`positionFirst`.
pub const BLOCKRAZOR_BUILDER_URL_VIRGINIA: &str = "https://virginia.builder.blockrazor.io";

/// Endpoint builder GLOBAL (dự phòng nếu vùng Virginia lỗi) — cùng method/
/// auth với `BLOCKRAZOR_BUILDER_URL_VIRGINIA`.
pub const BLOCKRAZOR_BUILDER_URL_GLOBAL: &str = "https://rpc.blockrazor.builders";

/// **Ví EOA của builder BlockRazor** — bribe đi tới ĐỊA CHỈ NÀY bằng 1
/// transfer BNB thường, **KHÔNG PHẢI `block.coinbase`** (đây là khác biệt
/// quan trọng so với mô hình Flashbots/Ethereum mà cụm `competitor-recon-and-strategy`
/// đã giả định sai với `bribe_mode="coinbase"` cũ). Pin từ docs chính thức Chủ
/// dán 2026-09-16.
pub const BLOCKRAZOR_BUILDER_EOA: &str = "0x1266C6bE60392A8Ff346E8d5ECCd3E69dD9c5F20";

/// Gas price TỐI THIỂU relay BlockRazor chấp nhận cho bundle gửi qua đường
/// builder (docs: ≥ 0.05 gwei). Đường 2 (`eth_sendMevBundle`) cho phép tx
/// 0 gwei miễn TRUNG BÌNH bundle ≥ 0.05 gwei.
pub const BLOCKRAZOR_MIN_GAS_PRICE_WEI: u128 = 50_000_000; // 0.05 gwei

/// **Builder Control EOA của 48 Club Puissant** — bribe đi tới đây bằng
/// transfer BNB (docs chính thức Chủ dán 2026-09-16). Cơ chế xếp hạng bundle
/// của 48 Club: `0.9 × gas fee của tx unique + BNB chuyển tới EOA này` — hệ
/// số 0.9 nghĩa là 1 BNB trả qua **gas** chỉ được tính bằng 0.9 BNB, còn 1
/// BNB **chuyển thẳng** được tính đủ 1.0 → **luôn ưu tiên nhét bribe vào
/// transfer**, giữ gas ở mức tối thiểu (0.05 gwei).
pub const CLUB48_BUILDER_EOA: &str = "0x4848489f0b2BEdd788c696e2D79b6b69D7484848";

/// Hệ số 48 Club áp cho phần bribe trả qua GAS (docs: `0.9 × gas fee`).
/// Dùng khi so sánh 2 cách trả bribe — KHÔNG phải hằng số dùng để tính tiền
/// gửi đi.
pub const CLUB48_GAS_FEE_WEIGHT: f64 = 0.9;

/// Cụm `bugfix-presign-and-contract-plan` (BỔ SUNG GIỮA PHIÊN) — bảng
/// `relay -> ví EOA builder` (cùng nội dung pin trong `DEX_REGISTRY.md`, kèm
/// `source_url` + ngày đọc docs).
pub fn builder_eoa_for(relay: &str) -> Option<&'static str> {
    match relay {
        "blockrazor" => Some(BLOCKRAZOR_BUILDER_EOA),
        "club48" => Some(CLUB48_BUILDER_EOA),
        _ => None,
    }
}

/// `front_raw_hex`/`back_raw_hex` có thể tới không kèm tiền tố `0x` — chuẩn
/// hoá về dạng có `0x` (quy ước hex chuẩn của mọi field `txs` trong 2 relay
/// trên). Hàm THUẦN chuỗi, KHÔNG validate độ dài/tính hợp lệ RLP của tx (chưa
/// có signer thật để tạo raw tx hợp lệ phiên này — xem doc-comment module).
fn normalize_raw_tx_hex(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
        trimmed.to_string()
    } else {
        format!("0x{trimmed}")
    }
}

/// Field optional của bundle 48 Club — mặc định (`Default`) tất cả `None`.
/// `48spSign` cố ý KHÔNG có field tương ứng ở đây (xem doc-comment module).
#[derive(Debug, Clone, Default)]
pub struct Club48BundleOptions {
    pub backrun_target: Option<String>,
    pub max_block_number: Option<u64>,
    pub max_timestamp: Option<u64>,
    pub reverting_tx_hashes: Option<Vec<String>>,
    pub no_merge: Option<bool>,
    pub position_first: Option<bool>,
}

/// Field optional của bundle BlockRazor — mặc định (`Default`) tất cả `None`.
#[derive(Debug, Clone, Default)]
pub struct BlockRazorBundleOptions {
    pub reverting_tx_hashes: Option<Vec<String>>,
    pub max_block_number: Option<u64>,
    /// Đường BUILDER (`eth_sendBundle`) — docs chính thức 2026-09-16.
    pub no_merge: Option<bool>,
    /// Đường BUILDER — yêu cầu bundle đứng ĐẦU block.
    pub position_first: Option<bool>,
}

/// Cụm `bugfix-presign-and-contract-plan` (BỔ SUNG GIỮA PHIÊN) — request
/// `eth_sendBundle` cho **BlockRazor Block Builder** (đường 1) kèm header
/// `Authorization`. Trả CẢ body lẫn endpoint/header để tầng gửi (chưa tồn
/// tại — `7.3`) không phải tự đoán; module này vẫn KHÔNG mở kết nối nào.
#[derive(Debug, Clone)]
pub struct BuilderRequest {
    pub url: &'static str,
    /// Giá trị header `Authorization` — LẤY TỪ `.env` (`BLOCKRAZOR_AUTH`),
    /// KHÔNG BAO GIỜ log nguyên văn (xem `build_and_log_relay_bundle_previews`).
    pub auth: String,
    pub body: Value,
}

/// Build request `eth_sendBundle` cho BlockRazor **Block Builder** (đường 1,
/// CÓ auth). Hàm THUẦN. `auth` rỗng -> trả `None`: relay này bị TẮT, caller
/// phải log rõ và rơi về đường 2 (`eth_sendMevBundle`, không auth) — KHÔNG
/// được tự bịa token, KHÔNG được panic (đúng yêu cầu Chủ: "thiếu → relay
/// disabled, log rõ, không panic").
pub fn build_blockrazor_builder_send_bundle_request(
    front_raw_hex: &str,
    victim_raw_hex: &str,
    back_raw_hex: &str,
    current_block: u64,
    request_id: u64,
    auth: &str,
    url: &'static str,
    opts: &BlockRazorBundleOptions,
) -> Option<BuilderRequest> {
    if auth.trim().is_empty() {
        return None;
    }
    let max_block_number = opts.max_block_number.unwrap_or(current_block + 100);
    let mut bundle = Map::new();
    bundle.insert(
        "txs".to_string(),
        json!([normalize_raw_tx_hex(front_raw_hex), normalize_raw_tx_hex(victim_raw_hex), normalize_raw_tx_hex(back_raw_hex)]),
    );
    bundle.insert("maxBlockNumber".to_string(), json!(max_block_number));
    if let Some(ref v) = opts.reverting_tx_hashes {
        bundle.insert("revertingTxHashes".to_string(), json!(v));
    }
    if let Some(v) = opts.no_merge {
        bundle.insert("noMerge".to_string(), json!(v));
    }
    if let Some(v) = opts.position_first {
        bundle.insert("positionFirst".to_string(), json!(v));
    }
    Some(BuilderRequest {
        url,
        auth: auth.to_string(),
        body: json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "eth_sendBundle",
            "params": [Value::Object(bundle)],
        }),
    })
}

/// Build request `eth_sendBundle` cho 48 Club (Puissant Builder v2). Hàm
/// THUẦN — chỉ trả `serde_json::Value`, KHÔNG mở kết nối nào. `current_block`
/// do caller tự cung cấp (module này không giữ `Provider`) để tính default
/// `maxBlockNumber = current_block + 100` khi `opts.max_block_number` là
/// `None`, đúng bảng field trong lệnh.
pub fn build_48club_send_bundle_request(
    front_raw_hex: &str,
    victim_raw_hex: &str,
    back_raw_hex: &str,
    current_block: u64,
    request_id: u64,
    opts: &Club48BundleOptions,
) -> Value {
    let max_block_number = opts.max_block_number.unwrap_or(current_block + 100);

    let mut bundle = Map::new();
    // Cụm `competitor-recon-and-strategy` (F-01, audit Critical) — SỬA bug
    // "bundle chỉ gồm [front, back], THIẾU victim tx" (nếu gửi live sẽ lỗ
    // chắc chắn: bot tự mua rồi tự bán mà KHÔNG có victim ở giữa, 2×0.25%
    // phí + price impact). Bundle nguyên tử giờ ĐÚNG 3 leg thứ tự
    // `[front, victim_raw, back]` — `victim_raw_hex` là raw tx ĐÃ KÝ THẬT
    // của victim (không phải bot tự ký), lấy qua `transport::fetch_raw_tx_verified`.
    bundle.insert(
        "txs".to_string(),
        json!([normalize_raw_tx_hex(front_raw_hex), normalize_raw_tx_hex(victim_raw_hex), normalize_raw_tx_hex(back_raw_hex)]),
    );
    bundle.insert("maxBlockNumber".to_string(), json!(max_block_number));
    if let Some(ref v) = opts.backrun_target {
        bundle.insert("backrunTarget".to_string(), json!(v));
    }
    if let Some(v) = opts.max_timestamp {
        bundle.insert("maxTimestamp".to_string(), json!(v));
    }
    if let Some(ref v) = opts.reverting_tx_hashes {
        bundle.insert("revertingTxHashes".to_string(), json!(v));
    }
    if let Some(v) = opts.no_merge {
        bundle.insert("noMerge".to_string(), json!(v));
    }
    if let Some(v) = opts.position_first {
        bundle.insert("positionFirst".to_string(), json!(v));
    }

    json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "eth_sendBundle",
        "params": [Value::Object(bundle)],
    })
}

/// Build request `eth_sendMevBundle` cho BlockRazor. Hàm THUẦN — chỉ trả
/// `serde_json::Value`, KHÔNG mở kết nối nào.
pub fn build_blockrazor_send_mev_bundle_request(
    front_raw_hex: &str,
    victim_raw_hex: &str,
    back_raw_hex: &str,
    current_block: u64,
    request_id: u64,
    opts: &BlockRazorBundleOptions,
) -> Value {
    let max_block_number = opts.max_block_number.unwrap_or(current_block + 100);

    let mut bundle = Map::new();
    // F-01 (xem doc-comment 48 Club ở trên, cùng lý do/cùng sửa).
    bundle.insert(
        "txs".to_string(),
        json!([normalize_raw_tx_hex(front_raw_hex), normalize_raw_tx_hex(victim_raw_hex), normalize_raw_tx_hex(back_raw_hex)]),
    );
    bundle.insert("maxBlockNumber".to_string(), json!(max_block_number));
    if let Some(ref v) = opts.reverting_tx_hashes {
        bundle.insert("revertingTxHashes".to_string(), json!(v));
    }

    json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "eth_sendMevBundle",
        "params": [Value::Object(bundle)],
    })
}

/// 2 request preview đã build, ghép cặp để log 1 lần — KHÔNG gửi đi đâu.
#[derive(Debug, Clone)]
pub struct RelayBundlePreview {
    pub club48_request: Value,
    pub blockrazor_request: Value,
}

/// Điểm gọi DUY NHẤT ghép build 2 request + log preview
/// (`logs/bot.jsonl` event `bundle.build_preview`). CHỈ LOG — không có
/// `Provider`/HTTP client nào ở đây hay bất kỳ đâu trong module này (xem test
/// `no_http_network_calls_anywhere_in_relay_rs` dưới).
pub fn build_and_log_relay_bundle_previews(
    logger: &BotLogger,
    front_raw_hex: &str,
    victim_raw_hex: &str,
    back_raw_hex: &str,
    current_block: u64,
    request_id: u64,
    club48_opts: &Club48BundleOptions,
    blockrazor_opts: &BlockRazorBundleOptions,
) -> RelayBundlePreview {
    let club48_request = build_48club_send_bundle_request(front_raw_hex, victim_raw_hex, back_raw_hex, current_block, request_id, club48_opts);
    let blockrazor_request =
        build_blockrazor_send_mev_bundle_request(front_raw_hex, victim_raw_hex, back_raw_hex, current_block, request_id, blockrazor_opts);

    logger.log(
        "bundle.build_preview",
        json!({
            "note": "PREVIEW ONLY - khong gui HTTP that, chua co signer that, chua co HTTP client",
            "club48": {
                "endpoint": CLUB48_RPC_URL,
                "request": club48_request,
            },
            "blockrazor": {
                "endpoint": BLOCKRAZOR_RPC_URL,
                "request": blockrazor_request,
            },
        }),
    );

    RelayBundlePreview { club48_request, blockrazor_request }
}

#[cfg(test)]
mod tests {
    use super::*;

    // FIXTURE — chuỗi hex GIẢ LẬP rõ ràng, KHÔNG phải raw tx thật/đã ký (chưa
    // có signer thật trong repo, xem docs/STATE.md mục 7.1). Một chuỗi CÓ
    // `0x`, một chuỗi KHÔNG có, để test cả 2 nhánh `normalize_raw_tx_hex`.
    const FIXTURE_FRONT_RAW_TX_HEX: &str = "f00dfixturefrontnotarealtx00000000000000000000000000000000001234";
    const FIXTURE_BACK_RAW_TX_HEX: &str = "0xfeedfixturebacknotarealtx0000000000000000000000000000000000005678";
    // Cụm `competitor-recon-and-strategy` (F-01) — leg victim MỚI (thứ tự
    // GIỮA front/back trong bundle nguyên tử) — fixture giả lập, KHÔNG phải
    // raw tx thật (test roundtrip với tx thật nằm ở
    // `transport::tests::real_rpc_reconstruct_raw_tx_type0_and_type2`).
    const FIXTURE_VICTIM_RAW_TX_HEX: &str = "0xbeeffixturevictimnotarealtx000000000000000000000000000000009abc";

    fn tmp_logger() -> (tempfile::TempDir, BotLogger) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("logs").join("bot.jsonl");
        let logger = BotLogger::new(&path).unwrap();
        (dir, logger)
    }

    /// SỬA ở `relay-finalize` (BAOCAO24) — `CLUB48_RPC_URL` cũ
    /// (`https://rpc.48.club`) xác nhận SAI bằng cURL thật ở BAOCAO23
    /// (`-32601 method eth_sendBundle does not exist/is not available`).
    /// Endpoint mới xác nhận ĐÚNG bằng cURL thật ở BAOCAO24 (payload rác
    /// `txs: ["0xdeadbeef"]` trả `-32000 rlp: value size exceeds available
    /// input length` — lỗi nội dung, không phải lỗi method/route).
    #[test]
    fn club48_rpc_url_pinned_to_verified_puissant_builder_endpoint() {
        assert_eq!(CLUB48_RPC_URL, "https://puissant-builder.48.club/");
    }

    #[test]
    fn normalize_raw_tx_hex_adds_0x_prefix_when_missing() {
        assert_eq!(normalize_raw_tx_hex("abcd"), "0xabcd");
    }

    #[test]
    fn normalize_raw_tx_hex_keeps_existing_0x_prefix() {
        assert_eq!(normalize_raw_tx_hex("0xabcd"), "0xabcd");
    }

    #[test]
    fn club48_request_matches_pinned_schema_with_default_options() {
        let req = build_48club_send_bundle_request(
            FIXTURE_FRONT_RAW_TX_HEX,
            FIXTURE_VICTIM_RAW_TX_HEX,
            FIXTURE_BACK_RAW_TX_HEX,
            1_000_000,
            7,
            &Club48BundleOptions::default(),
        );

        assert_eq!(req["jsonrpc"], "2.0");
        assert_eq!(req["id"], 7);
        assert_eq!(req["method"], "eth_sendBundle");

        let bundle = &req["params"][0];
        // F-01 - dung 3 leg, victim O GIUA (thu tu nguyen tu: front, victim, back).
        assert_eq!(bundle["txs"].as_array().unwrap().len(), 3, "bundle PHAI co dung 3 leg [front, victim, back], khong con thieu victim");
        assert_eq!(bundle["txs"][0], "0xf00dfixturefrontnotarealtx00000000000000000000000000000000001234");
        assert_eq!(bundle["txs"][1], FIXTURE_VICTIM_RAW_TX_HEX);
        assert_eq!(bundle["txs"][2], "0xfeedfixturebacknotarealtx0000000000000000000000000000000000005678");
        // default: current_block(1_000_000) + 100
        assert_eq!(bundle["maxBlockNumber"], 1_000_100);

        // optional fields KHONG duoc de None -> phai vang mat hoan toan (khong phai null)
        assert!(bundle.get("backrunTarget").is_none());
        assert!(bundle.get("maxTimestamp").is_none());
        assert!(bundle.get("revertingTxHashes").is_none());
        assert!(bundle.get("noMerge").is_none());
        assert!(bundle.get("positionFirst").is_none());
        // 48spSign KHONG duoc xuat hien duoi bat ky ten field nao (bo qua dung lenh)
        assert!(bundle.get("48spSign").is_none());
    }

    #[test]
    fn club48_request_includes_all_optional_fields_when_provided() {
        let opts = Club48BundleOptions {
            backrun_target: Some("0xdeadbeef".to_string()),
            max_block_number: Some(999_999),
            max_timestamp: Some(1_800_000_000),
            reverting_tx_hashes: Some(vec!["0xaaaa".to_string(), "0xbbbb".to_string()]),
            no_merge: Some(true),
            position_first: Some(false),
        };
        let req = build_48club_send_bundle_request(
            FIXTURE_FRONT_RAW_TX_HEX,
            FIXTURE_VICTIM_RAW_TX_HEX,
            FIXTURE_BACK_RAW_TX_HEX,
            1_000_000,
            1,
            &opts,
        );
        let bundle = &req["params"][0];

        // maxBlockNumber tuong minh khac default (999_999 != 1_000_100) -> khang dinh Some() thang the default
        assert_eq!(bundle["maxBlockNumber"], 999_999);
        assert_eq!(bundle["backrunTarget"], "0xdeadbeef");
        assert_eq!(bundle["maxTimestamp"], 1_800_000_000_u64);
        assert_eq!(bundle["revertingTxHashes"][0], "0xaaaa");
        assert_eq!(bundle["revertingTxHashes"][1], "0xbbbb");
        assert_eq!(bundle["noMerge"], true);
        assert_eq!(bundle["positionFirst"], false);
    }

    #[test]
    fn blockrazor_request_matches_pinned_schema_with_default_options() {
        let req = build_blockrazor_send_mev_bundle_request(
            FIXTURE_FRONT_RAW_TX_HEX,
            FIXTURE_VICTIM_RAW_TX_HEX,
            FIXTURE_BACK_RAW_TX_HEX,
            2_000_000,
            42,
            &BlockRazorBundleOptions::default(),
        );

        assert_eq!(req["jsonrpc"], "2.0");
        assert_eq!(req["id"], 42);
        assert_eq!(req["method"], "eth_sendMevBundle");

        let bundle = &req["params"][0];
        assert_eq!(bundle["txs"].as_array().unwrap().len(), 3, "bundle PHAI co dung 3 leg [front, victim, back]");
        assert_eq!(bundle["txs"][0], "0xf00dfixturefrontnotarealtx00000000000000000000000000000000001234");
        assert_eq!(bundle["txs"][1], FIXTURE_VICTIM_RAW_TX_HEX);
        assert_eq!(bundle["txs"][2], "0xfeedfixturebacknotarealtx0000000000000000000000000000000000005678");
        assert_eq!(bundle["maxBlockNumber"], 2_000_100);
        assert!(bundle.get("revertingTxHashes").is_none());
        // BlockRazor KHONG co backrunTarget/maxTimestamp/noMerge/positionFirst (chi 48 Club moi co)
        assert!(bundle.get("backrunTarget").is_none());
        assert!(bundle.get("maxTimestamp").is_none());
        assert!(bundle.get("noMerge").is_none());
        assert!(bundle.get("positionFirst").is_none());
    }

    // ===== Cụm `bugfix-presign-and-contract-plan` (BỔ SUNG GIỮA PHIÊN) =====

    /// ĐẠT CẦN DÁN — đường BUILDER (`eth_sendBundle` + header `Authorization`)
    /// theo docs chính thức BlockRazor Chủ dán 2026-09-16.
    #[test]
    fn blockrazor_builder_request_has_auth_header_method_and_all_doc_fields() {
        let opts = BlockRazorBundleOptions {
            reverting_tx_hashes: None,
            max_block_number: Some(500),
            no_merge: Some(true),
            position_first: Some(true),
        };
        let req = build_blockrazor_builder_send_bundle_request(
            FIXTURE_FRONT_RAW_TX_HEX,
            FIXTURE_VICTIM_RAW_TX_HEX,
            FIXTURE_BACK_RAW_TX_HEX,
            400,
            9,
            "SECRET_TOKEN_TEST",
            BLOCKRAZOR_BUILDER_URL_VIRGINIA,
            &opts,
        )
        .expect("co auth -> phai build duoc");
        assert_eq!(req.url, "https://virginia.builder.blockrazor.io");
        assert_eq!(req.auth, "SECRET_TOKEN_TEST");
        assert_eq!(req.body["method"], "eth_sendBundle", "duong builder dung eth_sendBundle, KHAC eth_sendMevBundle cua duong 2");
        let b = &req.body["params"][0];
        assert_eq!(b["txs"].as_array().unwrap().len(), 3, "bundle LUON 3 chan [front, victim, back]");
        assert_eq!(b["txs"][1], FIXTURE_VICTIM_RAW_TX_HEX);
        assert_eq!(b["maxBlockNumber"], 500);
        assert_eq!(b["noMerge"], true);
        assert_eq!(b["positionFirst"], true);
        assert!(b.get("revertingTxHashes").is_none(), "None -> bi loai khoi JSON, khong ghi null");
    }

    /// ĐẠT CẦN DÁN — "thiếu `BLOCKRAZOR_AUTH` → relay disabled, log rõ,
    /// KHÔNG panic" (yêu cầu Chủ). Hàm trả `None`, caller rơi về đường 2.
    #[test]
    fn blockrazor_builder_request_without_auth_is_disabled_not_panic() {
        let opts = BlockRazorBundleOptions::default();
        for empty in ["", "   "] {
            let r = build_blockrazor_builder_send_bundle_request(
                FIXTURE_FRONT_RAW_TX_HEX,
                FIXTURE_VICTIM_RAW_TX_HEX,
                FIXTURE_BACK_RAW_TX_HEX,
                1,
                1,
                empty,
                BLOCKRAZOR_BUILDER_URL_GLOBAL,
                &opts,
            );
            assert!(r.is_none(), "auth rong -> relay TAT, khong bia token, khong panic");
        }
    }

    /// ĐẠT CẦN DÁN — bảng `relay -> ví EOA builder` (pin trong
    /// `DEX_REGISTRY.md`): bribe là transfer BNB tới 2 địa chỉ NÀY, KHÔNG
    /// phải `block.coinbase`.
    #[test]
    fn builder_eoa_table_matches_official_docs() {
        assert_eq!(builder_eoa_for("blockrazor"), Some("0x1266C6bE60392A8Ff346E8d5ECCd3E69dD9c5F20"));
        assert_eq!(builder_eoa_for("club48"), Some("0x4848489f0b2BEdd788c696e2D79b6b69D7484848"));
        assert_eq!(builder_eoa_for("khong_ton_tai"), None, "relay la relay chua pin -> None, khong doan dia chi");
        // 2 vi builder PHAI khac nhau (moi relay mot vi rieng).
        assert_ne!(BLOCKRAZOR_BUILDER_EOA, CLUB48_BUILDER_EOA);
    }

    /// ĐẠT CẦN DÁN — 48 Club: `backrunTarget` = hash victim (điền khi có),
    /// `revertingTxHashes` để RỖNG (front/back KHÔNG được phép revert;
    /// victim là tx public nên cũng không nằm trong danh sách).
    #[test]
    fn club48_bundle_carries_backrun_target_and_empty_reverting_list() {
        let victim_hash = "0x3a8fa10d00000000000000000000000000000000000000000000000000000000";
        let opts = Club48BundleOptions {
            backrun_target: Some(victim_hash.to_string()),
            reverting_tx_hashes: None,
            ..Default::default()
        };
        let req = build_48club_send_bundle_request(
            FIXTURE_FRONT_RAW_TX_HEX,
            FIXTURE_VICTIM_RAW_TX_HEX,
            FIXTURE_BACK_RAW_TX_HEX,
            1_000,
            3,
            &opts,
        );
        let b = &req["params"][0];
        assert_eq!(b["backrunTarget"], victim_hash);
        assert!(b.get("revertingTxHashes").is_none(), "de RONG - khong chan nao duoc phep revert");
        assert!(b.get("48spSign").is_none(), "chua la member -> bo qua, khong bia chu ky");
    }

    #[test]
    fn blockrazor_request_includes_reverting_tx_hashes_and_explicit_max_block_number() {
        let opts = BlockRazorBundleOptions {
            reverting_tx_hashes: Some(vec!["0xcccc".to_string()]),
            max_block_number: Some(123_456),
            ..Default::default()
        };
        let req = build_blockrazor_send_mev_bundle_request(
            FIXTURE_FRONT_RAW_TX_HEX,
            FIXTURE_VICTIM_RAW_TX_HEX,
            FIXTURE_BACK_RAW_TX_HEX,
            2_000_000,
            1,
            &opts,
        );
        let bundle = &req["params"][0];
        assert_eq!(bundle["maxBlockNumber"], 123_456);
        assert_eq!(bundle["revertingTxHashes"][0], "0xcccc");
    }

    #[test]
    fn blockrazor_txs_never_exceeds_documented_50_tx_limit() {
        // Ham nay LUON dung 3 tx (front+victim+back, F-01) - khong co duong
        // nao build > 3 phan tu.
        let req = build_blockrazor_send_mev_bundle_request(
            FIXTURE_FRONT_RAW_TX_HEX,
            FIXTURE_VICTIM_RAW_TX_HEX,
            FIXTURE_BACK_RAW_TX_HEX,
            1,
            1,
            &BlockRazorBundleOptions::default(),
        );
        let txs = req["params"][0]["txs"].as_array().unwrap();
        assert!(txs.len() <= 50);
        assert_eq!(txs.len(), 3);
    }

    #[test]
    fn build_and_log_relay_bundle_previews_logs_single_event_with_both_requests() {
        let (_dir, logger) = tmp_logger();
        let preview = build_and_log_relay_bundle_previews(
            &logger,
            FIXTURE_FRONT_RAW_TX_HEX,
            FIXTURE_VICTIM_RAW_TX_HEX,
            FIXTURE_BACK_RAW_TX_HEX,
            5_000_000,
            1,
            &Club48BundleOptions::default(),
            &BlockRazorBundleOptions::default(),
        );

        let tail = logger.tail(10);
        assert_eq!(tail.len(), 1);
        assert_eq!(tail[0]["event"], "bundle.build_preview");
        assert_eq!(tail[0]["club48"]["endpoint"], CLUB48_RPC_URL);
        assert_eq!(tail[0]["blockrazor"]["endpoint"], BLOCKRAZOR_RPC_URL);
        assert_eq!(tail[0]["club48"]["request"], preview.club48_request);
        assert_eq!(tail[0]["blockrazor"]["request"], preview.blockrazor_request);
        assert_eq!(preview.club48_request["method"], "eth_sendBundle");
        assert_eq!(preview.blockrazor_request["method"], "eth_sendMevBundle");
    }

    /// `#[ignore]` — RPC thật. Chứng minh end-to-end F-01: lấy 1 raw tx THẬT
    /// đã ký trên chain (qua `transport::fetch_raw_tx_verified`, xem test
    /// tương ứng ở `transport.rs`) làm `victim_raw_hex`, ghép với front/back
    /// FIXTURE (chưa có signer thật cho 2 chân đó, xem `docs/STATE.md` mục
    /// `7.1`) — build ra bundle 3 leg đúng thứ tự `[front, victim_THẬT, back]`
    /// cho cả 2 relay, verify `txs[1]` chính là raw tx thật (bit-for-bit).
    #[tokio::test]
    #[ignore]
    async fn real_rpc_bundle_with_real_victim_raw_tx() {
        use crate::sim_evm::validate_rpc_urls;
        use crate::transport;
        use alloy::eips::BlockNumberOrTag;
        use alloy::providers::{Provider, ProviderBuilder};

        let urls = validate_rpc_urls();
        let mut provider = None;
        for u in &urls {
            if let Ok(p) = ProviderBuilder::new().connect(u).await {
                if p.get_chain_id().await.unwrap_or(0) == 56 {
                    provider = Some(p.erased());
                    break;
                }
            }
        }
        let Some(provider) = provider else {
            println!("SKIP (khong phai FAIL): khong ket noi duoc RPC nao");
            return;
        };
        let latest = provider.get_block_number().await.expect("eth_blockNumber that bai");
        let block = provider
            .get_block_by_number(BlockNumberOrTag::Number(latest.saturating_sub(2)))
            .full()
            .await
            .expect("get_block that bai")
            .expect("block phai ton tai");
        let victim_hash = block.transactions.txns().next().map(|t| <_ as alloy::network::TransactionResponse>::tx_hash(t));
        let Some(victim_hash) = victim_hash else {
            println!("SKIP (khong phai FAIL): block khong co tx nao");
            return;
        };
        let (victim_raw_bytes, source) = transport::fetch_raw_tx_verified(&provider, victim_hash).await.expect("fetch_raw_tx_verified that bai");
        let victim_raw_hex = format!("0x{}", victim_raw_bytes.iter().map(|b| format!("{b:02x}")).collect::<String>());
        println!("victim_raw THAT: hash={victim_hash:#x} nguon={} len={}", source.as_str(), victim_raw_bytes.len());

        let req = build_48club_send_bundle_request(FIXTURE_FRONT_RAW_TX_HEX, &victim_raw_hex, FIXTURE_BACK_RAW_TX_HEX, latest, 1, &Club48BundleOptions::default());
        let bundle = &req["params"][0];
        assert_eq!(bundle["txs"].as_array().unwrap().len(), 3);
        assert_eq!(bundle["txs"][1], victim_raw_hex, "leg giua bundle phai la victim_raw THAT, bit-for-bit");

        let req2 = build_blockrazor_send_mev_bundle_request(FIXTURE_FRONT_RAW_TX_HEX, &victim_raw_hex, FIXTURE_BACK_RAW_TX_HEX, latest, 1, &BlockRazorBundleOptions::default());
        assert_eq!(req2["params"][0]["txs"][1], victim_raw_hex);
        println!("48club + blockrazor bundle 3-leg voi victim THAT: OK");
    }

    /// ĐẠT CẦN DÁN: xác nhận KHÔNG có bất kỳ dấu hiệu gọi HTTP/network THẬT
    /// nào tồn tại trong `src/relay.rs` (kể cả `#[cfg(test)]`) — CHỈ scope
    /// file này (khác `no_send_raw_transaction_call_anywhere_in_src` ở
    /// `executor.rs` quét toàn `src/`), đúng lệnh "grep ... trong src/relay.rs
    /// = 0". Needle được GHÉP CHUỖI TẠI RUNTIME (không xuất hiện liên tục
    /// trong source) để chính dòng code check này không tự báo nó "vi phạm".
    /// Bỏ qua dòng comment (bắt đầu `//`) vì doc-comment đầu file có NHẮC TÊN
    /// các crate đó trong lời giải thích, không phải lời gọi thật.
    #[test]
    fn no_http_network_calls_anywhere_in_relay_rs() {
        // Ten bien CO Y NE tranh chua lien tuc chinh chuoi can tim (vd khong
        // dat ten "needle_reqwest" vi ban than no chua san "reqwest") - de
        // dong code nay khong tu bao no la vi pham khi quet chinh file nay.
        let needle_1: String = format!("{}{}", "req", "west");
        let needle_2: String = format!("{}{}", "hy", "per");
        let needle_3: String = format!("{}{}", "Tcp", "Stream");
        let needle_4: String = format!("{}{}", "http::", "Client");

        let content = include_str!("relay.rs");
        let mut offending = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim_start();
            let is_comment_line = trimmed.starts_with("//");
            if is_comment_line {
                continue;
            }
            let looks_like_network_call =
                line.contains(&needle_1) || line.contains(&needle_2) || line.contains(&needle_3) || line.contains(&needle_4);
            if looks_like_network_call {
                offending.push(format!("{}: {}", i + 1, line.trim()));
            }
        }
        assert!(
            offending.is_empty(),
            "tim thay dau hieu goi HTTP/network trong relay.rs (cam tuyet doi phien nay): {offending:?}"
        );
    }
}

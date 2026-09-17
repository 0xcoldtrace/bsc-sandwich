//! Cụm 3.3 — Sim V3 qua `QuoterV2` đã pin (`DEX_REGISTRY.md`,
//! `venues::V3_QUOTER_V2_ADDRESS`). Chỉ dùng `quoteExactInputSingle` (khớp
//! `decoder.rs` chỉ decode `exactInputSingle`/`exactInput` ĐÚNG 1 hop —
//! multihop đã bị chặn `not_wbnb_pair`, nên luôn quote được single-pool).
//!
//! Struct `QuoteExactInputSingleParams` xác nhận trực tiếp qua `curl` raw
//! source phiên này (KHÔNG dùng bản tóm tắt AI đầu tiên vì phát hiện lệch
//! thứ tự field với bản raw — xem `docs/STATE.md` mục V3 quoter):
//! `github.com/pancakeswap/pancake-v3-contracts`,
//! `projects/v3-periphery/contracts/interfaces/IQuoterV2.sol`:
//! ```solidity
//! struct QuoteExactInputSingleParams {
//!     address tokenIn;
//!     address tokenOut;
//!     uint256 amountIn;
//!     uint24 fee;
//!     uint160 sqrtPriceLimitX96;
//! }
//! ```
//! Hàm `quoteExactInputSingle` KHÔNG có modifier `view` trong Solidity (gọi
//! hàm non-view nội bộ để tính rồi trả kết quả bình thường — không dùng kiểu
//! revert-decode như QuoterV1) nhưng vẫn an toàn gọi qua `eth_call` vì
//! `eth_call` không bao giờ commit state lên chain bất kể mutability khai
//! báo trong Solidity.
//!
//! V4/Infinity: `pool.rs::resolve_infinity_pool` đã trả `hooks_unread` cho
//! MỌI token (chưa pin cách tìm `PoolKey`) — sim V4 phiên này KHÔNG gọi
//! `CLQuoter`/`BinQuoter` cho pool nào (không có `PoolKey` để build calldata,
//! gọi bừa là bịa tham số `hooks`/`parameters`, đúng AGENTS.md "không đoán
//! tick/hooks"). Giữ nguyên skip đó — xem `resolve_infinity_pool`.

use alloy::primitives::{keccak256, Address, U256};
use alloy::providers::Provider;
use alloy::rpc::types::eth::TransactionRequest;
use std::sync::LazyLock;

use crate::sim_arb::{ArbV3Pool, V3Family};

fn selector(sig: &str) -> [u8; 4] {
    let hash = keccak256(sig.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}

static SEL_QUOTE_EXACT_INPUT_SINGLE: LazyLock<[u8; 4]> =
    LazyLock::new(|| selector("quoteExactInputSingle((address,address,uint256,uint24,uint160))"));

fn encode_address_word(a: Address, out: &mut Vec<u8>) {
    out.extend_from_slice(&[0u8; 12]);
    out.extend_from_slice(a.as_slice());
}

fn encode_uint256_word(v: U256, out: &mut Vec<u8>) {
    out.extend_from_slice(&v.to_be_bytes::<32>());
}

fn encode_uint24_word(v: u32, out: &mut Vec<u8>) {
    let mut word = [0u8; 32];
    let bytes = v.to_be_bytes();
    word[29..32].copy_from_slice(&bytes[1..4]);
    out.extend_from_slice(&word);
}

/// Build calldata `quoteExactInputSingle(params)` — tuple toàn field static
/// (address/uint256/uint24/uint160) nên encode nối tiếp thẳng sau selector,
/// KHÔNG cần offset (không có phần tử dynamic trong struct này, khác
/// `exactInput`/UR path là `bytes`).
pub fn build_quote_calldata(token_in: Address, token_out: Address, fee: u32, amount_in: U256) -> Vec<u8> {
    let mut out = SEL_QUOTE_EXACT_INPUT_SINGLE.to_vec();
    encode_address_word(token_in, &mut out);
    encode_address_word(token_out, &mut out);
    encode_uint256_word(amount_in, &mut out);
    encode_uint24_word(fee, &mut out);
    encode_uint256_word(U256::ZERO, &mut out); // sqrtPriceLimitX96 = 0 (khong gioi han)
    out
}

/// Giá trị trả về đầu tiên (`amountOut`, uint256) trong 4 giá trị trả về —
/// chỉ cần `amountOut` cho sim, bỏ qua `sqrtPriceX96After`/ticks/gasEstimate.
pub fn decode_quote_amount_out(ret: &[u8]) -> Option<U256> {
    if ret.len() < 32 {
        return None;
    }
    Some(U256::from_be_slice(&ret[0..32]))
}

/// `eth_call` thật tới `QuoterV2` đã pin.
pub async fn quote_exact_input_single(
    provider: &dyn Provider,
    quoter: Address,
    token_in: Address,
    token_out: Address,
    fee: u32,
    amount_in: U256,
) -> Result<U256, String> {
    quote_exact_input_single_at(provider, quoter, token_in, token_out, fee, amount_in, None).await
}

/// Cụm `planB-B5-simarb-v3-measure` — cùng calldata, có thể ghim block
/// (`eth_call` tại block, cache phía caller theo `(pool, block)`).
pub async fn quote_exact_input_single_at(
    provider: &dyn Provider,
    quoter: Address,
    token_in: Address,
    token_out: Address,
    fee: u32,
    amount_in: U256,
    block: Option<u64>,
) -> Result<U256, String> {
    let calldata = build_quote_calldata(token_in, token_out, fee, amount_in);
    let tx = TransactionRequest::default().to(quoter).input(calldata.into());
    let ret = if let Some(b) = block {
        provider
            .call(tx)
            .block(alloy::eips::BlockId::number(b))
            .await
            .map_err(|e| format!("eth_call quoteExactInputSingle block={b} that bai: {e}"))?
    } else {
        provider
            .call(tx)
            .await
            .map_err(|e| format!("eth_call quoteExactInputSingle that bai: {e}"))?
    };
    decode_quote_amount_out(&ret).ok_or_else(|| "quoteExactInputSingle tra ve du lieu qua ngan".to_string())
}

/// Fit virtual CPMM cho 1 pool V3 bằng 2 `eth_call` QuoterV2 (đúng trần
/// ≤5 / route khi mỗi pool fit 1 lần rồi search closed-form 0 quote thêm).
/// `probe` = cỡ quote (wei) — dùng `1 BNB` hoặc tương đương USDT.
pub async fn fit_arb_v3_pool(
    provider: &dyn Provider,
    token: Address,
    p: &mut ArbV3Pool,
    block: Option<u64>,
    probe: U256,
) -> Result<u32, String> {
    if !p.reserve_quote.is_zero() && !p.reserve_token.is_zero() {
        return Ok(0);
    }
    let quoter = match p.family {
        V3Family::Pcs => crate::venues::v3_quoter(),
        V3Family::Uni => crate::venues::uni_v3_quoter(),
    };
    let a1 = (probe / U256::from(5u64)).max(U256::from(10_000_000_000_000_000u64));
    let a2 = probe.max(a1 + U256::from(1u64));
    let o1 = quote_exact_input_single_at(provider, quoter, p.quote, token, p.fee, a1, block).await?;
    let o2 = quote_exact_input_single_at(provider, quoter, p.quote, token, p.fee, a2, block).await?;
    let (rq, rt) = crate::sim_arb::fit_v3_virtual_reserves(p.fee, a1, o1, a2, o2)
        .ok_or_else(|| format!("fit v3 fail pool={:#x} fee={}", p.pool, p.fee))?;
    p.reserve_quote = rq;
    p.reserve_token = rt;
    Ok(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn addr(hex: &str) -> Address {
        Address::from_str(hex).unwrap()
    }

    #[test]
    fn calldata_layout_selector_then_5_static_words_no_offset() {
        let token_in = addr("0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c");
        let token_out = addr("0x111111111111111111111111111111111111beef");
        let calldata = build_quote_calldata(token_in, token_out, 2500, U256::from(1_000_000u64));
        assert_eq!(calldata.len(), 4 + 32 * 5);
        assert_eq!(&calldata[0..4], SEL_QUOTE_EXACT_INPUT_SINGLE.as_slice());
        assert_eq!(&calldata[4 + 12..4 + 32], token_in.as_slice());
        assert_eq!(&calldata[4 + 32 + 12..4 + 64], token_out.as_slice());
        // amountIn (word idx 2) = 1_000_000
        let amount_in_word = &calldata[4 + 64..4 + 96];
        assert_eq!(U256::from_be_slice(amount_in_word), U256::from(1_000_000u64));
        // fee (word idx 3) = 2500, right-aligned uint24
        let fee_word = &calldata[4 + 96..4 + 128];
        assert_eq!(fee_word[..29], [0u8; 29]);
        assert_eq!(u32::from_be_bytes([0, fee_word[29], fee_word[30], fee_word[31]]), 2500);
        // sqrtPriceLimitX96 (word idx 4) = 0
        assert_eq!(&calldata[4 + 128..4 + 160], [0u8; 32]);
    }

    #[test]
    fn decode_quote_amount_out_reads_first_word() {
        let mut ret = vec![0u8; 0];
        ret.extend_from_slice(&U256::from(123_456u64).to_be_bytes::<32>()); // amountOut
        ret.extend_from_slice(&[0u8; 32]); // sqrtPriceX96After (bo qua)
        ret.extend_from_slice(&[0u8; 32]); // initializedTicksCrossed (bo qua)
        ret.extend_from_slice(&[0u8; 32]); // gasEstimate (bo qua)
        assert_eq!(decode_quote_amount_out(&ret), Some(U256::from(123_456u64)));
    }

    #[test]
    fn decode_quote_amount_out_too_short_is_none() {
        assert_eq!(decode_quote_amount_out(&[0u8; 10]), None);
    }

    /// In ra selector THẬT tính bởi chính crate `alloy` (keccak256 thật, không
    /// phải số nhớ tay) — dán vào BAOCAO làm bằng chứng, không so với hằng số
    /// hardcode nào (phiên này không có nguồn xác nhận độc lập giá trị hex cụ
    /// thể ngoài chữ ký hàm đã fetch, tránh bịa "well_known" chưa kiểm).
    #[test]
    fn print_computed_selector_for_evidence() {
        println!("quoteExactInputSingle selector = 0x{}", hex_string(&*SEL_QUOTE_EXACT_INPUT_SINGLE));
    }

    fn hex_string(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// `#[ignore]` — chỉ chạy thủ công khi có RPC sống (giống quy ước
    /// `pool.rs::real_rpc_v2_get_pair_wbnb_usdt`). QuoterV2 + V3 Factory đã
    /// pin (`venues::V3_QUOTER_V2_ADDRESS`/`V3_FACTORY_ADDRESS`), WBNB/USDT
    /// là token BSC nổi tiếng chỉ để verify quote thật, không phải victim
    /// bịa. Dò cả 4 fee tier PIN, lấy tier đầu tiên có pool + quote > 0
    /// (không giả định trước tier nào có thanh khoản).
    #[tokio::test]
    #[ignore]
    async fn real_rpc_v3_quote_wbnb_to_usdt() {
        use crate::pool::{resolve_v3_pool, V3_FEE_TIERS};
        use crate::venues::{V3_FACTORY_ADDRESS, V3_QUOTER_V2_ADDRESS, WBNB_ADDRESS};
        use alloy::providers::{Provider, ProviderBuilder};

        let wbnb = addr(WBNB_ADDRESS);
        let usdt = addr("0x55d398326f99059ff775485246999027b3197955");
        let factory = addr(V3_FACTORY_ADDRESS);
        let quoter = addr(V3_QUOTER_V2_ADDRESS);

        let provider = ProviderBuilder::new()
            .connect("https://bsc-dataseed.binance.org/")
            .await
            .expect("ket noi RPC cong khai that bai");
        let chain_id = provider.get_chain_id().await.expect("eth_chainId that bai");
        assert_eq!(chain_id, 56);

        let (_pool, fee) = resolve_v3_pool(&provider, factory, usdt)
            .await
            .expect("eth_call getPool that bai")
            .expect("USDT/WBNB phai co pool V3 that tren mainnet o it nhat 1 trong 4 fee tier da pin");
        assert!(V3_FEE_TIERS.contains(&fee));

        let amount_in = U256::from(10_000_000_000_000_000u64); // 0.01 WBNB
        let amount_out = quote_exact_input_single(&provider, quoter, wbnb, usdt, fee, amount_in)
            .await
            .expect("quoteExactInputSingle that bai");
        println!("V3 QuoterV2 quote: 0.01 WBNB -> {amount_out} USDT wei (fee tier={fee})");
        assert!(amount_out > U256::ZERO);
    }
}

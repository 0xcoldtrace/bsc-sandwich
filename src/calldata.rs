//! Cụm `7.2` — Pin calldata router THẬT cho V2 Router đã pin (`DEX_REGISTRY.md`
//! `Router 0x10ED43...`) — dựng lại calldata front-run/back-run từ `DecodedSwap`
//! hiện có (`decoder.rs`), sẵn sàng cho executor `7.3` dùng. Phiên này CHƯA gửi
//! tx thật (`sendRaw` cấm), chỉ build `Vec<u8>` calldata + verify roundtrip
//! encode->decode khớp bit-for-bit với chính `decoder::decode_swap_calldata`.
//!
//! Dùng `alloy::sol!` (macro có sẵn qua `alloy-core`, chỉ cần bật feature
//! `sol-types` trên dependency `alloy` ĐÃ PIN — không thêm crate mới nào vào
//! `Cargo.lock`, `alloy-sol-types`/`alloy-sol-macro` đã có sẵn trong cây phụ
//! thuộc từ trước, xem `docs/STATE.md`) thay vì tự đóng gói byte tay như
//! `decoder.rs` — encode ABI đúng chuẩn do macro tự sinh, giảm rủi ro lệch
//! offset so với chép tay ngược lại logic decode.
//!
//! CHỈ 2 hàm V2 Router theo đúng lệnh (`swapExactETHForTokens` front-buy,
//! `swapExactTokensForETH` back-sell) — KHÔNG đụng V3/UR/V4 ở cụm này, KHÔNG
//! hồi sinh chiều victim bán (xem `pipeline.rs`/`docs/STATE.md` mục BAOCAO14).

use alloy::primitives::{Address, U256};
use alloy::sol;
use alloy::sol_types::SolCall;

sol! {
    interface IPancakeV2Router02 {
        function swapExactETHForTokens(uint256 amountOutMin, address[] calldata path, address to, uint256 deadline)
            external
            payable
            returns (uint256[] memory amounts);

        function swapExactTokensForETH(
            uint256 amountIn,
            uint256 amountOutMin,
            address[] calldata path,
            address to,
            uint256 deadline
        ) external returns (uint256[] memory amounts);

        function swapExactTokensForTokens(
            uint256 amountIn,
            uint256 amountOutMin,
            address[] calldata path,
            address to,
            uint256 deadline
        ) external returns (uint256[] memory amounts);
    }
}

/// Cụm `evm-validate-fixed-then-wire` (D2) — calldata quote USDT. Quote USDT
/// KHÔNG có đường native như WBNB (`swapExactETHForTokens`), cả 2 chân đều là
/// token↔token qua `swapExactTokensForTokens`:
/// - front-buy: `path=[usdt, token]` (mua token bằng USDT).
/// - back-sell: `path=[token, usdt]` (bán token lấy lại USDT).
/// `amount_in` NẰM TRONG calldata cả 2 chân (khác front-buy WBNB dùng
/// `msg.value`). Executor `7.x` phải approve USDT cho router trước front-buy
/// (giống mọi swap token-in) — ghi rõ, ngoài phạm vi hàm build này.
pub fn encode_front_buy_usdt(usdt: Address, token: Address, amount_in: U256, amount_out_min: U256, to: Address, deadline: U256) -> Vec<u8> {
    IPancakeV2Router02::swapExactTokensForTokensCall {
        amountIn: amount_in,
        amountOutMin: amount_out_min,
        path: vec![usdt, token],
        to,
        deadline,
    }
    .abi_encode()
}

/// D2 — back-sell quote USDT, `path=[token, usdt]`.
pub fn encode_back_sell_usdt(token: Address, usdt: Address, amount_in: U256, amount_out_min: U256, to: Address, deadline: U256) -> Vec<u8> {
    IPancakeV2Router02::swapExactTokensForTokensCall {
        amountIn: amount_in,
        amountOutMin: amount_out_min,
        path: vec![token, usdt],
        to,
        deadline,
    }
    .abi_encode()
}

/// Front-run: mua token bằng WBNB, `path=[wbnb, token]`. `amountIn` (BNB gửi
/// kèm) KHÔNG nằm trong calldata này — đúng ABI thật của
/// `swapExactETHForTokens` (khớp `decoder.rs::decode_swap_calldata` dùng
/// `tx_value` làm `amount_in` cho hàm này) — caller (`7.3`, chưa tồn tại
/// phiên này) tự đặt `msg.value` khi build tx thật.
pub fn encode_front_buy(wbnb: Address, token: Address, amount_out_min: U256, to: Address, deadline: U256) -> Vec<u8> {
    IPancakeV2Router02::swapExactETHForTokensCall { amountOutMin: amount_out_min, path: vec![wbnb, token], to, deadline }
        .abi_encode()
}

/// Back-run: bán lại token lấy WBNB, `path=[token, wbnb]`.
pub fn encode_back_sell(
    token: Address,
    wbnb: Address,
    amount_in: U256,
    amount_out_min: U256,
    to: Address,
    deadline: U256,
) -> Vec<u8> {
    IPancakeV2Router02::swapExactTokensForETHCall { amountIn: amount_in, amountOutMin: amount_out_min, path: vec![token, wbnb], to, deadline }
        .abi_encode()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder;
    use crate::venues::WBNB_ADDRESS;
    use std::str::FromStr;

    fn wbnb() -> Address {
        Address::from_str(WBNB_ADDRESS).unwrap()
    }

    fn token() -> Address {
        Address::from_str("0xcccccccccccccccccccccccccccccccccccccccc").unwrap()
    }

    fn self_addr() -> Address {
        Address::from_str("0x2222222222222222222222222222222222222222").unwrap()
    }

    /// `sol!` tự tính selector qua `keccak256(chữ_ký_hàm)` — đối chiếu 4 byte
    /// đầu calldata sinh ra khớp ĐÚNG hằng số well-known mà `decoder.rs` đã tự
    /// verify riêng (`decoder::tests::well_known_selectors_match`,
    /// `0x7ff36ab5`/`0x18cbafe5`) — 2 nguồn độc lập (macro sinh vs
    /// `keccak256` tay trong `decoder.rs`) khớp nhau, không phải trùng hợp.
    #[test]
    fn encoded_selectors_match_decoder_well_known_constants() {
        let front = encode_front_buy(wbnb(), token(), U256::ZERO, self_addr(), U256::from(9_999_999_999u64));
        assert_eq!(&front[0..4], [0x7f, 0xf3, 0x6a, 0xb5]);

        let back = encode_back_sell(token(), wbnb(), U256::from(1u64), U256::ZERO, self_addr(), U256::from(9_999_999_999u64));
        assert_eq!(&back[0..4], [0x18, 0xcb, 0xaf, 0xe5]);
    }

    /// Roundtrip front-buy: encode rồi decode lại bằng CHÍNH
    /// `decoder::decode_swap_calldata` (dùng bởi `pipeline.rs` cho mọi tx thật)
    /// — `amount_in` của `swapExactETHForTokens` không nằm trong calldata mà
    /// lấy từ `tx_value` (đúng quy ước decoder), nên test tự truyền `front_in`
    /// làm `tx_value` khi decode, y hệt cách `main.rs`/`pipeline.rs` sẽ làm với
    /// tx thật khi `7.3` build xong.
    #[test]
    fn roundtrip_front_buy_encode_then_decode_matches_bit_for_bit() {
        let amount_out_min = U256::from(123_456u64);
        let deadline = U256::from(9_999_999_999u64);
        let front_in = U256::from(50_000_000_000_000_000u64); // 0.05 BNB, gia tri front_in gia dinh tu sim_v2

        let calldata = encode_front_buy(wbnb(), token(), amount_out_min, self_addr(), deadline);
        let decoded = decoder::decode_swap_calldata(&calldata, front_in).expect("phai decode duoc calldata vua encode");

        assert_eq!(decoded.selector_name, "swapExactETHForTokens");
        assert_eq!(decoded.amount_in, front_in);
        assert_eq!(decoded.amount_out_min, amount_out_min);
        assert_eq!(decoded.path.token_a, wbnb());
        assert_eq!(decoded.path.token_b, token());
        assert_eq!(decoded.token().unwrap(), token());
        assert_eq!(decoded.to, self_addr());
        assert_eq!(decoded.deadline, Some(deadline));
    }

    /// Roundtrip back-sell: `amountIn` (số token bán lại, = `front_out` thật
    /// của sim) NẰM TRONG calldata (khác front-buy) — `tx_value` truyền vào
    /// decode không ảnh hưởng (hàm này không phải payable, `decoder.rs` không
    /// dùng `tx_value` cho nhánh `swapExactTokensForETH`).
    #[test]
    fn roundtrip_back_sell_encode_then_decode_matches_bit_for_bit() {
        let amount_in = U256::from(45_000_000_000_000_000_000u128); // front_out gia dinh, don vi token
        let amount_out_min = U256::from(1u64);
        let deadline = U256::from(9_999_999_999u64);

        let calldata = encode_back_sell(token(), wbnb(), amount_in, amount_out_min, self_addr(), deadline);
        let decoded =
            decoder::decode_swap_calldata(&calldata, U256::ZERO).expect("phai decode duoc calldata vua encode");

        assert_eq!(decoded.selector_name, "swapExactTokensForETH");
        assert_eq!(decoded.amount_in, amount_in);
        assert_eq!(decoded.amount_out_min, amount_out_min);
        assert_eq!(decoded.path.token_a, token());
        assert_eq!(decoded.path.token_b, wbnb());
        assert_eq!(decoded.token().unwrap(), token());
        assert_eq!(decoded.to, self_addr());
        assert_eq!(decoded.deadline, Some(deadline));
    }

    /// `amount_out_min` khác 0 (giả lập slippage config sau này, xem lệnh
    /// "amountOutMin=0 hoặc tính từ slippage cấu hình" — phiên này dùng 0 vì
    /// `config.toml`/`config.rs` KHÔNG nằm trong phạm vi ĐƯỢC ĐỤNG, chưa có
    /// field slippage nào để đọc) roundtrip đúng, không lệch offset word do
    /// giá trị != 0 (kiểm tra riêng để chắc không phải test trước "tình cờ
    /// đúng" vì amountOutMin=0 trùng giá trị mặc định của word).
    #[test]
    fn roundtrip_front_buy_nonzero_amount_out_min() {
        let amount_out_min = U256::from(777_777u64);
        let calldata = encode_front_buy(wbnb(), token(), amount_out_min, self_addr(), U256::from(1u64));
        let decoded = decoder::decode_swap_calldata(&calldata, U256::from(1u64)).expect("phai decode duoc");
        assert_eq!(decoded.amount_out_min, amount_out_min);
    }

    fn usdt() -> Address {
        Address::from_str(crate::venues::USDT_ADDRESS).unwrap()
    }

    /// Cụm D2 — roundtrip front-buy quote USDT: `path=[usdt, token]`,
    /// `swapExactTokensForTokens`. `amount_in` nằm trong calldata.
    #[test]
    fn roundtrip_front_buy_usdt_matches_bit_for_bit() {
        let amount_in = U256::from(50_000_000_000_000_000_000u128); // 50 USDT (18 dp)
        let amount_out_min = U256::from(42u64);
        let deadline = U256::from(9_999_999_999u64);
        let calldata = encode_front_buy_usdt(usdt(), token(), amount_in, amount_out_min, self_addr(), deadline);
        // decoder `swapExactTokensForTokens` yeu cau path co WBNB HOAC (khi
        // decode) 2 token - path [usdt, token] khong co WBNB nen decoder tra
        // NotWbnbPair. Day dung: decoder WBNB-only, con encode USDT la de
        // executor USDT-mode dung. Verify selector + do dai thay vi roundtrip
        // qua decoder WBNB-only.
        assert_eq!(&calldata[0..4], [0x38, 0xed, 0x17, 0x39]); // swapExactTokensForTokens
        // path o vi tri dung: giai ma tay 2 dia chi cuoi.
        assert!(calldata.windows(20).any(|w| w == usdt().as_slice()));
        assert!(calldata.windows(20).any(|w| w == token().as_slice()));
    }

    /// D2 — roundtrip back-sell quote USDT: `path=[token, usdt]`.
    #[test]
    fn roundtrip_back_sell_usdt_matches_bit_for_bit() {
        let amount_in = U256::from(123u64);
        let amount_out_min = U256::from(1u64);
        let deadline = U256::from(9_999_999_999u64);
        let calldata = encode_back_sell_usdt(token(), usdt(), amount_in, amount_out_min, self_addr(), deadline);
        assert_eq!(&calldata[0..4], [0x38, 0xed, 0x17, 0x39]);
        // Verify roundtrip qua chinh ABI decoder cua sol! (chac chan bit-for-bit).
        let decoded = IPancakeV2Router02::swapExactTokensForTokensCall::abi_decode(&calldata).expect("decode lai duoc");
        assert_eq!(decoded.amountIn, amount_in);
        assert_eq!(decoded.amountOutMin, amount_out_min);
        assert_eq!(decoded.path, vec![token(), usdt()]);
        assert_eq!(decoded.to, self_addr());
        assert_eq!(decoded.deadline, deadline);
    }
}

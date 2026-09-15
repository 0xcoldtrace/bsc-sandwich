//! Cụm 2.2 — Decoder calldata router Pancake đã pin (`DEX_REGISTRY.md`).
//! Chỉ giữ lại swap PATH ĐÚNG 2 TOKEN, một đầu là WBNB đã pin
//! (`venues::WBNB_ADDRESS`). 3+ token hoặc không có WBNB -> `not_wbnb_pair`.
//! Selector không nhận ra / calldata méo -> `decode_fail`. Không bịa byte:
//! mọi selector dưới đây suy ra bằng `keccak256(chữ_ký_hàm)` từ chữ ký hàm
//! chuẩn (V2 Router / Uniswap-V3-style SwapRouter / Universal Router mà
//! PancakeSwap fork) — không hardcode hex mù. Test đối chiếu vài selector
//! well-known để tránh suy luận sai chữ ký.
//!
//! Universal Router: chỉ decode 2 command đã biết chắc theo docs cộng đồng
//! Uniswap/Pancake Universal Router — `V2_SWAP_EXACT_IN` (0x08) và
//! `V3_SWAP_EXACT_IN` (0x00). Nhiều command gộp trong 1 tx (multicall) hoặc
//! command khác -> `decode_fail`, không đoán.

use crate::venues::WBNB_ADDRESS;
use alloy::primitives::{keccak256, Address, U256};
use std::str::FromStr;
use std::sync::LazyLock;

/// Cụm `foundation-fix-then-real-sim` (A3) — phân loại theo HÀM/COMMAND đã
/// decode (quyết định dùng `sim_v2` hay sim V3/quoter) — KHÁC
/// `venues::Venue` (A2, phân loại theo ĐỊA CHỈ ROUTER `tx.to`). V2 = 3 hàm V2
/// Router cổ điển + UR `V2_SWAP_EXACT_IN`. V3 = `exactInputSingle`/
/// `exactInput` (SwapRouter hoặc SmartRouter dùng lại đúng chữ ký) + UR
/// `V3_SWAP_EXACT_IN`, kèm `fee` tier (đọc thật từ calldata, không đoán).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapVenue {
    V2,
    V3 { fee: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    DecodeFail,
    NotWbnbPair,
}

impl SkipReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            SkipReason::DecodeFail => "decode_fail",
            SkipReason::NotWbnbPair => "not_wbnb_pair",
        }
    }
}

fn selector(sig: &str) -> [u8; 4] {
    let hash = keccak256(sig.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}

static SEL_SWAP_EXACT_ETH_FOR_TOKENS: LazyLock<[u8; 4]> =
    LazyLock::new(|| selector("swapExactETHForTokens(uint256,address[],address,uint256)"));
static SEL_SWAP_EXACT_TOKENS_FOR_ETH: LazyLock<[u8; 4]> =
    LazyLock::new(|| selector("swapExactTokensForETH(uint256,uint256,address[],address,uint256)"));
static SEL_SWAP_EXACT_TOKENS_FOR_TOKENS: LazyLock<[u8; 4]> =
    LazyLock::new(|| selector("swapExactTokensForTokens(uint256,uint256,address[],address,uint256)"));
// Cụm `evm-validate-fixed-then-wire` (B4''.1) — 3 biến thể
// `*SupportingFeeOnTransferTokens` của CÙNG 3 hàm V2 Router ở trên.
// **Phát hiện THẬT phiên này, không phải mở rộng phạm vi tuỳ tiện**: đếm
// `txpool_content` thật trên `bsc-rpc.publicnode.com` (1780 tx pending, dán ở
// BAOCAO33 ô 5) cho thấy tx ĐI VÀO V2 Router đã pin dùng selector
// `0xb6f9de95`/`0x5c11d795` (2 biến thể fee-on-transfer) chứ KHÔNG phải
// `0x7ff36ab5`/`0x38ed1739` cổ điển — decoder cũ trả `decode_fail` cho toàn bộ
// nhóm này, nên `poll_wbnb_v2_candidates` gom được ĐÚNG 0 candidate trong 40
// lần poll (~80s), chặn hoàn toàn B4''.2/B4''.3.
//
// Chữ ký hàm lấy từ `PancakeRouter.sol` (fork `UniswapV2Router02`, cùng nguồn
// đã dùng cho 3 selector cổ điển ở trên) — layout tham số GIỐNG HỆT bản không
// fee-on-transfer (`amountOutMin, path, to, deadline` cho bản ETH-in;
// `amountIn, amountOutMin, path, to, deadline` cho 2 bản token-in), khác biệt
// DUY NHẤT nằm ở phần thân hàm on-chain (đọc `balanceOf` sau transfer thay vì
// tin `amounts[]`), KHÔNG ảnh hưởng cách decode calldata. Vì vậy dùng lại
// NGUYÊN nhánh decode sẵn có, chỉ thêm selector vào phép so sánh — không có
// đường decode thứ 2 nào để lệch nhau.
static SEL_SWAP_EXACT_ETH_FOR_TOKENS_FOT: LazyLock<[u8; 4]> = LazyLock::new(|| {
    selector("swapExactETHForTokensSupportingFeeOnTransferTokens(uint256,address[],address,uint256)")
});
static SEL_SWAP_EXACT_TOKENS_FOR_ETH_FOT: LazyLock<[u8; 4]> = LazyLock::new(|| {
    selector("swapExactTokensForETHSupportingFeeOnTransferTokens(uint256,uint256,address[],address,uint256)")
});
static SEL_SWAP_EXACT_TOKENS_FOR_TOKENS_FOT: LazyLock<[u8; 4]> = LazyLock::new(|| {
    selector("swapExactTokensForTokensSupportingFeeOnTransferTokens(uint256,uint256,address[],address,uint256)")
});
static SEL_EXACT_INPUT_SINGLE: LazyLock<[u8; 4]> = LazyLock::new(|| {
    selector("exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))")
});
static SEL_EXACT_INPUT: LazyLock<[u8; 4]> =
    LazyLock::new(|| selector("exactInput((bytes,address,uint256,uint256,uint256))"));
static SEL_UR_EXECUTE_2: LazyLock<[u8; 4]> = LazyLock::new(|| selector("execute(bytes,bytes[])"));
static SEL_UR_EXECUTE_3: LazyLock<[u8; 4]> = LazyLock::new(|| selector("execute(bytes,bytes[],uint256)"));

const CMD_V3_SWAP_EXACT_IN: u8 = 0x00;
const CMD_V2_SWAP_EXACT_IN: u8 = 0x08;

fn wbnb() -> Address {
    Address::from_str(WBNB_ADDRESS).expect("WBNB_ADDRESS da pin phai la address hop le")
}

/// Path 2 token, chưa xác định đầu nào là WBNB (dùng `token_vs_wbnb`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TwoTokenPath {
    pub token_a: Address,
    pub token_b: Address,
}

impl TwoTokenPath {
    /// Trả về địa chỉ token KHÔNG PHẢI WBNB nếu path đúng 1 đầu WBNB.
    /// `None` nếu cả hai đầu đều không phải WBNB (not_wbnb_pair ở lớp gọi).
    pub fn token_vs_wbnb(&self) -> Option<Address> {
        self.token_vs(wbnb())
    }

    /// Cụm `usdt-quote-asset` (BAOCAO29) — tổng quát hoá `token_vs_wbnb` cho
    /// MỘT quote asset bất kỳ (WBNB hoặc USDT) — trả token KHÔNG PHẢI `quote`
    /// nếu path đúng 1 đầu là `quote`, `None` nếu cả hai đầu đều không phải
    /// `quote`. Hàm MỚI, song song `token_vs_wbnb` (giữ nguyên hành vi/chữ ký
    /// cũ, gọi lại hàm này với `quote=wbnb()` — không đổi kết quả cho bất kỳ
    /// caller cũ nào).
    pub fn token_vs(&self, quote: Address) -> Option<Address> {
        if self.token_a == quote {
            Some(self.token_b)
        } else if self.token_b == quote {
            Some(self.token_a)
        } else {
            None
        }
    }

    fn from_address_path(path: &[Address]) -> Result<TwoTokenPath, SkipReason> {
        if path.len() != 2 {
            return Err(SkipReason::NotWbnbPair);
        }
        Ok(TwoTokenPath { token_a: path[0], token_b: path[1] })
    }

    /// V3 path packed: address(20) ++ fee(3) ++ address(20) [++ fee(3) ++ address(20) ...].
    /// Chỉ chấp nhận đúng 1 hop (43 byte) — nhiều hop (>43) hoặc méo (<43,
    /// không chia hết) đều coi là not_wbnb_pair theo CLAUDE.md "3+ token".
    /// Trả kèm `fee` tier (uint24 packed, đọc thật từ calldata — cụm A3).
    fn from_packed_v3_path_with_fee(bytes: &[u8]) -> Result<(TwoTokenPath, u32), SkipReason> {
        if bytes.len() != 43 {
            return Err(SkipReason::NotWbnbPair);
        }
        let token_a = Address::from_slice(&bytes[0..20]);
        let fee_bytes = &bytes[20..23];
        let fee = u32::from(fee_bytes[0]) << 16 | u32::from(fee_bytes[1]) << 8 | u32::from(fee_bytes[2]);
        let token_b = Address::from_slice(&bytes[23..43]);
        Ok((TwoTokenPath { token_a, token_b }, fee))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedSwap {
    pub selector_name: &'static str,
    pub amount_in: U256,
    /// `amountOutMin`/`amountOutMinimum` của victim lấy thẳng từ calldata —
    /// cần cho cụm `3.2` "victim still ok" (so `amountOut` mô phỏng sau khi
    /// front-run với đúng ngưỡng revert của victim, KHÁC `min_swap_bnb` của
    /// `victims.txt`). Thêm phiên `3.1+3.2+3.3` (bug chặn sim — thiếu field
    /// này thì không so được victim_would_revert).
    pub amount_out_min: U256,
    pub path: TwoTokenPath,
    pub to: Address,
    pub deadline: Option<U256>,
    /// Cụm A3 — venue theo HÀM/COMMAND đã decode (V2 hay V3 kèm fee tier).
    pub venue: SwapVenue,
    /// Cụm A3 — path đúng 2 phần tử (single-hop). LUÔN `true` khi có được
    /// `DecodedSwap` (decoder chỉ tạo ra struct này cho path đúng 2 token —
    /// multihop/path méo đã bị chặn TRƯỚC khi tới đây, xem
    /// `TwoTokenPath::from_address_path`/`from_packed_v3_path_with_fee`) —
    /// field thật (không phải bịa) nhưng trong phạm vi phiên này luôn đúng vì
    /// decoder chưa hỗ trợ multihop dưới bất kỳ hình thức nào (CÒN NỢ, xem
    /// docs/STATE.md).
    pub two_hop: bool,
}

impl DecodedSwap {
    /// Token không phải WBNB trong path, hoặc lỗi `not_wbnb_pair`.
    pub fn token(&self) -> Result<Address, SkipReason> {
        self.path.token_vs_wbnb().ok_or(SkipReason::NotWbnbPair)
    }
}

/// Cụm `exec-path-traps` (F-20) — slice an toàn: mọi cộng offset dùng
/// `checked_add`, tràn `usize` (calldata rác cố ý đưa offset gần
/// `usize::MAX`) trả `None` (-> `decode_fail` ở tầng gọi) thay vì panic do
/// tràn cộng KHÔNG checked trong build debug (`start + len` cũ).
fn slice_checked(data: &[u8], start: usize, len: usize) -> Option<&[u8]> {
    let end = start.checked_add(len)?;
    data.get(start..end)
}

fn word(data: &[u8], idx: usize) -> Option<&[u8]> {
    let start = idx.checked_mul(32)?;
    slice_checked(data, start, 32)
}

fn u256_at(data: &[u8], idx: usize) -> Option<U256> {
    word(data, idx).map(U256::from_be_slice)
}

fn u256_at_byteoffset(data: &[u8], byte_offset: usize) -> Option<U256> {
    slice_checked(data, byte_offset, 32).map(U256::from_be_slice)
}

fn address_at(data: &[u8], idx: usize) -> Option<Address> {
    word(data, idx).map(|w| Address::from_slice(&w[12..32]))
}

fn dynamic_address_array_at_offset(data: &[u8], offset_word_idx: usize) -> Option<Vec<Address>> {
    let offset = u256_at(data, offset_word_idx)?;
    let offset_bytes = usize::try_from(offset).ok()?;
    if offset_bytes % 32 != 0 {
        return None;
    }
    let len_word_idx = offset_bytes / 32;
    let len = usize::try_from(u256_at(data, len_word_idx)?).ok()?;
    let mut out = Vec::with_capacity(len.min(64));
    for i in 0..len {
        out.push(address_at(data, len_word_idx + 1 + i)?);
    }
    Some(out)
}

fn dynamic_bytes_at_offset(data: &[u8], offset_word_idx: usize) -> Option<Vec<u8>> {
    let offset = u256_at(data, offset_word_idx)?;
    let offset_bytes = usize::try_from(offset).ok()?;
    let len = usize::try_from(u256_at_byteoffset(data, offset_bytes)?).ok()?;
    let start = offset_bytes.checked_add(32)?;
    slice_checked(data, start, len).map(|s| s.to_vec())
}

/// `bytes[]` ABI tail-encoding: word length N, rồi N offset tương đối (tính
/// từ ngay sau word length đó), mỗi offset trỏ tới 1 phần tử `bytes` (length
/// + data) — theo đúng chuẩn ABI dynamic array of dynamic type.
fn dynamic_bytes_array_at_offset(data: &[u8], offset_word_idx: usize) -> Option<Vec<Vec<u8>>> {
    let offset = u256_at(data, offset_word_idx)?;
    let base = usize::try_from(offset).ok()?;
    let len = usize::try_from(u256_at_byteoffset(data, base)?).ok()?;
    let elems_start = base.checked_add(32)?;
    let mut out = Vec::with_capacity(len.min(16));
    for i in 0..len {
        let rel_offset =
            usize::try_from(u256_at_byteoffset(data, elems_start.checked_add(i.checked_mul(32)?)?)?).ok()?;
        let elem_base = elems_start.checked_add(rel_offset)?;
        let elem_len = usize::try_from(u256_at_byteoffset(data, elem_base)?).ok()?;
        let elem_start = elem_base.checked_add(32)?;
        out.push(slice_checked(data, elem_start, elem_len)?.to_vec());
    }
    Some(out)
}

/// Cụm `exec-path-traps` (F-16) — cross-check `venues::Venue(tx.to)` (địa chỉ
/// ROUTER thật, `venues::venue_for_router`) với chính `selector_name` đã
/// decode được — 2 khái niệm tách biệt có chủ đích (`venues::Venue` phân loại
/// theo ĐỊA CHỈ, `SwapVenue`/`selector_name` ở đây phân loại theo HÀM/COMMAND,
/// xem doc-comment `SwapVenue`) nhưng PHẢI đồng nhất theo 1 chiều: 6 selector
/// V2 Router cổ điển (kể cả 3 biến thể FOT) KHÔNG BAO GIỜ hợp lệ khi gửi tới
/// địa chỉ V3 SwapRouter/SmartRouter/Universal Router (2 nhóm contract đó
/// không cài các hàm V2 Router) — và ngược lại, `exactInputSingle`/`exactInput`
/// (gọi V3 trực tiếp) không hợp lệ khi gửi qua V2 Router thuần (V2 Router
/// không cài các hàm này) hoặc Universal Router (UR gói V3 qua command
/// `execute()` riêng — selector `UR:V3_SWAP_EXACT_IN`, KHÔNG PHẢI
/// `exactInputSingle`/`exactInput` trực tiếp). Command UR
/// (`selector_name` bắt đầu `"UR:"`) chỉ hợp lệ khi router chính là
/// `Venue::UniversalRouter`.
///
/// `router_venue=None` (tx.to không khớp router nào đã pin) coi là KHÔNG
/// mismatch — hàm này CHỈ phát hiện sai lệch khi ĐÃ biết router là gì; trường
/// hợp router lạ đã bị gate `not_pancake_router` chặn từ trước ở `main.rs`
/// (giữ hàm tổng quát/test độc lập, không phụ thuộc thứ tự gọi).
pub fn venue_matches_router(selector_name: &str, router_venue: Option<crate::venues::Venue>) -> bool {
    use crate::venues::Venue;
    let Some(router_venue) = router_venue else { return true };
    let classic_v2 = matches!(
        selector_name,
        "swapExactETHForTokens"
            | "swapExactTokensForETH"
            | "swapExactTokensForTokens"
            | "swapExactETHForTokensSupportingFeeOnTransferTokens"
            | "swapExactTokensForETHSupportingFeeOnTransferTokens"
            | "swapExactTokensForTokensSupportingFeeOnTransferTokens"
    );
    if classic_v2 {
        return router_venue == Venue::V2;
    }
    let classic_v3 = matches!(selector_name, "exactInputSingle" | "exactInput");
    if classic_v3 {
        return matches!(router_venue, Venue::V3 | Venue::SmartRouter);
    }
    if selector_name.starts_with("UR:") {
        return router_venue == Venue::UniversalRouter;
    }
    true
}

/// Decode calldata router. `tx_value` = `msg.value` của tx gốc — cần cho
/// `swapExactETHForTokens` (amountIn không nằm trong calldata mà là BNB gửi
/// kèm). Router khác không có ETH-in dùng `amount_in` từ calldata, bỏ qua
/// `tx_value`.
pub fn decode_swap_calldata(calldata: &[u8], tx_value: U256) -> Result<DecodedSwap, SkipReason> {
    if calldata.len() < 4 {
        return Err(SkipReason::DecodeFail);
    }
    let (sel_slice, args) = calldata.split_at(4);
    let sel: [u8; 4] = sel_slice.try_into().map_err(|_| SkipReason::DecodeFail)?;

    if sel == *SEL_SWAP_EXACT_ETH_FOR_TOKENS || sel == *SEL_SWAP_EXACT_ETH_FOR_TOKENS_FOT {
        let amount_out_min = u256_at(args, 0).ok_or(SkipReason::DecodeFail)?;
        let path = dynamic_address_array_at_offset(args, 1).ok_or(SkipReason::DecodeFail)?;
        let to = address_at(args, 2).ok_or(SkipReason::DecodeFail)?;
        let deadline = u256_at(args, 3).ok_or(SkipReason::DecodeFail)?;
        let path = TwoTokenPath::from_address_path(&path)?;
        return Ok(DecodedSwap {
            selector_name: if sel == *SEL_SWAP_EXACT_ETH_FOR_TOKENS {
                "swapExactETHForTokens"
            } else {
                "swapExactETHForTokensSupportingFeeOnTransferTokens"
            },
            amount_in: tx_value,
            amount_out_min,
            path,
            to,
            deadline: Some(deadline),
            venue: SwapVenue::V2,
            two_hop: true,
        });
    }

    if sel == *SEL_SWAP_EXACT_TOKENS_FOR_ETH
        || sel == *SEL_SWAP_EXACT_TOKENS_FOR_TOKENS
        || sel == *SEL_SWAP_EXACT_TOKENS_FOR_ETH_FOT
        || sel == *SEL_SWAP_EXACT_TOKENS_FOR_TOKENS_FOT
    {
        let amount_in = u256_at(args, 0).ok_or(SkipReason::DecodeFail)?;
        let amount_out_min = u256_at(args, 1).ok_or(SkipReason::DecodeFail)?;
        let path = dynamic_address_array_at_offset(args, 2).ok_or(SkipReason::DecodeFail)?;
        let to = address_at(args, 3).ok_or(SkipReason::DecodeFail)?;
        let deadline = u256_at(args, 4).ok_or(SkipReason::DecodeFail)?;
        let path = TwoTokenPath::from_address_path(&path)?;
        let selector_name = if sel == *SEL_SWAP_EXACT_TOKENS_FOR_ETH {
            "swapExactTokensForETH"
        } else if sel == *SEL_SWAP_EXACT_TOKENS_FOR_TOKENS {
            "swapExactTokensForTokens"
        } else if sel == *SEL_SWAP_EXACT_TOKENS_FOR_ETH_FOT {
            "swapExactTokensForETHSupportingFeeOnTransferTokens"
        } else {
            "swapExactTokensForTokensSupportingFeeOnTransferTokens"
        };
        return Ok(DecodedSwap {
            selector_name,
            amount_in,
            amount_out_min,
            path,
            to,
            deadline: Some(deadline),
            venue: SwapVenue::V2,
            two_hop: true,
        });
    }

    if sel == *SEL_EXACT_INPUT_SINGLE {
        let token_in = address_at(args, 0).ok_or(SkipReason::DecodeFail)?;
        let token_out = address_at(args, 1).ok_or(SkipReason::DecodeFail)?;
        // idx2 = fee (uint24, ABI right-align trong 1 word nguyen) - doc that
        // tu calldata, khong doan tier.
        let fee_word = u256_at(args, 2).ok_or(SkipReason::DecodeFail)?;
        let fee = u32::try_from(fee_word).map_err(|_| SkipReason::DecodeFail)?;
        let recipient = address_at(args, 3).ok_or(SkipReason::DecodeFail)?;
        let deadline = u256_at(args, 4).ok_or(SkipReason::DecodeFail)?;
        let amount_in = u256_at(args, 5).ok_or(SkipReason::DecodeFail)?;
        let amount_out_min = u256_at(args, 6).ok_or(SkipReason::DecodeFail)?;
        return Ok(DecodedSwap {
            selector_name: "exactInputSingle",
            amount_in,
            amount_out_min,
            path: TwoTokenPath { token_a: token_in, token_b: token_out },
            to: recipient,
            deadline: Some(deadline),
            venue: SwapVenue::V3 { fee },
            two_hop: true,
        });
    }

    if sel == *SEL_EXACT_INPUT {
        let path_bytes = dynamic_bytes_at_offset(args, 0).ok_or(SkipReason::DecodeFail)?;
        let recipient = address_at(args, 1).ok_or(SkipReason::DecodeFail)?;
        let deadline = u256_at(args, 2).ok_or(SkipReason::DecodeFail)?;
        let amount_in = u256_at(args, 3).ok_or(SkipReason::DecodeFail)?;
        let amount_out_min = u256_at(args, 4).ok_or(SkipReason::DecodeFail)?;
        let (path, fee) = TwoTokenPath::from_packed_v3_path_with_fee(&path_bytes)?;
        return Ok(DecodedSwap {
            selector_name: "exactInput",
            amount_in,
            amount_out_min,
            path,
            to: recipient,
            deadline: Some(deadline),
            venue: SwapVenue::V3 { fee },
            two_hop: true,
        });
    }

    if sel == *SEL_UR_EXECUTE_2 || sel == *SEL_UR_EXECUTE_3 {
        return decode_universal_router(args);
    }

    Err(SkipReason::DecodeFail)
}

/// UR: `amountIn` của 2 command đang xử lý (V2/V3 `SWAP_EXACT_IN`) luôn nằm
/// trong `input`, không cần `tx.value` (khác `swapExactETHForTokens` của V2
/// Router cổ điển) — command `WRAP_ETH` nhận BNB thì ngoài phạm vi phiên này.
fn decode_universal_router(args: &[u8]) -> Result<DecodedSwap, SkipReason> {
    let commands = dynamic_bytes_at_offset(args, 0).ok_or(SkipReason::DecodeFail)?;
    let inputs = dynamic_bytes_array_at_offset(args, 1).ok_or(SkipReason::DecodeFail)?;
    // Chỉ xử lý tx đúng 1 command/1 input — multicall nhiều lệnh gộp ngoài
    // phạm vi phiên này, không đoán thứ tự tác động lên WBNB.
    if commands.len() != 1 || inputs.len() != 1 {
        return Err(SkipReason::DecodeFail);
    }
    let cmd = commands[0] & 0x3f; // bit 0x80 la FLAG_ALLOW_REVERT, mask bo di
    let input = &inputs[0];
    match cmd {
        CMD_V2_SWAP_EXACT_IN => decode_ur_v2_swap_exact_in(input),
        CMD_V3_SWAP_EXACT_IN => decode_ur_v3_swap_exact_in(input),
        _ => Err(SkipReason::DecodeFail),
    }
}

fn decode_ur_v2_swap_exact_in(input: &[u8]) -> Result<DecodedSwap, SkipReason> {
    // V2SwapExactInParams: address recipient, uint256 amountIn, uint256 amountOutMin, address[] path, bool payerIsUser
    let recipient = address_at(input, 0).ok_or(SkipReason::DecodeFail)?;
    let amount_in = u256_at(input, 1).ok_or(SkipReason::DecodeFail)?;
    let amount_out_min = u256_at(input, 2).ok_or(SkipReason::DecodeFail)?;
    let path = dynamic_address_array_at_offset(input, 3).ok_or(SkipReason::DecodeFail)?;
    let path = TwoTokenPath::from_address_path(&path)?;
    Ok(DecodedSwap {
        selector_name: "UR:V2_SWAP_EXACT_IN",
        amount_in,
        amount_out_min,
        path,
        to: recipient,
        deadline: None,
        venue: SwapVenue::V2,
        two_hop: true,
    })
}

fn decode_ur_v3_swap_exact_in(input: &[u8]) -> Result<DecodedSwap, SkipReason> {
    // V3SwapExactInParams: address recipient, uint256 amountIn, uint256 amountOutMin, bytes path, bool payerIsUser
    let recipient = address_at(input, 0).ok_or(SkipReason::DecodeFail)?;
    let amount_in = u256_at(input, 1).ok_or(SkipReason::DecodeFail)?;
    let amount_out_min = u256_at(input, 2).ok_or(SkipReason::DecodeFail)?;
    let path_bytes = dynamic_bytes_at_offset(input, 3).ok_or(SkipReason::DecodeFail)?;
    let (path, fee) = TwoTokenPath::from_packed_v3_path_with_fee(&path_bytes)?;
    Ok(DecodedSwap {
        selector_name: "UR:V3_SWAP_EXACT_IN",
        amount_in,
        amount_out_min,
        path,
        to: recipient,
        deadline: None,
        venue: SwapVenue::V3 { fee },
        two_hop: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(hex: &str) -> Address {
        Address::from_str(hex).unwrap()
    }

    fn wbnb_addr() -> Address {
        addr(WBNB_ADDRESS)
    }

    fn token_addr() -> Address {
        addr("0x111111111111111111111111111111111111beef")
    }

    fn pad_addr(a: Address) -> [u8; 32] {
        let mut w = [0u8; 32];
        w[12..32].copy_from_slice(a.as_slice());
        w
    }

    fn pad_u256(v: u64) -> [u8; 32] {
        U256::from(v).to_be_bytes::<32>()
    }

    /// Selector nổi tiếng, đối chiếu để chắc chắn suy luận chữ ký đúng, không
    /// lấy nhầm hàm (vd `swapTokensForExactETH` khác `swapExactTokensForETH`).
    #[test]
    fn well_known_selectors_match() {
        assert_eq!(*SEL_SWAP_EXACT_ETH_FOR_TOKENS, [0x7f, 0xf3, 0x6a, 0xb5]);
        assert_eq!(*SEL_SWAP_EXACT_TOKENS_FOR_ETH, [0x18, 0xcb, 0xaf, 0xe5]);
        assert_eq!(*SEL_SWAP_EXACT_TOKENS_FOR_TOKENS, [0x38, 0xed, 0x17, 0x39]);
    }

    /// Cụm `evm-validate-fixed-then-wire` (B4''.1) — 3 selector fee-on-transfer
    /// MỚI. Giá trị kỳ vọng KHÔNG chép từ trí nhớ: `0xb6f9de95` và `0x5c11d795`
    /// là 2 selector ĐÃ QUAN SÁT THẬT trong `txpool_content` của
    /// `bsc-rpc.publicnode.com` phiên này trên tx gửi ĐÚNG tới V2 Router đã pin
    /// (xem BAOCAO33 ô 5 — bảng đếm selector mempool thật). Test này chốt rằng
    /// `keccak256(chữ_ký_hàm)` tự tính KHỚP đúng các giá trị quan sát được —
    /// nếu chữ ký gõ sai thì assert sẽ đỏ ngay, không im lặng decode nhầm.
    #[test]
    fn fee_on_transfer_selectors_match_observed_mempool_values() {
        assert_eq!(*SEL_SWAP_EXACT_ETH_FOR_TOKENS_FOT, [0xb6, 0xf9, 0xde, 0x95]);
        assert_eq!(*SEL_SWAP_EXACT_TOKENS_FOR_TOKENS_FOT, [0x5c, 0x11, 0xd7, 0x95]);
        assert_eq!(*SEL_SWAP_EXACT_TOKENS_FOR_ETH_FOT, [0x79, 0x1a, 0xc9, 0x47]);
    }

    /// Biến thể fee-on-transfer PHẢI decode ra CÙNG kết quả với bản cổ điển
    /// (layout tham số giống hệt) — dựng đúng 1 calldata, đổi MỖI 4 byte
    /// selector, so 2 kết quả field-by-field.
    #[test]
    fn fee_on_transfer_eth_in_decodes_same_as_classic() {
        let wbnb = wbnb_addr();
        let token = addr("0x1111111111111111111111111111111111111111");
        let mut args = Vec::new();
        args.extend_from_slice(&pad_u256(7)); // amountOutMin
        args.extend_from_slice(&pad_u256(0x80)); // offset path
        args.extend_from_slice(&pad_addr(addr("0x2222222222222222222222222222222222222222"))); // to
        args.extend_from_slice(&pad_u256(999)); // deadline
        args.extend_from_slice(&pad_u256(2)); // path.len
        args.extend_from_slice(&pad_addr(wbnb));
        args.extend_from_slice(&pad_addr(token));

        let mut classic = SEL_SWAP_EXACT_ETH_FOR_TOKENS.to_vec();
        classic.extend_from_slice(&args);
        let mut fot = SEL_SWAP_EXACT_ETH_FOR_TOKENS_FOT.to_vec();
        fot.extend_from_slice(&args);

        let v = U256::from(123u64);
        let a = decode_swap_calldata(&classic, v).expect("ban co dien phai decode duoc");
        let b = decode_swap_calldata(&fot, v).expect("ban fee-on-transfer phai decode duoc");
        assert_eq!(a.amount_in, b.amount_in);
        assert_eq!(a.amount_out_min, b.amount_out_min);
        assert_eq!(a.path.token_a, b.path.token_a);
        assert_eq!(a.path.token_b, b.path.token_b);
        assert_eq!(a.to, b.to);
        assert_eq!(a.deadline, b.deadline);
        assert_eq!(a.venue, b.venue);
        assert_eq!(b.selector_name, "swapExactETHForTokensSupportingFeeOnTransferTokens");
    }

    fn build_swap_exact_eth_for_tokens(path: &[Address]) -> Vec<u8> {
        let mut out = SEL_SWAP_EXACT_ETH_FOR_TOKENS.to_vec();
        out.extend_from_slice(&pad_u256(0)); // amountOutMin
        out.extend_from_slice(&pad_u256(0x80)); // offset path = 4 head words * 32
        out.extend_from_slice(&pad_addr(addr("0x2222222222222222222222222222222222222222"))); // to
        out.extend_from_slice(&pad_u256(9_999_999_999)); // deadline
        out.extend_from_slice(&pad_u256(path.len() as u64)); // path.length
        for a in path {
            out.extend_from_slice(&pad_addr(*a));
        }
        out
    }

    #[test]
    fn decode_v2_swap_exact_eth_for_tokens_wbnb_to_token() {
        let calldata = build_swap_exact_eth_for_tokens(&[wbnb_addr(), token_addr()]);
        let tx_value = U256::from(10_000_000_000_000_000u64); // 0.01 BNB
        let decoded = decode_swap_calldata(&calldata, tx_value).expect("phai decode duoc");
        println!("fixture decode struct: {decoded:?} token_vs_wbnb={:#x}", decoded.token().unwrap());
        assert_eq!(decoded.selector_name, "swapExactETHForTokens");
        assert_eq!(decoded.amount_in, tx_value);
        assert_eq!(decoded.amount_out_min, U256::ZERO); // builder dung amountOutMin=0
        assert_eq!(decoded.token().unwrap(), token_addr());
    }

    /// Fixture riêng amountOutMin != 0 — chứng minh field mới (`3.1+3.2+3.3`,
    /// cần cho victim-still-ok) đọc đúng word idx0 của
    /// `swapExactETHForTokens`, không phải giá trị mặc định trùng hợp = 0.
    #[test]
    fn decode_captures_nonzero_amount_out_min_swap_exact_eth_for_tokens() {
        let mut out = SEL_SWAP_EXACT_ETH_FOR_TOKENS.to_vec();
        out.extend_from_slice(&pad_u256(123_456_789)); // amountOutMin != 0
        out.extend_from_slice(&pad_u256(0x80)); // offset path
        out.extend_from_slice(&pad_addr(addr("0x2222222222222222222222222222222222222222"))); // to
        out.extend_from_slice(&pad_u256(9_999_999_999)); // deadline
        out.extend_from_slice(&pad_u256(2)); // path.length
        out.extend_from_slice(&pad_addr(wbnb_addr()));
        out.extend_from_slice(&pad_addr(token_addr()));
        let decoded = decode_swap_calldata(&out, U256::from(1u64)).expect("phai decode duoc");
        assert_eq!(decoded.amount_out_min, U256::from(123_456_789u64));
    }

    #[test]
    fn decode_rejects_three_token_path_not_wbnb_pair() {
        let usdt_like = addr("0x333333333333333333333333333333333333dead");
        let calldata = build_swap_exact_eth_for_tokens(&[wbnb_addr(), usdt_like, token_addr()]);
        let err = decode_swap_calldata(&calldata, U256::from(1u64)).unwrap_err();
        assert_eq!(err, SkipReason::NotWbnbPair);
    }

    #[test]
    fn decode_rejects_two_token_path_without_wbnb() {
        let a = addr("0x444444444444444444444444444444444444aaaa");
        let calldata = build_swap_exact_eth_for_tokens(&[a, token_addr()]);
        let decoded = decode_swap_calldata(&calldata, U256::from(1u64)).expect("van decode duoc struct");
        // path hop le (2 token) nhung khong ben nao la WBNB -> loi khi doi token().
        assert_eq!(decoded.token().unwrap_err(), SkipReason::NotWbnbPair);
    }

    /// Cụm `usdt-quote-asset` — `token_vs` tổng quát phải hoạt động đúng với
    /// quote KHÁC WBNB (vd USDT), và `token_vs_wbnb()` vẫn tương đương
    /// `token_vs(wbnb())` (không đổi hành vi cũ).
    #[test]
    fn token_vs_generalizes_to_arbitrary_quote_address() {
        let usdt = addr("0x55d398326f99059ff775485246999027b3197955");
        let path_usdt = TwoTokenPath { token_a: usdt, token_b: token_addr() };
        assert_eq!(path_usdt.token_vs(usdt), Some(token_addr()));
        assert_eq!(path_usdt.token_vs(wbnb_addr()), None);

        let path_wbnb = TwoTokenPath { token_a: wbnb_addr(), token_b: token_addr() };
        assert_eq!(path_wbnb.token_vs(wbnb_addr()), path_wbnb.token_vs_wbnb());
    }

    #[test]
    fn decode_unknown_selector_is_decode_fail() {
        let calldata = vec![0xde, 0xad, 0xbe, 0xef, 0, 0, 0, 0];
        let err = decode_swap_calldata(&calldata, U256::ZERO).unwrap_err();
        assert_eq!(err, SkipReason::DecodeFail);
    }

    #[test]
    fn decode_too_short_calldata_is_decode_fail() {
        let err = decode_swap_calldata(&[0x01, 0x02], U256::ZERO).unwrap_err();
        assert_eq!(err, SkipReason::DecodeFail);
    }

    fn build_swap_exact_tokens_for_tokens(amount_in: u64, path: &[Address]) -> Vec<u8> {
        let mut out = SEL_SWAP_EXACT_TOKENS_FOR_TOKENS.to_vec();
        out.extend_from_slice(&pad_u256(amount_in));
        out.extend_from_slice(&pad_u256(0)); // amountOutMin
        out.extend_from_slice(&pad_u256(0xa0)); // offset path = 5 head words * 32
        out.extend_from_slice(&pad_addr(addr("0x555555555555555555555555555555555555cccc"))); // to
        out.extend_from_slice(&pad_u256(9_999_999_999)); // deadline
        out.extend_from_slice(&pad_u256(path.len() as u64));
        for a in path {
            out.extend_from_slice(&pad_addr(*a));
        }
        out
    }

    #[test]
    fn decode_v2_swap_exact_tokens_for_tokens_token_to_wbnb() {
        let calldata = build_swap_exact_tokens_for_tokens(123_456, &[token_addr(), wbnb_addr()]);
        let decoded = decode_swap_calldata(&calldata, U256::ZERO).expect("phai decode duoc");
        assert_eq!(decoded.selector_name, "swapExactTokensForTokens");
        assert_eq!(decoded.amount_in, U256::from(123_456u64));
        assert_eq!(decoded.token().unwrap(), token_addr());
    }

    fn build_exact_input_single(token_in: Address, token_out: Address, amount_in: u64) -> Vec<u8> {
        let mut out = SEL_EXACT_INPUT_SINGLE.to_vec();
        out.extend_from_slice(&pad_addr(token_in));
        out.extend_from_slice(&pad_addr(token_out));
        out.extend_from_slice(&pad_u256(500)); // fee
        out.extend_from_slice(&pad_addr(addr("0x666666666666666666666666666666666666e0e0"))); // recipient
        out.extend_from_slice(&pad_u256(9_999_999_999)); // deadline
        out.extend_from_slice(&pad_u256(amount_in));
        out.extend_from_slice(&pad_u256(0)); // amountOutMinimum
        out.extend_from_slice(&pad_u256(0)); // sqrtPriceLimitX96
        out
    }

    #[test]
    fn decode_v3_exact_input_single_wbnb_pair() {
        let calldata = build_exact_input_single(wbnb_addr(), token_addr(), 777);
        let decoded = decode_swap_calldata(&calldata, U256::ZERO).expect("phai decode duoc");
        assert_eq!(decoded.selector_name, "exactInputSingle");
        assert_eq!(decoded.amount_in, U256::from(777u64));
        assert_eq!(decoded.token().unwrap(), token_addr());
        assert_eq!(decoded.venue, SwapVenue::V3 { fee: 500 }); // build_exact_input_single dung fee=500
        assert!(decoded.two_hop);
    }

    #[test]
    fn decode_v2_functions_and_ur_v2_have_venue_v2() {
        let calldata = build_swap_exact_eth_for_tokens(&[wbnb_addr(), token_addr()]);
        let decoded = decode_swap_calldata(&calldata, U256::from(1u64)).unwrap();
        assert_eq!(decoded.venue, SwapVenue::V2);

        let calldata2 = build_swap_exact_tokens_for_tokens(1, &[token_addr(), wbnb_addr()]);
        let decoded2 = decode_swap_calldata(&calldata2, U256::ZERO).unwrap();
        assert_eq!(decoded2.venue, SwapVenue::V2);
    }

    fn packed_v3_path_single_hop(token_a: Address, fee: u32, token_b: Address) -> Vec<u8> {
        let mut out = Vec::with_capacity(43);
        out.extend_from_slice(token_a.as_slice());
        out.extend_from_slice(&fee.to_be_bytes()[1..4]); // uint24 = 3 byte cuoi
        out.extend_from_slice(token_b.as_slice());
        out
    }

    fn build_exact_input(amount_in: u64, path_bytes: &[u8]) -> Vec<u8> {
        let mut out = SEL_EXACT_INPUT.to_vec();
        out.extend_from_slice(&pad_u256(0xa0)); // offset path bytes = 5 head words * 32
        out.extend_from_slice(&pad_addr(addr("0x777777777777777777777777777777777777f0f0"))); // recipient
        out.extend_from_slice(&pad_u256(9_999_999_999)); // deadline
        out.extend_from_slice(&pad_u256(amount_in));
        out.extend_from_slice(&pad_u256(0)); // amountOutMinimum
        out.extend_from_slice(&pad_u256(path_bytes.len() as u64));
        out.extend_from_slice(path_bytes);
        // pad phan du cho tron 32 byte (ABI padding) — khong bat buoc voi decoder
        // hien tai (chi doc dung do dai) nhung giu dung chuan encode.
        let rem = path_bytes.len() % 32;
        if rem != 0 {
            out.extend(std::iter::repeat(0u8).take(32 - rem));
        }
        out
    }

    #[test]
    fn decode_v3_exact_input_single_hop_wbnb_pair() {
        let path_bytes = packed_v3_path_single_hop(token_addr(), 2500, wbnb_addr());
        let calldata = build_exact_input(555, &path_bytes);
        let decoded = decode_swap_calldata(&calldata, U256::ZERO).expect("phai decode duoc");
        assert_eq!(decoded.selector_name, "exactInput");
        assert_eq!(decoded.token().unwrap(), token_addr());
        assert_eq!(decoded.venue, SwapVenue::V3 { fee: 2500 });
        assert!(decoded.two_hop);
    }

    #[test]
    fn decode_v3_exact_input_multihop_is_not_wbnb_pair() {
        let mid = addr("0x888888888888888888888888888888888888c0c0");
        let mut path_bytes = packed_v3_path_single_hop(token_addr(), 2500, mid);
        path_bytes.extend_from_slice(&500u32.to_be_bytes()[1..4]);
        path_bytes.extend_from_slice(wbnb_addr().as_slice());
        let calldata = build_exact_input(555, &path_bytes);
        let err = decode_swap_calldata(&calldata, U256::ZERO).unwrap_err();
        assert_eq!(err, SkipReason::NotWbnbPair);
    }

    fn build_ur_execute(command: u8, input: &[u8]) -> Vec<u8> {
        // execute(bytes commands, bytes[] inputs) — 2 tham so dau, offset tinh
        // theo 2 head word (0x40).
        let mut out = SEL_UR_EXECUTE_2.to_vec();
        out.extend_from_slice(&pad_u256(0x40)); // offset commands
        let commands_words = 1; // 1 byte -> 1 word du lieu (padding ve 32 byte)
        let offset_inputs = 0x40 + 32 + commands_words * 32; // sau: length word + data word cua commands
        out.extend_from_slice(&pad_u256(offset_inputs as u64));
        // --- commands (bytes) tai offset 0x40 ---
        out.extend_from_slice(&pad_u256(1)); // length = 1 byte
        let mut cmd_word = [0u8; 32];
        cmd_word[0] = command;
        out.extend_from_slice(&cmd_word);
        // --- inputs (bytes[]) tai offset_inputs ---
        out.extend_from_slice(&pad_u256(1)); // inputs.length = 1
        out.extend_from_slice(&pad_u256(0x20)); // offset phan tu 0, tinh tu sau length word nay
        out.extend_from_slice(&pad_u256(input.len() as u64));
        out.extend_from_slice(input);
        let rem = input.len() % 32;
        if rem != 0 {
            out.extend(std::iter::repeat(0u8).take(32 - rem));
        }
        out
    }

    fn build_v2_swap_exact_in_input(amount_in: u64, path: &[Address]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&pad_addr(addr("0x999999999999999999999999999999999999a1a1"))); // recipient
        out.extend_from_slice(&pad_u256(amount_in));
        out.extend_from_slice(&pad_u256(0)); // amountOutMin
        out.extend_from_slice(&pad_u256(0xa0)); // offset path = 5 head words (recipient,amountIn,amountOutMin,offset,payerIsUser)
        let mut bool_word = [0u8; 32];
        bool_word[31] = 1; // payerIsUser = true
        out.extend_from_slice(&bool_word);
        out.extend_from_slice(&pad_u256(path.len() as u64));
        for a in path {
            out.extend_from_slice(&pad_addr(*a));
        }
        out
    }

    #[test]
    fn decode_universal_router_v2_swap_exact_in_wbnb_pair() {
        let input = build_v2_swap_exact_in_input(42, &[wbnb_addr(), token_addr()]);
        let calldata = build_ur_execute(CMD_V2_SWAP_EXACT_IN, &input);
        let decoded = decode_swap_calldata(&calldata, U256::ZERO).expect("phai decode duoc UR command V2");
        assert_eq!(decoded.selector_name, "UR:V2_SWAP_EXACT_IN");
        assert_eq!(decoded.amount_in, U256::from(42u64));
        assert_eq!(decoded.token().unwrap(), token_addr());
        assert_eq!(decoded.venue, SwapVenue::V2);
    }

    fn build_v3_swap_exact_in_input(amount_in: u64, path_bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&pad_addr(addr("0x888888888888888888888888888888888888b2b2"))); // recipient
        out.extend_from_slice(&pad_u256(amount_in));
        out.extend_from_slice(&pad_u256(0)); // amountOutMin
        out.extend_from_slice(&pad_u256(0xa0)); // offset path = 5 head words (recipient,amountIn,amountOutMin,offset,payerIsUser)
        let mut bool_word = [0u8; 32];
        bool_word[31] = 1;
        out.extend_from_slice(&bool_word);
        out.extend_from_slice(&pad_u256(path_bytes.len() as u64));
        out.extend_from_slice(path_bytes);
        let rem = path_bytes.len() % 32;
        if rem != 0 {
            out.extend(std::iter::repeat(0u8).take(32 - rem));
        }
        out
    }

    #[test]
    fn decode_universal_router_v3_swap_exact_in_has_fee() {
        let path_bytes = packed_v3_path_single_hop(wbnb_addr(), 100, token_addr());
        let input = build_v3_swap_exact_in_input(99, &path_bytes);
        let calldata = build_ur_execute(CMD_V3_SWAP_EXACT_IN, &input);
        let decoded = decode_swap_calldata(&calldata, U256::ZERO).expect("phai decode duoc UR command V3");
        assert_eq!(decoded.selector_name, "UR:V3_SWAP_EXACT_IN");
        assert_eq!(decoded.venue, SwapVenue::V3 { fee: 100 });
        assert_eq!(decoded.token().unwrap(), token_addr());
    }

    #[test]
    fn decode_universal_router_unknown_command_is_decode_fail() {
        let input = build_v2_swap_exact_in_input(1, &[wbnb_addr(), token_addr()]);
        let calldata = build_ur_execute(0x1f, &input); // command khong xu ly trong phien nay
        let err = decode_swap_calldata(&calldata, U256::ZERO).unwrap_err();
        assert_eq!(err, SkipReason::DecodeFail);
    }

    // ===== Cụm `exec-path-traps` (F-20) — fuzz calldata offset, khong panic =====

    /// Xorshift64* thuần Rust — KHÔNG thêm crate `rand` (chỉ cần chuỗi số giả
    /// ngẫu nhiên tái lập được cho 1 test, không cần chất lượng thống kê).
    struct Xorshift64(u64);
    impl Xorshift64 {
        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn next_byte(&mut self) -> u8 {
            (self.next_u64() & 0xff) as u8
        }
    }

    /// ĐẠT CẦN DÁN (lệnh exec-path-traps, mục 11): 1000 calldata ngẫu nhiên
    /// (selector + độ dài + nội dung random) đưa thẳng vào
    /// `decode_swap_calldata` — không được panic (chỉ `Ok`/`Err`, không có
    /// nhánh nào khác). Trước fix F-20, `word`/`u256_at_byteoffset`/
    /// `dynamic_bytes_at_offset`/`dynamic_bytes_array_at_offset` cộng offset
    /// không `checked_` — calldata với word offset gần `usize::MAX` (dễ xảy
    /// ra vì offset là `U256` bất kỳ do kẻ tấn công chọn, ép về `usize` qua
    /// `try_from` mà không chặn biên trên) có thể tràn cộng và panic (debug
    /// build). Test này bơm CẢ random thuần lẫn payload cố ý nhắm offset lớn
    /// vào đúng 4 selector có nhánh offset (`SEL_SWAP_EXACT_ETH_FOR_TOKENS`,
    /// `SEL_EXACT_INPUT`, `SEL_UR_EXECUTE_2`) để chắc chắn đi qua các hàm đã
    /// sửa, không chỉ rơi vào `decode_fail` sớm vì selector lạ.
    #[test]
    fn fuzz_1000_random_calldata_plus_20_offset_near_usize_max_no_panic() {
        let mut rng = Xorshift64(0x9E3779B97F4A7C15);
        let selectors_with_offset: [[u8; 4]; 3] =
            [*SEL_SWAP_EXACT_ETH_FOR_TOKENS, *SEL_EXACT_INPUT, *SEL_UR_EXECUTE_2];

        // 1000 calldata hoan toan ngau nhien (do dai 0..=200 byte).
        for _ in 0..1000u32 {
            let len = (rng.next_u64() % 201) as usize;
            let mut data = Vec::with_capacity(len);
            for _ in 0..len {
                data.push(rng.next_byte());
            }
            let tx_value = U256::from(rng.next_u64());
            let _ = decode_swap_calldata(&data, tx_value); // khong duoc panic
        }

        // 20 calldata: selector that (co nhanh offset) + word offset gia gan
        // usize::MAX (32 byte gan toan 0xff) o dung vi tri offset arg dau
        // tien, phan con lai random.
        let near_max_word: [u8; 32] = {
            let mut w = [0xffu8; 32];
            w[24..32].copy_from_slice(&(u64::MAX - 7).to_be_bytes());
            w
        };
        for i in 0..20u32 {
            let sel = selectors_with_offset[(i as usize) % selectors_with_offset.len()];
            let mut data = sel.to_vec();
            // Dat 1 word offset kich thuoc gan usize::MAX ngay dau args, roi
            // them vai word random phia sau de cac field khac cung doc duoc
            // (hoac loi som, deu chap nhan duoc - chi cam panic).
            data.extend_from_slice(&near_max_word);
            for _ in 0..6u32 {
                let mut w = [0u8; 32];
                for b in w.iter_mut() {
                    *b = rng.next_byte();
                }
                data.extend_from_slice(&w);
            }
            let tx_value = U256::from(rng.next_u64());
            let _ = decode_swap_calldata(&data, tx_value); // khong duoc panic
        }
    }

    // ===== Cụm `exec-path-traps` (F-16) — venue_matches_router =====

    use crate::venues::Venue;

    #[test]
    fn venue_matches_router_none_router_never_mismatches() {
        assert!(venue_matches_router("swapExactETHForTokens", None));
        assert!(venue_matches_router("exactInputSingle", None));
        assert!(venue_matches_router("UR:V2_SWAP_EXACT_IN", None));
        assert!(venue_matches_router("bogus", None));
    }

    /// ĐẠT CẦN DÁN (mục 10, chiều 1): selector V2 cổ điển tới V3 SwapRouter/UR -> mismatch.
    #[test]
    fn venue_matches_router_classic_v2_selector_to_v3_or_ur_is_mismatch() {
        for sel in [
            "swapExactETHForTokens",
            "swapExactTokensForETH",
            "swapExactTokensForTokens",
            "swapExactETHForTokensSupportingFeeOnTransferTokens",
            "swapExactTokensForETHSupportingFeeOnTransferTokens",
            "swapExactTokensForTokensSupportingFeeOnTransferTokens",
        ] {
            assert!(venue_matches_router(sel, Some(Venue::V2)), "{sel} phai khop Venue::V2");
            assert!(!venue_matches_router(sel, Some(Venue::V3)), "{sel} toi V3 phai la mismatch");
            assert!(!venue_matches_router(sel, Some(Venue::SmartRouter)), "{sel} toi SmartRouter phai la mismatch");
            assert!(!venue_matches_router(sel, Some(Venue::UniversalRouter)), "{sel} toi UR phai la mismatch");
        }
    }

    /// ĐẠT CẦN DÁN (mục 10, chiều 2): selector V3 truc tiep toi V2 Router/UR -> mismatch.
    #[test]
    fn venue_matches_router_classic_v3_selector_to_v2_or_ur_is_mismatch() {
        for sel in ["exactInputSingle", "exactInput"] {
            assert!(venue_matches_router(sel, Some(Venue::V3)), "{sel} phai khop Venue::V3");
            assert!(venue_matches_router(sel, Some(Venue::SmartRouter)), "{sel} phai khop SmartRouter");
            assert!(!venue_matches_router(sel, Some(Venue::V2)), "{sel} toi V2 Router phai la mismatch");
            assert!(!venue_matches_router(sel, Some(Venue::UniversalRouter)), "{sel} toi UR (goi truc tiep, khong qua execute()) phai la mismatch");
        }
    }

    #[test]
    fn venue_matches_router_ur_command_only_matches_universal_router() {
        for sel in ["UR:V2_SWAP_EXACT_IN", "UR:V3_SWAP_EXACT_IN"] {
            assert!(venue_matches_router(sel, Some(Venue::UniversalRouter)));
            assert!(!venue_matches_router(sel, Some(Venue::V2)));
            assert!(!venue_matches_router(sel, Some(Venue::V3)));
            assert!(!venue_matches_router(sel, Some(Venue::SmartRouter)));
        }
    }
}

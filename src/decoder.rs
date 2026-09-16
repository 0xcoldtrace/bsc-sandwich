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

// Cụm `decoder-coverage` (B2/B3) — biến thể SmartRouter KHÔNG có `deadline`
// (7-tuple, tất cả field static, không lệch nghĩa với bản có deadline ngoài
// việc thiếu 1 field) — selector đối chiếu THẬT với mempool BSC (xem
// `tests/fixtures/ur_calldata.jsonl`, cụm B1): `0x04e45aaf` quan sát 3 lần
// gửi tới SmartRouter đã pin.
static SEL_EXACT_INPUT_SINGLE_NO_DEADLINE: LazyLock<[u8; 4]> =
    LazyLock::new(|| selector("exactInputSingle(address,address,uint24,address,uint256,uint256,uint160)"));
/// exactOutput* — V3 exact-OUTPUT (amountOut cố định, amountIn là TRẦN
/// `amountInMaximum` chứ KHÔNG phải số thực chi) — KHÁC MÔ HÌNH sandwich
/// (`sim_v2`/`sim_evm` chỉ có exact-INPUT). Theo AGENTS.md lệnh B2:
/// "exactOutput* → venue_unpinned (đếm, chưa sim)" — decode đủ để phân loại
/// đúng `SwapVenue::V3`, KHÔNG dùng `amount_in`/`amount_out_min` giải mã được
/// ở đây cho bất kỳ tính toán sim nào (gate `VenueUnpinned` chặn trước khi
/// tới sim, xem `main.rs::handle_paper_tx`).
static SEL_EXACT_OUTPUT_SINGLE: LazyLock<[u8; 4]> = LazyLock::new(|| {
    selector("exactOutputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))")
});
static SEL_EXACT_OUTPUT_SINGLE_NO_DEADLINE: LazyLock<[u8; 4]> =
    LazyLock::new(|| selector("exactOutputSingle((address,address,uint24,address,uint256,uint256,uint160))"));
static SEL_EXACT_OUTPUT: LazyLock<[u8; 4]> =
    LazyLock::new(|| selector("exactOutput((bytes,address,uint256,uint256,uint256))"));
static SEL_EXACT_OUTPUT_NO_DEADLINE: LazyLock<[u8; 4]> =
    LazyLock::new(|| selector("exactOutput((bytes,address,uint256,uint256))"));
/// Cụm B3 — `SmartRouter.multicall` (3 chữ ký quan sát THẬT trong mempool,
/// `tests/fixtures/ur_calldata.jsonl`): `ac9650d8` (không deadline, phổ biến
/// nhất theo docs Uniswap Multicall gốc), `5ae401dc` (kèm `deadline` — cụm
/// `0xed3f...`? KHÔNG, xác nhận qua `keccak256` thật, xem test). Biến thể
/// `multicall(bytes32,bytes[])` (previousBlockhash guard) CHƯA quan sát được
/// trong mẫu thật phiên này — KHÔNG thêm (đúng luật "không đoán").
static SEL_MULTICALL: LazyLock<[u8; 4]> = LazyLock::new(|| selector("multicall(bytes[])"));
static SEL_MULTICALL_DEADLINE: LazyLock<[u8; 4]> = LazyLock::new(|| selector("multicall(uint256,bytes[])"));

const CMD_V3_SWAP_EXACT_IN: u8 = 0x00;
const CMD_V2_SWAP_EXACT_IN: u8 = 0x08;
/// Cụm B2 — command UR bổ sung cho multi-command sequence (Universal Router
/// command set chuẩn Uniswap/Pancake, đọc từ docs cộng đồng + đối chiếu THẬT
/// với mẫu mempool bắt được phiên này — xem `tests/fixtures/ur_calldata.jsonl`
/// bảng B1: `WRAP_ETH`+`V2_SWAP_EXACT_IN`/`V3_SWAP_EXACT_IN` xuất hiện thật).
const CMD_PERMIT2_PERMIT: u8 = 0x0a;
const CMD_WRAP_ETH: u8 = 0x0b;

/// `ActionConstants.CONTRACT_BALANCE` (Uniswap Universal Router periphery,
/// `0x8000...000` = `2^255`) — sentinel `amountIn` nghĩa là "dùng TOÀN BỘ số
/// dư router đang giữ" (thường do `WRAP_ETH` vừa wrap xong) thay vì 1 số cố
/// định trong calldata. Theo ĐÚNG lệnh B2 (xử lý phòng vệ cho trường hợp này)
/// — 8/8 mẫu `WRAP_ETH`→`V2_SWAP_EXACT_IN` THẬT bắt được phiên này (xem
/// `tests/fixtures/ur_calldata.jsonl`) đều encode `amountIn` = số BNB CỤ THỂ
/// (khớp `tx.value`), KHÔNG dùng sentinel — nhánh xử lý sentinel dưới đây vẫn
/// giữ (đúng lệnh, không đoán bỏ) nhưng CHƯA có bằng chứng THẬT nào kích hoạt
/// được nó trong mẫu phiên này, ghi rõ để không nhận vơ là đã verify runtime.
fn contract_balance_sentinel() -> U256 {
    U256::from(1u8) << 255
}

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
    /// không chia hết) đều coi là not_wbnb_pair theo AGENTS.md "3+ token".
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

fn address_at_byteoffset(data: &[u8], byte_offset: usize) -> Option<Address> {
    let w = slice_checked(data, byte_offset, 32)?;
    Some(Address::from_slice(&w[12..32]))
}

/// Cụm `decoder-coverage` (B2, fix bug pre-existing) — `exactInput`/`exactOutput`
/// (SwapRouter/SmartRouter) có ĐÚNG 1 tham số struct duy nhất chứa field
/// `bytes` ở VỊ TRÍ ĐẦU (dynamic) — ABI encode 2 LỚP offset: lớp NGOÀI (word
/// tại `outer_word_idx`, tính từ đầu `args`) trỏ tới điểm bắt đầu struct; TẠI
/// điểm đó, field `bytes` đầu tiên lại có offset RIÊNG (tính từ điểm bắt đầu
/// struct, KHÔNG PHẢI từ đầu `args`) trỏ tới length+data thật. Khác
/// `dynamic_bytes_at_offset` (1 lớp — đúng cho tham số dynamic ĐỘC LẬP không
/// nằm trong struct, vd UR `V2SwapExactInParams.path` — xem
/// `decode_ur_v2_swap_exact_in`).
///
/// Xác nhận bằng calldata THẬT (không suy đoán): tx thật gọi
/// `exactOutput((bytes,address,uint256,uint256))` (SmartRouter, selector
/// `0x09b81346`, xem `tests/fixtures/ur_calldata.jsonl`) có word0=`0x20`
/// (outer offset), word1=`0x80` (inner offset TÍNH TỪ word1, = 4 field head
/// * 32), word2=recipient (địa chỉ hợp lệ), word5=`0x2b`=43 (đúng độ dài 1
/// hop V3 path) — khớp CHÍNH XÁC layout 2 lớp, KHÔNG khớp layout 1 lớp (nếu 1
/// lớp, "length" sẽ đọc nhầm ở word1=128, sai hẳn). `SEL_EXACT_INPUT` (đã có
/// từ trước, cùng dạng struct) ĐƯỢC SỬA theo phát hiện này (xem
/// `decode_swap_calldata`) — trước đó decode SAI (1 lớp), khiến `exactInput`
/// thật gần như luôn lệch field.
fn dynamic_bytes_leading_field_of_sole_tuple_param(data: &[u8], outer_word_idx: usize) -> Option<Vec<u8>> {
    let outer_offset = usize::try_from(u256_at(data, outer_word_idx)?).ok()?;
    let inner_offset = usize::try_from(u256_at_byteoffset(data, outer_offset)?).ok()?;
    let field_start = outer_offset.checked_add(inner_offset)?;
    let len = usize::try_from(u256_at_byteoffset(data, field_start)?).ok()?;
    let start = field_start.checked_add(32)?;
    slice_checked(data, start, len).map(|s| s.to_vec())
}

/// Byte offset của field tĩnh thứ `field_index` (0-based) BÊN TRONG struct đã
/// resolve qua lớp offset NGOÀI ở `outer_word_idx` — dùng cho các field SAU
/// field `bytes` đầu tiên trong `exactInput`/`exactOutput` (`recipient`/
/// `deadline`/`amountIn`.../`amountOutMinimum`, đều tĩnh nên nằm ĐÚNG vị trí
/// `field_index` trong phần head của struct, không phụ thuộc độ dài `bytes`
/// thật sự bao nhiêu).
fn tuple_field_byteoffset(data: &[u8], outer_word_idx: usize, field_index: usize) -> Option<usize> {
    let outer_offset = usize::try_from(u256_at(data, outer_word_idx)?).ok()?;
    outer_offset.checked_add(field_index.checked_mul(32)?)
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
    // Cụm `decoder-coverage` (B2/B3) — thêm 4 selector_name mới (biến thể
    // không-deadline của SmartRouter + exactOutput/exactOutputSingle), cùng
    // nhóm router hợp lệ (V3 SwapRouter hoặc SmartRouter) với 2 tên cũ.
    let classic_v3 = matches!(
        selector_name,
        "exactInputSingle"
            | "exactInput"
            | "exactInputSingleNoDeadline"
            | "exactOutputSingle"
            | "exactOutputSingleNoDeadline"
            | "exactOutput"
            | "exactOutputNoDeadline"
    );
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

    if sel == *SEL_MULTICALL || sel == *SEL_MULTICALL_DEADLINE {
        return decode_multicall(args, sel == *SEL_MULTICALL_DEADLINE);
    }

    if sel == *SEL_UR_EXECUTE_2 || sel == *SEL_UR_EXECUTE_3 {
        return decode_universal_router(args, tx_value, sel == *SEL_UR_EXECUTE_3);
    }

    match try_decode_token_swap_selector(sel, args) {
        Some(r) => r,
        None => Err(SkipReason::DecodeFail),
    }
}

/// Cụm `decoder-coverage` (B2/B3) — MỌI selector "swap dùng token làm input"
/// KHÔNG phụ thuộc `tx.value` (khác 2 hàm ETH-in V2 Router cổ điển ở trên,
/// xử lý riêng ở `decode_swap_calldata` vì cần `tx_value`) — dùng CHUNG cho
/// dispatch top-level LẪN từng sub-call bên trong `SmartRouter.multicall`
/// (`decode_multicall`). `None` = selector không nhận ra (caller tự quyết
/// định coi là lỗi hay bỏ qua — bên trong multicall, 1 sub-call lạ như
/// `refundETH()`/`unwrapWETH9(...)`/`sweepToken(...)` là BÌNH THƯỜNG, không
/// phải lỗi, nên `decode_multicall` bỏ qua `None`; ở top-level thì `None` mới
/// là `decode_fail`).
fn try_decode_token_swap_selector(sel: [u8; 4], args: &[u8]) -> Option<Result<DecodedSwap, SkipReason>> {
    if sel == *SEL_SWAP_EXACT_TOKENS_FOR_ETH
        || sel == *SEL_SWAP_EXACT_TOKENS_FOR_TOKENS
        || sel == *SEL_SWAP_EXACT_TOKENS_FOR_ETH_FOT
        || sel == *SEL_SWAP_EXACT_TOKENS_FOR_TOKENS_FOT
    {
        return Some(decode_swap_exact_tokens_for_x(sel, args));
    }
    if sel == *SEL_EXACT_INPUT_SINGLE || sel == *SEL_EXACT_INPUT_SINGLE_NO_DEADLINE {
        return Some(decode_exact_input_single(args, sel == *SEL_EXACT_INPUT_SINGLE));
    }
    if sel == *SEL_EXACT_INPUT {
        return Some(decode_exact_input(args));
    }
    if sel == *SEL_EXACT_OUTPUT_SINGLE || sel == *SEL_EXACT_OUTPUT_SINGLE_NO_DEADLINE {
        return Some(decode_exact_output_single(args, sel == *SEL_EXACT_OUTPUT_SINGLE));
    }
    if sel == *SEL_EXACT_OUTPUT || sel == *SEL_EXACT_OUTPUT_NO_DEADLINE {
        return Some(decode_exact_output(args, sel == *SEL_EXACT_OUTPUT));
    }
    None
}

fn decode_swap_exact_tokens_for_x(sel: [u8; 4], args: &[u8]) -> Result<DecodedSwap, SkipReason> {
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
    Ok(DecodedSwap {
        selector_name,
        amount_in,
        amount_out_min,
        path,
        to,
        deadline: Some(deadline),
        venue: SwapVenue::V2,
        two_hop: true,
    })
}

/// `exactInputSingle` — tuple TOÀN field tĩnh (`address,address,uint24,
/// address,uint256[,uint256],uint256,uint160`) nên KHÔNG có offset nào (kể cả
/// bản có `deadline`, xem test `well_known_selectors_match`-style đối chiếu
/// cũ) — bản `has_deadline=false` (SmartRouter, `0x04e45aaf`, xác nhận THẬT
/// qua mempool, cụm B1) chỉ khác đúng 1 field bị bỏ (`deadline`), các field
/// còn lại GIỮ NGUYÊN thứ tự tương đối.
fn decode_exact_input_single(args: &[u8], has_deadline: bool) -> Result<DecodedSwap, SkipReason> {
    let token_in = address_at(args, 0).ok_or(SkipReason::DecodeFail)?;
    let token_out = address_at(args, 1).ok_or(SkipReason::DecodeFail)?;
    let fee_word = u256_at(args, 2).ok_or(SkipReason::DecodeFail)?;
    let fee = u32::try_from(fee_word).map_err(|_| SkipReason::DecodeFail)?;
    let recipient = address_at(args, 3).ok_or(SkipReason::DecodeFail)?;
    let (deadline, amount_in_idx) = if has_deadline {
        (Some(u256_at(args, 4).ok_or(SkipReason::DecodeFail)?), 5)
    } else {
        (None, 4)
    };
    let amount_in = u256_at(args, amount_in_idx).ok_or(SkipReason::DecodeFail)?;
    let amount_out_min = u256_at(args, amount_in_idx + 1).ok_or(SkipReason::DecodeFail)?;
    Ok(DecodedSwap {
        selector_name: if has_deadline { "exactInputSingle" } else { "exactInputSingleNoDeadline" },
        amount_in,
        amount_out_min,
        path: TwoTokenPath { token_a: token_in, token_b: token_out },
        to: recipient,
        deadline,
        venue: SwapVenue::V3 { fee },
        two_hop: true,
    })
}

/// `exactInput((bytes,address,uint256,uint256,uint256))` — struct field đầu
/// tiên (`bytes path`) là dynamic -> 2 LỚP offset thật (xem doc-comment
/// `dynamic_bytes_leading_field_of_sole_tuple_param`, cụm `decoder-coverage`
/// B2 FIX BUG: trước đó code đọc SAI theo kiểu 1 lớp, không khớp calldata
/// thật). `recipient`/`deadline`/`amount_in`/`amount_out_min` là field TĨNH
/// SAU field dynamic -> vẫn nằm ĐÚNG vị trí head (index 1/2/3/4) của struct,
/// đọc qua `tuple_field_byteoffset` (không phụ thuộc độ dài `path` thật).
fn decode_exact_input(args: &[u8]) -> Result<DecodedSwap, SkipReason> {
    let path_bytes = dynamic_bytes_leading_field_of_sole_tuple_param(args, 0).ok_or(SkipReason::DecodeFail)?;
    let recipient_off = tuple_field_byteoffset(args, 0, 1).ok_or(SkipReason::DecodeFail)?;
    let recipient = address_at_byteoffset(args, recipient_off).ok_or(SkipReason::DecodeFail)?;
    let deadline_off = tuple_field_byteoffset(args, 0, 2).ok_or(SkipReason::DecodeFail)?;
    let deadline = u256_at_byteoffset(args, deadline_off).ok_or(SkipReason::DecodeFail)?;
    let amount_in_off = tuple_field_byteoffset(args, 0, 3).ok_or(SkipReason::DecodeFail)?;
    let amount_in = u256_at_byteoffset(args, amount_in_off).ok_or(SkipReason::DecodeFail)?;
    let amount_out_min_off = tuple_field_byteoffset(args, 0, 4).ok_or(SkipReason::DecodeFail)?;
    let amount_out_min = u256_at_byteoffset(args, amount_out_min_off).ok_or(SkipReason::DecodeFail)?;
    let (path, fee) = TwoTokenPath::from_packed_v3_path_with_fee(&path_bytes)?;
    Ok(DecodedSwap {
        selector_name: "exactInput",
        amount_in,
        amount_out_min,
        path,
        to: recipient,
        deadline: Some(deadline),
        venue: SwapVenue::V3 { fee },
        two_hop: true,
    })
}

/// `exactOutputSingle` — exact-OUTPUT (amountOut cố định, `amountInMaximum`
/// là TRẦN không phải chi phí thật) — NGOÀI mô hình sandwich exact-input của
/// `sim_v2`/`sim_evm`. Vẫn decode đủ path/fee để phân loại ĐÚNG
/// `SwapVenue::V3` (→ `venue_unpinned`, đếm được, không sim) thay vì rơi vào
/// `decode_fail` — xem AGENTS.md lệnh B2 "exactOutput* → venue_unpinned".
/// `amount_in`/`amount_out_min` ở đây mang giá trị `amountInMaximum`/
/// `amountOut` (KHÔNG phải amount thực chi) — an toàn vì gate `VenueUnpinned`
/// chặn trước khi bất kỳ giá trị nào trong 2 field này được dùng để sim.
fn decode_exact_output_single(args: &[u8], has_deadline: bool) -> Result<DecodedSwap, SkipReason> {
    let token_in = address_at(args, 0).ok_or(SkipReason::DecodeFail)?;
    let token_out = address_at(args, 1).ok_or(SkipReason::DecodeFail)?;
    let fee_word = u256_at(args, 2).ok_or(SkipReason::DecodeFail)?;
    let fee = u32::try_from(fee_word).map_err(|_| SkipReason::DecodeFail)?;
    let recipient = address_at(args, 3).ok_or(SkipReason::DecodeFail)?;
    let (deadline, amount_out_idx) = if has_deadline {
        (Some(u256_at(args, 4).ok_or(SkipReason::DecodeFail)?), 5)
    } else {
        (None, 4)
    };
    let amount_out = u256_at(args, amount_out_idx).ok_or(SkipReason::DecodeFail)?;
    let amount_in_maximum = u256_at(args, amount_out_idx + 1).ok_or(SkipReason::DecodeFail)?;
    Ok(DecodedSwap {
        selector_name: if has_deadline { "exactOutputSingle" } else { "exactOutputSingleNoDeadline" },
        amount_in: amount_in_maximum,
        amount_out_min: amount_out,
        path: TwoTokenPath { token_a: token_in, token_b: token_out },
        to: recipient,
        deadline,
        venue: SwapVenue::V3 { fee },
        two_hop: true,
    })
}

/// `exactOutput((bytes,address,uint256[,uint256],uint256))` — cùng cơ chế 2
/// lớp offset của `decode_exact_input` (field `bytes path` đứng đầu, dynamic).
/// Xác nhận field order + 2-lớp-offset BẰNG CALLDATA THẬT (selector
/// `0x09b81346`, không-deadline — xem doc-comment
/// `dynamic_bytes_leading_field_of_sole_tuple_param` + test
/// `decode_exact_output_no_deadline_matches_real_smartrouter_calldata`).
fn decode_exact_output(args: &[u8], has_deadline: bool) -> Result<DecodedSwap, SkipReason> {
    let path_bytes = dynamic_bytes_leading_field_of_sole_tuple_param(args, 0).ok_or(SkipReason::DecodeFail)?;
    let recipient_off = tuple_field_byteoffset(args, 0, 1).ok_or(SkipReason::DecodeFail)?;
    let recipient = address_at_byteoffset(args, recipient_off).ok_or(SkipReason::DecodeFail)?;
    let (deadline, amount_out_field_idx) = if has_deadline {
        let off = tuple_field_byteoffset(args, 0, 2).ok_or(SkipReason::DecodeFail)?;
        (Some(u256_at_byteoffset(args, off).ok_or(SkipReason::DecodeFail)?), 3)
    } else {
        (None, 2)
    };
    let amount_out_off = tuple_field_byteoffset(args, 0, amount_out_field_idx).ok_or(SkipReason::DecodeFail)?;
    let amount_out = u256_at_byteoffset(args, amount_out_off).ok_or(SkipReason::DecodeFail)?;
    let amount_in_max_off = tuple_field_byteoffset(args, 0, amount_out_field_idx + 1).ok_or(SkipReason::DecodeFail)?;
    let amount_in_maximum = u256_at_byteoffset(args, amount_in_max_off).ok_or(SkipReason::DecodeFail)?;
    let (path, fee) = TwoTokenPath::from_packed_v3_path_with_fee(&path_bytes)?;
    Ok(DecodedSwap {
        selector_name: if has_deadline { "exactOutput" } else { "exactOutputNoDeadline" },
        amount_in: amount_in_maximum,
        amount_out_min: amount_out,
        path,
        to: recipient,
        deadline,
        venue: SwapVenue::V3 { fee },
        two_hop: true,
    })
}

/// Cụm B3 — `SmartRouter.multicall(bytes[])`/`multicall(uint256,bytes[])`:
/// bóc TỪNG sub-call, tìm ĐÚNG 1 sub-call là swap (qua
/// `try_decode_token_swap_selector`) — sub-call khác (`refundETH()`/
/// `unwrapWETH9(...)`/`sweepToken(...)`/...) bị bỏ qua (không phải lỗi, đây
/// là dọn dẹp sau swap, không đổi hướng/số lượng sandwich). Tìm thấy >1
/// sub-call swap -> `decode_fail` (multi-hop qua nhiều lời gọi riêng biệt,
/// KHÔNG đoán chuỗi nào là "thật", đúng luật "không đoán multihop"). 0 sub-call
/// swap (vd toàn bộ là NFT/refund) -> `decode_fail`. `deadline` của
/// `multicall(uint256,bytes[])` áp dụng cho sub-call nếu sub-call đó tự nó
/// không có deadline riêng (biến thể *NoDeadline).
fn decode_multicall(args: &[u8], has_deadline: bool) -> Result<DecodedSwap, SkipReason> {
    let calls_word_idx = if has_deadline { 1 } else { 0 };
    let calls = dynamic_bytes_array_at_offset(args, calls_word_idx).ok_or(SkipReason::DecodeFail)?;
    let outer_deadline = if has_deadline { u256_at(args, 0) } else { None };

    let mut found: Option<DecodedSwap> = None;
    for call in &calls {
        if call.len() < 4 {
            continue;
        }
        let (csel_slice, cargs) = call.split_at(4);
        let csel: [u8; 4] = match csel_slice.try_into() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if let Some(result) = try_decode_token_swap_selector(csel, cargs) {
            let mut decoded = result?;
            if found.is_some() {
                // >1 sub-call swap trong cung 1 multicall - khong doan thu tu
                // tac dong (co the la multihop qua nhieu lan goi rieng) - bo
                // qua toan bo, dung decode_fail thay vi chon dai 1 cai.
                return Err(SkipReason::DecodeFail);
            }
            if decoded.deadline.is_none() {
                decoded.deadline = outer_deadline;
            }
            found = Some(decoded);
        }
    }
    found.ok_or(SkipReason::DecodeFail)
}

/// Cụm `decoder-coverage` (B2) — Universal Router `execute()`, hỗ trợ NHIỀU
/// command trong 1 tx (trước đó chỉ nhận ĐÚNG 1 command/1 input, mọi tx
/// multicall thật -> `decode_fail` 100%, xem AGENTS.md lệnh B2 + mẫu thật
/// `tests/fixtures/ur_calldata.jsonl`). Chiến lược: tìm command SWAP đầu tiên
/// (`V2_SWAP_EXACT_IN`/`V3_SWAP_EXACT_IN`) trong chuỗi — các command khác chỉ
/// là tiền trạm/hậu trạm (`WRAP_ETH`/`PERMIT2_PERMIT` trước; `UNWRAP_WETH`/
/// `SWEEP`/`TRANSFER` sau) KHÔNG đổi path/hướng mua-bán của swap đó (hướng
/// mua/bán đã tự xác định đúng qua `path.token_a` ở tầng `pipeline.rs`, xem
/// `decode_and_classify`/`decode_and_classify_quote` — decoder KHÔNG cần biết
/// trước "đây là bán" chỉ vì thấy `UNWRAP_WETH` theo sau). Không tìm thấy
/// command swap nào (vd toàn `SEEAPORT_V1_5`/lệnh NFT khác — THỰC TẾ CHIẾM
/// ĐA SỐ mẫu `execute()` "decode_fail" cũ, ~81% trong mẫu B1, KHÔNG PHẢI
/// swap bị bỏ sót) -> `decode_fail`, đúng, không phải bug.
fn decode_universal_router(args: &[u8], tx_value: U256, has_deadline_param: bool) -> Result<DecodedSwap, SkipReason> {
    let commands = dynamic_bytes_at_offset(args, 0).ok_or(SkipReason::DecodeFail)?;
    let inputs = dynamic_bytes_array_at_offset(args, 1).ok_or(SkipReason::DecodeFail)?;
    if commands.is_empty() || commands.len() != inputs.len() {
        return Err(SkipReason::DecodeFail);
    }
    let deadline_override = if has_deadline_param { u256_at(args, 2) } else { None };
    let masked: Vec<u8> = commands.iter().map(|c| c & 0x3f).collect();

    let swap_pos = masked.iter().position(|&c| c == CMD_V2_SWAP_EXACT_IN || c == CMD_V3_SWAP_EXACT_IN);
    let Some(swap_pos) = swap_pos else {
        return Err(SkipReason::DecodeFail);
    };
    let cmd = masked[swap_pos];
    let swap_input = &inputs[swap_pos];

    let mut decoded =
        if cmd == CMD_V2_SWAP_EXACT_IN { decode_ur_v2_swap_exact_in(swap_input) } else { decode_ur_v3_swap_exact_in(swap_input) }?;

    // Cụm B2 — PERMIT2_PERMIT TRƯỚC swap -> payerIsUser (word idx4, field
    // TĨNH cuối `V2SwapExactInParams`/`V3SwapExactInParams`, luôn nằm inline
    // đúng vị trí bất kể `path` dynamic dài bao nhiêu) BẮT BUỘC = true — nếu
    // không, router đứng ra làm payer (không phải người gọi), giả định "attacker
    // biết nguồn vốn victim" không còn đúng -> decode_fail (an toàn), KHÔNG bịa.
    let has_permit2_before = masked[..swap_pos].iter().any(|&c| c == CMD_PERMIT2_PERMIT);
    if has_permit2_before {
        let payer_is_user = u256_at(swap_input, 4).ok_or(SkipReason::DecodeFail)? != U256::ZERO;
        if !payer_is_user {
            return Err(SkipReason::DecodeFail);
        }
    }

    // Cụm B2 — sentinel CONTRACT_BALANCE + WRAP_ETH truoc swap -> amount_in
    // THAT = tx.value (BNB gui kem giao dich nay). Xem doc-comment
    // `contract_balance_sentinel` (CHƯA quan sát được sentinel này trong mẫu
    // THẬT phiên này — nhánh này là xử lý phòng vệ theo đúng lệnh, không phải
    // đã verify runtime).
    if decoded.amount_in == contract_balance_sentinel() {
        let has_wrap_before = masked[..swap_pos].iter().any(|&c| c == CMD_WRAP_ETH);
        if !has_wrap_before {
            return Err(SkipReason::DecodeFail);
        }
        decoded.amount_in = tx_value;
    }

    if let Some(d) = deadline_override {
        decoded.deadline = Some(d);
    }
    Ok(decoded)
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

    /// Cụm `decoder-coverage` (B2, fix bug pre-existing) — `exactInput` nhận
    /// ĐÚNG 1 tham số struct `(bytes,address,uint256,uint256,uint256)` chứa
    /// field dynamic ở đầu -> 2 LỚP offset THẬT (đối chiếu calldata mainnet
    /// thật, xem doc-comment `dynamic_bytes_leading_field_of_sole_tuple_param`):
    /// word0 = offset NGOÀI (luôn `0x20`, trỏ ngay sau chính nó), rồi TẠI ĐÓ
    /// mới là 5 head-word của struct (offset TRONG trỏ path + 4 field tĩnh),
    /// rồi path length+data. Fixture CŨ (trước fix) chỉ có 1 lớp (thiếu hẳn
    /// word `0x20` mở đầu) — SAI so với ABI thật, che giấu bug decode.
    fn build_exact_input(amount_in: u64, path_bytes: &[u8]) -> Vec<u8> {
        let mut out = SEL_EXACT_INPUT.to_vec();
        out.extend_from_slice(&pad_u256(0x20)); // offset NGOAI - tuple bat dau ngay sau word nay
        // ===== tuple bat dau tai day (5 head-word cua struct) =====
        out.extend_from_slice(&pad_u256(0xa0)); // offset TRONG (relative tuple start) toi path = 5 head word * 32
        out.extend_from_slice(&pad_addr(addr("0x777777777777777777777777777777777777f0f0"))); // recipient
        out.extend_from_slice(&pad_u256(9_999_999_999)); // deadline
        out.extend_from_slice(&pad_u256(amount_in));
        out.extend_from_slice(&pad_u256(0)); // amountOutMinimum
        // ===== tail: path bytes =====
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

    /// ĐẠT CẦN DÁN (B4) — cross-check 2 CHIỀU cho 4 selector_name MỚI (B2/B3):
    /// biến thể không-deadline (`exactInputSingleNoDeadline`) và exact-output
    /// (`exactOutputSingle(NoDeadline)`/`exactOutput(NoDeadline)`) PHẢI cùng
    /// nhóm router hợp lệ (V3 SwapRouter/SmartRouter) như 2 selector cũ, và
    /// PHẢI mismatch khi gửi nhầm tới V2 Router/Universal Router.
    #[test]
    fn venue_matches_router_new_no_deadline_and_exact_output_selectors_two_way_check() {
        for sel in [
            "exactInputSingleNoDeadline",
            "exactOutputSingle",
            "exactOutputSingleNoDeadline",
            "exactOutput",
            "exactOutputNoDeadline",
        ] {
            assert!(venue_matches_router(sel, Some(Venue::V3)), "{sel} phai khop Venue::V3");
            assert!(venue_matches_router(sel, Some(Venue::SmartRouter)), "{sel} phai khop SmartRouter");
            assert!(!venue_matches_router(sel, Some(Venue::V2)), "{sel} toi V2 Router phai la mismatch");
            assert!(!venue_matches_router(sel, Some(Venue::UniversalRouter)), "{sel} toi UR truc tiep phai la mismatch");
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

    // ===== Cụm `decoder-coverage` (B1-B4) — UR đa lệnh + SmartRouter multicall =====

    fn hex_to_bytes(s: &str) -> Vec<u8> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    /// ĐẠT CẦN DÁN (B1 bảng thống kê, hàng SEAPORT_V1_5) — mẫu THẬT bắt được
    /// phiên này (`hash=0xffdbc947...`, `to`=UR Infinity đã pin, `tests/fixtures/ur_calldata.jsonl`)
    /// chứng minh: ~90% `decode_fail` cũ của UR Infinity KHÔNG PHẢI swap bị bỏ
    /// sót — là lệnh NFT marketplace (`SEAPORT_V1_5`, command `0x10`) hoàn
    /// toàn ngoài phạm vi bot. Sau B2, calldata này VẪN `decode_fail` — ĐÚNG,
    /// không phải bug còn sót.
    #[test]
    fn decode_universal_router_real_seaport_calldata_is_correctly_decode_fail() {
        let raw = hex_to_bytes("0xffdbc947663ff26e489e7c4207238fca3f26e53f41b40972a39d2bbf16106985"); // khong dung, chi de gan ten bien ro rang o duoi
        let _ = raw; // hash chi de doi chieu voi fixture file, khong phai calldata
        let calldata = hex_to_bytes("0x24856bc300000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000001100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000038000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000003080c1000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000026000000000000000000000000000000000000000000000000000000000000018000000000000000000000000000000000000000000000000000000000000002000000000000000000000000055d398326f99059ff775485246999027b3197955000000000000000000000000eec6574eabba52bac3f0277f2cd5ac7e67197886000000000000000000000000b0bb171d333569cfd28a37f5c5dddaaa90ad46af000000000000000000000000a0ffb9c1ce1fe56963b0321b32e7a0302114058b0000000000000000000000000000000000000000000000000000000000000043000000000000000000000000000000000000000000000000000000000001004500000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000b997f87127b244270700000000000000000000000000000000000000000000000f6a907309456a35f900000000000000000000000000000000000000000000000000000000000001400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004000000000000000000000000055d398326f99059ff775485246999027b319795500000000000000000000000000000000000000000000000f6a907309456a35f90000000000000000000000000000000000000000000000000000000000000060000000000000000000000000eec6574eabba52bac3f0277f2cd5ac7e671978860000000000000000000000002783a875a4450003a04a37fdaaf7a9c3bbc974630000000000000000000000000000000000000000000000000000000000002710");
        let err = decode_swap_calldata(&calldata, U256::ZERO).unwrap_err();
        assert_eq!(err, SkipReason::DecodeFail);
    }

    /// ĐẠT CẦN DÁN (B2) — `WRAP_ETH`→`V2_SWAP_EXACT_IN` (mẫu THẬT,
    /// `hash=0xfca5fffcbd32...`, `to`=UR Infinity) PHẢI decode được (trước fix:
    /// `decode_fail` 100% vì `commands.len()!=1`).
    #[test]
    fn decode_universal_router_real_wrap_eth_then_v2_swap_decodes() {
        let calldata = hex_to_bytes("0x3593564c000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000006aa9278a00000000000000000000000000000000000000000000000000000000000000020b080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000001988fe4052b80000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000001988fe4052b8000000000000000000000000000000000000000000000000c4965190bc34233326000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000bb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c0000000000000000000000002ab8a4dd2191989ac2898006df350b236d2b7777");
        let tx_value = U256::from_str_radix("1988fe4052b8000", 16).unwrap(); // dung so THAT tu chinh calldata nay (WRAP_ETH amount)
        let decoded = decode_swap_calldata(&calldata, tx_value).expect("phai decode duoc WRAP_ETH+V2_SWAP_EXACT_IN that");
        assert_eq!(decoded.selector_name, "UR:V2_SWAP_EXACT_IN");
        assert_eq!(decoded.venue, SwapVenue::V2);
        assert_eq!(decoded.amount_in, tx_value, "amount_in phai khop so THAT trong calldata (khong phai sentinel)");
        assert_eq!(decoded.path.token_a, wbnb_addr());
    }

    /// ĐẠT CẦN DÁN (B2) — `PERMIT2_PERMIT`→`V2_SWAP_EXACT_IN` (mẫu THẬT,
    /// `hash=0xfff720bd748c...` KHÔNG dùng — đây là mẫu khác có PERMIT2_PERMIT
    /// đứng trước V2_SWAP_EXACT_IN, `payerIsUser=true` thật) PHẢI decode được.
    #[test]
    fn decode_universal_router_real_permit2_then_v2_swap_decodes() {
        let calldata = hex_to_bytes("0x3593564c000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000006aa925f100000000000000000000000000000000000000000000000000000000000000030a080c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001e00000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000016000000000000000000000000090ec3140cc5ef37beee18bc9f6909958eba24444000000000000000000000000ffffffffffffffffffffffffffffffffffffffff0000000000000000000000000000000000000000000000000000ffffffffffff00000000000000000000000000000000000000000000000000000000000000000000000000000000000000001a0a18ac4becddbd6389559687d1a73d8927e416000000000000000000000000000000000000000000000000000000006aa925f100000000000000000000000000000000000000000000000000000000000000e00000000000000000000000000000000000000000000000000000000000000041b6a82c4ec7818eafaedc5462b81df2f1c14f89642ae06eab652d6306d2631a4654f7abf3fb6b118f56f051f5c50a5a66b67f05165b7192d1a6531fad31f751c11b0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000001a0a18ac4becddbd6389559687d1a73d8927e416000000000000000000000000000000000000000000153a218703c011228c83fd0000000000000000000000000000000000000000000000000124945394acffb100000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000200000000000000000000000090ec3140cc5ef37beee18bc9f6909958eba24444000000000000000000000000bb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c000000000000000000000000000000000000000000000000000000000000004000000000000000000000000003540f1771f1035e7817d7dfac07b06c6eff83af0000000000000000000000000000000000000000000000000124945394acffb1");
        let decoded = decode_swap_calldata(&calldata, U256::ZERO).expect("phai decode duoc PERMIT2_PERMIT+V2_SWAP_EXACT_IN that");
        assert_eq!(decoded.selector_name, "UR:V2_SWAP_EXACT_IN");
        assert_eq!(decoded.token().unwrap(), addr("0x90ec3140cc5ef37beee18bc9f6909958eba24444"));
    }

    /// `PERMIT2_PERMIT` trước swap nhưng `payerIsUser=false` -> decode_fail
    /// (an toàn — router đứng ra làm payer, không phải người gọi).
    #[test]
    fn decode_universal_router_permit2_then_v2_swap_payer_not_user_is_decode_fail() {
        let permit2_input = {
            // PERMIT2_PERMIT that duoc rut gon toi muc toi thieu decoder can:
            // decoder khong doc noi dung PERMIT2_PERMIT (bo qua hoan toan),
            // chi can no TON TAI truoc swap trong danh sach command.
            vec![0u8; 32]
        };
        let mut v2_input = build_v2_swap_exact_in_input(1, &[wbnb_addr(), token_addr()]);
        // payerIsUser nam o word idx4 (sau bool_word da dat true trong helper) -
        // ghi de lai thanh false (32 byte 0).
        v2_input[4 * 32..5 * 32].copy_from_slice(&[0u8; 32]);
        let calldata = build_ur_execute_multi(&[(CMD_PERMIT2_PERMIT, &permit2_input), (CMD_V2_SWAP_EXACT_IN, &v2_input)]);
        let err = decode_swap_calldata(&calldata, U256::ZERO).unwrap_err();
        assert_eq!(err, SkipReason::DecodeFail);
    }

    /// `WRAP_ETH`→`V3_SWAP_EXACT_IN` (mẫu THẬT, `hash=0xfc745adae487...`) —
    /// venue phải là V3 (đếm `venue_unpinned`, chưa sim), decode KHÔNG
    /// `decode_fail`.
    #[test]
    fn decode_universal_router_real_wrap_eth_then_v3_swap_has_v3_venue() {
        // Dung lai calldata WRAP_V2 nhung doi command thu 2 tu V2_IN(0x08)
        // sang V3_IN(0x00) + input dang packed-path (khong dung du lieu that
        // vi khong co san mau nay trong fixture 45 dong, dung fixture tay
        // nhung theo DUNG cau truc command da xac nhan qua sample that).
        let path_bytes = packed_v3_path_single_hop(wbnb_addr(), 500, token_addr());
        let v3_input = build_v3_swap_exact_in_input(123, &path_bytes);
        let wrap_input = vec![0u8; 64]; // WRAP_ETH(address,uint256) - decoder khong doc, chi can ton tai
        let calldata = build_ur_execute_multi(&[(CMD_WRAP_ETH, &wrap_input), (CMD_V3_SWAP_EXACT_IN, &v3_input)]);
        let decoded = decode_swap_calldata(&calldata, U256::from(1u64)).expect("phai decode duoc");
        assert_eq!(decoded.venue, SwapVenue::V3 { fee: 500 });
        assert_eq!(decoded.selector_name, "UR:V3_SWAP_EXACT_IN");
    }

    /// Sentinel `CONTRACT_BALANCE` trong `V2_SWAP_EXACT_IN.amountIn` + có
    /// `WRAP_ETH` trước -> `amount_in` PHẢI thay bằng `tx.value`.
    #[test]
    fn decode_universal_router_contract_balance_sentinel_with_wrap_eth_uses_tx_value() {
        let mut v2_input = build_v2_swap_exact_in_input(1, &[wbnb_addr(), token_addr()]);
        let sentinel = contract_balance_sentinel().to_be_bytes::<32>();
        v2_input[32..64].copy_from_slice(&sentinel); // amountIn = word idx1
        let wrap_input = vec![0u8; 64];
        let calldata = build_ur_execute_multi(&[(CMD_WRAP_ETH, &wrap_input), (CMD_V2_SWAP_EXACT_IN, &v2_input)]);
        let tx_value = U256::from(777_000_000_000_000_000u64);
        let decoded = decode_swap_calldata(&calldata, tx_value).expect("phai decode duoc");
        assert_eq!(decoded.amount_in, tx_value);
    }

    /// Sentinel `CONTRACT_BALANCE` nhưng KHÔNG có `WRAP_ETH` trước -> không
    /// biết lấy số thật từ đâu -> `decode_fail` (an toàn, không đoán = tx.value).
    #[test]
    fn decode_universal_router_contract_balance_sentinel_without_wrap_eth_is_decode_fail() {
        let mut v2_input = build_v2_swap_exact_in_input(1, &[wbnb_addr(), token_addr()]);
        let sentinel = contract_balance_sentinel().to_be_bytes::<32>();
        v2_input[32..64].copy_from_slice(&sentinel);
        let calldata = build_ur_execute(CMD_V2_SWAP_EXACT_IN, &v2_input);
        let err = decode_swap_calldata(&calldata, U256::from(1u64)).unwrap_err();
        assert_eq!(err, SkipReason::DecodeFail);
    }

    /// `execute(bytes,bytes[],uint256)` (SEL_UR_EXECUTE_3, có deadline) —
    /// deadline phải được đọc và gán vào `DecodedSwap.deadline` (trước đó UR
    /// LUÔN `None`, kể cả bản có tham số deadline).
    #[test]
    fn decode_universal_router_execute3_deadline_is_captured() {
        let calldata_2param = build_ur_execute(CMD_V2_SWAP_EXACT_IN, &build_v2_swap_exact_in_input(42, &[wbnb_addr(), token_addr()]));
        // Chuyen tu SEL_UR_EXECUTE_2 sang SEL_UR_EXECUTE_3 bang cach dung lai
        // builder execute_multi (co tham so deadline).
        let _ = calldata_2param;
        let calldata = build_ur_execute3(CMD_V2_SWAP_EXACT_IN, &build_v2_swap_exact_in_input(42, &[wbnb_addr(), token_addr()]), 9_999_999_999);
        let decoded = decode_swap_calldata(&calldata, U256::ZERO).expect("phai decode duoc");
        assert_eq!(decoded.deadline, Some(U256::from(9_999_999_999u64)));
    }

    /// >1 command khớp SWAP trong 1 `execute()` (2 lần `V2_SWAP_EXACT_IN` liên
    /// tiếp không WRAP/PERMIT2 xen giữa) — hàm chọn command SWAP ĐẦU TIÊN, vẫn
    /// decode được (không phải multihop qua nhiều router call riêng biệt như
    /// multicall, chỉ là 1 chuỗi command trong CÙNG 1 `execute()` — vẫn lấy
    /// hop đầu, ghi rõ đây là đơn giản hoá có chủ đích, không phải multihop
    /// đầy đủ).
    #[test]
    fn decode_universal_router_multiple_swap_commands_takes_first() {
        let input1 = build_v2_swap_exact_in_input(111, &[wbnb_addr(), token_addr()]);
        let input2 = build_v2_swap_exact_in_input(222, &[token_addr(), wbnb_addr()]);
        let calldata = build_ur_execute_multi(&[(CMD_V2_SWAP_EXACT_IN, &input1), (CMD_V2_SWAP_EXACT_IN, &input2)]);
        let decoded = decode_swap_calldata(&calldata, U256::ZERO).expect("phai decode duoc");
        assert_eq!(decoded.amount_in, U256::from(111u64));
    }

    // ===== B3 — SmartRouter multicall =====

    fn build_multicall(calls: &[Vec<u8>]) -> Vec<u8> {
        let mut out = SEL_MULTICALL.to_vec();
        out.extend_from_slice(&pad_u256(0x20)); // offset bytes[]
        out.extend_from_slice(&pad_u256(calls.len() as u64));
        let head_words = calls.len();
        let mut running_offset = head_words * 32;
        let mut heads = Vec::new();
        let mut tails = Vec::new();
        for c in calls {
            heads.extend_from_slice(&pad_u256(running_offset as u64));
            let mut tail = Vec::new();
            tail.extend_from_slice(&pad_u256(c.len() as u64));
            tail.extend_from_slice(c);
            let rem = c.len() % 32;
            if rem != 0 {
                tail.extend(std::iter::repeat(0u8).take(32 - rem));
            }
            running_offset += tail.len();
            tails.push(tail);
        }
        out.extend_from_slice(&heads);
        for t in tails {
            out.extend_from_slice(&t);
        }
        out
    }

    fn build_exact_input_single_no_deadline(token_in: Address, token_out: Address, fee: u64, amount_in: u64) -> Vec<u8> {
        let mut out = SEL_EXACT_INPUT_SINGLE_NO_DEADLINE.to_vec();
        out.extend_from_slice(&pad_addr(token_in));
        out.extend_from_slice(&pad_addr(token_out));
        out.extend_from_slice(&pad_u256(fee));
        out.extend_from_slice(&pad_addr(addr("0x666666666666666666666666666666666666e0e0")));
        out.extend_from_slice(&pad_u256(amount_in));
        out.extend_from_slice(&pad_u256(0)); // amountOutMinimum
        out.extend_from_slice(&pad_u256(0)); // sqrtPriceLimitX96
        out
    }

    /// ĐẠT CẦN DÁN (B3) — `multicall(bytes[])` bọc 1 `exactInputSingle` biến
    /// thể KHÔNG deadline (`0x04e45aaf`, xác nhận THẬT qua mempool — xem
    /// `tests/fixtures/ur_calldata.jsonl`) — decode phải ra ĐÚNG token/venue
    /// V3, KHÔNG còn `decode_fail`.
    #[test]
    fn decode_multicall_bytes_wraps_exact_input_single_no_deadline() {
        let call = build_exact_input_single_no_deadline(wbnb_addr(), token_addr(), 2500, 555);
        let calldata = build_multicall(&[call]);
        let decoded = decode_swap_calldata(&calldata, U256::ZERO).expect("phai decode duoc multicall(bytes[])");
        assert_eq!(decoded.selector_name, "exactInputSingleNoDeadline");
        assert_eq!(decoded.venue, SwapVenue::V3 { fee: 2500 });
        assert_eq!(decoded.amount_in, U256::from(555u64));
        assert_eq!(decoded.token().unwrap(), token_addr());
    }

    /// `multicall(bytes[])` với 1 sub-call swap + 1 sub-call KHÔNG phải swap
    /// (`refundETH()`, selector lạ với decoder) — sub-call lạ bị bỏ qua, KHÔNG
    /// làm hỏng việc decode sub-call swap.
    #[test]
    fn decode_multicall_ignores_non_swap_subcalls() {
        let swap_call = build_exact_input_single_no_deadline(wbnb_addr(), token_addr(), 100, 999);
        let refund_call = hex_to_bytes("0x12210e8a"); // refundETH() - decoder khong nhan dien
        let calldata = build_multicall(&[refund_call, swap_call]);
        let decoded = decode_swap_calldata(&calldata, U256::ZERO).expect("phai decode duoc, bo qua sub-call la");
        assert_eq!(decoded.amount_in, U256::from(999u64));
    }

    /// 2 sub-call ĐỀU là swap trong CÙNG 1 multicall -> `decode_fail` (không
    /// đoán hop nào là "thật", tránh multihop qua nhiều lời gọi riêng biệt bị
    /// hiểu sai thành 1 swap đơn).
    #[test]
    fn decode_multicall_two_swap_subcalls_is_decode_fail() {
        let call1 = build_exact_input_single_no_deadline(wbnb_addr(), token_addr(), 100, 1);
        let call2 = build_exact_input_single_no_deadline(token_addr(), wbnb_addr(), 100, 2);
        let calldata = build_multicall(&[call1, call2]);
        let err = decode_swap_calldata(&calldata, U256::ZERO).unwrap_err();
        assert_eq!(err, SkipReason::DecodeFail);
    }

    /// `multicall(uint256,bytes[])` (có deadline ngoài) bọc sub-call
    /// `exactInputSingleNoDeadline` (tự nó không có deadline) -> deadline
    /// NGOÀI phải được gán vào kết quả.
    #[test]
    fn decode_multicall_with_deadline_propagates_to_subcall_without_own_deadline() {
        let call = build_exact_input_single_no_deadline(wbnb_addr(), token_addr(), 500, 42);
        let mut out = SEL_MULTICALL_DEADLINE.to_vec();
        out.extend_from_slice(&pad_u256(9_999_999_999)); // deadline (inline, KHONG phai offset)
        out.extend_from_slice(&pad_u256(0x40)); // offset bytes[] = ngay sau 2 head word
        out.extend_from_slice(&pad_u256(1)); // so luong sub-call
        out.extend_from_slice(&pad_u256(0x20)); // offset phan tu 0 (tinh tu sau length word)
        out.extend_from_slice(&pad_u256(call.len() as u64));
        out.extend_from_slice(&call);
        let rem = call.len() % 32;
        if rem != 0 {
            out.extend(std::iter::repeat(0u8).take(32 - rem));
        }
        let decoded = decode_swap_calldata(&out, U256::ZERO).expect("phai decode duoc multicall(uint256,bytes[])");
        assert_eq!(decoded.deadline, Some(U256::from(9_999_999_999u64)));
    }

    /// ĐẠT CẦN DÁN (B2, fix bug pre-existing) — `exactOutput` KHÔNG-deadline
    /// (SmartRouter, selector `0x09b81346`) dùng ĐÚNG calldata THẬT bắt được
    /// phiên này (`hash=0xff500151ca688d2b...`, xem
    /// `tests/fixtures/ur_calldata.jsonl`) — GOLD STANDARD verify 2-lớp offset
    /// (xem doc-comment `dynamic_bytes_leading_field_of_sole_tuple_param`).
    #[test]
    fn decode_exact_output_no_deadline_matches_real_smartrouter_calldata() {
        let calldata = hex_to_bytes("0x09b81346000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000800000000000000000000000007d9dc5891aa3145a16c3b1e8b5e44b7a2c3813ed00000000000000000000000000000000000000000000001320a8d2d0cbfdc00000000000000000000000000000000000000000000000000e7df89c8fe6e6d060000000000000000000000000000000000000000000000000000000000000002b10d4183389e99233db3cc981c43443ebd28ebd5e00006455d398326f99059ff775485246999027b3197955000000000000000000000000000000000000000000");
        let decoded = decode_swap_calldata(&calldata, U256::ZERO).expect("phai decode duoc calldata that");
        assert_eq!(decoded.selector_name, "exactOutputNoDeadline");
        assert_eq!(decoded.deadline, None);
        // recipient doc dung tu tuple (khong phai gia tri rac cua layout 1 lop sai).
        assert_eq!(decoded.to, addr("0x7d9dc5891aa3145a16c3b1e8b5e44b7a2c3813ed"));
        assert_eq!(decoded.venue, SwapVenue::V3 { fee: 100 }); // fee tier doc tu path packed (0x000064 = 100)
        // path packed: 10d4183389e99233db3cc981c43443ebd28ebd5e | 000064 | 55d398326f99059ff775485246999027b3197955
        assert_eq!(decoded.path.token_a, addr("0x10d4183389e99233db3cc981c43443ebd28ebd5e"));
        assert_eq!(decoded.path.token_b, addr("0x55d398326f99059ff775485246999027b3197955")); // USDT
        assert_eq!(decoded.token().unwrap_err(), SkipReason::NotWbnbPair, "path nay khong co WBNB, dung ly");
    }

    /// `exactOutputSingle` KHÔNG-deadline (SmartRouter, `0x5023b4df`, all-static
    /// tuple) — decode đủ để phân loại `SwapVenue::V3`.
    #[test]
    fn decode_exact_output_single_no_deadline_decodes_v3_venue() {
        let mut out = SEL_EXACT_OUTPUT_SINGLE_NO_DEADLINE.to_vec();
        out.extend_from_slice(&pad_addr(token_addr())); // tokenIn
        out.extend_from_slice(&pad_addr(wbnb_addr())); // tokenOut
        out.extend_from_slice(&pad_u256(2500)); // fee
        out.extend_from_slice(&pad_addr(addr("0x666666666666666666666666666666666666e0e0"))); // recipient
        out.extend_from_slice(&pad_u256(1_000_000)); // amountOut
        out.extend_from_slice(&pad_u256(2_000_000)); // amountInMaximum
        out.extend_from_slice(&pad_u256(0)); // sqrtPriceLimitX96
        let decoded = decode_swap_calldata(&out, U256::ZERO).expect("phai decode duoc");
        assert_eq!(decoded.selector_name, "exactOutputSingleNoDeadline");
        assert_eq!(decoded.venue, SwapVenue::V3 { fee: 2500 });
        assert_eq!(decoded.deadline, None);
    }

    // ===== Helper builder dùng chung cho test B2 =====

    fn build_ur_execute_multi(cmds: &[(u8, &[u8])]) -> Vec<u8> {
        let mut out = SEL_UR_EXECUTE_2.to_vec();
        out.extend_from_slice(&pad_u256(0x40)); // offset commands
        let commands_words = ((cmds.len() + 31) / 32).max(1) as u64; // 1 byte/command, lam tron 32
        let offset_inputs = 0x40 + 32 + commands_words * 32;
        out.extend_from_slice(&pad_u256(offset_inputs));
        // commands (bytes)
        out.extend_from_slice(&pad_u256(cmds.len() as u64));
        let mut cmd_bytes = vec![0u8; (commands_words * 32) as usize];
        for (i, (c, _)) in cmds.iter().enumerate() {
            cmd_bytes[i] = *c;
        }
        out.extend_from_slice(&cmd_bytes);
        // inputs (bytes[])
        out.extend_from_slice(&pad_u256(cmds.len() as u64));
        let head_words = cmds.len();
        let mut running_offset = head_words * 32;
        let mut heads = Vec::new();
        let mut tails = Vec::new();
        for (_, input) in cmds {
            heads.extend_from_slice(&pad_u256(running_offset as u64));
            let mut tail = Vec::new();
            tail.extend_from_slice(&pad_u256(input.len() as u64));
            tail.extend_from_slice(input);
            let rem = input.len() % 32;
            if rem != 0 {
                tail.extend(std::iter::repeat(0u8).take(32 - rem));
            }
            running_offset += tail.len();
            tails.push(tail);
        }
        out.extend_from_slice(&heads);
        for t in tails {
            out.extend_from_slice(&t);
        }
        out
    }

    fn build_ur_execute3(command: u8, input: &[u8], deadline: u64) -> Vec<u8> {
        // execute(bytes commands, bytes[] inputs, uint256 deadline) - 3 tham
        // so dau, offset tinh theo 3 head word (0x60).
        let mut out = SEL_UR_EXECUTE_3.to_vec();
        out.extend_from_slice(&pad_u256(0x60)); // offset commands
        let offset_inputs = 0x60 + 32 + 32; // sau: length word + 1 data word cua commands (1 command)
        out.extend_from_slice(&pad_u256(offset_inputs as u64));
        out.extend_from_slice(&pad_u256(deadline));
        out.extend_from_slice(&pad_u256(1)); // commands.length = 1
        let mut cmd_word = [0u8; 32];
        cmd_word[0] = command;
        out.extend_from_slice(&cmd_word);
        out.extend_from_slice(&pad_u256(1)); // inputs.length = 1
        out.extend_from_slice(&pad_u256(0x20));
        out.extend_from_slice(&pad_u256(input.len() as u64));
        out.extend_from_slice(input);
        let rem = input.len() % 32;
        if rem != 0 {
            out.extend(std::iter::repeat(0u8).take(32 - rem));
        }
        out
    }
}

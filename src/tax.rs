//! Cụm 3.3 "Tax stub" — cache đo tax roundtrip theo block, hết hạn sau
//! `tax_cache_blocks` (AGENTS.md mục "Config"). CHƯA đo -> `honeypot_or_tax`
//! SKIP (đúng luật, mặc định an toàn — `TaxCache::get_fresh` trả `None`).
//!
//! `measure_roundtrip_via_router` viết plumbing `eth_call` THẬT (2 chặng:
//! mua qua `getAmountsOut([WBNB,token])` rồi bán lại
//! `getAmountsOut([token,WBNB])` bằng đúng router V2 đã pin) — nhưng đây là
//! số đo AMM-math THUẦN (phí 0.25% x2 + price impact), KHÔNG PHẢI tax
//! fee-on-transfer thật của token. Lý do: `UniswapV2Router02`-style tính
//! `amounts[]` trả về từ CÔNG THỨC `getAmountsOut` nội bộ RỒI mới transfer —
//! không đọc lại `balanceOf` thực tế sau khi transfer, nên với MỌI token
//! (kể cả token có tax), `amounts[]` trả về luôn giống hệt `getAmountsOut`
//! độc lập — hai eth_call trên KHÔNG THỂ thấy được tax thật bằng chứng minh
//! toán học (không phải giả định). Biến thể
//! `swapExactETHForTokensSupportingFeeOnTransferTokens` có đọc `balanceOf`
//! thật nhưng là hàm `void` (không trả amount, chỉ revert theo
//! `amountOutMin`) nên cũng không tự đọc được số liệu qua 1 `eth_call`.
//!
//! Đo tax thật (balanceOf trước/sau 1 giao dịch thật) cần 1 hợp đồng "probe"
//! triển khai TẠM trong đúng 1 `eth_call` (kỹ thuật honeypot-detector chuẩn:
//! `to: null` + bytecode tạo hợp đồng thực hiện mua->đọc balance->bán->đọc
//! balance->revert kèm data) — NGOÀI PHẠM VI phiên này (cần viết/kiểm bytecode
//! EVM tay hoặc compiler, rủi ro sai cao nếu làm vội), ghi rõ CÒN NỢ, không
//! bịa số đo giả. VÌ VẬY: cache ở đây KHÔNG được tự động điền bởi
//! `measure_roundtrip_via_router` trong pipeline `4.1` — chỉ điền thủ công
//! (test/chủ) cho tới khi có hàm đo thật ở cụm sau.

use alloy::primitives::{keccak256, Address, U256};
use alloy::providers::Provider;
use alloy::rpc::types::eth::TransactionRequest;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::LazyLock;

fn selector(sig: &str) -> [u8; 4] {
    let hash = keccak256(sig.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}

static SEL_GET_AMOUNTS_OUT: LazyLock<[u8; 4]> = LazyLock::new(|| selector("getAmountsOut(uint256,address[])"));

fn build_get_amounts_out_calldata(amount_in: U256, path: &[Address]) -> Vec<u8> {
    let mut out = SEL_GET_AMOUNTS_OUT.to_vec();
    out.extend_from_slice(&amount_in.to_be_bytes::<32>());
    let mut offset_word = [0u8; 32];
    offset_word[31] = 0x40; // offset path = 2 head word * 32
    out.extend_from_slice(&offset_word);
    let mut len_word = [0u8; 32];
    len_word[31] = path.len() as u8; // path chi 2 phan tu trong pham vi phien nay
    out.extend_from_slice(&len_word);
    for a in path {
        out.extend_from_slice(&[0u8; 12]);
        out.extend_from_slice(a.as_slice());
    }
    out
}

/// `uint256[] amounts` trả về của `getAmountsOut` — chỉ cần phần tử CUỐI
/// (amountOut của chặng này). Layout ABI: offset word (luôn `0x20` vì đây là
/// return value dynamic DUY NHẤT) rồi length rồi N phần tử.
fn decode_last_amounts_out(ret: &[u8]) -> Option<U256> {
    if ret.len() < 64 {
        return None;
    }
    let len = usize::try_from(U256::from_be_slice(&ret[32..64])).ok()?;
    if len == 0 {
        return None;
    }
    let last_start = 64 + (len - 1) * 32;
    ret.get(last_start..last_start + 32).map(U256::from_be_slice)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundtripQuote {
    pub buy_out: U256,
    pub sell_back: U256,
    pub measured_at_block: u64,
}

impl RoundtripQuote {
    /// Hao hụt roundtrip tính theo basis point, U256 nguyên (không float) —
    /// CHỈ là số tham khảo AMM-math (xem doc-comment đầu file), KHÔNG PHẢI
    /// tax thật. `None` nếu `probe_in = 0`.
    pub fn loss_bps(&self, probe_in: U256) -> Option<u64> {
        if probe_in.is_zero() {
            return None;
        }
        if self.sell_back >= probe_in {
            return Some(0);
        }
        let diff = probe_in - self.sell_back;
        let bps = diff.checked_mul(U256::from(10_000u64))? / probe_in;
        u64::try_from(bps).ok()
    }
}

/// `eth_call` THẬT 2 chặng qua V2 Router đã pin — xem giới hạn ở doc-comment
/// đầu file (KHÔNG phát hiện được fee-on-transfer thật).
pub async fn measure_roundtrip_via_router(
    provider: &dyn Provider,
    router: Address,
    wbnb: Address,
    token: Address,
    probe_in: U256,
    current_block: u64,
) -> Result<RoundtripQuote, String> {
    let buy_calldata = build_get_amounts_out_calldata(probe_in, &[wbnb, token]);
    let buy_tx = TransactionRequest::default().to(router).input(buy_calldata.into());
    let buy_ret = provider.call(buy_tx).await.map_err(|e| format!("eth_call getAmountsOut(buy) that bai: {e}"))?;
    let buy_out =
        decode_last_amounts_out(&buy_ret).ok_or_else(|| "getAmountsOut(buy) tra ve du lieu qua ngan".to_string())?;

    let sell_calldata = build_get_amounts_out_calldata(buy_out, &[token, wbnb]);
    let sell_tx = TransactionRequest::default().to(router).input(sell_calldata.into());
    let sell_ret =
        provider.call(sell_tx).await.map_err(|e| format!("eth_call getAmountsOut(sell) that bai: {e}"))?;
    let sell_back =
        decode_last_amounts_out(&sell_ret).ok_or_else(|| "getAmountsOut(sell) tra ve du lieu qua ngan".to_string())?;

    Ok(RoundtripQuote { buy_out, sell_back, measured_at_block: current_block })
}

/// Kết quả đo tax roundtrip đã CACHE (điền thủ công/test cho tới khi có hàm
/// đo thật — xem doc-comment đầu file).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaxMeasurement {
    pub roundtrip_tax_bps: u32,
    pub measured_at_block: u64,
    /// Cụm `evm-validate-fixed-then-wire` (C1) — 2 chiều đo riêng bằng EVM
    /// thật (`sim_evm::measure_tax_evm`). `None` cho các bản ghi CŨ điền tay
    /// qua `POST /api/tax`/`tax_inject.jsonl` bằng đường
    /// `inject_from_buy_sell_bps` trước phiên này — KHÔNG bịa số.
    pub buy_bps: Option<u32>,
    pub sell_bps: Option<u32>,
    /// `true` khi EVM xác nhận mua được nhưng KHÔNG bán lại được (honeypot
    /// thật) — khác hẳn "tax cao" (vẫn bán được, chỉ lỗ).
    pub honeypot: bool,
}

impl TaxMeasurement {
    /// Bản ghi đo THỦ CÔNG/legacy (chỉ có `roundtrip_tax_bps`, không tách 2
    /// chiều) — dùng bởi test và đường inject cũ. `honeypot=false` vì đường
    /// này KHÔNG có căn cứ để kết luận honeypot (chỉ EVM mới kết luận được).
    pub fn manual(roundtrip_tax_bps: u32, measured_at_block: u64) -> Self {
        Self { roundtrip_tax_bps, measured_at_block, buy_bps: None, sell_bps: None, honeypot: false }
    }

    /// Cụm C1 — bản ghi đo bằng EVM thật (2 chiều tách riêng + cờ honeypot).
    pub fn from_evm(m: crate::sim_evm::EvmTaxMeasurement, measured_at_block: u64) -> Self {
        Self {
            roundtrip_tax_bps: combine_roundtrip_bps(m.buy_bps, m.sell_bps),
            measured_at_block,
            buy_bps: Some(m.buy_bps),
            sell_bps: Some(m.sell_bps),
            honeypot: m.honeypot,
        }
    }
}

/// Cụm `evm-validate-fixed-then-wire` (C2) — khoá cache là CẶP
/// `(token, quote)`, KHÔNG phải chỉ `token`: cùng 1 token có thể có pool với
/// WBNB và pool với USDT, tax fee-on-transfer đo qua 2 đường đó KHÁC NHAU
/// (khác pool, khác đường router, một số token miễn tax cho cặp nhất định).
/// Dùng chung 1 khoá cho cả 2 sẽ lẫn số đo — sai âm thầm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaxKey {
    pub token: Address,
    pub quote: Address,
}

#[derive(Debug, Default)]
pub struct TaxCache {
    entries: HashMap<TaxKey, TaxMeasurement>,
    /// C2 — mốc thời gian THỰC của mỗi lần đo, cho TTL theo giây
    /// (`tax_cache_ttl_sec`). Tách khỏi `TaxMeasurement` (struct đó `Copy` +
    /// được serialize ra web/API, `Instant` không serialize được).
    measured_at: HashMap<TaxKey, std::time::Instant>,
}

impl TaxCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ghi theo khoá CŨ (chỉ token) — quote mặc định WBNB, giữ cho mọi call
    /// site cũ (`POST /api/tax`, `tax_inject.jsonl`) không phải đổi.
    pub fn insert(&mut self, token: Address, measurement: TaxMeasurement) {
        self.insert_for_quote(token, crate::pool::wbnb(), measurement);
    }

    /// C2 — ghi theo cặp `(token, quote)` + đóng dấu thời gian thực cho TTL.
    pub fn insert_for_quote(&mut self, token: Address, quote: Address, measurement: TaxMeasurement) {
        let k = TaxKey { token, quote };
        self.entries.insert(k, measurement);
        self.measured_at.insert(k, std::time::Instant::now());
    }

    /// C2 — ngưỡng tươi/cũ THẬT SỰ dùng trong pipeline: TTL theo GIÂY
    /// (`tax_cache_ttl_sec`), không phải theo block. `tax_cache_blocks` vẫn
    /// đọc được từ config (không phá `config.toml` cũ của Chủ) nhưng KHÔNG
    /// còn quyết định gì — xem `docs/STATE.md`.
    pub fn get_fresh_ttl(&self, token: Address, quote: Address, ttl: std::time::Duration) -> Option<TaxMeasurement> {
        let k = TaxKey { token, quote };
        let m = self.entries.get(&k)?;
        let t = self.measured_at.get(&k)?;
        if t.elapsed() > ttl {
            None
        } else {
            Some(*m)
        }
    }

    /// `None` khi CHƯA đo HOẶC đã quá hạn `tax_cache_blocks` — caller PHẢI xử
    /// lý như chưa đo (`honeypot_or_tax` SKIP), đúng AGENTS.md.
    pub fn get_fresh(&self, token: Address, current_block: u64, tax_cache_blocks: u32) -> Option<TaxMeasurement> {
        let m = self.entries.get(&TaxKey { token, quote: crate::pool::wbnb() })?;
        let age = current_block.saturating_sub(m.measured_at_block);
        if age > tax_cache_blocks as u64 {
            None
        } else {
            Some(*m)
        }
    }

    /// Cụm tax-cache-inject — điểm GHI duy nhất từ `buy_bps`/`sell_bps` thô
    /// (`POST /api/tax` VÀ `state/tax_inject.jsonl` đều gọi hàm này, tránh 2
    /// nơi tính `combine_roundtrip_bps` khác nhau rồi lệch kết quả).
    pub fn inject_from_buy_sell_bps(&mut self, token: Address, buy_bps: u32, sell_bps: u32, measured_at_block: u64) {
        let roundtrip_tax_bps = combine_roundtrip_bps(buy_bps, sell_bps);
        self.insert(
            token,
            TaxMeasurement {
                roundtrip_tax_bps,
                measured_at_block,
                buy_bps: Some(buy_bps),
                sell_bps: Some(sell_bps),
                honeypot: false,
            },
        );
    }

    /// Toàn bộ cache hiện có — dùng cho web dashboard (bảng token/bps/block,
    /// xem `web.rs`), cùng khuôn `VictimBook::entries`.
    pub fn entries(&self) -> impl Iterator<Item = (&TaxKey, &TaxMeasurement)> {
        self.entries.iter()
    }

    /// Cụm `truth-victim-ok-and-memleak` (mục 3) — số entry đang giữ, cho
    /// `GET /api/mem`. Cache này chặn theo TTL khi ĐỌC (`get_fresh`) nhưng
    /// KHÔNG xoá entry hết hạn, nên số này chỉ tăng theo số `(token, quote)`
    /// KHÁC NHAU từng gặp — ở mode 2 nó bị chặn bởi `pairs.txt` (126 dòng),
    /// nên không phải nghi phạm rò rỉ; vẫn báo cáo để không phải đoán.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Kết hợp `buy_bps`/`sell_bps` (mỗi chiều đo riêng, đơn vị basis point)
/// thành `roundtrip_tax_bps` đúng công thức tổn thất kép
/// `1 - (1-buy)(1-sell) = buy + sell - buy*sell` (KHÔNG cộng đơn giản —
/// cộng đơn giản đếm trùng phần giao giữa 2 lần tax). Toàn bộ tính bằng
/// `u128` nguyên (không float, khớp AGENTS.md "U256 only" cho math tiền —
/// bps ở đây không phải U256 vì không phải số tiền wei, nhưng vẫn giữ
/// nguyên tắc số nguyên chính xác tuyệt đối, không sai số float).
/// `saturating_sub` tự vệ input rác (`buy_bps`/`sell_bps` > 10_000, tức
/// > 100% — dữ liệu chủ tự điền tay có thể gõ sai) để không panic do tràn
/// số âm trên kiểu không dấu.
pub fn combine_roundtrip_bps(buy_bps: u32, sell_bps: u32) -> u32 {
    let buy = buy_bps as u128;
    let sell = sell_bps as u128;
    let cross = (buy * sell) / 10_000u128;
    let total = (buy + sell).saturating_sub(cross);
    total.min(u32::MAX as u128) as u32
}

/// Parse 1 dòng `state/tax_inject.jsonl`: `token,buy_bps,sell_bps` (CSV thô,
/// đúng khuôn `transport::parse_inject_line` — tên đuôi file `.jsonl` theo
/// đúng lệnh nhưng NỘI DUNG mỗi dòng là CSV, không phải JSON). `0,0` = chủ
/// đã đo tay xác nhận zero-tax (đúng nghĩa lệnh, KHÔNG phải giá trị mặc định
/// tự động — hàm này chỉ parse, không tự bịa `0,0` cho token không có dòng).
pub fn parse_tax_inject_line(line: &str) -> Result<(Address, u32, u32), String> {
    let line = line.trim();
    let parts: Vec<&str> = line.splitn(3, ',').collect();
    if parts.len() != 3 {
        return Err("dinh dang phai la token,buy_bps,sell_bps".to_string());
    }
    let token = Address::from_str(parts[0].trim()).map_err(|e| format!("token khong hop le: {e}"))?;
    let buy_bps: u32 = parts[1].trim().parse().map_err(|_| "buy_bps khong hop le (can so nguyen khong am)".to_string())?;
    let sell_bps: u32 = parts[2].trim().parse().map_err(|_| "sell_bps khong hop le (can so nguyen khong am)".to_string())?;
    Ok((token, buy_bps, sell_bps))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn addr(hex: &str) -> Address {
        Address::from_str(hex).unwrap()
    }

    #[test]
    fn cache_miss_when_never_measured() {
        let cache = TaxCache::new();
        assert!(cache.get_fresh(addr("0x1111111111111111111111111111111111111111"), 100, 30).is_none());
    }

    #[test]
    fn cache_hit_within_window_then_stale_after_tax_cache_blocks() {
        let mut cache = TaxCache::new();
        let token = addr("0x2222222222222222222222222222222222222222");
        cache.insert(token, TaxMeasurement::manual(0, 1000));
        assert!(cache.get_fresh(token, 1010, 30).is_some()); // 10 block trong 30
        assert!(cache.get_fresh(token, 1030, 30).is_some()); // dung bien 30
        assert!(cache.get_fresh(token, 1031, 30).is_none()); // 31 > 30 -> het han
    }

    #[test]
    fn decode_last_amounts_out_reads_final_element() {
        let mut ret = vec![0u8; 0];
        ret.extend_from_slice(&U256::from(0x20u64).to_be_bytes::<32>()); // offset
        ret.extend_from_slice(&U256::from(2u64).to_be_bytes::<32>()); // length
        ret.extend_from_slice(&U256::from(111u64).to_be_bytes::<32>());
        ret.extend_from_slice(&U256::from(222u64).to_be_bytes::<32>());
        assert_eq!(decode_last_amounts_out(&ret), Some(U256::from(222u64)));
    }

    #[test]
    fn decode_last_amounts_out_too_short_is_none() {
        assert_eq!(decode_last_amounts_out(&[0u8; 40]), None);
    }

    #[test]
    fn build_calldata_has_correct_selector_and_length() {
        let wbnb = addr("0x333333333333333333333333333333333333cccc");
        let token = addr("0x444444444444444444444444444444444444dddd");
        let calldata = build_get_amounts_out_calldata(U256::from(1_000_000u64), &[wbnb, token]);
        assert_eq!(&calldata[0..4], SEL_GET_AMOUNTS_OUT.as_slice());
        assert_eq!(calldata.len(), 4 + 32 * 3 + 32 * 2);
        // head: amountIn(32) + offset(32) + length(32) = 96 byte, roi moi
        // phan tu path la 1 word (address right-aligned).
        assert_eq!(&calldata[4 + 96 + 12..4 + 128], wbnb.as_slice());
        assert_eq!(&calldata[4 + 128 + 12..4 + 160], token.as_slice());
    }

    #[test]
    fn loss_bps_exact_integer_math() {
        let quote =
            RoundtripQuote { buy_out: U256::from(990u64), sell_back: U256::from(950u64), measured_at_block: 1 };
        let probe_in = U256::from(1000u64);
        // diff=50, bps = 50*10000/1000 = 500 (5.00%)
        assert_eq!(quote.loss_bps(probe_in), Some(500));
    }

    #[test]
    fn loss_bps_zero_probe_is_none() {
        let quote = RoundtripQuote { buy_out: U256::ZERO, sell_back: U256::ZERO, measured_at_block: 1 };
        assert_eq!(quote.loss_bps(U256::ZERO), None);
    }

    #[test]
    fn combine_roundtrip_bps_zero_zero_is_zero() {
        assert_eq!(combine_roundtrip_bps(0, 0), 0);
    }

    /// buy=1% (100 bps), sell=1% (100 bps) -> khong phai cong don gian 200,
    /// ma 1-(0.99*0.99) = 1.99% = 199 bps (tru phan giao 100*100/10000=1).
    #[test]
    fn combine_roundtrip_bps_accounts_for_double_counted_cross_term() {
        assert_eq!(combine_roundtrip_bps(100, 100), 199);
    }

    #[test]
    fn combine_roundtrip_bps_garbage_input_over_100_percent_no_panic() {
        // buy=sell=50_000 bps (500%, du lieu tay go sai) -> khong duoc panic
        // do tran u32 khi tru; cross (50_000*50_000/10_000=250_000) > buy+sell
        // (100_000) nen phai saturating_sub ve 0, khong am.
        assert_eq!(combine_roundtrip_bps(50_000, 50_000), 0);
    }

    #[test]
    fn parse_tax_inject_line_valid() {
        let (token, buy, sell) =
            parse_tax_inject_line("0xcccccccccccccccccccccccccccccccccccccccc,0,0").expect("dong hop le phai parse duoc");
        assert_eq!(token, addr("0xcccccccccccccccccccccccccccccccccccccccc"));
        assert_eq!(buy, 0);
        assert_eq!(sell, 0);
    }

    #[test]
    fn parse_tax_inject_line_rejects_wrong_field_count() {
        let err = parse_tax_inject_line("0xcccccccccccccccccccccccccccccccccccccccc,0").unwrap_err();
        assert!(err.contains("token,buy_bps,sell_bps"));
    }

    #[test]
    fn parse_tax_inject_line_rejects_bad_token() {
        let err = parse_tax_inject_line("not_an_address,0,0").unwrap_err();
        assert!(err.contains("token khong hop le"));
    }

    #[test]
    fn parse_tax_inject_line_rejects_bad_bps() {
        let err = parse_tax_inject_line("0xcccccccccccccccccccccccccccccccccccccccc,abc,0").unwrap_err();
        assert!(err.contains("buy_bps khong hop le"));
    }

    /// Điểm ghi duy nhất `inject_from_buy_sell_bps` — inject `0,0` (đã đo tay
    /// zero-tax, đúng lệnh "0,0 = zero-tax đã đo tay") phải cho ra cache
    /// TƯƠI ngay tại block vừa inject.
    #[test]
    fn inject_from_buy_sell_bps_zero_zero_is_fresh_immediately() {
        let mut cache = TaxCache::new();
        let token = addr("0xdddddddddddddddddddddddddddddddddddddddd");
        cache.inject_from_buy_sell_bps(token, 0, 0, 5000);
        let m = cache.get_fresh(token, 5000, 30).expect("vua inject phai tuoi ngay");
        assert_eq!(m.roundtrip_tax_bps, 0);
        assert_eq!(m.measured_at_block, 5000);
    }

    #[test]
    fn entries_lists_all_injected_tokens() {
        let mut cache = TaxCache::new();
        let t1 = addr("0x1111111111111111111111111111111111111111");
        let t2 = addr("0x2222222222222222222222222222222222222222");
        cache.inject_from_buy_sell_bps(t1, 0, 0, 100);
        cache.inject_from_buy_sell_bps(t2, 50, 50, 100);
        let seen: std::collections::HashSet<Address> = cache.entries().map(|(k, _)| k.token).collect();
        assert_eq!(seen.len(), 2);
        assert!(seen.contains(&t1));
        assert!(seen.contains(&t2));
    }
}

//! Cụm `planB-backrun-opportunity` (mục 2) — NGUỒN VAY FLASH cho chiến lược
//! backrun-arb nguyên tử.
//!
//! Module này CHỈ ĐỌC chain (`eth_call` `balanceOf`/fee getter). Không ký,
//! không gửi, không dựng calldata gửi thật — hợp đồng `ArbExecutor` là cụm
//! sau (`docs/CONTRACT_DESIGN.md` mục "ArbExecutor", chỉ thiết kế ở cụm này).
//!
//! # Vì sao 4 nguồn, và vì sao thứ tự này
//!
//! Thứ tự ưu tiên do Chủ chốt (khối lệnh `planB-backrun-opportunity`), nhưng
//! module KHÔNG hardcode "luôn chọn Infinity": `choose_flash_source` đọc
//! SNAPSHOT THẬT tại block hiện tại rồi chọn nguồn RẺ NHẤT còn ĐỦ SÂU cho số
//! tiền cần vay. Nguồn phí bằng nhau thì mới xét tới thứ tự ưu tiên.
//!
//! 1. **PancakeSwap Infinity Vault** (`0x238a…5e6c4`, đã pin `DEX_REGISTRY.md`)
//!    — phí **0**. Cơ chế (đọc trực tiếp `pancakeswap/infinity-core` phiên
//!    này, xem `docs/STATE.md`): `Vault.lock(data)` → gọi lại
//!    `ILockCallback(msg.sender).lockAcquired(data)` → trong callback gọi
//!    `take(currency, to, amount)` (ghi delta ÂM cho locker rồi
//!    `currency.transfer`) → làm arb → `sync(currency)` + trả token về Vault
//!    → `settle()` (ghi delta DƯƠNG bằng đúng phần chênh `balanceOfSelf`
//!    trước/sau). Hết `lock`, Vault revert nếu `getUnsettledDeltasCount() != 0`.
//!    KHÔNG có bất kỳ dòng code nào trừ phí trong `take`/`_settle` — xem trích
//!    dẫn nguyên văn trong `docs/STATE.md`.
//!
//!    **Trần vay = `IERC20(token).balanceOf(VAULT)` THẬT, KHÔNG phải
//!    `reservesOfApp`.** `take()` gọi `currency.transfer(to, amount)` — một
//!    transfer ERC20 thường từ số dư THẬT của Vault; `reservesOfApp` chỉ là
//!    sổ kế toán theo từng app đã đăng ký (dùng cho `AppDeficit`), và một
//!    locker thường KHÔNG chạm vào sổ đó (chỉ `accountAppBalanceDelta` mới
//!    chạm). Đo thật 2026-09-16 tại block 122 209 916: `balanceOf` = 177,32
//!    WBNB trong khi `reservesOfApp(CLPoolManager)+reservesOfApp(BinPoolManager)`
//!    = 132,89 WBNB — chênh 44 WBNB, nên lấy nhầm `reservesOfApp` sẽ tự cắt
//!    25 % chiều sâu mà không có lý do.
//!
//! 2. **Balancer V2 Vault** (`0xBA12…F2C8`) — phí đọc từ
//!    `ProtocolFeesCollector.getFlashLoanFeePercentage()` (đo thật phiên này:
//!    **0**). NHƯNG xem `docs/STATE.md` mục rủi ro: trên **BSC** vault này
//!    gần như RỖNG (đo thật cùng block: 0,000435 WBNB / 5,0e-13 USDT) nên
//!    thực tế KHÔNG dùng được, độc lập với chuyện wind-down.
//!
//! 3. **Pancake V2 flash swap** (`pancakeCall`) — không có contract riêng để
//!    đọc: chiều sâu = reserve của CHÍNH pool đang arb, nên `FlashSnapshot`
//!    không đọc gì cho nguồn này; caller truyền `v2_pool_depth` vào
//!    `choose_flash_source`. Phí: trả LẠI cùng token thì phải hoàn
//!    `ceil(amount * 10000 / 9975)` (ràng buộc `k` của pool, phí 0,25 % tính
//!    trên phần vào) — xem `flash_fee_wei`.
//!
//! 4. **Aave V3 BSC** (`Pool` `0x6807…e0cB`) — phí đọc từ
//!    `FLASHLOAN_PREMIUM_TOTAL()` (đo thật phiên này: `5` = 0,05 %). Chiều
//!    sâu = số token nằm ở aToken tương ứng (`getReserveData(asset).aTokenAddress`
//!    rồi `balanceOf(aToken)`).

use alloy::primitives::{keccak256, Address, U256};
use alloy::providers::Provider;
use alloy::rpc::types::eth::TransactionRequest;
use std::str::FromStr;

/// Vault Infinity — ĐÃ pin ở `DEX_REGISTRY.md` mục V4/Infinity (getCode 8347
/// byte). Khai lại ở đây để `flash.rs` không phụ thuộc thứ tự khai trong
/// `venues.rs`; test `flash_addresses_match_registry` đối chiếu.
pub const INFINITY_VAULT_ADDRESS: &str = "0x238a358808379702088667322f80aC48bAd5e6c4";
/// Balancer V2 Vault trên BSC — pin phiên `planB-backrun-opportunity`.
pub const BALANCER_V2_VAULT_ADDRESS: &str = "0xBA12222222228d8Ba445958a75a0704d566BF2C8";
/// Balancer V2 `ProtocolFeesCollector` — nơi đọc `getFlashLoanFeePercentage()`.
pub const BALANCER_PROTOCOL_FEES_COLLECTOR_ADDRESS: &str = "0xce88686553686DA562CE7Cea497CE749DA109f9F";
/// Aave V3 `Pool` trên BSC — pin phiên `planB-backrun-opportunity`.
pub const AAVE_V3_POOL_ADDRESS: &str = "0x6807dc923806fE8Fd134338EABCA509979a7e0cB";

/// Phí flash swap Pancake V2 tính theo bps DANH NGHĨA (0,25 %). Số thật dùng
/// để tính tiền là `flash_fee_wei` (làm tròn LÊN theo đúng ràng buộc `k`),
/// hằng số này chỉ để so sánh/xếp hạng nguồn.
pub const PANCAKE_V2_FLASH_FEE_BPS: u32 = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FlashSource {
    InfinityVault,
    BalancerV2,
    PancakeV2FlashSwap,
    AaveV3,
}

impl FlashSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            FlashSource::InfinityVault => "infinity_vault",
            FlashSource::BalancerV2 => "balancer_v2",
            FlashSource::PancakeV2FlashSwap => "pancake_v2_flash_swap",
            FlashSource::AaveV3 => "aave_v3",
        }
    }

    /// Thứ tự ưu tiên do Chủ chốt — CHỈ dùng để phá hoà khi phí BẰNG NHAU,
    /// không bao giờ thắng một nguồn rẻ hơn.
    pub fn priority(&self) -> u8 {
        match self {
            FlashSource::InfinityVault => 0,
            FlashSource::BalancerV2 => 1,
            FlashSource::PancakeV2FlashSwap => 2,
            FlashSource::AaveV3 => 3,
        }
    }

    pub fn all() -> [FlashSource; 4] {
        [FlashSource::InfinityVault, FlashSource::BalancerV2, FlashSource::PancakeV2FlashSwap, FlashSource::AaveV3]
    }
}

/// Phí phải trả THÊM khi vay `amount` từ `source` với `fee_bps` đọc được.
///
/// - `PancakeV2FlashSwap`: hoàn lại cùng token → pool đòi
///   `ceil(amount * 10000 / 9975)`, nên phí = số đó trừ `amount` (LỚN HƠN
///   `amount * 25 / 10000` một chút — đúng bản chất "phí tính trên phần vào").
/// - Nguồn khác: `ceil(amount * fee_bps / 10000)`.
///
/// Luôn làm tròn LÊN (bất lợi cho ta) — không bao giờ báo lãi nhờ làm tròn.
pub fn flash_fee_wei(source: FlashSource, amount: U256, fee_bps: u32) -> Option<U256> {
    if amount.is_zero() {
        return Some(U256::ZERO);
    }
    match source {
        FlashSource::PancakeV2FlashSwap => {
            let num = amount.checked_mul(U256::from(10_000u64))?;
            let den = U256::from(9_975u64);
            let repay = num.div_ceil(den);
            repay.checked_sub(amount)
        }
        _ => {
            if fee_bps == 0 {
                return Some(U256::ZERO);
            }
            let num = amount.checked_mul(U256::from(fee_bps))?;
            Some(num.div_ceil(U256::from(10_000u64)))
        }
    }
}

/// Trạng thái 1 nguồn flash tại 1 block cụ thể.
#[derive(Debug, Clone)]
pub struct FlashSourceState {
    pub source: FlashSource,
    /// `None` = chưa đọc được phí (lỗi RPC) → nguồn KHÔNG được chọn (không
    /// đoán phí bằng 0).
    pub fee_bps: Option<u32>,
    /// `(token, số token nguồn này đang thực sự giữ)`. Token không có trong
    /// danh sách = chưa đo → không chọn.
    pub available: Vec<(Address, U256)>,
    pub error: Option<String>,
}

impl FlashSourceState {
    pub fn available_for(&self, token: Address) -> Option<U256> {
        self.available.iter().find(|(t, _)| *t == token).map(|(_, v)| *v)
    }
}

/// Ảnh chụp mọi nguồn flash tại 1 block. `flash_source_task` (main.rs) ghi
/// lại mỗi `flash_source_interval_sec`; `choose_flash_source` đọc từ đây,
/// KHÔNG tự gọi RPC (đường nóng không được chặn vì 4 lời gọi `eth_call`).
#[derive(Debug, Clone, Default)]
pub struct FlashSnapshot {
    pub block: u64,
    pub measured_at_unix: u64,
    pub states: Vec<FlashSourceState>,
}

/// Nguồn đã chọn cho 1 lần vay cụ thể.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlashPick {
    pub source: FlashSource,
    pub fee_bps: u32,
    pub fee_wei: U256,
    /// Chiều sâu đo được của nguồn đã chọn (để log/kiểm lại, không dùng tính tiền).
    pub available: U256,
}

impl FlashSnapshot {
    pub fn state(&self, source: FlashSource) -> Option<&FlashSourceState> {
        self.states.iter().find(|s| s.source == source)
    }

    /// Nguồn RẺ NHẤT còn ĐỦ SÂU để vay `amount` của `token` tại block đã chụp.
    ///
    /// `v2_pool_depth` = reserve quote của chính pool sẽ arb (chiều sâu khả
    /// dụng của `pancakeCall`) — `None` nghĩa caller không cho phép dùng
    /// nguồn đó cho route này.
    ///
    /// Trả `None` khi KHÔNG nguồn nào đủ sâu: caller phải dịch thành skip
    /// `arb_no_flash_source`, KHÔNG được âm thầm coi phí = 0.
    pub fn choose_flash_source(&self, token: Address, amount: U256, v2_pool_depth: Option<U256>) -> Option<FlashPick> {
        if amount.is_zero() {
            return None;
        }
        let mut best: Option<FlashPick> = None;
        for source in FlashSource::all() {
            let (fee_bps, available) = match source {
                FlashSource::PancakeV2FlashSwap => match v2_pool_depth {
                    Some(d) => (PANCAKE_V2_FLASH_FEE_BPS, d),
                    None => continue,
                },
                _ => {
                    let st = match self.state(source) {
                        Some(s) => s,
                        None => continue,
                    };
                    let fee = match st.fee_bps {
                        Some(f) => f,
                        None => continue,
                    };
                    match st.available_for(token) {
                        Some(a) => (fee, a),
                        None => continue,
                    }
                }
            };
            if available < amount {
                continue;
            }
            let fee_wei = match flash_fee_wei(source, amount, fee_bps) {
                Some(f) => f,
                None => continue,
            };
            let pick = FlashPick { source, fee_bps, fee_wei, available };
            let better = match &best {
                None => true,
                Some(b) => (pick.fee_wei, pick.source.priority()) < (b.fee_wei, b.source.priority()),
            };
            if better {
                best = Some(pick);
            }
        }
        best
    }
}

fn selector(sig: &str) -> [u8; 4] {
    let h = keccak256(sig.as_bytes());
    [h[0], h[1], h[2], h[3]]
}

fn addr(s: &str) -> Address {
    Address::from_str(s).expect("dia chi pin hop le")
}

pub fn infinity_vault() -> Address {
    addr(INFINITY_VAULT_ADDRESS)
}
pub fn balancer_vault() -> Address {
    addr(BALANCER_V2_VAULT_ADDRESS)
}
pub fn balancer_fees_collector() -> Address {
    addr(BALANCER_PROTOCOL_FEES_COLLECTOR_ADDRESS)
}
pub fn aave_v3_pool() -> Address {
    addr(AAVE_V3_POOL_ADDRESS)
}

fn build_balance_of_calldata(owner: Address) -> Vec<u8> {
    let mut out = Vec::with_capacity(36);
    out.extend_from_slice(&selector("balanceOf(address)"));
    out.extend_from_slice(&[0u8; 12]);
    out.extend_from_slice(owner.as_slice());
    out
}

fn decode_u256_return(ret: &[u8]) -> Option<U256> {
    if ret.len() < 32 {
        return None;
    }
    Some(U256::from_be_slice(&ret[0..32]))
}

async fn call_u256(provider: &dyn Provider, to: Address, calldata: Vec<u8>) -> Result<U256, String> {
    let tx = TransactionRequest::default().to(to).input(calldata.into());
    let ret = provider.call(tx).await.map_err(|e| format!("eth_call {to:#x} that bai: {e}"))?;
    decode_u256_return(&ret).ok_or_else(|| format!("eth_call {to:#x} tra ve du lieu qua ngan"))
}

/// `IERC20(token).balanceOf(holder)`.
pub async fn erc20_balance_of(provider: &dyn Provider, token: Address, holder: Address) -> Result<U256, String> {
    call_u256(provider, token, build_balance_of_calldata(holder)).await
}

/// Đọc chiều sâu Infinity Vault cho danh sách token. Xem doc đầu file để
/// biết vì sao dùng `balanceOf` chứ không phải `reservesOfApp`.
pub async fn read_infinity_state(provider: &dyn Provider, tokens: &[Address]) -> FlashSourceState {
    let mut available = Vec::new();
    let mut error = None;
    for &t in tokens {
        match erc20_balance_of(provider, t, infinity_vault()).await {
            Ok(v) => available.push((t, v)),
            Err(e) => {
                if error.is_none() {
                    error = Some(e);
                }
            }
        }
    }
    FlashSourceState { source: FlashSource::InfinityVault, fee_bps: Some(0), available, error }
}

/// Balancer V2: phí đọc THẬT từ `ProtocolFeesCollector.getFlashLoanFeePercentage()`
/// (đơn vị 1e18 = 100 %) rồi quy về bps. Đọc lỗi → `fee_bps = None` (nguồn bị
/// loại, không đoán 0).
pub async fn read_balancer_state(provider: &dyn Provider, tokens: &[Address]) -> FlashSourceState {
    let mut error = None;
    let fee_bps = match call_u256(provider, balancer_fees_collector(), selector("getFlashLoanFeePercentage()").to_vec()).await {
        Ok(v) => Some(fee_percent_1e18_to_bps(v)),
        Err(e) => {
            error = Some(e);
            None
        }
    };
    let mut available = Vec::new();
    for &t in tokens {
        match erc20_balance_of(provider, t, balancer_vault()).await {
            Ok(v) => available.push((t, v)),
            Err(e) => {
                if error.is_none() {
                    error = Some(e);
                }
            }
        }
    }
    FlashSourceState { source: FlashSource::BalancerV2, fee_bps, available, error }
}

/// `getFlashLoanFeePercentage()` trả tỉ lệ theo thang 1e18 (1e18 = 100 %).
/// Quy về bps làm tròn LÊN (phí cao hơn = bất lợi cho ta, không bao giờ báo
/// rẻ hơn thật).
pub fn fee_percent_1e18_to_bps(v: U256) -> u32 {
    if v.is_zero() {
        return 0;
    }
    let scaled = v.saturating_mul(U256::from(10_000u64));
    let bps = scaled.div_ceil(U256::from(1_000_000_000_000_000_000u128));
    u32::try_from(bps).unwrap_or(u32::MAX)
}

/// Aave V3: `FLASHLOAN_PREMIUM_TOTAL()` (đã là bps) + chiều sâu = số token
/// nằm ở aToken (`getReserveData(asset)` → `aTokenAddress`).
pub async fn read_aave_state(provider: &dyn Provider, tokens: &[Address]) -> FlashSourceState {
    let mut error = None;
    let fee_bps = match call_u256(provider, aave_v3_pool(), selector("FLASHLOAN_PREMIUM_TOTAL()").to_vec()).await {
        Ok(v) => u32::try_from(v).ok(),
        Err(e) => {
            error = Some(e);
            None
        }
    };
    let mut available = Vec::new();
    for &t in tokens {
        match aave_atoken_liquidity(provider, t).await {
            Ok(v) => available.push((t, v)),
            Err(e) => {
                if error.is_none() {
                    error = Some(e);
                }
            }
        }
    }
    FlashSourceState { source: FlashSource::AaveV3, fee_bps, available, error }
}

/// `Pool.getReserveData(address)` trả struct `ReserveData`. `aTokenAddress`
/// nằm ở slot thứ 8 của phần trả về (0-index) theo layout Aave V3
/// `DataTypes.ReserveData` — CHỈ đọc, sai layout thì `balanceOf` sẽ lỗi/0 và
/// nguồn bị loại, không bao giờ làm ta báo lãi sai (chiều sâu chỉ dùng để
/// CHẶN, không cộng vào lợi nhuận).
async fn aave_atoken_liquidity(provider: &dyn Provider, asset: Address) -> Result<U256, String> {
    let mut calldata = Vec::with_capacity(36);
    calldata.extend_from_slice(&selector("getReserveData(address)"));
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(asset.as_slice());
    let tx = TransactionRequest::default().to(aave_v3_pool()).input(calldata.into());
    let ret = provider.call(tx).await.map_err(|e| format!("eth_call aave getReserveData that bai: {e}"))?;
    let off = 8 * 32;
    if ret.len() < off + 32 {
        return Err("aave getReserveData tra ve du lieu qua ngan".to_string());
    }
    let atoken = Address::from_slice(&ret[off + 12..off + 32]);
    if atoken == Address::ZERO {
        return Err("aave chua liet ke asset nay (aTokenAddress = 0)".to_string());
    }
    erc20_balance_of(provider, asset, atoken).await
}

/// Chụp cả 3 nguồn đọc-được (Infinity / Balancer / Aave). Pancake V2 flash
/// swap KHÔNG có gì để chụp — chiều sâu của nó là reserve pool đang arb, do
/// caller truyền vào `choose_flash_source`.
pub async fn read_flash_snapshot(provider: &dyn Provider, block: u64, tokens: &[Address]) -> FlashSnapshot {
    let infinity = read_infinity_state(provider, tokens).await;
    let balancer = read_balancer_state(provider, tokens).await;
    let aave = read_aave_state(provider, tokens).await;
    FlashSnapshot {
        block,
        measured_at_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        states: vec![infinity, balancer, aave],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::venues::{USDT_ADDRESS, WBNB_ADDRESS};

    fn wbnb() -> Address {
        Address::from_str(WBNB_ADDRESS).unwrap()
    }
    fn usdt() -> Address {
        Address::from_str(USDT_ADDRESS).unwrap()
    }

    #[test]
    fn flash_addresses_are_20_byte_hex_and_checksum_parse() {
        for s in [
            INFINITY_VAULT_ADDRESS,
            BALANCER_V2_VAULT_ADDRESS,
            BALANCER_PROTOCOL_FEES_COLLECTOR_ADDRESS,
            AAVE_V3_POOL_ADDRESS,
        ] {
            assert!(Address::from_str(s).is_ok(), "dia chi pin phai parse duoc: {s}");
        }
        // Vault Infinity phai TRUNG dung dia chi da pin trong DEX_REGISTRY.md
        // muc V4/Infinity - neu ai do sua 1 trong 2 cho, test nay do.
        assert_eq!(
            INFINITY_VAULT_ADDRESS.to_lowercase(),
            "0x238a358808379702088667322f80ac48bad5e6c4",
            "Vault Infinity phai khop DEX_REGISTRY.md"
        );
    }

    /// Phí flash swap V2 phải LỚN HƠN `amount*25/10000` — đây chính là chỗ
    /// dễ tính thiếu nhất (0,25 % "của phần vào" chứ không phải của phần vay).
    #[test]
    fn pancake_v2_flash_fee_lon_hon_25bps_phang() {
        let amount = U256::from(1_000_000_000_000_000_000u128); // 1 BNB
        let fee = flash_fee_wei(FlashSource::PancakeV2FlashSwap, amount, PANCAKE_V2_FLASH_FEE_BPS).unwrap();
        let naive = amount * U256::from(25u64) / U256::from(10_000u64);
        assert!(fee > naive, "phi V2 flash swap phai lon hon 25bps phang (fee={fee}, naive={naive})");
        // ceil(1e18 * 10000 / 9975) - 1e18: 1e22/9975 = 1002506265664160401.0025..
        // -> ceil = 1002506265664160402 -> phi = 2506265664160402
        assert_eq!(fee, U256::from(2_506_265_664_160_402u128));
    }

    #[test]
    fn flash_fee_zero_bps_la_zero_va_lam_tron_len() {
        let amount = U256::from(1_000_000u64);
        assert_eq!(flash_fee_wei(FlashSource::InfinityVault, amount, 0).unwrap(), U256::ZERO);
        // 1 wei * 5 bps = 0.0005 wei -> lam tron LEN = 1 wei (khong bao gio 0)
        assert_eq!(flash_fee_wei(FlashSource::AaveV3, U256::from(1u64), 5).unwrap(), U256::from(1u64));
    }

    #[test]
    fn fee_percent_1e18_doi_sang_bps() {
        assert_eq!(fee_percent_1e18_to_bps(U256::ZERO), 0);
        // 0.01e18 = 1% = 100 bps
        assert_eq!(fee_percent_1e18_to_bps(U256::from(10_000_000_000_000_000u128)), 100);
        // 1 wei (cuc nho) -> lam tron LEN thanh 1 bps, khong phai 0
        assert_eq!(fee_percent_1e18_to_bps(U256::from(1u64)), 1);
    }

    fn snapshot_fixture() -> FlashSnapshot {
        FlashSnapshot {
            block: 1,
            measured_at_unix: 0,
            states: vec![
                FlashSourceState {
                    source: FlashSource::InfinityVault,
                    fee_bps: Some(0),
                    available: vec![(wbnb(), U256::from(100u64)), (usdt(), U256::from(10u64))],
                    error: None,
                },
                FlashSourceState {
                    source: FlashSource::BalancerV2,
                    fee_bps: Some(0),
                    available: vec![(wbnb(), U256::from(1u64))],
                    error: None,
                },
                FlashSourceState {
                    source: FlashSource::AaveV3,
                    fee_bps: Some(5),
                    available: vec![(wbnb(), U256::from(1_000_000u64)), (usdt(), U256::from(1_000_000u64))],
                    error: None,
                },
            ],
        }
    }

    #[test]
    fn chon_nguon_re_nhat_con_du_sau() {
        let s = snapshot_fixture();
        // 50 <= 100 -> Infinity (phi 0) thang
        let p = s.choose_flash_source(wbnb(), U256::from(50u64), None).unwrap();
        assert_eq!(p.source, FlashSource::InfinityVault);
        assert_eq!(p.fee_wei, U256::ZERO);
    }

    #[test]
    fn nguon_re_nhung_can_thi_bi_bo_qua_khong_phai_bao_loi() {
        let s = snapshot_fixture();
        // 500 > 100 (Infinity) va > 1 (Balancer) -> phai roi xuong Aave
        let p = s.choose_flash_source(wbnb(), U256::from(500u64), None).unwrap();
        assert_eq!(p.source, FlashSource::AaveV3);
        assert_eq!(p.fee_bps, 5);
    }

    #[test]
    fn khong_nguon_nao_du_sau_thi_none_chu_khong_phi_0() {
        let s = snapshot_fixture();
        assert!(s.choose_flash_source(wbnb(), U256::from(2_000_000u64), None).is_none());
        // token chua do bao gio -> cung None (khong doan)
        let unknown = Address::from_str("0x000000000000000000000000000000000000dEaD").unwrap();
        assert!(s.choose_flash_source(unknown, U256::from(1u64), None).is_none());
    }

    #[test]
    fn v2_flash_swap_chi_duoc_chon_khi_caller_cho_phep_va_khong_bao_gio_thang_nguon_0_phi() {
        let s = snapshot_fixture();
        // Pool sau 1_000_000 nhung phi 25bps -> van thua Infinity (phi 0) o muc 50
        let p = s.choose_flash_source(wbnb(), U256::from(50u64), Some(U256::from(1_000_000u64))).unwrap();
        assert_eq!(p.source, FlashSource::InfinityVault);
        // O muc 500 thi Infinity can, V2 flash swap (25bps) thua Aave (5bps)
        let p = s.choose_flash_source(wbnb(), U256::from(500u64), Some(U256::from(1_000_000u64))).unwrap();
        assert_eq!(p.source, FlashSource::AaveV3, "5bps phai re hon 25bps");
    }

    #[test]
    fn fee_bps_doc_loi_thi_nguon_bi_loai_khong_coi_nhu_0() {
        let mut s = snapshot_fixture();
        s.states[0].fee_bps = None; // Infinity doc loi
        let p = s.choose_flash_source(wbnb(), U256::from(50u64), None).unwrap();
        assert_ne!(p.source, FlashSource::InfinityVault);
    }
}

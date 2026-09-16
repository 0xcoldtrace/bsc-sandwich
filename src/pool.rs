//! Cụm 2.3 — Resolve pool token/WBNB cho các family đã PIN (`DEX_REGISTRY.md`).
//!
//! - V2: `Factory.getPair(tokenA, tokenB)` — pair == address(0) -> `no_pool`.
//! - V3: `Factory.getPool(tokenA, tokenB, fee)` với đúng 4 fee tier PancakeSwap
//!   V3 (100/500/2500/10000 — xác nhận từ constructor `PancakeV3Factory.sol`,
//!   nguồn: `github.com/pancakeswap/pancake-v3-contracts`,
//!   `projects/v3-core/contracts/PancakeV3Factory.sol`, đọc trực tiếp phiên
//!   này, không đoán thêm tier khác Uniswap-mainnet 3000).
//! - V4/Infinity: Infinity không có factory kiểu `getPool(token0,token1)` —
//!   pool được định danh bằng `PoolId = keccak256(PoolKey)` với
//!   `PoolKey{currency0,currency1,hooks,poolManager,fee,parameters}` (nguồn:
//!   `github.com/pancakeswap/infinity-core`, `src/types/PoolKey.sol`, đọc
//!   trực tiếp phiên này). Vì không suy được `hooks`/`parameters` chỉ từ 2
//!   địa chỉ token, `resolve_infinity_pool` (phiên `v4-pool-resolve`) quét
//!   THẬT sự kiện `Initialize` (nguồn `ICLPoolManager.sol`/
//!   `IBinPoolManager.sol`, đọc trực tiếp) qua `eth_getLogs` trên
//!   `CLPoolManager`/`BinPoolManager` đã pin để tìm `PoolKey` thật của pool.
//!   Không tìm thấy log nào cho cặp `{token, WBNB}` -> vẫn `hooks_unread`,
//!   skip đúng pool đó, không tắt V2/V3 (theo CLAUDE.md mục "Decode được
//!   phép" / enum skip).
//!
//! `eth_call` dùng selector suy từ `keccak256(chữ_ký_hàm)` (không hardcode
//! hex mù), qua `alloy::providers::Provider::call`. `eth_getLogs` (Infinity)
//! dùng topic0 suy từ `keccak256(chữ_ký_event_đầy_đủ)` cùng cách, xem
//! `docs/STATE.md` mục `v4-pool-resolve` để có bằng chứng đối chiếu với dữ
//! liệu chain thật.

use alloy::primitives::{keccak256, Address, B256, U256};
use alloy::providers::Provider;
use alloy::rpc::types::eth::{Filter, Log, TransactionRequest};
use std::str::FromStr;
use std::sync::LazyLock;

use crate::venues::WBNB_ADDRESS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolSkipReason {
    NoPool,
    HooksUnread,
    ThinLiq,
}

impl PoolSkipReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            PoolSkipReason::NoPool => "no_pool",
            PoolSkipReason::HooksUnread => "hooks_unread",
            PoolSkipReason::ThinLiq => "thin_liq",
        }
    }
}

/// PancakeSwap V3 dùng đúng 4 fee tier này trên mọi pool (xác nhận nguồn ở
/// đầu file) — KHÔNG có tier 3000 (khác Uniswap V3 mainnet).
pub const V3_FEE_TIERS: [u32; 4] = [100, 500, 2500, 10000];

/// Family Infinity — `CLPoolManager` (concentrated liquidity) hoặc
/// `BinPoolManager` (liquidity book). Mỗi family có `event Initialize` khác
/// nhau ở phần dữ liệu cuối (CL: sqrtPriceX96+tick, Bin: activeId) nên
/// `topic0` (chữ ký event đầy đủ) khác nhau — xem `CL_INITIALIZE_TOPIC0`/
/// `BIN_INITIALIZE_TOPIC0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfinityPoolFamily {
    Cl,
    Bin,
}

impl InfinityPoolFamily {
    pub fn as_str(&self) -> &'static str {
        match self {
            InfinityPoolFamily::Cl => "cl",
            InfinityPoolFamily::Bin => "bin",
        }
    }
}

/// Một `PoolKey` Infinity thật, dựng lại từ `event Initialize` (log thật qua
/// `eth_getLogs`) — đủ field để gọi `CLQuoter`/`BinQuoter` ở cụm sim sau này
/// (KHÔNG làm ở phiên `v4-pool-resolve`, đó là cụm khác).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfinityPoolMatch {
    pub family: InfinityPoolFamily,
    pub pool_manager: Address,
    pub pool_id: B256,
    pub currency0: Address,
    pub currency1: Address,
    pub hooks: Address,
    pub fee: u32,
    pub parameters: [u8; 32],
}

/// `event Initialize(PoolId indexed id, Currency indexed currency0, Currency
/// indexed currency1, IHooks hooks, uint24 fee, bytes32 parameters, uint160
/// sqrtPriceX96, int24 tick)` — nguồn `ICLPoolManager.sol`
/// (`github.com/pancakeswap/infinity-core`, `src/pool-cl/interfaces/`, đọc
/// trực tiếp phiên `v4-pool-resolve`, 2026-09-15). `PoolId`/`Currency`/
/// `IHooks`/`IPoolManager` là custom value type bọc `bytes32`/`address` —
/// chữ ký ABI dùng kiểu gốc: `Initialize(bytes32,address,address,address,
/// uint24,bytes32,uint160,int24)`. Topic0 = `keccak256` chữ ký đó (test
/// `cl_initialize_topic0_matches_known_value` đối chiếu; đã verify RUNTIME
/// THẬT bằng `eth_getLogs` lọc đúng topic0 này trả về log thật trên
/// CLPoolManager pinned, xem `docs/STATE.md`).
const CL_INITIALIZE_EVENT_SIG: &str = "Initialize(bytes32,address,address,address,uint24,bytes32,uint160,int24)";

/// `event Initialize(PoolId indexed id, Currency indexed currency0, Currency
/// indexed currency1, IHooks hooks, uint24 fee, bytes32 parameters, uint24
/// activeId)` — nguồn `IBinPoolManager.sol` (cùng repo, `src/pool-bin/
/// interfaces/`, đọc trực tiếp phiên `v4-pool-resolve`). Chữ ký ABI:
/// `Initialize(bytes32,address,address,address,uint24,bytes32,uint24)`
/// (khác CL ở 2 field cuối: chỉ 1 `activeId` thay vì `sqrtPriceX96`+`tick`).
const BIN_INITIALIZE_EVENT_SIG: &str = "Initialize(bytes32,address,address,address,uint24,bytes32,uint24)";

static CL_INITIALIZE_TOPIC0: LazyLock<B256> = LazyLock::new(|| keccak256(CL_INITIALIZE_EVENT_SIG.as_bytes()));
static BIN_INITIALIZE_TOPIC0: LazyLock<B256> = LazyLock::new(|| keccak256(BIN_INITIALIZE_EVENT_SIG.as_bytes()));

/// Số block quét mỗi lần gọi `eth_getLogs` — giới hạn "hợp lý" theo CLAUDE.md
/// ("không quét toàn chain từ block 0"). Giá trị `5_000` chọn THẤP HƠN mọi
/// giới hạn thật đo được trong phiên `v4-pool-resolve` (2026-09-15): node
/// công khai `bsc-rpc.publicnode.com` cho quét free-tier tới `10_000` block
/// lùi từ tip (`20_000` bị chặn "Archive requests require a personal
/// token"), node `bsc.drpc.org` giới hạn cứng `10_000` block/lần
/// ("ranges over 10000 blocks are not supported on free plan"). `5_000` vừa
/// an toàn dưới cả 2 giới hạn vừa để lại biên nếu RPC khác chặt hơn — xem
/// `docs/STATE.md` mục `v4-pool-resolve` để có log thật của các phép thử
/// này.
const LOG_SCAN_CHUNK_BLOCKS: u64 = 5_000;

fn selector(sig: &str) -> [u8; 4] {
    let hash = keccak256(sig.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}

static SEL_GET_PAIR: LazyLock<[u8; 4]> = LazyLock::new(|| selector("getPair(address,address)"));
static SEL_GET_POOL: LazyLock<[u8; 4]> = LazyLock::new(|| selector("getPool(address,address,uint24)"));
static SEL_GET_RESERVES: LazyLock<[u8; 4]> = LazyLock::new(|| selector("getReserves()"));
static SEL_TOKEN0: LazyLock<[u8; 4]> = LazyLock::new(|| selector("token0()"));
static SEL_TOKEN1: LazyLock<[u8; 4]> = LazyLock::new(|| selector("token1()"));

pub fn wbnb() -> Address {
    Address::from_str(WBNB_ADDRESS).expect("WBNB_ADDRESS da pin phai la address hop le")
}

fn encode_address_word(a: Address, out: &mut Vec<u8>) {
    out.extend_from_slice(&[0u8; 12]);
    out.extend_from_slice(a.as_slice());
}

fn encode_uint24_word(v: u32, out: &mut Vec<u8>) {
    let mut word = [0u8; 32];
    let bytes = v.to_be_bytes();
    word[29..32].copy_from_slice(&bytes[1..4]);
    out.extend_from_slice(&word);
}

pub(crate) fn build_get_pair_calldata(token_a: Address, token_b: Address) -> Vec<u8> {
    let mut out = SEL_GET_PAIR.to_vec();
    encode_address_word(token_a, &mut out);
    encode_address_word(token_b, &mut out);
    out
}

fn build_get_pool_calldata(token_a: Address, token_b: Address, fee: u32) -> Vec<u8> {
    let mut out = SEL_GET_POOL.to_vec();
    encode_address_word(token_a, &mut out);
    encode_address_word(token_b, &mut out);
    encode_uint24_word(fee, &mut out);
    out
}

/// Giải mã return value 1 address (32 byte, right-aligned) của getPair/getPool.
pub(crate) fn decode_address_return(data: &[u8]) -> Option<Address> {
    if data.len() < 32 {
        return None;
    }
    Some(Address::from_slice(&data[data.len() - 32 + 12..data.len() - 32 + 32]))
}

pub fn is_zero_address(a: Address) -> bool {
    a == Address::ZERO
}

/// `Factory.getPair(token, quote)` qua `eth_call` — tổng quát hoá cho MỘT
/// quote asset bất kỳ (WBNB hoặc USDT, cụm `usdt-quote-asset` BAOCAO29). Trả
/// `no_pool` nếu pair là `address(0)` (chưa có pool). Lỗi RPC/transport trả
/// `Err(String)` riêng — không lẫn với `no_pool` (khác nhau về ý nghĩa: RPC
/// lỗi khác "chắc chắn không có pool").
pub async fn resolve_v2_pair_for_quote(
    provider: &dyn Provider,
    factory: Address,
    token: Address,
    quote: Address,
) -> Result<Result<Address, PoolSkipReason>, String> {
    let calldata = build_get_pair_calldata(token, quote);
    let tx = TransactionRequest::default().to(factory).input(calldata.into());
    let ret = provider.call(tx).await.map_err(|e| format!("eth_call getPair (quote={quote:#x}) that bai: {e}"))?;
    let pair = decode_address_return(&ret).ok_or_else(|| "getPair tra ve du lieu qua ngan".to_string())?;
    if is_zero_address(pair) {
        Ok(Err(PoolSkipReason::NoPool))
    } else {
        Ok(Ok(pair))
    }
}

/// `Factory.getPair(token, WBNB)` — GIỮ NGUYÊN chữ ký/hành vi cũ (mọi caller
/// hiện có không đổi), giờ chỉ là 1 lớp mỏng gọi `resolve_v2_pair_for_quote`
/// với `quote=wbnb()` (cụm `usdt-quote-asset`, BAOCAO29 — refactor KHÔNG đổi
/// kết quả, xem test `resolve_v2_pair_matches_resolve_v2_pair_for_quote_wbnb`
/// gián tiếp qua đối chiếu calldata sinh ra).
pub async fn resolve_v2_pair(
    provider: &dyn Provider,
    factory: Address,
    token: Address,
) -> Result<Result<Address, PoolSkipReason>, String> {
    resolve_v2_pair_for_quote(provider, factory, token, wbnb()).await
}

/// `Factory.getPool(token, WBNB, fee)` cho từng fee tier PIN
/// (`V3_FEE_TIERS`), trả pool đầu tiên khác `address(0)`. Không có tier nào
/// có pool -> `no_pool`.
pub async fn resolve_v3_pool(
    provider: &dyn Provider,
    factory: Address,
    token: Address,
) -> Result<Result<(Address, u32), PoolSkipReason>, String> {
    for fee in V3_FEE_TIERS {
        let calldata = build_get_pool_calldata(token, wbnb(), fee);
        let tx = TransactionRequest::default().to(factory).input(calldata.into());
        let ret = provider.call(tx).await.map_err(|e| format!("eth_call getPool(fee={fee}) that bai: {e}"))?;
        let pool = decode_address_return(&ret).ok_or_else(|| "getPool tra ve du lieu qua ngan".to_string())?;
        if !is_zero_address(pool) {
            return Ok(Ok((pool, fee)));
        }
    }
    Ok(Err(PoolSkipReason::NoPool))
}

/// `PancakeV2Pair.getReserves()` — trả `(reserveWBNB, reserveToken)` đã sắp
/// đúng chiều. V2 pair sort `token0/token1` theo address tăng dần khi tạo
/// pair (CREATE2 factory), không cố định WBNB ở vị trí nào -> gọi `token0()`
/// thật để biết chiều, không đoán theo so sánh address off-chain (đủ đúng
/// nhưng gọi on-chain cho chắc, cùng round-trip RPC với `getReserves`). Cần
/// cho `3.1` (math sandwich cần reserve thật, không chỉ địa chỉ pool).
pub async fn get_reserves_vs_wbnb(provider: &dyn Provider, pair: Address) -> Result<(U256, U256), String> {
    get_reserves_vs_quote(provider, pair, wbnb()).await
}

/// Cụm `usdt-quote-asset` (BAOCAO29) — tổng quát hoá `get_reserves_vs_wbnb`
/// cho MỘT quote asset bất kỳ (WBNB hoặc USDT): trả `(reserve_quote,
/// reserve_token)` đã sắp đúng chiều. `get_reserves_vs_wbnb` ở trên giờ chỉ
/// là lớp mỏng gọi hàm này với `quote=wbnb()` — KHÔNG đổi hành vi/chữ ký cũ
/// cho bất kỳ caller nào (`pipeline::resolve_v2_reserves` vẫn gọi
/// `get_reserves_vs_wbnb` y hệt trước).
pub async fn get_reserves_vs_quote(provider: &dyn Provider, pair: Address, quote: Address) -> Result<(U256, U256), String> {
    let token0_tx = TransactionRequest::default().to(pair).input(SEL_TOKEN0.to_vec().into());
    let token0_ret = provider.call(token0_tx).await.map_err(|e| format!("eth_call token0 that bai: {e}"))?;
    let token0 = decode_address_return(&token0_ret).ok_or_else(|| "token0 tra ve du lieu qua ngan".to_string())?;

    let reserves_tx = TransactionRequest::default().to(pair).input(SEL_GET_RESERVES.to_vec().into());
    let ret = provider.call(reserves_tx).await.map_err(|e| format!("eth_call getReserves that bai: {e}"))?;
    let (reserve0, reserve1) =
        decode_reserves_return(&ret).ok_or_else(|| "getReserves tra ve du lieu qua ngan".to_string())?;

    if token0 == quote {
        Ok((reserve0, reserve1))
    } else {
        Ok((reserve1, reserve0))
    }
}

/// Cụm `evm-validate-wire-tax` (B4'.4) — trả RAW `(token0, reserve0,
/// reserve1)` KHÔNG sắp lại theo quote asset nào — dùng để RESET trực tiếp
/// storage slot 8 (`reserve0|reserve1|blockTimestampLast` packed, layout
/// chuẩn `UniswapV2Pair.sol` mà PancakeSwap V2 fork y hệt) trong `revm` giữa
/// các lần thử `front_in` khác nhau của ternary search EVM (`sim_evm.rs`),
/// KHÔNG phải để quyết định sim (đó vẫn là `get_reserves_vs_wbnb`/`_quote`).
pub async fn get_raw_reserves_and_token0(provider: &dyn Provider, pair: Address) -> Result<(Address, U256, U256), String> {
    let token0_tx = TransactionRequest::default().to(pair).input(SEL_TOKEN0.to_vec().into());
    let token0_ret = provider.call(token0_tx).await.map_err(|e| format!("eth_call token0 that bai: {e}"))?;
    let token0 = decode_address_return(&token0_ret).ok_or_else(|| "token0 tra ve du lieu qua ngan".to_string())?;
    let reserves_tx = TransactionRequest::default().to(pair).input(SEL_GET_RESERVES.to_vec().into());
    let ret = provider.call(reserves_tx).await.map_err(|e| format!("eth_call getReserves that bai: {e}"))?;
    let (reserve0, reserve1) = decode_reserves_return(&ret).ok_or_else(|| "getReserves tra ve du lieu qua ngan".to_string())?;
    Ok((token0, reserve0, reserve1))
}

/// Cụm `evm-validate-wire-tax` (B4'.3(b)) — `(token0, token1)` THẬT của 1
/// pair V2 bất kỳ (không cần biết trước 1 trong 2 token) — dùng để nhận
/// diện pair có phải WBNB-pair không khi chỉ có ĐỊA CHỈ PAIR (từ `Swap`
/// event `log.address()`, không có calldata router để suy `token`).
pub async fn get_pair_tokens(provider: &dyn Provider, pair: Address) -> Result<(Address, Address), String> {
    let token0_tx = TransactionRequest::default().to(pair).input(SEL_TOKEN0.to_vec().into());
    let token0_ret = provider.call(token0_tx).await.map_err(|e| format!("eth_call token0 that bai: {e}"))?;
    let token0 = decode_address_return(&token0_ret).ok_or_else(|| "token0 tra ve du lieu qua ngan".to_string())?;
    let token1_tx = TransactionRequest::default().to(pair).input(SEL_TOKEN1.to_vec().into());
    let token1_ret = provider.call(token1_tx).await.map_err(|e| format!("eth_call token1 that bai: {e}"))?;
    let token1 = decode_address_return(&token1_ret).ok_or_else(|| "token1 tra ve du lieu qua ngan".to_string())?;
    Ok((token0, token1))
}

/// `getReserves()` trả `(uint112 reserve0, uint112 reserve1, uint32
/// blockTimestampLast)` — mỗi giá trị right-aligned trong 1 word 32 byte
/// (ABI luôn pad về 32 byte dù kiểu gốc nhỏ hơn). Bỏ qua
/// `blockTimestampLast` (không cần cho sim `3.1`).
fn decode_reserves_return(data: &[u8]) -> Option<(U256, U256)> {
    if data.len() < 64 {
        return None;
    }
    let reserve0 = U256::from_be_slice(&data[0..32]);
    let reserve1 = U256::from_be_slice(&data[32..64]);
    Some((reserve0, reserve1))
}

// ============================================================================
// Cụm `econ-truth-latency-vps` (mục 3) — Sync event (topic0
// `keccak256("Sync(uint112,uint112)")`) để cập nhật `ReserveCache` NGAY khi
// pool đổi state, KHÔNG cần `eth_call getReserves` trên đường nóng mỗi tx.
// ============================================================================

const SYNC_EVENT_SIG: &str = "Sync(uint112,uint112)";

/// Topic0 của event `Sync(uint112,uint112)` (`UniswapV2Pair.sol`, PancakeSwap
/// V2 fork y hệt layout) — suy từ `keccak256` chữ ký thật, KHÔNG hardcode hex
/// nhớ tay (cùng quy ước `sim_evm::swap_topic0`/`pool::CL_INITIALIZE_TOPIC0`).
pub fn sync_topic0() -> B256 {
    keccak256(SYNC_EVENT_SIG.as_bytes())
}

/// `data` (KHÔNG indexed) của log `Sync` = ĐÚNG 2 word 32-byte
/// (`reserve0`,`reserve1`) — cùng layout 2 word đầu của `getReserves()` trả
/// về (bỏ `blockTimestampLast`), dùng lại `decode_reserves_return`.
pub fn decode_sync_log_reserves(data: &[u8]) -> Option<(U256, U256)> {
    decode_reserves_return(data)
}

/// Sắp `(reserve0, reserve1)` THÔ (từ log `Sync`) về đúng chiều
/// `(reserve_quote, reserve_token)` — cần biết TRƯỚC `token0` của pool này
/// (1 lần duy nhất/pool, qua `eth_call token0()` — KHÔNG lặp lại mỗi event,
/// xem `main.rs::subscribe_sync_events`). Cùng logic sắp chiều với
/// `get_reserves_vs_quote` (đã verify RPC thật), tách ra đây để dùng được mà
/// KHÔNG cần `Provider` (test thuần, và dùng trực tiếp trong event handler).
pub fn order_reserves_by_quote(token0: Address, quote: Address, reserve0: U256, reserve1: U256) -> (U256, U256) {
    if token0 == quote {
        (reserve0, reserve1)
    } else {
        (reserve1, reserve0)
    }
}

/// `PoolId = keccak256(PoolKey)` — theo `PoolId.sol::PoolIdLibrary.toId`
/// (nguồn `github.com/pancakeswap/infinity-core`, `src/types/PoolId.sol`,
/// đọc trực tiếp phiên `v4-pool-resolve`): `keccak256` trên đúng 192 byte
/// (6 slot 32-byte của struct `PoolKey` trong memory — `Currency`/`IHooks`/
/// `IPoolManager` đều là custom type bọc `address`, mỗi field CHIẾM ĐỦ 1
/// slot 32 byte dù kiểu gốc nhỏ hơn, đúng layout memory Solidity, không
/// phải `abi.encodePacked`). Verify BIT-FOR-BIT khớp `id` thật lấy từ 1 log
/// `Initialize` THẬT trên BSC mainnet (test
/// `compute_pool_id_matches_real_onchain_event`, dữ liệu chain thật dán ở
/// `docs/STATE.md`/BAOCAO19) — không phải suy đoán công thức.
pub fn compute_pool_id(currency0: Address, currency1: Address, hooks: Address, pool_manager: Address, fee: u32, parameters: [u8; 32]) -> B256 {
    let mut buf = Vec::with_capacity(192);
    encode_address_word(currency0, &mut buf);
    encode_address_word(currency1, &mut buf);
    encode_address_word(hooks, &mut buf);
    encode_address_word(pool_manager, &mut buf);
    encode_uint24_word(fee, &mut buf);
    buf.extend_from_slice(&parameters);
    keccak256(&buf)
}

/// Giải mã 1 log `Initialize` thật (topics = `[topic0, id, currency0,
/// currency1]`, data = `hooks(32B) ++ fee(32B) ++ parameters(32B) ++ ...`
/// — phần đuôi sau `parameters` khác nhau giữa CL/Bin, không cần cho
/// `PoolKey` nên bỏ qua, chỉ cần `data.len() >= 96`). Tính lại `PoolId` từ
/// dữ liệu decode được rồi ĐỐI CHIẾU với `id` (topic 1, do chain tự tính) —
/// không khớp thì log bất thường/decode sai, bỏ log đó (phòng vệ, chưa từng
/// xảy ra với dữ liệu thật phiên này).
fn decode_initialize_log(log: &Log, family: InfinityPoolFamily, pool_manager: Address) -> Option<InfinityPoolMatch> {
    let topics = log.topics();
    if topics.len() < 4 {
        return None;
    }
    let id_topic = topics[1];
    let currency0 = Address::from_word(topics[2]);
    let currency1 = Address::from_word(topics[3]);

    let data = &log.data().data;
    if data.len() < 96 {
        return None;
    }
    let hooks = decode_address_return(&data[0..32])?;
    let fee_bytes: [u8; 4] = data[60..64].try_into().ok()?;
    let fee = u32::from_be_bytes(fee_bytes);
    let mut parameters = [0u8; 32];
    parameters.copy_from_slice(&data[64..96]);

    let computed_id = compute_pool_id(currency0, currency1, hooks, pool_manager, fee, parameters);
    if computed_id != id_topic {
        return None;
    }

    Some(InfinityPoolMatch { family, pool_manager, pool_id: computed_id, currency0, currency1, hooks, fee, parameters })
}

/// Quét `eth_getLogs` thật trên 1 pool manager, lọc `topic0` = event
/// `Initialize` đúng family, `topic2`/`topic3` = tập `{token, wbnb}` (2
/// chiều — 1 `Filter` OR trên từng vị trí khớp cả `(token,wbnb)` lẫn
/// `(wbnb,token)`, xem `docs/STATE.md`). Chia block range thành chunk
/// `LOG_SCAN_CHUNK_BLOCKS` để không vượt giới hạn RPC free-tier thật đo
/// được phiên này — KHÔNG quét toàn chain (`from_block`/`to_block` do caller
/// truyền, phiên này chưa pin block deploy PoolManager nên KHÔNG có mặc
/// định cứng, xem MISSING trong BAOCAO19).
async fn scan_initialize_logs(
    provider: &dyn Provider,
    manager: Address,
    topic0: B256,
    token: Address,
    wbnb: Address,
    from_block: u64,
    to_block: u64,
) -> Result<Vec<Log>, String> {
    if from_block > to_block {
        return Err(format!("from_block ({from_block}) > to_block ({to_block}) - khoang block khong hop le"));
    }
    let currency_topic = vec![token.into_word(), wbnb.into_word()];
    let mut out = Vec::new();
    let mut chunk_start = from_block;
    loop {
        let chunk_end = chunk_start.saturating_add(LOG_SCAN_CHUNK_BLOCKS - 1).min(to_block);
        let filter = Filter::new()
            .address(manager)
            .event_signature(topic0)
            .topic2(currency_topic.clone())
            .topic3(currency_topic.clone())
            .from_block(chunk_start)
            .to_block(chunk_end);
        let logs = provider
            .get_logs(&filter)
            .await
            .map_err(|e| format!("eth_getLogs that bai (manager {manager:#x}, block {chunk_start}-{chunk_end}): {e}"))?;
        out.extend(logs);
        if chunk_end >= to_block {
            break;
        }
        chunk_start = chunk_end + 1;
    }
    Ok(out)
}

/// V4/Infinity: quét THẬT sự kiện `Initialize` trên `CLPoolManager` +
/// `BinPoolManager` đã pin (`DEX_REGISTRY.md`) qua `eth_getLogs` trong
/// `[from_block, to_block]` (caller truyền — xem `scan_initialize_logs`),
/// dựng lại `PoolKey` thật cho cặp `{token, WBNB}`. Không tìm thấy log nào
/// -> `Ok(Err(HooksUnread))` (đúng luật CLAUDE.md, skip đúng pool đó). Tìm
/// thấy NHIỀU pool cùng cặp (khác fee tier/hook) -> trả TẤT CẢ, KHÔNG tự
/// chọn 1 cái — tầng sim (cụm khác, chưa làm phiên này) tự chọn max profit.
/// Lỗi RPC (mạng/node) tách riêng ở `Err(String)` ngoài, giống
/// `resolve_v2_pair`/`resolve_v3_pool`.
pub async fn resolve_infinity_pool(
    provider: &dyn Provider,
    cl_pool_manager: Address,
    bin_pool_manager: Address,
    token: Address,
    from_block: u64,
    to_block: u64,
) -> Result<Result<Vec<InfinityPoolMatch>, PoolSkipReason>, String> {
    let wbnb_addr = wbnb();
    let mut matches = Vec::new();
    let targets = [
        (cl_pool_manager, *CL_INITIALIZE_TOPIC0, InfinityPoolFamily::Cl),
        (bin_pool_manager, *BIN_INITIALIZE_TOPIC0, InfinityPoolFamily::Bin),
    ];
    for (manager, topic0, family) in targets {
        let logs = scan_initialize_logs(provider, manager, topic0, token, wbnb_addr, from_block, to_block).await?;
        for log in &logs {
            if let Some(m) = decode_initialize_log(log, family, manager) {
                matches.push(m);
            }
        }
    }
    if matches.is_empty() {
        Ok(Err(PoolSkipReason::HooksUnread))
    } else {
        Ok(Ok(matches))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(hex: &str) -> Address {
        Address::from_str(hex).unwrap()
    }

    #[test]
    fn get_pair_selector_matches_well_known_value() {
        // getPair(address,address) — selector chuẩn Uniswap-V2-fork, đối
        // chiếu tránh suy luận sai chữ ký hàm.
        assert_eq!(*SEL_GET_PAIR, [0xe6, 0xa4, 0x39, 0x05]);
    }

    #[test]
    fn get_pool_selector_matches_well_known_value() {
        assert_eq!(*SEL_GET_POOL, [0x16, 0x98, 0xee, 0x82]);
    }

    /// Cụm `usdt-quote-asset` — `build_get_pair_calldata` (dùng bởi
    /// `resolve_v2_pair_for_quote`) phải encode ĐÚNG với quote KHÁC WBNB (vd
    /// USDT) — chứng minh generic hoá không hardcode WBNB vào calldata.
    #[test]
    fn calldata_encodes_correctly_for_non_wbnb_quote_usdt() {
        let token = addr("0x111111111111111111111111111111111111beef");
        let usdt = addr("0x55d398326f99059ff775485246999027b3197955");
        let calldata = build_get_pair_calldata(token, usdt);
        assert_eq!(calldata.len(), 4 + 32 + 32);
        assert_eq!(&calldata[0..4], SEL_GET_PAIR.as_slice());
        assert_eq!(&calldata[4 + 12..4 + 32], token.as_slice());
        assert_eq!(&calldata[4 + 32 + 12..4 + 64], usdt.as_slice(), "quote phai duoc encode dung, khong bi hardcode WBNB");
    }

    #[test]
    fn calldata_encodes_selector_then_two_address_words() {
        let a = addr("0x111111111111111111111111111111111111beef");
        let b = wbnb();
        let calldata = build_get_pair_calldata(a, b);
        assert_eq!(calldata.len(), 4 + 32 + 32);
        assert_eq!(&calldata[0..4], SEL_GET_PAIR.as_slice());
        assert_eq!(&calldata[4 + 12..4 + 32], a.as_slice());
        assert_eq!(&calldata[4 + 32 + 12..4 + 64], b.as_slice());
    }

    #[test]
    fn calldata_get_pool_encodes_fee_right_aligned_uint24() {
        let a = addr("0x222222222222222222222222222222222222cafe");
        let calldata = build_get_pool_calldata(a, wbnb(), 2500);
        assert_eq!(calldata.len(), 4 + 32 + 32 + 32);
        let fee_word = &calldata[4 + 64..4 + 96];
        assert_eq!(fee_word[..29], [0u8; 29]);
        assert_eq!(u32::from_be_bytes([0, fee_word[29], fee_word[30], fee_word[31]]), 2500);
    }

    #[test]
    fn decode_address_return_reads_last_20_bytes() {
        let mut ret = vec![0u8; 32];
        let a = addr("0x333333333333333333333333333333333333dead");
        ret[12..32].copy_from_slice(a.as_slice());
        assert_eq!(decode_address_return(&ret), Some(a));
    }

    #[test]
    fn decode_address_return_too_short_is_none() {
        assert_eq!(decode_address_return(&[0u8; 10]), None);
    }

    #[test]
    fn zero_address_pair_is_no_pool() {
        assert!(is_zero_address(Address::ZERO));
        assert!(!is_zero_address(wbnb()));
    }

    #[test]
    fn get_reserves_selector_matches_well_known_value() {
        // getReserves() — selector chuẩn Uniswap-V2-fork.
        assert_eq!(*SEL_GET_RESERVES, [0x09, 0x02, 0xf1, 0xac]);
    }

    #[test]
    fn token0_selector_matches_well_known_value() {
        assert_eq!(*SEL_TOKEN0, [0x0d, 0xfe, 0x16, 0x81]);
    }

    #[test]
    fn decode_reserves_return_reads_two_u256_words_ignores_timestamp() {
        let mut ret = vec![0u8; 0];
        ret.extend_from_slice(&U256::from(111u64).to_be_bytes::<32>());
        ret.extend_from_slice(&U256::from(222u64).to_be_bytes::<32>());
        ret.extend_from_slice(&U256::from(9_999_999u64).to_be_bytes::<32>()); // blockTimestampLast, bo qua
        assert_eq!(decode_reserves_return(&ret), Some((U256::from(111u64), U256::from(222u64))));
    }

    #[test]
    fn decode_reserves_return_too_short_is_none() {
        assert_eq!(decode_reserves_return(&[0u8; 40]), None);
    }

    // ===== Cụm `econ-truth-latency-vps` (mục 3) — Sync event =====

    #[test]
    fn sync_topic0_matches_known_keccak() {
        // keccak256("Sync(uint112,uint112)") — giá trị chuẩn PancakeSwap
        // V2/UniswapV2 (quan sát thật trên nhiều block explorer).
        assert_eq!(format!("{:#x}", sync_topic0()), "0x1c411e9a96e071241c2f21f7726b17ae89e3cab4c78be50e062b03a9fffbbad1");
    }

    #[test]
    fn decode_sync_log_reserves_reads_two_words() {
        let mut data = Vec::new();
        data.extend_from_slice(&U256::from(500u64).to_be_bytes::<32>());
        data.extend_from_slice(&U256::from(999u64).to_be_bytes::<32>());
        assert_eq!(decode_sync_log_reserves(&data), Some((U256::from(500u64), U256::from(999u64))));
    }

    #[test]
    fn decode_sync_log_reserves_too_short_is_none() {
        assert_eq!(decode_sync_log_reserves(&[0u8; 10]), None);
    }

    #[test]
    fn order_reserves_by_quote_token0_is_quote_keeps_order() {
        let quote = wbnb();
        let token = addr("0x2222222222222222222222222222222222222222");
        let (rq, rt) = order_reserves_by_quote(quote, quote, U256::from(10u64), U256::from(20u64));
        assert_eq!((rq, rt), (U256::from(10u64), U256::from(20u64)));
        // token0 = token (khac quote) -> phai dao nguoc.
        let (rq2, rt2) = order_reserves_by_quote(token, quote, U256::from(10u64), U256::from(20u64));
        assert_eq!((rq2, rt2), (U256::from(20u64), U256::from(10u64)));
    }

    #[test]
    fn v3_fee_tiers_match_pancake_factory_constructor() {
        assert_eq!(V3_FEE_TIERS, [100, 500, 2500, 10000]);
    }

    /// Parse hex (có/không `0x`) thành `Vec<u8>` — chỉ dùng dựng fixture test
    /// từ dữ liệu log thật, không phải logic production.
    fn hex_to_bytes(s: &str) -> Vec<u8> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    fn hex_to_b256(s: &str) -> B256 {
        B256::from_slice(&hex_to_bytes(s))
    }

    #[test]
    fn cl_initialize_topic0_matches_known_value() {
        // Đối chiếu keccak256 tự tính (không hardcode hex nhớ tay) với giá
        // trị đã dùng THẬT để lọc `eth_getLogs` runtime thật phiên
        // `v4-pool-resolve` (dán response thật ở docs/STATE.md/BAOCAO19) —
        // eth_getLogs trả kết quả đúng với topic0 này chứng minh chữ ký
        // event suy từ `ICLPoolManager.sol` là đúng.
        assert_eq!(*CL_INITIALIZE_TOPIC0, hex_to_b256("426cc62fe6a33a40ba2788c2c87a9c34ee4582b95bc9fa5a7bb7ae70b750b99c"));
    }

    #[test]
    fn bin_initialize_topic0_matches_known_value() {
        assert_eq!(*BIN_INITIALIZE_TOPIC0, hex_to_b256("ddfde5903015c0eb1671976c6c8f760f1328bec57f15286b6bdab2f955cab9c9"));
    }

    /// Dữ liệu 1 log `Initialize` THẬT trên BSC mainnet — `CLPoolManager`
    /// (`0xa0ffb9c1ce1fe56963b0321b32e7a0302114058b`, đã pin), block
    /// `121882377`, tx `0xca9952e56f7e0168d420898b0a78eb48cac27c12137a0a7cdb3f020abe0e341f`,
    /// lấy qua `eth_getLogs` thật trên `bsc-rpc.publicnode.com` phiên
    /// `v4-pool-resolve` (2026-09-15, dán đầy đủ raw response ở
    /// `docs/STATE.md`). Cặp `currency0=USDT`/`currency1=0x65e7a112...`
    /// KHÔNG phải WBNB (phiên này không tìm được ví dụ WBNB thật trong tầm
    /// với RPC free-tier — xem MISSING ở BAOCAO19) nhưng vẫn là log thật
    /// 100%, đủ để verify công thức `PoolId`/decode bit-for-bit đúng với
    /// dữ liệu on-chain thật, không phải fixture bịa.
    fn real_cl_initialize_log() -> Log {
        let manager = addr("0xa0ffb9c1ce1fe56963b0321b32e7a0302114058b");
        let topics = vec![
            *CL_INITIALIZE_TOPIC0,
            hex_to_b256("b7fdb401951747b812b65ebdefb3aae879fc7cf4027de366b5011832d6e2080b"),
            hex_to_b256("00000000000000000000000055d398326f99059ff775485246999027b3197955"),
            hex_to_b256("00000000000000000000000065e7a112db1142eae919201b1232f7aa488ed83c"),
        ];
        let data_bytes = hex_to_bytes(concat!(
            "0000000000000000000000000000000000000000000000000000000000000000",
            "00000000000000000000000000000000000000000000000000000000000db622",
            "0000000000000000000000000000000000000000000000000000000000010000",
            "0000000000000000000000000000000000000000000088fc42d902766a40259d",
            "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffc6cab",
        ));
        let log_data = alloy::primitives::LogData::new_unchecked(topics, data_bytes.into());
        let inner = alloy::primitives::Log { address: manager, data: log_data };
        Log { inner, ..Default::default() }
    }

    #[test]
    fn compute_pool_id_matches_real_onchain_event() {
        let log = real_cl_initialize_log();
        let manager = log.address();
        let currency0 = addr("0x55d398326f99059ff775485246999027b3197955");
        let currency1 = addr("0x65e7a112db1142eae919201b1232f7aa488ed83c");
        let hooks = Address::ZERO;
        let fee = 898594u32;
        let parameters = {
            let mut p = [0u8; 32];
            p.copy_from_slice(&hex_to_bytes("0000000000000000000000000000000000000000000000000000000000010000"));
            p
        };
        let computed = compute_pool_id(currency0, currency1, hooks, manager, fee, parameters);
        assert_eq!(computed, log.topics()[1], "PoolId tu tinh phai khop id that tren chain (topic1)");
    }

    #[test]
    fn decode_initialize_log_roundtrip_on_real_onchain_log() {
        let log = real_cl_initialize_log();
        let manager = log.address();
        let m = decode_initialize_log(&log, InfinityPoolFamily::Cl, manager).expect("log that phai decode duoc");
        assert_eq!(m.family, InfinityPoolFamily::Cl);
        assert_eq!(m.pool_manager, manager);
        assert_eq!(m.currency0, addr("0x55d398326f99059ff775485246999027b3197955"));
        assert_eq!(m.currency1, addr("0x65e7a112db1142eae919201b1232f7aa488ed83c"));
        assert_eq!(m.hooks, Address::ZERO);
        assert_eq!(m.fee, 898594);
        assert_eq!(m.pool_id, log.topics()[1]);
    }

    #[test]
    fn decode_initialize_log_rejects_when_id_does_not_match_recomputed_pool_id() {
        let mut log = real_cl_initialize_log();
        // Bóp méo currency1 (giả lập log bất thường/decode sai) -> PoolId
        // tự tính sẽ lệch topic1 that -> phải bị từ chối (None), không tin
        // liều 1 phần dữ liệu khi phần khác đã sai.
        log.inner.data.topics_mut()[3] = hex_to_b256("000000000000000000000000000000000000000000000000000000000000dead");
        assert_eq!(decode_initialize_log(&log, InfinityPoolFamily::Cl, log.address()), None);
    }

    #[test]
    fn decode_initialize_log_rejects_too_few_topics() {
        let mut log = real_cl_initialize_log();
        log.inner.data.set_topics_truncating(vec![*CL_INITIALIZE_TOPIC0]);
        assert_eq!(decode_initialize_log(&log, InfinityPoolFamily::Cl, log.address()), None);
    }

    #[test]
    fn decode_initialize_log_rejects_short_data() {
        let mut log = real_cl_initialize_log();
        log.inner.data.data = alloy::primitives::Bytes::from(vec![0u8; 64]); // < 96 byte toi thieu
        assert_eq!(decode_initialize_log(&log, InfinityPoolFamily::Cl, log.address()), None);
    }

    #[test]
    fn infinity_pool_family_as_str() {
        assert_eq!(InfinityPoolFamily::Cl.as_str(), "cl");
        assert_eq!(InfinityPoolFamily::Bin.as_str(), "bin");
    }

    /// `#[ignore]` — chỉ chạy thủ công (`cargo test -- --ignored`) khi có RPC
    /// sống, KHÔNG nằm trong `cargo test` mặc định (đúng "test không bắt live
    /// node"). Dùng RPC công khai giống cách BAOCAO02 pin registry — không
    /// phải RPC runtime của bot (`.env`/`BSC_HTTP` vẫn do chủ điền, không đổi
    /// ở đây). USDT (BSC-USD) là token BSC nổi tiếng, chỉ dùng để verify
    /// `getPair` trả về 1 pool thật, không phải victim/token bịa.
    #[tokio::test]
    #[ignore]
    async fn real_rpc_v2_get_pair_wbnb_usdt() {
        use alloy::providers::{Provider, ProviderBuilder};

        let usdt = addr("0x55d398326f99059ff775485246999027b3197955");
        let v2_factory = addr("0xca143ce32fe78f1f7019d7d551a6402fc5350c73");

        let provider = ProviderBuilder::new()
            .connect("https://bsc-dataseed.binance.org/")
            .await
            .expect("ket noi RPC cong khai that bai");
        let chain_id = provider.get_chain_id().await.expect("eth_chainId that bai");
        assert_eq!(chain_id, 56);

        let outcome = resolve_v2_pair(&provider, v2_factory, usdt).await.expect("eth_call that bai");
        let pair = outcome.expect("USDT/WBNB phai co pool V2 that tren mainnet");
        println!("V2 getPair(USDT, WBNB) real pair = {pair:#x}");
        assert!(!is_zero_address(pair));
    }

    /// `#[ignore]` — chỉ chạy thủ công. Cụm `usdt-quote-asset` (BAOCAO29):
    /// chứng minh `resolve_v2_pair_for_quote`/`get_reserves_vs_quote` (tổng
    /// quát hoá MỚI) hoạt động đúng với quote KHÁC WBNB thật trên mainnet —
    /// gọi `getPair(WBNB, USDT)` (2 token nổi tiếng, chắc chắn có pool V2
    /// thật) rồi đọc reserve theo chiều USDT.
    #[tokio::test]
    #[ignore]
    async fn real_rpc_v2_get_pair_and_reserves_for_non_wbnb_quote_usdt() {
        use alloy::providers::{Provider, ProviderBuilder};

        let usdt = addr("0x55d398326f99059ff775485246999027b3197955");
        let v2_factory = addr("0xca143ce32fe78f1f7019d7d551a6402fc5350c73");

        let provider = ProviderBuilder::new()
            .connect("https://bsc-dataseed.binance.org/")
            .await
            .expect("ket noi RPC cong khai that bai");
        let chain_id = provider.get_chain_id().await.expect("eth_chainId that bai");
        assert_eq!(chain_id, 56);

        let outcome = resolve_v2_pair_for_quote(&provider, v2_factory, wbnb(), usdt).await.expect("eth_call that bai");
        let pair = outcome.expect("WBNB/USDT phai co pool V2 that tren mainnet");
        println!("V2 getPair(WBNB, quote=USDT) real pair = {pair:#x}");
        assert!(!is_zero_address(pair));

        let (reserve_usdt, reserve_wbnb) = get_reserves_vs_quote(&provider, pair, usdt).await.expect("getReserves that bai");
        println!("reserve_usdt={reserve_usdt} reserve_wbnb={reserve_wbnb}");
        assert!(reserve_usdt > U256::ZERO && reserve_wbnb > U256::ZERO);
    }

    /// `#[ignore]` — chỉ chạy thủ công. Quét lại CHÍNH block range đã tìm ra
    /// `real_cl_initialize_log()` (block `121882377`, `bsc-rpc.publicnode.com`)
    /// bằng `scan_initialize_logs` thật (không phải fixture tay) — chứng
    /// minh cả pipeline `Filter`/`eth_getLogs`/chunk thật hoạt động, không
    /// chỉ hàm decode thuần. Range cố định lịch sử (không tính theo "khối
    /// mới nhất") nên deterministic — nhưng CÓ THỂ lỗi "Archive requests
    /// require a personal token" ở phiên sau nếu node công khai đẩy biên
    /// free-tier xa hơn theo thời gian (giới hạn hạ tầng ngoài tầm kiểm soát
    /// code, xem `docs/STATE.md`).
    #[tokio::test]
    #[ignore]
    async fn real_rpc_scan_finds_known_historical_cl_initialize_log() {
        use alloy::providers::ProviderBuilder;

        let cl_manager = addr("0xa0ffb9c1ce1fe56963b0321b32e7a0302114058b");
        let usdt = addr("0x55d398326f99059ff775485246999027b3197955");
        let other_currency = addr("0x65e7a112db1142eae919201b1232f7aa488ed83c");

        let provider = ProviderBuilder::new()
            .connect("https://bsc-rpc.publicnode.com")
            .await
            .expect("ket noi RPC cong khai that bai");

        let logs = scan_initialize_logs(&provider, cl_manager, *CL_INITIALIZE_TOPIC0, usdt, other_currency, 121_882_370, 121_882_385)
            .await
            .expect("eth_getLogs that bai");
        assert!(!logs.is_empty(), "phai tim thay lai log Initialize that da biet tu phien v4-pool-resolve");
        let decoded = decode_initialize_log(&logs[0], InfinityPoolFamily::Cl, cl_manager).expect("log that phai decode duoc");
        println!("real scan found pool_id = {:#x}", decoded.pool_id);
        assert_eq!(decoded.currency0, usdt);
        assert_eq!(decoded.currency1, other_currency);
    }

    /// `#[ignore]` — chỉ chạy thủ công. Gọi `resolve_infinity_pool` THẬT
    /// (đường public API của cụm này) trên 1 cửa sổ block GẦN NHẤT (tính
    /// động theo `get_block_number()`, không cố định) — chứng minh hàm
    /// public hoạt động end-to-end với RPC sống. KHÔNG assert tìm thấy pool
    /// WBNB (phiên `v4-pool-resolve` không tìm được ví dụ WBNB thật nào
    /// trong tầm với RPC free-tier — xem MISSING ở BAOCAO19) — chỉ assert
    /// hàm chạy xong không lỗi RPC và trả đúng kiểu `HooksUnread` khi rỗng
    /// (đường "không tìm thấy" hợp lệ, không phải lỗi).
    #[tokio::test]
    #[ignore]
    async fn real_rpc_resolve_infinity_pool_runs_end_to_end_on_recent_window() {
        use alloy::providers::{Provider, ProviderBuilder};

        let cl_manager = addr("0xa0ffb9c1ce1fe56963b0321b32e7a0302114058b");
        let bin_manager = addr("0xc697d2898e0d09264376196696c51d7abbbaa4a9");
        // Token bat ky (USDT that) - chi de goi thu duong that, khong gia dinh
        // no co pool Infinity/WBNB (phien nay chua tim duoc vi du that).
        let token = addr("0x55d398326f99059ff775485246999027b3197955");

        let provider = ProviderBuilder::new()
            .connect("https://bsc-rpc.publicnode.com")
            .await
            .expect("ket noi RPC cong khai that bai");
        let latest = provider.get_block_number().await.expect("eth_blockNumber that bai");
        let from_block = latest.saturating_sub(9_000);

        let outcome = resolve_infinity_pool(&provider, cl_manager, bin_manager, token, from_block, latest)
            .await
            .expect("eth_getLogs khong duoc loi RPC (dung provider that, cua so nho)");
        match outcome {
            Ok(matches) => println!("tim thay {} pool Infinity WBNB/USDT that (bat ngo nhung hop le)", matches.len()),
            Err(PoolSkipReason::HooksUnread) => println!("khong tim thay pool WBNB/USDT Infinity trong cua so nay - dung luat, skip pool do"),
            Err(other) => panic!("ket qua khong mong doi: {other:?}"),
        }
    }
}

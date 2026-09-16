//! Cụm `foundation-fix-then-real-sim` (B) — sim sandwich bằng EVM THẬT
//! (`revm`, fork 1 block cụ thể qua `AlloyDB`) thay vì chỉ dựa công thức
//! đóng (`sim_v2.rs`, vẫn giữ lại CHỈ để ước lượng khoảng `front_in`, xem
//! `pipeline.rs`). Chạy đúng calldata router THẬT
//! (`*SupportingFeeOnTransferTokens`) nên tax/fee-on-transfer/honeypot tự lộ
//! ra qua `balanceOf` thực tế sau transfer — không cần đoán/không cần hợp
//! đồng "probe" viết tay (khác giới hạn cũ ghi trong `tax.rs`).
//!
//! PHẠM VI phiên này: quote asset WBNB, pool V2 (khớp `sim_v2::PoolReserves`)
//! — front-buy dùng NATIVE BNB thẳng (`swapExactETHForTokensSupportingFeeOnTransferTokens`,
//! payable, không cần `WBNB.deposit()`/approve trước), back-sell trả về
//! NATIVE BNB (`swapExactTokensForETHSupportingFeeOnTransferTokens`). Quote
//! USDT (không có cơ chế wrap native) CHƯA làm ở đây — CÒN NỢ, xem BAOCAO31.
//!
//! Attacker EOA là địa chỉ CỐ ĐỊNH, KHÔNG PHẢI ví thật (`ATTACKER_ADDRESS`)
//! — nạp số dư native qua `CacheDB::insert_account_info` (ghi đè lớp cache
//! CỤC BỘ, không đụng chain thật), gas_price=0 cho MỌI tx của attacker (front/
//! approve/back) để `profit_wei = back_out - front_in` KHÔNG lẫn chi phí gas
//! (đúng AGENTS.md — gas chặn riêng bằng field BNB, không trộn vào đây). Tx
//! của VICTIM replay ĐÚNG `from`/`to`/`value`/`input`/`gas`/`gas_price`/`nonce`
//! thật — không override gì (đang dùng đúng state thật của họ tại block fork
//! qua `AlloyDB`, không cần cấp giả).
//!
//! Không tìm `front_in` tối ưu bằng full EVM ternary search (CÒN NỢ, xem
//! BAOCAO31 — mỗi lần thử tốn round-trip RPC qua `AlloyDB`, ternary search
//! nhiều bước sẽ tốn rất nhiều `eth_call`/`eth_getStorageAt`): dùng THẲNG
//! ước lượng `sim_v2::search_max_front_in` (công thức đóng, rẻ, off-chain)
//! làm `front_in` DUY NHẤT đưa vào EVM thật — EVM thật ở đây đóng vai trò XÁC
//! NHẬN/ĐO LẠI profit thật (có tax) tại đúng mức đó, không phải tìm lại từ
//! đầu. Việc thu hẹp thêm bằng vài điểm quanh ước lượng (`refine_candidates`)
//! có hỗ trợ nhưng mặc định gọi 1 điểm duy nhất, xem `pipeline.rs`.

use alloy::primitives::{Address, Bytes, TxKind, U256};
use alloy::providers::{DynProvider, Provider};
use alloy::sol;
use alloy::sol_types::SolCall;
use revm::context::TxEnv;
use revm::context_interface::ContextTr;
use revm::database::{AlloyDB, BlockId, CacheDB, WrapDatabaseAsync};
use revm::primitives::hardfork::SpecId;
use revm::state::AccountInfo;
use revm::{Context, Database, ExecuteCommitEvm, ExecuteEvm, MainBuilder, MainContext};
use std::str::FromStr;

use crate::transport::PendingTxRaw;
use crate::venues::{V2_ROUTER_ADDRESS, WBNB_ADDRESS};

sol! {
    interface IPancakeV2RouterFeeOnTransfer {
        function swapExactETHForTokensSupportingFeeOnTransferTokens(
            uint256 amountOutMin, address[] calldata path, address to, uint256 deadline
        ) external payable;

        function swapExactTokensForETHSupportingFeeOnTransferTokens(
            uint256 amountIn, uint256 amountOutMin, address[] calldata path, address to, uint256 deadline
        ) external;

        function swapExactTokensForTokensSupportingFeeOnTransferTokens(
            uint256 amountIn, uint256 amountOutMin, address[] calldata path, address to, uint256 deadline
        ) external;

        function getAmountsOut(uint256 amountIn, address[] calldata path) external view returns (uint256[] memory amounts);
    }

    interface IERC20Min {
        function balanceOf(address owner) external view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
    }
}

/// EOA giả cố định dùng làm attacker trong sim — KHÔNG PHẢI ví thật, không
/// bao giờ ký/gửi gì lên chain thật (chỉ tồn tại trong `CacheDB` cục bộ của 1
/// lần gọi `simulate_sandwich`). Số dư của địa chỉ này trên chain thật (nếu
/// có) KHÔNG được đọc — `insert_account_info` ghi đè hoàn toàn TRƯỚC khi bất
/// kỳ tx nào chạy, `CacheDB` không bao giờ hỏi lại `AlloyDB` cho địa chỉ đã
/// có trong cache.
pub fn attacker_address() -> Address {
    Address::from_str("0x00000000000000000000000000005a4e64494348").expect("dia chi hop le")
}

const DEADLINE_MAX: u64 = u64::MAX;
/// Đệm số dư native cấp cho attacker (BNB) — chỉ cần > `front_in` bất kỳ,
/// không ảnh hưởng kết quả AMM (chỉ `front_in` truyền vào calldata mới quyết
/// định số lượng swap thật). 1,000,000 BNB đủ dư cho `max_front_bnb` thực tế.
const FUND_BNB_WEI: u128 = 1_000_000_000_000_000_000_000_000u128;

#[derive(Debug)]
pub enum SimEvmError {
    Fork(String),
    Exec(String),
    Decode(String),
    /// Cụm `evm-validate-fixed-then-wire` (B3.5) — 1 tx trong sandwich REVERT
    /// thật ở tầng EVM (front-buy/back-sell/approve trả `!is_success`), KHÔNG
    /// phải lỗi RPC/fork. Đây là tín hiệu HONEYPOT/anti-bot thật (token chặn
    /// bán, reflection làm lệch số, `TransferHelper: TRANSFER_FROM_FAILED`,
    /// `PancakeLibrary: INSUFFICIENT_INPUT_AMOUNT`...) — `decide_with_evm`
    /// dịch thành `honeypot_or_tax` (an toàn: không sim tiếp), KHÔNG phải
    /// `sim_error` (vốn dành cho RPC không kham nổi tải EVM).
    Revert(String),
}

impl SimEvmError {
    /// `true` khi là revert EVM (B3.5) — caller phân biệt honeypot_or_tax với
    /// sim_error.
    pub fn is_revert(&self) -> bool {
        matches!(self, SimEvmError::Revert(_))
    }
}

impl std::fmt::Display for SimEvmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimEvmError::Fork(e) => write!(f, "sim_evm fork loi: {e}"),
            SimEvmError::Exec(e) => write!(f, "sim_evm thuc thi loi: {e}"),
            SimEvmError::Decode(e) => write!(f, "sim_evm decode ket qua loi: {e}"),
            SimEvmError::Revert(e) => write!(f, "sim_evm revert EVM (honeypot/anti-bot): {e}"),
        }
    }
}
impl std::error::Error for SimEvmError {}

/// Kết quả 1 lần sim sandwich bằng EVM thật — tax/honeypot đã PHẢN ÁNH THẬT
/// trong `token_received`/`back_out` (không cần đo riêng), vì calldata dùng
/// đúng biến thể `*SupportingFeeOnTransferTokens`.
#[derive(Debug, Clone)]
pub struct EvmSandwichOutcome {
    pub front_in: U256,
    /// Token attacker nhận được sau front-buy (ĐÃ trừ tax mua nếu có).
    pub token_received: U256,
    /// BNB attacker nhận được sau back-sell (ĐÃ trừ tax bán nếu có).
    pub back_out: U256,
    /// `back_out - front_in`, KHÔNG trừ gas (đúng AGENTS.md, gas chặn riêng).
    pub profit_wei: i128,
    pub victim_success: bool,
    /// Cụm `truth-victim-ok-and-memleak` (mục 1) — LÝ DO victim revert, decode
    /// từ output của `ExecutionResult::Revert` (chuẩn Solidity
    /// `Error(string)` = selector `0x08c379a0`). Đây là thứ PHÂN ĐỊNH được 2
    /// giả thuyết ghi ở `docs/STATE.md` mục 5b mà không cần đoán:
    /// `"PancakeRouter: INSUFFICIENT_OUTPUT_AMOUNT"` ⇒ chân front của ta đẩy
    /// victim qua `amountOutMin`; `"TransferHelper: TRANSFER_FROM_FAILED"` ⇒
    /// state ví victim (thiếu balance/allowance tại block fork); chuỗi khác
    /// (ví dụ luật anti-bot của chính token) ⇒ nguyên nhân thứ 3, ghi nguyên
    /// văn thay vì gán bừa vào 1 trong 2 giả thuyết. `None` khi victim
    /// THÀNH CÔNG, hoặc khi `Halt` (out-of-gas…) — khi đó `victim_halt` mang
    /// mô tả.
    pub victim_revert_reason: Option<String>,
    /// Số token victim THẬT SỰ nhận được trong lần replay này (đo bằng delta
    /// `balanceOf(token, victim.from)` quanh tx victim) — so trực tiếp được
    /// với `victim_out` của V2-math và với `amountOutMin` trong calldata.
    /// `U256::ZERO` khi victim revert.
    pub victim_out: U256,
    /// Ước lượng buy-tax (bps) = so `token_received` thật với
    /// `getAmountsOut` (AMM-math thuần, KHÔNG tax) tại đúng thời điểm front
    /// chạy — `None` nếu `getAmountsOut` lỗi/trả 0 (không đo được).
    pub buy_tax_bps: Option<u32>,
    /// Ước lượng sell-tax (bps), cùng cơ chế, đo NGAY TRƯỚC back-sell (sau
    /// khi victim đã chạy, reserve đã đổi — đúng state tại thời điểm bán).
    pub sell_tax_bps: Option<u32>,
    /// Cụm `verify-cluster-as-victim` (mục 2) — số dư quote của victim ĐỌC
    /// ĐƯỢC tại block fork, TRƯỚC khi nạp thêm gì. `0` cho ví burner của cụm
    /// đối thủ (được cấp vốn trong CHÍNH block đào, xem
    /// `run_sandwich_quote_topup`).
    pub victim_quote_balance_before: U256,
    /// Allowance `victim -> router` tại block fork. KHÔNG BAO GIỜ bị sim ghi
    /// đè — nếu `TRANSFER_FROM_FAILED` còn xảy ra sau khi đã nạp đủ số dư thì
    /// con số này chỉ ra ngay đó là vấn đề allowance thật.
    pub victim_quote_allowance: U256,
    /// `true` khi sim đã GHI THÊM số dư quote cho victim để replay được. Mọi
    /// con số lãi đi kèm cờ này là "lãi NẾU victim có tiền như lúc họ thật sự
    /// chạy", KHÔNG phải lãi đã xác nhận trên state thật của block fork.
    pub victim_quote_topped_up: bool,
}

type ForkDb = CacheDB<WrapDatabaseAsync<AlloyDB<alloy::network::Ethereum, DynProvider>>>;
type ForkContext = revm::handler::MainnetContext<ForkDb>;
type ForkEvm = revm::MainnetEvm<ForkContext>;

fn wbnb() -> Address {
    Address::from_str(WBNB_ADDRESS).expect("WBNB_ADDRESS da pin phai hop le")
}
fn router() -> Address {
    Address::from_str(V2_ROUTER_ADDRESS).expect("V2_ROUTER_ADDRESS da pin phai hop le")
}

/// Cụm `truth-victim-ok-and-memleak` (mục 1) — đọc LÝ DO revert từ
/// `ExecutionResult`. Solidity `require(cond, "msg")` sinh ra output
/// `Error(string)` (selector `0x08c379a0` + ABI-encode 1 string); `Panic(uint)`
/// là `0x4e487b71`. Trả `None` khi tx THÀNH CÔNG. Với `Halt` (out-of-gas,
/// stack…) trả mô tả dạng `"halt: <reason>"` — KHÔNG bịa một chuỗi require
/// không tồn tại. Output revert không theo chuẩn nào (custom error) được trả
/// nguyên dạng hex rút gọn để còn tra lại được.
fn revert_reason(result: &revm::context::result::ExecutionResult) -> Option<String> {
    use revm::context::result::ExecutionResult as ER;
    match result {
        ER::Success { .. } => None,
        ER::Halt { reason, .. } => Some(format!("halt: {reason:?}")),
        ER::Revert { output, .. } => {
            let bytes = output.as_ref();
            if bytes.is_empty() {
                return Some("revert: (khong co du lieu)".to_string());
            }
            if bytes.len() >= 4 && bytes[..4] == [0x08, 0xc3, 0x79, 0xa0] {
                if let Ok(msg) = <String as alloy::sol_types::SolValue>::abi_decode(&bytes[4..]) {
                    return Some(msg);
                }
            }
            if bytes.len() >= 36 && bytes[..4] == [0x4e, 0x48, 0x7b, 0x71] {
                let code = U256::from_be_slice(&bytes[4..36]);
                return Some(format!("panic: {code}"));
            }
            let head: String = bytes.iter().take(16).map(|b| format!("{b:02x}")).collect();
            Some(format!("revert: 0x{head} (len={})", bytes.len()))
        }
    }
}

fn u256_to_u128_bps(numer_diff: U256, denom: U256) -> Option<u32> {
    if denom.is_zero() {
        return None;
    }
    let bps = numer_diff.checked_mul(U256::from(10_000u64))? / denom;
    u32::try_from(bps).ok()
}

/// Cụm `evm-validate-wire-tax` (B4') — tách phần FORK (async, cần
/// `Provider`) khỏi phần THỰC THI (sync, chỉ cần `&mut ForkEvm`) để B4'.4
/// (ternary search EVM) tái dùng ĐÚNG 1 lần fork/fetch cho nhiều lần thử
/// `front_in` khác nhau — không đổi hành vi `simulate_sandwich` (chỉ đổi
/// chỗ, logic bên trong y hệt cũ, xem `real_rpc_sim_evm_matches_sim_v2_when_zero_tax`
/// vẫn pass sau khi tách).
async fn open_fork(provider: DynProvider, fork_block: u64) -> Result<(ForkDb, u64), SimEvmError> {
    // Lay block that (so/timestamp) MOT LAN de dat block env dung - can cho
    // deadline check that su cua router (deadline < block.timestamp -> revert).
    let block_header = provider
        .get_block_by_number(alloy::eips::BlockNumberOrTag::Number(fork_block))
        .await
        .map_err(|e| SimEvmError::Fork(format!("get_block_by_number that bai: {e}")))?
        .ok_or_else(|| SimEvmError::Fork(format!("khong tim thay block {fork_block}")))?;
    let block_timestamp = block_header.header.timestamp;

    let alloy_db = AlloyDB::<alloy::network::Ethereum, DynProvider>::new(provider, BlockId::number(fork_block));
    let wrapped = WrapDatabaseAsync::new(alloy_db)
        .ok_or_else(|| SimEvmError::Fork("WrapDatabaseAsync::new that bai (can tokio multi-thread runtime)".to_string()))?;
    let db: ForkDb = CacheDB::new(wrapped);
    Ok((db, block_timestamp))
}

fn build_evm(db: ForkDb, fork_block: u64, block_timestamp: u64) -> ForkEvm {
    let ctx = Context::mainnet()
        .modify_cfg_chained(|cfg| {
            cfg.chain_id = 56;
            // Sim don thuan kinh te (khong phai validate block that) - tat
            // nonce check de khong phai tu quan ly nonce cho attacker giua
            // cac tx (front/approve/back, ca 3 dung chung nonce=0, xem
            // run_sandwich) - CO CHU Y ap dung cho CA victim vi day la
            // 1 setting toan cuc cua revm (khong co co che tat rieng tung tx).
            //
            // Cum `exec-path-traps` (F-13) - dung y "disable_nonce_check CHI
            // ap cho executor gia, KHONG cho victim" duoc dam bao O BEN NGOAI
            // ham nay: main.rs::run_evm_decision goi transport::fetch_expected_nonce
            // (eth_getTransactionCount that) + transport::compare_nonce TRUOC
            // KHI fork nay duoc mo, tu choi (nonce_stale/nonce_future) moi
            // candidate co nonce victim sai lech - vi vay setting cfg nay chi
            // con anh huong toi 3 tx gia cua attacker (dung y dinh), nonce
            // victim da duoc xac minh THAT qua RPC truoc do, khong phu thuoc
            // revm co check hay khong.
            cfg.disable_nonce_check = true;
            // BSC spec gan nhat pho bien co san trong revm - khong co
            // SpecId rieng cho BSC, cac opcode swap/ERC20 khong dung tinh
            // nang fork-specific (blob/4844) nen an toan dung spec Ethereum
            // moi nhat co san. Ghi ro: KHONG phai khop chinh xac hardfork
            // BSC that, chi la lua chon thuc te (xem BAOCAO31).
            cfg.set_spec_and_mainnet_gas_params(SpecId::CANCUN);
        })
        .modify_block_chained(|b| {
            b.number = U256::from(fork_block);
            b.timestamp = U256::from(block_timestamp);
            b.basefee = 0;
        })
        .with_db(db);
    ctx.build_mainnet()
}

/// Fork tại `fork_block`, cấp số dư native cho attacker, chạy front-buy (NATIVE
/// BNB -> token) -> victim (replay THẬT, nguyên `from`/`to`/`value`/`input`/
/// `gas`/`gas_price`/`nonce`) -> back-sell (token -> NATIVE BNB, bán TOÀN BỘ
/// `token_received`). `provider` dùng để fork qua `AlloyDB` (mọi
/// account/storage khác attacker đọc THẬT từ chain tại `fork_block`).
///
/// `victim.to` PHẢI là `Some` (router thật) — tx inject cũ (`to=None`) không
/// dùng được hàm này (đúng thiết kế, B chỉ chạy cho candidate đã qua gate
/// `venue_v2` ở `main.rs`, nơi `to` luôn biết).
pub async fn simulate_sandwich(
    provider: DynProvider,
    fork_block: u64,
    front_in: U256,
    token: Address,
    victim: &PendingTxRaw,
) -> Result<EvmSandwichOutcome, SimEvmError> {
    simulate_sandwich_quote(provider, fork_block, front_in, token, wbnb(), victim).await
}

/// Cụm `decision-data-24h` (mục 4) — bản TỔNG QUÁT theo QUOTE ASSET của
/// `simulate_sandwich`. `quote = WBNB` giữ nguyên hành vi cũ (front-buy bằng
/// BNB native qua `swapExactETHForTokens*`); `quote = USDT` (hoặc bất kỳ
/// ERC20 quote nào đã pin) dựng CẢ 2 chân bằng
/// `swapExactTokensForTokensSupportingFeeOnTransferTokens` và cấp vốn quote
/// cho attacker bằng cách GHI THẲNG storage `balanceOf` (kỹ thuật
/// sentinel-probe đã verify ở B4'.4 và đang dùng trong `measure_tax_on_fork`)
/// — BSC không có cơ chế "wrap" USDT.
///
/// Lý do cụm này cần: shadow mode 30 phút ở BAOCAO42 ký được 9 bundle, cả 9
/// đều quote USDT, và cả 9 dòng `shadow.sim` đều
/// `skipped:"usdt_not_supported_by_simulate_sandwich"` — tức KHÔNG có
/// `profit_sim` nào để đối chiếu với `profit_net` của đường nóng V2-math.
///
/// **Đơn vị `profit_wei`/`back_out` trả về là ĐƠN VỊ CỦA `quote`** (BNB khi
/// quote=WBNB, USDT khi quote=USDT) — caller phải log kèm quote, không được
/// mặc định coi là BNB.
pub async fn simulate_sandwich_quote(
    provider: DynProvider,
    fork_block: u64,
    front_in: U256,
    token: Address,
    quote: Address,
    victim: &PendingTxRaw,
) -> Result<EvmSandwichOutcome, SimEvmError> {
    simulate_sandwich_quote_topup(provider, fork_block, front_in, token, quote, victim, None).await
}

/// Cụm `verify-cluster-as-victim` (mục 2) — `simulate_sandwich_quote` + nạp
/// vốn quote cho VICTIM khi cần (xem `run_sandwich_quote_topup` để biết vì
/// sao ví burner của cụm đối thủ không thể replay nếu không nạp).
pub async fn simulate_sandwich_quote_topup(
    provider: DynProvider,
    fork_block: u64,
    front_in: U256,
    token: Address,
    quote: Address,
    victim: &PendingTxRaw,
    victim_topup: Option<U256>,
) -> Result<EvmSandwichOutcome, SimEvmError> {
    let (mut db, block_timestamp) = open_fork(provider, fork_block).await?;
    db.insert_account_info(attacker_address(), AccountInfo::from_balance(U256::from(FUND_BNB_WEI)));
    let mut evm = build_evm(db, fork_block, block_timestamp);
    run_sandwich_quote_topup(&mut evm, front_in, token, quote, victim, victim_topup)
}

/// Phần THỰC THI thuần (sync) của `simulate_sandwich` — tách ra để B4'.4 gọi
/// lặp lại trên CÙNG 1 `ForkEvm` đã warm (nhiều `front_in` khác nhau) mà
/// không fork/fetch lại. Logic y hệt `simulate_sandwich` cũ (chỉ code-move).
fn run_sandwich(evm: &mut ForkEvm, front_in: U256, token: Address, victim: &PendingTxRaw) -> Result<EvmSandwichOutcome, SimEvmError> {
    run_sandwich_quote(evm, front_in, token, wbnb(), victim)
}

/// Cụm `decision-data-24h` (mục 4) — `run_sandwich` tổng quát theo quote asset
/// (xem doc-comment `simulate_sandwich_quote`). `quote == WBNB` đi ĐÚNG đường
/// code cũ (native BNB), không đổi một byte hành vi nào.
fn run_sandwich_quote(
    evm: &mut ForkEvm,
    front_in: U256,
    token: Address,
    quote: Address,
    victim: &PendingTxRaw,
) -> Result<EvmSandwichOutcome, SimEvmError> {
    run_sandwich_quote_topup(evm, front_in, token, quote, victim, None)
}

/// Cụm `verify-cluster-as-victim` (mục 2) — bản có **nạp vốn cho VICTIM**.
///
/// # Vì sao cần
///
/// Đo thật ở phiên này: 5/5 victim quote-USDT mà bot ký bundle shadow đều
/// `TransferHelper: TRANSFER_FROM_FAILED` **ngay cả ở `front_in = 0`**, tức
/// không liên quan gì tới chân front của ta. Đối chiếu on-chain
/// (`eth_getBlockReceipts` của đúng block đào) cho thấy **cả 5 ví victim được
/// seed `0xB406…` chuyển USDT trong CHÍNH block đó, ở tx_index ngay TRƯỚC
/// giao dịch swap của họ**. `AlloyDB` đọc state ở CUỐI block, nên fork tại
/// `block − 1` thì ví victim CHƯA có đồng USDT nào — replay tất nhiên hỏng.
/// Đây KHÔNG phải "victim sẽ revert trong thực tế": trên chain cả 5 tx đều
/// `status = 0x1`.
///
/// Không có block nào trên chain mà (a) đã áp lệnh cấp vốn và (b) chưa áp
/// giao dịch victim — trạng thái đó chỉ tồn tại GIỮA hai tx trong cùng một
/// block. Nên muốn biết "kẹp cụm này lãi bao nhiêu" thì bắt buộc phải dựng
/// lại trạng thái đó: nạp cho victim đúng lượng quote họ sắp tiêu, bằng CÙNG
/// kỹ thuật ghi thẳng storage `balanceOf` đã dùng cho attacker.
///
/// `victim_topup = Some(amount_in)` bật cơ chế này; `None` giữ hành vi cũ
/// nguyên vẹn. Kết quả luôn tự khai báo qua `victim_quote_topped_up` —
/// **không được đọc một con số lãi có `victim_quote_topped_up = true` như thể
/// nó đã được xác nhận trên state thật của chain**.
fn run_sandwich_quote_topup(
    evm: &mut ForkEvm,
    front_in: U256,
    token: Address,
    quote: Address,
    victim: &PendingTxRaw,
    victim_topup: Option<U256>,
) -> Result<EvmSandwichOutcome, SimEvmError> {
    let victim_to = victim.to.ok_or_else(|| SimEvmError::Fork("victim.to=None, khong the replay (thieu router that)".to_string()))?;
    let attacker = attacker_address();
    let is_native_quote = quote == wbnb();
    // Cum `truth-victim-ok-and-memleak` (muc 1) — `front_in = 0` la che do
    // CHUNG NGHIEM: replay MOT MINH victim tren fork, KHONG co chan front/back
    // nao cua ta. Day la bien the (a) cua thi nghiem phan dinh victim_ok=false:
    // victim van hong khi front_in=0 => do state cua chinh vi victim tai block
    // fork; victim song => chinh chan front cua ta giet victim.
    // Khong the di duong code thuong voi front_in=0 vi router revert
    // INSUFFICIENT_INPUT_AMOUNT (loi cua TA, khong phai cau tra loi can tim).
    let victim_only = front_in.is_zero();

    // Quote ERC20 (USDT): cap von quote cho attacker bang cach ghi thang
    // storage `balanceOf` (khong co co che wrap) + approve router 1 lan.
    // `fund_quote` = 2x front_in de sau khi tieu `front_in` van con du lam moc
    // doi chieu; phan CHUA TIEU (`fund_quote - front_in`) duoc tru ra khi tinh
    // `back_out`, nen so von cap KHONG the lam sai lech loi/lo.
    let fund_quote = if is_native_quote || victim_only { U256::ZERO } else { front_in.saturating_mul(U256::from(2u64)) };
    if !is_native_quote && !victim_only {
        let slot = probe_erc20_balance_slot(evm, quote, attacker)?;
        set_erc20_balance(evm, quote, attacker, slot, fund_quote)?;
        let approve_quote = IERC20Min::approveCall { spender: router(), amount: U256::MAX }.abi_encode();
        let tx = TxEnv::builder()
            .caller(attacker)
            .kind(TxKind::Call(quote))
            .gas_limit(200_000)
            .gas_price(0)
            .nonce(0)
            .chain_id(Some(56))
            .data(Bytes::from(approve_quote))
            .build_fill();
        let r = evm.transact_commit(tx).map_err(|e| SimEvmError::Exec(format!("approve quote: {e:?}")))?;
        if !r.is_success() {
            return Err(SimEvmError::Revert(format!("approve quote revert/halt: {r:?}")));
        }
    }

    // ---- buy-tax estimate NGAY TRUOC front-buy (state fork nguyen ven) ----
    let expected_token_out = if victim_only { None } else { quote_amounts_out(evm, quote, token, front_in).ok().flatten() };

    // ---- front-buy: quote -> token ----
    let front_calldata = if is_native_quote {
        IPancakeV2RouterFeeOnTransfer::swapExactETHForTokensSupportingFeeOnTransferTokensCall {
            amountOutMin: U256::ZERO,
            path: vec![quote, token],
            to: attacker,
            deadline: U256::from(DEADLINE_MAX),
        }
        .abi_encode()
    } else {
        IPancakeV2RouterFeeOnTransfer::swapExactTokensForTokensSupportingFeeOnTransferTokensCall {
            amountIn: front_in,
            amountOutMin: U256::ZERO,
            path: vec![quote, token],
            to: attacker,
            deadline: U256::from(DEADLINE_MAX),
        }
        .abi_encode()
    };
    let front_tx_opt = if victim_only { None } else { Some(TxEnv::builder()
        .caller(attacker)
        .kind(TxKind::Call(router()))
        .value(if is_native_quote { front_in } else { U256::ZERO })
        .gas_limit(3_000_000)
        .gas_price(0)
        .nonce(0)
        .chain_id(Some(56))
        .data(Bytes::from(front_calldata))
        .build_fill()) };
    if let Some(front_tx) = front_tx_opt {
        let front_result = evm.transact_commit(front_tx).map_err(|e| SimEvmError::Exec(format!("front-buy: {e:?}")))?;
        if !front_result.is_success() {
            return Err(SimEvmError::Revert(format!("front-buy revert/halt: {front_result:?}")));
        }
    }

    let token_received = if victim_only { U256::ZERO } else { read_balance(evm, token, attacker)? };
    let buy_tax_bps = expected_token_out.and_then(|expected| shortfall_bps(expected, token_received));

    // ---- approve (max) truoc khi ban lai ----
    let approve_calldata =
        IERC20Min::approveCall { spender: router(), amount: U256::MAX }.abi_encode();
    let approve_tx = TxEnv::builder()
        .caller(attacker)
        .kind(TxKind::Call(token))
        .gas_limit(200_000)
        .gas_price(0)
        .nonce(0)
        .chain_id(Some(56))
        .data(Bytes::from(approve_calldata))
        .build_fill();
    if !victim_only {
        let approve_result = evm.transact_commit(approve_tx).map_err(|e| SimEvmError::Exec(format!("approve: {e:?}")))?;
        if !approve_result.is_success() {
            return Err(SimEvmError::Revert(format!("approve revert/halt: {approve_result:?}")));
        }
    }

    // ---- victim: replay DUNG calldata/from/value/gas/nonce that ----
    // Cum `truth-victim-ok-and-memleak` (muc 1): do `balanceOf(token, victim.from)`
    // NGAY TRUOC va NGAY SAU tx victim -> `victim_out` THAT trong replay, so
    // truc tiep duoc voi `victim_out` cua V2-math va `amountOutMin` calldata.
    // ---- Cụm `verify-cluster-as-victim` (mục 2): nạp vốn cho victim ----
    let victim_quote_balance_before = read_balance(evm, quote, victim.from).unwrap_or(U256::ZERO);
    let victim_quote_allowance = read_allowance(evm, quote, victim.from, victim_to).unwrap_or(U256::ZERO);
    let mut victim_quote_topped_up = false;
    if let Some(need) = victim_topup {
        if !is_native_quote && !need.is_zero() && victim_quote_balance_before < need {
            let slot = probe_erc20_balance_slot(evm, quote, victim.from)?;
            set_erc20_balance(evm, quote, victim.from, slot, need)?;
            victim_quote_topped_up = true;
        }
    }
    let victim_token_before = read_balance(evm, token, victim.from).unwrap_or(U256::ZERO);
    let victim_gas_price = u128::try_from(victim.gas_price).unwrap_or(0);
    let victim_tx = TxEnv::builder()
        .caller(victim.from)
        .kind(TxKind::Call(victim_to))
        .value(victim.value)
        .gas_limit(victim.gas.max(200_000))
        .gas_price(victim_gas_price)
        .nonce(victim.nonce)
        .chain_id(Some(56))
        .data(Bytes::from(victim.input.clone()))
        .build_fill();
    let victim_result = evm.transact_commit(victim_tx).map_err(|e| SimEvmError::Exec(format!("victim replay: {e:?}")))?;
    let victim_success = victim_result.is_success();
    let victim_revert_reason = revert_reason(&victim_result);
    let victim_out = if victim_success {
        read_balance(evm, token, victim.from).unwrap_or(U256::ZERO).saturating_sub(victim_token_before)
    } else {
        U256::ZERO
    };

    // ---- sell-tax estimate NGAY TRUOC back-sell (state da qua victim) ----
    let expected_bnb_out =
        if victim_only { None } else { quote_amounts_out(evm, token, quote, token_received).ok().flatten() };

    // ---- back-sell: token (TOAN BO da nhan) -> quote ----
    let back_calldata = if is_native_quote {
        IPancakeV2RouterFeeOnTransfer::swapExactTokensForETHSupportingFeeOnTransferTokensCall {
            amountIn: token_received,
            amountOutMin: U256::ZERO,
            path: vec![token, quote],
            to: attacker,
            deadline: U256::from(DEADLINE_MAX),
        }
        .abi_encode()
    } else {
        IPancakeV2RouterFeeOnTransfer::swapExactTokensForTokensSupportingFeeOnTransferTokensCall {
            amountIn: token_received,
            amountOutMin: U256::ZERO,
            path: vec![token, quote],
            to: attacker,
            deadline: U256::from(DEADLINE_MAX),
        }
        .abi_encode()
    };
    let back_tx_opt = if victim_only { None } else { Some(TxEnv::builder()
        .caller(attacker)
        .kind(TxKind::Call(router()))
        .gas_limit(3_000_000)
        .gas_price(0)
        .nonce(0)
        .chain_id(Some(56))
        .data(Bytes::from(back_calldata))
        .build_fill()) };
    if let Some(back_tx) = back_tx_opt {
        let back_result = evm.transact_commit(back_tx).map_err(|e| SimEvmError::Exec(format!("back-sell: {e:?}")))?;
        if !back_result.is_success() {
            return Err(SimEvmError::Revert(format!("back-sell revert/halt: {back_result:?}")));
        }
    }

    let back_out = if victim_only {
        // Khong co chan nao cua ta chay -> khong co dong tien nao cua ta.
        U256::ZERO
    } else if is_native_quote {
        let final_native_balance = evm
            .ctx
            .db_mut()
            .basic(attacker)
            .map_err(|e| SimEvmError::Exec(format!("doc so du cuoi that bai: {e:?}")))?
            .map(|info| info.balance)
            .unwrap_or(U256::ZERO);
        // gas_price=0 cho MOI tx attacker (front/approve/back) -> native balance
        // chi doi vi swap that (khong lan gas): final = FUND - front_in + back_out.
        let fund = U256::from(FUND_BNB_WEI);
        (final_native_balance + front_in).saturating_sub(fund)
    } else {
        // Quote ERC20: doc `balanceOf(quote, attacker)` va tru phan von CHUA
        // TIEU (`fund_quote - front_in`) - gas khong tra bang quote nen so nay
        // chi phan anh 2 chan swap.
        let final_quote_balance = read_balance(evm, quote, attacker)?;
        final_quote_balance.saturating_sub(fund_quote.saturating_sub(front_in))
    };

    let front_i = i128::try_from(front_in).map_err(|_| SimEvmError::Decode("front_in vuot i128".to_string()))?;
    let back_i = i128::try_from(back_out).map_err(|_| SimEvmError::Decode("back_out vuot i128".to_string()))?;
    let sell_tax_bps = expected_bnb_out.and_then(|expected| shortfall_bps(expected, back_out));

    Ok(EvmSandwichOutcome {
        front_in,
        token_received,
        back_out,
        profit_wei: back_i - front_i,
        victim_success,
        victim_revert_reason,
        victim_out,
        buy_tax_bps,
        sell_tax_bps,
        victim_quote_balance_before,
        victim_quote_allowance,
        victim_quote_topped_up,
    })
}

/// `expected` (AMM-math thuần, `getAmountsOut`) so với `actual` (đo thật qua
/// `balanceOf`/số dư native) -> bps hao hụt (`0` nếu `actual >= expected` —
/// KHÔNG âm, tax không thể "âm" theo mô hình này). `None` nếu `expected=0`.
fn shortfall_bps(expected: U256, actual: U256) -> Option<u32> {
    if actual >= expected {
        return Some(0);
    }
    u256_to_u128_bps(expected - actual, expected)
}

/// `caller` LUÔN là `attacker_address()` (EOA giả cố định) — KHÔNG PHẢI
/// `owner` (tham số ABI của `balanceOf`, có thể là 1 CONTRACT như pair V2).
/// Phát hiện thật (B4'.4 lần chạy đầu, khi thêm `probe_erc20_balance_slot`
/// cho `pair`): dùng `owner` làm `caller` vi phạm EIP-3607 ("không cho phép
/// tx từ 1 address có bytecode") — revm trả lỗi `RejectCallerWithCode` khi
/// `owner` là contract thật. `balanceOf` là view function, người gọi là ai
/// không ảnh hưởng kết quả, nên tách hẳn caller khỏi owner để đọc được
/// balance của BẤT KỲ địa chỉ nào (kể cả contract).
fn read_balance(evm: &mut ForkEvm, token: Address, owner: Address) -> Result<U256, SimEvmError> {
    let calldata = IERC20Min::balanceOfCall { owner }.abi_encode();
    let tx = TxEnv::builder()
        .caller(attacker_address())
        .kind(TxKind::Call(token))
        .gas_limit(200_000)
        .gas_price(0)
        .nonce(0)
        .chain_id(Some(56))
        .data(Bytes::from(calldata))
        .build_fill();
    let result = evm.transact(tx).map_err(|e| SimEvmError::Exec(format!("balanceOf: {e:?}")))?;
    let output = result.result.output().ok_or_else(|| SimEvmError::Decode("balanceOf khong co output".to_string()))?;
    IERC20Min::balanceOfCall::abi_decode_returns(output).map_err(|e| SimEvmError::Decode(format!("balanceOf decode: {e}")))
}

/// Cụm `verify-cluster-as-victim` (mục 2) — `allowance(owner -> spender)` tại
/// state hiện tại của fork. Dùng để PHÂN ĐỊNH `TRANSFER_FROM_FAILED`: thiếu
/// số dư (nạp được, xem `run_sandwich_quote_topup`) hay thiếu allowance (KHÔNG
/// nạp — allowance là quyết định thật của victim, giả nó đi là bịa).
fn read_allowance(evm: &mut ForkEvm, token: Address, owner: Address, spender: Address) -> Result<U256, SimEvmError> {
    let calldata = IERC20Min::allowanceCall { owner, spender }.abi_encode();
    let tx = TxEnv::builder()
        .caller(attacker_address())
        .kind(TxKind::Call(token))
        .gas_limit(200_000)
        .gas_price(0)
        .nonce(0)
        .chain_id(Some(56))
        .data(Bytes::from(calldata))
        .build_fill();
    let result = evm.transact(tx).map_err(|e| SimEvmError::Exec(format!("allowance: {e:?}")))?;
    let output = result.result.output().ok_or_else(|| SimEvmError::Decode("allowance khong co output".to_string()))?;
    IERC20Min::allowanceCall::abi_decode_returns(output).map_err(|e| SimEvmError::Decode(format!("allowance decode: {e}")))
}

/// `getAmountsOut` (AMM-math THUẦN, không tax) tại ĐÚNG state hiện tại —
/// gọi qua `evm.transact` (KHÔNG commit, không đổi state EVM) nên có thể gọi
/// TRƯỚC 1 tx thay đổi state mà không ảnh hưởng gì. `None` nếu call lỗi/decode
/// lỗi/`amount_in=0` (không đo được, KHÔNG bịa số 0 giả).
fn quote_amounts_out(evm: &mut ForkEvm, from_token: Address, to_token: Address, amount_in: U256) -> Result<Option<U256>, SimEvmError> {
    if amount_in.is_zero() {
        return Ok(None);
    }
    let calldata =
        IPancakeV2RouterFeeOnTransfer::getAmountsOutCall { amountIn: amount_in, path: vec![from_token, to_token] }
            .abi_encode();
    let tx = TxEnv::builder()
        .caller(attacker_address())
        .kind(TxKind::Call(router()))
        .gas_limit(300_000)
        .gas_price(0)
        .nonce(0)
        .chain_id(Some(56))
        .data(Bytes::from(calldata))
        .build_fill();
    let result = match evm.transact(tx) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    let output = match result.result.output() {
        Some(o) => o,
        None => return Ok(None),
    };
    let amounts = match IPancakeV2RouterFeeOnTransfer::getAmountsOutCall::abi_decode_returns(output) {
        Ok(a) => a,
        Err(_) => return Ok(None),
    };
    Ok(amounts.last().copied())
}

// ============================================================================
// Cụm `evm-validate-wire-tax` (B4') — validate TỰ ĐỘNG (không cần BscScan):
// B4'.3(a) dự đoán victim đơn lẻ, B4'.3(b) replay sandwich THẬT đã xảy ra,
// B4'.4 ternary search front_in bằng EVM trên CÙNG 1 fork đã warm.
// ============================================================================

/// Gom logic quét `txpool_content` tìm candidate V2 WBNB-buy THẬT đang chờ
/// trong mempool — dùng CHUNG cho nhiều test `#[ignore]` (trước phiên này
/// logic này chỉ có 1 bản riêng bên trong
/// `real_rpc_sim_evm_matches_sim_v2_when_zero_tax`).
#[allow(dead_code)] // dung trong test #[ignore] (real_rpc_*)
pub(crate) async fn poll_wbnb_v2_candidates(
    provider: &DynProvider,
    v2_router: Address,
    min_count: usize,
    max_polls: u32,
) -> Vec<(PendingTxRaw, Address)> {
    use alloy::consensus::Transaction as _;

    #[derive(Debug, serde::Deserialize)]
    struct TxpoolContentPendingOnly {
        #[serde(default)]
        pending: std::collections::BTreeMap<String, std::collections::BTreeMap<String, alloy::rpc::types::eth::Transaction>>,
    }

    let mut candidates: Vec<(PendingTxRaw, Address)> = Vec::new();
    let mut seen_hashes: std::collections::HashSet<alloy::primitives::B256> = std::collections::HashSet::new();
    for attempt in 0..max_polls {
        let content: TxpoolContentPendingOnly = match provider
            .raw_request("txpool_content".into(), alloy::rpc::client::NoParams::default())
            .await
        {
            Ok(c) => c,
            Err(e) => {
                println!("poll_wbnb_v2_candidates #{attempt}: txpool_content loi ({e}), thu lai");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };
        for by_nonce in content.pending.values() {
            for tx in by_nonce.values() {
                if tx.to() != Some(v2_router) {
                    continue;
                }
                let decoded = match crate::decoder::decode_swap_calldata(tx.input(), tx.value()) {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                if decoded.path.token_a != wbnb() {
                    continue;
                }
                let token = match decoded.token() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                if decoded.amount_in < U256::from(10_000_000_000_000_000u64) {
                    continue; // qua nho (<0.01 BNB), bo qua de co price impact do duoc
                }
                let raw = crate::transport::pending_tx_from_rpc(tx);
                if !seen_hashes.insert(raw.hash) {
                    continue; // da thay tx nay o lan poll truoc, bo qua trung lap
                }
                candidates.push((raw, token));
            }
        }
        println!("poll_wbnb_v2_candidates #{attempt}: tich luy {} candidate (distinct)", candidates.len());
        if candidates.len() >= min_count {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    candidates
}

// ============================================================================
// Cụm `evm-validate-fixed-then-wire` (B4'') — 3 sửa so với B4' (BAOCAO32):
// (1) chọn mẫu có ý nghĩa (`is_meaningful_candidate`), (2) lấy `victim_out_real`
// từ `eth_getTransactionReceipt` của CHÍNH victim thay vì `eth_getLogs` dải
// rộng, (3) RPC failover + backoff khi gặp `-32005`.
// ============================================================================

/// Chữ ký `Swap` của `UniswapV2Pair` (PancakeSwap V2 fork y hệt) — dùng cho
/// CẢ 2 đường: log THẬT trong `eth_getTransactionReceipt` và log EVM sinh ra
/// khi replay victim trong revm. Cùng 1 hằng số nên 2 đường không thể lệch
/// nhau vì gõ sai chữ ký.
pub const SWAP_EVENT_SIG: &str = "Swap(address,uint256,uint256,uint256,uint256,address)";

pub fn swap_topic0() -> alloy::primitives::B256 {
    alloy::primitives::keccak256(SWAP_EVENT_SIG.as_bytes())
}

/// Đọc `amountOut` của ĐÚNG token cần quan tâm từ `data` (phần KHÔNG indexed)
/// của 1 log `Swap` V2. Layout cố định theo `UniswapV2Pair.sol`:
/// `amount0In(32) | amount1In(32) | amount0Out(32) | amount1Out(32)` — `sender`
/// và `to` là indexed nên KHÔNG nằm trong `data`. `None` nếu `data` không đủ
/// 128 byte (log không phải Swap V2 chuẩn — KHÔNG đoán).
pub fn decode_v2_swap_amount_out(data: &[u8], token_is_token0: bool) -> Option<U256> {
    if data.len() < 128 {
        return None;
    }
    let off = if token_is_token0 { 64 } else { 96 };
    Some(U256::from_be_slice(&data[off..off + 32]))
}

/// B4''.1 — chỉ nhận candidate ĐỦ LỚN để phép so sánh có ý nghĩa: `victim_in`
/// ≥ 0.05 BNB VÀ `victim_in / reserve_quote` ≥ 0.02% (2 bps). Lý do (phát
/// hiện thật BAOCAO32/B4'.2): victim quá nhỏ so pool → `search_max_front_in`
/// hội tụ về `front_in` 1-2 wei, `back_out` bị floor về cùng 1 wei ở CẢ 2
/// đường tính, `profit=-1` — số liệu đó KHÔNG phân biệt được sim đúng hay
/// sai, nên không dùng để validate. Trả `Err(lý do)` để caller LOG được vì
/// sao loại (đúng lệnh "Log lý do loại candidate nhỏ").
pub fn is_meaningful_candidate(victim_in: U256, reserve_quote: U256) -> Result<(), String> {
    const MIN_VICTIM_IN_WEI: u128 = 50_000_000_000_000_000; // 0.05 BNB
    const MIN_IMPACT_BPS: u64 = 2; // 0.02%
    if victim_in < U256::from(MIN_VICTIM_IN_WEI) {
        return Err(format!("victim_in={victim_in} < 0.05 BNB ({MIN_VICTIM_IN_WEI} wei)"));
    }
    if reserve_quote.is_zero() {
        return Err("reserve_quote=0".to_string());
    }
    let impact_bps = (victim_in.saturating_mul(U256::from(10_000u64))) / reserve_quote;
    if impact_bps < U256::from(MIN_IMPACT_BPS) {
        return Err(format!(
            "impact={impact_bps}bps < {MIN_IMPACT_BPS}bps (victim_in={victim_in} / reserve_quote={reserve_quote})"
        ));
    }
    Ok(())
}

/// Danh sách URL RPC dùng cho các test `real_rpc_*` — đọc `BSC_HTTP_VALIDATE`
/// (Chủ cấp, KHÔNG ghi vào file repo) rồi tới các URL dự phòng Chủ đã dán.
/// Rơi về `bsc-dataseed` (tiền lệ mọi phiên trước) khi không có env nào.
/// KHÔNG in giá trị env ra log (có thể chứa key nếu Chủ đổi sau này).
pub fn validate_rpc_urls() -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    if let Ok(v) = std::env::var("BSC_HTTP_VALIDATE") {
        for part in v.split(',') {
            let p = part.trim();
            if !p.is_empty() {
                urls.push(p.to_string());
            }
        }
    }
    for extra in ["https://bsc.blockrazor.xyz", "https://bsc-dataseed1.defibit.io"] {
        if !urls.iter().any(|u| u == extra) {
            urls.push(extra.to_string());
        }
    }
    if urls.is_empty() {
        urls.push("https://bsc-dataseed.binance.org/".to_string());
    }
    urls
}

/// B4''.4 — đếm lỗi `-32005` ("limit exceeded") THẬT gặp trong 1 lần chạy.
/// Bắt buộc phải dán con số này trong BAOCAO để phân biệt "dữ liệu hiếm" với
/// "RPC không quét được" (điều kiện của B4''.5).
#[derive(Debug, Default)]
pub struct RpcErrorStats {
    pub limit_exceeded_32005: std::sync::atomic::AtomicU32,
    pub other_errors: std::sync::atomic::AtomicU32,
}

impl RpcErrorStats {
    pub fn record(&self, err: &str) {
        use std::sync::atomic::Ordering;
        if err.contains("-32005") || err.contains("limit exceeded") {
            self.limit_exceeded_32005.fetch_add(1, Ordering::Relaxed);
        } else {
            self.other_errors.fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn n_32005(&self) -> u32 {
        self.limit_exceeded_32005.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub fn n_other(&self) -> u32 {
        self.other_errors.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// B4''.2 — gom victim candidate từ các block ĐÃ MINED gần nhất, thay vì từ
/// `txpool_content`.
///
/// **Vì sao đổi nguồn mẫu (phát hiện THẬT phiên này, không phải tiện tay đổi)**:
/// đếm `txpool_content` thật của `bsc-rpc.publicnode.com` nhiều lần (dán ở
/// BAOCAO33 ô 5) cho thấy toàn bộ tx pending đi vào V2 Router đã pin có
/// `value` cỡ `0.0004`–`0.004` BNB — dưới XA ngưỡng `0.05` BNB mà B4''.1 đòi.
/// Nghĩa là nguồn mempool của node này KHÔNG THỂ sinh ra mẫu đủ lớn, và đó là
/// đặc tính DỮ LIỆU (victim V2 thật trên BSC phần lớn rất nhỏ), không phải lỗi
/// RPC — số `-32005` của các lần chạy đó ĐỀU BẰNG 0.
///
/// Block đã mined có lưu lượng lớn hơn nhiều bậc nên gom được mẫu đủ lớn.
/// Ngữ nghĩa validate KHÔNG đổi: vẫn là "fork tại state block CHA, replay
/// ĐÚNG tx victim, so output EVM với output THẬT" — đúng cơ chế bot dùng
/// trong sản xuất (fork block hiện tại, replay tx đang chờ). Điều kiện cô lập
/// (`pair` chỉ có ĐÚNG 1 `Swap` trong block đó) đảm bảo state tại block cha
/// CHÍNH LÀ state victim thật sự gặp — không có swap nào khác chen vào trước
/// nó trong cùng block.
#[allow(dead_code)] // dung trong test #[ignore] (real_rpc_*)
pub(crate) async fn collect_mined_v2_buy_candidates(
    provider: &DynProvider,
    from_block: u64,
    to_block: u64,
    routers: &[Address],
    min_value_wei: U256,
    max_count: usize,
    stats: &RpcErrorStats,
) -> Vec<(PendingTxRaw, Address, u64)> {
    use alloy::consensus::Transaction as _;
    use alloy::eips::BlockNumberOrTag;

    let mut out: Vec<(PendingTxRaw, Address, u64)> = Vec::new();
    for bn in (from_block..=to_block).rev() {
        let block = match provider.get_block_by_number(BlockNumberOrTag::Number(bn)).full().await {
            Ok(Some(b)) => b,
            Ok(None) => continue,
            Err(e) => {
                stats.record(&e.to_string());
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };
        for tx in block.transactions.txns() {
            let Some(to) = tx.to() else { continue };
            if !routers.contains(&to) {
                continue;
            }
            let Ok(decoded) = crate::decoder::decode_swap_calldata(tx.input(), tx.value()) else {
                continue;
            };
            if decoded.path.token_a != wbnb() {
                continue;
            }
            let Ok(token) = decoded.token() else { continue };
            if decoded.amount_in < min_value_wei {
                continue;
            }
            out.push((crate::transport::pending_tx_from_rpc(tx), token, bn));
            if out.len() >= max_count {
                return out;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    out
}

/// B4'.3(a) / B4''.2 — dự đoán 1 victim ĐƠN LẺ (không front/back), so với thật
/// sau khi mined. Fork tại `fork_block`, đo `balanceOf(victim.from, token)`
/// TRƯỚC/SAU khi replay ĐÚNG tx victim — không cần front/back nên không cần
/// `PoolReserves`.
pub async fn predict_victim_token_delta(
    provider: DynProvider,
    fork_block: u64,
    victim: &PendingTxRaw,
    token: Address,
) -> Result<(bool, U256), SimEvmError> {
    let victim_to = victim.to.ok_or_else(|| SimEvmError::Fork("victim.to=None, khong the replay".to_string()))?;
    let (db, block_timestamp) = open_fork(provider, fork_block).await?;
    let mut evm = build_evm(db, fork_block, block_timestamp);

    let before = read_balance(&mut evm, token, victim.from)?;
    let victim_gas_price = u128::try_from(victim.gas_price).unwrap_or(0);
    let victim_tx = TxEnv::builder()
        .caller(victim.from)
        .kind(TxKind::Call(victim_to))
        .value(victim.value)
        .gas_limit(victim.gas.max(200_000))
        .gas_price(victim_gas_price)
        .nonce(victim.nonce)
        .chain_id(Some(56))
        .data(Bytes::from(victim.input.clone()))
        .build_fill();
    let result = evm.transact_commit(victim_tx).map_err(|e| SimEvmError::Exec(format!("victim replay (predict): {e:?}")))?;
    let success = result.is_success();
    let after = read_balance(&mut evm, token, victim.from)?;
    Ok((success, after.saturating_sub(before)))
}

// ============================================================================
// Cụm `evm-validate-fixed-then-wire` (B3.2) — FORK CACHE THEO BLOCK
// ============================================================================

/// Fork dùng chung cho MỌI tx trong CÙNG 1 block — mục tiêu: chỉ trả giá
/// fork/fetch remote 1 lần/block thay vì 1 lần/tx.
///
/// **Vì sao snapshot `accounts` mà GIỮ `contracts`** (quyết định kỹ thuật, ghi
/// rõ thay vì im lặng): `revm::database::CacheDB` có `pub cache: Cache` gồm
/// `accounts` (info + storage, BỊ tx ghi đè) và `contracts` (bytecode theo
/// code-hash) + `block_hashes`. `WrapDatabaseAsync` KHÔNG `derive(Clone)` nên
/// KHÔNG clone được cả `ForkDb`; nhưng `Cache` thì clone được vì `CacheDB`
/// `derive(Clone)`.
/// - `accounts` PHẢI khôi phục về ảnh chụp trước tx, nếu không tx sau sẽ thấy
///   state do tx trước ghi (2 victim cùng 1 pair trong 1 block sẽ ra số SAI).
/// - `contracts`/`block_hashes` GIỮ LẠI tích luỹ: tại 1 block fork CỐ ĐỊNH,
///   bytecode là BẤT BIẾN — giữ lại không thể gây sai, mà đây chính là phần
///   fetch nặng nhất (bytecode router/WBNB/token/pair dùng chung giữa các tx).
///
/// Đây là đánh đổi có chủ đích: an toàn tuyệt đối về ĐÚNG/SAI, chỉ giữ lại
/// phần cache chắc chắn bất biến. Số ms/tx thực đo được dán ở BAOCAO33 ô 5 —
/// KHÔNG khẳng định trước là đạt mốc ≤50ms.
pub struct BlockForkCache {
    block: u64,
    block_timestamp: u64,
    evm: ForkEvm,
    /// Ảnh chụp `accounts` ngay sau khi fork (rỗng) — khôi phục trước MỖI tx.
    base_accounts: std::collections::HashMap<Address, revm::database::DbAccount, alloy::primitives::map::FbBuildHasher<20>>,
}

impl BlockForkCache {
    pub async fn open(provider: DynProvider, block: u64) -> Result<Self, SimEvmError> {
        let (db, block_timestamp) = open_fork(provider, block).await?;
        let base_accounts = db.cache.accounts.clone();
        let evm = build_evm(db, block, block_timestamp);
        Ok(Self { block, block_timestamp, evm, base_accounts })
    }

    pub fn block(&self) -> u64 {
        self.block
    }

    pub fn block_timestamp(&self) -> u64 {
        self.block_timestamp
    }

    /// Khôi phục `accounts` về ảnh chụp gốc (giữ `contracts`/`block_hashes` đã
    /// warm) rồi cấp lại số dư native cho attacker — trạng thái SẠCH y hệt
    /// một fork mới, nhưng không tốn 1 vòng RPC nào cho bytecode đã có.
    fn reset(&mut self) {
        self.evm.ctx.db_mut().cache.accounts = self.base_accounts.clone();
        self.evm
            .ctx
            .db_mut()
            .insert_account_info(attacker_address(), AccountInfo::from_balance(U256::from(FUND_BNB_WEI)));
    }

    /// Chạy 1 sandwich trên fork dùng chung. Trả kèm thời gian (ms) để đo
    /// warm-cache hiệu quả tới đâu.
    pub fn run_sandwich_cached(
        &mut self,
        front_in: U256,
        token: Address,
        victim: &PendingTxRaw,
    ) -> (Result<EvmSandwichOutcome, SimEvmError>, f64) {
        let t0 = std::time::Instant::now();
        self.reset();
        let r = run_sandwich(&mut self.evm, front_in, token, victim);
        (r, t0.elapsed().as_secs_f64() * 1000.0)
    }

    /// Cụm `truth-victim-ok-and-memleak` (mục 1) — bản quote-aware của
    /// `run_sandwich_cached`, để thí nghiệm 3 biến thể `front_in` chạy trên
    /// ĐÚNG MỘT fork (cùng block, cùng state gốc) thay vì 5 fork khác nhau —
    /// nếu mỗi biến thể tự fork riêng thì block/state đã trôi và kết quả
    /// KHÔNG so sánh được với nhau, tức thí nghiệm mất ý nghĩa.
    pub fn run_sandwich_quote_cached(
        &mut self,
        front_in: U256,
        token: Address,
        quote: Address,
        victim: &PendingTxRaw,
    ) -> (Result<EvmSandwichOutcome, SimEvmError>, f64) {
        self.run_sandwich_quote_cached_topup(front_in, token, quote, victim, None)
    }

    /// Cụm `verify-cluster-as-victim` (mục 2) — bản có nạp vốn cho victim, để
    /// THANG PHÂN ĐỊNH (`diagnose_victim_ok`) dùng CÙNG điều kiện với
    /// `shadow.sim`; nếu không, thang sẽ luôn trả `state_fork_sai` cho ví
    /// burner của cụm đối thủ và không phân định được gì thêm.
    pub fn run_sandwich_quote_cached_topup(
        &mut self,
        front_in: U256,
        token: Address,
        quote: Address,
        victim: &PendingTxRaw,
        victim_topup: Option<U256>,
    ) -> (Result<EvmSandwichOutcome, SimEvmError>, f64) {
        let t0 = std::time::Instant::now();
        self.reset();
        let r = run_sandwich_quote_topup(&mut self.evm, front_in, token, quote, victim, victim_topup);
        (r, t0.elapsed().as_secs_f64() * 1000.0)
    }

    /// C1 — đo tax mua/bán bằng EVM thật trên fork đã warm (xem `tax.rs`).
    pub fn measure_tax_cached(
        &mut self,
        token: Address,
        quote: Address,
        probe_in: U256,
    ) -> (Result<EvmTaxMeasurement, SimEvmError>, f64) {
        let t0 = std::time::Instant::now();
        self.reset();
        let r = measure_tax_on_fork(&mut self.evm, token, quote, probe_in);
        (r, t0.elapsed().as_secs_f64() * 1000.0)
    }
}

// ============================================================================
// Cụm `truth-victim-ok-and-memleak` (mục 1) — THÍ NGHIỆM PHÂN ĐỊNH `victim_ok=false`
// ============================================================================

/// Một biến thể `front_in` trong thí nghiệm phân định — xem
/// `diagnose_victim_ok`.
#[derive(Debug, Clone)]
pub struct VictimDiagRow {
    /// Nhãn biến thể theo đúng lệnh: `a:front=0`, `b:front=v2`, `c:50%`,
    /// `c:25%`, `c:10%`.
    pub label: String,
    pub front_in: U256,
    pub victim_ok: bool,
    /// Token victim nhận được TRONG replay này (0 khi revert).
    pub victim_out: U256,
    pub victim_revert_reason: Option<String>,
    /// Lãi/lỗ EVM ở mức `front_in` này, ĐƠN VỊ CỦA QUOTE. `0` ở biến thể (a)
    /// vì không có chân nào của ta chạy.
    pub profit_wei: i128,
    /// Lỗi ở tầng sim (fork/revert chân của TA) — khác hẳn `victim_ok=false`.
    pub err: Option<String>,
    pub ms: f64,
}

/// Kết quả thí nghiệm + KẾT LUẬN tự động bằng số (không để người đọc tự đoán).
#[derive(Debug, Clone)]
pub struct VictimDiagReport {
    pub fork_block: u64,
    pub rows: Vec<VictimDiagRow>,
    /// `front_in` LỚN NHẤT trong các biến thể đã thử mà victim VẪN SỐNG
    /// (`None` = không mức nào sống, kể cả `front_in=0`).
    pub max_front_victim_alive: Option<U256>,
    /// Một trong: `"front_giet_victim"` (a sống, b chết),
    /// `"state_fork_sai"` (a chết luôn — victim hỏng dù ta không đụng gì),
    /// `"khong_tai_hien"` (b sống — lần chạy này victim không hỏng),
    /// `"sim_error"` (không chạy được).
    pub verdict: &'static str,
}

/// Cụm `truth-victim-ok-and-memleak` (mục 1) — CÂU HỎI SỐ 1 của lệnh: khi
/// `shadow.sim` báo `victim_ok=false`, đó là do **chân front của ta giết
/// victim** hay do **state fork sai** (ví victim không có tiền/allowance tại
/// block fork)?
///
/// Thí nghiệm chạy 5 biến thể `front_in` trên **ĐÚNG MỘT fork** (
/// `BlockForkCache`, `reset()` giữa mỗi lần nên state gốc y hệt nhau):
/// - **(a)** `front_in = 0` — replay MỘT MÌNH victim, ta không đụng gì.
/// - **(b)** `front_in` = đúng giá trị V2-math đã chọn (mức đang bị nghi).
/// - **(c)** `50% / 25% / 10%` của (b) — tìm NGƯỠNG victim bắt đầu sống.
///
/// Phân định (`verdict`):
/// - (a) victim SỐNG mà (b) CHẾT ⇒ `front_giet_victim` — lỗi ở ta, phải hạ
///   `front_in` (mục 2 của lệnh).
/// - (a) victim CHẾT ⇒ `state_fork_sai` — không liên quan chân front, phải
///   sửa fork (nonce/balance/allowance tại block pin).
/// - (b) victim SỐNG ⇒ `khong_tai_hien` — lần chạy này không tái hiện được.
///
/// KHÔNG chặn đường nóng: hàm `async`, gọi từ task nền của shadow.
pub async fn diagnose_victim_ok(
    provider: DynProvider,
    fork_block: u64,
    token: Address,
    quote: Address,
    victim: &PendingTxRaw,
    front_in_v2: U256,
) -> Result<VictimDiagReport, SimEvmError> {
    diagnose_victim_ok_variants(provider, fork_block, token, quote, victim, &victim_diag_ladder(front_in_v2), None).await
}

/// Thang 5 biến thể MẶC ĐỊNH (a/b/c của lệnh) quanh 1 mức `front_in`.
pub fn victim_diag_ladder(front_in_v2: U256) -> Vec<(String, U256)> {
    vec![
        ("a:front=0".to_string(), U256::ZERO),
        ("b:front=v2".to_string(), front_in_v2),
        ("c:50%".to_string(), front_in_v2 / U256::from(2u64)),
        ("c:25%".to_string(), front_in_v2 / U256::from(4u64)),
        ("c:10%".to_string(), front_in_v2 / U256::from(10u64)),
    ]
}

/// Bản nhận DANH SÁCH biến thể tuỳ ý của `diagnose_victim_ok` — cần cho thí
/// nghiệm đối chứng "V2-math thô" vs "V2-math ĐÃ RÀNG BUỘC victim-ok" (mục 2)
/// trên CÙNG một fork, vì so 2 mức `front_in` ở 2 fork khác nhau thì block đã
/// trôi và kết luận vô nghĩa. `verdict` vẫn đọc theo 2 nhãn chuẩn
/// `a:front=0` / `b:front=v2` nếu chúng có mặt.
pub async fn diagnose_victim_ok_variants(
    provider: DynProvider,
    fork_block: u64,
    token: Address,
    quote: Address,
    victim: &PendingTxRaw,
    variants: &[(String, U256)],
    victim_topup: Option<U256>,
) -> Result<VictimDiagReport, SimEvmError> {
    let mut fork = BlockForkCache::open(provider, fork_block).await?;
    let mut rows: Vec<VictimDiagRow> = Vec::with_capacity(variants.len());
    for (label, front_in) in variants.iter().map(|(l, f)| (l.clone(), *f)) {
        let (res, ms) = fork.run_sandwich_quote_cached_topup(front_in, token, quote, victim, victim_topup);
        rows.push(match res {
            Ok(o) => VictimDiagRow {
                label: label.clone(),
                front_in,
                victim_ok: o.victim_success,
                victim_out: o.victim_out,
                victim_revert_reason: o.victim_revert_reason,
                profit_wei: o.profit_wei,
                err: None,
                ms,
            },
            Err(e) => VictimDiagRow {
                label: label.clone(),
                front_in,
                victim_ok: false,
                victim_out: U256::ZERO,
                victim_revert_reason: None,
                profit_wei: 0,
                err: Some(e.to_string()),
                ms,
            },
        });
    }

    let alive_zero = rows.iter().find(|r| r.front_in.is_zero()).map(|r| r.victim_ok && r.err.is_none());
    let alive_v2 = rows.iter().find(|r| r.label == "b:front=v2").map(|r| r.victim_ok && r.err.is_none());
    let max_front_victim_alive =
        rows.iter().filter(|r| r.victim_ok && r.err.is_none()).map(|r| r.front_in).max();
    let verdict = match (alive_zero, alive_v2) {
        (Some(false), _) => "state_fork_sai",
        (Some(true), Some(true)) => "khong_tai_hien",
        (Some(true), Some(false)) => "front_giet_victim",
        _ => "sim_error",
    };
    Ok(VictimDiagReport { fork_block, rows, max_front_victim_alive, verdict })
}

// ============================================================================
// Cụm `evm-validate-fixed-then-wire` (C1) — ĐO TAX THẬT bằng revm
// ============================================================================

/// Kết quả đo tax bằng EVM thật — thay hẳn `tax.rs::measure_roundtrip_via_router`
/// (chỉ đo được AMM-math thuần, KHÔNG thấy fee-on-transfer; xem `docs/STATE.md`
/// mục "Tax stub").
#[derive(Debug, Clone, Copy)]
pub struct EvmTaxMeasurement {
    pub buy_bps: u32,
    pub sell_bps: u32,
    /// `true` khi mua được nhưng KHÔNG bán lại được (back-sell revert) hoặc
    /// mua về 0 token — dấu hiệu honeypot kinh điển.
    pub honeypot: bool,
}

/// C1 — mua `probe_in` quote-asset lấy token, đọc `balanceOf` THẬT, rồi bán
/// lại toàn bộ; so mỗi chiều với `getAmountsOut` (AMM-math thuần) để ra bps
/// hao hụt. Chạy trên fork đã warm, KHÔNG commit gì lên chain thật.
///
/// Hỗ trợ CẢ quote WBNB (đi đường native `swapExactETHForTokens…`, không cần
/// cấp token) LẪN quote USDT (B3.3 — cấp số dư USDT cho attacker bằng
/// `set_erc20_balance` sau khi dò slot, rồi đi đường `swapExactTokensForTokens…`).
fn measure_tax_on_fork(
    evm: &mut ForkEvm,
    token: Address,
    quote: Address,
    probe_in: U256,
) -> Result<EvmTaxMeasurement, SimEvmError> {
    let attacker = attacker_address();
    let is_native_quote = quote == wbnb();

    // ---- chuan bi so du quote cho attacker ----
    if !is_native_quote {
        // B3.3 - quote USDT: khong co co che "wrap native", phai ghi thang
        // storage balanceOf(attacker) cua token quote (dung LAI ky thuat
        // sentinel-probe da verify o B4'.4).
        let slot = probe_erc20_balance_slot(evm, quote, attacker)?;
        set_erc20_balance(evm, quote, attacker, slot, probe_in)?;
        let approve = IERC20Min::approveCall { spender: router(), amount: U256::MAX }.abi_encode();
        let tx = TxEnv::builder()
            .caller(attacker)
            .kind(TxKind::Call(quote))
            .gas_limit(200_000)
            .gas_price(0)
            .nonce(0)
            .chain_id(Some(56))
            .data(Bytes::from(approve))
            .build_fill();
        let r = evm.transact_commit(tx).map_err(|e| SimEvmError::Exec(format!("approve quote: {e:?}")))?;
        if !r.is_success() {
            return Err(SimEvmError::Exec(format!("approve quote revert: {r:?}")));
        }
    }

    // ---- BUY: quote -> token ----
    let expected_buy = quote_amounts_out(evm, quote, token, probe_in).ok().flatten();
    let buy_calldata = if is_native_quote {
        IPancakeV2RouterFeeOnTransfer::swapExactETHForTokensSupportingFeeOnTransferTokensCall {
            amountOutMin: U256::ZERO,
            path: vec![quote, token],
            to: attacker,
            deadline: U256::from(DEADLINE_MAX),
        }
        .abi_encode()
    } else {
        IPancakeV2RouterFeeOnTransfer::swapExactTokensForTokensSupportingFeeOnTransferTokensCall {
            amountIn: probe_in,
            amountOutMin: U256::ZERO,
            path: vec![quote, token],
            to: attacker,
            deadline: U256::from(DEADLINE_MAX),
        }
        .abi_encode()
    };
    let mut buy = TxEnv::builder()
        .caller(attacker)
        .kind(TxKind::Call(router()))
        .gas_limit(3_000_000)
        .gas_price(0)
        .nonce(0)
        .chain_id(Some(56))
        .data(Bytes::from(buy_calldata));
    if is_native_quote {
        buy = buy.value(probe_in);
    }
    let buy_res = evm.transact_commit(buy.build_fill()).map_err(|e| SimEvmError::Exec(format!("tax buy: {e:?}")))?;
    if !buy_res.is_success() {
        // Khong mua duoc -> khong ket luan duoc tax, KHONG bia so 0.
        return Err(SimEvmError::Exec(format!("tax buy revert/halt: {buy_res:?}")));
    }
    let token_received = read_balance(evm, token, attacker)?;
    if token_received.is_zero() {
        return Ok(EvmTaxMeasurement { buy_bps: 10_000, sell_bps: 10_000, honeypot: true });
    }
    let buy_bps = expected_buy.and_then(|e| shortfall_bps(e, token_received)).unwrap_or(0);

    // ---- SELL: token -> quote ----
    let approve_token = IERC20Min::approveCall { spender: router(), amount: U256::MAX }.abi_encode();
    let tx = TxEnv::builder()
        .caller(attacker)
        .kind(TxKind::Call(token))
        .gas_limit(300_000)
        .gas_price(0)
        .nonce(0)
        .chain_id(Some(56))
        .data(Bytes::from(approve_token))
        .build_fill();
    let ar = evm.transact_commit(tx).map_err(|e| SimEvmError::Exec(format!("tax approve token: {e:?}")))?;
    if !ar.is_success() {
        return Ok(EvmTaxMeasurement { buy_bps, sell_bps: 10_000, honeypot: true });
    }

    let expected_sell = quote_amounts_out(evm, token, quote, token_received).ok().flatten();
    let quote_before = if is_native_quote {
        evm.ctx
            .db_mut()
            .basic(attacker)
            .map_err(|e| SimEvmError::Exec(format!("doc so du native: {e:?}")))?
            .map(|i| i.balance)
            .unwrap_or(U256::ZERO)
    } else {
        read_balance(evm, quote, attacker)?
    };
    let sell_calldata = if is_native_quote {
        IPancakeV2RouterFeeOnTransfer::swapExactTokensForETHSupportingFeeOnTransferTokensCall {
            amountIn: token_received,
            amountOutMin: U256::ZERO,
            path: vec![token, quote],
            to: attacker,
            deadline: U256::from(DEADLINE_MAX),
        }
        .abi_encode()
    } else {
        IPancakeV2RouterFeeOnTransfer::swapExactTokensForTokensSupportingFeeOnTransferTokensCall {
            amountIn: token_received,
            amountOutMin: U256::ZERO,
            path: vec![token, quote],
            to: attacker,
            deadline: U256::from(DEADLINE_MAX),
        }
        .abi_encode()
    };
    let sell = TxEnv::builder()
        .caller(attacker)
        .kind(TxKind::Call(router()))
        .gas_limit(3_000_000)
        .gas_price(0)
        .nonce(0)
        .chain_id(Some(56))
        .data(Bytes::from(sell_calldata))
        .build_fill();
    let sell_res = evm.transact_commit(sell).map_err(|e| SimEvmError::Exec(format!("tax sell: {e:?}")))?;
    if !sell_res.is_success() {
        // Mua duoc nhung KHONG ban lai duoc = honeypot that (day chinh la
        // truong hop cong thuc dong `sim_v2` khong bao gio phat hien duoc).
        return Ok(EvmTaxMeasurement { buy_bps, sell_bps: 10_000, honeypot: true });
    }
    let quote_after = if is_native_quote {
        evm.ctx
            .db_mut()
            .basic(attacker)
            .map_err(|e| SimEvmError::Exec(format!("doc so du native sau ban: {e:?}")))?
            .map(|i| i.balance)
            .unwrap_or(U256::ZERO)
    } else {
        read_balance(evm, quote, attacker)?
    };
    let quote_out = quote_after.saturating_sub(quote_before);
    let sell_bps = expected_sell.and_then(|e| shortfall_bps(e, quote_out)).unwrap_or(0);

    Ok(EvmTaxMeasurement { buy_bps, sell_bps, honeypot: quote_out.is_zero() })
}

// ============================================================================
// Cụm `real-economics-mode2` (F-03) — đo gas UNIT thật (không phải wei) của
// 1 chân front-buy/back-sell bằng revm, dùng để cache `gas_units_front`/
// `gas_units_back` lúc boot (fallback config nếu đo lỗi).
// ============================================================================

/// Đo gas unit THẬT của `swapExactETHForTokensSupportingFeeOnTransferTokens`
/// (front) rồi `swapExactTokensForETHSupportingFeeOnTransferTokens` (back)
/// trên 1 `token` đã vet (nên chọn token zero-tax/đã xác nhận sạch để phép đo
/// không lẫn thêm logic lạ, dù gas unit đo được không đổi nhiều theo tax —
/// đường transfer ERC20 chuẩn tốn gas gần như cố định). `probe_in` nên đủ
/// nhỏ để không tốn thời gian nhưng đủ khác 0 để router thực thi trọn vẹn
/// (không revert vì INSUFFICIENT_OUTPUT).
pub async fn measure_gas_units(
    provider: DynProvider,
    fork_block: u64,
    token: Address,
    probe_in: U256,
) -> Result<(u64, u64), SimEvmError> {
    let (mut db, ts) = open_fork(provider, fork_block).await?;
    db.insert_account_info(attacker_address(), AccountInfo::from_balance(U256::from(FUND_BNB_WEI)));
    let mut evm = build_evm(db, fork_block, ts);
    let attacker = attacker_address();

    let front_calldata = IPancakeV2RouterFeeOnTransfer::swapExactETHForTokensSupportingFeeOnTransferTokensCall {
        amountOutMin: U256::ZERO,
        path: vec![wbnb(), token],
        to: attacker,
        deadline: U256::from(DEADLINE_MAX),
    }
    .abi_encode();
    let front_tx = TxEnv::builder()
        .caller(attacker)
        .kind(TxKind::Call(router()))
        .value(probe_in)
        .gas_limit(3_000_000)
        .gas_price(0)
        .nonce(0)
        .chain_id(Some(56))
        .data(Bytes::from(front_calldata))
        .build_fill();
    let front_result = evm.transact_commit(front_tx).map_err(|e| SimEvmError::Exec(format!("front-buy do gas: {e:?}")))?;
    if !front_result.is_success() {
        return Err(SimEvmError::Revert(format!("front-buy revert luc do gas unit: {front_result:?}")));
    }
    let front_gas_units = front_result.tx_gas_used();

    let token_received = read_balance(&mut evm, token, attacker)?;
    if token_received.is_zero() {
        return Err(SimEvmError::Exec("front-buy khong nhan duoc token nao, khong do duoc back-sell".to_string()));
    }

    let approve_calldata = IERC20Min::approveCall { spender: router(), amount: U256::MAX }.abi_encode();
    let approve_tx = TxEnv::builder()
        .caller(attacker)
        .kind(TxKind::Call(token))
        .gas_limit(200_000)
        .gas_price(0)
        .nonce(0)
        .chain_id(Some(56))
        .data(Bytes::from(approve_calldata))
        .build_fill();
    let approve_result = evm.transact_commit(approve_tx).map_err(|e| SimEvmError::Exec(format!("approve do gas: {e:?}")))?;
    if !approve_result.is_success() {
        return Err(SimEvmError::Revert(format!("approve revert luc do gas unit: {approve_result:?}")));
    }

    let back_calldata = IPancakeV2RouterFeeOnTransfer::swapExactTokensForETHSupportingFeeOnTransferTokensCall {
        amountIn: token_received,
        amountOutMin: U256::ZERO,
        path: vec![token, wbnb()],
        to: attacker,
        deadline: U256::from(DEADLINE_MAX),
    }
    .abi_encode();
    let back_tx = TxEnv::builder()
        .caller(attacker)
        .kind(TxKind::Call(router()))
        .gas_limit(3_000_000)
        .gas_price(0)
        .nonce(0)
        .chain_id(Some(56))
        .data(Bytes::from(back_calldata))
        .build_fill();
    let back_result = evm.transact_commit(back_tx).map_err(|e| SimEvmError::Exec(format!("back-sell do gas: {e:?}")))?;
    if !back_result.is_success() {
        return Err(SimEvmError::Revert(format!("back-sell revert luc do gas unit: {back_result:?}")));
    }
    let back_gas_units = back_result.tx_gas_used();

    Ok((front_gas_units, back_gas_units))
}

/// C1 — bản độc lập (tự fork) cho test/đường gọi không có `BlockForkCache`.
pub async fn measure_tax_evm(
    provider: DynProvider,
    fork_block: u64,
    token: Address,
    quote: Address,
    probe_in: U256,
) -> Result<EvmTaxMeasurement, SimEvmError> {
    let (mut db, ts) = open_fork(provider, fork_block).await?;
    db.insert_account_info(attacker_address(), AccountInfo::from_balance(U256::from(FUND_BNB_WEI)));
    let mut evm = build_evm(db, fork_block, ts);
    measure_tax_on_fork(&mut evm, token, quote, probe_in)
}

/// B3.2 — thu hẹp `front_in` bằng EVM THẬT trên fork đã warm, xuất phát từ ước
/// lượng công thức đóng `sim_v2` (đúng AGENTS.md: công thức đóng CHỈ để ước
/// lượng khoảng, quyết định cuối cùng dùng EVM thật).
///
/// Thử một lưới nhỏ quanh ước lượng (`×0.5 … ×1.5`, kẹp trong `[1, max_front]`)
/// thay vì ternary search nhiều vòng: mỗi lần thử là 1 sandwich ĐẦY ĐỦ trên
/// fork (reset sạch giữa các lần nhờ `BlockForkCache::reset`), nên số lần thử
/// đổi THẲNG thành độ trễ trên đường nóng. 5 điểm là thoả hiệp giữa chất lượng
/// tối ưu và độ trễ — số lần thử + ms thực đo được log ra `sim.evm` để Chủ tự
/// thấy chi phí, không phải đoán.
///
/// Trả về outcome có `profit_wei` LỚN NHẤT trong các lần chạy THÀNH CÔNG.
/// `Err` chỉ khi KHÔNG lần nào chạy được (mọi lần đều lỗi EVM/RPC) — caller
/// dịch thành `sim_error`, KHÔNG âm thầm rơi về `sim_v2`.
pub fn refine_front_in_on_fork(
    fork: &mut BlockForkCache,
    token: Address,
    victim: &PendingTxRaw,
    sim_v2_estimate: U256,
    max_front_wei: U256,
) -> Result<(EvmSandwichOutcome, u32, f64), SimEvmError> {
    // **Chỉ đánh giá ĐÚNG điểm ước lượng `sim_v2`** (kẹp trong `[1, max_front]`),
    // KHÔNG quét nhiều điểm trên fork dùng chung. Lý do (đo THẬT phiên này —
    // BAOCAO33 ô 5): `BlockForkCache::reset()` khôi phục `accounts` về ảnh chụp
    // rỗng để 2 lần thử độc lập nhau, nhưng làm vậy XOÁ luôn dữ liệu account đã
    // fetch từ remote -> mỗi lần thử lại cold-fetch (~3.6s/lần). Đây đúng thiết
    // kế gốc cụm B (BAOCAO31): "dùng THẲNG ước lượng `sim_v2::search_max_front_in`
    // làm `front_in` DUY NHẤT đưa vào EVM thật — EVM đóng vai trò XÁC NHẬN/ĐO
    // LẠI profit thật (có tax) tại đúng mức đó, không tìm lại từ đầu". Việc quét
    // nhiều `front_in` với warm-cache thật (reset slot-level, ~3-10ms/lần) nằm ở
    // đường `refine_front_in_with_evm` (B4''.4) — cần biết token0/pair/reserve
    // để reset đúng slot, không tổng quát hoá được ở đây.
    let one = U256::from(1u64);
    let candidates: Vec<U256> = vec![sim_v2_estimate.clamp(one, max_front_wei.max(one))];

    let mut best: Option<EvmSandwichOutcome> = None;
    let mut last_err: Option<SimEvmError> = None;
    let mut tried = 0u32;
    let mut total_ms = 0.0;
    for front_in in candidates {
        let (res, ms) = fork.run_sandwich_cached(front_in, token, victim);
        tried += 1;
        total_ms += ms;
        match res {
            Ok(o) => {
                if best.as_ref().map(|b| o.profit_wei > b.profit_wei).unwrap_or(true) {
                    best = Some(o);
                }
            }
            Err(e) => last_err = Some(e),
        }
    }
    match best {
        Some(b) => Ok((b, tried, total_ms)),
        None => Err(last_err.unwrap_or_else(|| SimEvmError::Exec("khong lan thu nao chay duoc".to_string()))),
    }
}

/// Kết quả dự đoán victim đơn lẻ dùng cho B4''.2 / B3.4 (validator nhúng).
#[derive(Debug, Clone)]
pub struct VictimPrediction {
    pub success: bool,
    /// `amountOut` của token đọc từ log `Swap` của ĐÚNG `pair` do chính EVM
    /// sinh ra khi replay — so ĐƯỢC TRỰC TIẾP với log `Swap` trong
    /// `eth_getTransactionReceipt` thật (cùng nguồn ngữ nghĩa, cùng decoder
    /// `decode_v2_swap_amount_out`). `None` nếu tx không chạm `pair` (hoặc
    /// revert) — KHÔNG bịa 0.
    pub swap_out: Option<U256>,
    /// Delta `balanceOf(victim.from)` — giữ lại để so sánh phụ (khác
    /// `swap_out` đúng bằng tax-on-transfer nếu token có tax).
    pub balance_delta: U256,
}

/// B4''.2 — replay victim ĐƠN LẺ và đọc `amountOut` từ log `Swap` của `pair`
/// do EVM sinh ra. Đây là số ĐỐI CHỨNG đúng với `victim_out_real` lấy từ
/// `eth_getTransactionReceipt` (cùng event, cùng pair, cùng decoder) — khác
/// `predict_victim_token_delta` (đo `balanceOf`, lệch đúng bằng tax nếu token
/// có fee-on-transfer, nên KHÔNG so trực tiếp với log Swap được).
pub async fn predict_victim_swap_out(
    provider: DynProvider,
    fork_block: u64,
    victim: &PendingTxRaw,
    token: Address,
    pair: Address,
    token_is_token0: bool,
) -> Result<VictimPrediction, SimEvmError> {
    let victim_to = victim.to.ok_or_else(|| SimEvmError::Fork("victim.to=None, khong the replay".to_string()))?;
    let (db, block_timestamp) = open_fork(provider, fork_block).await?;
    let mut evm = build_evm(db, fork_block, block_timestamp);

    let before = read_balance(&mut evm, token, victim.from)?;
    let victim_gas_price = u128::try_from(victim.gas_price).unwrap_or(0);
    let victim_tx = TxEnv::builder()
        .caller(victim.from)
        .kind(TxKind::Call(victim_to))
        .value(victim.value)
        .gas_limit(victim.gas.max(200_000))
        .gas_price(victim_gas_price)
        .nonce(victim.nonce)
        .chain_id(Some(56))
        .data(Bytes::from(victim.input.clone()))
        .build_fill();
    let result = evm
        .transact_commit(victim_tx)
        .map_err(|e| SimEvmError::Exec(format!("victim replay (swap_out): {e:?}")))?;
    let success = result.is_success();

    let topic0 = swap_topic0();
    let mut swap_out: Option<U256> = None;
    for log in result.logs() {
        if log.address != pair {
            continue;
        }
        if log.data.topics().first() != Some(&topic0) {
            continue;
        }
        swap_out = decode_v2_swap_amount_out(log.data.data.as_ref(), token_is_token0);
        break;
    }

    let after = read_balance(&mut evm, token, victim.from)?;
    Ok(VictimPrediction { success, swap_out, balance_delta: after.saturating_sub(before) })
}

/// Solidity mapping storage key chuẩn: `keccak256(abi.encode(key, slot))` —
/// `holder` right-pad về address word, `slot` là index khai báo biến mapping
/// trong contract (KHÔNG phải slot cuối cùng của 1 entry — đó chính là kết
/// quả hàm này).
fn mapping_storage_key(holder: Address, slot: U256) -> U256 {
    let mut buf = [0u8; 64];
    buf[12..32].copy_from_slice(holder.as_slice());
    buf[32..64].copy_from_slice(&slot.to_be_bytes::<32>());
    U256::from_be_bytes(alloy::primitives::keccak256(buf).0)
}

/// B4'.4 + B3.3 (dùng lại được cho USDT storage-override sau này) — dò slot
/// khai báo `mapping(address=>uint256) balances` bằng cách ghi 1 giá trị
/// SENTINEL vào từng slot ứng viên (0..20) rồi gọi `balanceOf` thật qua EVM
/// xem có đọc lại ĐÚNG sentinel không — token nào khác sentinel thì khôi phục
/// giá trị cũ (không để lại rác) rồi thử slot kế. `Err` nếu không token nào
/// trong 8 token allowlist chuẩn BEP20 khớp slot 0..20 (không đoán bừa).
fn probe_erc20_balance_slot(evm: &mut ForkEvm, token: Address, holder: Address) -> Result<u32, SimEvmError> {
    let sentinel = U256::from(0x1234_5678_90ab_cdefu64);
    for slot in 0u32..20 {
        let key = mapping_storage_key(holder, U256::from(slot));
        let original = evm
            .ctx
            .db_mut()
            .storage(token, key)
            .map_err(|e| SimEvmError::Exec(format!("doc storage slot {slot} that bai: {e:?}")))?;
        evm.ctx
            .db_mut()
            .insert_account_storage(token, key, sentinel)
            .map_err(|e| SimEvmError::Exec(format!("ghi storage slot {slot} that bai: {e:?}")))?;
        let bal = read_balance(evm, token, holder)?;
        if bal == sentinel {
            return Ok(slot);
        }
        evm.ctx
            .db_mut()
            .insert_account_storage(token, key, original)
            .map_err(|e| SimEvmError::Exec(format!("khoi phuc storage slot {slot} that bai: {e:?}")))?;
    }
    Err(SimEvmError::Exec(format!("khong tim thay storage slot balanceOf(holder) cho token {token:#x} trong pham vi slot 0..20")))
}

fn set_erc20_balance(evm: &mut ForkEvm, token: Address, holder: Address, slot: u32, value: U256) -> Result<(), SimEvmError> {
    let key = mapping_storage_key(holder, U256::from(slot));
    evm.ctx.db_mut().insert_account_storage(token, key, value).map_err(|e| SimEvmError::Exec(format!("set balance slot {slot} that bai: {e:?}")))
}

/// Pack `(reserve0, reserve1, blockTimestampLast)` vào ĐÚNG layout storage
/// slot 8 của `UniswapV2Pair.sol` (PancakeSwap V2 fork y hệt — xem
/// `docs/STATE.md` mục B4'.4 để có bằng chứng đối chiếu `eth_getStorageAt`
/// THẬT với `getReserves()` THẬT trước khi dùng hàm này để reset state).
fn pack_v2_reserves_slot(reserve0: U256, reserve1: U256, block_timestamp_last: u64) -> U256 {
    let mask112 = (U256::from(1u64) << 112) - U256::from(1u64);
    (reserve0 & mask112) | ((reserve1 & mask112) << 112) | (U256::from(block_timestamp_last) << 224)
}

/// Chiều ngược lại `pack_v2_reserves_slot` — dùng để VERIFY layout slot 8
/// bằng cách so kết quả unpack từ `eth_getStorageAt` THẬT với `getReserves()`
/// THẬT (2 nguồn độc lập phải khớp bit-for-bit trước khi tin dùng để reset).
pub fn unpack_v2_reserves_slot(word: U256) -> (U256, U256, u64) {
    let mask112 = (U256::from(1u64) << 112) - U256::from(1u64);
    let reserve0 = word & mask112;
    let reserve1 = (word >> 112) & mask112;
    let ts_u256 = (word >> 224) & U256::from(u32::MAX);
    let ts: u64 = u64::try_from(ts_u256).unwrap_or(0);
    (reserve0, reserve1, ts)
}

fn reset_pair_reserves(evm: &mut ForkEvm, pair: Address, reserve0: U256, reserve1: U256, block_timestamp_last: u64) -> Result<(), SimEvmError> {
    let packed = pack_v2_reserves_slot(reserve0, reserve1, block_timestamp_last);
    evm.ctx.db_mut().insert_account_storage(pair, U256::from(8u64), packed).map_err(|e| SimEvmError::Exec(format!("reset reserve slot 8 that bai: {e:?}")))
}

/// 1 lần thử `front_in` trong ternary search EVM — `result` lưu message lỗi
/// dạng `String` (không phải `SimEvmError`) vì cần giữ trong `Vec` để in log
/// cuối cùng, `SimEvmError` không bắt buộc `Clone`.
pub struct EvmSearchAttempt {
    pub front_in: U256,
    pub result: Result<EvmSandwichOutcome, String>,
    pub duration_ms: f64,
}

pub struct EvmSearchResult {
    pub best: Option<EvmSandwichOutcome>,
    pub attempts: Vec<EvmSearchAttempt>,
    pub balance_slot: u32,
    pub avg_duration_ms: f64,
}

/// B4'.4 — ternary search `front_in` quanh `sim_v2_estimate` (±50%, kẹp
/// trong `[1, max_front_wei]`) bằng EVM THẬT, TRÊN CÙNG 1 `ForkEvm` đã warm
/// (fork/fetch account info+bytecode CHỈ 1 LẦN ở đầu hàm) — mỗi vòng reset
/// state đã biết (attacker native, attacker token balance qua slot đã dò,
/// pair reserves qua slot 8 đã verify) bằng ghi trực tiếp `insert_account_*`
/// (KHÔNG gọi lại AlloyDB/RPC), rồi chạy `run_sandwich` y hệt
/// `simulate_sandwich`. `rounds` = 6..8 theo lệnh, mỗi vòng 2 lần sim (mid1,
/// mid2) + 2 lần sim biên cuối (lo, hi) sau khi hội tụ.
///
/// **Phát hiện thật (lần chạy đầu tiên B4'.4)**: chỉ reset slot 8 (reserve
/// CACHE) KHÔNG đủ — `PancakeV2Pair.swap()` (nguồn `UniswapV2Pair.sol` mà
/// Pancake fork y hệt) tự đọc `balanceOf(address(this))` THẬT của cả 2 token
/// để tính `amountIn` thực tế nhận được (không tin số attacker khai), rồi so
/// với reserve cache qua bất biến K. Sau lần thử ĐẦU (front-buy chuyển WBNB
/// thật vào pair, làm `WBNB.balanceOf(pair)` tăng thật trong cache), các lần
/// thử SAU reset slot 8 về reserve cũ nhưng `balanceOf(pair)` thật vẫn cao
/// hơn → `swap()` tính sai `amountIn` → revert `Pancake: K`. Sửa: reset
/// THÊM `balanceOf(pair)` của CẢ WBNB lẫn `token` (dò slot `balanceOf` của
/// WBNB riêng vì khác contract, dùng LẠI slot `token` đã dò cho attacker vì
/// slot mapping là theo CONTRACT không theo holder).
#[allow(clippy::too_many_arguments)]
pub async fn refine_front_in_with_evm(
    provider: DynProvider,
    fork_block: u64,
    token: Address,
    token0: Address,
    pair: Address,
    raw_reserve0: U256,
    raw_reserve1: U256,
    pair_block_timestamp_last: u64,
    victim: &PendingTxRaw,
    sim_v2_estimate: U256,
    max_front_wei: U256,
    rounds: u32,
) -> Result<EvmSearchResult, SimEvmError> {
    let (db, block_timestamp) = open_fork(provider, fork_block).await?;
    let mut evm = build_evm(db, fork_block, block_timestamp);
    let attacker = attacker_address();
    let (wbnb_reserve, token_reserve) = if token0 == wbnb() { (raw_reserve0, raw_reserve1) } else { (raw_reserve1, raw_reserve0) };

    // Reset native/reserve TRUOC khi do 2 slot balanceOf (khong can token
    // balance dung luc nay - probe dung sentinel doc-lap voi gia tri hien tai).
    evm.ctx.db_mut().insert_account_info(attacker, AccountInfo::from_balance(U256::from(FUND_BNB_WEI)));
    reset_pair_reserves(&mut evm, pair, raw_reserve0, raw_reserve1, pair_block_timestamp_last)?;
    let balance_slot = probe_erc20_balance_slot(&mut evm, token, attacker)?;
    let wbnb_balance_slot = probe_erc20_balance_slot(&mut evm, wbnb(), pair)?;

    let reset_all = |evm: &mut ForkEvm, slot: u32, wbnb_slot: u32| -> Result<(), SimEvmError> {
        evm.ctx.db_mut().insert_account_info(attacker, AccountInfo::from_balance(U256::from(FUND_BNB_WEI)));
        reset_pair_reserves(evm, pair, raw_reserve0, raw_reserve1, pair_block_timestamp_last)?;
        set_erc20_balance(evm, token, attacker, slot, U256::ZERO)?;
        // dua balanceOf(pair) THAT ve dung reserve da fetch - khop dieu kien
        // swap() tu kiem (xem doc-comment "Phat hien that" o tren).
        set_erc20_balance(evm, wbnb(), pair, wbnb_slot, wbnb_reserve)?;
        set_erc20_balance(evm, token, pair, slot, token_reserve)
    };

    let one = U256::from(1u64);
    let two = U256::from(2u64);
    let half_estimate = sim_v2_estimate / two;
    let one_half_estimate = (sim_v2_estimate + half_estimate).min(max_front_wei);
    let mut lo = half_estimate.max(one);
    let mut hi = one_half_estimate.max(lo + one);

    let mut attempts: Vec<EvmSearchAttempt> = Vec::new();
    let mut run_one = |evm: &mut ForkEvm, front_in: U256| -> i128 {
        let t0 = std::time::Instant::now();
        let reset = reset_all(evm, balance_slot, wbnb_balance_slot);
        let outcome = match reset {
            Ok(()) => run_sandwich(evm, front_in, token, victim),
            Err(e) => Err(e),
        };
        let duration_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let profit = outcome.as_ref().map(|o| o.profit_wei).unwrap_or(i128::MIN);
        attempts.push(EvmSearchAttempt { front_in, result: outcome.map_err(|e| e.to_string()), duration_ms });
        profit
    };

    for _ in 0..rounds.clamp(6, 8) {
        if hi <= lo + one {
            break;
        }
        let third = (hi - lo) / U256::from(3u64);
        let mid1 = lo + third;
        let mid2 = hi - third;
        let p1 = run_one(&mut evm, mid1);
        let p2 = run_one(&mut evm, mid2);
        if p1 < p2 {
            lo = mid1;
        } else {
            hi = mid2;
        }
    }
    run_one(&mut evm, lo);
    run_one(&mut evm, hi);
    run_one(&mut evm, max_front_wei);

    let mut best: Option<EvmSandwichOutcome> = None;
    for a in &attempts {
        if let Ok(o) = &a.result {
            let better = best.as_ref().map(|b| o.profit_wei > b.profit_wei).unwrap_or(true);
            if better {
                best = Some(o.clone());
            }
        }
    }
    let avg_duration_ms = if attempts.is_empty() { 0.0 } else { attempts.iter().map(|a| a.duration_ms).sum::<f64>() / attempts.len() as f64 };

    Ok(EvmSearchResult { best, attempts, balance_slot, avg_duration_ms })
}

/// B4'.3(b) — kết quả replay 1 bộ ba sandwich THẬT (front/victim/back đã
/// MINED) bằng EVM, so với "profit thật" tính từ số dư native on-chain THẬT
/// (KHÔNG tính gas — trừ lại gas_used*gas_price của front/back để cô lập
/// đúng phần swap, khớp cách `sim_evm`/`sim_v2` định nghĩa `profit_wei`).
#[derive(Debug, Clone)]
pub struct RealTripletReplay {
    pub profit_evm_wei: i128,
    pub victim_success_evm: bool,
}

/// Replay ĐÚNG 3 tx THẬT (front/victim/back, calldata/from/to/value/gas/nonce
/// nguyên bản) trên fork tại `fork_block = block_N - 1`. `gas_price` của
/// front/back bị GHI ĐÈ = 0 (giống `attacker` trong `simulate_sandwich`) để
/// số dư native attacker chỉ đổi vì SWAP thật, không lẫn gas — cho phép tính
/// `profit_evm = native_balance_sau_back - native_balance_truoc_front` TRỰC
/// TIẾP, không cần decode Swap event.
pub async fn replay_real_triplet(
    provider: DynProvider,
    fork_block: u64,
    front: &PendingTxRaw,
    victim: &PendingTxRaw,
    back: &PendingTxRaw,
) -> Result<RealTripletReplay, SimEvmError> {
    let front_to = front.to.ok_or_else(|| SimEvmError::Fork("front.to=None".to_string()))?;
    let victim_to = victim.to.ok_or_else(|| SimEvmError::Fork("victim.to=None".to_string()))?;
    let back_to = back.to.ok_or_else(|| SimEvmError::Fork("back.to=None".to_string()))?;
    let attacker = front.from;

    let (db, block_timestamp) = open_fork(provider, fork_block).await?;
    let mut evm = build_evm(db, fork_block, block_timestamp);

    let balance_before = evm
        .ctx
        .db_mut()
        .basic(attacker)
        .map_err(|e| SimEvmError::Exec(format!("doc so du truoc front that bai: {e:?}")))?
        .map(|i| i.balance)
        .unwrap_or(U256::ZERO);

    let front_tx = TxEnv::builder()
        .caller(front.from)
        .kind(TxKind::Call(front_to))
        .value(front.value)
        .gas_limit(front.gas.max(200_000))
        .gas_price(0)
        .nonce(front.nonce)
        .chain_id(Some(56))
        .data(Bytes::from(front.input.clone()))
        .build_fill();
    let front_result = evm.transact_commit(front_tx).map_err(|e| SimEvmError::Exec(format!("front replay: {e:?}")))?;
    if !front_result.is_success() {
        return Err(SimEvmError::Exec(format!("front replay revert/halt: {front_result:?}")));
    }

    let victim_gas_price = u128::try_from(victim.gas_price).unwrap_or(0);
    let victim_tx = TxEnv::builder()
        .caller(victim.from)
        .kind(TxKind::Call(victim_to))
        .value(victim.value)
        .gas_limit(victim.gas.max(200_000))
        .gas_price(victim_gas_price)
        .nonce(victim.nonce)
        .chain_id(Some(56))
        .data(Bytes::from(victim.input.clone()))
        .build_fill();
    let victim_result = evm.transact_commit(victim_tx).map_err(|e| SimEvmError::Exec(format!("victim replay: {e:?}")))?;
    let victim_success_evm = victim_result.is_success();

    let back_tx = TxEnv::builder()
        .caller(back.from)
        .kind(TxKind::Call(back_to))
        .value(back.value)
        .gas_limit(back.gas.max(200_000))
        .gas_price(0)
        .nonce(back.nonce)
        .chain_id(Some(56))
        .data(Bytes::from(back.input.clone()))
        .build_fill();
    let back_result = evm.transact_commit(back_tx).map_err(|e| SimEvmError::Exec(format!("back replay: {e:?}")))?;
    if !back_result.is_success() {
        return Err(SimEvmError::Exec(format!("back replay revert/halt: {back_result:?}")));
    }

    let balance_after = evm
        .ctx
        .db_mut()
        .basic(attacker)
        .map_err(|e| SimEvmError::Exec(format!("doc so du sau back that bai: {e:?}")))?
        .map(|i| i.balance)
        .unwrap_or(U256::ZERO);

    // gas_price=0 cho front/back -> balance chi doi vi swap that (khong lan
    // gas): profit_evm = balance_after - balance_before (co the am neu balance_after < balance_before).
    let before_i = i128::try_from(balance_before).map_err(|_| SimEvmError::Decode("balance_before vuot i128".to_string()))?;
    let after_i = i128::try_from(balance_after).map_err(|_| SimEvmError::Decode("balance_after vuot i128".to_string()))?;

    Ok(RealTripletReplay { profit_evm_wei: after_i - before_i, victim_success_evm })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attacker_address_is_stable_and_valid() {
        let a = attacker_address();
        let b = attacker_address();
        assert_eq!(a, b);
    }

    #[test]
    fn u256_to_u128_bps_zero_denom_is_none() {
        assert_eq!(u256_to_u128_bps(U256::from(1u64), U256::ZERO), None);
    }

    #[test]
    fn u256_to_u128_bps_exact_math() {
        // diff=50, denom=1000 -> 500 bps (5%)
        assert_eq!(u256_to_u128_bps(U256::from(50u64), U256::from(1000u64)), Some(500));
    }

    // ===== Cụm `evm-validate-fixed-then-wire` — test THUẦN cho hàm mới =====

    #[test]
    fn is_meaningful_candidate_rejects_too_small_and_low_impact() {
        // < 0.05 BNB -> loai
        assert!(is_meaningful_candidate(U256::from(10_000_000_000_000_000u128), U256::from(1_000_000_000_000_000_000u128)).is_err());
        // >= 0.05 BNB nhung impact < 0.02% (pool khong lo: 0.05 / 1_000_000 = 0.000005%)
        let huge_reserve = U256::from(1_000_000u128) * U256::from(1_000_000_000_000_000_000u128);
        assert!(is_meaningful_candidate(U256::from(50_000_000_000_000_000u128), huge_reserve).is_err());
        // 0.05 BNB vao pool 100 BNB = 0.05% impact >= 0.02% -> nhan
        assert!(is_meaningful_candidate(U256::from(50_000_000_000_000_000u128), U256::from(100u128) * U256::from(1_000_000_000_000_000_000u128)).is_ok());
    }

    #[test]
    fn decode_v2_swap_amount_out_reads_correct_side() {
        // data = amount0In | amount1In | amount0Out | amount1Out
        let mut data = [0u8; 128];
        data[64..96].copy_from_slice(&U256::from(111u64).to_be_bytes::<32>()); // amount0Out
        data[96..128].copy_from_slice(&U256::from(222u64).to_be_bytes::<32>()); // amount1Out
        assert_eq!(decode_v2_swap_amount_out(&data, true), Some(U256::from(111u64)));
        assert_eq!(decode_v2_swap_amount_out(&data, false), Some(U256::from(222u64)));
        assert_eq!(decode_v2_swap_amount_out(&[0u8; 64], true), None); // du lieu ngan
    }

    #[test]
    fn swap_topic0_matches_known_keccak() {
        // keccak256("Swap(address,uint256,uint256,uint256,uint256,address)")
        // = 0xd78ad95f... (gia tri quan sat that trong log Pancake V2, dan BAOCAO33)
        let t = swap_topic0();
        assert_eq!(format!("{t:#x}"), "0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822");
    }

    #[test]
    fn rpc_error_stats_classifies_32005() {
        let s = RpcErrorStats::default();
        s.record("server returned an error response: error code -32005: limit exceeded");
        s.record("some other transport error");
        s.record("limit exceeded again");
        assert_eq!(s.n_32005(), 2);
        assert_eq!(s.n_other(), 1);
    }

    #[test]
    fn validate_rpc_urls_reads_env_then_fallbacks() {
        // Khong set env -> luon co it nhat 1 URL (fallback), khong panic.
        let urls = validate_rpc_urls();
        assert!(!urls.is_empty());
    }

    /// Cụm `evm-validate-wire-tax` B4'.1+B4'.2 (thay bản cũ chỉ dừng ở 1
    /// candidate zero-tax đầu tiên) — chạy qua MỌI candidate đo được
    /// (`victim_success=true`), assert: (i) `buy_tax_bps`/`sell_tax_bps`
    /// PHẢI `Some`; (ii) cả 2 tax ≤10bps thì `|profit_evm-profit_v2|` ≤0.5%;
    /// (iii) tax >10bps thì `profit_evm < profit_v2` (đúng chiều — tax làm
    /// giảm lợi nhuận, KHÔNG được đo cao hơn công thức đóng không thấy tax).
    /// Log B4'.2: in ĐỦ front_in/front_out/victim_out/back_out (cả 2 đường
    /// sim_v2 và sim_evm) + reserve pool cho MỌI candidate, kể cả candidate
    /// cho `profit=-1` — số liệu đủ để soi tại sao nhiều token khác nhau
    /// cùng ra `-1` (xem docs/STATE.md mục B4'.2 để có phân tích công thức).
    /// Cụm `real-economics-mode2` (F-03) — ĐẠT CẦN DÁN: đo gas unit THẬT
    /// (`measure_gas_units`) trên CAKE (zero-tax allowlisted, chắc chắn có
    /// pool WBNB sâu) tại block hiện tại thật trên mainnet — in ra 2 số gas
    /// unit thật để dán vào BAOCAO (không bịa, không chỉ assert `>0` mù mờ).
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_rpc_measure_gas_units_on_cake() {
        use alloy::providers::{Provider, ProviderBuilder};
        use std::str::FromStr as _;

        let provider: DynProvider = ProviderBuilder::new()
            .connect("https://bsc-dataseed.binance.org/")
            .await
            .expect("ket noi RPC cong khai that bai")
            .erased();
        let chain_id = provider.get_chain_id().await.expect("eth_chainId that bai");
        assert_eq!(chain_id, 56);
        let fork_block = provider.get_block_number().await.expect("eth_blockNumber that bai");
        let cake = Address::from_str("0x0E09FaBB73Bd3Ade0a17ECC321fD13a19e81cE82").unwrap(); // CAKE, zero-tax allowlisted
        let probe_in = U256::from(50_000_000_000_000_000u128); // 0.05 BNB

        let (front_units, back_units) =
            measure_gas_units(provider, fork_block, cake, probe_in).await.expect("do gas unit that bai");
        println!("real_rpc_measure_gas_units_on_cake THAT: fork_block={fork_block} front_units={front_units} back_units={back_units}");
        assert!(front_units > 21_000, "front swap phai ton hon gas transfer BNB thuan (21000), got {front_units}");
        assert!(back_units > 21_000, "back swap phai ton hon gas transfer BNB thuan (21000), got {back_units}");
    }

    // `flavor = "multi_thread"` BẮT BUỘC — `WrapDatabaseAsync::new` (bridge
    // async AlloyDB -> sync Database revm cần) chỉ hoạt động trên runtime
    // multi-thread (đúng doc-comment crate, xem `revm-database-interface`);
    // `#[tokio::test]` mặc định single-thread sẽ fail ngay ở bước fork.
    // Production (`main.rs::main`, `#[tokio::main]`) đã multi-thread sẵn
    // (`Cargo.toml` bật `rt-multi-thread`), không cần đổi gì ở đó.
    /// Cụm `decision-data-24h` (mục 5) — TRẢ LỜI câu "node nào vet được pool
    /// nào": chạy ĐÚNG phép đo của `pairs_vet_task` (`measure_tax_evm` tại
    /// block hiện tại) cho 2 pool USDT bận nhất trên TỪNG URL RPC, in kết quả
    /// + LỚP LỖI (`transport::classify_vet_error`).
    ///
    /// Lý do cần: shadow run 30 phút của cụm này có 8/8 candidate rơi vào đúng
    /// 2 pool đó và cả 8 đều abort `vet_stale` — phải biết đây là "node không
    /// kham được" (hạ tầng, cần `BSC_HTTP_SIM` trả phí) hay "token có vấn đề"
    /// (nghiệp vụ). KHÔNG assert node nào phải chạy được (đó là chuyện của
    /// nhà cung cấp RPC, không phải của code) — test này ĐẠT khi CHẠY được và
    /// IN ra bảng thật; số liệu trong BAOCAO đọc từ bảng đó.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_rpc_which_node_can_vet_the_two_hot_usdt_pools() {
        use alloy::providers::{Provider, ProviderBuilder};
        use std::str::FromStr as _;

        let usdt = Address::from_str(crate::venues::USDT_ADDRESS).unwrap();
        let probe_in = U256::from(50u64) * U256::from(10u64).pow(U256::from(18u64)); // 50 USDT, khop probe_in_for_quote
        let tokens = [
            ("BNC", "0x01FBEd06A70EB1b4A0B43Dbc6a94f99944CD7777"),
            ("BinanceTown", "0xe210C0583C1071714EDed2d8bEEab05Ab5bB7777"),
        ];
        // Danh sach URL: `BSC_HTTP` cua Chu (neu co) + cac URL du phong.
        let mut urls = crate::transport::collect_rpc_urls_from_env("BSC_HTTP");
        for u in validate_rpc_urls() {
            if !urls.contains(&u) {
                urls.push(u);
            }
        }
        urls = crate::transport::filter_read_urls(urls);
        println!("real_rpc_which_node_can_vet: {} URL, 2 pool USDT nong nhat", urls.len());

        let mut any_ok = false;
        for url in &urls {
            let host = url.split('/').nth(2).unwrap_or("?");
            let provider: DynProvider = match ProviderBuilder::new().connect(url).await {
                Ok(p) => p.erased(),
                Err(e) => {
                    println!("  {host:<34} KET NOI LOI: {e}");
                    continue;
                }
            };
            let block = match provider.get_block_number().await {
                Ok(b) => b,
                Err(e) => {
                    println!("  {host:<34} eth_blockNumber LOI: {e}");
                    continue;
                }
            };
            for (sym, addr) in &tokens {
                let token = Address::from_str(addr).unwrap();
                match measure_tax_evm(provider.clone(), block, token, usdt, probe_in).await {
                    Ok(m) => {
                        any_ok = true;
                        println!(
                            "  {host:<34} {sym:<12} block={block} OK  buy_bps={} sell_bps={} honeypot={}",
                            m.buy_bps, m.sell_bps, m.honeypot
                        );
                    }
                    Err(e) => {
                        let es = e.to_string();
                        println!(
                            "  {host:<34} {sym:<12} block={block} LOI[{}] {}",
                            crate::transport::classify_vet_error(&es),
                            es.chars().take(110).collect::<String>()
                        );
                    }
                }
            }
        }
        println!("=> co it nhat 1 node vet duoc 2 pool nay: {any_ok}");

        // Phan 2 — DO SAU BLOCK: `pairs_vet_task` fork tai `app_state.last_block`
        // (block bot thay gan nhat), KHONG phai dung dinh chain. Neu node chi
        // giu state vai block thi vet se hong khi bot cham 1 nhip - do THAT
        // thay vi doan.
        if let Some(url) = urls.first() {
            let host = url.split('/').nth(2).unwrap_or("?");
            if let Ok(p) = ProviderBuilder::new().connect(url).await {
                let provider: DynProvider = p.erased();
                if let Ok(head) = provider.get_block_number().await {
                    let token = Address::from_str(tokens[0].1).unwrap();
                    for depth in [0u64, 2, 10, 50, 200] {
                        let b = head.saturating_sub(depth);
                        match measure_tax_evm(provider.clone(), b, token, usdt, probe_in).await {
                            Ok(m) => println!("  do sau: {host} block=head-{depth} ({b}) OK buy={} sell={}", m.buy_bps, m.sell_bps),
                            Err(e) => {
                                let es = e.to_string();
                                println!(
                                    "  do sau: {host} block=head-{depth} ({b}) LOI[{}] {}",
                                    crate::transport::classify_vet_error(&es),
                                    es.chars().take(90).collect::<String>()
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Cụm `decision-data-24h` (mục 4) — kiểm CƠ CHẾ nhánh quote USDT của
    /// `simulate_sandwich_quote` trên STATE THẬT tại block mới nhất: cấp vốn
    /// USDT cho attacker bằng ghi storage (sentinel-probe), approve router,
    /// front-buy `swapExactTokensForTokens(USDT→token)`, back-sell ngược lại.
    ///
    /// Victim ở đây là 1 tx USDT-buy DỰNG TAY từ một ví có USDT thật (không
    /// phải tx pending — test này kiểm PLUMBING, không kiểm dự đoán kinh tế:
    /// phần đó do shadow run 30 phút + `shadow.sim` đo trên victim THẬT).
    /// Vì vậy chỉ assert những gì chứng minh được: 2 chân swap CHẠY ĐƯỢC trên
    /// state thật, và round-trip khi không có victim ở giữa phải LỖ (đúng phí
    /// pool 2 × 0.25% + trượt giá), KHÔNG được lãi — lãi ở đây sẽ là dấu hiệu
    /// kế toán `back_out` sai chiều.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_rpc_simulate_sandwich_quote_usdt_leg_mechanics() {
        use alloy::providers::{Provider, ProviderBuilder};
        use std::str::FromStr as _;

        let url = validate_rpc_urls()[0].clone();
        let provider: DynProvider = ProviderBuilder::new().connect(&url).await.expect("ket noi RPC that bai").erased();
        let chain_id = provider.get_chain_id().await.expect("eth_chainId that bai");
        assert_eq!(chain_id, 56, "phai la BSC mainnet");
        let fork_block = provider.get_block_number().await.expect("eth_blockNumber that bai");

        // BNC/USDT - pool USDT dong nhat trong `pairs.txt` theo do that 10.92h
        // tren VPS (`logs/vps_analysis/1b_top_pools.tsv`).
        let token = Address::from_str("0x01FBEd06A70EB1b4A0B43Dbc6a94f99944CD7777").unwrap();
        let usdt = Address::from_str(crate::venues::USDT_ADDRESS).unwrap();
        let front_in = U256::from(1_000u64) * U256::from(10u64).pow(U256::from(18u64)); // 1000 USDT

        // Victim dung tay: 1 ví có USDT that mua token bang USDT qua V2 Router.
        let victim_from = Address::from_str("0xB406021E07b31E1f7850FCcCD7076094f18d07eF").unwrap();
        let victim_calldata = IPancakeV2RouterFeeOnTransfer::swapExactTokensForTokensSupportingFeeOnTransferTokensCall {
            amountIn: U256::from(100u64) * U256::from(10u64).pow(U256::from(18u64)),
            amountOutMin: U256::ZERO,
            path: vec![usdt, token],
            to: victim_from,
            deadline: U256::from(DEADLINE_MAX),
        }
        .abi_encode();
        let victim = PendingTxRaw {
            from: victim_from,
            to: Some(router()),
            value: U256::ZERO,
            input: victim_calldata,
            hash: alloy::primitives::B256::ZERO,
            gas: 400_000,
            gas_price: U256::from(1_000_000_000u64),
            nonce: 0,
        };

        let outcome = simulate_sandwich_quote(provider.clone(), fork_block, front_in, token, usdt, &victim)
            .await
            .expect("nhanh quote USDT phai chay duoc tren state THAT (truoc cum nay: khong co nhanh nay)");

        println!(
            "real_rpc_simulate_sandwich_quote_usdt THAT: fork_block={fork_block} token={token:#x} quote=USDT \n\
             front_in={} USDT  token_received={}  back_out={} USDT  profit={} (don vi USDT wei)  victim_success={}  buy_tax_bps={:?} sell_tax_bps={:?}",
            front_in,
            outcome.token_received,
            outcome.back_out,
            outcome.profit_wei,
            outcome.victim_success,
            outcome.buy_tax_bps,
            outcome.sell_tax_bps
        );

        assert!(!outcome.token_received.is_zero(), "front-buy bang USDT phai nhan duoc token that");
        assert!(!outcome.back_out.is_zero(), "back-sell phai tra lai USDT");
        // Round-trip qua CUNG 1 pool voi victim NHO (100 USDT) ke giua: phi
        // pool 2 x 0.25% (~50 bps) LON HON phan bot lai duoc tu victim, nen
        // ket qua phai LO — nhung lo IT HON 50 bps dung bang phan victim bu
        // lai. Do that lan chay dau: lo 23 bps (victim_success=true) — day
        // chinh la bang chung ca 3 chan (front USDT, victim replay, back
        // USDT) deu chay that tren state that.
        let front_i = i128::try_from(front_in).unwrap();
        assert!(
            outcome.profit_wei < 0,
            "victim 100 USDT qua nho so front 1000 USDT -> phai LO; lai o day la dau hieu ke toan back_out sai chieu (nhan duoc {})",
            outcome.profit_wei
        );
        let loss_bps = (-outcome.profit_wei) * 10_000 / front_i;
        println!("  round-trip loss = {loss_bps} bps (phi pool ~50 bps TRU phan victim bu lai; victim_success={})", outcome.victim_success);
        assert!(loss_bps <= 200, "lo {loss_bps} bps qua lon so phi pool 50 bps = dau hieu ke toan back_out sai, khong phai phi pool");
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_rpc_sim_evm_matches_sim_v2_when_zero_tax() {
        use crate::pool;
        use crate::sim_v2;
        use alloy::providers::{Provider, ProviderBuilder};
        use std::str::FromStr as _;

        let v2_router = router();
        let provider: DynProvider = ProviderBuilder::new()
            .connect("https://bsc-dataseed.binance.org/")
            .await
            .expect("ket noi RPC cong khai that bai")
            .erased();
        let chain_id = provider.get_chain_id().await.expect("eth_chainId that bai");
        assert_eq!(chain_id, 56);

        let candidates = poll_wbnb_v2_candidates(&provider, v2_router, 8, 40).await;
        println!(
            "tim thay {} candidate V2 WBNB-buy DANG CHO THAT trong mempool (txpool_content, gop nhieu lan poll, toi da 40 lan)",
            candidates.len()
        );
        if candidates.is_empty() {
            println!("SKIP (khong phai FAIL): khong tim thay candidate nao trong mempool sau 40 lan poll (~80s) - ban chat mempool thay doi lien tuc, thu lai sau");
            return;
        }

        let factory = Address::from_str(crate::venues::V2_FACTORY_ADDRESS).unwrap();
        let max_front = U256::from(1_500_000_000_000_000_000u128); // 1.5 BNB, khop config.toml ship
        let fork_block = provider.get_block_number().await.expect("eth_blockNumber that bai lan 2");

        let mut measured = 0u32;
        let mut zero_tax_count = 0u32;
        for (victim_raw, token) in &candidates {
            let pair = match pool::resolve_v2_pair(&provider, factory, *token).await {
                Ok(Ok(p)) => p,
                _ => continue,
            };
            let (reserve_wbnb, reserve_token) = match pool::get_reserves_vs_wbnb(&provider, pair).await {
                Ok(r) => r,
                Err(_) => continue,
            };
            let reserves = sim_v2::PoolReserves { reserve_wbnb, reserve_token };
            let quote_v2 = match sim_v2::search_max_front_in(reserves, victim_raw.value, max_front, 0) {
                Some(q) => q,
                None => continue,
            };

            let evm_outcome = match simulate_sandwich(provider.clone(), fork_block, quote_v2.front_in, *token, victim_raw).await {
                Ok(o) => o,
                Err(e) => {
                    println!("candidate token={token:#x}: sim_evm loi ({e}), thu candidate ke");
                    continue;
                }
            };

            // B4'.2 - log DAY DU so cho TUNG candidate (ca candidate profit=-1).
            println!(
                "candidate token={token:#x} pair={pair:#x} reserve_wbnb={reserve_wbnb} reserve_token={reserve_token} victim_in={}: \
sim_v2[front_in={} front_out={} victim_out={} back_out={} profit={}] \
sim_evm[token_received={} back_out={} profit={}] victim_success={} buy_tax_bps={:?} sell_tax_bps={:?} gas_wei=0",
                victim_raw.value,
                quote_v2.front_in, quote_v2.front_out, quote_v2.victim_out, quote_v2.back_out, quote_v2.profit_wei,
                evm_outcome.token_received, evm_outcome.back_out, evm_outcome.profit_wei,
                evm_outcome.victim_success, evm_outcome.buy_tax_bps, evm_outcome.sell_tax_bps
            );
            if quote_v2.front_out.is_zero() {
                println!("  => GIAI THICH profit=-1: front_out sim_v2=0 (front_in={} qua nho de mua duoc token nao, ternary search hoi tu ve bien duoi vi KHONG co front_in nao trong [0,{}] co loi) -> quote_at nhanh vao branch \"khong mua duoc gi\", profit=-front_in-gas=-{}-0={}", quote_v2.front_in, max_front, quote_v2.front_in, quote_v2.profit_wei);
            }

            if !evm_outcome.victim_success {
                // victim dang PENDING that (chua mined) - co the se bi revert
                // that tren chain that (vd front-run tu bot khac truoc no) -
                // khong phai loi cua sim, chi khong danh gia duoc candidate nay.
                continue;
            }
            measured += 1;

            // (i) buy_tax_bps/sell_tax_bps PHAI Some khi victim_success=true.
            assert!(evm_outcome.buy_tax_bps.is_some(), "buy_tax_bps phai Some khi victim_success=true, token={token:#x}");
            assert!(evm_outcome.sell_tax_bps.is_some(), "sell_tax_bps phai Some khi victim_success=true, token={token:#x}");

            let is_zero_tax = matches!(evm_outcome.buy_tax_bps, Some(b) if b <= 10) && matches!(evm_outcome.sell_tax_bps, Some(s) if s <= 10);
            if is_zero_tax {
                zero_tax_count += 1;
                // (ii) ca 2 tax <=10bps -> profit sim_v2 vs sim_evm lech <=0.5%.
                let diff = (quote_v2.profit_wei - evm_outcome.profit_wei).abs();
                let base = quote_v2.profit_wei.abs().max(1);
                let pct = (diff as f64) / (base as f64) * 100.0;
                println!("  => zero-tax: lech profit sim_v2 vs sim_evm = {pct:.4}%");
                assert!(pct <= 0.5, "lech {pct:.4}% > 0.5% cho token zero-tax {token:#x}");
            } else {
                // (iii) tax >10bps (it nhat 1 chieu) -> profit_evm KHONG
                // DUOC cao hon profit_v2 (dung chieu - tax lam giam loi,
                // cong thuc dong sim_v2 khong thay tax nen khong the lac
                // quan HON EVM that). Dung `<=` chu KHONG phai `<` nghiem
                // ngat: PHAT HIEN THAT (lan chay B4'.1 co RPC song, token
                // 0xcf0e...7777, buy_tax=299bps) - front_in qua nho (2 wei)
                // khien back_out ca 2 duong tinh cung floor ve 1 wei (V2
                // token_received truoc khi floor: 77305 vs 74986 - DUNG the
                // hien tax ~3%, chi FINAL back_out sau chia nguyen bi trung
                // do so tuyet doi qua nho) -> profit_evm==profit_v2==-1,
                // KHONG phai loi logic tax, la ranh gioi lam tron so nguyen
                // o front_in gan 0. Xem docs/STATE.md muc B4'.1.
                println!("  => co tax: profit_evm={} profit_v2={} (ky vong evm <= v2)", evm_outcome.profit_wei, quote_v2.profit_wei);
                assert!(
                    evm_outcome.profit_wei <= quote_v2.profit_wei,
                    "co tax (buy={:?}bps sell={:?}bps) nhung profit_evm ({}) CAO HON profit_v2 ({}), token={token:#x}",
                    evm_outcome.buy_tax_bps, evm_outcome.sell_tax_bps, evm_outcome.profit_wei, quote_v2.profit_wei
                );
            }
        }

        println!("TONG: {measured} candidate do duoc (victim_success=true) / {} candidate ban dau, trong do {zero_tax_count} zero-tax (<=10bps ca 2 chieu)", candidates.len());
        if measured == 0 {
            println!("SKIP (khong phai FAIL): khong candidate nao co victim_success=true de danh gia trong lan chay nay - thu lai sau");
        }
    }

    /// Helper CHỈ DÙNG TRONG TEST THẬT — dựng `PendingTxRaw` từ 1 tx đã MINED
    /// (`alloy::rpc::types::eth::Transaction`), khác `transport::pending_tx_from_rpc`
    /// (dùng cho tx PENDING qua subscribe/txpool) chỉ khác Ở CHỖ tx đã mined
    /// vẫn implement ĐÚNG 2 trait cần (`TransactionResponse`+`consensus::Transaction`)
    /// nên gọi lại được HÀM THẬT, không viết decode tay riêng cho test.
    fn transport_pending_tx_from_alloy(tx: &alloy::rpc::types::eth::Transaction) -> PendingTxRaw {
        crate::transport::pending_tx_from_rpc(tx)
    }

    /// Cụm `evm-validate-fixed-then-wire` B4''.2 — dự đoán victim ĐƠN LẺ khớp
    /// on-chain THẬT. **3 sửa so với B4'.3(a)** (BAOCAO32 không đạt số):
    ///
    /// 1. **B4''.1 chọn mẫu có ý nghĩa** — candidate phải qua
    ///    `is_meaningful_candidate` (`victim_in ≥ 0.05 BNB` VÀ impact ≥ 0.02%
    ///    reserve). Lý do loại được LOG đầy đủ (không im lặng bỏ) — mẫu quá
    ///    nhỏ cho `profit=-1` ở cả 2 đường tính, vô nghĩa để validate.
    /// 2. **B4''.2 `victim_out_real` lấy từ `eth_getTransactionReceipt`** của
    ///    CHÍNH victim (log `Swap` của pair nằm sẵn trong receipt) — KHÔNG
    ///    còn dùng `eth_getLogs` để LẤY SỐ. `eth_getLogs` chỉ còn dùng ĐÚNG 1
    ///    LẦN mỗi tx để KIỂM TRA CÔ LẬP (`fromBlock=toBlock=block_mined`,
    ///    `address=pair`, `topic0=Swap`) — >1 Swap thì loại.
    ///    Phía dự đoán dùng `predict_victim_swap_out` (đọc log `Swap` do
    ///    CHÍNH revm sinh ra) — so apples-to-apples với log `Swap` thật, thay
    ///    vì so `balanceOf` delta với log (2 đại lượng lệch nhau đúng bằng
    ///    tax-on-transfer — nguồn sai số giả trong B4'.3(a)).
    /// 3. **B4''.4 RPC failover + backoff** — `-32005` thì lùi 2s và đổi URL
    ///    kế; receipt đi qua danh sách failover (publicnode TỪ CHỐI receipt,
    ///    xem ghi chú trong thân hàm). Số `-32005` được ĐẾM và in ra.
    ///
    /// **Kiến trúc VÒNG (mới so với lần chạy #4)** — bắt buộc vì cửa sổ state:
    /// mọi RPC công khai đo được phiên này chỉ giữ state ~128 block gần nhất
    /// (`eth_call` lùi 100 block OK, lùi 300 block trả "Archive requests
    /// require a personal token" / "missing trie node" — bảng đo dán ở
    /// BAOCAO33 ô 5). BSC ra block ~0.75s nên 128 block ≈ 96 GIÂY. Lần chạy #4
    /// quét 60 block RỒI mới xử lý tuần tự → tới lúc fork thì `block_n - 1` đã
    /// rơi ra ngoài cửa sổ, 5/9 candidate chết vì lỗi archive. Sửa: mỗi VÒNG
    /// chỉ quét `SCAN_DEPTH` block mới nhất và fork NGAY trong vòng đó, lặp
    /// nhiều vòng để tích luỹ đủ mẫu theo thời gian thực.
    ///
    /// Ngưỡng GIỮ NGUYÊN theo lệnh: ≥5 tx đủ điều kiện, ≥80% lệch ≤1%.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_rpc_victim_prediction_matches_onchain() {
        use crate::pool;
        use alloy::providers::{Provider, ProviderBuilder};
        use alloy::rpc::types::eth::Filter;
        use std::str::FromStr as _;

        const SCAN_DEPTH: u64 = 12;
        const MAX_ROUNDS: u32 = 14;
        const TARGET_ELIGIBLE: u32 = 8;

        let stats = RpcErrorStats::default();
        let urls = validate_rpc_urls();
        println!("B4''.2 dung {} URL RPC (khong in gia tri URL)", urls.len());
        let provider: DynProvider =
            ProviderBuilder::new().connect(&urls[0]).await.expect("ket noi RPC validate that bai").erased();
        assert_eq!(provider.get_chain_id().await.expect("eth_chainId that bai"), 56);

        let mut fallbacks: Vec<DynProvider> = Vec::new();
        for u in urls.iter().skip(1) {
            if let Ok(p) = ProviderBuilder::new().connect(u).await {
                fallbacks.push(p.erased());
            }
        }
        println!("B4''.4: {} provider du phong cho receipt/getLogs + backoff -32005", fallbacks.len());

        let factory = Address::from_str(crate::venues::V2_FACTORY_ADDRESS).unwrap();
        let swap_topic = swap_topic0();
        let routers: Vec<Address> =
            crate::venues::PANCAKE_ROUTERS.iter().filter_map(|(a, _)| Address::from_str(a).ok()).collect();

        let mut eligible = 0u32;
        let mut within_1pct = 0u32;
        let mut rows: Vec<String> = Vec::new();
        let mut rejected_small = 0u32;
        let mut seen_hashes: std::collections::HashSet<alloy::primitives::B256> = std::collections::HashSet::new();
        let mut last_scanned: u64 = 0;

        for round in 0..MAX_ROUNDS {
            if eligible >= TARGET_ELIGIBLE {
                break;
            }
            let latest = match provider.get_block_number().await {
                Ok(b) => b,
                Err(e) => {
                    stats.record(&e.to_string());
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
            };
            let from = if last_scanned == 0 { latest.saturating_sub(SCAN_DEPTH) } else { (last_scanned + 1).max(latest.saturating_sub(SCAN_DEPTH)) };
            if from > latest {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                continue;
            }
            last_scanned = latest;

            let candidates = collect_mined_v2_buy_candidates(
                &provider,
                from,
                latest,
                &routers,
                U256::from(50_000_000_000_000_000u128), // 0.05 BNB, khop B4''.1
                12,
                &stats,
            )
            .await;
            println!("vong#{round}: quet block {from}..={latest} -> {} candidate >=0.05 BNB", candidates.len());

            for (victim, token, block_n) in &candidates {
                if !seen_hashes.insert(victim.hash) {
                    continue;
                }
                let block_n = *block_n;
                if block_n == 0 {
                    continue;
                }

                let pair = match pool::resolve_v2_pair(&provider, factory, *token).await {
                    Ok(Ok(p)) => p,
                    _ => continue,
                };
                let (token0, r0, r1) = match pool::get_raw_reserves_and_token0(&provider, pair).await {
                    Ok(v) => v,
                    Err(e) => {
                        stats.record(&e.to_string());
                        continue;
                    }
                };
                let token_is_token0 = token0 == *token;
                let reserve_quote = if token_is_token0 { r1 } else { r0 };
                if let Err(why) = is_meaningful_candidate(victim.value, reserve_quote) {
                    rejected_small += 1;
                    println!("  LOAI token={token:#x} (B4''.1): {why}");
                    continue;
                }

                // ---- victim_out_real tu RECEIPT (failover: publicnode TU CHOI
                // `eth_getTransactionReceipt` voi "-32602 Archive requests
                // require a personal token" du van phuc vu `eth_call` 128 block
                // + `txpool_content`; blockrazor/defibit tra binh thuong -
                // verify bang curl, dan o BAOCAO33 o 5). FORK van dung provider
                // chinh (node da xac nhan co state 128 block).
                let mut receipt = None;
                for p in std::iter::once(&provider).chain(fallbacks.iter()) {
                    match p.get_transaction_receipt(victim.hash).await {
                        Ok(Some(r)) => {
                            receipt = Some(r);
                            break;
                        }
                        Ok(None) => break,
                        Err(e) => {
                            let msg = e.to_string();
                            stats.record(&msg);
                            if msg.contains("-32005") || msg.contains("limit exceeded") {
                                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            }
                        }
                    }
                }
                let Some(receipt) = receipt else {
                    println!("  victim {:#x}: khong provider nao tra duoc receipt", victim.hash);
                    continue;
                };
                let mut victim_out_real: Option<U256> = None;
                for log in receipt.inner.logs() {
                    if log.address() != pair || log.topics().first() != Some(&swap_topic) {
                        continue;
                    }
                    victim_out_real = decode_v2_swap_amount_out(log.data().data.as_ref(), token_is_token0);
                    break;
                }
                let Some(victim_out_real) = victim_out_real else {
                    println!("  victim {:#x}: receipt khong co log Swap cua pair {pair:#x}", victim.hash);
                    continue;
                };

                // ---- CO LAP: DUNG 1 loi goi eth_getLogs (co failover) ----
                let filter =
                    Filter::new().address(pair).event_signature(swap_topic).from_block(block_n).to_block(block_n);
                let mut logs_opt = None;
                for p in std::iter::once(&provider).chain(fallbacks.iter()) {
                    match p.get_logs(&filter).await {
                        Ok(l) => {
                            logs_opt = Some(l);
                            break;
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            stats.record(&msg);
                            if msg.contains("-32005") || msg.contains("limit exceeded") {
                                println!("  victim {:#x}: -32005 tren eth_getLogs, backoff 2s + doi URL", victim.hash);
                                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            }
                        }
                    }
                }
                let Some(logs) = logs_opt else {
                    println!("  victim {:#x}: eth_getLogs that bai han", victim.hash);
                    continue;
                };
                if logs.len() != 1 || logs[0].transaction_hash != Some(victim.hash) {
                    println!(
                        "  victim {:#x}: pair co {} Swap trong block {block_n} (can dung 1) - KHONG co lap, loai",
                        victim.hash,
                        logs.len()
                    );
                    continue;
                }

                // ---- du doan bang EVM tai block CHA (fork NGAY, con trong cua so state) ----
                let pred = match predict_victim_swap_out(
                    provider.clone(),
                    block_n - 1,
                    victim,
                    *token,
                    pair,
                    token_is_token0,
                )
                .await
                {
                    Ok(p) => p,
                    Err(e) => {
                        stats.record(&e.to_string());
                        println!("  victim {:#x}: predict EVM loi ({e})", victim.hash);
                        continue;
                    }
                };
                let Some(pred_out) = pred.swap_out else {
                    println!("  victim {:#x}: replay EVM khong sinh log Swap (success={})", victim.hash, pred.success);
                    continue;
                };

                eligible += 1;
                let diff =
                    if pred_out > victim_out_real { pred_out - victim_out_real } else { victim_out_real - pred_out };
                let base = victim_out_real.max(U256::from(1u64));
                let pct = (u128::try_from(diff).unwrap_or(u128::MAX) as f64)
                    / (u128::try_from(base).unwrap_or(1) as f64)
                    * 100.0;
                let ok = pct <= 1.0;
                if ok {
                    within_1pct += 1;
                }
                let row = format!(
                    "hash={:#x} pair={pair:#x} block={block_n} pred={pred_out} real={victim_out_real} lech={pct:.6}% ok={ok}",
                    victim.hash
                );
                println!("  B4''.2 ROW {row}");
                rows.push(row);
            }
            tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        }

        println!("==== B4''.2 BANG KET QUA ({} dong) ====", rows.len());
        for r in &rows {
            println!("{r}");
        }
        println!(
            "TONG B4''.2: {eligible} tx du dieu kien, {within_1pct} lech <=1% ({:.1}%), {rejected_small} bi loai vi qua nho",
            if eligible > 0 { within_1pct as f64 / eligible as f64 * 100.0 } else { 0.0 }
        );
        println!("RPC_ERRORS trong lan chay nay: -32005={} khac={}", stats.n_32005(), stats.n_other());
        if eligible < 5 {
            println!("SKIP (khong phai FAIL): chi {eligible} tx du dieu kien (<5 theo lenh) - xem so -32005 o tren de phan biet 'hiem' voi 'khong quet duoc'");
            return;
        }
        let pass_pct = within_1pct as f64 / eligible as f64 * 100.0;
        assert!(pass_pct >= 80.0, "chi {pass_pct:.1}% lech <=1% (can >=80%), {within_1pct}/{eligible}");
    }

    /// Cụm `evm-validate-fixed-then-wire` B4''.3 — replay SANDWICH THẬT đã xảy
    /// ra (KHÔNG cần BscScan). **Sửa so với B4'.3(b)** (BAOCAO32 không đạt số):
    ///
    /// - Quét bằng `eth_getLogs` `topic0=Swap` **KHÔNG lọc `address`** (lấy
    ///   MỌI pair) thay vì fetch từng block đầy đủ rồi lọc `tx.to()`.
    /// - Dải quét 2000 block/vòng, lặp lùi tối đa 10 vòng (20k block ≈ 4 giờ).
    /// - Nhận diện bộ ba theo **logIndex liên tiếp CÙNG pair** (3 tx phân
    ///   biệt), rồi mới kiểm `from`: `i` và `i+2` cùng `from`, `i+1` khác.
    /// - Chỉ REPLAY các bộ nằm trong 100 block gần nhất (giới hạn state
    ///   non-archive, đo thật phiên này); bộ cũ hơn CHỈ ghi nhận hash để
    ///   chứng minh tồn tại.
    ///
    /// **Giới hạn hạ tầng đo THẬT phiên này (không phải giả định)**:
    /// `bsc-rpc.publicnode.com` TỪ CHỐI `eth_getLogs` không có `address`
    /// ("-32701: Please specify an address in your request"), nên KHÔNG dùng
    /// được cho mục này; `bsc.blockrazor.xyz` CHO PHÉP không lọc address
    /// nhưng chặn dải > 25 block ("-32000: log query range must not exceed 25
    /// blocks"). Vì vậy "1 lời gọi cho dải 2000 block" theo lệnh là BẤT KHẢ
    /// THI trên mọi endpoint Chủ cấp — thay bằng chia nhỏ thành các lời gọi
    /// `CHUNK` block liên tiếp phủ ĐÚNG dải 2000 block đó (cùng kết quả logic,
    /// chỉ khác số lời gọi). Ghi rõ ở đây thay vì im lặng đổi.
    ///
    /// Ngưỡng GIỮ NGUYÊN theo lệnh: ≥3 bộ replay được, lệch ≤2%.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_rpc_replay_real_sandwich_triplets() {
        use alloy::eips::BlockNumberOrTag;
        use alloy::network::TransactionResponse as _;
        use alloy::providers::{Provider, ProviderBuilder};
        use alloy::rpc::types::eth::Filter;

        const REPLAY_WINDOW: u64 = 100;
        /// Tong thoi gian tail toi da (giay) — mat do do that o lan #1 la
        /// ~1 bo sandwich / 250 block (~190s), nen ~20 phut du de gom >=3 bo.
        const MAX_TAIL_SECS: u64 = 1200;

        let stats = RpcErrorStats::default();
        let urls = validate_rpc_urls();
        let mut providers: Vec<(String, DynProvider)> = Vec::new();
        for u in &urls {
            if let Ok(p) = ProviderBuilder::new().connect(u).await {
                providers.push((u.clone(), p.erased()));
            }
        }
        assert!(!providers.is_empty(), "khong ket noi duoc RPC nao");
        let state_provider = providers[0].1.clone();
        let latest = state_provider.get_block_number().await.expect("eth_blockNumber that bai");

        // ---- chon provider CHO PHEP eth_getLogs KHONG loc address + do CHUNK toi da ----
        let probe_topic = swap_topic0();
        let mut scan: Option<(String, DynProvider, u64)> = None;
        for (name, p) in &providers {
            let mut chunk_ok = 0u64;
            for try_chunk in [2000, 100, 25, 10] {
                let lo = latest.saturating_sub(try_chunk - 1);
                let f = Filter::new().event_signature(probe_topic).from_block(lo).to_block(latest);
                match p.get_logs(&f).await {
                    Ok(_) => {
                        chunk_ok = try_chunk;
                        break;
                    }
                    Err(e) => {
                        stats.record(&e.to_string());
                        println!("  probe {name} chunk={try_chunk}: {e}");
                    }
                }
            }
            if chunk_ok > 0 {
                println!("=> dung {name} cho quet log khong loc address, CHUNK={chunk_ok} block/loi goi");
                scan = Some((name.clone(), p.clone(), chunk_ok));
                break;
            }
        }
        let Some((_scan_name, scan_provider, chunk)) = scan else {
            println!("SKIP (khong phai FAIL): KHONG endpoint nao cho eth_getLogs khong loc address");
            println!("RPC_ERRORS: -32005={} khac={}", stats.n_32005(), stats.n_other());
            return;
        };

        struct Cand {
            block: u64,
            pair: Address,
            hashes: [alloy::primitives::B256; 3],
        }

        // ================= CHE DO TAIL TRUC TIEP (sua sau lan chay #1) =======
        // Lan chay #1 (log day du o BAOCAO33 o 5) quet 1970 block LUI VE QUA
        // KHU: TIM DUOC 8 bo sandwich THAT (co hash, chung minh co che phat
        // hien DUNG va du lieu CO TON TAI), `-32005`=0 — nhung KHONG replay
        // duoc bo nao vi ca 8 deu cach block hien tai 1405..3160 block, trong
        // khi state non-archive chi giu ~128 block (do that phien nay). Ban
        // than viec quet 2000 block mat 589s, trong thoi gian do chain da
        // chay them ~800 block, nen moi bo tim duoc deu "gia" ngay khi tim ra.
        //
        // Sua: TAIL truc tiep dau chain — moi vong chi quet cac block MOI ke
        // tu vong truoc (toi da TAIL_MAX block), phat hien bo ba va REPLAY
        // NGAY trong khi state van con. Mat do do duoc o lan #1: ~8 bo /
        // 1970 block ≈ 1 bo / 250 block ≈ 1 bo / ~190 giay (BSC ~0.75s/block),
        // nen tail du lau se tich luy du 3 bo.
        const TAIL_MAX: u64 = 60;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(MAX_TAIL_SECS);

        let mut total_logs = 0u64;
        let mut total_groups = 0u64;
        let mut blocks_scanned = 0u64;
        let mut confirmed: Vec<(u64, Address, [alloy::primitives::B256; 3], Address)> = Vec::new();
        let mut replayed = 0u32;
        let mut within_2pct = 0u32;
        let mut replay_rows: Vec<String> = Vec::new();
        let mut cursor: u64 = latest;

        while std::time::Instant::now() < deadline && replayed < 3 {
            let head = match state_provider.get_block_number().await {
                Ok(b) => b,
                Err(e) => {
                    stats.record(&e.to_string());
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
            };
            if head <= cursor {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
            let lo = (cursor + 1).max(head.saturating_sub(TAIL_MAX));
            let hi = head;

            // ---- quet log Swap (khong loc address) cho [lo, hi] ----
            let mut triplets: Vec<Cand> = Vec::new();
            let mut c = lo;
            while c <= hi {
                let c_hi = (c + chunk - 1).min(hi);
                let f = Filter::new().event_signature(probe_topic).from_block(c).to_block(c_hi);
                match scan_provider.get_logs(&f).await {
                    Ok(logs) => {
                        total_logs += logs.len() as u64;
                        blocks_scanned += c_hi - c + 1;
                        let mut g: std::collections::HashMap<(u64, Address), Vec<(u64, alloy::primitives::B256)>> =
                            std::collections::HashMap::new();
                        for l in &logs {
                            let (Some(bn), Some(li), Some(th)) = (l.block_number, l.log_index, l.transaction_hash)
                            else {
                                continue;
                            };
                            g.entry((bn, l.address())).or_default().push((li, th));
                        }
                        for ((bn, pair), mut v) in g {
                            if v.len() < 3 {
                                continue;
                            }
                            v.sort_by_key(|(li, _)| *li);
                            for w in v.windows(3) {
                                let (h0, h1, h2) = (w[0].1, w[1].1, w[2].1);
                                if h0 == h1 || h1 == h2 || h0 == h2 {
                                    continue;
                                }
                                total_groups += 1;
                                triplets.push(Cand { block: bn, pair, hashes: [h0, h1, h2] });
                            }
                        }
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        stats.record(&msg);
                        if msg.contains("-32005") || msg.contains("limit exceeded") {
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        }
                    }
                }
                c = c_hi + 1;
                tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            }
            cursor = hi;

            // ---- doc `from` cho cac block co ung vien ----
            let need: std::collections::BTreeSet<u64> = triplets.iter().map(|t| t.block).collect();
            let mut froms: std::collections::HashMap<u64, std::collections::HashMap<alloy::primitives::B256, Address>> =
                std::collections::HashMap::new();
            for bn in need {
                match scan_provider.get_block_by_number(BlockNumberOrTag::Number(bn)).full().await {
                    Ok(Some(b)) => {
                        froms.insert(bn, b.transactions.txns().map(|t| (t.tx_hash(), t.from())).collect());
                    }
                    Ok(None) => {}
                    Err(e) => stats.record(&e.to_string()),
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }

            let mut found_this_round = 0u32;
            for t in &triplets {
                let Some(m) = froms.get(&t.block) else { continue };
                let (Some(&f0), Some(&f1), Some(&f2)) =
                    (m.get(&t.hashes[0]), m.get(&t.hashes[1]), m.get(&t.hashes[2]))
                else {
                    continue;
                };
                if f0 != f2 || f1 == f0 {
                    continue;
                }
                if confirmed.iter().any(|(b, p, h, _)| *b == t.block && *p == t.pair && *h == t.hashes) {
                    continue;
                }
                confirmed.push((t.block, t.pair, t.hashes, f0));
                found_this_round += 1;
                println!(
                    "  SANDWICH block={} pair={:#x} attacker={f0:#x} front={:#x} victim={:#x} back={:#x}",
                    t.block, t.pair, t.hashes[0], t.hashes[1], t.hashes[2]
                );

                // ---- REPLAY NGAY (state van con trong cua so ~128 block) ----
                let now = state_provider.get_block_number().await.unwrap_or(hi);
                if now.saturating_sub(t.block) > REPLAY_WINDOW {
                    println!("    -> cach {} block > {REPLAY_WINDOW}, KHONG replay (state non-archive)", now - t.block);
                    continue;
                }
                let mut txs: Vec<PendingTxRaw> = Vec::new();
                for hh in t.hashes.iter() {
                    match state_provider.get_transaction_by_hash(*hh).await {
                        Ok(Some(tx)) => txs.push(crate::transport::pending_tx_from_rpc(&tx)),
                        Ok(None) => {}
                        Err(e) => stats.record(&e.to_string()),
                    }
                }
                if txs.len() != 3 {
                    println!("    -> khong lay du 3 tx ({}), bo qua", txs.len());
                    continue;
                }
                let rep = match replay_real_triplet(state_provider.clone(), t.block - 1, &txs[0], &txs[1], &txs[2]).await
                {
                    Ok(r) => r,
                    Err(e) => {
                        stats.record(&e.to_string());
                        println!("    -> replay EVM loi ({e})");
                        continue;
                    }
                };
                let bal_before = match state_provider.get_balance(f0).number(t.block - 1).await {
                    Ok(b) => b,
                    Err(e) => {
                        stats.record(&e.to_string());
                        continue;
                    }
                };
                let bal_after = match state_provider.get_balance(f0).number(t.block).await {
                    Ok(b) => b,
                    Err(e) => {
                        stats.record(&e.to_string());
                        continue;
                    }
                };
                let mut gas_cost = U256::ZERO;
                for hh in [t.hashes[0], t.hashes[2]] {
                    for p in std::iter::once(&state_provider).chain(providers.iter().map(|(_, p)| p)) {
                        if let Ok(Some(r)) = p.get_transaction_receipt(hh).await {
                            gas_cost += U256::from(r.gas_used) * U256::from(r.effective_gas_price);
                            break;
                        }
                    }
                }
                let real_delta_i = if bal_after >= bal_before {
                    i128::try_from(bal_after - bal_before).unwrap_or(0)
                } else {
                    -(i128::try_from(bal_before - bal_after).unwrap_or(0))
                };
                let profit_real = real_delta_i + i128::try_from(gas_cost).unwrap_or(0);
                replayed += 1;
                let diff = (profit_real - rep.profit_evm_wei).abs();
                let base = profit_real.abs().max(1);
                let pct = (diff as f64) / (base as f64) * 100.0;
                if pct <= 2.0 {
                    within_2pct += 1;
                }
                let row = format!(
                    "block={} pair={:#x} front={:#x} victim={:#x} back={:#x} profit_evm={} profit_real={profit_real} lech={pct:.4}% victim_success_evm={}",
                    t.block, t.pair, t.hashes[0], t.hashes[1], t.hashes[2], rep.profit_evm_wei, rep.victim_success_evm
                );
                println!("    -> REPLAY {row}");
                replay_rows.push(row);
            }
            println!(
                "tail {lo}..={hi}: {} ung vien, {found_this_round} sandwich moi (tong {} | replay {replayed}) | log={total_logs} nhom={total_groups}",
                triplets.len(),
                confirmed.len()
            );
        }

        println!("==== B4''.3 TONG QUET (che do tail) ====");
        println!("block da quet: {blocks_scanned}, log Swap doc duoc: {total_logs}");
        println!("bo 3 swap lien tiep cung pair (3 tx phan biet): {total_groups}");
        println!("bo THOA dinh nghia sandwich (i&i+2 cung from): {}", confirmed.len());
        for (bn, pair, h, atk) in &confirmed {
            println!(
                "  SANDWICH block={bn} pair={pair:#x} attacker={atk:#x} front={:#x} victim={:#x} back={:#x}",
                h[0], h[1], h[2]
            );
        }
        println!("==== B4''.3 BANG REPLAY ({} dong) ====", replay_rows.len());
        for r in &replay_rows {
            println!("{r}");
        }
        println!("TONG B4''.3: {replayed} bo replay duoc, {within_2pct} lech <=2%");
        println!("RPC_ERRORS trong lan chay nay: -32005={} khac={}", stats.n_32005(), stats.n_other());
        if replayed < 3 {
            println!(
                "SKIP (khong phai FAIL): chi replay duoc {replayed} bo (<3 theo lenh), -32005={} \
(=> phan biet duoc: {total_logs} log da doc THANH CONG tren {blocks_scanned} block, \
{} bo sandwich THAT da tim thay co hash — co che phat hien DUNG, thieu la do MAT DO du lieu + cua so state ~128 block)",
                stats.n_32005(),
                confirmed.len()
            );
            return;
        }
        assert!(within_2pct >= 3, "chi {within_2pct}/{replayed} bo lech <=2% (can >=3)");
    }

    /// Cụm `evm-validate-wire-tax` B4'.4 — ternary search `front_in` bằng
    /// EVM THẬT (6-8 vòng quanh ước lượng `sim_v2` ±50%), mỗi vòng 2 lần sim
    /// trên CÙNG 1 `ForkEvm` đã warm (fork/fetch account info+bytecode CHỈ 1
    /// LẦN). Trước khi dùng slot 8 để reset reserve, VERIFY layout đó khớp
    /// `eth_getStorageAt` THẬT (không giả định suông). Đo thời gian trung
    /// bình 1 lần sim (ms) — số liệu, không phải assertion pass/fail.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_rpc_ternary_search_evm_warm_cache_timing() {
        use crate::pool;
        use crate::sim_v2;
        use alloy::providers::{Provider, ProviderBuilder};
        use std::str::FromStr as _;

        let v2_router = router();
        let provider: DynProvider = ProviderBuilder::new()
            .connect("https://bsc-dataseed.binance.org/")
            .await
            .expect("ket noi RPC cong khai that bai")
            .erased();
        assert_eq!(provider.get_chain_id().await.expect("eth_chainId that bai"), 56);

        let candidates = poll_wbnb_v2_candidates(&provider, v2_router, 1, 40).await;
        if candidates.is_empty() {
            println!("SKIP (khong phai FAIL): khong tim thay candidate nao trong mempool sau 40 lan poll - thu lai sau");
            return;
        }
        let (victim, token) = &candidates[0];

        let factory = Address::from_str(crate::venues::V2_FACTORY_ADDRESS).unwrap();
        let pair = match pool::resolve_v2_pair(&provider, factory, *token).await {
            Ok(Ok(p)) => p,
            other => {
                println!("SKIP (khong phai FAIL): resolve_v2_pair that bai/khong co pool ({other:?})");
                return;
            }
        };
        let (token0, raw_reserve0, raw_reserve1) =
            pool::get_raw_reserves_and_token0(&provider, pair).await.expect("get_raw_reserves_and_token0 that bai");

        // VERIFY layout slot 8 (reserve0|reserve1|blockTimestampLast packed)
        // bang eth_getStorageAt THAT - khong tin suong theo doc UniswapV2Pair.sol.
        let slot8_raw = provider.get_storage_at(pair, U256::from(8u64)).await.expect("eth_getStorageAt slot 8 that bai");
        let (unpacked_r0, unpacked_r1, block_timestamp_last) = unpack_v2_reserves_slot(slot8_raw);
        println!(
            "VERIFY slot8: pair={pair:#x} eth_getStorageAt(8)={slot8_raw:#x} -> unpack(reserve0={unpacked_r0}, reserve1={unpacked_r1}, ts={block_timestamp_last}) vs getReserves() THAT (reserve0={raw_reserve0}, reserve1={raw_reserve1})"
        );
        if unpacked_r0 != raw_reserve0 || unpacked_r1 != raw_reserve1 {
            println!("SKIP (khong phai FAIL): layout slot 8 KHONG khop pair {pair:#x} (co the pair nay dung storage layout khac) - khong dung reset-by-slot cho pair nay, thu candidate khac");
            return;
        }
        println!("=> slot 8 KHOP CHINH XAC voi getReserves() that - dung duoc de reset state giua cac lan thu front_in");

        let (reserve_wbnb, reserve_token) = if token0 == wbnb() { (raw_reserve0, raw_reserve1) } else { (raw_reserve1, raw_reserve0) };
        let reserves = sim_v2::PoolReserves { reserve_wbnb, reserve_token };
        let max_front = U256::from(1_500_000_000_000_000_000u128);
        let quote_v2 = sim_v2::search_max_front_in(reserves, victim.value, max_front, 0).expect("search_max_front_in phai co quote");
        println!("sim_v2 uoc luong: front_in={} profit={}", quote_v2.front_in, quote_v2.profit_wei);

        let fork_block = provider.get_block_number().await.expect("eth_blockNumber that bai lan 2");
        // PHAT HIEN THAT (lan chay dau khi co RPC song): khong phai MOI
        // token deu co mapping `balanceOf` nam trong slot 0..20 chuan (vd
        // token ke thua nhieu contract cha, dat state var truoc balances) -
        // `probe_erc20_balance_slot` tra loi that (Err ro rang), KHONG doan
        // bua. Coi day la SKIP candidate (dung dinh dang "SKIP" nhu cac test
        // khac trong cum nay), KHONG panic ca test - dung dung "khong
        // candidate nao" thi moi la CHUA XONG that su.
        let result = match refine_front_in_with_evm(
            provider.clone(),
            fork_block,
            *token,
            token0,
            pair,
            raw_reserve0,
            raw_reserve1,
            block_timestamp_last,
            victim,
            quote_v2.front_in,
            max_front,
            8,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                println!("SKIP (khong phai FAIL): refine_front_in_with_evm loi cho token {token:#x} ({e}) - co the token nay khong dung layout storage chuan, thu lai voi candidate khac");
                return;
            }
        };

        println!("balance_slot (mapping balanceOf) do duoc = {}", result.balance_slot);
        for (i, a) in result.attempts.iter().enumerate() {
            match &a.result {
                Ok(o) => println!("  attempt#{i} front_in={} profit_evm={} victim_success={} thoi_gian={:.2}ms", a.front_in, o.profit_wei, o.victim_success, a.duration_ms),
                Err(e) => println!("  attempt#{i} front_in={} LOI={e} thoi_gian={:.2}ms", a.front_in, a.duration_ms),
            }
        }
        println!("TONG B4'.4: {} lan sim, trung binh {:.2}ms/lan", result.attempts.len(), result.avg_duration_ms);
        match &result.best {
            Some(b) => println!("BEST: front_in={} profit_evm={} (so voi sim_v2 uoc luong front_in={} profit={})", b.front_in, b.profit_wei, quote_v2.front_in, quote_v2.profit_wei),
            None => println!("BEST: khong co attempt nao thanh cong"),
        }
        assert!(!result.attempts.is_empty(), "phai co it nhat 1 attempt");
    }

    /// Cụm `evm-validate-fixed-then-wire` (B3.2 + C1 + B3.4 deliverable) —
    /// từ mempool/block sống: chạy `BlockForkCache` (fork dùng chung), đo
    /// `sim.evm` (front_in/profit/tax + ms), đo tax bằng `measure_tax_evm`
    /// (C1), và cho vài victim vào CÙNG 1 fork để đo ms/tx SAU khi warm
    /// (mục tiêu ≤50ms/tx sau tx đầu). In ≥3 dòng `sim.evm` mẫu.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_rpc_sim_evm_block_cache_and_tax() {
        use crate::pool;
        use alloy::providers::{Provider, ProviderBuilder};
        use std::str::FromStr as _;

        let stats = RpcErrorStats::default();
        let urls = validate_rpc_urls();
        let provider: DynProvider =
            ProviderBuilder::new().connect(&urls[0]).await.expect("ket noi RPC that bai").erased();
        assert_eq!(provider.get_chain_id().await.expect("chain_id"), 56);

        let latest = provider.get_block_number().await.expect("block_number");
        let routers: Vec<Address> =
            crate::venues::PANCAKE_ROUTERS.iter().filter_map(|(a, _)| Address::from_str(a).ok()).collect();
        let candidates = collect_mined_v2_buy_candidates(
            &provider,
            latest.saturating_sub(20),
            latest,
            &routers,
            U256::from(50_000_000_000_000_000u128),
            8,
            &stats,
        )
        .await;
        println!("thu thap {} candidate cho sim.evm block-cache", candidates.len());
        if candidates.is_empty() {
            println!("SKIP (khong phai FAIL): 0 candidate >=0.05 BNB trong 20 block gan nhat");
            return;
        }

        let factory = Address::from_str(crate::venues::V2_FACTORY_ADDRESS).unwrap();
        let fork_block = provider.get_block_number().await.expect("block_number 2");
        let mut fork = match BlockForkCache::open(provider.clone(), fork_block).await {
            Ok(f) => f,
            Err(e) => {
                println!("SKIP (khong phai FAIL): mo fork loi ({e})");
                return;
            }
        };

        let max_front = U256::from(1_500_000_000_000_000_000u128);
        let mut printed = 0u32;
        for (victim, token, _b) in &candidates {
            let (t0, r0, r1) = match pool::get_raw_reserves_and_token0(&provider, {
                match pool::resolve_v2_pair(&provider, factory, *token).await {
                    Ok(Ok(p)) => p,
                    _ => continue,
                }
            })
            .await
            {
                Ok(v) => v,
                Err(_) => continue,
            };
            let (rq, rt) = if t0 == wbnb() { (r0, r1) } else { (r1, r0) };
            let reserves = crate::sim_v2::PoolReserves { reserve_wbnb: rq, reserve_token: rt };
            let est = match crate::sim_v2::search_max_front_in(reserves, victim.value, max_front, 0) {
                Some(q) => q.front_in,
                None => continue,
            };
            match refine_front_in_on_fork(&mut fork, *token, victim, est, max_front) {
                Ok((o, tried, ms)) => {
                    println!(
                        "sim.evm token={token:#x} front_in={} token_received={} back_out={} profit_evm={} victim_success={} buy_tax_bps={:?} sell_tax_bps={:?} attempts={tried} tong_ms={ms:.2} ms/attempt={:.2}",
                        o.front_in, o.token_received, o.back_out, o.profit_wei, o.victim_success, o.buy_tax_bps, o.sell_tax_bps, ms / tried.max(1) as f64
                    );
                    printed += 1;
                }
                Err(e) => println!("sim.evm token={token:#x} LOI={e}"),
            }
            // C1 - do tax rieng bang measure_tax_evm tren fork moi (doc lap).
            match measure_tax_evm(provider.clone(), fork_block, *token, wbnb(), U256::from(50_000_000_000_000_000u128)).await {
                Ok(m) => println!("  tax_evm token={token:#x} buy_bps={} sell_bps={} honeypot={}", m.buy_bps, m.sell_bps, m.honeypot),
                Err(e) => println!("  tax_evm token={token:#x} LOI={e}"),
            }
            if printed >= 5 {
                break;
            }
        }
        println!("in {printed} dong sim.evm. RPC_ERRORS: -32005={} khac={}", stats.n_32005(), stats.n_other());
        assert!(printed >= 1, "phai in duoc it nhat 1 dong sim.evm (mempool song)");
    }

    /// Cụm `truth-victim-ok-and-memleak` (mục 1) — THÍ NGHIỆM PHÂN ĐỊNH
    /// `victim_ok=false`, chạy DÀY trên victim THẬT thay vì chờ shadow ký được
    /// bundle (RUN 4 chỉ ra 4 mẫu / 30 phút — không đủ ≥10 mẫu lệnh yêu cầu).
    ///
    /// **Vì sao lấy victim ĐÃ ĐÀO thay vì mempool**: tx đã đào cho biết
    /// SỰ THẬT ngoài đời (`status`, `amountOut` thật) để đối chiếu, và fork tại
    /// `mined_block - 1` là ĐÚNG state mà bot nhìn thấy lúc tx còn pending —
    /// y hệt `fork_block` mà `spawn_shadow_bundle_sim` dùng. Điều kiện duy
    /// nhất: `mined_block - 1` phải còn trong cửa sổ state (~128 block) của
    /// node, nên chỉ quét các block SÁT đỉnh.
    ///
    /// 4 bundle RUN 4 (BAOCAO43) KHÔNG fork lại được: block 122169515/
    /// 122169998/122171061/122171147 đã quá sâu, MỌI RPC trong `.env` trả
    /// `missing trie node`/`not supported`/`Archive requests require a personal
    /// token` — đo thật, dán ở BAOCAO44 ô 5. Với 4 hash đó dùng phân tích
    /// KHÔNG cần archive (`status`, funding trong block, `amountOutMin` vs
    /// `amountOut` thật).
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn real_rpc_victim_ok_verdict_ladder() {
        use alloy::consensus::Transaction as _;
        use alloy::network::TransactionResponse as _;
        use alloy::providers::ProviderBuilder;
        use alloy::rpc::types::TransactionRequest;

        let want: usize = std::env::var("LADDER_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(10);
        let mut urls: Vec<String> = Vec::new();
        if let Ok(v) = std::env::var("BSC_HTTP_SIM") {
            urls.extend(v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
        }
        urls.extend(validate_rpc_urls());
        println!("real_rpc_victim_ok_verdict_ladder: {} URL ung vien, muc tieu {want} case", urls.len());

        // Chon node DUY NHAT vua co tx da dao vua con state block-1 (2 dieu
        // kien khac nhau - node "not supported" o do sau 1 block van tra tx).
        let mut chosen: Option<(String, DynProvider, u64)> = None;
        for u in &urls {
            let Ok(p) = ProviderBuilder::new().connect(u).await else { continue };
            let p = p.erased();
            let Ok(head) = p.get_block_number().await else { continue };
            // Doc thu 1 storage slot tai head-3: node nao khong giu state se loi.
            let probe = TransactionRequest::default()
                .to(crate::venues::wbnb_addr())
                .input(alloy::hex::decode("18160ddd").unwrap().into());
            match p.call(probe).block(BlockId::number(head - 3)).await {
                Ok(_) => {
                    println!("  node GIU state head-3: {} (head={head})", crate::transport::redact_rpc_url(u));
                    chosen = Some((u.clone(), p, head));
                    break;
                }
                Err(e) => println!("  node KHONG giu state head-3: {} -> {e}", crate::transport::redact_rpc_url(u)),
            }
        }
        let Some((url, provider, head)) = chosen else {
            println!("MISSING: khong RPC nao trong .env giu state o do sau 3 block -> khong chay duoc thi nghiem");
            return;
        };
        println!("== dung node {} , head={head} ==", crate::transport::redact_rpc_url(&url));

        let router = Address::from_str(crate::venues::V2_ROUTER_ADDRESS).unwrap();
        let factory = Address::from_str(crate::venues::V2_FACTORY_ADDRESS).unwrap();
        let usdt = crate::venues::usdt_addr();
        let wbnb_a = wbnb();

        let mut rows_printed = 0usize;
        let mut verdicts: std::collections::BTreeMap<&'static str, usize> = Default::default();
        let mut reasons: std::collections::BTreeMap<String, usize> = Default::default();
        let mut scanned_blocks = 0u64;
        let mut n_gated_total = 0usize;
        let mut n_gated_victim_ok = 0usize;
        let mut gated_reasons: std::collections::BTreeMap<String, usize> = Default::default();
        // Luon BAM DINH dinh chain: xu ly block `head-1` va fork tai `head-2`.
        // Neu quet lui ve qua khu thi do sau fork tang dan va vuot cua so state
        // (~128 block) -> thi nghiem chet giua chung. Doi block moi thay vi lui.
        let mut b = head - 1;
        let t_start = std::time::Instant::now();
        let budget = std::time::Duration::from_secs(
            std::env::var("LADDER_BUDGET_SEC").ok().and_then(|v| v.parse().ok()).unwrap_or(900),
        );
        // Nguong "co y nghia kinh te" - bo dust, vi front_in toi uu cho dust
        // cung la dust va khong tra loi duoc cau hoi nao.
        let min_in_usdt = U256::from(100u64) * U256::from(10u64).pow(U256::from(18u64));
        let min_in_wbnb = U256::from(10u64).pow(U256::from(17u64)); // 0.1 BNB

        println!();
        println!("{:<20} {:<5} {:>14} {:>16} {:>12} {:>8} | {:>5} {:>5} {:>5} {:>5} {:>5} {:>8} | {:>16} {:>12} {}",
            "victim_hash", "quote", "amountOutMin", "front_in_v2", "impact_%", "mined",
            "a:0", "b:v2", "c50", "c25", "c10", "d:gated", "front_gated", "profit_gated", "verdict/reason");

        while rows_printed < want && t_start.elapsed() < budget {
            // Cho block MOI (khong lui ve qua khu).
            let now_head = provider.get_block_number().await.unwrap_or(b);
            if now_head < b + 1 {
                tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                continue;
            }
            b = now_head - 1;
            scanned_blocks += 1;
            let blk = match provider.get_block_by_number(alloy::eips::BlockNumberOrTag::Number(b)).full().await {
                Ok(Some(x)) => x,
                _ => continue,
            };
            let txs: Vec<_> = blk.transactions.txns().cloned().collect();
            for tx in txs {
                if rows_printed >= want { break }
                if tx.to() != Some(router) { continue }
                let Ok(dec) = crate::decoder::decode_swap_calldata(tx.input(), tx.value()) else { continue };
                let quote_in = dec.path.token_a;
                if quote_in != usdt && quote_in != wbnb_a { continue }
                let Ok(token) = dec.token() else { continue };
                if dec.amount_in < if quote_in == usdt { min_in_usdt } else { min_in_wbnb } { continue }

                // Reserve tai block-1 (dung state ma bot nhin thay luc pending).
                let at = BlockId::number(b - 1);
                let gp = crate::pool::build_get_pair_calldata(token, quote_in);
                let Ok(ret) = provider.call(TransactionRequest::default().to(factory).input(gp.into())).block(at).await else { continue };
                let Some(pair) = crate::pool::decode_address_return(&ret) else { continue };
                if pair == Address::ZERO { continue }
                let Ok(t0ret) = provider.call(TransactionRequest::default().to(pair).input(alloy::hex::decode("0dfe1681").unwrap().into())).block(at).await else { continue };
                let Some(token0) = crate::pool::decode_address_return(&t0ret) else { continue };
                let Ok(rret) = provider.call(TransactionRequest::default().to(pair).input(alloy::hex::decode("0902f1ac").unwrap().into())).block(at).await else { continue };
                if rret.len() < 64 { continue }
                let r0 = U256::from_be_slice(&rret[0..32]);
                let r1 = U256::from_be_slice(&rret[32..64]);
                let (rq, rt) = if token0 == quote_in { (r0, r1) } else { (r1, r0) };
                if rq.is_zero() || rt.is_zero() { continue }

                let cap = if quote_in == usdt {
                    U256::from(3000u64) * U256::from(10u64).pow(U256::from(18u64))
                } else {
                    U256::from(5u64) * U256::from(10u64).pow(U256::from(18u64))
                };
                let reserves = crate::sim_v2::PoolReserves { reserve_wbnb: rq, reserve_token: rt };
                let Some(q) = crate::sim_v2::search_max_front_in(reserves, dec.amount_in, cap, 0) else { continue };
                // Bo case front_in dust (khong do duoc price impact co nghia).
                if q.front_in < if quote_in == usdt { min_in_usdt } else { min_in_wbnb } { continue }

                // Cum `truth-victim-ok-and-memleak` (muc 2) - mUC `front_in` MA
                // BOT THAT SU CHON sau khi sua: co rang buoc victim phai song
                // theo V2-math. Day moi la so can doi chieu voi EVM; muc tho
                // `b:front=v2` giu lai lam DOI CHUNG (truoc khi sua).
                let gated = crate::sim_v2::search_max_front_in_victim_ok(
                    reserves, dec.amount_in, cap, 0, dec.amount_out_min,
                );
                let mut variants = victim_diag_ladder(q.front_in);
                variants.push((
                    "d:front=v2_gated".to_string(),
                    gated.map(|g| g.front_in).unwrap_or(U256::ZERO),
                ));

                let victim_raw = crate::transport::pending_tx_from_rpc(&tx);
                let rep = match diagnose_victim_ok_variants(provider.clone(), b - 1, token, quote_in, &victim_raw, &variants, None).await {
                    Ok(r) => r,
                    Err(e) => { println!("  {} fork loi: {e}", &format!("{:#x}", tx.tx_hash())[..18]); continue }
                };

                let get = |lbl: &str| rep.rows.iter().find(|r| r.label == lbl);
                let fl = |lbl: &str| match get(lbl) {
                    Some(r) if r.err.is_some() => "ERR".to_string(),
                    Some(r) => (if r.victim_ok { "OK" } else { "X" }).to_string(),
                    None => "-".to_string(),
                };
                let impact = q.front_in.to::<u128>() as f64 / rq.to::<u128>() as f64 * 100.0;
                let best = rep.rows.iter().filter(|r| r.victim_ok && r.err.is_none() && !r.front_in.is_zero())
                    .max_by_key(|r| r.front_in);
                let profit_at_max = best.map(|r| r.profit_wei as f64 / 1e18).unwrap_or(0.0);
                // Cum `truth-victim-ok-and-memleak`: "ERR" la loi o CHAN CUA TA
                // (fork/front/back revert), KHAC han victim revert - phai in ra
                // chu khong duoc gom chung vao mot chu "ERR" khong tra cuu duoc.
                let reason = rep
                    .rows
                    .iter()
                    .find(|r| r.label == "b:front=v2")
                    .map(|r| match (&r.victim_revert_reason, &r.err) {
                        (_, Some(e)) => format!("SIM_ERR: {}", e.chars().take(110).collect::<String>()),
                        (Some(v), None) => v.clone(),
                        (None, None) => "-".to_string(),
                    })
                    .unwrap_or_else(|| "-".to_string());
                *verdicts.entry(rep.verdict).or_default() += 1;
                *reasons.entry(reason.clone()).or_default() += 1;

                let gated_row = get("d:front=v2_gated");
                let gated_front = gated_row.map(|r| r.front_in.to::<u128>() as f64 / 1e18).unwrap_or(0.0);
                let gated_profit = gated_row.map(|r| r.profit_wei as f64 / 1e18).unwrap_or(0.0);
                if let Some(r) = gated_row {
                    if !r.front_in.is_zero() {
                        n_gated_total += 1;
                        if r.victim_ok && r.err.is_none() {
                            n_gated_victim_ok += 1;
                        }
                        match (&r.victim_revert_reason, &r.err) {
                            (_, Some(e)) => {
                                *gated_reasons
                                    .entry(format!("SIM_ERR: {}", e.chars().take(110).collect::<String>()))
                                    .or_default() += 1
                            }
                            (Some(v), None) => *gated_reasons.entry(v.clone()).or_default() += 1,
                            (None, None) => {}
                        }
                    }
                }
                let _ = profit_at_max;
                println!("{:<20} {:<5} {:>14.4} {:>16.4} {:>12.4} {:>8} | {:>5} {:>5} {:>5} {:>5} {:>5} {:>8} | {:>16.6} {:>12.6} {} | {}",
                    &format!("{:#x}", tx.tx_hash())[..18],
                    if quote_in == usdt { "usdt" } else { "wbnb" },
                    dec.amount_out_min.to::<u128>() as f64 / 1e18,
                    q.front_in.to::<u128>() as f64 / 1e18,
                    impact, b,
                    fl("a:front=0"), fl("b:front=v2"), fl("c:50%"), fl("c:25%"), fl("c:10%"), fl("d:front=v2_gated"),
                    gated_front, gated_profit, rep.verdict, reason);
                rows_printed += 1;
            }
        }

        println!();
        println!("== TONG KET {rows_printed} case / {scanned_blocks} block (node {} , tu block {} toi {b}) ==",
            crate::transport::redact_rpc_url(&url), head - 1);
        for (v, n) in &verdicts { println!("   verdict {v:<20} = {n}"); }
        for (r, n) in &reasons { println!("   ly do revert (b:front=v2) {r:<50} = {n}"); }
        println!();
        println!("== MUC 2: front_in DA RANG BUOC victim-ok (d:front=v2_gated) ==");
        println!("   so case co front_gated > 0      = {n_gated_total}");
        println!("   trong do victim SONG trong EVM  = {n_gated_victim_ok}");
        for (r, n) in &gated_reasons { println!("   ly do revert con lai {r:<50} = {n}"); }
        assert!(rows_printed >= 1, "phai phan dinh duoc it nhat 1 case (khong duoc pass rong - luat #3)");
    }

    /// Cụm `verify-cluster-as-victim` (mục 2) — CHỨNG MINH TRÊN CHAIN THẬT
    /// rằng `victim_ok=false` của ví burner cụm đối thủ là ARTIFACT của phép
    /// đo, không phải sự thật kinh tế.
    ///
    /// Cách chạy (KHÔNG cần biết trước hash nào): quét lùi từ block mới nhất,
    /// tìm block có `Transfer(USDT)` **từ** 1 trong 3 seed tới một địa chỉ X,
    /// rồi tìm trong CHÍNH block đó một tx do X gửi tới V2 Router. Đó đúng là
    /// mẫu hình "cấp vốn + swap trong cùng block". Với mẫu tìm được, chạy
    /// `simulate_sandwich_quote_topup` tại `mined_block − 1` HAI LẦN:
    ///
    ///   (a) `victim_topup = None`  -> kỳ vọng `TRANSFER_FROM_FAILED`
    ///       (ví X chưa có USDT nào ở cuối block trước)
    ///   (b) `victim_topup = Some(amount_in)` -> kỳ vọng victim SỐNG
    ///
    /// Test in số THẬT (block, địa chỉ, `amount_in`, lãi mô phỏng) — luật #3
    /// AGENTS.md, không được "pass rỗng". Nếu quét hết ngân sách block mà
    /// không gặp mẫu hình nào thì in `MISSING` và KHÔNG assert (cụm đối thủ có
    /// thể đã ngừng/đổi cách hoạt động — đó là dữ kiện, không phải lỗi code).
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_rpc_cluster_burner_victim_song_lai_khi_duoc_nap_von() {
        use alloy::providers::{Provider, ProviderBuilder};
        use std::str::FromStr as _;

        let urls = crate::transport::filter_read_urls(crate::transport::collect_rpc_urls_from_env("BSC_HTTP_SIM"));
        let url = match urls.first() {
            Some(u) => u.clone(),
            None => {
                println!("MISSING: BSC_HTTP_SIM rong - khong chay duoc test nay");
                return;
            }
        };
        let provider: DynProvider = ProviderBuilder::new().connect(&url).await.expect("ket noi RPC").erased();
        assert_eq!(provider.get_chain_id().await.unwrap(), 56, "phai la BSC chain 56");
        let head = provider.get_block_number().await.expect("eth_blockNumber");
        let usdt = Address::from_str(crate::venues::USDT_ADDRESS).unwrap();
        let v2_router = router();
        let seeds = crate::competitor::seed_set();
        let transfer_topic = crate::competitor::transfer_topic0();

        // Ngan sach quet (block), doi duoc bang `CLUSTER_SCAN_BLOCKS`. Mac dinh
        // 400: do that o phien nay la ~1 mau hinh moi 55 block, nen 400 cho
        // bien an toan ma van chi ~3 phut chuoi. Moi block 1 `eth_getLogs`, va
        // chi block CO log seed moi ton them 1 `eth_getBlockByNumber` full.
        let budget: u64 = std::env::var("CLUSTER_SCAN_BLOCKS").ok().and_then(|v| v.parse().ok()).unwrap_or(400);
        let mut found: Option<(u64, Address, PendingTxRaw, U256)> = None;
        let mut scanned = 0u64;
        for b in (head.saturating_sub(budget + 2)..=head.saturating_sub(2)).rev() {
            scanned += 1;
            let logs = match provider
                .get_logs(
                    &alloy::rpc::types::Filter::new()
                        .from_block(b)
                        .to_block(b)
                        .address(usdt)
                        .event_signature(transfer_topic),
                )
                .await
            {
                Ok(l) => l,
                Err(_) => continue,
            };
            let mut funded: std::collections::HashSet<Address> = std::collections::HashSet::new();
            for l in &logs {
                let (Some(from_t), Some(to_t)) = (l.topics().get(1), l.topics().get(2)) else { continue };
                if seeds.contains(&crate::competitor::address_from_topic(*from_t)) {
                    funded.insert(crate::competitor::address_from_topic(*to_t));
                }
            }
            if funded.is_empty() {
                continue;
            }
            let Ok(Some(blk)) = provider
                .get_block_by_number(alloy::eips::BlockNumberOrTag::Number(b))
                .full()
                .await
            else {
                continue;
            };
            let txs = match blk.transactions.as_transactions() {
                Some(t) => t,
                None => continue,
            };
            for tx in txs {
                use alloy::consensus::Transaction as _;
                use alloy::network::TransactionResponse as _;
                if tx.to() != Some(v2_router) || !funded.contains(&tx.from()) {
                    continue;
                }
                let raw = transport_pending_tx_from_alloy(tx);
                let Ok(dec) = crate::decoder::decode_swap_calldata(&raw.input, raw.value) else { continue };
                // Chi lay chieu MUA bang USDT (dau vao path la quote USDT).
                if dec.path.token_a != usdt {
                    continue;
                }
                found = Some((b, dec.path.token_b, raw, dec.amount_in));
                break;
            }
            if found.is_some() {
                break;
            }
        }

        let Some((mined_block, token, victim, amount_in)) = found else {
            println!(
                "MISSING: quet {scanned} block lui tu {head} khong gap mau hinh \
                 'seed cap von USDT + vi do swap qua V2 Router trong CUNG block'"
            );
            return;
        };
        let fork_block = mined_block - 1;
        println!(
            "MAU THAT: block dao={mined_block} fork={fork_block} victim={:#x} from={:#x} token={token:#x} amount_in={} USDT",
            victim.hash,
            victim.from,
            amount_in.to::<u128>() as f64 / 1e18
        );

        // front_in nho, co dinh (10 USDT) - test nay do CO CHE, khong do lai.
        let front_in = U256::from(10u64) * U256::from(10u64).pow(U256::from(18u64));

        let khong_nap =
            simulate_sandwich_quote_topup(provider.clone(), fork_block, front_in, token, usdt, &victim, None)
                .await
                .expect("sim (khong nap) phai chay duoc");
        println!(
            "  (a) KHONG nap von: victim_ok={} reason={:?} so_du_quote_cua_victim_tai_fork={} topped_up={}",
            khong_nap.victim_success,
            khong_nap.victim_revert_reason,
            khong_nap.victim_quote_balance_before,
            khong_nap.victim_quote_topped_up
        );

        let co_nap = simulate_sandwich_quote_topup(
            provider.clone(),
            fork_block,
            front_in,
            token,
            usdt,
            &victim,
            Some(amount_in),
        )
        .await
        .expect("sim (co nap) phai chay duoc");
        println!(
            "  (b) CO nap von:    victim_ok={} reason={:?} topped_up={} victim_out={} profit_sim_usdt={:.6} allowance={}",
            co_nap.victim_success,
            co_nap.victim_revert_reason,
            co_nap.victim_quote_topped_up,
            co_nap.victim_out,
            co_nap.profit_wei as f64 / 1e18,
            co_nap.victim_quote_allowance
        );

        assert!(
            khong_nap.victim_quote_balance_before < amount_in,
            "mau hinh phai la 'vi chua co du USDT tai block truoc' - neu khong, test nay khong chung minh gi"
        );
        assert!(!khong_nap.victim_success, "khong nap von -> victim PHAI hong (day la artifact can chung minh)");
        assert!(co_nap.victim_quote_topped_up, "(b) phai thuc su nap von");
        assert!(
            co_nap.victim_success,
            "nap dung so USDT victim sap tieu -> victim PHAI song; neu van hong, xem allowance = {}",
            co_nap.victim_quote_allowance
        );
    }
}

use alloy::primitives::U256;
use serde::Deserialize;
use std::fmt;
use std::path::Path;
use std::time::{Duration, Instant};

pub const REQUIRED_CHAIN_ID: u64 = 56;
const WEI_PER_BNB: f64 = 1_000_000_000_000_000_000.0;

/// Toàn bộ field bắt buộc theo CLAUDE.md mục "Config — thiếu field = fail load".
/// Không có field nào mang `#[serde(default)]` — thiếu field trong config.toml
/// khiến `toml::from_str` trả lỗi ngay, đúng luật "thiếu field = fail load".
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub chain_id: u64,
    pub dry_run: bool,
    pub allow_live: bool,
    pub bot_armed: bool,

    pub scan_v2: bool,
    pub scan_v3: bool,
    pub scan_v4: bool,

    pub live_v2: bool,
    pub live_v3: bool,
    pub live_v4: bool,

    pub min_profit_bnb: f64,
    pub max_front_bnb: f64,
    pub min_reserve_wbnb: f64,

    pub victims_path: String,
    pub victims_reload_sec: u64,
    /// Cụm config-hot-reload (phiên này) — hot-reload `config.toml`, cùng cơ
    /// chế `reload_if_due` như `VictimBook` (`src/victims.rs`). Ship = 15
    /// (khớp `victims_reload_sec`).
    pub config_reload_sec: u64,

    /// Cụm `5.2` — cadence (ms) `main.rs::poll_txpool_pending` gọi
    /// `txpool_content` khi fallback từ WSS (không có `BSC_WS` hoặc
    /// subscription rớt). Ship `400`. KHÔNG liên quan
    /// `victims_reload_sec`/`config_reload_sec` (đó là hot-reload NGƯỠNG,
    /// đây là cadence đọc mempool) — vẫn là field bắt buộc trong
    /// `config.toml` vì chủ có thể cần chỉnh nếu RPC node rate-limit.
    pub pending_poll_ms: u64,

    /// Cụm `5.3` — số hash pending-tx MỚI (chưa `seen`) tối đa xử lý mỗi vòng
    /// poll `txpool_content` (`main.rs::poll_txpool_pending`). Ship `32`.
    /// `txpool_content` trả TOÀN BỘ pool đang chờ mỗi lần gọi — không giới
    /// hạn sẽ khiến 1 vòng poll spawn hàng trăm/ngàn `handle_paper_tx` cùng
    /// lúc khi mempool đông (đã thấy `decode_fail=856` trong ~10s ở BAOCAO08).
    /// Field bắt buộc (không `#[serde(default)]`) — thiếu = fail load, cùng
    /// khuôn `pending_poll_ms`.
    pub pending_txpool_max_per_poll: u32,

    pub gas_reserve_bnb_wei: u64,
    pub front_max_gas_bnb_wei: u64,
    pub back_max_gas_bnb_wei: u64,

    pub tx_timeout_sec: u64,
    pub ws_silence_sec: u64,
    pub max_consecutive_loss: u32,
    pub max_exposure_bnb: f64,

    pub web_bind: String,
    pub web_port: u16,

    pub max_roundtrip_tax: f64,
    pub tax_cache_blocks: u32,

    /// Cụm `7.3` (BAOCAO16) — buffer giây cộng vào thời điểm build tx paper
    /// (`executor::compute_deadline`) để ra `deadline` cho router V2
    /// (`swapExactETHForTokens`/`swapExactTokensForETH`). Dùng wall-clock
    /// (`SystemTime::now()`), KHÔNG phải `block.timestamp` on-chain thật (hàm
    /// build tx thuần/sync, không có `Provider` — xem docs/STATE.md mục
    /// "7.3"). Field bắt buộc, ship `120` (2 phút, đủ dư so block time BSC
    /// ~3s/block).
    pub executor_deadline_buffer_sec: u64,

    /// Cụm pair-mode — đường dẫn `pairs.txt` (`PairBook`), cùng khuôn
    /// `victims_path`. Field bắt buộc (thiếu = fail load).
    pub pairs_path: String,
    /// Chu kỳ hot-reload `pairs.txt` (giây) — cùng cơ chế
    /// `PairBook::reload_if_due`/`victims_reload_sec`.
    pub pairs_reload_sec: u64,
    /// Ngưỡng `min_swap` GLOBAL (BNB) áp dụng cho MỌI pool trong `pairs.txt`
    /// (khác `victims.txt` nơi mỗi ví có min riêng) — chủ chỉnh tự do trong
    /// `config.toml`, hot-reload qua `config_reload_sec`.
    pub pairs_min_swap_bnb: f64,

    /// Cụm `universal-pair-scan` — bật chế độ quét MỌI pool WBNB thay vì chỉ
    /// pool đã khai trong `pairs.txt`. Ship `false` (AN TOÀN — hành vi giữ
    /// NGUYÊN y hệt trước phiên này: tx không khớp wallet-mode lẫn
    /// `PairBook` vẫn là `not_in_list`). Khi `true`,
    /// `pipeline::decide_paper_v2` coi tx không khớp cả 2 mode trên là
    /// candidate `source="universal"`, dùng LẠI đúng ngưỡng GLOBAL
    /// `pairs_min_swap_bnb` (không có ngưỡng riêng cho universal). Field bắt
    /// buộc (thiếu = fail load, cùng khuôn mọi field khác) — chủ phải tự bật
    /// trong `config.toml`, Code không tự đổi mặc định thành `true`.
    pub pair_scan_universal: bool,

    /// Cụm `explicit-mode-flags` (phiên này) — cờ TƯỜNG MINH bật/tắt nhánh
    /// wallet-mode trong `decide_paper_v2`/`decide_and_build_paper_v2`.
    /// Ship `true` BẮT BUỘC: đây là hành vi GỐC đang chạy thật dựa trên
    /// `victims.txt` từ trước phiên này — nếu ship `false` sẽ vô tình TẮT bot
    /// đang vận hành ngay khi chủ cập nhật `config.toml`/`Cargo.lock` (cấm
    /// tuyệt đối theo lệnh gốc). `false` -> `decide_paper_v2` bỏ HẲN nhánh
    /// wallet (kể cả khi `from` khớp `victims.txt`), rơi xuống pair/universal/
    /// `not_in_list` theo đúng thứ tự ưu tiên cũ. Field bắt buộc (thiếu = fail
    /// load, cùng khuôn mọi field khác) — chủ tự tắt/bật trong `config.toml`,
    /// hot-reload qua `config_reload_sec`, không cần build/restart.
    pub wallet_scan_enabled: bool,

    /// Cụm `explicit-mode-flags` (phiên này) — cờ TƯỜNG MINH bật/tắt nhánh
    /// pair-mode (`PairBook`, `pairs.txt`) trong `decide_paper_v2`. Ship
    /// `true` BẮT BUỘC (cùng lý do `wallet_scan_enabled` — hành vi gốc dựa
    /// trên `pairs.txt` đang chạy thật, không được đổi default thành `false`).
    /// `false` -> nhánh pair bị bỏ HẲN (kể cả khi `pair_addr` khớp
    /// `PairBook`), rơi xuống universal/`not_in_list`. KHÔNG đổi
    /// `pair_scan_universal` (cờ riêng, giữ nguyên tên/default `false` từ
    /// `universal-pair-scan`, BAOCAO20) — 3 cờ độc lập, chủ tự chọn tổ hợp
    /// trong `config.toml`, code KHÔNG ép buộc chỉ-1-mode (đó là việc chủ tự
    /// quản lý, không phải validate/fail-load).
    pub pair_scan_enabled: bool,

    /// Cụm tax-cache-inject (phiên này) — cổng cho phép `POST /api/tax` +
    /// `state/tax_inject.jsonl` ghi vào `TaxCache` lúc bot đang chạy. Ship
    /// `true` (paper, dry_run) để chủ/test điền cache thủ công không cần
    /// build lại — KHÔNG liên quan cổng live (`live_gate_ok`), chỉ kiểm soát
    /// nguồn ghi cache tax, giữ nguyên luật "chưa đo -> honeypot_or_tax" khi
    /// `false` (API/file bị bỏ qua, không panic, không lỗi im lặng — trả rõ
    /// `ok:false` ở web, log `tax.inject_skipped` ở file watcher).
    pub allow_tax_inject: bool,

    /// Cụm `usdt-quote-asset` (BAOCAO29) — bật quét quote asset thứ 2 (USDT)
    /// song song WBNB. Ship `false` (AN TOÀN — hành vi WBNB không đổi gì khi
    /// tắt, đúng CLAUDE.md "scan_quote_usdt=false → hành vi WBNB không đổi
    /// gì"). Field bắt buộc (thiếu = fail load, cùng khuôn mọi field khác).
    pub scan_quote_usdt: bool,

    /// Cụm `usdt-quote-asset` — ngưỡng lợi nhuận tối thiểu cho pool quote
    /// USDT, ĐƠN VỊ USDT (KHÔNG phải BNB, KHÔNG quy đổi/không price oracle —
    /// đúng CLAUDE.md mục Math "profit_usdt = backUSDT - frontUSDT THUẦN").
    /// Hot-reload/validate cùng luật `min_profit_bnb` (âm/không hữu hạn =
    /// fail load, `0` hợp lệ).
    pub min_profit_usdt: f64,

    /// Cụm `usdt-quote-asset` — trần `front_in` (ĐƠN VỊ USDT) cho pool quote
    /// USDT — tương đương `max_front_bnb` nhưng KHÔNG gộp với
    /// `max_exposure_bnb` (field đó là BNB, ngoài phạm vi lệnh USDT lần này).
    pub max_front_usdt: f64,

    /// Cụm `usdt-quote-asset` — ngưỡng thanh khoản tối thiểu (ĐƠN VỊ USDT)
    /// cho pool quote USDT — tương đương `min_reserve_wbnb`.
    pub min_reserve_usdt: f64,

    /// Cụm `evm-validate-fixed-then-wire` (B3.1) — chọn ĐỘNG CƠ quyết định
    /// `Simulated`/`victim_would_revert`/lợi nhuận:
    /// - `"evm"` (ship): fork block hiện tại qua `revm` + `AlloyDB`, chạy
    ///   calldata router THẬT (`sim_evm.rs`). Thấy được fee-on-transfer/
    ///   honeypot/hooks thật — đúng CLAUDE.md mục Math ("quyết định cuối cùng
    ///   dùng EVM THẬT, công thức đóng chỉ ước lượng khoảng `front_in`").
    /// - `"v2"`: giữ NGUYÊN hành vi công thức đóng `sim_v2` như mọi phiên
    ///   trước — đường lùi an toàn nếu RPC không kham nổi tải EVM.
    ///
    /// Giá trị khác 2 chuỗi trên = FAIL LOAD (không im lặng rơi về mặc định —
    /// gõ sai `"EVM"`/`"revm"` phải báo lỗi rõ, không âm thầm chạy sai động cơ).
    /// Hot-reload theo `config_reload_sec` như mọi field khác.
    pub sim_engine: String,

    /// Cụm `evm-validate-fixed-then-wire` (C2) — TTL (giây) cho `TaxCache` đo
    /// tự động bằng `revm`. Thay `tax_cache_blocks` làm ngưỡng tươi/cũ THẬT
    /// SỰ được dùng; `tax_cache_blocks` VẪN đọc được (không bỏ field, không
    /// phá `config.toml` cũ của Chủ) nhưng KHÔNG còn quyết định gì — xem
    /// `tax.rs::TaxCache::get_fresh_ttl`.
    pub tax_cache_ttl_sec: u64,

    /// Cụm `evm-validate-fixed-then-wire` (D1), giờ ĐÃ WIRE thật vào build
    /// (cụm `exec-path-traps` F-08) — slippage riêng cho chân FRONT-BUY
    /// (bps). Thay hẳn `executor_slippage_bps` (field đó đã XOÁ — 2 chân có
    /// rủi ro khác nhau: front-buy chạy TRƯỚC victim nên giá gần như chắc
    /// chắn, chịu được slippage chặt, không nên dùng chung 1 số với back-sell).
    pub front_slippage_bps: u32,

    /// Cụm `evm-validate-fixed-then-wire` (D1) — slippage riêng cho chân
    /// BACK-SELL (bps), nới hơn `front_slippage_bps` vì chân này chạy SAU
    /// victim, giá đã dịch chuyển nhiều hơn và có thể dính tax bán.
    pub back_slippage_bps: u32,

    /// Cụm `strategy-lock-mode2` — chu kỳ (giây) task nền `pairs_vet_task`
    /// đo lại tax/honeypot bằng EVM thật (`sim_evm::measure_tax_evm`) cho
    /// MỌI entry `pairs.txt` đã có `vetted`. Độc lập với đường nóng (không
    /// chặn `handle_paper_tx`). Ship `600`. Field bắt buộc, cùng khuôn mọi
    /// field khác (thiếu = fail load).
    pub pairs_vet_interval_sec: u64,

    /// Cụm `strategy-lock-mode2` — `true` (ship, AN TOÀN): entry `pairs.txt`
    /// KHÔNG có `vetted YYYY-MM-DD` hợp lệ trong comment bị `PairBook::reload`
    /// bỏ qua hẳn (không vào map, log `pair.unvetted` 1 lần/reload) — không
    /// bao giờ thành candidate (`not_in_list`). `false` = quay lại hành vi cũ
    /// (mọi entry resolve được đều thành candidate, bất kể vet) — Chủ tự tắt
    /// nếu muốn, KHÔNG phải mặc định. Field bắt buộc (thiếu = fail load).
    pub pairs_require_vetted: bool,

    /// Cụm `real-economics-mode2` (F-03) — số gas UNIT (KHÔNG phải wei) cho
    /// chân front-buy (`swapExactETHForTokensSupportingFeeOnTransferTokens`),
    /// dùng làm FALLBACK khi chưa đo được thật bằng revm lúc boot
    /// (`main.rs` đo 1 lần trên 1 pair đã vet trong `pairs.txt`, cache vào
    /// `AppStateInner`; đo lỗi thì dùng thẳng field này). Ship `160000` (ước
    /// lượng thực tế 1 swap V2 fee-on-transfer trên BSC). Field bắt buộc
    /// (thiếu = fail load, cùng khuôn mọi field khác).
    pub gas_units_front: u64,

    /// F-03 — như trên nhưng cho chân back-sell
    /// (`swapExactTokensForETHSupportingFeeOnTransferTokens`). Ship `140000`.
    pub gas_units_back: u64,

    /// F-03 — trần `eth_gasPrice` (gwei nguyên) — oracle đọc được gas price
    /// CAO HƠN ngưỡng này (mạng đang tắc nghẽn bất thường) thì coi như
    /// `gas_cap` (an toàn, không đoán gas thật sẽ rẻ lại kịp lúc). Ship `10`
    /// (gwei) — BSC bình thường quanh 1-3 gwei, 10 gwei đã là tắc nghẽn rõ
    /// rệt. Field bắt buộc (thiếu = fail load).
    pub gas_price_max_gwei: u64,

    /// Cụm `competitor-recon-and-strategy` (F-02, mô hình bribe) — % lợi
    /// nhuận GỘP (trước bribe, `SandwichQuote.profit_wei`) dùng làm bribe mô
    /// phỏng để cạnh tranh vị trí trong block. Ship `40.0` (%) — mốc khởi
    /// tạo THÔ (chưa có số liệu đối thủ thật đủ mẫu để hiệu chỉnh riêng, xem
    /// `baocao/evidence/competitor_recon_run1.txt`); Chủ tự chỉnh theo số
    /// liệu trinh sát cập nhật. Kẹp `[0,100]` ở nơi dùng (`pipeline::compute_bribe_wei`),
    /// không panic với input rác. Field bắt buộc (thiếu = fail load).
    pub bribe_pct_of_profit: f64,
    /// F-02 — sàn bribe (BNB) khi `bribe_pct_of_profit` tính ra số quá nhỏ.
    /// Ship `0.0005`.
    pub bribe_min_bnb: f64,
    /// F-02 — trần bribe (BNB) — `0` = không trần (chỉ dùng `bribe_pct_of_profit`
    /// + sàn). Ship `0.01`.
    pub bribe_max_bnb: f64,
    /// F-02 — cơ chế bribe: `"builder_transfer"` hoặc `"gaspriority"`.
    ///
    /// **Cụm `bugfix-presign-and-contract-plan` (BỔ SUNG GIỮA PHIÊN,
    /// 2026-09-16) — ĐỔI TÊN `"coinbase"` → `"builder_transfer"`, và đó KHÔNG
    /// chỉ là đổi tên**: docs chính thức BlockRazor (Chủ dán) xác nhận bribe
    /// trên BSC đi tới **ví EOA của builder**
    /// (`relay::BLOCKRAZOR_BUILDER_EOA` = `0x1266C6bE…dD9c5F20`), KHÔNG PHẢI
    /// `block.coinbase` như mô hình Flashbots/Ethereum mà cụm
    /// `competitor-recon-and-strategy` đã giả định. Transfer BNB đó nằm
    /// TRONG CHÂN BACK của bundle. Giá trị `"coinbase"` cũ = FAIL LOAD với
    /// thông báo chỉ rõ tên mới (không âm thầm nhận, vì ngữ nghĩa đã khác).
    ///
    /// `"gaspriority"` (tăng `maxPriorityFeePerGas`) giữ nguyên ý nghĩa.
    pub bribe_mode: String,

    /// Cụm `bugfix-presign-and-contract-plan` (BỔ SUNG GIỮA PHIÊN) — ví EOA
    /// builder của **BlockRazor** nhận bribe khi `bribe_mode="builder_transfer"`.
    /// Ship = địa chỉ pin trong docs BlockRazor (`relay::BLOCKRAZOR_BUILDER_EOA`,
    /// bảng đối chiếu trong `DEX_REGISTRY.md`). Mỗi relay có ví builder KHÁC
    /// NHAU nên là 2 field riêng. Không phải address 20 byte hex = FAIL LOAD.
    pub blockrazor_builder_eoa: String,

    /// Ví **Builder Control EOA của 48 Club Puissant** nhận bribe (docs chính
    /// thức Chủ dán 2026-09-16: `relay::CLUB48_BUILDER_EOA` =
    /// `0x4848489f…D7484848`). Xếp hạng bundle của 48 Club =
    /// `0.9 × gas fee của tx unique + BNB chuyển tới EOA này` → **ưu tiên
    /// nhét bribe vào transfer**, giữ gas ở mức tối thiểu 0.05 gwei.
    pub club48_builder_eoa: String,

    /// Cụm `competitor-recon-and-strategy` (mục 4, shadow mode) — chế độ
    /// thực thi: `"off"` (ship, KHÔNG có gì khác paper mode hiện có — không
    /// load signer, không ký), `"shadow"` (KÝ THẬT bằng `PRIVATE_KEY` trong
    /// `.env`, KHÔNG BAO GIỜ broadcast — chỉ log `bundle.shadow`), `"live"`
    /// (CHƯA implement trong cụm này — vẫn cần cụm `strategy-exec` + lệnh
    /// riêng, xem CLAUDE.md mục "Live"; đọc field này KHÔNG tự mở khoá gửi
    /// tx thật, `executor::can_send_live`/`Config::gate_check` vẫn là cổng
    /// DUY NHẤT quyết định có được gửi hay không). Giá trị khác 3 chuỗi này
    /// = FAIL LOAD (cùng khuôn `sim_engine`/`bribe_mode`).
    pub live_mode: String,

    /// Cụm `bugfix-presign-and-contract-plan` (A3) — cho phép coi tx của CỤM
    /// ĐỐI THỦ đã nhận diện (xem `competitor::ClusterIndex`) là nạn nhân hợp
    /// lệ hay không. Ship `false`: khi `live_mode` khác `"off"` (shadow/live
    /// — tức đã có khả năng KÝ thật), candidate có `from` thuộc cụm sẽ KHÔNG
    /// được `Simulated` mà bị skip `competitor_victim`. Ở `live_mode="off"`
    /// (paper thuần) cờ này KHÔNG chặn gì — vẫn `Simulated` và vẫn ghi cờ
    /// `victim_in_competitor_cluster` vào `sim.result`, để `/api/econ` đếm
    /// được nhóm này là bao nhiêu phần trăm cơ hội (nếu chặn ngay ở paper thì
    /// mất hẳn số liệu để Chủ quyết định).
    pub allow_competitor_victims: bool,

    /// Cụm `planB-backrun-opportunity` — CHIẾN LƯỢC thực thi:
    /// - `"backrun"` (ship, Chủ chốt 2026-09-16): backrun-arb nguyên tử bằng
    ///   flash loan. Không cần đứng trước victim, không cần `victim_ok`,
    ///   không cần vốn xoay — xem CLAUDE.md mục "Chiến lược đã chốt".
    /// - `"sandwich"`: đường cũ (front/back kẹp victim). Code KHÔNG bị xoá,
    ///   chỉ TẮT bằng field này, bật lại được nếu Chủ đổi ý.
    ///
    /// Giá trị khác 2 chuỗi này = FAIL LOAD (cùng khuôn `sim_engine`/
    /// `bribe_mode`/`live_mode` — gõ sai phải báo đỏ, không âm thầm chạy
    /// nhầm chiến lược).
    pub strategy: String,

    /// Cụm `planB-backrun-opportunity` — trần số tiền VAY FLASH mỗi route arb
    /// khi quote vay là WBNB (đơn vị BNB). Khác `max_front_bnb` (trần vốn TỰ
    /// CÓ của sandwich): tiền này là tiền vay, trả trong cùng tx, nên trần
    /// đóng vai trò chặn rủi ro trượt giá/gas chứ không phải chặn vốn. Ship
    /// `20`. Chủ chỉnh tự do, hot-reload như mọi ngưỡng khác.
    pub arb_max_borrow_bnb: f64,

    /// Như trên, cho route vay bằng USDT (đơn vị USDT). Ship `12000`.
    pub arb_max_borrow_usdt: f64,

    /// Cụm `planB-backrun-opportunity` (mục 2) — chu kỳ (giây) task nền
    /// `flash_source_task` đọc lại chiều sâu + phí của các nguồn flash
    /// (Infinity Vault / Balancer V2 / Aave V3). Ship `300` (5 phút, đúng
    /// khối lệnh). Độc lập đường nóng: đường nóng CHỈ đọc snapshot đã chụp,
    /// không bao giờ tự gọi RPC để chọn nguồn.
    pub flash_source_interval_sec: u64,

    /// Cụm `planB-backrun-opportunity` (mục 1) — đường dẫn bản đồ venue
    /// (`state/multi_venue.json`, sinh bởi `cargo run --bin venue_map`).
    /// Thiếu file = không có token nào có venue thứ 2 → mọi tx `arb_no_second_venue`,
    /// bot KHÔNG crash (đúng luật "thiếu dữ liệu thì skip, không đoán").
    pub multi_venue_path: String,

    /// Cụm `planB-backrun-opportunity` (mục 5) — gas UNIT đo thật cho route
    /// arb đi qua `Vault.lock` của Infinity (take → 2 swap V2 → sync/settle).
    /// Fallback khi chưa đo được bằng revm lúc boot, cùng khuôn
    /// `gas_units_front`/`gas_units_back`.
    pub gas_units_arb_infinity: u64,

    /// Như trên, cho route arb vay bằng Pancake V2 flash swap (`pancakeCall`).
    pub gas_units_arb_v2flash: u64,

    /// Mốc lần reload gần nhất — KHÔNG đọc/ghi từ `config.toml`
    /// (`#[serde(skip)]`, mặc định `None`). Dùng bởi `reload_if_due`, cùng
    /// quy ước `VictimBook::last_reload` (`src/victims.rs`).
    #[serde(skip)]
    pub last_reload: Option<Instant>,
}

/// Cụm `bugfix-presign-and-contract-plan` (BỔ SUNG GIỮA PHIÊN) — kiểm 1
/// chuỗi có phải address 20 byte hex (`0x` + 40 ký tự hex) không. THUẦN,
/// không phụ thuộc `alloy` (giữ `config.rs` không kéo thêm dependency chỉ để
/// validate 1 field).
pub fn is_address_20_bytes(s: &str) -> bool {
    let t = s.trim();
    t.len() == 42 && t.starts_with("0x") && t[2..].chars().all(|c| c.is_ascii_hexdigit())
}

#[derive(Debug)]
pub enum ConfigError {
    Parse(String),
    InvalidChainId(u64),
    InvalidNumber(String),
    Io(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Parse(e) => write!(f, "config parse error (thieu field hoac sai kieu): {e}"),
            ConfigError::InvalidChainId(id) => write!(f, "chain_id={id} != {REQUIRED_CHAIN_ID}, fail load"),
            ConfigError::InvalidNumber(field) => write!(f, "field '{field}' phai la so >= 0 va huu han, fail load"),
            ConfigError::Io(e) => write!(f, "config io error: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Quy đổi BNB (f64, đọc thẳng từ `config.toml`) sang wei — dùng `round()`
/// thay vì cắt cụt để giảm sai số float, nhưng KHÔNG chính xác tuyệt đối như
/// `victims::bnb_str_to_wei` (parse chuỗi thập phân, đúng ngoài giới hạn
/// f64). Chấp nhận được ở đây vì các ngưỡng này (`min_profit_bnb`,
/// `max_front_bnb`, `min_reserve_wbnb`) là NGƯỠNG SO SÁNH chủ tự chỉnh, sai
/// lệch cỡ vài wei (f64 có ~15-17 chữ số thập phân có nghĩa) không đổi quyết
/// định — khác `victims.txt` nơi `min_swap_bnb` của TỪNG ví cần đúng tuyệt
/// đối. Giá trị âm/không hữu hạn đã bị chặn ở `validate()` lúc load, nhưng
/// hàm này vẫn tự vệ (không panic) nếu gọi trực tiếp với input rác.
pub fn bnb_f64_to_wei(v: f64) -> U256 {
    if !v.is_finite() || v < 0.0 {
        return U256::ZERO;
    }
    let wei = (v * WEI_PER_BNB).round();
    if wei <= 0.0 {
        return U256::ZERO;
    }
    // f64 khong bieu dien chinh xac moi so nguyen wei o bien lon, nhung
    // max_front_bnb/min_reserve_wbnb thuc te (< 10^6 BNB) van nam trong vung
    // f64 giu duoc do chinh xac nguyen can thiet cho muc dich ngưỡng so sánh.
    U256::from(wei as u128)
}

/// Cụm `exec-path-traps` (F-08) — field `config.toml` đã ĐỔI TÊN, còn xuất
/// hiện thì fail load với thông báo RÕ (khác lỗi parse serde mù mờ "unknown
/// field" — serde MẶC ĐỊNH không `deny_unknown_fields` nên field lạ vốn bị
/// ÂM THẦM bỏ qua, không báo gì cả). `(tên_cũ, gợi_ý)`.
const RENAMED_FIELDS: &[(&str, &str)] = &[("executor_slippage_bps", "front_slippage_bps / back_slippage_bps")];

impl Config {
    pub fn from_str(s: &str) -> Result<Self, ConfigError> {
        let raw: toml::Value = toml::from_str(s).map_err(|e| ConfigError::Parse(e.to_string()))?;
        if let Some(table) = raw.as_table() {
            for (old_name, new_name) in RENAMED_FIELDS {
                if table.contains_key(*old_name) {
                    return Err(ConfigError::Parse(format!(
                        "field '{old_name}' da doi ten thanh '{new_name}' (cum exec-path-traps) - xoa field cu khoi config.toml"
                    )));
                }
            }
        }
        let mut cfg: Config = toml::from_str(s).map_err(|e| ConfigError::Parse(e.to_string()))?;
        cfg.validate()?;
        // Danh dau moc "vua load xong" ngay tai day (cung quy uoc
        // VictimBook::load_from_str) de reload_if_due goi sau do khong coi day
        // la lan reload dau tien (tranh doc lai file ngay sau khi vua load).
        cfg.last_reload = Some(Instant::now());
        Ok(cfg)
    }

    /// Fail load CHỈ 4 lý do (CLAUDE.md phiên config-hot-reload): thiếu field
    /// (serde tự fail ở `toml::from_str`), `chain_id != 56`, số âm/không hữu
    /// hạn ở field ngưỡng BNB, parse lỗi. Cho phép `min_profit_bnb=0`,
    /// `max_roundtrip_tax=0`, `max_front_bnb` rất lớn — không chặn biên trên.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.chain_id != REQUIRED_CHAIN_ID {
            return Err(ConfigError::InvalidChainId(self.chain_id));
        }
        let checks: [(&str, f64); 14] = [
            ("min_profit_bnb", self.min_profit_bnb),
            ("max_front_bnb", self.max_front_bnb),
            ("min_reserve_wbnb", self.min_reserve_wbnb),
            ("max_roundtrip_tax", self.max_roundtrip_tax),
            ("max_exposure_bnb", self.max_exposure_bnb),
            ("pairs_min_swap_bnb", self.pairs_min_swap_bnb),
            ("min_profit_usdt", self.min_profit_usdt),
            ("max_front_usdt", self.max_front_usdt),
            ("min_reserve_usdt", self.min_reserve_usdt),
            ("bribe_pct_of_profit", self.bribe_pct_of_profit),
            ("bribe_min_bnb", self.bribe_min_bnb),
            ("bribe_max_bnb", self.bribe_max_bnb),
            ("arb_max_borrow_bnb", self.arb_max_borrow_bnb),
            ("arb_max_borrow_usdt", self.arb_max_borrow_usdt),
        ];
        for (name, v) in checks {
            if !v.is_finite() || v < 0.0 {
                return Err(ConfigError::InvalidNumber(name.to_string()));
            }
        }
        // Cụm `evm-validate-fixed-then-wire` (B3.1) — `sim_engine` chỉ nhận
        // ĐÚNG 2 chuỗi. Gõ sai (vd "EVM", "revm", "V2") = FAIL LOAD, KHÔNG âm
        // thầm rơi về mặc định: chạy nhầm động cơ quyết định là sai lệch NGHIÊM
        // TRỌNG (công thức đóng không thấy tax/honeypot), phải báo đỏ ngay.
        if self.sim_engine != "evm" && self.sim_engine != "v2" {
            return Err(ConfigError::InvalidNumber(format!(
                "sim_engine phai la \"evm\" hoac \"v2\" (nhan duoc {:?})",
                self.sim_engine
            )));
        }
        // Cụm `competitor-recon-and-strategy` (F-02) — cùng khuôn `sim_engine`:
        // gõ sai `bribe_mode` fail load rõ ràng, không âm thầm rơi về mặc định.
        // Cum BO SUNG GIUA PHIEN - "coinbase" CU khong con hop le (ngu nghia
        // doi han: bribe di toi vi EOA builder, khong phai block.coinbase).
        if self.bribe_mode == "coinbase" {
            return Err(ConfigError::InvalidNumber(
                "bribe_mode = \"coinbase\" da doi thanh \"builder_transfer\" (docs BlockRazor 2026-09-16: bribe la transfer BNB toi VI EOA cua builder, KHONG phai block.coinbase) — sua config.toml".to_string(),
            ));
        }
        for (name, value) in [
            ("blockrazor_builder_eoa", &self.blockrazor_builder_eoa),
            ("club48_builder_eoa", &self.club48_builder_eoa),
        ] {
            if !is_address_20_bytes(value) {
                return Err(ConfigError::InvalidNumber(format!("{name} phai la address 20 byte hex (nhan duoc {value:?})")));
            }
        }
        if self.bribe_mode != "builder_transfer" && self.bribe_mode != "gaspriority" {
            return Err(ConfigError::InvalidNumber(format!(
                "bribe_mode phai la \"builder_transfer\" hoac \"gaspriority\" (nhan duoc {:?})",
                self.bribe_mode
            )));
        }
        // Cum `planB-backrun-opportunity` - cung khuon `sim_engine`: go sai
        // `strategy` = FAIL LOAD, khong am tham chay nham chien luoc.
        if self.strategy != "backrun" && self.strategy != "sandwich" {
            return Err(ConfigError::InvalidNumber(format!(
                "strategy phai la \"backrun\" hoac \"sandwich\" (nhan duoc {:?})",
                self.strategy
            )));
        }
        if self.live_mode != "off" && self.live_mode != "shadow" && self.live_mode != "live" {
            return Err(ConfigError::InvalidNumber(format!(
                "live_mode phai la \"off\", \"shadow\" hoac \"live\" (nhan duoc {:?})",
                self.live_mode
            )));
        }
        Ok(())
    }

    /// Cụm `planB-backrun-opportunity` — điểm đọc DUY NHẤT của `strategy`
    /// trong production path (cùng quy ước `sim_engine_is_evm`).
    pub fn strategy_is_backrun(&self) -> bool {
        self.strategy == "backrun"
    }

    /// F-02/mục 4 — `true` chỉ khi `live_mode="shadow"` — điểm đọc DUY NHẤT
    /// (cùng quy ước `sim_engine_is_evm`), tránh so chuỗi rải rác.
    pub fn live_mode_is_shadow(&self) -> bool {
        self.live_mode == "shadow"
    }

    /// F-02 — sàn bribe (wei BNB).
    pub fn bribe_min_wei(&self) -> u128 {
        let w = bnb_f64_to_wei(self.bribe_min_bnb);
        u128::try_from(w).unwrap_or(u128::MAX)
    }

    /// F-02 — trần bribe (wei BNB), `0` nghĩa "không trần" (caller tự diễn giải).
    pub fn bribe_max_wei(&self) -> u128 {
        let w = bnb_f64_to_wei(self.bribe_max_bnb);
        u128::try_from(w).unwrap_or(u128::MAX)
    }

    /// `true` khi động cơ quyết định là EVM thật (`revm`) — điểm đọc DUY NHẤT
    /// của field `sim_engine` trong production path, để không rải so chuỗi
    /// khắp nơi (dễ lệch hoa/thường giữa các call site).
    pub fn sim_engine_is_evm(&self) -> bool {
        self.sim_engine == "evm"
    }

    /// TTL cache tax dưới dạng `Duration` (C2).
    pub fn tax_cache_ttl(&self) -> Duration {
        Duration::from_secs(self.tax_cache_ttl_sec)
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let s = std::fs::read_to_string(path).map_err(|e| ConfigError::Io(e.to_string()))?;
        Self::from_str(&s)
    }

    /// Hot-reload có điều kiện thời gian — cùng khuôn mẫu
    /// `VictimBook::reload_if_due` (`src/victims.rs`), `now` truyền từ ngoài
    /// để test được không cần sleep thật. Reload lỗi (config.toml sửa sai
    /// giữa chừng) -> GIỮ config cũ + log, không panic, không crash bot —
    /// khác `*self = new` vô điều kiện.
    pub fn reload_if_due(&mut self, path: &Path, reload_interval: Duration, now: Instant) -> bool {
        let due = match self.last_reload {
            None => true,
            Some(last) => now.saturating_duration_since(last) >= reload_interval,
        };
        if due {
            match Config::load(path) {
                Ok(mut new_cfg) => {
                    new_cfg.last_reload = Some(now);
                    *self = new_cfg;
                }
                Err(e) => {
                    eprintln!("config.toml reload that bai (giu config cu, khong crash): {e}");
                    self.last_reload = Some(now);
                }
            }
        }
        due
    }

    /// Ngưỡng lợi nhuận tối thiểu để coi 1 sandwich là đáng làm (`4.1`
    /// pipeline dùng), quy đổi wei từ `min_profit_bnb`.
    pub fn min_profit_wei(&self) -> U256 {
        bnb_f64_to_wei(self.min_profit_bnb)
    }

    /// Trần `front_in` (wei) cho `sim_v2::search_max_front_in`.
    pub fn max_front_wei(&self) -> U256 {
        bnb_f64_to_wei(self.max_front_bnb)
    }

    /// Ngưỡng thanh khoản tối thiểu (wei WBNB) — pool mỏng hơn mức này ->
    /// `thin_liq` skip.
    pub fn min_reserve_wei(&self) -> U256 {
        bnb_f64_to_wei(self.min_reserve_wbnb)
    }

    /// `max_roundtrip_tax` (phân số, vd `0.005` = 0.5%) quy đổi sang basis
    /// point nguyên để so trực tiếp với `tax::TaxMeasurement::roundtrip_tax_bps`
    /// (u32, không float) — làm tròn gần nhất, `clamp` để không tràn `u32`.
    pub fn max_roundtrip_tax_bps(&self) -> u32 {
        let bps = (self.max_roundtrip_tax * 10_000.0).round();
        if !bps.is_finite() || bps < 0.0 {
            0
        } else if bps > u32::MAX as f64 {
            u32::MAX
        } else {
            bps as u32
        }
    }

    /// Cụm `real-economics-mode2` (F-03) — TRẦN gas front+back cộng dồn
    /// (wei). TRƯỚC cụm này đây là số dùng THẲNG làm chi phí gas trừ vào
    /// profit (sai — cao hơn thực tế 10-100 lần, xem audit F-03); TỪ cụm này
    /// chỉ còn là TRẦN so sánh với `gas_cost_wei` đo thật
    /// (`pipeline::compute_gas_cost_wei`) — `gas_cost_wei > gas_wei()` thì
    /// skip `gas_cap`, KHÔNG còn dùng trực tiếp làm chi phí trừ vào profit
    /// trên đường nóng `sim_engine="v2"` nữa.
    pub fn gas_wei(&self) -> u128 {
        self.front_max_gas_bnb_wei as u128 + self.back_max_gas_bnb_wei as u128
    }

    /// F-03 — `gas_price_max_gwei` quy đổi wei/gas-unit, dùng để so với
    /// `eth_gasPrice` đo được (`GasOracle`) — vượt ngưỡng này thì coi gas
    /// hiện tại "quá đắt", không đoán sẽ rẻ lại kịp lúc.
    pub fn gas_price_max_wei(&self) -> u128 {
        self.gas_price_max_gwei as u128 * 1_000_000_000u128
    }

    /// Cụm `usdt-quote-asset` — ngưỡng lợi nhuận tối thiểu (wei USDT, 18
    /// decimal trên BSC — cùng độ lớn đơn vị `1e18` như BNB nên tái dùng
    /// `bnb_f64_to_wei` an toàn, KHÔNG phải quy đổi giá) cho `evaluate_candidate_quote`
    /// nhánh USDT.
    pub fn min_profit_usdt_wei(&self) -> U256 {
        bnb_f64_to_wei(self.min_profit_usdt)
    }

    /// Cụm `usdt-quote-asset` — trần `front_in` (wei USDT) cho pool quote
    /// USDT, dùng THẲNG (không gộp `max_exposure_wei`/BNB — ngoài phạm vi
    /// lệnh USDT lần này).
    pub fn max_front_usdt_wei(&self) -> U256 {
        bnb_f64_to_wei(self.max_front_usdt)
    }

    /// Cụm `usdt-quote-asset` — ngưỡng thanh khoản tối thiểu (wei USDT) cho
    /// pool quote USDT.
    pub fn min_reserve_usdt_wei(&self) -> U256 {
        bnb_f64_to_wei(self.min_reserve_usdt)
    }

    /// Cụm `5.2` — trần EXPOSURE (wei) riêng, ĐỀ XUẤT trong lệnh (chưa bị Grok
    /// phản đối, ghi quyết định ở `docs/STATE.md` mục "5.2"): `max_exposure_bnb
    /// == 0` nghĩa là TẮT cap này (chỉ còn `max_front_bnb` giới hạn), số dương
    /// là trần cứng thứ hai. `None` = tắt, `Some(wei)` = đang bật.
    pub fn max_exposure_wei(&self) -> Option<U256> {
        if self.max_exposure_bnb == 0.0 {
            None
        } else {
            Some(bnb_f64_to_wei(self.max_exposure_bnb))
        }
    }

    /// Trần `front_in` THỰC TẾ truyền vào `sim_v2::search_max_front_in` —
    /// `min(max_front_wei, max_exposure_wei)` khi exposure cap đang bật, hoặc
    /// thẳng `max_front_wei` khi tắt (`max_exposure_bnb == 0`). Đây là điểm
    /// DUY NHẤT nơi 2 ngưỡng này được gộp — `pipeline.rs::decide_paper` gọi
    /// hàm này thay vì `max_front_wei()` trực tiếp.
    pub fn effective_front_cap_wei(&self) -> U256 {
        match self.max_exposure_wei() {
            Some(exposure) => self.max_front_wei().min(exposure),
            None => self.max_front_wei(),
        }
    }

    /// Cụm `5.2` — cảnh báo BOOT (KHÔNG fail load, đúng lệnh "Không fail
    /// load"): `min_profit_bnb` (ngưỡng lời TỐI THIỂU, đã trừ gas — xem
    /// `sim_v2::quote_at`) thấp hơn tổng gas cap 2 chiều front+back. Đây chỉ
    /// là gợi ý cho chủ (đặt `min_profit_bnb` quá thấp so với chi phí gas
    /// thô dễ gây hiểu lầm), KHÔNG phải bug tính toán — `decide_paper` vẫn
    /// đúng vì gas đã bị trừ vào `profit_wei` TRƯỚC khi so `min_profit_wei`.
    pub fn gas_warning_needed(&self) -> bool {
        self.min_profit_wei() < U256::from(self.gas_wei())
    }

    /// Ngưỡng `min_swap` GLOBAL (wei) cho mọi pool trong `PairBook` (cụm
    /// pair-mode) — quy đổi từ `pairs_min_swap_bnb`, cùng đánh đổi độ chính
    /// xác f64 như các ngưỡng BNB khác ở trên (chấp nhận được, đây cũng là
    /// NGƯỠNG SO SÁNH chủ tự chỉnh).
    pub fn pairs_min_swap_wei(&self) -> U256 {
        bnb_f64_to_wei(self.pairs_min_swap_bnb)
    }

    /// Cụm `exec-path-traps` (F-05) — cổng live CHI TIẾT, NGUỒN DUY NHẤT cho
    /// mọi điều kiện live theo CLAUDE.md mục "Live". `halt_exists` đọc từ
    /// `state/halt.lock`. `version_flag_name`/`version_live`/`version_pinned`
    /// là cờ riêng của family (v2/v3/v4) ĐANG được đánh giá, gọi lại hàm này
    /// riêng cho từng family cần live. Gom TẤT CẢ lý do thiếu vào `failures`
    /// (không dừng ở lý do đầu tiên) — trước bản sửa này, `executor::gate_check`
    /// có 1 bản CHÉP RIÊNG của các điều kiện này mà THIẾU `version_pinned`
    /// (audit F-05: 2 cổng lệch nhau) — giờ chỉ còn ĐÚNG 1 chỗ định nghĩa,
    /// `live_gate_ok`/`executor::can_send_live`/`executor::gate_check` (giữ
    /// lại làm hàm mỏng gọi thẳng xuống đây) đều dùng chung.
    pub fn gate_check(&self, halt_exists: bool, version_flag_name: &str, version_live: bool, version_pinned: bool) -> LiveGateStatus {
        let mut failures = Vec::new();
        if !self.allow_live {
            failures.push("allow_live=false".to_string());
        }
        if self.dry_run {
            failures.push("dry_run=true".to_string());
        }
        if !self.bot_armed {
            failures.push("bot_armed=false".to_string());
        }
        if halt_exists {
            failures.push("halt.lock present".to_string());
        }
        if self.chain_id != REQUIRED_CHAIN_ID {
            failures.push(format!("chain_id={} != {REQUIRED_CHAIN_ID}", self.chain_id));
        }
        if !version_live {
            failures.push(format!("{version_flag_name}=false"));
        }
        if !version_pinned {
            failures.push(format!("{version_flag_name}_pinned=false"));
        }
        if self.front_max_gas_bnb_wei == 0 {
            failures.push("front_max_gas_bnb_wei=0".to_string());
        }
        if self.back_max_gas_bnb_wei == 0 {
            failures.push("back_max_gas_bnb_wei=0".to_string());
        }
        LiveGateStatus { ok: failures.is_empty(), failures }
    }

    /// Cổng live theo CLAUDE.md mục "Live", dạng `bool` tiện dụng — GỌI THẲNG
    /// `gate_check` ở trên (F-05, không còn 2 danh sách điều kiện tách rời).
    /// `gas cap > 0` đọc THẲNG từ `front_max_gas_bnb_wei`/`back_max_gas_bnb_wei`
    /// (bên trong `gate_check`) thay vì nhận tham số `gas_cap_positive` rời
    /// như bản cũ — tránh caller tự tính sai/quên đồng bộ 2 field gas.
    pub fn live_gate_ok(&self, halt_locked: bool, version_live: bool, version_pinned: bool) -> bool {
        self.gate_check(halt_locked, "version", version_live, version_pinned).ok
    }
}

/// Cụm `7.1` — kết quả cổng live CHI TIẾT (khác `live_gate_ok` chỉ trả
/// `bool`): liệt kê ĐỦ các điều kiện đang THIẾU để chủ/Grok biết chính xác
/// cái gì chưa xanh, thay vì chỉ biết `false`. Dời từ `executor.rs` sang đây
/// (F-05) vì `Config::gate_check` giờ là nguồn duy nhất sinh ra kiểu này.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveGateStatus {
    pub ok: bool,
    pub failures: Vec<String>,
}

/// Cụm pair-mode/7.1 — wire 2 field từng chỉ validate-lúc-load mà chưa có
/// logic tiêu thụ nào (`max_consecutive_loss`/`gas_reserve_bnb_wei`, ghi nợ
/// từ phiên config-hot-reload, xem `docs/TASKS.md`).
///
/// `record_result`/`consecutive_loss_exceeded` là cổng dành cho TẦNG EXECUTOR
/// THẬT (`7.x`, chưa tồn tại) gọi sau khi biết kết quả on-chain THẬT của 1
/// giao dịch — paper loop hiện tại (`dry_run=true`) KHÔNG tự gọi
/// `record_result` vì `PipelineOutcome::Simulated` ở dry-run không phải giao
/// dịch thật, không có lỗ thật để đếm (không bịa logic "thua giả"). Phiên
/// này CHỈ wire sẵn cổng kiểm tra (`decide_paper_v2` gọi
/// `consecutive_loss_exceeded` trước khi sim — nếu bot từng bị 7.x đánh dấu
/// đã lỗ liên tiếp đủ ngưỡng, coi như `unprofitable`, dùng lại đúng enum
/// `PipelineSkip` sẵn có trong CLAUDE.md, KHÔNG thêm skip reason mới) +
/// `front_cap_after_gas_reserve` (giữ `gas_reserve_bnb_wei` LUÔN không bị
/// dùng làm vốn front-run, trừ thẳng vào trần front_in hiệu lực).
#[derive(Debug, Default)]
pub struct RiskGuard {
    consecutive_loss: u32,
}

impl RiskGuard {
    pub fn new() -> Self {
        Self { consecutive_loss: 0 }
    }

    /// Gọi bởi tầng executor thật (`7.x`) sau khi biết kết quả THẬT của 1
    /// giao dịch: `is_loss=true` (lỗ) tăng đếm liên tiếp; `false` (lãi) reset
    /// về 0.
    pub fn record_result(&mut self, is_loss: bool) {
        if is_loss {
            self.consecutive_loss += 1;
        } else {
            self.consecutive_loss = 0;
        }
    }

    pub fn consecutive_loss(&self) -> u32 {
        self.consecutive_loss
    }

    /// `max_consecutive_loss=0` nghĩa là TẮT guard này (giữ nguyên tinh thần
    /// "0 = tắt cap" của `max_exposure_bnb`) — số dương là ngưỡng chặn thật.
    pub fn consecutive_loss_exceeded(&self, max_consecutive_loss: u32) -> bool {
        max_consecutive_loss > 0 && self.consecutive_loss >= max_consecutive_loss
    }

    /// Trần `front_in` CÒN LẠI sau khi trừ `gas_reserve_bnb_wei` khỏi `cap`
    /// (thường là `Config::effective_front_cap_wei()`) — giữ luôn 1 khoản dự
    /// trữ KHÔNG bị dùng làm vốn front-run. `saturating_sub` để không tràn âm
    /// khi `gas_reserve_bnb_wei` > cap (trả về 0, không panic).
    pub fn front_cap_after_gas_reserve(cap: U256, gas_reserve_bnb_wei: u64) -> U256 {
        cap.saturating_sub(U256::from(gas_reserve_bnb_wei))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_toml() -> String {
        r#"
chain_id = 56
dry_run = true
allow_live = false
bot_armed = false
scan_v2 = true
scan_v3 = true
scan_v4 = true
live_v2 = false
live_v3 = false
live_v4 = false
min_profit_bnb = 0.01
max_front_bnb = 1.5
min_reserve_wbnb = 20
victims_path = "victims.txt"
victims_reload_sec = 15
config_reload_sec = 15
pending_poll_ms = 400
pending_txpool_max_per_poll = 32
gas_reserve_bnb_wei = 5000000000000000
front_max_gas_bnb_wei = 3000000000000000
back_max_gas_bnb_wei = 3000000000000000
tx_timeout_sec = 30
ws_silence_sec = 60
max_consecutive_loss = 3
max_exposure_bnb = 5.0
web_bind = "127.0.0.1"
web_port = 8787
max_roundtrip_tax = 0.005
tax_cache_blocks = 30
allow_tax_inject = true
executor_deadline_buffer_sec = 120
pairs_path = "pairs.txt"
pairs_reload_sec = 30
pairs_min_swap_bnb = 0.05
pair_scan_universal = false
wallet_scan_enabled = false
pair_scan_enabled = true
scan_quote_usdt = false
min_profit_usdt = 3.0
max_front_usdt = 3000.0
min_reserve_usdt = 15000.0
sim_engine = "v2"
tax_cache_ttl_sec = 600
front_slippage_bps = 10
back_slippage_bps = 50
pairs_vet_interval_sec = 600
pairs_require_vetted = true
gas_units_front = 160000
gas_units_back = 140000
gas_price_max_gwei = 10
bribe_pct_of_profit = 0.0
bribe_min_bnb = 0.0
bribe_max_bnb = 0.0
bribe_mode = "builder_transfer"
blockrazor_builder_eoa = "0x1266C6bE60392A8Ff346E8d5ECCd3E69dD9c5F20"
club48_builder_eoa = "0x4848489f0b2BEdd788c696e2D79b6b69D7484848"
live_mode = "off"
strategy = "backrun"
arb_max_borrow_bnb = 20
arb_max_borrow_usdt = 12000
flash_source_interval_sec = 300
multi_venue_path = "state/multi_venue.json"
gas_units_arb_infinity = 420000
gas_units_arb_v2flash = 330000
allow_competitor_victims = false
"#
        .to_string()
    }

    #[test]
    fn load_ok() {
        let cfg = Config::from_str(&base_toml()).expect("should load");
        assert_eq!(cfg.chain_id, 56);
        assert!(cfg.dry_run);
        assert!(!cfg.allow_live);
        assert_eq!(cfg.web_port, 8787);
        assert!(cfg.allow_tax_inject);
    }

    /// `allow_tax_inject` la field bat buoc moi (cum tax-cache-inject) —
    /// thieu field nay cung phai fail load dung luat CLAUDE.md, giong het
    /// `missing_min_profit_bnb_fails` o duoi.
    #[test]
    fn missing_allow_tax_inject_fails() {
        let toml_str = base_toml().replace("allow_tax_inject = true\n", "");
        let err = Config::from_str(&toml_str).unwrap_err();
        match err {
            ConfigError::Parse(_) => {}
            other => panic!("expected Parse error, got {other:?}"),
        }
    }

    #[test]
    fn missing_min_profit_bnb_fails() {
        let toml_str = base_toml().replace("min_profit_bnb = 0.01\n", "");
        let err = Config::from_str(&toml_str).unwrap_err();
        match err {
            ConfigError::Parse(_) => {}
            other => panic!("expected Parse error, got {other:?}"),
        }
    }

    #[test]
    fn chain_id_1_fails() {
        let toml_str = base_toml().replace("chain_id = 56", "chain_id = 1");
        let err = Config::from_str(&toml_str).unwrap_err();
        match err {
            ConfigError::InvalidChainId(1) => {}
            other => panic!("expected InvalidChainId(1), got {other:?}"),
        }
    }

    #[test]
    fn dry_run_true_blocks_live_gate() {
        let cfg = Config::from_str(&base_toml()).unwrap();
        // Không có halt, version pinned+live giả định true, gas cap > 0
        // (base_toml ship có front/back_max_gas_bnb_wei > 0): nhưng
        // dry_run=true và allow_live=false nên cổng live PHẢI đóng.
        assert!(!cfg.live_gate_ok(false, true, true));
    }

    /// Cụm `exec-path-traps` (F-05) — ĐẠT CẦN DÁN "2 cổng cho cùng kết quả
    /// với mọi tổ hợp 8 cờ": duyệt toàn bộ 2^8=256 tổ hợp của 8 điều kiện live
    /// (allow_live, dry_run, bot_armed, halt, chain_id==56, version_live,
    /// version_pinned, gas_cap>0 — gộp front/back thành 1 cờ vì `gate_check`
    /// luôn kiểm CẢ HAI cùng lúc) và xác nhận `live_gate_ok(...)` (bool) ==
    /// `gate_check(...).ok` (chi tiết) cho MỌI tổ hợp — không chỉ vài case
    /// tay. Trước F-05, `executor::gate_check` là 1 bản CHÉP RIÊNG thiếu
    /// `version_pinned` nên có thể lệch với `live_gate_ok`; giờ cả hai gọi
    /// chung 1 hàm nên test này chủ yếu là bảo vệ hồi quy (chống ai đó tách
    /// lại thành 2 bản trong tương lai).
    #[test]
    fn gate_check_and_live_gate_ok_agree_on_all_256_flag_combinations() {
        let mut cfg = Config::from_str(&base_toml()).unwrap();
        for bits in 0u32..256 {
            cfg.allow_live = bits & 1 != 0;
            cfg.dry_run = bits & 2 != 0;
            cfg.bot_armed = bits & 4 != 0;
            let halt = bits & 8 != 0;
            cfg.chain_id = if bits & 16 != 0 { 56 } else { 1 };
            let version_live = bits & 32 != 0;
            let version_pinned = bits & 64 != 0;
            let gas_ok = bits & 128 != 0;
            cfg.front_max_gas_bnb_wei = if gas_ok { 1 } else { 0 };
            cfg.back_max_gas_bnb_wei = if gas_ok { 1 } else { 0 };

            let bool_result = cfg.live_gate_ok(halt, version_live, version_pinned);
            let detailed_result = cfg.gate_check(halt, "live_v2", version_live, version_pinned).ok;
            assert_eq!(
                bool_result, detailed_result,
                "bits={bits:#010b}: live_gate_ok={bool_result} != gate_check.ok={detailed_result}"
            );
        }
        // chain_id bi doi trong vong lap tren - dat lai 56 de khong lam
        // "bau" cfg cho test khac neu ai do tai dung bien nay (khong xay ra
        // trong suite hien tai vi cfg la local, nhung ghi ro cho ro rang).
        cfg.chain_id = 56;
    }

    /// Lệnh chủ: "load max_front_bnb=100, min_profit_bnb=0 → OK" — không chặn
    /// biên trên `max_front_bnb`, cho phép `min_profit_bnb=0`.
    #[test]
    fn max_front_bnb_100_and_min_profit_bnb_0_load_ok() {
        let toml_str = base_toml()
            .replace("max_front_bnb = 1.5", "max_front_bnb = 100")
            .replace("min_profit_bnb = 0.01", "min_profit_bnb = 0");
        let cfg = Config::from_str(&toml_str).expect("max_front_bnb=100 + min_profit_bnb=0 phai load duoc");
        assert_eq!(cfg.max_front_bnb, 100.0);
        assert_eq!(cfg.min_profit_bnb, 0.0);
        assert_eq!(cfg.min_profit_wei(), U256::ZERO);
    }

    /// Lệnh chủ: "load min_profit_bnb=-1 → fail".
    #[test]
    fn min_profit_bnb_negative_fails() {
        let toml_str = base_toml().replace("min_profit_bnb = 0.01", "min_profit_bnb = -1");
        let err = Config::from_str(&toml_str).unwrap_err();
        match err {
            ConfigError::InvalidNumber(field) => assert_eq!(field, "min_profit_bnb"),
            other => panic!("expect InvalidNumber(min_profit_bnb), got {other:?}"),
        }
    }

    /// `max_roundtrip_tax=0` phải load OK (zero-tax only, theo CLAUDE.md mục
    /// "Config").
    #[test]
    fn max_roundtrip_tax_zero_load_ok() {
        let toml_str = base_toml().replace("max_roundtrip_tax = 0.005", "max_roundtrip_tax = 0");
        let cfg = Config::from_str(&toml_str).expect("max_roundtrip_tax=0 phai load duoc");
        assert_eq!(cfg.max_roundtrip_tax_bps(), 0);
    }

    /// Mỗi field ngưỡng BNB âm đều fail load riêng biệt — không chỉ
    /// `min_profit_bnb`.
    #[test]
    fn each_negative_threshold_field_fails_load() {
        for (needle, replacement) in [
            ("max_front_bnb = 1.5", "max_front_bnb = -0.1"),
            ("min_reserve_wbnb = 20", "min_reserve_wbnb = -1"),
            ("max_roundtrip_tax = 0.005", "max_roundtrip_tax = -0.001"),
            ("max_exposure_bnb = 5.0", "max_exposure_bnb = -5.0"),
        ] {
            let toml_str = base_toml().replace(needle, replacement);
            assert!(
                matches!(Config::from_str(&toml_str), Err(ConfigError::InvalidNumber(_))),
                "field am tu '{needle}' -> '{replacement}' phai fail load"
            );
        }
    }

    #[test]
    fn bnb_f64_to_wei_matches_exact_integer_cases() {
        assert_eq!(bnb_f64_to_wei(0.01), U256::from(10_000_000_000_000_000u128));
        assert_eq!(bnb_f64_to_wei(1.5), U256::from(1_500_000_000_000_000_000u128));
        assert_eq!(bnb_f64_to_wei(0.0), U256::ZERO);
        assert_eq!(bnb_f64_to_wei(-1.0), U256::ZERO); // am -> 0, khong panic (validate() da chan tu load)
    }

    #[test]
    fn max_roundtrip_tax_bps_rounds_to_nearest() {
        assert_eq!(bnb_f64_to_wei(0.005), U256::from(5_000_000_000_000_000u128));
        let cfg = Config::from_str(&base_toml()).unwrap();
        assert_eq!(cfg.max_roundtrip_tax_bps(), 50); // 0.005 * 10000 = 50 bps
    }

    /// Hot-reload `config.toml` giống `VictimBook::reload_if_due` — cùng
    /// khuôn mẫu test `victims::tests::reload_respects_interval_with_injected_clock`.
    #[test]
    fn reload_respects_interval_and_picks_up_new_min_profit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, base_toml()).unwrap();

        let mut cfg = Config::load(&path).expect("load ban dau phai OK");
        assert_eq!(cfg.min_profit_bnb, 0.01);
        let t0 = cfg.last_reload.expect("from_str phai tu dat last_reload");
        // Sua file NGAY (chua qua interval) -> reload_if_due(false) khong doc
        // lai, gia tri cu (0.01) van con.
        std::fs::write(&path, base_toml().replace("min_profit_bnb = 0.01", "min_profit_bnb = 0.05")).unwrap();
        assert!(!cfg.reload_if_due(&path, Duration::from_secs(15), t0));
        assert_eq!(cfg.min_profit_bnb, 0.01, "chua du interval thi khong duoc doi");

        // Qua interval -> doc lai, thay gia tri moi.
        let t1 = t0 + Duration::from_secs(16);
        assert!(cfg.reload_if_due(&path, Duration::from_secs(15), t1));
        assert_eq!(cfg.min_profit_bnb, 0.05, "qua interval phai doc gia tri moi tu file");
    }

    /// `pending_poll_ms` là field bắt buộc mới (cụm `5.2`) — thiếu field này
    /// phải fail load, cùng khuôn `missing_allow_tax_inject_fails`.
    #[test]
    fn missing_pending_poll_ms_fails() {
        let toml_str = base_toml().replace("pending_poll_ms = 400\n", "");
        let err = Config::from_str(&toml_str).unwrap_err();
        match err {
            ConfigError::Parse(_) => {}
            other => panic!("expected Parse error, got {other:?}"),
        }
    }

    /// `pending_txpool_max_per_poll` là field bắt buộc mới (cụm `5.3`) —
    /// thiếu field này phải fail load, cùng khuôn các field bắt buộc khác.
    #[test]
    fn missing_pending_txpool_max_per_poll_fails() {
        let toml_str = base_toml().replace("pending_txpool_max_per_poll = 32\n", "");
        let err = Config::from_str(&toml_str).unwrap_err();
        match err {
            ConfigError::Parse(_) => {}
            other => panic!("expected Parse error, got {other:?}"),
        }
    }

    #[test]
    fn pending_txpool_max_per_poll_loads_ship_value() {
        let cfg = Config::from_str(&base_toml()).unwrap();
        assert_eq!(cfg.pending_txpool_max_per_poll, 32);
    }

    /// ĐẠT CẦN DÁN (lệnh 5.2, mục 1): "Test: max_front=10, max_exposure=5 ->
    /// front_in <= 5 BNB wei" — kiểm ở tầng `Config` thuần trước khi kiểm
    /// full pipeline (`pipeline::tests::max_exposure_bnb_caps_front_in_tighter_than_max_front_bnb`).
    #[test]
    fn effective_front_cap_wei_uses_min_of_max_front_and_max_exposure() {
        let toml_str = base_toml().replace("max_front_bnb = 1.5", "max_front_bnb = 10");
        let cfg = Config::from_str(&toml_str).unwrap();
        assert_eq!(cfg.max_front_wei(), bnb_f64_to_wei(10.0));
        assert_eq!(cfg.max_exposure_wei(), Some(bnb_f64_to_wei(5.0)));
        assert_eq!(
            cfg.effective_front_cap_wei(),
            bnb_f64_to_wei(5.0),
            "max_exposure_bnb=5 phai chan chat hon max_front_bnb=10"
        );
    }

    /// `max_exposure_bnb = 0` phải TẮT cap này — `effective_front_cap_wei`
    /// quay về đúng `max_front_wei` (không còn bị 5 BNB cũ chặn).
    #[test]
    fn effective_front_cap_wei_uncapped_when_max_exposure_zero() {
        let toml_str = base_toml()
            .replace("max_front_bnb = 1.5", "max_front_bnb = 10")
            .replace("max_exposure_bnb = 5.0", "max_exposure_bnb = 0");
        let cfg = Config::from_str(&toml_str).expect("max_exposure_bnb=0 phai load duoc (khong am)");
        assert_eq!(cfg.max_exposure_wei(), None);
        assert_eq!(cfg.effective_front_cap_wei(), cfg.max_front_wei());
    }

    /// ĐẠT CẦN DÁN (lệnh 5.2, mục 2): boot warning khi `min_profit_bnb` thấp
    /// hơn tổng gas 2 chiều. `base_toml()` ship `front_max_gas_bnb_wei =
    /// back_max_gas_bnb_wei = 0.003 BNB` -> tổng `0.006 BNB`; hạ
    /// `min_profit_bnb` xuống `0.001` (< 0.006) phải bật cảnh báo.
    #[test]
    fn gas_warning_needed_true_when_min_profit_below_gas_total() {
        let toml_str = base_toml().replace("min_profit_bnb = 0.01", "min_profit_bnb = 0.001");
        let cfg = Config::from_str(&toml_str).unwrap();
        assert!(cfg.gas_warning_needed(), "0.001 BNB < 0.006 BNB gas total phai bat canh bao");
    }

    /// `base_toml()` mặc định `min_profit_bnb = 0.01` (> gas total 0.006) ->
    /// KHÔNG cảnh báo.
    #[test]
    fn gas_warning_needed_false_when_min_profit_above_gas_total() {
        let cfg = Config::from_str(&base_toml()).unwrap();
        assert!(!cfg.gas_warning_needed(), "0.01 BNB > 0.006 BNB gas total khong duoc canh bao");
    }

    /// Reload lỗi (config.toml bị sửa dở dang, thiếu field) -> GIỮ config cũ,
    /// không panic, không crash bot.
    #[test]
    fn reload_keeps_old_config_on_parse_error_no_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, base_toml()).unwrap();

        let mut cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.min_profit_bnb, 0.01);

        // Ghi file dang do (thieu field) ngay giua luc sua tay.
        std::fs::write(&path, "chain_id = 56\ndry_run = true\n").unwrap();
        let t1 = Instant::now() + Duration::from_secs(20);
        let due = cfg.reload_if_due(&path, Duration::from_secs(15), t1);
        assert!(due, "van tinh la 'da thu reload' dung ky, du that bai");
        assert_eq!(cfg.min_profit_bnb, 0.01, "reload that bai phai giu nguyen config cu");
    }

    /// `executor_deadline_buffer_sec` la field bat buoc (cum `7.3`, BAOCAO16)
    /// - thieu no phai fail load, cung khuon cac field bat buoc khac.
    #[test]
    fn missing_executor_fields_fail_load() {
        for needle in ["executor_deadline_buffer_sec = 120\n"] {
            let toml_str = base_toml().replace(needle, "");
            let err = Config::from_str(&toml_str).unwrap_err();
            match err {
                ConfigError::Parse(_) => {}
                other => panic!("expected Parse error khi thieu '{needle}', got {other:?}"),
            }
        }
    }

    #[test]
    fn executor_fields_load_ship_defaults() {
        let cfg = Config::from_str(&base_toml()).unwrap();
        assert_eq!(cfg.executor_deadline_buffer_sec, 120);
    }

    /// Cụm `exec-path-traps` (F-08) — ĐẠT CẦN DÁN: field `executor_slippage_bps`
    /// (đã đổi tên) còn sót trong `config.toml` -> fail load với thông báo RÕ
    /// nhắc tên field mới, KHÔNG âm thầm bị serde bỏ qua (mặc định
    /// `toml::from_str` không `deny_unknown_fields`).
    #[test]
    fn executor_slippage_bps_still_present_fails_load_with_clear_rename_message() {
        let toml_str = format!("{}\nexecutor_slippage_bps = 50\n", base_toml());
        let err = Config::from_str(&toml_str).unwrap_err();
        match err {
            ConfigError::Parse(msg) => {
                assert!(msg.contains("executor_slippage_bps"), "thong bao phai nhac ten field cu: {msg}");
                assert!(msg.contains("da doi ten"), "thong bao phai noi ro 'da doi ten': {msg}");
            }
            other => panic!("expected Parse error, got {other:?}"),
        }
    }

    /// `pairs_path`/`pairs_reload_sec`/`pairs_min_swap_bnb` la field bat buoc
    /// moi (cum pair-mode) - thieu field nao cung phai fail load, cung khuon
    /// cac field bat buoc khac.
    #[test]
    fn missing_pairs_fields_fail_load() {
        for needle in ["pairs_path = \"pairs.txt\"\n", "pairs_reload_sec = 30\n", "pairs_min_swap_bnb = 0.05\n"] {
            let toml_str = base_toml().replace(needle, "");
            let err = Config::from_str(&toml_str).unwrap_err();
            match err {
                ConfigError::Parse(_) => {}
                other => panic!("expected Parse error khi thieu '{needle}', got {other:?}"),
            }
        }
    }

    #[test]
    fn pairs_min_swap_bnb_negative_fails_load() {
        let toml_str = base_toml().replace("pairs_min_swap_bnb = 0.05", "pairs_min_swap_bnb = -0.05");
        match Config::from_str(&toml_str) {
            Err(ConfigError::InvalidNumber(field)) => assert_eq!(field, "pairs_min_swap_bnb"),
            other => panic!("expect InvalidNumber(pairs_min_swap_bnb), got {other:?}"),
        }
    }

    #[test]
    fn pairs_min_swap_wei_matches_bnb_f64_to_wei() {
        let cfg = Config::from_str(&base_toml()).unwrap();
        assert_eq!(cfg.pairs_min_swap_wei(), bnb_f64_to_wei(0.05));
    }

    /// `pair_scan_universal` la field bat buoc moi (cum `universal-pair-scan`)
    /// - thieu field nay phai fail load, cung khuon cac field bat buoc khac.
    #[test]
    fn missing_pair_scan_universal_fails() {
        let toml_str = base_toml().replace("pair_scan_universal = false\n", "");
        let err = Config::from_str(&toml_str).unwrap_err();
        match err {
            ConfigError::Parse(_) => {}
            other => panic!("expected Parse error, got {other:?}"),
        }
    }

    /// Ship mac dinh `pair_scan_universal = false` - AN TOAN, chu phai tu bat.
    #[test]
    fn pair_scan_universal_ship_default_is_false() {
        let cfg = Config::from_str(&base_toml()).unwrap();
        assert!(!cfg.pair_scan_universal);
    }

    #[test]
    fn pair_scan_universal_true_loads_ok() {
        let toml_str = base_toml().replace("pair_scan_universal = false", "pair_scan_universal = true");
        let cfg = Config::from_str(&toml_str).expect("pair_scan_universal=true phai load duoc");
        assert!(cfg.pair_scan_universal);
    }

    /// ĐẠT CẦN DÁN: `risk_guard_gas_reserve` — `front_cap_after_gas_reserve`
    /// trừ đúng `gas_reserve_bnb_wei` khỏi trần, và không tràn âm khi
    /// `gas_reserve` > cap.
    #[test]
    fn risk_guard_gas_reserve() {
        let cap = bnb_f64_to_wei(1.5);
        let reserve_wei: u64 = 5_000_000_000_000_000; // 0.005 BNB, khop config ship
        let after = RiskGuard::front_cap_after_gas_reserve(cap, reserve_wei);
        assert_eq!(after, cap - U256::from(reserve_wei));

        // gas_reserve > cap -> saturating ve 0, khong panic/khong am.
        let tiny_cap = U256::from(100u64);
        let huge_reserve: u64 = 1_000_000_000_000_000_000;
        assert_eq!(RiskGuard::front_cap_after_gas_reserve(tiny_cap, huge_reserve), U256::ZERO);
    }

    /// ĐẠT CẦN DÁN: `risk_guard_consecutive_loss` — đếm lỗ liên tiếp, reset
    /// khi có 1 lần lãi, `max_consecutive_loss=0` tắt guard.
    #[test]
    fn risk_guard_consecutive_loss() {
        let mut guard = RiskGuard::new();
        assert!(!guard.consecutive_loss_exceeded(3));
        guard.record_result(true);
        guard.record_result(true);
        assert_eq!(guard.consecutive_loss(), 2);
        assert!(!guard.consecutive_loss_exceeded(3));
        guard.record_result(true);
        assert_eq!(guard.consecutive_loss(), 3);
        assert!(guard.consecutive_loss_exceeded(3));

        // 1 lan lai -> reset ve 0.
        guard.record_result(false);
        assert_eq!(guard.consecutive_loss(), 0);
        assert!(!guard.consecutive_loss_exceeded(3));

        // max_consecutive_loss=0 -> tat guard du dang lo lien tiep nhieu.
        guard.record_result(true);
        guard.record_result(true);
        guard.record_result(true);
        guard.record_result(true);
        assert!(!guard.consecutive_loss_exceeded(0));
    }

    /// `wallet_scan_enabled`/`pair_scan_enabled` la field bat buoc moi (cum
    /// `explicit-mode-flags`) - thieu field nao cung phai fail load, cung
    /// khuon cac field bat buoc khac.
    #[test]
    fn missing_explicit_mode_flags_fail_load() {
        for needle in ["wallet_scan_enabled = false\n", "pair_scan_enabled = true\n"] {
            let toml_str = base_toml().replace(needle, "");
            let err = Config::from_str(&toml_str).unwrap_err();
            match err {
                ConfigError::Parse(_) => {}
                other => panic!("expected Parse error khi thieu '{needle}', got {other:?}"),
            }
        }
    }

    /// Cụm `strategy-lock-mode2` (Chủ chốt 2026-09-15) — ship mặc định ĐỔI:
    /// CHỈ mode 2 (pair-mode) bật, mode 1 (wallet/victims.txt) TẮT. Đây là
    /// hành vi GỐC MỚI kể từ chiến lược này — KHÔNG được đổi lại `true` trừ
    /// khi Chủ ra lệnh quay về đa-mode (xem CLAUDE.md mục "Chiến lược đã chốt").
    #[test]
    fn explicit_mode_flags_ship_default_is_mode2_only() {
        let cfg = Config::from_str(&base_toml()).unwrap();
        assert!(!cfg.wallet_scan_enabled, "wallet_scan_enabled phai ship false (mode 2 only)");
        assert!(cfg.pair_scan_enabled, "pair_scan_enabled phai ship true (nguon candidate duy nhat)");
        assert!(!cfg.pair_scan_universal, "pair_scan_universal phai ship false (mode 3 TAT)");
    }

    #[test]
    fn explicit_mode_flags_can_be_set_true_independently() {
        let toml_str = base_toml()
            .replace("wallet_scan_enabled = false", "wallet_scan_enabled = true")
            .replace("pair_scan_enabled = true", "pair_scan_enabled = false");
        let cfg = Config::from_str(&toml_str).expect("to hop nguoc lai van phai load duoc (khong ep 1 mode)");
        assert!(cfg.wallet_scan_enabled);
        assert!(!cfg.pair_scan_enabled);
    }

    /// Cụm `usdt-quote-asset` (BAOCAO29) — 4 field mới bắt buộc, thiếu field
    /// nao cung phai fail load, cung khuon moi field bat buoc khac.
    #[test]
    fn missing_usdt_quote_asset_fields_fail_load() {
        for needle in [
            "scan_quote_usdt = false\n",
            "min_profit_usdt = 3.0\n",
            "max_front_usdt = 3000.0\n",
            "min_reserve_usdt = 15000.0\n",
        ] {
            let toml_str = base_toml().replace(needle, "");
            let err = Config::from_str(&toml_str).unwrap_err();
            match err {
                ConfigError::Parse(_) => {}
                other => panic!("expected Parse error khi thieu '{needle}', got {other:?}"),
            }
        }
    }

    /// Ship mac dinh `scan_quote_usdt = false` - AN TOAN, hanh vi WBNB khong
    /// doi gi khi tat (dung CLAUDE.md).
    #[test]
    fn scan_quote_usdt_ship_default_is_false() {
        let cfg = Config::from_str(&base_toml()).unwrap();
        assert!(!cfg.scan_quote_usdt);
        assert_eq!(cfg.min_profit_usdt, 3.0);
        assert_eq!(cfg.max_front_usdt, 3000.0);
        assert_eq!(cfg.min_reserve_usdt, 15000.0);
    }

    #[test]
    fn scan_quote_usdt_true_loads_ok() {
        let toml_str = base_toml().replace("scan_quote_usdt = false", "scan_quote_usdt = true");
        let cfg = Config::from_str(&toml_str).expect("scan_quote_usdt=true phai load duoc");
        assert!(cfg.scan_quote_usdt);
    }

    /// Moi field nguong USDT am deu fail load rieng biet, cung khuon
    /// `each_negative_threshold_field_fails_load` (WBNB).
    #[test]
    fn each_negative_usdt_threshold_field_fails_load() {
        for (needle, replacement) in [
            ("min_profit_usdt = 3.0", "min_profit_usdt = -1.0"),
            ("max_front_usdt = 3000.0", "max_front_usdt = -1.0"),
            ("min_reserve_usdt = 15000.0", "min_reserve_usdt = -1.0"),
        ] {
            let toml_str = base_toml().replace(needle, replacement);
            assert!(
                matches!(Config::from_str(&toml_str), Err(ConfigError::InvalidNumber(_))),
                "field am tu '{needle}' -> '{replacement}' phai fail load"
            );
        }
    }

    /// `min_profit_usdt=0`/`max_front_usdt` rat lon deu hop le (khong chan
    /// bien tren), cung luat cac nguong BNB.
    #[test]
    fn usdt_thresholds_zero_and_large_load_ok() {
        let toml_str = base_toml()
            .replace("min_profit_usdt = 3.0", "min_profit_usdt = 0")
            .replace("max_front_usdt = 3000.0", "max_front_usdt = 1000000");
        let cfg = Config::from_str(&toml_str).expect("min_profit_usdt=0 + max_front_usdt lon phai load duoc");
        assert_eq!(cfg.min_profit_usdt_wei(), U256::ZERO);
        assert_eq!(cfg.max_front_usdt_wei(), bnb_f64_to_wei(1_000_000.0));
    }

    #[test]
    fn usdt_wei_conversions_match_bnb_f64_to_wei() {
        let cfg = Config::from_str(&base_toml()).unwrap();
        assert_eq!(cfg.min_profit_usdt_wei(), bnb_f64_to_wei(3.0));
        assert_eq!(cfg.max_front_usdt_wei(), bnb_f64_to_wei(3000.0));
        assert_eq!(cfg.min_reserve_usdt_wei(), bnb_f64_to_wei(15000.0));
    }

    /// Cụm `strategy-lock-mode2` — 2 field mới bắt buộc, thiếu field nào cũng
    /// phải fail load, cùng khuôn mọi field bắt buộc khác.
    #[test]
    fn missing_pairs_vet_fields_fail_load() {
        for needle in ["pairs_vet_interval_sec = 600\n", "pairs_require_vetted = true\n"] {
            let toml_str = base_toml().replace(needle, "");
            let err = Config::from_str(&toml_str).unwrap_err();
            match err {
                ConfigError::Parse(_) => {}
                other => panic!("expected Parse error khi thieu '{needle}', got {other:?}"),
            }
        }
    }

    /// Ship mặc định `pairs_vet_interval_sec=600`/`pairs_require_vetted=true`
    /// (AN TOÀN — token chưa có `vetted` trong `pairs.txt` không được sim).
    #[test]
    fn pairs_vet_fields_ship_defaults() {
        let cfg = Config::from_str(&base_toml()).unwrap();
        assert_eq!(cfg.pairs_vet_interval_sec, 600);
        assert!(cfg.pairs_require_vetted);
    }

    /// Cụm `real-economics-mode2` (F-03) — 3 field mới bắt buộc, thiếu field
    /// nào cũng phải fail load, cùng khuôn mọi field bắt buộc khác.
    #[test]
    fn missing_gas_units_or_gas_price_max_fields_fail_load() {
        for needle in ["gas_units_front = 160000\n", "gas_units_back = 140000\n", "gas_price_max_gwei = 10\n"] {
            let toml_str = base_toml().replace(needle, "");
            let err = Config::from_str(&toml_str).unwrap_err();
            match err {
                ConfigError::Parse(_) => {}
                other => panic!("expected Parse error khi thieu '{needle}', got {other:?}"),
            }
        }
    }

    #[test]
    fn gas_units_and_gas_price_max_ship_defaults() {
        let cfg = Config::from_str(&base_toml()).unwrap();
        assert_eq!(cfg.gas_units_front, 160_000);
        assert_eq!(cfg.gas_units_back, 140_000);
        assert_eq!(cfg.gas_price_max_gwei, 10);
        assert_eq!(cfg.gas_price_max_wei(), 10_000_000_000u128);
    }

    #[test]
    fn pairs_require_vetted_false_loads_ok() {
        let toml_str = base_toml().replace("pairs_require_vetted = true", "pairs_require_vetted = false");
        let cfg = Config::from_str(&toml_str).expect("pairs_require_vetted=false phai load duoc");
        assert!(!cfg.pairs_require_vetted);
    }
}

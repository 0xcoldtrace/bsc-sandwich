# CONTRACT_DESIGN — Executor contract + kiến trúc ví (THIẾT KẾ, CHƯA CODE)

> **Trạng thái: TÀI LIỆU THIẾT KẾ.** Cụm `bugfix-presign-and-contract-plan`
> (PHẦN B) — lệnh ghi rõ: *"CHƯA viết Solidity, CHƯA deploy"*. Không có file
> `.sol` nào trong repo sau cụm này. Mọi con số gas/chi phí dưới đây là **ước
> lượng** (ghi rõ nguồn suy ra), TRỪ 2 số đã ĐO THẬT bằng `revm` ở BAOCAO38.
> Điều kiện go/no-go ở mục **B7** — chưa đạt thì KHÔNG deploy.

Mục lục:

- [B1. Kiến trúc ví](#b1-kiến-trúc-ví)
- [B2. Contract Executor](#b2-contract-executor)
- [B3. Luồng bundle + bribe](#b3-luồng-bundle--bribe)
- [B4. Bảo mật](#b4-bảo-mật)
- [B5. Gas ước tính, test plan, kế hoạch deploy](#b5-gas-ước-tính-test-plan-kế-hoạch-deploy)
- [B6. Thay đổi cần ở bot Rust](#b6-thay-đổi-cần-ở-bot-rust)
- [B7. Rủi ro còn lại + go/no-go](#b7-rủi-ro-còn-lại--điều-kiện-gono-go)

## Cơ sở thực nghiệm (không phải giả định)

Thiết kế dưới đây dựa trên 3 nguồn ĐÃ VERIFY trong repo này, không phải mẫu
chung trên internet:

1. **Cơ chế đối thủ thật** (BAOCAO41 mục 1 + "SỬA GIỮA PHIÊN", verify bằng
   RPC thật): ví kho `0xB406…d07eF` (nonce > 350.000) giữ USDT và **duy trì
   `approve`** cho contract dispatcher `0xa739Dfab…69758c`; mỗi lời gọi
   `0x5aab2274` phát ra một loạt cặp `(Transfer USDT kho→ví X, Approval mới)`
   — tức **cấp vốn tức thì cho nhiều ví "burner" cùng lúc** (tx
   `0x4916caa0f1…` block `122082156` cấp vốn 3 ví, cả 3 swap ngay 3 vị trí
   tx_index kế tiếp trên CÙNG pool). Ví trung tâm gom lãi:
   `0x8180aD6A…6c54` (nhận vốn 2850 lần / 50.000 block).
2. **Audit F-01/F-02** (`baocao/BAOCAO_AUDIT_2026-09-15.md`): bundle PHẢI
   nguyên tử 3 chân `[front, victim_raw, back]` (đã sửa ở `relay.rs`); mô hình
   bribe phải là chi phí trừ vào lãi TRƯỚC khi quyết định (đã sửa ở
   `pipeline::compute_bribe_wei`).
3. **Docs chính thức 2 relay** (Chủ dán 2026-09-16, pin trong
   `DEX_REGISTRY.md`): bribe trên BSC đi tới **ví EOA của builder** bằng
   **transfer BNB**, KHÔNG phải `block.coinbase`.

---

## B1. Kiến trúc ví

3 vai trò, **3 loại khóa ở 3 nơi khác nhau**. Nguyên tắc: khóa nằm trên VPS
chỉ được phép làm mất phần tiền tối thiểu nếu bị lộ.

| Vai trò | Số lượng | Giữ gì | Ký gì | Khóa nằm ở đâu |
|---|---|---|---|---|
| **Ví kho** (treasury) | 1 | Toàn bộ WBNB/USDT vốn kinh doanh | **KHÔNG ký giao dịch swap nào.** Chỉ ký đúng 2 loại: `approve(executor, ...)` và nạp/rút vốn | **KHÔNG BAO GIỜ ở VPS.** Ví cứng hoặc máy Chủ, ký thủ công, tần suất rất thấp |
| **Ví tay** (executor wallets) | 2–3 | Chỉ BNB trả gas (đề xuất ≤ 0.2 BNB/ví) | Ký chân front + chân back trong bundle | VPS (`.env` `PRIVATE_KEY`, `chmod 600`, gitignore) |
| **Ví owner** | 1 | Không giữ vốn | `withdraw` / `pause` / `setExecutor` của contract | Ví cứng hoặc máy Chủ. **KHÔNG ở VPS** |

Lý do tách đúng như cơ chế đối thủ đã quan sát: tiền nằm ở ví kho, **contract
lấy tiền bằng `transferFrom` tại thời điểm chạy**, ví ký giao dịch không hề
giữ tiền. Ví tay lộ khóa → kẻ tấn công chỉ lấy được vài chục đô BNB gas và
có thể *gọi* executor (xem B4 — vẫn không rút được tiền ra khỏi contract).

**Vì sao 2–3 ví tay chứ không phải 1**: bundle có 2 chân, và nonce phải liên
tục. Dùng 1 ví thì front nonce `n`, back nonce `n+1` — nếu 1 bundle rớt,
nonce `n` treo lại chặn mọi bundle sau. Dùng 2 ví (A ký front, B ký back) thì
2 chuỗi nonce độc lập, 1 bundle hỏng không kẹt ví còn lại; ví thứ 3 làm dự
phòng khi 1 ví đang có tx pending. **Đánh đổi phải ghi rõ**: 2 ví khác nhau
trong cùng bundle là chuyện bình thường với builder (bundle chỉ cần đúng thứ
tự), nhưng làm việc *ước lượng nonce* phức tạp hơn — `SelfNonceCache` (A4)
phải mở rộng thành map theo địa chỉ.

**Ví kho KHÔNG cấp `approve` vô hạn**: xem B4.

---

## B2. Contract Executor

Solidity `0.8.x` (0.8.24+, đã có `transient` nếu cần), **KHÔNG proxy** (không
upgradeable): proxy thêm 1 `delegatecall` mỗi lời gọi (~2.100 gas) và mở ra
cả một lớp rủi ro storage-collision, trong khi contract này nhỏ và rẻ để
deploy lại — cần sửa thì deploy bản mới rồi `setExecutor`/đổi địa chỉ trong
`config.toml`.

### Giao diện

Chọn **2 hàm riêng `frontRun` / `backRun`** (KHÔNG phải 1 hàm `sandwich`).
Lý do bắt buộc: victim tx nằm GIỮA 2 chân, 2 chân là **2 transaction riêng
biệt** trong bundle — không thể gói vào 1 lời gọi.

```solidity
function frontRun(
    address pair,          // pool V2 da vet (bot truyen, contract KHONG tu tim)
    address quote,         // WBNB hoac USDT (whitelist on-chain)
    address token,         // token mua vao
    uint256 frontIn,       // luong quote bo ra
    uint256 minTokenOut,   // chan truot gia (bot tinh = front_slippage_bps)
    uint256 deadlineBlock  // block.number <= deadlineBlock, neu khong -> revert
) external onlyExecutor whenNotPaused;

function backRun(
    address pair,
    address quote,
    address token,
    uint256 tokenIn,          // ban het so token chan front nhan duoc
    uint256 minQuoteOut,      // chan truot gia (back_slippage_bps)
    uint256 minProfitQuote,   // LAI ROI toi thieu SAU khi tru bribe
    address bribeTo,          // vi EOA builder (BlockRazor/48 Club, pin trong config)
    uint256 bribeAmount,      // BNB chuyen cho builder
    uint256 deadlineBlock
) external onlyExecutor whenNotPaused;
```

### Luồng bên trong

`frontRun`:
1. `require(block.number <= deadlineBlock)` → `E_DEADLINE`.
2. `require(quoteWhitelisted[quote])` → `E_QUOTE` (chỉ WBNB/USDT đã pin).
3. `require(frontIn <= maxFrontPerTx[quote])` → `E_FRONT_CAP` (trần ON-CHAIN,
   độc lập với `max_front_bnb` trong `config.toml` — xem B4).
4. `IERC20(quote).transferFrom(treasury, pair, frontIn)` — **chuyển THẲNG vào
   pool**, không qua contract (tiết kiệm 1 lần `transfer`).
5. Đọc `pair.getReserves()` + `pair.token0()`, tính `amountOut` bằng đúng
   công thức V2 phí 0.25% (giống `sim_v2.rs`), `require(amountOut >= minTokenOut)`
   → `E_MIN_OUT`.
6. `pair.swap(amount0Out, amount1Out, address(this), "")` — token về CONTRACT,
   không về ví tay.

`backRun`:
1–3. `deadlineBlock` / whitelist như trên.
4. Ghi `uint256 balBefore = IERC20(quote).balanceOf(address(this))`.
5. `IERC20(token).transfer(pair, tokenIn)` + `pair.swap(...)` nhận quote về
   contract. `require(quoteOut >= minQuoteOut)` → `E_MIN_OUT`.
6. **Kiểm lãi**: `uint256 gained = IERC20(quote).balanceOf(address(this)) - balBefore;`
   `require(gained >= minProfitQuote + bribeQuoteEquivalent)` → `E_NO_PROFIT`.
7. **CHỈ KHI ĐÃ QUA BƯỚC 6** mới trả bribe:
   `(bool ok, ) = bribeTo.call{value: bribeAmount}(""); require(ok)` → `E_BRIBE`.
8. Trả vốn gốc về ví kho (`IERC20(quote).transfer(treasury, frontIn)`), lãi
   giữ lại trong contract cho `owner` rút định kỳ (giảm số lần transfer mỗi
   vòng — mỗi `transfer` ERC20 ~25–30k gas).

**Thứ tự bước 6 → 7 là điểm thiết kế quan trọng nhất** (yêu cầu Chủ, BỔ SUNG
GIỮA PHIÊN): bribe chuyển ở **CUỐI**, **sau** khi đã xác nhận có lãi. Back
revert vì bất kỳ lý do gì → bribe **không hề rời contract**. Đây là khác biệt
so với kiểu bribe qua gas/priority fee: gas thì mất kể cả khi revert.

### Xác nhận độc lập: đối thủ ĐANG dùng đúng mẫu này (A8, 2026-09-16)

Phân tích tx `0x7574d418…cc8bc` (block `121919996`, Chủ chỉ định) cho kết quả
**xác nhận chéo** thiết kế trên, không phải suy đoán:

- 2 địa chỉ nhận USDT trong tx đó (`0x3164240e…6fa7ae`, `0x40cc5efd…b5e5a5`
  — chính 2 địa chỉ Chủ hỏi ở BAOCAO41, nay tra ra đầy đủ) **KHÔNG phải ví
  fee của builder/validator**: không phải `block.coinbase` của block đó, đều
  là EOA, và **VÀO = RA đúng bằng nhau** (326.34/326.34 và 19.68/19.68) — không
  giữ lại đồng nào.
- Chúng nhận USDT từ ĐÚNG 1 nguồn (kho `0xB406…`) rồi chuyển **TOÀN BỘ thẳng
  vào địa chỉ POOL** (`0xdfe23efb…`, `0x7fd71204…` — `eth_call` xác nhận cả 2
  là pair V2 thật: `factory = 0xcA143Ce3…` đã pin), số dư sau đó về 0.
- Nghĩa là đối thủ **chuyển token thẳng vào pair rồi gọi `pair.swap()`**, KHÔNG
  đi qua Router — đúng mẫu tiết kiệm gas mô tả ngay dưới đây.

Bằng chứng đầy đủ: `baocao/evidence/a8_two_usdt_recipients.txt`.

### Vì sao swap CẤP PAIR chứ không qua Router

`IPancakeRouter.swapExactTokensForTokens` phải: `transferFrom` về router →
tính path → gọi `pair.swap` → `transfer` cho `to`. Gọi thẳng `pair.swap` bỏ
được 1 lớp call + 1 lần transfer + toàn bộ vòng lặp path. Ước tính tiết kiệm
~25–40k gas/chân (xem B5). Giá phải trả: **contract tự tính `amountOut`** nên
công thức 0.25% phải khớp `sim_v2.rs` từng wei; sai lệch sẽ lộ ra ngay ở test
fork (B5) vì `minTokenOut` do bot tính.

### Danh sách ĐẦY ĐỦ revert reason

| Mã | Điều kiện | Chân |
|---|---|---|
| `E_AUTH` | `msg.sender` không thuộc danh sách executor | cả 2 |
| `E_OWNER` | hàm quản trị bị gọi bởi địa chỉ khác `owner` | admin |
| `E_PAUSED` | `paused == true` | cả 2 |
| `E_DEADLINE` | `block.number > deadlineBlock` | cả 2 |
| `E_QUOTE` | `quote` không thuộc whitelist (WBNB/USDT) | cả 2 |
| `E_FRONT_CAP` | `frontIn > maxFrontPerTx[quote]` | front |
| `E_MIN_OUT` | `amountOut < minTokenOut` / `quoteOut < minQuoteOut` | cả 2 |
| `E_NO_PROFIT` | `gained < minProfitQuote + bribe` | back |
| `E_BRIBE` | chuyển BNB cho builder thất bại | back |
| `E_TRANSFER` | `transferFrom`/`transfer` trả `false` hoặc revert | cả 2 |
| `E_REENTRANCY` | vào lại khi khoá đang bật | cả 2 |
| `E_NO_BALANCE` | contract không đủ BNB để trả bribe | back |

---

## B3. Luồng bundle + bribe

```
bundle = [
  tx0 = frontRun(...)   ky boi vi tay A   -> contract
  tx1 = victim_raw      RAW THAT cua victim (transport::fetch_raw_tx_verified)
  tx2 = backRun(...)    ky boi vi tay B   -> contract, CHUA bribe transfer o cuoi
]
```

### Bribe (CẬP NHẬT BỔ SUNG GIỮA PHIÊN 2026-09-16 — thay hẳn mô hình cũ)

Mô hình cũ trong repo (`bribe_mode="coinbase"`) **SAI với BSC**: bribe không
đi tới `block.coinbase`. Theo docs chính thức 2 relay (pin trong
`DEX_REGISTRY.md`):

| Relay | Ví EOA nhận bribe | Quy tắc |
|---|---|---|
| BlockRazor | `0x1266C6bE60392A8Ff346E8d5ECCd3E69dD9c5F20` | gas ≥ 0.05 gwei; bribe = transfer BNB |
| 48 Club Puissant | `0x4848489f0b2BEdd788c696e2D79b6b69D7484848` | xếp hạng = `0.9 × gas fee tx unique + BNB chuyển tới EOA` |

Hệ số `0.9` của 48 Club là lý do kỹ thuật để **luôn ưu tiên bribe qua
transfer** thay vì qua gas: 1 BNB trả bằng gas chỉ được tính 0.9 BNB khi xếp
hạng, 1 BNB chuyển thẳng được tính đủ. Vì vậy `bribe_mode` ship
`"builder_transfer"`, gas giữ mức tối thiểu 0.05 gwei, và **transfer bribe
nằm trong chân back** (tham số `bribeTo`/`bribeAmount` của `backRun`).

`revertingTxHashes`: **để rỗng** cho cả 2 relay — front/back không được phép
revert, và victim là tx public nên cũng không đưa vào danh sách.

### Victim bị bỏ hoặc revert — phân tích theo từng relay

Đây là câu hỏi sống còn: nếu victim không nằm trong block cùng bundle, bot tự
mua rồi tự bán và **lỗ chắc chắn** 2×0.25% phí + price impact.

| Tình huống | BlockRazor (`eth_sendBundle`, builder) | 48 Club Puissant |
|---|---|---|
| Builder loại cả bundle (không đủ hấp dẫn) | Không tx nào lên chain. **Mất 0** (không trả gas cho tx chưa on-chain) | Như trên |
| Victim revert on-chain | Bundle có `revertingTxHashes` rỗng → **cả bundle bị loại**, không lên chain. Mất 0 | Như trên |
| Builder cố tình tách bundle | `noMerge: true` yêu cầu không trộn với bundle khác; `positionFirst` xin vị trí đầu. Docs không cam kết tuyệt đối → **rủi ro còn lại, ghi ở B7** | `noMerge`/`positionFirst` cùng tham số |
| Chỉ front lên chain (trường hợp xấu nhất) | Không xảy ra với bundle nguyên tử đúng nghĩa; **nhưng** nếu từng phải fallback gửi lẻ qua RPC thường thì có thể → **CẤM gửi lẻ** (audit F-01, AGENTS.md) | như trên |

**Lớp phòng thủ cuối cùng nằm TRONG contract, không phụ thuộc lời hứa của
relay**: `backRun` revert khi `gained < minProfitQuote` (`E_NO_PROFIT`). Nếu
victim không chạy, giá không dịch, `gained` không đủ → back revert → cả bundle
bị loại → **không mất gì ngoài cơ hội**. Bribe cũng không mất (bước 7 sau
bước 6).

### Tra trạng thái bundle → `RiskGuard`

48 Club có API tra trạng thái bundle on-chain
(`docs.48.club/puissant-builder/bundle-submission-and-on-chain-status-query`).
Tầng gửi thật (`7.3`) phải gọi API này sau mỗi lần gửi, ghi `bundle.result`
vào `logs/bot.jsonl` và đẩy vào `RiskGuard::record_result` — đó chính là
nguồn tín hiệu "lỗ liên tiếp" mà `max_consecutive_loss` đang chờ (hiện
`record_result` chưa có nguồn thật nào ở đường paper, xem `docs/STATE.md`).

---

## B4. Bảo mật

1. **Reentrancy**: `pair.swap` gọi callback `uniswapV2Call` khi `data` khác
   rỗng — thiết kế này LUÔN truyền `data = ""` nên không có callback. Vẫn đặt
   khoá `nonReentrant` (transient storage, ~100 gas) trên cả 2 hàm: token lạ
   có thể gọi lại trong `transfer` (ERC777-style hook).
2. **Token lạ**: `pairs.txt` đã vet tay + vet nền, nhưng contract **không tin**
   — dùng `balanceOf` trước/sau (bước 4/6 của `backRun`) thay vì tin giá trị
   trả về của `transfer`; xử lý token không trả `bool` (USDT-style) bằng
   `SafeERC20`-pattern (low-level call + kiểm `returndata`).
3. **Approve tối thiểu**: ví kho **KHÔNG** `approve(type(uint256).max)`.
   Approve đúng hạn mức vòng vốn (ví dụ 50 BNB tương đương), Chủ gia hạn khi
   cạn. Ví kho lộ hạn mức → thiệt hại có trần. (Đối thủ `0xB406` làm ngược
   lại: `Approval` được làm mới liên tục trong mỗi lần cấp vốn — tiện hơn
   nhưng rủi ro hơn.)
4. **Trần `frontIn` ON-CHAIN**: `maxFrontPerTx[quote]` là biến storage do
   `owner` đặt. Đây là lớp độc lập với `max_front_bnb` của `config.toml`:
   config bị sửa (nhầm hoặc do VPS bị chiếm) cũng KHÔNG vượt được trần
   on-chain.
5. **Pause khẩn**: `owner` gọi `pause()` → mọi `frontRun`/`backRun` revert
   `E_PAUSED`. Khác `state/halt.lock` (chỉ dừng tiến trình bot): pause chặn ở
   tầng chain, vẫn hiệu lực kể cả khi VPS bị chiếm hoàn toàn.
6. **Ví tay lộ khóa — mất gì**: (a) BNB gas trong ví đó (≤ 0.2 BNB theo B1);
   (b) kẻ tấn công có thể gọi `frontRun`/`backRun` với tham số xấu. Chặn
   bằng: trần `maxFrontPerTx`, whitelist quote, `minProfitQuote` do CHÍNH
   contract kiểm (không tin tham số lãi 0), và `owner` có thể `setExecutor`
   loại ví bị lộ ngay. **Không mất** vốn ví kho (vốn chỉ chảy `treasury →
   pair` rồi `pair → contract`, và chỉ `owner` mới `withdraw` được).
7. **Không nhận BNB ngoài luồng**: `receive()` chỉ chấp nhận từ `owner` (nạp
   BNB để trả bribe), mọi nguồn khác revert — tránh bị "tặng" BNB để làm
   nhiễu kế toán.

---

## B5. Gas ước tính, test plan, kế hoạch deploy

### Gas

Mốc ĐO THẬT (BAOCAO38, `real_rpc_measure_gas_units_on_cake`, revm fork block
`122022012`) khi đi **qua Router**: front `121.916`, back `105.539`.

| Đường | front | back | Nguồn |
|---|---|---|---|
| Qua V2 Router (hiện tại) | **121.916** | **105.539** | ĐO THẬT, revm |
| Qua contract executor (swap cấp pair) | ~85.000–95.000 | ~75.000–85.000 | **ƯỚC LƯỢNG**: bỏ 1 lớp external call của Router (~2.600), bỏ 1 lần `transfer` ERC20 (~25.000), bỏ vòng lặp path + kiểm deadline của Router (~3.000–5.000); cộng lại `nonReentrant` + whitelist + kiểm trần (~3.000) |
| Bribe transfer (trong back) | — | +~9.000 | **ƯỚC LƯỢNG**: `call` chuyển BNB tới EOA đã có balance (2.300 + 6.700 overhead) |

Tiết kiệm ước tính ~30% gas/vòng. **Phải đo lại bằng foundry fork trước khi
tin** — bảng này chưa được dùng để kết luận kinh tế ở bất kỳ đâu.

### Test plan (foundry, fork BSC mainnet)

| # | Test | Kỳ vọng |
|---|---|---|
| 1 | Happy path: fork tại block có victim thật, chạy front → victim replay → back | `gained > 0`, bribe đã tới `bribeTo`, vốn về ví kho |
| 2 | Victim revert | `backRun` revert `E_NO_PROFIT`, **bribe KHÔNG rời contract** |
| 3 | Victim không có trong block (bundle bị tách) | như #2 |
| 4 | Token có tax 5% | `E_MIN_OUT` hoặc `E_NO_PROFIT`, không bao giờ lãi âm âm thầm |
| 5 | Honeypot (bán bị chặn) | `backRun` revert, mất đúng gas front |
| 6 | Token reentrancy (ERC777 hook gọi lại `backRun`) | `E_REENTRANCY` |
| 7 | `frontIn` vượt `maxFrontPerTx` | `E_FRONT_CAP` |
| 8 | Ví lạ gọi `frontRun` | `E_AUTH` |
| 9 | `paused=true` | `E_PAUSED` |
| 10 | `deadlineBlock` đã qua | `E_DEADLINE` |
| 11 | Đối chiếu `amountOut` contract vs `sim_v2::v2_amount_out` trên 100 mẫu ngẫu nhiên | khớp **từng wei** |
| 12 | Đo gas thật 2 hàm | điền vào bảng gas trên, thay số ước lượng |

### Deploy

1. **BSC testnet (97)**: deploy, chạy toàn bộ 12 test, chạy 1 vòng thật với
   token test. Chi phí ~0 (tBNB free).
2. **Mainnet (56)**: deploy 1 lần. Chi phí ước lượng: bytecode ~4–6 KB →
   ~1.2–1.6 triệu gas × 0.05 gwei ≈ **0.00006–0.00008 BNB**. Phí verify
   BscScan: 0.
3. **Nạp**: ví kho `approve` hạn mức; nạp ~0.05 BNB vào contract để trả bribe;
   `setExecutor([viA, viB])`; `setMaxFrontPerTx`.
4. **Bật dần**: `live_v2=true` trước, 1 ví tay, `max_front_bnb` nhỏ (0.1), theo
   dõi `bundle.result` ≥ 20 bundle rồi mới nâng.

---

## B6. Thay đổi cần ở bot Rust

| File | Thay đổi | Khối lượng ước lượng |
|---|---|---|
| `src/calldata.rs` | Thêm `encode_front_run(...)`/`encode_back_run(...)` (ABI contract mới, 6–10 tham số) thay cho `encode_front_buy`/`encode_back_sell` hiện gọi Router. 2 hàm cũ GIỮ LẠI (đường Router vẫn dùng khi chưa deploy contract) | ~150 dòng + 6 test |
| `src/executor.rs` | Thêm `contract_address` vào cổng `can_send_live` (thiếu địa chỉ contract + `live_mode="live"` → từ chối, cùng khuôn F-06 `to != 0x0`) | ~40 dòng + 3 test |
| `src/relay.rs` | ĐÃ CÓ sau cụm này: `build_blockrazor_builder_send_bundle_request` (auth), `builder_eoa_for`. Còn thiếu: tầng **gửi HTTP thật** (module MỚI, vì `relay.rs` giữ charter "không network") + parse phản hồi + tra trạng thái bundle 48 Club | ~250 dòng + test RPC thật |
| `src/config.rs` / `config.toml` | 3 field mới: `executor_contract` (address, rỗng = dùng đường Router), `executor_wallets` (danh sách địa chỉ ví tay), `treasury_address`. ĐÃ CÓ: `blockrazor_builder_eoa`, `club48_builder_eoa`, `bribe_mode="builder_transfer"` | ~80 dòng + 4 test |
| `src/transport.rs` | `SelfNonceCache` (A4) mở rộng từ 1 địa chỉ → map nhiều ví tay | ~60 dòng + 2 test |
| `src/main.rs` | Đường live: chọn ví tay (round-robin), ký 2 chân bằng 2 ví khác nhau, lấy `victim_raw` (`fetch_raw_tx_verified`), gọi tầng gửi, ghi `bundle.result` → `RiskGuard::record_result` | ~200 dòng |
| `src/web.rs` | `/api/status` thêm khối contract (địa chỉ, paused?, số dư bribe, hạn mức approve còn lại) | ~60 dòng |

**Tổng ước lượng: ~850 dòng Rust + ~20 test mới + 1 file Solidity ~250 dòng +
12 test foundry.** Đây là khối lượng của **cụm 6 `strategy-exec`**, không phải
việc làm kèm trong một cụm khác.

---

## B7. Rủi ro còn lại + điều kiện go/no-go

### Rủi ro không khử được bằng thiết kế

1. **Builder không cam kết giữ nguyên bundle.** `noMerge`/`positionFirst` là
   tham số *xin*, không phải bảo đảm. Phòng thủ duy nhất là `E_NO_PROFIT` ở
   chân back (mất gas, không mất vốn).
2. **Cạnh tranh không quan sát được.** Mọi phép đo cạnh tranh trong repo
   (BAOCAO40 14+29 case, BAOCAO41 bracket ±3, `compete.check`) chỉ nhìn được
   mempool CÔNG KHAI. Bot đối thủ gửi qua bundle riêng không lộ ra ở đó.
3. **`0.9 × gas fee` của 48 Club có thể đổi**; docs là nguồn duy nhất, không
   có cách verify on-chain.
4. **Ví tay trên VPS luôn là bề mặt tấn công.** Giảm thiểu được (B4), không
   khử được.
5. **Cụm đối thủ `0xB406`/`0xa739` hoạt động mạnh nhất trên pool USDT** —
   `pairs.txt` có 18 pool USDT. Cờ `allow_competitor_victims=false` (A3) tránh
   đối đầu, nhưng cũng cắt bớt cơ hội.

### Điều kiện GO / NO-GO (lệnh B7)

**CHỈ deploy khi CẢ 2 điều kiện dưới ĐỀU đạt, đo bằng chính BAOCAO của cụm
này hoặc cụm sau:**

| # | Điều kiện | Cách đo | Trạng thái tại BAOCAO42 |
|---|---|---|---|
| 1 | **Shadow ký kịp ≥ 50%** số candidate `Simulated` | `bundle.shadow` ÷ (`bundle.shadow` + `tx.abort`) trong 1 lần chạy ≥ 30 phút | ✅ **ĐẠT — 9/12 = 75%** (WSL, 30 phút, 2026-09-16; 3 abort đều là `vet_stale` ở 15 phút đầu khi vet nền chưa phủ hết 126 pool) |
| 2 | **`/api/econ` có pool quote WBNB KHÔNG bị cụm đối thủ phủ** | `top_pools_by_net[].competitor_touched == false` VÀ `net_pos > 0` cho ít nhất 1 pool **WBNB** | ❌ **KHÔNG ĐẠT** — 30 phút: **0 pool quote WBNB nào có lãi**; cả 2 pool có lãi (`0xdfe23efb…` net_pos=7, `0xcec13213…` net_pos=5) đều là **quote USDT** VÀ đều bị cụm đối thủ chạm (7 và 14 candidate là chính ví của cụm; `compete.result` bắt được 4 và 3 lần tx liền kề chạm cùng pool) |

### Kết luận go/no-go tại BAOCAO42: **NO-GO**

Điều kiện 1 đạt rõ ràng (độ trễ đường ký đã hết là nút thắt: p95 presign
**0.011 ms**). Điều kiện 2 **không đạt** — và đây mới là điều kiện quyết
định: toàn bộ lợi nhuận quan sát được đang nằm đúng trên 2 pool "sân nhà" của
cụm đối thủ, còn 108 pool quote WBNB trong `pairs.txt` không sinh ra cơ hội có
lãi nào trong 30 phút.

Vì vậy **KHÔNG viết Solidity, KHÔNG deploy** ở cụm này (đúng lệnh). Việc cần
làm TRƯỚC khi xét lại:

1. Chạy dài hơn (≥ 4–24 giờ) để biết "0 pool WBNB có lãi" là đặc điểm thật của
   danh sách pool hay chỉ là cửa sổ 30 phút quá ngắn.
2. Nếu vẫn 0: hoặc đổi danh sách pool WBNB (`pairs.txt`), hoặc chấp nhận cạnh
   tranh trực tiếp trên pool USDT (đặt `allow_competitor_victims=true`, có ý
   thức), hoặc chuyển hướng chiến lược — 3 lựa chọn này là quyết định của Chủ,
   không phải của Code.

---

## ArbExecutor (cụm `planB-B0-complete`, 2026-09-16) — THIẾT KẾ, CHƯA CODE

> **Không viết Solidity, không deploy.** Chỉ khi B0 Go (ô 10 BAOCAO47) mới
> sang cụm B1. Thiết kế này THAY THẾ `frontRun`/`backRun` sandwich ở B2 cho
> chiến lược backrun-arb; sandwich giữ trong tài liệu lịch sử, không xoá.

### Vai trò

1 contract, **không proxy**, Solidity 0.8.24+. Ví tay chỉ giữ BNB gas+bribe.
Contract **không giữ vốn**: vay flash → arb → trả nợ → gửi lãi về ví kho →
bribe builder, tất cả trong 1 tx. Không lãi → revert (mất gas, không mất
bribe nếu bundle không được chọn).

### 4 entry flash → 1 `_route`

Bốn hàm `external` chỉ khác nguồn vay; đều gọi `_route` nội bộ:

| Entry | Nguồn | Cơ chế |
|---|---|---|
| `flashInfinity(bytes route, uint256 minProfit)` | Infinity Vault `lock` | `lock` → `lockAcquired` → `take` → `_route` → `sync` + trả → `settle` |
| `flashAave(address asset, uint256 amount, bytes route, uint256 minProfit)` | Aave V3 Pool | `flashLoanSimple` / `executeOperation` |
| `flashBalancer(address[] tokens, uint256[] amounts, bytes route, uint256 minProfit)` | Balancer V2 Vault | `flashLoan` / `receiveFlashLoan` — trên BSC gần rỗng, giữ entry để nếu vault được nạp lại |
| `flashV2Pair(address pair, uint256 amount0Out, uint256 amount1Out, bytes route, uint256 minProfit)` | Pancake V2 `pancakeCall` | `pair.swap(..., data)` |

`route` encode (B0, V2-only): `(address token, address buyPair, address sellPair, address borrowQuote, uint256 borrow, bool needBridge, address bridgePair)`.
`_route` thực hiện: swap V2 mua pool rẻ → swap V2 bán pool đắt → (nếu khác quote) swap WBNB↔USDT qua `bridgePair`.

### Chân V3 (cụm `planB-B5-simarb-v3-measure` — thiết kế, CHƯA code)

Bổ sung 2 hop V3 vào `_route` (cùng `minProfit` / bribe / không giữ vốn):

| Family | Router đã pin | Calldata |
|---|---|---|
| Pancake V3 | SwapRouter `0x1b81D678ffb9C0263b24A97847620C99d213eB14` | `exactInputSingle((tokenIn,tokenOut,fee,recipient,deadline,amountIn,amountOutMinimum,sqrtPriceLimitX96))` — **có deadline** |
| Uniswap V3 BSC | SwapRouter02 `0xB971eF87ede563556b2ED4b1C0b0019111Dd85d2` | `exactInputSingle((tokenIn,tokenOut,fee,recipient,amountIn,amountOutMinimum,sqrtPriceLimitX96))` — **không deadline** |

`route` mở rộng: thêm `uint8 buyKind` / `sellKind` (`0=V2 pair`, `1=PCS V3`, `2=Uni V3`) + `uint24 buyFee` / `sellFee` (0 khi V2). `_route` chọn router + selector theo kind; approve max 1 lần lúc deploy cho cả 2 SwapRouter (WBNB/USDT). Không đoán tick/hooks — chỉ `exactInputSingle` 1 hop. Infinity CL vẫn ngoài phạm vi (chờ B4 sim).

Thứ tự hop không đổi: flash → mua venue rẻ → bán venue đắt → (bridge) → trả nợ → `minProfit` → bribe → treasury.

### `minProfit` revert

Sau khi trả nợ flash + phí, số quote còn lại phải `>= minProfit` (đơn vị
`borrowQuote`). Thấp hơn → `revert ProfitBelowMin()`. Đây là cổng duy nhất
quyết định tx thành công; bot tính `minProfit` = `max(min_profit_*, 1 wei)`
từ sim.

### Bribe khi thành công

Cuối `_route`, **chỉ khi** lãi ≥ `minProfit`:

```solidity
if (bribeAmount > 0 && bribeTo != address(0)) {
    (bool ok, ) = bribeTo.call{value: bribeAmount}("");
    if (!ok) revert BribeFailed();
}
```

`bribeTo` = EOA builder đã pin (`DEX_REGISTRY.md`: BlockRazor /
48 Club). Bribe nằm TRONG cùng tx nên chỉ chuyển khi tx thành công (bundle
không chọn → không mất bribe). Lãi còn lại `transfer` thẳng về `treasury`
(ví kho, immutables lúc deploy). Contract không `receive` vốn kinh doanh.

### Không giữ vốn

- Không `deposit`/`withdraw` vốn swap. Owner chỉ `withdraw` BNB/token **mắc
  kẹt** (dust / airdrop), `pause`, `setExecutor`.
- `onlyExecutor`: ví tay trên VPS. Owner ở ví cứng của Chủ.
- Approve router/pair được set 1 lần lúc deploy (`type(uint256).max`) cho
  WBNB/USDT/V2 Router — không approve token lạ.

### Bẫy `sync` / `settle` (Infinity)

Đọc `Vault.sol` 2026-09-16: `_settle` tính `paid = balanceOfSelf() -
reservesBefore`, mà `reservesBefore` chỉ được đặt bởi `sync()`. **Quên
`sync` trước khi chuyển token về Vault → `paid` sai → `lock` revert
`CurrencyNotSettled` → mất gas.**

Trình tự bắt buộc trong `lockAcquired`:

1. `take(currency, address(this), amount)`
2. `_route(...)` (2–3 swap V2)
3. `sync(currency)`
4. `IERC20(currency).transfer(vault, repay)`  // repay = amount (+ 0 phí)
5. `settle()`
6. kiểm `minProfit`, bribe, gửi lãi về treasury

Không `mint`/`burn` surplus token. Không `clear` (đốt dư dương). Delta phải
về 0 trước khi `lock` return.

Callback Aave/Balancer/V2: trả nợ TRƯỚC khi kiểm `minProfit` (nguồn flash
đòi repay trong cùng callback). Thứ tự: nhận flash → `_route` → trả nợ+phí
→ `minProfit` → bribe → treasury.

### Foundry fork test (B1, chưa làm)

- Fork BSC, pin block. Gọi từng entry với route 2 pool V2 đã vet.
- Case lãi: `minProfit` thấp → success, treasury tăng, vault/aToken/pair
  cân bằng.
- Case lỗ: `minProfit` cao / pool cân bằng → revert, 0 chuyển bribe.
- Case quên `sync` (test âm): phải `CurrencyNotSettled`.
- Case `onlyExecutor` / `pause`.
- Audit độc lập vòng 2 trước deploy (AGENTS.md B1).

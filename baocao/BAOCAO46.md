# BAOCAO46 — cụm `planB-backrun-opportunity` (B0)

## 1. LÁT

`planB-backrun-opportunity` cụm **B0** — Chủ CHỐT bỏ sandwich, chuyển sang
**backrun-arb nguyên tử bằng flash loan**; cụm này chỉ ĐO cơ hội bằng số, 0
tiền, không contract, không live.

**Chủ ra lệnh "dừng an toàn" giữa phiên.** Phiên dừng khi mới xong phần NỀN
(nguồn flash + math arb + config), **chưa chạy bất kỳ mục ĐO nào**. Ô 9 ghi
`CHƯA XONG`, ô 10 liệt kê đầy đủ phần còn lại. Không có mục nào trong lệnh bị
bỏ im lặng.

Làm được: mục 2 (một phần — xác minh nguồn flash), mục 3 (một phần — `sim_arb.rs`
+ config `strategy`), mục 1 (chỉ khảo sát sơ bộ, **chưa** sinh
`state/multi_venue.json`).
Chưa động tới: mục 4, 5, 6, 7, 8.

## 2. LỆNH NHẬN

Khối lệnh Grok `planB-backrun-opportunity`, mục 1→8. HEAD lệnh ghi `ae4efea`;
**HEAD THẬT lúc mở phiên là `edb4461`** (= `ae4efea` + 2 commit cuối của
BAOCAO45: `2591b18` rồi `edb4461`). Máy: WSL. Cấm `.env`, `live_mode="live"`,
sendRaw, contract, đụng bot VPS. Không subagent (luật #4 — phiên này không gọi
subagent nào).

Giữa phiên Chủ nhắn **"dừng an toàn"**.

## 3. FILE ĐỔI

Commit nội dung: `71e3c15002a20f12816d821b28616363e52bea91`.
Commit cuối phiên (chỉ điền hash vào chính file này): xem dòng `Commit:` cuối file.

| File | Trạng thái | Nội dung |
|---|---|---|
| `src/flash.rs` | **MỚI** | 4 nguồn flash, chỉ đọc chain, `choose_flash_source` |
| `src/sim_arb.rs` | **MỚI** | Math backrun-arb 2 chân + bridge, ternary search cỡ vay |
| `src/lib.rs` | sửa | khai báo 2 module mới |
| `src/config.rs` | sửa | +7 field, validate `strategy`, `strategy_is_backrun()`, fixture test |
| `src/pipeline.rs` | sửa | **CHỈ** fixture test (`test_config_toml`) — 0 dòng logic đổi |
| `config.toml` | sửa | +7 field, `strategy = "backrun"` |
| `docs/STATE.md` | sửa | +1 mục `planB-backrun-opportunity` (bằng chứng mã nguồn + số đo) |
| `baocao/BAOCAO46.md` | MỚI | file này |
| `baocao/evidence/baocao46_flash_sources.txt` | MỚI | getCode + chiều sâu + phí thật |
| `baocao/evidence/baocao46_two_venue_v2.txt` | MỚI | 87/128 token có 2 pool V2 |
| `baocao/evidence/baocao46_cargo_test.txt` | MỚI | output test thật |

**KHÔNG đụng**: `.env`, `pairs.txt`, `victims.txt`, `DEX_REGISTRY.md`,
`CLAUDE.md`, `docs/CONTRACT_DESIGN.md`, `src/main.rs`, `src/web.rs`, cờ live,
bot VPS.

7 field mới trong `config.toml` (thiếu = fail load, đúng khuôn mọi field khác):
`strategy` `arb_max_borrow_bnb` `arb_max_borrow_usdt`
`flash_source_interval_sec` `multi_venue_path` `gas_units_arb_infinity`
`gas_units_arb_v2flash`.

> **CẢNH BÁO cho Chủ trước khi deploy VPS:** `config.toml` trên VPS là file
> RIÊNG. Binary mới sẽ **fail load** với `config.toml` cũ (thiếu 7 field). Phải
> cập nhật `config.toml` VPS CÙNG LÚC với binary, nếu không bot VPS không boot.

## 4. LỆNH CHẠY

```
cargo build --release
cargo test --lib
cargo test --lib -- flash:: sim_arb::
curl -X POST ... eth_chainId / eth_getCode        (bsc-dataseed1.bnbchain.org)
cast call <token> "balanceOf(address)" <vault>    (chiều sâu 4 nguồn flash)
cast call <PFC> "getFlashLoanFeePercentage()"     (phí Balancer)
cast call <AavePool> "FLASHLOAN_PREMIUM_TOTAL()"  (phí Aave)
cast call <V2Factory> "getPair(address,address)"  (x2 quote x128 token)
```

## 5. OUTPUT THẬT

**Máy: WSL.** Chạy qua `cargo test` → theo luật #2 ghi **git HEAD lúc chạy =
`edb44610e70ca247dcfa5f5f54c35c7e5fae1462`**. `rustc 1.97.1 (8bab26f4f 2026-07-14)`.
`sha256sum target/release/bsc_sandwich` =
`965598cf20a3095ec0e3d9a9da2c992abef34ac6f0e1fa80ebe7b8caab42af39`
(**binary này CHƯA chạy thật lần nào** — không có paper run nào trong phiên).

```
--- cargo test --lib -- flash:: sim_arb:: (18 test MOI cua cum nay) ---
running 18 tests
test flash::tests::fee_percent_1e18_doi_sang_bps ... ok
test flash::tests::flash_fee_zero_bps_la_zero_va_lam_tron_len ... ok
test flash::tests::chon_nguon_re_nhat_con_du_sau ... ok
test flash::tests::fee_bps_doc_loi_thi_nguon_bi_loai_khong_coi_nhu_0 ... ok
test flash::tests::khong_nguon_nao_du_sau_thi_none_chu_khong_phi_0 ... ok
test flash::tests::pancake_v2_flash_fee_lon_hon_25bps_phang ... ok
test flash::tests::nguon_re_nhung_can_thi_bi_bo_qua_khong_phai_bao_loi ... ok
test flash::tests::flash_addresses_are_20_byte_hex_and_checksum_parse ... ok
test flash::tests::v2_flash_swap_chi_duoc_chon_khi_caller_cho_phep_va_khong_bao_gio_thang_nguon_0_phi ... ok
test sim_arb::tests::ap_victim_doi_dung_chieu_reserve ... ok
test sim_arb::tests::bribe_khop_pipeline ... ok
test sim_arb::tests::sanity_chan_so_vo_ly ... ok
test sim_arb::tests::hai_pool_can_bang_thi_arb_luon_lo ... ok
test sim_arb::tests::pool_lech_gia_thi_arb_co_lai_va_lai_nho_hon_do_lech ... ok
test sim_arb::tests::gas_va_bribe_tru_that_va_bribe_khong_am ... ok
test sim_arb::tests::phi_flash_lam_giam_lai_that_su ... ok
test sim_arb::tests::khac_quote_phai_co_bridge_that ... ok
test sim_arb::tests::best_arb_tu_chon_dung_huong ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 417 filtered out; finished in 0.01s

--- cargo test --lib (TOAN BO) ---
test result: ok. 417 passed; 0 failed; 18 ignored; 0 measured; 0 filtered out; finished in 0.26s
```

Số test: **399 → 417** (+18). File đầy đủ: `baocao/evidence/baocao46_cargo_test.txt`.

**KHÔNG có output runtime nào khác để dán** — không paper run, không
`real_rpc_*`, không `/api/*`. Đúng luật #3: không suy diễn kết quả cho thứ
chưa chạy.

## 6. CHAIN

`eth_chainId` = `0x38` (56) xác nhận TRƯỚC mọi `getCode`/`eth_call`.
RPC công khai `https://bsc-dataseed1.bnbchain.org` (đúng tiền lệ BAOCAO02/29 —
RPC dùng để pin, không phải RPC runtime của bot). Máy: **WSL**.
Bằng chứng đầy đủ: `baocao/evidence/baocao46_flash_sources.txt`.

```
--- eth_getCode (do dai bytecode, byte) ---
InfinityVault                    0x238a358808379702088667322f80aC48bAd5e6c4  getCode_len=8347
BalancerV2Vault                  0xBA12222222228d8Ba445958a75a0704d566BF2C8  getCode_len=24512
BalancerProtocolFeesCollector    0xce88686553686DA562CE7Cea497CE749DA109f9F  getCode_len=2880
AaveV3Pool                       0x6807dc923806fE8Fd134338EABCA509979a7e0cB  getCode_len=1933

--- chieu sau + phi THAT tai block 122212446 ---
token        infinity_vault_balanceOf            balancer_v2_vault_balanceOf
WBNB         188.925365039353240685              0.000435286748097377
USDT         35597526.377583619785329791         0.000000000000500461
ETH          74.293220896910964929               0.000000000000000016
BTCB         2.286165503107133871                0.000000000000000000

reservesOfApp(CLPoolManager , WBNB) = 132886104475849357421
reservesOfApp(BinPoolManager, WBNB) = 4306988033198381
  -> tong 2 app = 132.886 WBNB NHUNG balanceOf (cung block) = 188.925 WBNB:
     tran vay THAT la balanceOf, chenh 56.04 WBNB (29.7%)

Balancer ProtocolFeesCollector.getFlashLoanFeePercentage() = 0  (thang 1e18 -> 0 bps)
Aave V3 Pool.FLASHLOAN_PREMIUM_TOTAL()                     = 5  (bps -> 0.05%)
```

Khảo sát venue (`baocao/evidence/baocao46_two_venue_v2.txt`):
`tokens_checked=128  have_BOTH_v2_quotes=87` — qua
`V2Factory.getPair(token,WBNB)` + `getPair(token,USDT)`, cùng RPC.
**CHƯA đọc reserve** → đây là cấu trúc venue, KHÔNG phải danh sách pool đủ sâu.

## 7. REGISTRY

**CHƯA GHI `DEX_REGISTRY.md`.** Lệnh yêu cầu pin nguồn flash + IVault vào
registry kèm `source_url` + ngày + `eth_getCode`; dữ liệu đã có đủ (ô 6) nhưng
phiên dừng trước khi ghi. Đây là món nợ có sẵn nguyên liệu, phiên sau dán
thẳng từ `baocao/evidence/baocao46_flash_sources.txt`.

Vault Infinity `0x238a…5e6c4` **đã pin từ trước** (BAOCAO02, mục V4/Infinity,
getCode 8347 — khớp đúng số đo lại phiên này). 3 địa chỉ còn lại
(Balancer Vault / ProtocolFeesCollector / Aave V3 Pool) **chưa có hàng trong
registry**.

Không thêm/sửa/xoá pin nào khác.

## 8. KHÔNG LÀM

- Không viết/deploy contract; không gửi tx; không `sendRaw`; `live_mode` giữ `"off"`.
- Không đụng bot VPS, không SSH vào máy chạy bot.
- Không sửa `pairs.txt`, `.env`, `CLAUDE.md`, `DEX_REGISTRY.md`.
- Không xoá đường sandwich — chỉ thêm cờ `strategy` để tắt; `sim_v2.rs`,
  `sim_evm.rs`, `executor.rs` giữ nguyên 100 %.
- `strategy = "backrun"` **hiện chưa nối vào `main.rs`/`pipeline.rs`** — đặt cờ
  thành `"backrun"` lúc này **không đổi hành vi bot**. Nói rõ để không ai đọc
  config rồi tưởng bot đã chạy backrun.
- Không gọi subagent (luật #4).

## 9. CHỮ

**CHƯA XONG**

Lý do: Chủ ra lệnh dừng giữa phiên. 5/8 mục của khối lệnh chưa chạy, và
**không có số liệu nào để kết luận Go/No-Go** — đó mới là sản phẩm chính của
cụm B0.

## 10. CÒN NỢ / LÁT SAU

### A. Chưa làm — mục nguyên vẹn của khối lệnh

- **Mục 1 — `state/multi_venue.json`: CHƯA SINH.** Mới có khảo sát sơ bộ
  87/128 token (chỉ địa chỉ pair, **không có reserve**). Chưa có bin sinh file,
  chưa chạy `discover_v2 --quote both`, chưa ghi nhận ứng viên V3/Infinity.
- **Mục 2 — task nền 5 phút: CHƯA CÓ.** `flash.rs` có sẵn
  `read_flash_snapshot`, nhưng **chưa ai gọi nó**: chưa có
  `flash_source_task` trong `main.rs`, chưa có dòng log `flash.source` nào.
  Field `flash_source_interval_sec` hiện là cờ chết.
- **Mục 3 — kiểm chéo revm: CHƯA CHẠY.** `sim_arb.rs` chỉ được kiểm bằng 9
  test fixture tay. **Chưa có 1 case nào đối chiếu với EVM thật**, nên
  yêu cầu "≥20 case, lệch ≤2%" là MISSING hoàn toàn. Math arb hiện **chưa
  được chứng minh khớp chain**.
- **Mục 4 — bảng đo: CHƯA CHẠY CẢ (a) LẪN (b).** Không có số cơ hội/ngày,
  p50/p90 lãi, cỡ vay p80, % do cụm `0xB406` tạo, % bị backrun trước.
- **Mục 5 — gas_units: CHƯA ĐO.** `gas_units_arb_infinity = 420000` và
  `gas_units_arb_v2flash = 330000` trong `config.toml` là **ước lượng thô do
  Code chọn, KHÔNG phải số đo revm**. Phải đo lại trước khi dùng cho bất kỳ
  kết luận kinh tế nào.
- **Mục 6 — Go/No-Go: KHÔNG KẾT LUẬN ĐƯỢC.** Không có dữ liệu. Cũng chưa có
  bảng top 20 token thiếu venue thứ 2.
- **Mục 7 — `docs/CONTRACT_DESIGN.md` mục "ArbExecutor": CHƯA VIẾT.**
- **Mục 8 — kịch bản rủi ro vào `STATE.md`: VIẾT MỘT PHẦN.** Đã ghi rủi ro
  Balancer (rỗng + wind-down) và giới hạn quét log Infinity. **Chưa** ghi:
  Vault Infinity cạn token, bribe war ở vị trí backrun, pool thứ 2 quá mỏng,
  cụm `0xB406` biến mất.

### B. Chặn thật — cần Chủ quyết

- **Không truy cập được VPS đang chạy bot.** `~/.ssh/config` của WSL chỉ có
  VPS Singapore (`bsc-vps`), và máy đó `systemctl is-active` trả `inactive`,
  không có thư mục bot. Máy chạy bot của BAOCAO45 (`VPS-511043-157`) **không
  có trong SSH config**. Mục 4(a) (log shadow ≥6 h) **không thể làm** nếu Chủ
  không cấp IP/credential. Ghi `MISSING`, không suy diễn.

### C. Phát hiện đã có, phiên sau dùng ngay (chi tiết ở `docs/STATE.md`)

- **Balancer V2 trên BSC vô dụng** (0,000435 WBNB) — thứ tự ưu tiên thực tế
  phải là **Infinity → Aave (5 bps) → V2 flash swap (25 bps)**, không phải
  thứ tự trong khối lệnh. `choose_flash_source` không hardcode nên tự xử lý
  đúng, nhưng Chủ nên biết để không chờ Balancer.
- **Trần vay Infinity = `balanceOf`, không phải `reservesOfApp`** (chênh 29,7 %).
- **Trình tự `take → arb → sync → trả → settle`** — quên `sync` là revert
  `CurrencyNotSettled`, mất gas. Bẫy số 1 khi viết `ArbExecutor`.
- **Phí V2 flash swap là 25,06 bps chứ không phải 25 bps phẳng.**
- **Không enumerate được pool Infinity theo `poolId`** (hooks/fee tự do) →
  bắt buộc quét log, mà RPC free-tier chặn 5 000 block/lần.

### D. Nợ cũ chưa đụng (từ BAOCAO45 trở về trước)

`/api/econ` vẫn O(kích thước log); p95 nhóm đi tới sim vẫn 848 ms; tỉ lệ THẮNG
cuộc đua vẫn MISSING; đường `sim_engine="evm"` vẫn dùng trần gas cấu hình.

---

Commit: `71e3c15002a20f12816d821b28616363e52bea91` (toàn bộ nội dung cụm).
Commit sau đó chỉ điền hash này vào ô 3 + dòng này của `baocao/BAOCAO46.md`,
không đổi code — đúng tiền lệ BAOCAO45 (`edb4461`).

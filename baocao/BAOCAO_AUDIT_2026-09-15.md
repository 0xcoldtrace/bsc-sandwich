# BÁO CÁO AUDIT ĐỘC LẬP — SANDWICH BOT BSC

**Ngày:** 2026-09-15
**Người thực hiện:** auditor độc lập (AI). KHÔNG phải người viết code, KHÔNG phải người điều hành.
**Phạm vi:** repo local `C:\Users\Admin\Documents\bsc-sandwich` + VPS vận hành thật (SSH root, được cấp quyền).
**Quy ước redact:** IP/hostname VPS, URL RPC có key, `PRIVATE_KEY` — KHÔNG xuất hiện trong tài liệu này.

> **Nguyên tắc tự áp:** mọi con số trong báo cáo này đều đến từ (a) đọc trực tiếp code, hoặc
> (b) lệnh tôi tự chạy và dán output. Chỗ nào không kiểm được thì ghi rõ "KHÔNG KIỂM ĐƯỢC + lý do".
> Không suy đoán thành kết luận. Mục 6 liệt kê đầy đủ giới hạn.

---

## 1. TÓM TẮT ĐIỀU HÀNH

1. **Bot hiện KHÔNG THỂ mất tiền.** Toàn repo không có một lời gọi ký/gửi tx nào; `alloy` không bật
   feature signer; `relay.rs` là dead code; trên VPS `PRIVATE_KEY` rỗng và mọi cờ live đều tắt.
2. **Phát hiện lớn nhất: VPS KHÔNG chạy code đã audit.** `src/sim_evm.rs` (toàn bộ động cơ EVM của
   BAOCAO31/32/33) **không tồn tại** trên VPS; `Cargo.toml` VPS không có `revm`.
3. Hệ quả đo được: production `decode_fail` = **99.8%**, và **0 sim trong toàn bộ 1.8 GB log** —
   `sim.result`, `sim.evm`, `tx.build`, `venue.pick` đều bằng 0 kể từ lúc khởi động.
4. Tôi đã build code đã audit vào thư mục riêng trên VPS và chạy thật 17m45s: `decode_fail` tụt từ
   99.8% xuống **94/15330**, và EVM chạy thật (**515 `sim.evm`**). Bản mới thực sự sửa được lỗi.
5. **Nhưng chất lượng sim ngoài đời kém hơn nhiều so với test offline:** validator nhúng đo
   **12/18 = 66.7%** lệch ≤1% (2/2 block cô lập đạt 0.0%; 10/16 block KHÔNG cô lập đạt ≤1%,
   6 trường hợp lệch >1%, cao nhất 3.10%).
6. **`sim_error` chiếm 299/515 = 58%** số quyết định EVM — lỗi RPC/fork là chế độ hỏng chủ đạo,
   không phải trường hợp hiếm.
7. **Độ trễ EVM thật: p50 382ms, p95 10,689ms, max 11,217ms** (n=170) so với block time BSC 0.75s.
   p95 chậm hơn block time ~14 lần.
8. **Lỗi Critical tiềm ẩn mới phát hiện qua chạy thật:** `tx.build` (nơi sinh calldata) kích hoạt
   theo kết quả **công thức đóng**, TRƯỚC khi EVM phán quyết. Đo thật: **501 `tx.build` / 1 `simulated`**.
9. **Calldata sinh ra có `to = 0x000...000`** — quan sát trực tiếp trong log thật, không chỉ đọc source.
10. **Mức sẵn sàng live: 0.5/5.** Không phải vì code tệ, mà vì code đã audit **chưa từng được triển khai**,
    và không có git để phát hiện điều đó.

### 3 việc PHẢI làm trước khi cân nhắc live

| # | Việc | Vì sao |
|---|---|---|
| 1 | `git init` + commit + deploy theo commit hash | Chấm dứt tình trạng "code audit ≠ code chạy" mà không ai biết |
| 2 | Bundle nguyên tử `[front, victim, back]` + mô hình gas-price/bribe | Hiện bundle thiếu victim ⇒ lỗ chắc chắn; không có logic outbid victim |
| 3 | Dời `tx.build` xuống SAU phán quyết EVM; thay `Address::ZERO` bằng ví thật; back-sell dùng `balanceOf` | 3 lỗi này cùng nằm trên đường sẽ-thành-executor ở 7.3 |

---

## 2. BẢNG PHÁT HIỆN

### 2.1 Phát hiện trong CODE (F-xx)

| ID | Mức | Mô tả | Bằng chứng |
|---|---|---|---|
| F-01 | **Critical** | Bundle relay chỉ gồm `[front, back]`, **thiếu victim tx** ⇒ nếu nối live là lỗ chắc chắn (2×0.25% phí + impact) | `relay.rs:118`, `relay.rs:159` |
| F-02 | **Critical** | Không có mô hình gas-price/bribe. Front-run buộc phải outbid victim — không có logic nào | grep `gas_price` ngoài `sim_evm.rs` chỉ thấy `transport.rs` lưu trữ |
| F-26 | **Critical** | `tx.build` sinh calldata theo kết quả **công thức đóng**, TRƯỚC khi `run_evm_decision` phán quyết. Đo thật: **501 tx.build / 1 simulated** | `pipeline.rs:484-490` gọi trước `main.rs:989-999` |
| F-03 | **High** | `gas_wei()` dùng **trần gas** làm **chi phí gas** (0.006 BNB) — cao hơn gas thật của 2 swap V2 nhiều lần | `config.rs:371-373`, `config.toml:37-38` |
| F-04 | **High** | `RiskGuard::record_result` **không có call site sản xuất** ⇒ `max_consecutive_loss` vĩnh viễn vô hiệu | def `config.rs:481`; call chỉ ở `config.rs:900-917` (test); `risk_guard` chỉ `.read()` `main.rs:930` |
| F-05 | **High** | `gate_check` **thiếu điều kiện `version_pinned`** mà CLAUDE.md yêu cầu — 2 cổng live không đồng nhất | `executor.rs:64-91` vs `config.rs:441-450` |
| F-06 | **High** | Calldata build với `to = Address::ZERO`. **Quan sát trong log thật**: word `to` của `swapExactTokensForETH` = `0x000…000` | `executor.rs:144,253-254`; log `tx.build` trên VPS |
| F-07 | **High** | Back-sell dùng `amountIn = quote.front_out` (số cứng) thay vì `balanceOf` lúc chạy ⇒ tax/lệch làm `transferFrom` fail ⇒ **kẹt token** | `executor.rs:217` |
| F-08 | **High** | `front_slippage_bps=10` / `back_slippage_bps=50` **không được dùng ở đâu**; cả 2 chân dùng `executor_slippage_bps=50` | `executor.rs:253-254` |
| F-09 | **High** | Decoder bỏ sót `multicall`, `exactOutput*`, biến thể SmartRouter không-deadline, UR đa lệnh | `decoder.rs:356`, `:366-368`. **Xác nhận thực nghiệm: `venue_v3 = 0`** ở CẢ 2 lần chạy |
| F-10 | **High** | EVM chỉ chạy khi công thức đóng đã ra `Simulated` ⇒ EVM chỉ **từ chối** được, không **cứu** được | `main.rs:989-999` |
| F-11 | **High** | Fork revm mở lại **mỗi candidate**; `reset()` xoá sạch `accounts` ⇒ cold-fetch lặp lại | `main.rs:1057`; `sim_evm.rs:1001-1008` |
| F-12 | **Medium** | `tokio::spawn` **trước** khi lấy semaphore ⇒ task tích luỹ không giới hạn ở nhánh WS | spawn `main.rs:444` vs acquire `main.rs:874` |
| F-13 | **Medium** | Không kiểm nonce victim; `disable_nonce_check=true` | `sim_evm.rs:195` |
| F-14 | **Medium** | Skip reason `deadline` khai báo nhưng **không code path nào sinh ra**. Đo thật: `deadline: 0` | `venues.rs:177`; `/api/skips` |
| F-15 | **Medium** | `passes_router_gate(None) == true`; nhánh `watch_inject_file` không gọi gate | `main.rs:809-814`; chỉ 2 call site `:440`, `:583` |
| F-16 | **Medium** | Không cross-check `venue(tx.to)` với `venue(selector)` | `venues.rs:59-64` (chủ ý), `decoder.rs:249` |
| F-17 | **Medium** | Test `real_rpc_*` **pass rỗng**: `return` trước assert khi thiếu mẫu (11 chỗ) | `sim_evm.rs:1870-1873` trước assert `:1875`; `:2183-2189` trước `:2191`; +9 chỗ |
| F-18 | **Low** | `is_meaningful_candidate(victim.value, …)` dùng `msg.value` trong khi bộ lọc trên dùng `decoded.amount_in` ⇒ loại nhầm candidate token-in | `sim_evm.rs:1738` vs `:659`. Quan sát: 4 dòng `victim_in=0` khi tôi chạy test |
| F-19 | **Low** | `SpecId::CANCUN` + `basefee=0`; `b.number = fork_block` trong khi state là *sau* block đó | `sim_evm.rs:195-206` |
| F-20 | **Low** | 4 chỗ cộng offset không `checked_` ⇒ panic ở debug build (tokio cô lập task, không chết process) | `decoder.rs:187,195,221,240` |
| F-21 | **Low** | `redact_rpc_url` giữ **host** ⇒ provider để key ở subdomain vẫn lộ | `transport.rs:73-83` |
| F-27 | **Low** | `/api/validate` `within_1pct` = 2/18 (11.1%) **mâu thuẫn với chính `lech_pct` từng dòng** (tôi đếm tay: 12/18 ≤1%) | output `/api/validate`, mục 5.4 |
| F-22 | Info | `measure_roundtrip_via_router` dead code (0 call site) | `tax.rs:99` |
| F-23 | Info | Doc-comment `refine_front_in_on_fork` nói "lưới 5 điểm" nhưng code chạy **1 điểm** | `sim_evm.rs:981-990` vs `:1010` |
| F-24 | Info | **Không có commit git nào** (local lẫn VPS) | `git log` → "does not have any commits yet" |
| F-25 | Info | `PRIVATE_TX_URL` có trong `.env.example` nhưng không được đọc ở đâu trong `src/` | grep toàn repo |

### 2.2 Phát hiện VẬN HÀNH / BẢO MẬT trên VPS (V-xx)

| ID | Mức | Mô tả | Bằng chứng |
|---|---|---|---|
| V-01 | **Critical** | **Production chạy code cũ**, thiếu `sim_evm.rs` + 3 selector FOT ⇒ decode_fail 99.8%, **0 sim trong toàn bộ vòng đời** | manifest sha256; `sim.result:0`/`sim.evm:0` trên 1.8 GB log |
| V-02 | **Critical** | **SSH: root + password + không firewall + không fail2ban**, port 22 mở Internet | `PermitRootLogin yes`, `PasswordAuthentication yes`, `ufw inactive`, `iptables -P ACCEPT` |
| V-03 | **High** | `logs/bot.jsonl` **1.82 GB, không logrotate** | `stat`; không có rule trong `/etc/logrotate.d/` |
| V-04 | **High** | VPS **không phải repo git** ⇒ không truy vết/rollback/audit hồi tố được | `fatal: not a git repository` |
| V-05 | **High** | `BSC_WS` trỏ host **HTTP** (`bsc-dataseed1…`), `rpc_probe` trả **404** ⇒ chưa từng có WSS; chạy suy giảm bằng txpool polling | `rpc_probe`; `pending_source:"txpool"` |
| V-06 | **High** | **`halt.lock` KHÔNG dừng paper loop** — tồn tại 6.4h mà bot vẫn xử lý tx | halt.lock 22:29:57 vs `tx.skip` 04:52:23; `halt.triggered` = 0 dòng |
| V-07 | **Medium** | Unit systemd **transient**, `Restart=no` ⇒ mất sau reboot, không tự hồi phục sau crash | `UnitFileState=transient`, `FragmentPath=/run/...` |
| V-08 | **Medium** | `config.toml` quyền **666**, cây `src/` **777** — world-writable; `config.toml` chứa cờ live | `stat -c %a` |
| V-09 | **Medium** | Ngưỡng test còn nguyên trong production: `min_profit_bnb=0`, `min_reserve_wbnb=0`, `pair_scan_universal=true` | config.toml VPS |
| V-10 | **Medium** | **4/36 URL RPC hỏng** (3×429, 1×403) vẫn nằm trong pool failover | `rpc_probe` |
| V-11 | **Low** | Tax cache 3 entry stale (lệch 51,837 block so `tax_cache_blocks=30`) | `/api/tax` `fresh:false` |
| V-12 | **Info** | **Không có rò rỉ bí mật.** `PRIVATE_KEY` rỗng; 0 dòng log chứa khoá/URL-có-key/IP; 1 SSH key hợp lệ | quét 1.82 GB + journalctl + history |

---

## 3. PHƯƠNG PHÁP — TÔI ĐÃ TỰ CHẠY GÌ

| Lệnh | Kết quả |
|---|---|
| `cargo test` (local) | **246 passed, 0 failed, 10 ignored** + 9 bin test |
| `cargo test real_rpc_victim_prediction_matches_onchain --ignored` (local, RPC công khai) | **8/8 lệch 0.000000%**, 416s, block 121965760–121966581 |
| `sha256sum` toàn bộ `src/`, `web/`, meta (local + VPS) | bảng đối chiếu mục 5.1 |
| `rpc_probe` trên VPS với `.env` thật | 36 HTTP + 1 WSS, bảng mục 5.5 |
| Build code đã audit trên VPS + chạy paper **17m45s** | funnel/sim.evm/validate mục 5.4 |
| Quét 1.82 GB `bot.jsonl` tìm bí mật | 0 khớp mọi mẫu |

---

## 4. KẾT QUẢ MỤC A–F (AUDIT CODE)

### A. Đúng đắn của sim

**A1 — Cấu hình revm** (`sim_evm.rs:186-210`): `chain_id=56` ✓. Các điểm lệch so chain thật:
`disable_nonce_check=true` (`:195`); `SpecId::CANCUN` — không phải hardfork BSC thật, code tự ghi nhận
(`:201`); `basefee=0` (`:206`); `b.number = fork_block` trong khi `AlloyDB` đọc state *sau* block đó,
tức lệch 1 (`:204`); `b.timestamp` là của block cha nên check deadline **nới hơn** thực tế.
`read_balance` tách `caller` khỏi `owner` để né EIP-3607 (`:374-378`) — **đúng**, đã sửa bug thật.
Điểm lệch lớn nhất không nằm ở revm mà ở chỗ sim chạy front→approve→victim→back trong một block
**không có tx nào khác** (xem D2).

**A2 — Cấp số dư giả**: `insert_account_info(attacker, 1e24)` (`:229`) ghi đè account attacker trong
`CacheDB` cục bộ, hợp lệ cho EOA. **Storage override (slot balanceOf, slot 8 reserve) KHÔNG nằm trên
đường sản xuất** — `probe_erc20_balance_slot`/`reset_pair_reserves`/`refine_front_in_with_evm` chỉ
dùng ở (a) đo tax quote USDT (`:826`) và (b) test `#[ignore]` (`:2261`).
Token không chuẩn: proxy an toàn (storage ở proxy); rebase/reflection ⇒ probe trả `Err` ⇒ **fail-closed**;
blacklist/pause không bị probe phát hiện nhưng **được** bắt gián tiếp vì back-sell revert ⇒
`honeypot_or_tax` (`pipeline.rs:759`). **Xác nhận thực nghiệm**: `/api/tax` trên VPS ghi nhận 2 token
`honeypot:true` với `sell_bps:10000` — cơ chế này hoạt động thật trên dữ liệu sống.

**A3 — Tôi ĐÃ TỰ CHẠY** `real_rpc_victim_prediction_matches_onchain`:
```
TONG B4''.2: 8 tx du dieu kien, 8 lech <=1% (100.0%), 4 bi loai vi qua nho
RPC_ERRORS trong lan chay nay: -32005=0 khac=35
test result: ok. 1 passed; 0 failed        (416.41s)
```
8/8 dòng `lech=0.000000% ok=true`, block 121965760–121966581 — **khác hoàn toàn** block trong BAOCAO33.
⇒ **Claim BAOCAO33 được tái lập độc lập.**

**Ba giới hạn phải nói rõ:**
1. Kết quả 0.000000% gần như **tất yếu** khi replay đúng tx trên đúng state cha bằng cùng một EVM —
   nó validate *plumbing*, không validate *mô hình sandwich*.
2. Bộ lọc "đúng 1 Swap/block" (`sim_evm.rs:1808`) **loại bỏ đúng các block tranh chấp**. Trong log tôi
   chạy, nhiều candidate bị loại vì pair có 2/3/7 Swap trong block — đó chính là kịch bản MEV thật.
3. **Mục 5.4 chứng minh giới hạn này bằng số**: validator nhúng chạy trên block KHÔNG cô lập chỉ đạt
   10/16 ≤1%, lệch tới 3.10%.

**A4 — Thuật toán `front_in`**: có tối ưu thật ở tầng công thức đóng (`sim_v2.rs:114-155`, ternary
search 256 vòng, hợp lệ vì profit lõm với constant-product); **KHÔNG có tối ưu ở tầng EVM** —
`refine_front_in_on_fork` đánh giá **đúng 1 điểm** (`:1010`), dù doc-comment `:981-990` nói "lưới 5 điểm"
(F-23). Rủi ro sai dấu lãi/lỗ **thấp**: EVM đo profit thật tại điểm đó nên không thể "dương giả";
hệ quả thật là **bỏ sót**, thiên lệch bảo thủ.

**A5 — Victim không revert**: `amountOutMin` có kiểm (`sim_v2.rs:161` + `evm.victim_success`
`pipeline.rs:764`). **Deadline**: không có gate tường minh (F-14, đo thật `deadline:0`), nhưng EVM
replay dùng `block.timestamp` thật nên deadline hết hạn ⇒ revert ⇒ bắt được gián tiếp; ở
`sim_engine="v2"` thì **không kiểm gì**. **Nonce**: KHÔNG kiểm (`disable_nonce_check=true`).
`gas_limit(victim.gas.max(200_000))` (`:293`) **nâng** gas limit victim lên tối thiểu 200k, có thể khiến
tx thật OOG lại thành công trong sim. **Victim bị thay thế (same nonce) / mined trước front-run:
KHÔNG có xử lý nào** — không theo dõi hash, không hủy, không kiểm lại.

### B. Gate và filter

**B1 — Thứ tự gate thật** (`main.rs:869-1006`):
```
record_seen → passes_router_gate(tx.to ∈ 5 router)      → not_pancake_router
 → spawn → semaphore(4) → dry_run? (return nếu false)
 → precheck_token_and_venue                              → decode_fail | not_wbnb_pair
     ├ V3 → venue_unpinned  (KHÔNG gọi resolve_v2_reserves)
     └ V2 → resolve_v2_reserves                          → no_pool
         → decide_and_build_paper_v2 (wallet|pair|universal)
            → below_min → thin_liq → [tax gate BỎ QUA nếu evm] → RiskGuard
            → sim_v2 search → victim_would_revert → unprofitable
            → ***tx.build sinh calldata TẠI ĐÂY***   ← F-26
 → [nhánh USDT nếu not_wbnb_pair && scan_quote_usdt]
 → run_evm_decision (chỉ khi prior == Simulated)         → phán quyết cuối
```
- Tx tới sim mà chưa qua lọc `tx.to`: **có 1 trường hợp** — `to == None` được cho qua (`main.rs:811`),
  và `watch_inject_file` không gọi gate (chỉ 2 call site `:440`, `:583`).
- Tx V3/UR bị sim bằng pool V2: **KHÔNG**. Bug cũ đã sửa đúng — `main.rs:914-918` chặn `SwapVenue::V3`
  thành `venue_unpinned` **trước** `resolve_v2_reserves`.

**B2 — Inventory decoder.** Decode được (10): 6 selector V2 (3 cổ điển + 3 `*SupportingFeeOnTransferTokens`),
`exactInputSingle`/`exactInput` (**chỉ biến thể có deadline**), `execute(bytes,bytes[])` +
`execute(bytes,bytes[],uint256)` với đúng 2 command `V2_SWAP_EXACT_IN(0x08)`/`V3_SWAP_EXACT_IN(0x00)`.

Thiếu (đều → `decode_fail` tại `decoder.rs:356`):
- V2: `swapETHForExactTokens`, `swapTokensForExactETH`, `swapTokensForExactTokens`
- V3: `exactOutputSingle`, `exactOutput`; **biến thể SmartRouter không-deadline** — trong khi SmartRouter
  **đang nằm trong** `PANCAKE_ROUTERS`
- **`multicall(...)` cả 3 dạng** — UI PancakeSwap thường bọc `exactInputSingle` trong multicall
- UR: mọi command khác 2 command trên (`WRAP_ETH`, `PERMIT2_PERMIT`, `SWEEP`, `*_EXACT_OUT`…)

UR đa lệnh bị từ chối cứng: `if commands.len() != 1 || inputs.len() != 1 { return Err(DecodeFail) }`
(`decoder.rs:366-368`) — fail-safe nhưng loại gần hết UR thật.

`path.len() != 2` → `NotWbnbPair` (`decoder.rs:130-135`); V3 packed path phải đúng 43 byte (`:141-150`).

**Bằng chứng thực nghiệm cho mức nghiêm trọng: `venue_v3 = 0` ở CẢ HAI lần chạy** (production 6.6h và
bản mới 17m45s), dù `scan_v3=true` và 4 router V3/SmartRouter/UR đều trong allowlist.

**B3 — Tax gate khi `sim_engine="evm"`.** Xác nhận trong code: `pipeline.rs:340-348` và `:611-619` —
`if !cfg.sim_engine_is_evm() { …tax gate… }`. Thay thế: `run_evm_decision` đo tax bằng revm
(`main.rs:1062-1076`); không đo được ⇒ `honeypot_or_tax` (**fail-closed, đúng**).

Kẽ hở cho token tax động/anti-bot — **có thật, 2 tầng**:
1. TTL `tax_cache_ttl_sec = 600s` ≈ 800 block BSC. Token đổi tax/bật blacklist trong cửa sổ đó sẽ lọt.
2. Trong 1 lần sim, đo tax và chạy sandwich đều ở **cùng 1 block fork**. Token áp tax theo `block.number`,
   theo số lần mua, hoặc cooldown per-address sẽ cho kết quả khác khi bán ở block sau.
   `sim_evm.rs` **không mô phỏng độ trễ block giữa front và back**.

**B4 — Allowlist zero-tax (8 token)** (`venues.rs:294-303`): địa chỉ **đúng** so token chuẩn BSC
(WBNB, USDT, USDC, BUSD, USD1, CAKE, BTCB, ETH). Allowlist chỉ miễn *đo tax*, không miễn cổng khác —
đúng như doc ghi. Không token nào trong đó có fee-on-transfer. USDC/BUSD/USD1 có cơ chế blacklist/pause
tập trung, nhưng trường hợp đó swap sẽ revert và bị bắt ở tầng revert, không phải tầng tax ⇒ chấp nhận được.

### C. Nguồn dữ liệu và hiệu năng

**C1 — Độ phủ mempool.** Thứ tự WSS → `txpool_content` → `inject_only` (`main.rs:393-445`, `:502-593`).
Buffer `PENDING_WS_CHANNEL_SIZE=4096` (`transport.rs:29`); `SEEN_CAP=50_000` eviction FIFO (`main.rs:520,570`);
`pending_txpool_max_per_poll=32` mỗi 400ms ⇒ **trần 80 tx/s** cho đường txpool.
Drop đã được chứng minh bằng log của chính repo: `"subscription rot: channel lagged by 24"`.
**F-12**: `tokio::spawn` xảy ra **trước** khi lấy semaphore ⇒ nhánh WS không có trần, task tích luỹ
không giới hạn khi mempool đông.

**C2 — Latency. ĐO THẬT trên VPS** (n=170 lần EVM thực thi, `attempts>0`):
```
min = 269.1 ms    p50 = 382.3 ms    p95 = 10,689.0 ms    max = 11,217.4 ms
```
So block time BSC 0.75s: p50 lọt trong 1 block, **p95 chậm hơn ~14 lần**. Con số này CHƯA gồm
`resolve_v2_reserves` (2 `eth_call`) và hàng đợi semaphore.
Điểm nghẽn: (1) mở fork revm mỗi tx qua RPC, (2) `reset()` xoá cache buộc cold-fetch, (3) revm `!Send`
buộc block 1 worker thread, (4) semaphore 4.

**C3 — Fork cache dùng lại giữa các tx? KHÔNG.** `BlockForkCache::open` gọi **bên trong**
`run_evm_decision` ⇒ mỗi candidate (`main.rs:1057`). Fork không nằm trong `AppState` vì `revm::Evm`
là `!Send`. Tệ hơn: trong *cùng* một candidate, `measure_tax_cached` và `refine_front_in_on_fork`
mỗi cái gọi `reset()` xoá `accounts` ⇒ **hai lần cold-fetch cho cùng một tx**.
**Live có khả thi không? Không, với kiến trúc hiện tại** — cần worker-thread actor giữ fork theo block,
reset ở mức slot, và node BSC riêng.

### D. An toàn tài chính khi lên live

**D1 — Mọi điều kiện để 1 tx thật được ký/gửi.** Grep toàn repo
(`sendRawTransaction|send_raw_transaction|eth_sendTransaction|sign|signer|LocalSigner|PrivateKeySigner`):
**không có một lời gọi ký hoặc gửi nào.** Bằng chứng cấu trúc, mạnh hơn cờ:
- `Cargo.toml:27-34` — `alloy` **không bật** feature `signer-local`/`signers`
- `executor.rs:99-106` — `load_signer` trả `B256` thô, **không dẫn xuất được cả địa chỉ public**
- `relay.rs` — **0 call site** ngoài chính nó; không có HTTP client
- `handle_paper_tx` **return sớm nếu `!cfg.dry_run`** (`main.rs:880-885`) ⇒ đặt `dry_run=false` khiến bot
  **không làm gì cả**, không phải giao dịch thật
- Có test tự kiểm `no_send_raw_transaction_call_anywhere_in_src` (`executor.rs:557`)

| Điều kiện | `live_gate_ok` (`config.rs:441-450`) | `gate_check` (`executor.rs:64-91`) |
|---|---|---|
| allow_live / !dry_run / bot_armed | ✓ | ✓ |
| !halt.lock / chain_id==56 / live_v* | ✓ | ✓ |
| gas cap > 0 | ✓ | ✓ |
| **version_pinned** | ✓ | **THIẾU** (F-05) |

**Không có đường nào gửi tx bỏ qua cờ — vì không có đường gửi nào tồn tại.**

**D2 — Slippage / deadline / amountOutMin.** Đính chính tiền đề: **`front_slippage_bps=10` và
`back_slippage_bps=50` không được dùng ở đâu cả** (F-08); cả 2 chân dùng `executor_slippage_bps=50`.

Kịch bản mất tiền:
1. **Bundle không nguyên tử (F-01)** — `[front, back]` không có victim ⇒ bot tự mua rồi tự bán ⇒ **lỗ chắc chắn**.
2. **Front vào mà victim không vào** — victim bị thay thế/revert/bị bot khác kẹp trước. Bot giữ token,
   back-run bán vào pool chính nó vừa đẩy giá. **Không có logic phát hiện/hủy.**
3. **Kẹt token (F-07)** — back-sell dùng `amountIn` cố định; nhận ít hơn ⇒ `transferFrom` fail ⇒ revert ⇒
   **bot giữ token, mất toàn bộ `front_in`** tới khi can thiệp tay.
4. **Bị sandwich ngược** — back-slippage rộng, không có bảo vệ.
5. **`to = Address::ZERO` (F-06)** — **đã quan sát trong calldata log thật**. Hiện bị `dry_run` chặn build,
   nhưng nếu 7.3 nối signer mà quên thay ⇒ đốt toàn bộ token.
6. **F-26** — `tx.build` là điểm tự nhiên để gắn "gửi đi" ở 7.3. Đo thật **501 tx.build / 1 simulated**
   ⇒ nếu gắn send tại đó, bot gửi ~500 giao dịch lỗ mỗi 18 phút. Phán quyết EVM đến **quá muộn** để chặn.
7. deadline = wall-clock + 120s (`executor.rs:150-153`) — hợp lý, không phải rủi ro.

**D3 — RiskGuard có được gọi thật không?**

| Hàm | Định nghĩa | Call site sản xuất |
|---|---|---|
| `consecutive_loss_exceeded` | `config.rs:495` | ✓ `pipeline.rs:352` |
| `front_cap_after_gas_reserve` | `config.rs:503` | ✓ `pipeline.rs:355,621,750,2109` |
| **`record_result`** | `config.rs:481` | **KHÔNG CÓ** — chỉ `config.rs:900-917` (test) |

`risk_guard` chỉ được `.read()` (`main.rs:930`), **không bao giờ `.write()`** ⇒ `consecutive_loss`
vĩnh viễn = 0 ⇒ **`max_consecutive_loss=3` không bao giờ kích hoạt**.
`max_exposure_bnb` **có** ăn thật (`config.rs:409-416` → `pipeline.rs:355`); `gas_reserve_bnb_wei` cũng trừ thật.

**D4 — Relay.**

| Endpoint | Method | Bundle nguyên tử? |
|---|---|---|
| 48 Club Puissant | `eth_sendBundle` | **Có** |
| BlockRazor | `eth_sendMevBundle` | **Có** (docs: tối đa 50 tx) |
| `rpc.48.club` (cũ) | JSON-RPC thường | **Không** — `-32601 method eth_sendBundle does not exist` (`relay.rs:57-64`) |

**Nhưng code không dùng được khả năng đó**: `txs` hardcode đúng 2 phần tử `[front, back]`
(`relay.rs:118,159`), không có tham số victim.
**Gửi lẻ qua RPC riêng tư:** mất nguyên tử hoàn toàn ⇒ đúng kịch bản 2 và 3 ở trên; với sandwich
về cơ bản là **không nên làm**.
Trên VPS `PRIVATE_TX_URL` **rỗng** ⇒ public mempool, không relay nào đang được dùng.

**D5 — Bí mật.** Xem mục 5.3 — **không có rò rỉ thực tế**; 1 rủi ro tiềm ẩn (F-21) nếu đổi sang RPC
có key ở subdomain.

### E. Tính đáng tin của báo cáo

**Bối cảnh: `git log` → "does not have any commits yet"** (cả local lẫn VPS). ⇒ **Không thể kiểm chứng
hồi tố bất kỳ claim "FILE ĐỔI" hay số test lịch sử nào.** Chỉ trạng thái cuối là auditable (F-24).

**E1 — Đối chiếu claim định lượng**

| Báo cáo | Claim | Kết quả |
|---|---|---|
| 26 | 0 Simulated vì `measure_roundtrip_via_router` không tự chạy | **CONFIRMED** — `tax.rs:99`, 0 call site |
| 26 | Funnel VPS `decode_fail=85707` … | UNVERIFIABLE — `logs/`,`state/` gitignored, không artifact |
| 27 | `pair.parse_error=900` do trailing comment | CONFIRMED là bug thật, đã sửa ở `pairbook.rs:173` |
| 27 | **"KHÔNG phải lỗi logic pipeline"** | **CONTRADICTED** — chính `pipeline.rs:341-343` là nguyên nhân; BAOCAO33 sau đó gọi là "deadlock, BUG THẬT" |
| 28 | 2 script latency mới | CONFIRMED |
| 28 | NỢ: `tx.skip` luôn log `token:null` | CONFIRMED là bug thật |
| 29 | `USDT_GET_CODE_LEN=4413`, 4 config field, USDT không trừ gas | CONFIRMED (`venues.rs:48`, `config.rs:146-162`, `pipeline.rs:602`) |
| 29 | "đã sửa `tx.skip.token`" | **PARTIAL** — vẫn `None` cho `not_wbnb_pair`, chính test khẳng định (`main.rs:1240`) |
| 30 | `FunnelCounters` 9 field trong `main.rs` | **CONTRADICTED bởi code hiện tại** — 15 field trong `web.rs:163-183`, tên khác hẳn |
| 31 | `passes_router_gate`, `SEEN_CAP=50_000`, revm 43 | **CONFIRMED** (`main.rs:520,809`, `Cargo.toml:37`) |
| 32 | Sửa `read_balance` caller (EIP-3607) | **CONFIRMED** (`sim_evm.rs:374-378`) |
| 32 | Nới assertion `<` → `<=` | CONFIRMED tự nhận (`sim_evm.rs:1577`) |
| 33 | **246 passed, 10 ignored** | **CONFIRMED — tôi tự chạy** |
| 33 | 3 selector `*SupportingFeeOnTransferTokens` | **CONFIRMED — tôi tự đọc** `decoder.rs:76-84` |
| 33 | **B4''.2 = 9/9 lệch 0.000000%** | **CONFIRMED — tôi tự tái lập 8/8, block khác** |
| 33 | `ZERO_TAX_ALLOWLIST` 8 token | **CONFIRMED** `venues.rs:294-303` |
| 33 | Sửa deadlock tax gate | **CONFIRMED** `pipeline.rs:340-348`, `:611-619` |
| 33 | 28 bộ sandwich thật, funnel `seen:9019`, warm 6ms | UNVERIFIABLE — test `#[ignore]` + run cục bộ, không artifact |

**E2 — "Đã sửa bug X" mà bug vẫn còn?** Không có trường hợp sai hoàn toàn. Hai trường hợp nói quá:
`tx.skip.token` (BAOCAO29) chưa xong cho bucket `not_wbnb_pair`; `measure_roundtrip_via_router`
(BAOCAO26 ghi nợ) vẫn dead code.

**E3 — Test bị nới / pass rỗng.** **11 chỗ** trong `sim_evm.rs` biến "không đủ mẫu" thành **PASS**
bằng `println!("SKIP (khong phai FAIL)"); return;` đặt **trước** assert thật — đáng chú ý
`:1870-1873` trước `assert!(pass_pct >= 80.0)` ở `:1875`, và `:2183-2189` trước
`assert!(within_2pct >= 3)` ở `:2191`. Cộng `:1577` assert nới từ `<` thành `<= 0.5`.
**Hệ quả:** suite 246-test xanh chứa **ZERO** validation on-chain — cả 10 test on-chain đều `#[ignore]`
và khi chạy có thể pass với 0 mẫu. Không có `assert!(true)` hay assert tautological nào khác.

**Đánh giá E:** claim **cấu trúc** chính xác ở tỷ lệ rất cao — mọi thứ tôi kiểm đều tồn tại đúng chỗ.
Báo cáo **trung thực bất thường** về việc không đạt (BAOCAO33 ô 10 tự ghi "B4''.3 KHÔNG đạt số (1/3)").
Điểm yếu: (a) bằng chứng on-chain nằm trong test có thể pass rỗng; (b) không git history;
(c) BAOCAO27 khẳng định sai và làm mất 2 phiên; (d) **và nghiêm trọng nhất — không báo cáo nào nói rõ
rằng số liệu từ BAOCAO31 trở đi KHÔNG đến từ VPS production** (xem mục 5.6).

### F. Kinh tế

**F1 — Sandwich public mempool BSC với bot EOA + router call có kỳ vọng dương không?**
**Ý kiến: KHÔNG, gần như chắc chắn âm với kiến trúc hiện tại. Mức chắc chắn: cao.**

Trước hết phải bác một suy luận sai: funnel `venue_v2=125 → unprofitable=34 → simulated=0` **không**
chứng minh "không có cơ hội", vì `gas_wei()` trừ **trần gas 0.006 BNB** khỏi mọi profit (F-03).
⇒ **Phải sửa F-03 rồi đo lại** trước khi kết luận về kinh tế.

Kể cả sau khi sửa, kỳ vọng vẫn âm vì lý do cấu trúc:
1. **Không bundle nguyên tử (F-01) + không mô hình bribe (F-02)** ⇒ không cạnh tranh được vị trí trong block.
2. **Bằng chứng từ chính dữ liệu của họ**: 28 bộ sandwich thật/2400 block, **cả 28 đều route qua contract
   riêng, không phải EOA** (BAOCAO33 ô 5). EOA + 2 router call là mô hình đã bị đào thải.
3. **Độ phủ decoder ≈ 0 cho V3/UR (F-09)** — `venue_v3 = 0` ở cả hai lần chạy thật của tôi.
4. **Latency p95 10.7s vs block 0.75s (C2)** — tìm ra cơ hội thì đã quá muộn.
5. **`sim_error` 58%** — hơn nửa số quyết định EVM không hoàn thành được trên RPC công khai.

Từ 17m45s chạy thật: **1 `simulated`** trên 15,330 tx quan sát / 125 candidate V2.
Profit thô quan sát được (chưa trừ gas, đơn vị wei BNB): phần lớn âm lớn
(−0.0989 … −0.1379 BNB), hai giá trị dương: **+0.00964 BNB** và **+0.00458 BNB**.
Với `gas_wei` = 0.006 BNB, chỉ giá trị +0.00964 vượt qua ⇒ khớp đúng 1 `simulated`.

**Điều kiện cần để có kỳ vọng dương** (tất cả, không phải chọn một): contract executor nguyên tử;
bundle qua builder với bribe cạnh tranh; nguồn tx tốt hơn public mempool; vốn đủ; độ phủ decode đủ rộng.

**F2 — So sánh (đổi rủi ro, không thiết kế)**

| Hướng | Được | Mất / rủi ro |
|---|---|---|
| **Backrun-only** | Không cần đứng trước victim ⇒ **không cần bundle nguyên tử, không cần outbid**; thất bại chỉ mất gas | Biên mỏng hơn; cạnh tranh tốc độ thuần. **Bước kế tiếp hợp lý nhất từ code hiện tại** |
| **Mempool trả phí** | Thấy tx sớm hơn, ít drop (giải quyết C1/V-05) | Chi phí cố định ăn vào EV; không giải quyết F-01/F-02 |
| **Contract executor nguyên tử** | Loại bỏ F-06, F-07, F-26 và rủi ro front-vào-victim-không-vào; cho phép revert-nếu-không-lãi | Phải viết + audit Solidity (rủi ro mới, nghiêm trọng hơn bug Rust); vẫn **cần** bundle để đảm bảo thứ tự |

**Khuyến nghị:** nếu mục tiêu là kiếm tiền, **backrun-only + contract executor** có tỷ lệ rủi ro/phần
thưởng tốt hơn nhiều so với đuổi theo sandwich trên public mempool bằng EOA.

---

## 5. KẾT QUẢ MỤC G (AUDIT TRÊN VPS)

### 5.0 Snapshot (việc đầu tiên)

```
UTC vào máy    : 2026-09-15T04:48:40Z
tar            : /root/audit_snapshot_20260915T044840Z.tgz  (253,953,279 B)
sha256(tar)    : a2e91e852fb90dc86822cd62e8058a9bd5283675c2241c75eb701d09dbf841e1
git rev-parse  : fatal: not a git repository        ← KHÔNG PHẢI repo git
sha256(binary) : d79358ed1b006dd281ef28122cc6fdf31b7cba59cab75e99bbca440456e40467
PRE-STATE bot  : ĐANG CHẠY — pid 40884, elapsed 06:35:56
                 systemd unit bsc-sandwich-paper.service (transient, Restart=no)
```

### 5.1 G1 — Khác biệt local vs VPS

| Hạng mục | Local (đã audit) | VPS (đang chạy) |
|---|---|---|
| git | không có commit nào | **không phải repo git** |
| `src/sim_evm.rs` | có (2388 dòng) | **KHÔNG CÓ** |
| `Cargo.toml` → `revm` | `revm = "43"` | **không có revm** |
| `decoder.rs` → `SupportingFeeOnTransferTokens` | 3 selector | **0 occurrence** |
| `venues.rs` → `ZERO_TAX_ALLOWLIST` | 8 token | **0 occurrence** |
| `config.toml` → `sim_engine` | `"evm"` | **field không tồn tại** |
| `/api/funnel`, `/api/validate` | có | **trả rỗng** |
| binary mtime | — | 2026-09-14 20:47:11 |
| source mới nhất | — | 2026-09-14 20:06 (`relay.rs`) |
| rustc | 1.98.0 | **không cài** |

**GIỐNG HỆT (10 file):** `src/bin/rpc_probe.rs`, `executor.rs`, `logger.rs`, `relay.rs`, `sim_v2.rs`,
`sim_v3.rs`, `state.rs`, `victims.rs`, `web/style.css`, + `pairs.txt`, `victims.txt`

**KHÁC (17):** `calldata.rs`, `config.rs`, `decoder.rs`, `lib.rs`, `main.rs`, `pairbook.rs`,
`pipeline.rs`, `pool.rs`, `tax.rs`, `transport.rs`, `venues.rs`, `web.rs`, `web/app.js`,
`web/index.html`, `Cargo.toml`, `Cargo.lock`, `config.toml`

**THIẾU trên VPS (1): `src/sim_evm.rs`**

**Binary build từ code nào?** mtime binary `20:47:11` **sau** source mới nhất `20:06` ⇒ binary **khớp
đúng cây source đang có trên VPS**. Không có dấu hiệu bị thay. Kết luận: binary hợp lệ, **cây source là
bản cũ từ trước cụm `evm-validate-*`**. Không `strings` tìm được version (binary đã strip).

**Process thật:** binary `/root/bsc-sandwich/target/release/bsc_sandwich`, user `root`,
cwd `/root/bsc-sandwich`, khởi động bằng `systemd-run` **transient**
(`ExecStart=/bin/bash -c 'set -a; source .env; set +a; exec …/bsc_sandwich'`), `Restart=no`,
`FragmentPath=/run/systemd/transient/…` ⇒ **mất sau reboot, không tự restart khi crash.**

### 5.2 G2 — Cấu hình và trạng thái thật

`config.toml` VPS = 3436 B vs local 5430 B. Khác biệt cốt lõi:
```
dry_run=true  allow_live=false  bot_armed=false  live_v2/v3/v4=false     ✓ AN TOÀN
min_profit_bnb   = 0.0     ← local 0.002
min_reserve_wbnb = 0.0     ← local 20
pair_scan_universal = true ← local false
(KHÔNG có sim_engine / tax_cache_ttl_sec / front_slippage_bps / back_slippage_bps / scan_quote_usdt)
```
**File vs runtime: KHỚP** — hot-reload không gây lệch.

```
/api/status : {"chain_id":56,"state":"WATCHING","pending_source":"txpool","last_block":121968654,
               "uptime_sec":23881,"halt_lock":true,"dry_run":true,"allow_live":false,"bot_armed":false}
/api/skips  : decode_fail=886,461  not_wbnb_pair=1,574  honeypot_or_tax=270  no_pool=10
              below_min=0  not_in_list=0  unprofitable=0  victim_would_revert=0
/api/funnel : (rỗng)      /api/validate : (rỗng)      ← endpoint chưa tồn tại ở bản này
```

**Quét TOÀN BỘ 1.8 GB `logs/bot.jsonl`:**
```
sim.result: 0   sim.evm: 0   tx.build: 0   venue.pick: 0   bundle.build_preview: 0   halt.triggered: 0
```
**Bot chưa từng sinh ra MỘT kết quả sim nào trong suốt vòng đời.** Không phải "0 Simulated vì không có
cơ hội" — mà **không tx nào đi qua nổi bước decode** (decode_fail 99.8%).

Phân bố 400k dòng cuối: `decode_fail` 192,657 / 193,040 = **99.8%**; `pair.parse_error` **13,300**
(bug pairs.txt đã sửa ở local `pairbook.rs:173`, **chưa bao giờ tới VPS**; `pairs.txt` local ≡ VPS).

**`state/`:** chỉ có `halt.lock` (4 B, "halt", mtime 2026-09-14 22:29:57). **Không có** `disarm.req`,
`reset.req`, `tax_inject.jsonl`, `inject_tx.jsonl` ⇒ **không có tx giả/tax inject nào còn sót**.
`/api/tax` có 3 entry nhưng **cả 3 `fresh:false`** (lệch 51,837 block) ⇒ hết hạn, không ảnh hưởng khi live.

**Phát hiện V-06:** `halt.lock` tồn tại 6.4 giờ nhưng bot **vẫn xử lý tx** — dòng `tx.skip` mới nhất
`04:52:23`, tức 6h22m *sau* khi halt.lock xuất hiện. `halt.lock` chỉ đóng **cổng live**, **không dừng
paper loop**; `halt.triggered` = 0 dòng.

**`.env`** (chỉ tên biến + hình dạng):

| Biến | Tình trạng |
|---|---|
| `PRIVATE_KEY` | **RỖNG (0 ký tự)** ✓ |
| `PRIVATE_TX_URL` | **RỖNG (0 ký tự)** ⇒ public mempool |
| `BSC_HTTP` | **26 URL** trong 1 biến (chuỗi phẩy) |
| `BSC_HTTP_2..5` | 1 URL mỗi biến |
| `BSC_WS` | 1 URL — host là một **dataseed HTTP**, không phải WSS |

### 5.3 G3 — Bí mật, quyền, rò rỉ

**Quét 1.82 GB `bot.jsonl` + journalctl + bash_history — SẠCH:**

| Mẫu quét | Số dòng khớp |
|---|---|
| `PRIVATE_KEY` | **0** |
| `0x` + 64 hex | **0** |
| URL chứa `?`/`apikey`/`token=` | **0** |
| IP VPS | **0** |
| journalctl chứa key/hex64 | **0** |
| bash_history (19 dòng) in khoá | **0** |

Host RPC được log gồm một provider để key trong **path** — `redact_rpc_url` cắt path nên **key không bị
ghi**. Redaction hoạt động đúng trên dữ liệu thật.
`authorized_keys`: **1 key duy nhất**, comment `bsc-sandwich`, không có key lạ.
`find` toàn máy: chỉ 1 `.env` và 1 binary — **không có bản sao khoá/binary cũ rải rác**.

**Quyền file — có vấn đề:**

| Đường dẫn | Quyền | Đánh giá |
|---|---|---|
| `.env` | **600 root:root** | ✓ đúng |
| `state/`, `logs/` | 755 | chấp nhận được |
| **`config.toml`** | **666** | ✗ world-writable (chứa cờ live) |
| **repo root, `src/`, `src/*.rs`** | **777 / 666** | ✗ world-writable |
| `key/` | không tồn tại trên VPS | ✓ khoá SSH chỉ ở máy dev |

### 5.4 G6 — Chạy xác minh sống (code ĐÃ AUDIT, build riêng trên VPS)

Vì VPS thiếu `sim_evm.rs` + `revm` + toolchain, tôi (được Chủ đồng ý) cài Rust, build code đã audit vào
**`/root/audit_build`** (KHÔNG đụng production), chạy paper **17m45s** trên port scratch 18899 với
`sim_engine="evm"`, `pair_scan_universal=true`, `scan_quote_usdt=true`, ngưỡng thấp,
`dry_run=true / allow_live=false / bot_armed=false`, và `BSC_WS` override sang một WSS thật **trong
môi trường tiến trình con** (KHÔNG sửa `.env`).

Build: `Finished release profile in 1m 23s` (8 core).

**Funnel (17m45s, `uptime_sec=1065`):**
```
seen=15330  not_pancake_router=14987  decode_fail=94   not_wbnb_pair=101
venue_v2=125  venue_v3=0  no_pool=1  below_min=88  thin_liq=6
honeypot_or_tax=3  unprofitable=34  victim_would_revert=4  sim_error=15  simulated=0
```
**`/api/skips`:** `decode_fail=3619  below_min=1345  sell_direction=2362  not_quote_pair=324
unprofitable=547  sim_error=311  victim_would_revert=94  venue_unpinned=22  thin_liq=80
honeypot_or_tax=46  no_pool=81  deadline=0`

**Sự kiện:**
```
tx.seen 10590 | tx.skip 8838 | rpc.block 2368 | pair.resolve_fail 991
sim.evm 515   | tx.build 501 | validate.victim 18 | funnel.minute 18 | sim.result 1
```

**So sánh quyết định của production (code cũ) và code đã audit — cùng máy, cùng mempool:**

| Chỉ số | Production (code cũ, 6h38m) | Code đã audit (17m45s) |
|---|---|---|
| `decode_fail` | 886,461 (**99.8%** số skip) | 94 trong funnel / 3,619 trong skips |
| `sim.evm` | **0** | **515** |
| `tx.build` | **0** | **501** |
| `venue_v3` | không có counter | **0** |
| `simulated` | **0** | **1** (`sim.result` = 1) |

⇒ **Bản mới thực sự sửa được lỗi decode.** Nhưng vẫn `venue_v3 = 0` (F-09) và `simulated` gần như 0.

**Phân bố quyết định `sim.evm` (n=515):**
```
sim_error            299  (58.1%)
unprofitable         160  (31.1%)
honeypot_or_tax       46  ( 8.9%)
victim_would_revert    9  ( 1.7%)
simulated              1  ( 0.2%)
```
Trong đó **attempts=0: 345** (bị gate trước khi EVM chạy) và **attempts>0: 170** (EVM thực thi thật).
Quote split: `wbnb` 487, `usdt` 28.

**Độ trễ EVM thật (n=170, chỉ các lần `attempts>0`):**
```
min = 269.1 ms   p50 = 382.3 ms   p95 = 10,689.0 ms   max = 11,217.4 ms
```

**Tax đo bằng EVM — HOẠT ĐỘNG THẬT.** `/api/tax` ghi nhận nhiều entry đo tự động, gồm
**2 token `honeypot: true` với `sell_bps: 10000`** (mua được, bán revert) và 1 token `buy_bps: 99`.
Đây là năng lực mà công thức đóng **không bao giờ** phát hiện được — điểm mạnh thật của cụm C1.

**Validator nhúng `/api/validate` — phát hiện quan trọng nhất của G6:**

`total: 18`. Tôi tự đếm từ `lech_pct` của 18 dòng:

| Nhóm | Số dòng | ≤1% | >1% |
|---|---|---|---|
| `isolated: true` | 2 | **2 (100%)** — cả 2 đều 0.0% | 0 |
| `isolated: false` | 16 | **10 (62.5%)** | **6** — 1.11, 1.19, 1.61, 2.40, 2.40, 3.10% |
| **Tổng** | **18** | **12 (66.7%)** | **6 (33.3%)** |

⇒ **Xác nhận bằng số cho cảnh báo ở A3:** trên block **cô lập** sim khớp tuyệt đối (2/2 = 0.0%),
nhưng trên block **có tx khác chen vào** — tức điều kiện MEV thật — độ chính xác tụt xuống 62.5%,
sai số tới **3.10%**. Với sandwich, sai 3% ở `victim_out` đủ để đảo dấu lợi nhuận.

**F-27 — mâu thuẫn trong chính API:** `/api/validate` trả `within_1pct: 2`, `within_1pct_ratio: 0.111`,
trong khi 12/18 dòng có `lech_pct ≤ 1.0`. Bộ đếm dường như chỉ tính dòng `isolated:true`.
Tôi báo cáo **cả hai** con số và không phán xét con nào đúng — cần tác giả xác nhận.

**Chưa thu được:** phân bố `victim_in` theo bucket (<0.01 / 0.01–0.05 / …) — `tx.skip` và `sim.evm`
**không log `amount_in`**, nên không trích được từ log. **KHÔNG KIỂM ĐƯỢC**, không suy đoán.

**Test `#[ignore] real_rpc_*` trên VPS:** **KHÔNG CHẠY** — Chủ yêu cầu dừng sớm để viết báo cáo.
Đã chạy bản tương đương ở local (mục A3, 8/8).

### 5.5 G5 — Vận hành và hiệu năng

```
timedatectl : synchronized: yes ✓          disk: 34G, còn 29G
RAM         : 7948 MB tổng, bot RSS 766 MB (9.4%)      CPU: 8 core, bot 7.8%
logs/bot.jsonl : 1,823,220,837 B (1.82 GB), KHÔNG có logrotate rule
rustc       : VPS không cài (local 1.98.0; bản tôi cài tạm để build: 1.98.1)
cargo build --release (code đã audit, trên VPS) : thành công 1m23s
```

**`rpc_probe` — RTT thật đo TẠI VPS (36 HTTP + 1 WSS, 5 mẫu/URL):**

| Nhóm | p50 | Ghi chú |
|---|---|---|
| Nhanh nhất (~11 URL) | **5.6 – 10.8 ms** | vị trí rất tốt so cụm RPC US |
| Trung bình (~9 URL) | 12 – 42 ms | |
| Chậm (~12 URL) | 125 – 876 ms | |
| **Lỗi (4 URL)** | — | 2×429 rate-limit, 1×429 throughput, 1×**403** |
| **WSS (1 URL)** | — | **404 Not Found** ⇒ không có WSS hoạt động |

⇒ Độ trễ mạng **không phải** nút thắt. Nút thắt là kiến trúc (C2/C3) và thiếu WSS (V-05).

### 5.6 Mâu thuẫn quy trình giữa các BAOCAO và thực tế VPS

| Claim | Thực tế trên VPS |
|---|---|
| BAOCAO26/27/28: `decode_fail` áp đảo, 0 Simulated | **KHỚP** — nay đã rõ nguyên nhân gốc: thiếu 3 selector FOT |
| BAOCAO27: `pair.parse_error` do trailing comment | **VẪN ĐANG XẢY RA** — 13,300 dòng |
| BAOCAO31: "đã sửa `pairbook.rs:173`" | Đúng ở local, **chưa bao giờ tới VPS** |
| BAOCAO33: 3 selector FOT, `sim_engine="evm"`, funnel `seen:9019` | **Không có trên VPS** — chạy ở môi trường khác |

⇒ **BAOCAO26–28 báo cáo số liệu VPS; BAOCAO31–33 báo cáo số liệu từ môi trường khác nhưng không nói rõ**,
khiến người đọc dễ tin rằng bot production đã được nâng cấp. **Nó chưa.**

### 5.7 G7 — Thử phá

| Thử nghiệm | Kết quả |
|---|---|
| **halt.lock khi bot đang chạy** | **ĐÃ CÓ CÂU TRẢ LỜI từ quan sát, không cần tạo mới**: halt.lock tồn tại sẵn 6.4h, bot **vẫn xử lý tx**. Halt chỉ chặn cổng live, **không dừng paper loop**, không ghi `halt.triggered` |
| **Cắt WS → fallback txpool** | **ĐÃ CÓ CÂU TRẢ LỜI từ quan sát**: `BSC_WS` vốn đã hỏng (404). Bot **không halt**, fallback sang `txpool_content` đúng thiết kế — nhưng đã âm thầm chạy suy giảm suốt từ đầu |
| Config sai (thiếu field / chain_id≠56 / số âm) | **CHƯA CHẠY** — Chủ yêu cầu dừng sớm |
| Inject tx honeypot qua `inject_tx.jsonl` | **CHƯA CHẠY** trực tiếp. Tuy nhiên cơ chế đã được chứng minh gián tiếp: `/api/tax` bắt được **2 honeypot thật** (`sell_bps:10000`) từ mempool sống |

---

## 6. NHẬT KÝ THAY ĐỔI VPS + XÁC NHẬN KHÔI PHỤC

### 6.1 Nhật ký (nội dung `/root/audit_changes.log` trước khi xoá)

```
2026-09-15T04:49:11Z | mkdir /root/audit_snapshot + tar audit_snapshot_20260915T044840Z.tgz
                     | snapshot bắt buộc trước audit | rollback: rm -rf …
2026-09-15T04:56:16Z | ./target/release/rpc_probe (đo RTT G5)
                     | ghi artifacts/rpc_probe.json | rollback: khôi phục từ tar
2026-09-15T05:01:27Z | rustup install (CARGO_HOME/RUSTUP_HOME=/root)
                     | cần toolchain build code đã audit cho G6 | rollback: rustup self uninstall
2026-09-15T05:01:45Z | mkdir /root/audit_build + giải nén code đã audit (KHÔNG gồm .env/key/state/logs)
                     | G6 cần code có sim_evm.rs | rollback: rm -rf /root/audit_build
2026-09-15T05:05:03Z | chạy audit_build/…/bsc_sandwich config.audit.toml (port 18899, dry_run=true,
                       BSC_WS override trong MÔI TRƯỜNG CON — KHÔNG sửa .env)
                     | thu số liệu G6 | rollback: kill tiến trình, rm -rf /root/audit_build
```

### 6.2 Khôi phục — Chủ chọn "xoá hết, trả máy nguyên trạng 100%"

```
1. dừng bot scratch                      → stopped (1 tiến trình còn sót pid 354218 đã kill -KILL)
2. khôi phục artifacts/rpc_probe.json    → RESTORED, mtime gốc 2026-09-14 16:29:25
3. rm -rf /root/audit_build              → removed
4. gỡ Rust (rustup self uninstall + rm)  → cargo removed, /root/.cargo và /root/.rustup không còn
5. rm snapshot + tar + audit_changes.log → không còn
```

**XÁC NHẬN CUỐI:**
```
bot pid 40884            : ĐANG CHẠY (elapsed 07:12:00) — KHÔNG dừng/khởi động lại
service                  : active
sha256 binary            : d79358ed1b006dd281ef28122cc6fdf31b7cba59cab75e99bbca440456e40467  (KHỚP snapshot)
sha256 config.toml       : b2af8c19b073cdd5fadd51c15b267b7a9cdc1ca406159b40c084b1c454e1c5c2  (KHỚP snapshot)
state/                   : chỉ halt.lock (nguyên trạng)
.env                     : mtime 2026-09-14 22:11:11 — KHÔNG đụng (chỉ đọc)
artifacts/rpc_probe.json : mtime 2026-09-14 16:29:25 (bản gốc)
logs/bot.jsonl           : 1,823,220,837 B — TĂNG so lúc bắt đầu (bot vẫn ghi), KHÔNG bị xoá
src/ file count          : 19 (nguyên trạng)
cổng lắng nghe           : 127.0.0.1:8787 (bot), 127.0.0.53:53, 0.0.0.0:22 — ĐÚNG như lúc vào
file audit còn sót /root : NONE
```

**Những gì tôi KHÔNG khôi phục:** không có. Toàn bộ thay đổi đã được hoàn tác theo yêu cầu.

**Một điểm ghi nhận trung thực:** `df` lúc 04:50 báo 7.2G used, sau dọn dẹp báo 5.2G used.
Tôi **không đo `df` trước khi tạo tar**, nên **không có baseline thật để so sánh** và
**không kết luận nguyên nhân chênh lệch**. Điều tôi kiểm chứng được và đã kiểm: mọi file production
còn nguyên (hash binary + config khớp snapshot, `logs/bot.jsonl` **tăng** chứ không giảm, `.env`/`state/`
/`victims.txt`/`pairs.txt` nguyên mtime gốc), và tôi **không xoá bất cứ thứ gì dưới `/root/bsc-sandwich`**.

**Tôi KHÔNG làm:** không bật cờ live nào, không ký/gửi tx, không gọi relay, không commit/push,
không xoá log/state cũ, không tạo user/SSH key, không sửa `.env`, không dừng bot production.

---

## 7. GIỚI HẠN CỦA AUDIT

**Đọc TRỰC TIẾP:** `sim_evm.rs` (1–1260), `pipeline.rs` (280–580, 700–850), `main.rs` (425–446, 560–600,
800–1130), `executor.rs` (1–360), `relay.rs` (1–215), `sim_v2.rs` (60–164), `config.toml`, `Cargo.toml`,
`.env.example`, `venues.rs:294-312`, `decoder.rs:45-95,186-245`, `transport.rs:70-95`,
`docs/STATE.md` (1–626/3222), `baocao/BAOCAO33.md`, + ~30 lệnh grep/sed xác minh chéo.

**CHẠY THẬT:** `cargo test` (246 passed/10 ignored); `real_rpc_victim_prediction_matches_onchain --ignored`
(416s, 8/8 khớp 0.000000%); trên VPS: `rpc_probe`, `cargo build --release`, paper run 17m45s, quét 1.82 GB log.

**Đọc QUA MÔ TẢ của subagent** (đã tự xác minh lại các điểm then chốt bằng grep/sed trực tiếp):
bảng inventory đầy đủ `decoder.rs`; đối chiếu chi tiết BAOCAO26–32.

**KHÔNG đọc:** `web.rs` (trừ grep secret), `pool.rs`, `pairbook.rs`, `tax.rs` (trừ grep), `victims.rs`,
`state.rs`, `logger.rs`, `calldata.rs` (trừ grep), `src/bin/rpc_probe.rs`, `web/*`, `scripts/*`,
`docs/STATE.md` 627–3222, `docs/TASKS.md`, `DEX_REGISTRY.md`, `baocao/BAOCAO01–25`.

**KHÔNG chạy:** 9 test `#[ignore]` còn lại; test `#[ignore]` trên VPS; G7 config-fail-load và
inject-honeypot trực tiếp; đo latency end-to-end "thấy tx → gửi bundle"; bất kỳ `curl` nào tới endpoint relay.

**KHÔNG KIỂM ĐƯỢC (và không suy đoán thành kết luận):**
- Phân bố `victim_in` theo bucket — `tx.skip`/`sim.evm` không log `amount_in`.
- Mọi số liệu runtime trong BAOCAO26–33 (`logs/`,`state/` gitignored, không artifact).
- Độ phủ mempool theo phần trăm — tôi không đo throughput mempool BSC độc lập.
- Hệ số sai của mô hình gas theo BNB tuyệt đối — tôi xác minh được `gas_wei()` dùng *trần* thay vì
  *chi phí* (đọc code), nhưng không gọi `eth_gasPrice` để ra hệ số chính xác.
- Hình dạng `params` thật mà 48 Club/BlockRazor chấp nhận (không gửi request thật).
- Nguyên nhân chênh lệch `df` nêu ở mục 6.2.

**Tôi là AI.** Không sửa code, không gửi tx, không dùng key nào. Mọi thao tác trên VPS đã ghi nhật ký
và hoàn tác.

---

## 8. KẾT LUẬN

**Mức sẵn sàng live: 0.5 / 5.**

Hạ từ 1/5 (sau audit code) xuống 0.5/5 sau khi audit VPS, vì lý do không nằm ở chất lượng code:
**codebase đã audit chưa từng được triển khai.** Khoảng cách giữa "code được review" và "thứ đang chạy"
nghiêm trọng hơn bất kỳ bug đơn lẻ nào — nó có nghĩa là mọi bản sửa trong tương lai cũng có thể không
bao giờ tới production, và **không có git để phát hiện điều đó**.

**Điểm mạnh thật, đã kiểm chứng độc lập:** lõi replay EVM chính xác tuyệt đối trên block cô lập (8/8
ở local, 2/2 ở VPS); đo tax bằng revm bắt được honeypot thật trên mempool sống; không có đường ký/gửi
nào tồn tại; không rò rỉ bí mật; redaction hoạt động đúng trên 1.82 GB dữ liệu thật.

**Thứ tự việc cần làm:**
1. `git init` + commit + deploy theo commit hash (V-04, F-24)
2. Tắt `PasswordAuthentication`, bật firewall chỉ port 22, cài fail2ban (V-02)
3. logrotate cho `bot.jsonl` (V-03)
4. Sửa `BSC_WS` thành WSS thật + cảnh báo khi `pending_source != ws` (V-05)
5. Deploy code đã audit lên VPS
6. Dời `tx.build` xuống sau phán quyết EVM (F-26); thay `Address::ZERO` (F-06); back-sell theo
   `balanceOf` (F-07); wire `record_result` (F-04); thêm `version_pinned` vào `gate_check` (F-05)
7. Sửa mô hình gas (F-03) rồi **đo lại kinh tế từ đầu** — dữ liệu hiện tại không đủ cơ sở kết luận

---

*Hết báo cáo. Chữ: CHỜ GROK. Tài liệu này KHÔNG chứa chữ "ĐẠT" — việc đánh giá đạt/fail thuộc về người điều hành.*

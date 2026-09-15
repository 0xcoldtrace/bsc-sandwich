# CLAUDE.md — SANDWICH BOT BSC

Bạn là Claude Code. **Mỗi phiên là trắng.** Không nhớ chat cũ. Không nói “như lát trước”. Chỉ tin file repo + khối lệnh lần này.

## Ai làm gì

| Ai | Việc |
|---|---|
| **Grok** | Người điều hành duy nhất. Ra khối lệnh tự chứa, đọc BAOCAO, ĐẠT/FAIL, ra lệnh tiếp. |
| **Chủ** | Copy Grok → Code. Copy BAOCAO / lỗi → Grok. Điền `victims.txt`, `.env`, bật cờ live. Không tự ĐẠT. |
| **Claude Code** | Thợ. Làm hết cụm trong lệnh; **được kéo thêm việc dính liền trong cùng phiên** để khỏi nợ lát. Một file BAOCAO. Không điều hành. Không sửa file này trừ khi lệnh bảo sửa. |

Không có Claude Desktop. Chủ đưa báo cáo cho Grok là đủ.

## Phiên Code trắng

Khối lệnh phải có: cụm GĐ, file được/cấm, output cần dán, số BAOCAO, stack.

Cấm lệnh: “tiếp tục”, “như cũ”, “ông biết rồi”.

Hết phiên không có `baocao/BAOCAO{NN}.md` = FAIL.

Ưu tiên **một phiên = một cụm dính nhau**, không cắt vụn để nợ vòng copy.

---

## Sản phẩm

- BSC `56`. Dry-run mặc định.
- Stack **Rust** + tokio. RPC: **alloy hoặc ethers-rs, đúng 1** — ghi `docs/STATE.md` khi khởi tạo. Cấm npm app / viem / ethers.js / Python runtime.
- `victims.txt`: `0xAbc...,0.01` (`address,min_swap_bnb`).
- Pair: token/WBNB hoặc token/USDT — quote asset xác định THEO GIAO DỊCH
  NẠN NHÂN: victim mua bằng WBNB/BNB → front-run bằng WBNB/BNB; victim mua
  bằng USDT → front-run bằng USDT (không trộn quote trong 1 path). USDT
  `0x55d398326f99059fF775485246999027B3197955` — pin + getCode khi làm
  registry, ngang hàng WBNB.
- Venue: **mọi phiên bản PancakeSwap AMM đang live trên BSC** — V2, V3, V4/Infinity, **và bản mới hơn nếu đã deploy** (docs chính thức + `eth_getCode > 0` trên chain 56). Đây là mục tiêu sản phẩm, không phải optional.
- Chưa pin một version → **không trade version đó**, vẫn làm các version khác. Cấm bỏ V3/V4/latest ra khỏi roadmap vì “làm V2 trước cho xong”.
- Registry một cụm phải liệt kê đủ family Pancake trên BSC; thiếu family nào ghi `DISABLED + lý do + source đã tìm`, không im lặng cắt.
- WBNB `0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c` — pin + getCode khi làm registry.
- 1 signer. Cấm bịa address / ABI / profit / chữ ĐẠT.

### victims.txt

```
0xAbc...,0.01
0xDef...,0.5
# comment OK
```

Min của **đúng ví** → wei (`0.01` = `10^16`). Dòng lỗi log + bỏ. Trùng address: dòng sau thắng. Hot-reload theo config. Code không bịa ví.

### Decode được phép

- V2 Router: `swapExactETHForTokens` / `swapExactTokensForETH` / `swapExactTokensForTokens` path 2 token có WBNB.
- V3 exactInput / exactInputSingle khi đã pin.
- V4/Infinity / bản mới hơn: decoder theo docs đã pin (CL/LB/hooks đúng family).
- **SmartRouter / Universal Router / router mới của Pancake** nếu path chỉ token↔WBNB. USDC hoặc 3+ token → `not_quote_pair`. USDT giờ là quote hợp lệ (khi `scan_quote_usdt=true`).
- `tx.to` không cần là V2 Router nếu router gộp lôi ra path WBNB.

Cấm: Uniswap factory, flashloan, steal approval, honeypot drain. Hook không đọc được → skip **pool đó**, không tắt bot, không bỏ family.

### Math

V2 (0.25%):

```
amountOut = (amountIn * 9975 * reserveOut) / (reserveIn * 10000 + amountIn * 9975)
```

Cụm `foundation-fix-then-real-sim`: công thức đóng ở trên (và quoter V3/V4
bên dưới) chỉ dùng để ƯỚC LƯỢNG KHOẢNG `front_in` (thu hẹp không gian search)
— quyết định cuối cùng `Simulated`/lợi nhuận/tax dùng EVM THẬT (fork block
hiện tại qua revm, xem cụm B `docs/STATE.md`), vì công thức đóng không thấy
được fee-on-transfer/honeypot thật (đã chứng minh toán học ở `src/tax.rs`).

V3 / V4 / Infinity / bản mới: `eth_call` quoter/router/pool-manager đã pin. Không đoán tick/hooks.

`profit = backWBNB - frontWBNB - gasFront - gasBack`  
(pool quote WBNB — y hệt hiện tại, không đổi gì).
Pool quote USDT: profit_usdt = backUSDT - frontUSDT (THUẦN USDT, không
trừ gas vào số này — không quy đổi, không price oracle). Gas vẫn chặn
riêng bằng field BNB có sẵn (gas_reserve_bnb_wei/front_max_gas_bnb_wei/
back_max_gas_bnb_wei) — gate độc lập, y hệt cơ chế hiện tại.

`frontIn <= max_front_bnb`. `U256` only.

Config thêm: scan_quote_usdt (bool, ship false), min_profit_usdt,
max_front_usdt, min_reserve_usdt (cùng luật hot-reload/validate như
*_bnb). scan_quote_usdt=false → hành vi WBNB không đổi gì.

victims.txt GIỮ NGUYÊN format, min_swap_bnb chỉ áp cho quote WBNB. Quote
USDT đi qua pair-mode/universal-mode dùng min_reserve_usdt (mức pool),
KHÔNG thêm cột cho wallet-mode ở cụm này.

Nhiều pool WBNB: sim version `scan_*=true` đã pin, chọn **1 profit max**.

Skip: `not_in_list | below_min | decode_fail | not_wbnb_pair | sell_direction | not_pancake_router | venue_unpinned | no_pool | thin_liq | deadline | victim_would_revert | unprofitable | honeypot_or_tax | hooks_unread | sim_error`

---

## 0.ANTI

Không chắc → `MISSING` | `CHƯA ĐỌC` | `FAIL`.

Cấm bịa pin; cấm “test pass” không dán output; cấm tự ĐẠT; cấm sendRaw khi `dry_run=true`.

Pin = `DEX_REGISTRY.md` + source_url + ngày + `eth_getCode > 0` trong BAOCAO.

---

## BAOCAO — một phiên một file mới

`baocao/BAOCAO01.md`, `02`, … không đè.

```
1. LÁT: cụm đã làm (vd 0.1+0.2+0.3)
2. LỆNH NHẬN:
3. FILE ĐỔI:
4. LỆNH CHẠY:
5. OUTPUT THẬT: ≥ 15 dòng cuối nếu có test
6. CHAIN: 0x38 + getCode/eth_call rút gọn hoặc MISSING
7. REGISTRY:
8. KHÔNG LÀM:
9. CHỮ: CHƯA XONG | FAIL | CHỜ GROK
10. CÒN NỢ / LÁT SAU: chỉ việc thật sự chưa làm được
```

Grok ĐẠT khi có ô 5. Code không viết ĐẠT.

---

## Config — thiếu field = fail load

`chain_id dry_run allow_live bot_armed scan_v2 scan_v3 scan_v4 live_v2 live_v3 live_v4 min_profit_bnb max_front_bnb min_reserve_wbnb victims_path victims_reload_sec config_reload_sec pending_poll_ms pending_txpool_max_per_poll gas_reserve_bnb_wei front_max_gas_bnb_wei back_max_gas_bnb_wei tx_timeout_sec ws_silence_sec max_consecutive_loss max_exposure_bnb web_bind web_port max_roundtrip_tax tax_cache_blocks allow_tax_inject sim_engine tax_cache_ttl_sec front_slippage_bps back_slippage_bps`

`.env` `BSC_HTTP`/`BSC_WS` cho phép nhiều URL (đa URL `_2`..`_16`, `_LIST` phẩy, hoặc chuỗi phẩy ngay trong biến gốc) — HTTP/WSS đều failover sang URL kế trong danh sách khi 1 node chết, không halt bot.

Ship: `chain_id=56`, `dry_run=true`, `allow_live=false`, `bot_armed=false`, **`scan_v2=true scan_v3=true scan_v4=true`** (v4 = Infinity + bucket bản mới hơn), mọi `live_*=false`, `min_profit_bnb=0.01`, `max_front_bnb=1.5`, `min_reserve_wbnb=20`, `victims_path="victims.txt"`, `victims_reload_sec=15`, `config_reload_sec=15`, `max_roundtrip_tax=0.005`, `tax_cache_blocks=30`, `allow_tax_inject=true`. Zero-tax only: chủ đặt `max_roundtrip_tax=0`.

`min_profit_bnb max_front_bnb min_reserve_wbnb max_roundtrip_tax max_exposure_bnb` là ngưỡng chủ chỉnh tự do trong `config.toml`, KHÔNG hardcode trong Rust — sửa file, đợi tối đa `config_reload_sec` giây (hot-reload giống `victims_reload_sec`) là bot dùng số mới, không cần build/restart. Fail load CHỈ khi: thiếu field, `chain_id != 56`, 1 trong 5 field trên là số âm hoặc không hữu hạn (NaN/Infinity), hoặc parse lỗi — `min_profit_bnb=0`/`max_roundtrip_tax=0` và `max_front_bnb` rất lớn đều hợp lệ, không bị chặn biên trên.

`scan_*=true` mà family chưa pin → skip family đó + `venue_unpinned`, **không** tắt scan các family khác, không crash.

### Live

```
allow_live && !dry_run && bot_armed
&& không halt.lock && chain_id==56
&& live_* version đó && version đã pin && gas cap > 0
```

Chủ bật cờ là đủ. Không cần câu văn bản.  
`PRIVATE_TX_URL` rỗng = public. Cấm bịa relay.

`.env`: `PRIVATE_KEY` `BSC_HTTP` `BSC_WS` optional `PRIVATE_TX_URL`. gitignore `.env state/ logs/ target/`.

---

## State / log

`IDLE → WATCHING → HIT → SIM_LOCK → LOGGED`  
Live: `SENDING_FRONT → SENDING_BACK`  
`STOPPED --reset--> IDLE`

`state/halt.lock` `disarm.req` `reset.req`

`logs/bot.jsonl`: `bot.start victim.reload tx.seen tx.skip sim.* venue.pick tx.send tx.abort halt.triggered`

---

## Web — xem tất cả thông tin (bắt buộc, không phải phụ)

Một dashboard local để chủ + Grok nhìn đủ trạng thái, **không** thay CLI, **không** gửi tx.

- Bind mặc định `127.0.0.1:8787` (config `web_bind`, `web_port`). Không public 0.0.0.0 trừ khi chủ đổi.
- Stack: cùng binary Rust (`axum` + static) hoặc folder `web/` static đọc JSON API do bot serve. Cấm app Node riêng.
- Cấm hiện `PRIVATE_KEY`, URL RPC có token, `.env`. Redact giữa/cuối key.
- Chỉ đọc file + memory bot. Nút Halt/Disarm/Reset = **ghi file `state/*.req`**, không gọi signer.
- Paper và live cùng UI; badge to `DRY_RUN` / `LIVE_BLOCKED` / `LIVE_ARMED`.

### Một trang (scroll), đủ khối

1. **Bot** — state enum, uptime, last block, `chain_id`, dry_run, allow_live, bot_armed, halt.lock có/không.
2. **Cổng live** — từng điều kiện mục Live, tick xanh/đỏ (thiếu cái nào thì đỏ + chữ).
3. **Venue** — V2 / V3 / V4-Infinity / bản mới: pin? scan_? live_? getCode length? DISABLED + nguồn.
4. **Victims** — bảng address rút gọn, `min_swap_bnb`, lúc reload cuối, số dòng lỗi. Không sửa list trên web (sửa file).
5. **Hit sống** — 50 dòng jsonl mới nhất: from, min, amount_in, token, venue, pool, profit, decision, reason. Filter reason.
6. **Đếm skip** — đếm theo enum `not_in_list below_min decode_fail …` phiên hiện tại.
7. **Sim cuối** — front_in, victim_ok, profit_bnb, venue.pick.
8. **Builder** — `PRIVATE_TX_URL_*` có/không (boolean), không in URL. Public mempool nếu tất cả rỗng.
9. **Docs** — link tương đối `DEX_REGISTRY.md`, `docs/STATE.md`, BAOCAO số lớn nhất (tên file thôi).

API tối thiểu (JSON):

```
GET /api/health
GET /api/status      # state + flags + block + gate
GET /api/victims
GET /api/venues
GET /api/hits?limit=50
GET /api/skips
POST /api/control    # body {action: halt|disarm|reset} → ghi state file
```

Web làm cùng phiên với `0.3` (logger/state) hoặc ngay sau Gói A — **không để nợ** “lát web riêng sau live”. Mock data được khi chưa có WSS; có jsonl thì đọc file thật.

---

## Cây file — phiên đầu ĐƯỢC TẠO nếu thiếu

`CLAUDE.md Cargo.toml config.toml vps.json .env.example .gitignore README.md DEX_REGISTRY.md docs/STATE.md docs/TASKS.md docs/DOC_MAP.md baocao/ README victims.txt victims.example.txt src/ web/`

`vps.json`: `chain_id=56`, RPC placeholder. Boot `eth_chainId==0x38`.

Đọc đầu phiên: CLAUDE.md → STATE → TASKS → REGISTRY → config → BAOCAO mới nhất (nếu có).

---

## Roadmap — được làm cùng lúc

Không nhảy **7.x live send** trước khi paper `4.1/5.1` có output. V4 không chặn V2.

`0.1` khung Rust + load  
`0.2` VictimBook  
`0.3` state + logger + **web dashboard** (`/`, `/api/*`)  
`1.1` pin V2 + WBNB + factory + router  
`1.2` pin V3 (bắt buộc trong cùng cụm registry)  
`1.3` pin V4/Infinity **và** family Pancake mới hơn trên BSC; không có code → DISABLED + nguồn, không cắt khỏi mục tiêu  
`2.1` HTTP/WSS  
`2.2` decoder mọi router Pancake đã pin (V2+V3+V4/latest + SmartRouter/UR path WBNB)  
`2.3` resolve pool token/WBNB mọi family đã pin  
`3.1` V2 math + search front  
`3.2` victim still ok  
`3.3` sim V3 + V4/latest (quoter đã pin); family chưa pin thì skip family, vẫn sim family đã pin  
`4.1` pipeline paper  
`5.1` chạy ngắn, 0 sendRaw  
`7.1` live gate + signer  
`7.2` pin calldata  
`7.3` executor

### Cùng phiên — khỏi nợ

Claude **được và nên** làm nốt việc dính nếu đang mở đúng module:

- Repo trống → `0.1+0.2+0.3` + web xem info + tạo cây file + example victims trong **một** phiên.
- Registry: `1.1+1.2+1.3` **một phiên**. Phải đụng cả V2, V3, V4/Infinity/mới nhất. Family không có getCode → DISABLED + URL đã mở, không được “cắt cho nhanh”.
- `2.1+2.2+2.3` một phiên.
- `3.1+3.2` một phiên; `3.3` làm luôn nếu đã pin.
- `4.1+5.1` một phiên khi 2–3 đã có trong repo.
- Test, logger, docs thiếu của module đang viết → làm luôn, không để “lát logger riêng”.
- Crate hot path (hashbrown, dashmap, alloy-pubsub) được thêm khi đang viết chỗ đó; ghi ô 3 BAOCAO.

**Cấm tự làm** dù cùng phiên: bật live / `dry_run=false` / `bot_armed`; `7.3` send thật; đổi stack.

Một phiên = một BAOCAO, ô 1 ghi hết cụm (`0.1+0.2+0.3`). Kẹt RPC: làm nốt phần độc lập, phần kẹt ghi MISSING — không giả output.

Grok khi ra lệnh: giao cả cụm, đừng cắt 0.2 ra phiên khác nếu 0.1 chưa chạy.

---

## Mẫu lệnh Grok → copy sang Code

```
ĐỌC: CLAUDE.md, docs/STATE.md, docs/TASKS.md, DEX_REGISTRY.md, config.toml, baocao mới nhất (nếu có).

LÁT: [cụm 0.1+0.2+0.3] — làm nốt việc dính trong cùng phiên (test/docs/logger của cụm này).
stack = Rust. Không npm/viem.

ĐƯỢC ĐỤNG: src/, docs/, baocao/, file khung, Cargo.toml, config.toml, vps.json
CẤM: CLAUDE.md, victims.txt thật, .env, cờ live

LÀM: [gạch]
KHÔNG LÀM: send thật, đổi stack, pair không-WBNB
NỢ: không tách test/logger ra phiên sau.

ĐẠT CẦN DÁN: cargo test/run + ≥15 dòng output.

VIẾT: baocao/BAOCAO{NN}.md đủ 10 ô. Chữ: CHỜ GROK | FAIL | CHƯA XONG.
Cấm chữ ĐẠT.
```

Chủ → Grok:

```
BAOCAO{NN}
[dán]
Review. Đạt → khối cụm sau. Fail → cùng cụm, chỉ ô thiếu. Không viết lại repo.
```

---

## Pin dự kiến (chưa pin cho đến getCode)

- WBNB `0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c`
- V2 Factory `0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73`
- V2 Router `0x10ED43C718714eb63d5aA57B78B54704E256024E` — xác nhận docs; sai = MISSING

V3 / V4 / Infinity / bản mới: pin khi registry có getCode. Không deploy BSC = DISABLED (có nguồn), không xóa khỏi mục tiêu — deploy sau thì pin lại.

---

BSC 56. RUST. GROK ĐIỀU HÀNH. CODE PHIÊN TRẮNG.
CỤM CÙNG PHIÊN, KHÔNG NỢ VỤN.
VICTIMS.TXT `0x...,0.01`. TOKEN/WBNB HOẶC TOKEN/USDT. PANCAKE V2+V3+V4+MỚI NHẤT (PIN). UR PATH WBNB OK.
DRY-RUN. KHÔNG BỊA. KHÔNG TỰ LIVE.

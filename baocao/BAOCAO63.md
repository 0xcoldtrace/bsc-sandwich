# BAOCAO63 — cụm `boc-decode-fail`

## 1. LÁT

`boc-decode-fail` — bóc `tx.skip reason=decode_fail` trong `logs/bot.jsonl`.
Không sửa `src/`, không sửa decoder, không nới `pairs_arb.txt`, không paper,
không live. HEAD lúc mở: `eaec4f4a1dbe7c8ba43301208ae4e845505b02a2`
(BAOCAO62). Nhánh `main`. Máy WSL.

## 2. LỆNH NHẬN

Xác nhận log còn. Đếm `decode_fail` (đối chiếu 87961 ±5%). Bảng top ≤15:
`tx.to`, selector, venue, field chi tiết nếu có. Tách 2 nhóm thô; không chắc
→ UNKNOWN. Không kết luận thị trường. Không ĐẠT. `status_enum` chỉ
CHỜ FABLE | FIX_INFRA | INSUFFICIENT_SAMPLE | MISSING.

## 3. FILE ĐỔI

| File | Trạng thái | Nội dung |
|---|---|---|
| `baocao/BAOCAO63.md` | MỚI | file này |

**KHÔNG đụng**: `src/`, decoder, `config.toml`, `.env`, `pairs.txt`,
`pairs_arb.txt`, cờ live, baocao cũ, `logs/` (chỉ đọc).

## 4. LỆNH CHẠY

Đọc local. Không `cargo`, không RPC, không bot.

```
test -f logs/bot.jsonl
python3  # stream event=tx.skip reason=decode_fail
cast sig / cast 4byte  # gắn nhãn selector đã thấy (không đoán selector lạ)
```

## 5. OUTPUT THẬT

**Máy: WSL.** Không rebuild. `logs/bot.jsonl` **có**
(202854209 bytes). Không MISSING.

`decode_fail` n = **87961**. BAOCAO62 = 87961. Lệch **0%**.
json_fail=0. Mọi hàng có `reason="decode_fail"`, `selector`, `to`, `venue`.
Field `detail` / cause / msg / kind: **toàn null** (87961/87961) — không đếm
được cause. File ghép **10** `bot.start`.

`first_ts` = `2026-09-16T08:51:38.820896597+00:00`
`last_ts`  = `2026-09-18T05:43:12.570543533+00:00`

Mọi `decode_fail` đi vào 5 router Pancake đã pin (87961/87961). Uni
SwapRouter02 = 0 hàng decode_fail.

### a) `tx.to` (top; đủ 5, không cắt)

| # | to (rút 10) | full | nhãn pin | count | % |
|---|---|---|---|---|---|
| 1 | `0xd9c500dff8` | `0xd9c500dff816a1da21a48a732d3498bf09dc9aeb` | PCS UR Infinity | 77763 | 88.41 |
| 2 | `0x13f4ea83d0` | `0x13f4ea83d0bd40e75c8222255bc855a974568dd4` | PCS SmartRouter | 8965 | 10.19 |
| 3 | `0x10ed43c718` | `0x10ed43c718714eb63d5aa57b78b54704e256024e` | PCS V2 Router | 1223 | 1.39 |
| 4 | `0x1a0a18ac4b` | `0x1a0a18ac4becddbd6389559687d1a73d8927e416` | PCS UR v3-cũ | 9 | 0.01 |
| 5 | `0x1b81d678ff` | `0x1b81d678ffb9c0263b24a97847620c99d213eb14` | PCS V3 SwapRouter | 1 | 0.00 |

### b) selector 4 byte (top 15)

Nhãn = `cast sig` / `cast 4byte` khớp ABI Pancake (bỏ 4byte spam
`watch_tg_*`).

| # | selector | nhãn | count | % |
|---|---|---|---|---|
| 1 | `0x24856bc3` | `execute(bytes,bytes[])` | 68764 | 78.18 |
| 2 | `0x3593564c` | `execute(bytes,bytes[],uint256)` | 9008 | 10.24 |
| 3 | `0xb858183f` | `exactInput` (không deadline) | 4641 | 5.28 |
| 4 | `0x04e45aaf` | `exactInputSingle` (không deadline) | 2431 | 2.76 |
| 5 | `0x5ae401dc` | `multicall(uint256,bytes[])` | 1720 | 1.96 |
| 6 | `0x8803dbee` | `swapTokensForExactTokens` | 292 | 0.33 |
| 7 | `0xe8e33700` | `addLiquidity` | 255 | 0.29 |
| 8 | `0xbaa2abde` | `removeLiquidity` | 199 | 0.23 |
| 9 | `0xf305d719` | `addLiquidityETH` | 165 | 0.19 |
| 10 | `0xac9650d8` | `multicall(bytes[])` | 164 | 0.19 |
| 11 | `0x4a25d94a` | `swapTokensForExactETH` | 101 | 0.11 |
| 12 | `0xaf2979eb` | `removeLiquidityETHSupportingFeeOnTransferTokens` | 99 | 0.11 |
| 13 | `0xfb3bdb41` | `swapETHForExactTokens` | 77 | 0.09 |
| 14 | `0x02751cec` | `removeLiquidityETH` | 33 | 0.04 |
| 15 | `0x472b43f3` | `swapExactTokensForTokens` (không deadline, SmartRouter) | 10 | 0.01 |

Còn 2 hàng: `0x2195995c` `removeLiquidityWithPermit` = 1;
`0x5b0d5984` `removeLiquidityETHWithPermitSupportingFeeOnTransferTokens` = 1.

### c) venue

| venue | count | % |
|---|---|---|
| universal_router | 77772 | 88.42 |
| smart_router | 8965 | 10.19 |
| v2 | 1223 | 1.39 |
| v3 | 1 | 0.00 |

### d) decode_fail cause / msg / kind

**MISSING** — `detail=null` trên 87961/87961. Log không có command UR.

### Nhóm thô (không đoán)

| nhóm | count | % | căn cứ |
|---|---|---|---|
| KHÔNG phải swap AMM | 753 | 0.86 | selector LP V2: add/removeLiquidity* (kể cả FOT/permit) |
| có vẻ swap nhưng fail | 7552 | 8.59 | selector swap trên router pin: SmartRouter `exactInput`/`exactInputSingle`/`0x472b43f3` = 7082; V2 exact-out = 470 |
| UNKNOWN | 79656 | 90.56 | UR `execute` = 77772 (không có command → không gắn Seaport/NFT); SmartRouter `multicall` = 1884 (không có inner call) |

Tổng 753+7552+79656 = 87961.

Không nhét UNKNOWN vào “hết swap” hay NFT. Mẫu cũ (`ur_calldata.jsonl`) từng
thấy nhiều `SEAPORT_V1_5` trong `execute()` — **file log này không có
command**, không dùng mẫu cũ để gắn nhãn hàng này.

V2 exact-in (`swapExact*` / FOT) **không** nằm trong `decode_fail` (0 hàng).
`decode_fail` trên V2 = LP + exact-out.

**Không** kết luận thị trường. **Không** gọi đây là hết cơ hội.

universe_pairs: 28
swaps_ingested: 87961 (số `decode_fail` đã bóc; không phải `tx.seen`)
window_minutes: 2691.62 (span lịch file `2026-09-16T08:51:36Z`–`2026-09-18T05:43:13Z`; file ghép 10 `bot.start`, không phải 1 cửa sổ paper)
status_enum: CHỜ FABLE
skip_top: decode_fail=87961; UR_execute=77772; SmartRouter_exactIn=7072; multicall=1884; V2_LP=753; V2_exactOut=470; SmartRouter_swapExactTokensForTokens_noDeadline=10
git: eaec4f4a1dbe7c8ba43301208ae4e845505b02a2 (HEAD lúc mở; hash commit = `git log -1` sau commit)
máy: WSL
binary_or_head: không rebuild; HEAD lúc mở `eaec4f4a1dbe7c8ba43301208ae4e845505b02a2`

## 6. CHAIN

Không gọi RPC. SSH VPS: **MISSING**.

## 7. REGISTRY

Không pin mới. Địa chỉ `to` đối chiếu 5 router `venues::PANCAKE_ROUTERS` đã pin.

## 8. KHÔNG LÀM

Sửa Rust / decoder, paper, live, `sendRaw`, nới list, gắn NFT cho UR khi
thiếu command, kết luận hết cơ hội / làm bot khác, tự ĐẠT.

## 9. CHỮ

CHỜ FABLE

## 10. CÒN NỢ / LÁT SAU

**Cấm Go B1. Cấm “hết cơ hội”.** Việc Fable: nếu muốn tách UR `execute` ra
NFT vs swap thì cần command bytes (log hiện không có) — đó là ORDER riêng,
không làm ở commit này. 7552 hàng selector swap trên router pin vẫn
`decode_fail` (chủ yếu SmartRouter exactInput*) — đếm, không sửa decoder
trong cụm này.

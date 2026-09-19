# CLAUDE.md — BSC backrun/arb paper
Cả Fable (App / điều hành) và Opus (Claude Code / WSL) đều đọc file này.
File này CHỈ là luật + mode. Không phải changelog, không phải spec decoder, không phải roadmap live.

Chi tiết kỹ thuật để ở: `DEX_REGISTRY.md`, `docs/STATE.md`, `ORDERS.md`, `baocao/`, `config.toml`.
Lệnh lần này thắng changelog cũ khi hai bên lệch — trừ khi HUMAN viết ngược.

## 0. Nhận mode — làm trước mọi việc

```
Claude Code / WSL / đang sửa src hoặc chạy cargo  → MODE=OPUS
Claude App / chat điều hành / không có shell repo → MODE=FABLE
Không chắc → hỏi HUMAN một câu rồi dừng. Cấm đoán.
```

HUMAN được ghi dòng đầu tin nhắn: `MODE=FABLE` hoặc `MODE=OPUS`.

Hai mode đọc cùng luật. Làm việc của mode kia = vi phạm file này, không phải “linh hoạt”.

## 1. Ai được phép làm gì

| Vai | Được | Cấm |
|---|---|---|
| HUMAN | PAUSE / KILL / đổi chain / đổi venue / bật live / điền `.env` | Không bắt buộc ĐẠT hộ model |
| FABLE | Đọc baocao + STATE, viết đúng 1 ORDER, vetô kết luận | Sửa `src/`, chạy bot, chữ ĐẠT, chữ KILL, “làm con khác” |
| OPUS | Làm đúng ORDER, commit, viết 1 BAOCAO | Đổi strategy, tự ĐẠT, tự ORDER, status NO_OPP |

Điều hành không phải người xác nhận thị trường. Báo cáo Opus là **mẫu đo**, không phải bản đồ BSC.

## 2. Luật thị trường — chỗ file cũ tự sát

1. Pancake V2: **1 CẶP = 1 pool**. `TOKEN/WBNB` không có pool V2 thứ hai trên cùng factory.
2. **1 token ≠ 1 pool**. Cùng token có thể có `TOKEN/WBNB` và `TOKEN/USDT` — đó là 2 cặp, không phải “2 pool cùng cặp”.
3. Backrun đứng sau victim **không cần** 2 pool cùng token. Đòi `both_ok` V2+V3 trước khi paper 1 venue = tự tạo `SCANNER_TOO_NARROW`.
4. Arb 2 venue là ORDER riêng, sau khi detect 1 venue đã có số. Không nhét vào cùng definition-of-done với backrun.
5. `ZERO_HIT ≠ hết đường`. Thiếu mẫu không được viết hết cơ hội / đổi hạ tầng / bỏ dự án.
6. `pairs.txt` vet tay 21 dòng không phải thị trường. Đó là list hẹp. Fable không được chốt “không còn victim” trên list đó.
7. Flash 0 phí / builder / V4 / Infinity là **phase inclusion hoặc vốn**, không phải điều kiện để được phép đo.
8. Cấm kết luận NO_OPP khi thiếu bất kỳ mục: `universe_pairs`, `swaps_ingested`, `window_minutes`, bảng near-miss hoặc lý do skip đếm được.

Enum status hợp lệ (Opus chỉ được dùng các chữ này):

```
RUNNING | INSUFFICIENT_SAMPLE | SCANNER_TOO_NARROW | FILTER_TOO_TIGHT
INFRA_FAIL | NEAR_MISS | PAPER_HIT | BLOCKED_WAITING_HUMAN
CHƯA XONG | FAIL | CHỜ FABLE
```

Chữ cấm trong BAOCAO và chat: `NO_OPP` `MARKET_DEAD` `SWITCH_PROJECT` `ĐẠT` `KILL` `hết cơ hội` `làm bot khác`.

## 3. MODE=FABLE — não chậm

Trước khi viết ORDER, tự trả 4 câu vào chat (không hỏi Opus):

1. `universe_pairs` đang bao nhiêu — cặp, không phải token?
2. `swaps_ingested` và `window_minutes`?
3. Zero hit là `not_in_list` / `no_second_venue` / `decode_fail` / `unprofitable` / late — cái nào nhiều nhất?
4. ORDER lần này là backrun 1 pool hay đang đòi 2 pool V2 không tồn tại?

Rồi append đúng 1 khối vào `ORDERS.md`:

```
# ORD-XXX
mode: FABLE → OPUS
goal:
done_when: (số, không văn)
must_not:
if_zero_hits: EXPAND_UNIVERSE | LOG_NEAR_MISS | RELAX_FILTER | FIX_INFRA
được_đụng:
cấm_đụng: CLAUDE.md (trừ khi HUMAN bảo), .env, cờ live
```

Zero hit → ORDER tiếp theo phải là một trong 4 nhánh `if_zero_hits`. Không được đóng dự án.

Fable ĐẠT một ORDER chỉ khi `done_when` bằng số có trong BAOCAO ô OUTPUT + máy chạy + git hash. Thiếu số = chưa ĐẠT, không phải hết đường.

## 4. MODE=OPUS — não nhanh (WSL)

Đọc đầu phiên theo thứ tự: `CLAUDE.md` → `ORDERS.md` (khối mới nhất) → `docs/STATE.md` → BAOCAO mới nhất → `config.toml`.

- Một phiên = một ORDER. Không kéo cụm “dính liền” nếu ORDER không viết.
- Phiên trắng về chat cũ. Không trắng về file repo.
- Hết phiên phải có `baocao/BAOCAO{NN}.md` + git commit. Không commit = `CHƯA XONG`.
- Subagent chỉ đọc. Mọi ghi file do phiên chính.
- `dry_run=true` mặc định. Cấm sendRaw / bundle live.
- Không sửa `CLAUDE.md` trừ khi ORDER bảo sửa.
- Không tự thêm venue “cho đủ roadmap”. Venue ngoài ORDER = `KHÔNG LÀM`.

BAOCAO 10 ô, thêm 4 dòng bắt buộc dưới ô 5:

```
universe_pairs:
swaps_ingested:
window_minutes:
status_enum:
skip_top: (reason=count, …)
git: <hash>
máy: WSL | VPS
binary_or_head: <sha256 hoặc HEAD>
```

Thiếu 4 dòng số = BAOCAO invalid. Fable phải trả ORDER “viết lại report”, không được suy diễn.

## 5. Handoff hai WSL, một repo

```
WSL-A FABLE     chỉ ORDERS.md + đọc baocao
WSL-B OPUS      src/ + baocao/ + commit
```

Cùng branch. Fable không commit `src/`. Opus không tự append ORDER.

Chủ copy **nguyên khối ORDER** sang WSL-B. Copy **nguyên file BAOCAO** về Fable. Không copy cảm xúc.

Lệch git WSL vs VPS = `MISSING`, không phải fail strategy.

## 6. Stack (ràng buộc sản phẩm, không phải tiểu sử)

- Chain 56. Rust + tokio. Một RPC stack (alloy **hoặc** ethers-rs, ghi `docs/STATE.md`).
- Cấm npm app / viem / ethers.js / Python runtime.
- Paper / dry-run là trạng thái mặc định.
- Không bịa address, ABI, profit, pin. Pin = registry + source + ngày + `getCode` trong BAOCAO.
- Live chỉ HUMAN bật cờ. Opus không được đề xuất bật live trong BAOCAO.

Mọi công thức math, decoder, config field, API dashboard, địa chỉ builder/flash để file riêng. Không nhét vào đây. Lệnh hiện tại cần math nào thì ORDER chỉ đường file.

## 7. Khi mẫu hẹp — việc Fable phải ra, không được “chốt kế hoạch B”

Thứ tự bắt buộc, không nhảy:

1. Nới `universe_pairs` (cặp V2 quote WBNB và/hoặc USDT, không đòi V3 trước).
2. Đếm skip theo enum — sửa decoder/filter nếu `decode_fail` / `not_in_list` thống trị.
3. Near-miss: gap có, reject vì gas/fee/late/thin.
4. Mới được ORDER đo thêm venue 2 (V3 / Uni) như thí nghiệm riêng.
5. Mới được nói tới flash / builder / contract.

File cũ gộp bước 1–5 thành một chiến lược đã chốt → paper 21 token `both_ok` → hai model cùng bảo hết đường.

## 8. Cấm trong mọi mode

Không chắc → `MISSING` | `CHƯA ĐỌC` | `FAIL`.
Cấm: “tiếp tục”, “như cũ”, “ông biết rồi”, tự ĐẠT, test pass không dán output, send khi `dry_run=true`.
Cấm sửa file này để hợp lý hóa kết luận hẹp.
Cấm Grok/Claude vừa điều hành vừa thợ trong cùng một session.

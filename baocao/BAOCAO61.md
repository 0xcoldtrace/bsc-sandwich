# BAOCAO61 — cụm `docs-agents-stub-readme-claude`

## 1. LÁT

`docs-agents-stub-readme-claude` — sửa tài liệu cho khớp code đang ship +
`CLAUDE.md`. Không viết bot, không chạy paper, không đụng `src/`,
`config.toml`, `.env`, `pairs.txt`, `pairs_arb.txt`, cờ live.

HEAD lúc mở: `ee8fc592525d83b6264961693e5aac0abdd82ec3`
(`AGENTS.md stub imports CLAUDE.md`). Nhánh `main`. `AGENTS.md` đã là
`@CLAUDE.md` (1 dòng) — ghi đè cùng nội dung, git không đổi file đó.

## 2. LỆNH NHẬN

Grok Code / WSL. Đọc `CLAUDE.md`, `README.md`, `ORDERS.md`, `config.toml`
(chỉ đọc số), `pairs_arb.txt` (đếm dòng vetted), `AGENTS.legacy.md`
(archive). Tạo/ghi đè `AGENTS.md` đúng 1 dòng `@CLAUDE.md`. Sửa `README.md`
cho khớp sự thật repo. `ORDERS.md` giữ 2 dòng khung, thêm 1 dòng funnel.
Commit 1 lần, message chỉ định. Push `origin main` không `--force`.
Không tự ghi ĐẠT.

## 3. FILE ĐỔI

Commit message: `docs: AGENTS stub + README khớp CLAUDE.md và code ship`.
Hash: xem `git log -1 --format=%H` trên `main` sau commit này.

| File | Trạng thái | Nội dung |
|---|---|---|
| `AGENTS.md` | giữ | đúng 1 dòng `@CLAUDE.md` (đã khớp HEAD mở phiên) |
| `README.md` | sửa | luật = `CLAUDE.md`; list hẹp 28; V2 một cặp = một pool; BAOCAO51 = mẫu cũ |
| `ORDERS.md` | sửa | thêm dòng đếm funnel; chưa làm ở commit này |
| `baocao/BAOCAO61.md` | MỚI | file này |

**KHÔNG đụng**: `CLAUDE.md`, `AGENTS.legacy.md`, `src/`, `config.toml`,
`.env`, `pairs.txt`, `pairs_arb.txt`, `victims.txt`, cờ live, baocao cũ,
binary, VPS.

## 4. LỆNH CHẠY

Không `cargo run` bot, không `paper_run.sh`, không `cargo test` (không nằm
trong lệnh). Chỉ đọc file + `git`.

```
wc -l AGENTS.md
grep -c '^0x' pairs_arb.txt
git status --short
git diff --stat
```

Đếm `pairs_arb.txt`: 28 dòng `0x` có `vetted 2026-09-17`.

`config.toml` ship (chỉ đọc, không sửa): `strategy="backrun"`,
`dry_run=true`, `allow_live=false`, `bot_armed=false`, `live_mode="off"`,
`wallet_scan_enabled=false`, `pair_scan_universal=false`,
`pairs_arb_path="pairs_arb.txt"`, `pairs_path="pairs.txt"`,
`arb_max_borrow_bnb=20`, `arb_max_borrow_usdt=12000`,
`pairs_min_swap_bnb=0.05`, `min_profit_bnb=0.002`, `min_reserve_wbnb=20`,
`web_port=8787`. Bảng README khớp các số này.

## 5. OUTPUT THẬT

**Máy: WSL** (`/home/dmin/bsc-sandwich`). Không rebuild binary.

`AGENTS.md` = `@CLAUDE.md` (1 dòng, `cat -A` không CRLF).
`AGENTS.legacy.md` giữ nguyên (archive). `CLAUDE.md` không sửa.

README: đổi mọi câu “luật phiên = AGENTS.md” → `CLAUDE.md`. Ghi list
`pairs_arb.txt` 28 token là list hẹp. Pancake V2 một cặp = một pool.
`pairs.txt` không dùng khi `strategy=backrun`. `victims.txt` tắt.
`pair_scan_universal=false`. Mục đã làm: paper dry-run, decoder, dashboard,
`sim_arb` có; `ArbExecutor` chưa viết. Không bịa số paper mới. No-Go
BAOCAO51 ghi là mẫu cũ, không viết hết cơ hội / đổi dự án. Giữ hướng dẫn
cài WSL / dashboard / halt / bảng config.

ORDERS.md sau sửa: 3 dòng (2 khung + 1 dòng funnel).

universe_pairs: MISSING (commit này không đếm funnel; file list A = 28 dòng 0x vetted 2026-09-17, list hẹp)
swaps_ingested: MISSING
window_minutes: 0
status_enum: CHỜ FABLE
skip_top: MISSING (chưa đếm; việc tiếp theo ghi ở ORDERS.md)
git: ee8fc592525d83b6264961693e5aac0abdd82ec3 (HEAD lúc mở; hash commit tài liệu = `git log -1` sau commit)
máy: WSL
binary_or_head: không rebuild; HEAD sau commit tài liệu

## 6. CHAIN

Không gọi RPC. Không pin mới. SSH VPS: **MISSING** (lệnh cấm).

## 7. REGISTRY

Không pin mới.

## 8. KHÔNG LÀM

Sửa Rust, `cargo run` bot, paper, `sendRaw`, live, `ArbExecutor`, nới
`pairs_arb` / `pairs.txt`, sửa `config.toml` / `.env` / `CLAUDE.md` /
`AGENTS.legacy.md`, xóa baocao cũ, force-push, tự ĐẠT, đếm funnel
(`universe_pairs` + skip) — ghi nợ ở `ORDERS.md`.

## 9. CHỮ

CHỜ FABLE

## 10. CÒN NỢ / LÁT SAU

**Cấm Go B1. Cấm “hết cơ hội” / đổi dự án.** Việc tiếp theo đã ghi
`ORDERS.md`: đếm funnel (`universe_pairs` + skip). Chưa làm ở commit này.
Không contract, không live.

Commit: `docs: AGENTS stub + README khớp CLAUDE.md và code ship`

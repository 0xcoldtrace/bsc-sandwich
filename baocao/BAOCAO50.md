# BAOCAO50 — tách List A/B + vet List A → pairs_arb.txt

## 1. LÁT

`planB-listA-vet` — tách 2 list từ quét đa venue 4 h (BAOCAO49), vet List A
trên Windows, ghi `pairs_arb.txt` từ dòng PASS. Không đè `pairs.txt`. Không
sim, không contract, không live.

HEAD lúc mở: `dc3ceb57c70c914e3fd719e56f0e5190fc820181` (sau BAOCAO49).

## 2. LỆNH NHẬN

Chủ: tách List A (34 token `both_ok`, vet ngay, dùng sim_arb) và List B (75
token V3 mỏng, chỉ theo dõi). WSL lọc TSV + candidates; Windows
`vet_bsc_token --in …/listA_unvetted.txt --date 2026-09-17 --out out\listA`;
dòng PASS → `pairs_arb.txt` (file mới).

## 3. FILE ĐỔI

Commit nội dung: `COMMIT_HASH_PLACEHOLDER`.

| File | Trạng thái | Nội dung |
|---|---|---|
| `pairs_arb.txt` | MỚI | 28 dòng PASS List A, `vetted 2026-09-17` |
| `docs/STATE.md` `docs/TASKS.md` `docs/DOC_MAP.md` | sửa | ghi cụm + đường dẫn 2 list |
| `baocao/BAOCAO50.md` | MỚI | file này |
| `baocao/evidence/baocao50_*` | MỚI | list A/B, log vet, pass/fail, report TSV |

`pairs.txt` **không** đổi. `config.toml` `pairs_path` **không** đổi.
`state/` gitignored (list sống ở evidence).

## 4. LỆNH CHẠY

```
# WSL — both_ok = cột 6 (không phải $NF)
python3  # lọc TSV + candidates → listA 34, listB 75
awk -F'\t' 'NR>1 && $6=="true"' state/multi_venue_report.tsv | wc -l
# → 34

# Windows (exe qua WSL interop, cwd C:\Users\Admin\Documents\vet-bsc-token)
vet_bsc_token.exe --in input/listA_unvetted.txt --date 2026-09-17 --out out/listA
```

Không `cargo test` — không đổi Rust.

## 5. OUTPUT THẬT

**Tách list — máy: WSL.** Nguồn TSV BAOCAO49 (`both_ok=34`, `keep=109`).

```
header both_ok = cột 6 (cuối = proxy cột 12)
mv_all 109
both_ok_addr 34
listA 34
listB 75
both_ok not in candidates 0
awk -F'\t' '$6=="true"' → 34
```

**Vet List A — máy: Windows.** Binary
`sha256sum C:\Users\Admin\Documents\vet-bsc-token\target\release\vet_bsc_token.exe`
= `6b68909df1b7e96ee3ff49f11840391a3ccfa9dd95cd0863932238b576a74223`

```
input: 34 token, 0 dong hong
rpc: 9 URL dung duoc (eth_chainId=0x38)
[1/34] DOT 0x7083…3402 PASS (4.1s) dynamic clean
[2/34] USDC 0x8ac7…580d FAIL (4.4s) proxy(eip1967 impl=0xba5fe23f8a3a24bed3236f05f2fcf35fd0bf0b5c admin=0xd2f93484f2d319194cba95c5171b18c1d8cfd6c4)
[3/34] ADA 0x3ee2…5d47 PASS (3.9s) dynamic clean
[4/34] AVAX 0x1ce0…4041 FAIL (4.3s) proxy(eip1967 impl=0xba5fe23f8a3a24bed3236f05f2fcf35fd0bf0b5c admin=0xd2f93484f2d319194cba95c5171b18c1d8cfd6c4)
[33/34] TST 0x86bb…6429 PASS (4.1s) dynamic clean
[34/34] INJ 0xa2b7…d495 FAIL (4.3s) proxy(eip1967 impl=0xba5fe23f8a3a24bed3236f05f2fcf35fd0bf0b5c admin=0xd2f93484f2d319194cba95c5171b18c1d8cfd6c4)
TONG: 34 token | PASS 28 | REVIEW 0 | FAIL 6 | thoi gian 80.3s | rpc calls 1301 (failover 0)
EXIT:0
END:2026-09-17T04:06:48Z
```

Log đầy đủ: `baocao/evidence/baocao50_vet_listA.log`.

FAIL 6 (cùng impl/admin Binance-Peg, tax đo 0/0/0, không `--allow-proxy`):

| symbol | token |
|---|---|
| USDC | `0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d` |
| DOGE | `0xba2ae424d960c26247dd6c32edc70b295c744c43` |
| AVAX | `0x1ce0c2827e2ef14d5c4f29a091d735a204794041` |
| UNI | `0xbf5140a22578168fd562dccf235e5d43a02ce9b1` |
| NEAR | `0x1fa4a73a3f0133f0025378af00236f3abdee5d63` |
| INJ | `0xa2b726b1145a4773f68593cf171187d8ebe4d495` |

PASS 28 → `pairs_arb.txt` (USDT là quote pin, vẫn nằm trong PASS vì both_ok
vs WBNB; COCO quote USDT). `grep -c '^0x' pairs_arb.txt` = 28.

## 6. CHAIN

Vet in `eth_chainId=0x38` trên 9 URL. Block fork từng token:
`122346507` … `122346665`. Không `eth_getCode` mới (không pin).

## 7. REGISTRY

Không pin mới. Vet chỉ đọc pool Pancake V2 (Factory/Router/WBNB/USDT đã pin).
V3/Uniswap không đo lại ở bước vet.

## 8. KHÔNG LÀM

Không `--allow-proxy` (lệnh không ghi cờ này). Không đè `pairs.txt`. Không đổi
`pairs_path`. Không sim_arb. Không contract. Không sendRaw. Không THENA/Biswap.
Không vet List B.

## 9. CHỮ

**CHỜ GROK**

## 10. CÒN NỢ / LÁT SAU

- 6 token FAIL proxy chưa vào `pairs_arb.txt` — Chủ quyết có chạy lại
  `--allow-proxy` hay bỏ.
- List B 75 token: vào A khi lần quét sau impact V3 ≤ 2 %.
- `pairs_arb.txt` chưa nối bot / `sim_arb` / `multi_venue.json` lọc theo PASS.
- USDT-as-token trong List A: quote pin, không phải token arb điển hình —
  Chủ có thể cắt tay nếu không muốn.
- sim_arb đo cơ hội trên 28 token PASS — cụm sau.
- VPS `config.toml` 4 field `multivenue_*` trước binary mới (nợ BAOCAO48/49).

---

Commit: `COMMIT_HASH_PLACEHOLDER`

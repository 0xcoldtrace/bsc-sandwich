# baocao/ — báo cáo mỗi phiên làm việc

Mỗi phiên Code viết đúng **một** file mới: `BAOCAO01.md`, `BAOCAO02.md`, ...
theo số tăng dần, không đè lên file cũ, không sửa lại nội dung báo cáo của
phiên trước (kể cả khi phiên đó có lỗi — sửa lỗi thì ghi vào báo cáo MỚI,
không quay lại sửa báo cáo cũ). File `BAOCAO_AUDIT_*.md` là báo cáo audit độc
lập, cùng luật không sửa lại.

## 10 ô bắt buộc

Mỗi `BAOCAO{NN}.md` phải có đủ 10 ô theo đúng thứ tự (xem `CLAUDE.md` mục
"BAOCAO — một phiên một file mới" để có định dạng gốc):

1. **LÁT** — cụm/việc đã làm trong phiên này (ví dụ `0.1+0.2+0.3` hoặc tên
   cụm như `strategy-lock-mode2`). Nếu gộp nhiều cụm trong 1 phiên, ghi hết.
2. **LỆNH NHẬN** — tóm tắt khối lệnh đã nhận (đủ để người đọc sau không cần
   lục lại chat để biết phiên này được giao làm gì).
3. **FILE ĐỔI** — liệt kê từng file đã sửa/thêm/xoá + tóm tắt thay đổi, kèm
   **hash git commit** của cụm này (`git log -1 --format=%H` SAU khi commit
   xong — xem "Luật commit" dưới đây).
4. **LỆNH CHẠY** — các lệnh đã chạy thật để tạo ra số liệu ở ô 5 (`cargo
   test`, `cargo build --release`, `scripts/paper_run.sh`, `curl` API...).
5. **OUTPUT THẬT** — dán tối thiểu 15 dòng cuối của output thật (nếu có
   test/run) — không tóm tắt bằng lời, dán nguyên văn.
6. **CHAIN** — xác nhận `chain_id=0x38` (56) + `eth_getCode`/`eth_call` rút
   gọn nếu phiên có động tới RPC thật, hoặc ghi `MISSING` nếu không áp dụng.
7. **REGISTRY** — venue nào đụng tới trong phiên này, trạng thái pin.
8. **KHÔNG LÀM** — việc nằm trong phạm vi lệnh nhưng cố tình không làm (và
   lý do), khác với "còn nợ" (việc chưa làm được vì lý do khách quan).
9. **CHỮ** — chỉ được là `CHƯA XONG | FAIL | CHỜ GROK`. **Cấm tự viết `ĐẠT`**
   — chỉ người đọc báo cáo (Grok/Chủ) mới được quyết định ĐẠT sau khi xem ô 5.
10. **CÒN NỢ / LÁT SAU** — việc THẬT SỰ chưa làm được (không phải việc ngoài
    phạm vi lệnh), để phiên sau không phải dò lại từ đầu.

## Luật commit + hash (CLAUDE.md mục "3 luật bổ sung")

- Mỗi cụm trong khối lệnh phải kết thúc bằng **1 git commit** trước khi đóng
  phiên. Không commit được (ví dụ hook chặn) → ô 9 ghi `CHƯA XONG`, không
  được ghi `CHỜ GROK`.
- Mọi số liệu runtime dán vào ô 5 (`cargo test`, paper run, `/api/*`, log
  jsonl...) phải kèm: **(a)** máy chạy — ghi rõ `WSL` hay `VPS`, **(b)**
  `sha256sum` của binary đã build lúc chạy (`target/release/bsc_sandwich`)
  hoặc git HEAD hash nếu chạy qua `cargo test`/`cargo run` trực tiếp. Thiếu 1
  trong 2 mục này thì số liệu đó coi như `MISSING`, không tính là bằng chứng
  để Grok/Chủ chấm ĐẠT.
- Test nhóm `real_rpc_*` (`#[ignore]`, gọi RPC thật) chỉ được báo là đã chạy
  khi có output THẬT dán kèm (số block, giá trị quote, tx hash...) — không
  được ghi "ignored" rồi coi như đã verify, không dán dòng `... ok` mà không
  kèm số liệu RPC thật ngay phía trên.

## Quy trình đọc/viết báo cáo

Grok (người điều hành, đọc `CLAUDE.md`) ra khối lệnh tự chứa cho từng cụm;
Chủ chuyển khối lệnh đó cho Claude Code chạy trong phiên trắng; Claude Code
làm xong ghi đúng 1 file `BAOCAO{NN}.md` mới theo khuôn 10 ô ở trên rồi báo
`CHỜ GROK`/`FAIL`/`CHƯA XONG` — không tự phán ĐẠT. Chủ copy nội dung
`BAOCAO{NN}.md` gửi lại cho Grok đọc/duyệt; Grok quyết định ĐẠT hay yêu cầu
sửa (chỉ sửa đúng ô còn thiếu trong cùng cụm, không viết lại repo). Xem
`CLAUDE.md` mục "Mẫu lệnh Grok → copy sang Code" để có khuôn khối lệnh đầy đủ.

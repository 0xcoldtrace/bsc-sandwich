//! Cụm `truth-victim-ok-and-memleak` (mục 3) — đo bộ nhớ THẬT của chính tiến
//! trình bot + liệt kê mọi container sống lâu kèm TRẦN của nó.
//!
//! # Vì sao cụm này tồn tại
//!
//! Lần chạy paper 24 h đầu tiên trên VPS (BAOCAO43 ô 5 mục 0) bị OOM-kill sau
//! 656 phút với `anon-rss 7,6 GB` trên máy 8 GB — tức ~11 MB/phút tăng đều.
//! Không có một số đo bộ nhớ nào trong log nên không truy nguyên được, chỉ
//! biết nó chết. Module này sửa ĐÚNG chỗ đó: bot tự đo `VmRSS` mỗi phút và tự
//! khai báo kích thước từng cấu trúc sống lâu, nên lần sau nếu RSS tăng thì
//! biết NGAY container nào tăng theo, không phải đoán.
//!
//! # `VmRSS` chứ không phải `VmSize`
//!
//! OOM-killer của Linux quyết định theo bộ nhớ ẩn danh THƯỜNG TRÚ (`anon-rss`
//! trong dòng `dmesg`), không theo không gian địa chỉ ảo. `VmSize` của một
//! tiến trình Rust/tokio luôn lớn hơn nhiều lần vì reserve vùng ảo cho arena
//! của allocator — dùng nó sẽ báo động giả. Vì vậy đọc `VmRSS`, và ghi thêm
//! `VmHWM` (đỉnh cao nhất từ lúc khởi động) để thấy được cả spike đã xảy ra
//! giữa 2 lần đo.

/// Một container sống lâu trong bot + trần của nó — dựng bởi `web.rs` từ
/// `AppState` (nơi duy nhất nhìn thấy hết), tiêu thụ bởi `GET /api/mem` và
/// task log `mem.rss_mb`.
#[derive(Debug, Clone)]
pub struct ContainerSize {
    /// Tên đúng như trong mã nguồn, để tra thẳng được.
    pub name: &'static str,
    /// Số phần tử ĐANG sống. `None` = **khoá đang bận** nên lần đo này bỏ
    /// qua container đó.
    ///
    /// Vì sao phải có `None` thay vì chờ lấy khoá: `pairs_vet_task` giữ khoá
    /// ghi `pairbook` trong SUỐT một vòng vet (đo thật 3–4 phút). `RwLock` của
    /// tokio công bằng với writer, nên một `read()` bình thường sẽ xếp hàng
    /// sau writer đó và task đo bộ nhớ đứng im luôn. Đã xảy ra THẬT trong
    /// chính cụm này: lần chạy 62 phút đầu tiên chỉ ghi được **2 dòng**
    /// `mem.rss_mb` (phút 0 và phút 1) rồi tắt tiếng — tức số đo quan trọng
    /// nhất của mục 3 biến mất đúng vì cách đo. Dùng `try_read()`: đo được
    /// thì đo, bận thì ghi `null` và đi tiếp, KHÔNG BAO GIỜ chặn.
    pub len: Option<usize>,
    /// Trần đã đặt. `None` = **chưa có trần** (phải coi là nghi phạm rò rỉ).
    pub cap: Option<usize>,
    /// Trần theo cái gì: `"entry"`, `"block"`, `"dong"`… — `len` và `cap`
    /// không phải lúc nào cũng cùng đơn vị (vd `ReserveCache` chặn theo BLOCK
    /// nhưng `len` đếm ENTRY), nói rõ để không đọc nhầm.
    pub cap_unit: &'static str,
}

/// Một dòng của `/proc/self/status` tính bằng KiB. `None` khi không đọc được
/// (không phải Linux, hoặc `/proc` không mount) — KHÔNG bịa số 0.
fn proc_status_kb(field: &str) -> Option<u64> {
    let content = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix(field) {
            let rest = rest.trim_start_matches(':').trim();
            let num = rest.split_whitespace().next()?;
            return num.parse::<u64>().ok();
        }
    }
    None
}

/// Bộ nhớ thường trú HIỆN TẠI (MiB). `None` = không đọc được.
pub fn rss_mb() -> Option<f64> {
    proc_status_kb("VmRSS").map(|kb| kb as f64 / 1024.0)
}

/// ĐỈNH bộ nhớ thường trú từ lúc khởi động (MiB, `VmHWM`) — bắt được spike
/// xảy ra GIỮA 2 lần đo mỗi phút (vd một lời gọi `/api/econ` đọc cả
/// `bot.jsonl` 200 MB vào RAM rồi giải phóng).
pub fn rss_peak_mb() -> Option<f64> {
    proc_status_kb("VmHWM").map(|kb| kb as f64 / 1024.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trên Linux (môi trường dev = WSL, production = VPS) `/proc/self/status`
    /// luôn có `VmRSS` và nó phải > 0 — nếu test này fail thì mọi con số
    /// `mem.rss_mb` trong BAOCAO đều vô nghĩa, nên nó đáng là test thật.
    #[test]
    #[cfg(target_os = "linux")]
    fn rss_mb_doc_duoc_va_lon_hon_0() {
        let rss = rss_mb().expect("Linux phai doc duoc VmRSS tu /proc/self/status");
        assert!(rss > 0.0, "VmRSS phai > 0, doc duoc {rss}");
        let peak = rss_peak_mb().expect("Linux phai doc duoc VmHWM");
        assert!(peak >= rss, "VmHWM ({peak}) khong the nho hon VmRSS ({rss})");
    }

    #[test]
    fn field_khong_ton_tai_tra_none_khong_panic() {
        assert!(proc_status_kb("KhongCoTruongNay").is_none());
    }
}

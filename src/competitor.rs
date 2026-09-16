//! Cụm `bugfix-presign-and-contract-plan` (A3) — nhận diện victim thuộc CỤM
//! ĐỐI THỦ MEV đã trinh sát được (BAOCAO41 mục 1 + "SỬA GIỮA PHIÊN").
//!
//! # Cụm này là gì (bằng chứng on-chain THẬT, không suy diễn)
//!
//! Trinh sát ở `competitor-recon-and-strategy` xác nhận một hệ thống bot
//! ĐA-VÍ đang hoạt động trên BSC:
//!
//! - `0xB406021E07b31E1f7850FCcCD7076094f18d07eF` — EOA "kho" (nonce >350.000),
//!   nguồn vốn USDT.
//! - `0xa739Dfab40ef6585f1174fcE90EC96330669758c` — contract "dispatcher":
//!   mỗi lời gọi `0x5aab2274` phát ra một loạt cặp
//!   `(Transfer USDT từ 0xB406 → ví X, Approval(0xB406, 0xa739))`, tức CẤP
//!   VỐN TỨC THÌ cho nhiều ví "burner" cùng lúc (verify thật: tx
//!   `0x4916caa0f1…` block `122082156` cấp vốn 3 ví, cả 3 swap ngay 3 vị trí
//!   tx_index kế tiếp trên CÙNG pool).
//! - `0x8180aD6A7c9f8f4864e9909480fbA4123FcE6c54` — ví TRUNG TÂM (nhận vốn
//!   2850 lần / 50.000 block, áp đảo ví xếp thứ 2 chỉ 5 lần).
//!
//! Vì các ví thực sự SWAP là ví dùng-một-lần (không thể liệt kê tĩnh), nhận
//! diện phải ĐỘNG: **bất kỳ ví nào nhận Transfer quote-asset từ một địa chỉ
//! seed trong block hiện tại hoặc block liền trước đều được coi là thuộc
//! cụm** — đúng cơ chế "cấp vốn tức thì rồi swap ngay" đã quan sát.
//!
//! # Dùng để làm gì
//!
//! Đây KHÔNG phải nạn nhân bình thường: front-run một chân của bot khác là
//! đối đầu trực tiếp với hệ thống có sẵn hạ tầng gửi bundle riêng. Cờ
//! `victim_in_competitor_cluster` được ghi vào `sim.result`, đếm riêng trong
//! `/api/econ`, và khi `allow_competitor_victims=false` (ship) + `live_mode`
//! khác `"off"` thì candidate nhóm này KHÔNG được `Simulated` (skip
//! `competitor_victim`).

use alloy::primitives::{Address, B256};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

/// 3 địa chỉ SEED (lệnh A3) — địa chỉ ĐẦY ĐỦ đã verify on-chain ở BAOCAO41.
pub const SEED_ADDRESSES: [&str; 3] = [
    "0xB406021E07b31E1f7850FCcCD7076094f18d07eF", // EOA kho USDT
    "0xa739Dfab40ef6585f1174fcE90EC96330669758c", // contract dispatcher (selector 0x5aab2274)
    "0x8180aD6A7c9f8f4864e9909480fbA4123FcE6c54", // vi trung tam (2850 lan nhan von/50k block)
];

/// `keccak256("Transfer(address,address,uint256)")` — suy runtime, KHÔNG
/// hardcode chuỗi hex (cùng nguyên tắc `pool::sync_topic0`).
pub fn transfer_topic0() -> B256 {
    alloy::primitives::keccak256(b"Transfer(address,address,uint256)")
}

/// Địa chỉ 20 byte nằm ở 12 byte cuối của 1 topic 32 byte (chuẩn ABI index).
pub fn address_from_topic(topic: B256) -> Address {
    Address::from_slice(&topic.as_slice()[12..])
}

pub fn seed_set() -> HashSet<Address> {
    SEED_ADDRESSES.iter().filter_map(|s| Address::from_str(s).ok()).collect()
}

/// Số block giữ lại danh sách ví vừa được seed cấp vốn — lệnh A3 yêu cầu
/// "block hiện tại/trước", tức 2 block.
pub const FUNDED_WINDOW_BLOCKS: usize = 2;

/// Chỉ mục cụm đối thủ: seed TĨNH + ví được cấp vốn ĐỘNG theo block.
#[derive(Debug, Default)]
pub struct ClusterIndex {
    seeds: HashSet<Address>,
    /// `block -> tập ví nhận Transfer quote từ seed trong block đó`.
    funded: HashMap<u64, HashSet<Address>>,
    /// Tổng số lần ghi nhận 1 ví mới được cấp vốn (thống kê cho `/api/compete`).
    funded_total: u64,
}

impl ClusterIndex {
    pub fn new() -> Self {
        Self { seeds: seed_set(), funded: HashMap::new(), funded_total: 0 }
    }

    /// Ghi nhận 1 ví vừa nhận quote-asset từ seed tại `block`.
    pub fn note_funded(&mut self, block: u64, wallet: Address) {
        self.funded.entry(block).or_default().insert(wallet);
        self.funded_total += 1;
        // Chỉ giữ `FUNDED_WINDOW_BLOCKS` block MỚI NHẤT.
        if self.funded.len() > FUNDED_WINDOW_BLOCKS {
            let mut blocks: Vec<u64> = self.funded.keys().copied().collect();
            blocks.sort_unstable();
            let drop_n = blocks.len() - FUNDED_WINDOW_BLOCKS;
            for b in blocks.into_iter().take(drop_n) {
                self.funded.remove(&b);
            }
        }
    }

    /// `true` khi `addr` là seed, HOẶC vừa nhận quote từ seed trong
    /// `current_block`/block liền trước.
    pub fn contains(&self, addr: Address, current_block: u64) -> bool {
        if self.seeds.contains(&addr) {
            return true;
        }
        for b in [current_block, current_block.saturating_sub(1)] {
            if self.funded.get(&b).map(|s| s.contains(&addr)).unwrap_or(false) {
                return true;
            }
        }
        false
    }

    pub fn is_seed(&self, addr: Address) -> bool {
        self.seeds.contains(&addr)
    }

    pub fn funded_total(&self) -> u64 {
        self.funded_total
    }

    pub fn funded_now(&self) -> usize {
        self.funded.values().map(|s| s.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> Address {
        Address::from_str(s).unwrap()
    }

    #[test]
    fn seed_set_has_exactly_the_three_verified_addresses() {
        let set = seed_set();
        assert_eq!(set.len(), 3, "3 dia chi seed phai parse duoc het");
        assert!(set.contains(&a("0xB406021E07b31E1f7850FCcCD7076094f18d07eF")));
        assert!(set.contains(&a("0xa739Dfab40ef6585f1174fcE90EC96330669758c")));
        assert!(set.contains(&a("0x8180aD6A7c9f8f4864e9909480fbA4123FcE6c54")));
    }

    /// ĐẠT CẦN DÁN (A3) — victim `0xaaBae02D453823E0CE3C86f8A1d29d3Da0a3eaf7`
    /// (ví trong dòng A1 của lệnh, đã verify on-chain BAOCAO41: nhận 2757.93
    /// USDT từ `0xB406` ở block `122076185` rồi swap NGAY tx kế tiếp) PHẢI
    /// được nhận diện là thuộc cụm — dù KHÔNG nằm trong danh sách seed tĩnh.
    #[test]
    fn funded_wallet_from_real_block_122076185_is_in_cluster() {
        let victim = a("0xaaBae02D453823E0CE3C86f8A1d29d3Da0a3eaf7");
        let mut idx = ClusterIndex::new();
        assert!(!idx.contains(victim, 122_076_185), "truoc khi thay Transfer thi chua biet gi - khong duoc doan");
        idx.note_funded(122_076_185, victim);
        assert!(idx.contains(victim, 122_076_185), "cung block -> trong cum");
        assert!(idx.contains(victim, 122_076_186), "block ke tiep -> van trong cum (cua so 2 block)");
        assert!(!idx.contains(victim, 122_076_190), "xa hon cua so -> khong con tinh");
    }

    #[test]
    fn seed_address_is_always_in_cluster_regardless_of_block() {
        let idx = ClusterIndex::new();
        assert!(idx.contains(a("0xB406021E07b31E1f7850FCcCD7076094f18d07eF"), 1));
        assert!(idx.contains(a("0xa739Dfab40ef6585f1174fcE90EC96330669758c"), 999_999_999));
        assert!(!idx.contains(a("0x1111111111111111111111111111111111111111"), 1));
    }

    #[test]
    fn funded_window_keeps_only_two_newest_blocks() {
        let mut idx = ClusterIndex::new();
        let w1 = a("0x1111111111111111111111111111111111111111");
        let w2 = a("0x2222222222222222222222222222222222222222");
        let w3 = a("0x3333333333333333333333333333333333333333");
        idx.note_funded(100, w1);
        idx.note_funded(101, w2);
        idx.note_funded(102, w3);
        assert!(!idx.contains(w1, 102), "block 100 da bi day ra khoi cua so 2 block");
        assert!(idx.contains(w2, 102));
        assert!(idx.contains(w3, 102));
        assert_eq!(idx.funded_total(), 3);
    }

    #[test]
    fn address_from_topic_reads_last_20_bytes() {
        let mut raw = [0u8; 32];
        raw[12..].copy_from_slice(a("0xaaBae02D453823E0CE3C86f8A1d29d3Da0a3eaf7").as_slice());
        assert_eq!(address_from_topic(B256::from(raw)), a("0xaaBae02D453823E0CE3C86f8A1d29d3Da0a3eaf7"));
    }

    #[test]
    fn transfer_topic0_matches_known_erc20_signature() {
        // Gia tri chuan cua ERC20 Transfer - doi chieu voi hex cong bo rong rai.
        assert_eq!(
            format!("{:#x}", transfer_topic0()),
            "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
        );
    }
}

// ---------------------------------------------------------------------------
// Cụm `verify-cluster-as-victim` (mục 5) — CẢNH BÁO KHI CỤM ĐỐI THỦ BIẾN MẤT
// ---------------------------------------------------------------------------
//
// Kịch bản rủi ro (chi tiết ở `docs/STATE.md`, mục "Rủi ro phản ứng của cụm
// đối thủ"): cụm 0xB406 có thể (a) chuyển sang gửi tx qua relay private nên
// bot không còn thấy chúng trong mempool, (b) đổi ví seed/dispatcher nên
// `SEED_ADDRESSES` không còn khớp, hoặc (c) đổi sang pool khác không nằm
// trong `pairs.txt`. CẢ BA kịch bản có CÙNG một dấu hiệu quan sát được từ
// phía bot: **số candidate được nhận diện thuộc cụm mỗi giờ tụt mạnh**.
//
// Vì phần lớn cơ hội có lãi đo được cho tới nay đều là tx của cụm này
// (481/497 trong 10,92 h, BAOCAO44), mất nguồn đó = mất phần lớn lý do chạy
// bot; Chủ phải biết NGAY chứ không phải phát hiện sau vài ngày nhìn lãi.
//
// Ngưỡng theo lệnh: **giảm > 80 %/giờ** so với giờ liền trước thì cảnh báo.

/// Số bucket phút giữ lại — 180 phút (3 giờ) là đủ cho 2 cửa sổ 60 phút liền
/// nhau cộng biên, và là TRẦN CỨNG (container này không được phép phình theo
/// thời gian chạy — bài học BUG #3 của `truth-victim-ok-and-memleak`).
pub const CLUSTER_RATE_BUCKET_CAP: usize = 180;
/// Cửa sổ so sánh (phút).
pub const CLUSTER_RATE_WINDOW_MIN: u64 = 60;
/// Ngưỡng cảnh báo theo lệnh: tụt hơn 80 % so với cửa sổ liền trước.
pub const CLUSTER_RATE_ALERT_DROP_PCT: f64 = 80.0;
/// Cửa sổ trước phải có ÍT NHẤT ngần này candidate cụm thì mới được coi là có
/// "đường nền" để so — nếu không, một giờ vốn dĩ vắng khách sẽ sinh cảnh báo
/// giả (0 → 0 hay 2 → 0 không nói lên điều gì).
pub const CLUSTER_RATE_MIN_BASELINE: u64 = 20;
/// Không cảnh báo lại trong vòng ngần này phút kể từ lần cảnh báo trước —
/// tránh spam log mỗi chu kỳ kiểm khi tình trạng kéo dài.
pub const CLUSTER_RATE_ALERT_COOLDOWN_MIN: u64 = 60;

/// Một lần cảnh báo "cụm đối thủ đang biến mất khỏi tầm nhìn của bot".
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterRateAlert {
    /// Số candidate thuộc cụm trong 60 phút gần nhất.
    pub cur_cluster: u64,
    /// Số candidate thuộc cụm trong 60 phút LIỀN TRƯỚC đó.
    pub prev_cluster: u64,
    /// Phần trăm sụt giảm (`(prev - cur) / prev * 100`).
    pub drop_pct: f64,
    /// Tổng candidate (mọi nguồn) của 2 cửa sổ — để phân biệt "cụm biến mất"
    /// với "cả mempool im lặng / bot mất kết nối WS".
    pub cur_total: u64,
    pub prev_total: u64,
}

/// Đếm candidate theo phút để phát hiện cụm đối thủ biến mất.
///
/// Bộ đếm này KHÔNG nằm trên đường nóng theo nghĩa RPC — `note()` chỉ đẩy 1 số
/// vào bucket phút cuối, còn `evaluate()` do một task nền gọi định kỳ.
#[derive(Debug, Default)]
pub struct ClusterRateWatch {
    /// `(phút epoch, candidate thuộc cụm, tổng candidate)`, tăng dần theo phút.
    buckets: std::collections::VecDeque<(u64, u64, u64)>,
    alerts: u64,
    last_alert_minute: Option<u64>,
}

impl ClusterRateWatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ghi nhận 1 candidate đã đi tới bước nhận diện cụm tại `minute` (phút
    /// epoch = `unix_ts / 60`).
    pub fn note(&mut self, minute: u64, in_cluster: bool) {
        match self.buckets.back_mut() {
            Some((m, c, t)) if *m == minute => {
                *t += 1;
                if in_cluster {
                    *c += 1;
                }
            }
            _ => self.buckets.push_back((minute, u64::from(in_cluster), 1)),
        }
        while self.buckets.len() > CLUSTER_RATE_BUCKET_CAP {
            self.buckets.pop_front();
        }
    }

    /// Tổng `(cụm, tổng)` của các bucket nằm trong `[now - older + 1, now - newer]`
    /// tính bằng phút lùi về quá khứ.
    pub fn window(&self, now_minute: u64, newer_ago: u64, older_ago: u64) -> (u64, u64) {
        let hi = now_minute.saturating_sub(newer_ago);
        let lo = now_minute.saturating_sub(older_ago);
        self.buckets
            .iter()
            .filter(|(m, _, _)| *m >= lo && *m <= hi)
            .fold((0, 0), |(c, t), (_, bc, bt)| (c + bc, t + bt))
    }

    /// So cửa sổ 60 phút gần nhất với 60 phút liền trước; trả `Some(alert)` khi
    /// vi phạm ngưỡng VÀ chưa cảnh báo trong `CLUSTER_RATE_ALERT_COOLDOWN_MIN`.
    pub fn evaluate(&mut self, now_minute: u64) -> Option<ClusterRateAlert> {
        let w = CLUSTER_RATE_WINDOW_MIN;
        let (cur_cluster, cur_total) = self.window(now_minute, 0, w - 1);
        let (prev_cluster, prev_total) = self.window(now_minute, w, 2 * w - 1);
        if prev_cluster < CLUSTER_RATE_MIN_BASELINE {
            return None;
        }
        let drop_pct = (prev_cluster - cur_cluster.min(prev_cluster)) as f64 / prev_cluster as f64 * 100.0;
        if drop_pct <= CLUSTER_RATE_ALERT_DROP_PCT {
            return None;
        }
        if let Some(last) = self.last_alert_minute {
            if now_minute.saturating_sub(last) < CLUSTER_RATE_ALERT_COOLDOWN_MIN {
                return None;
            }
        }
        self.last_alert_minute = Some(now_minute);
        self.alerts += 1;
        Some(ClusterRateAlert { cur_cluster, prev_cluster, drop_pct, cur_total, prev_total })
    }

    pub fn alerts(&self) -> u64 {
        self.alerts
    }

    pub fn len(&self) -> usize {
        self.buckets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }
}

#[cfg(test)]
mod rate_tests {
    use super::*;

    /// Phút epoch giả lập: 0 = "cách đây 120 phút", 119 = "phút hiện tại".
    fn fill(w: &mut ClusterRateWatch, from: u64, to: u64, cluster_per_min: u64, other_per_min: u64) {
        for m in from..=to {
            for _ in 0..cluster_per_min {
                w.note(m, true);
            }
            for _ in 0..other_per_min {
                w.note(m, false);
            }
        }
    }

    #[test]
    fn tut_hon_80_phan_tram_thi_canh_bao() {
        let mut w = ClusterRateWatch::new();
        fill(&mut w, 0, 59, 2, 3); // gio truoc: 120 candidate cum
        fill(&mut w, 60, 119, 0, 3); // gio nay: 0 candidate cum
        let alert = w.evaluate(119).expect("phai canh bao khi tut 100%");
        assert_eq!(alert.prev_cluster, 120);
        assert_eq!(alert.cur_cluster, 0);
        assert!((alert.drop_pct - 100.0).abs() < 1e-9);
        // Tong candidate VAN cao -> phan biet duoc "cum bien mat" voi "mat WS".
        assert_eq!(alert.cur_total, 180);
    }

    #[test]
    fn tut_duoi_nguong_thi_im_lang() {
        let mut w = ClusterRateWatch::new();
        fill(&mut w, 0, 59, 2, 0); // 120
        fill(&mut w, 60, 119, 1, 0); // 60 -> tut 50%, duoi nguong 80%
        assert!(w.evaluate(119).is_none());
    }

    #[test]
    fn duong_nen_qua_thap_thi_khong_canh_bao_gia() {
        let mut w = ClusterRateWatch::new();
        // Gio truoc chi co 5 candidate cum (< CLUSTER_RATE_MIN_BASELINE=20).
        for m in 0..5 {
            w.note(m, true);
        }
        fill(&mut w, 60, 119, 0, 10);
        assert!(w.evaluate(119).is_none(), "gio vang khach khong duoc sinh canh bao gia");
    }

    #[test]
    fn co_cooldown_khong_spam_moi_chu_ky() {
        let mut w = ClusterRateWatch::new();
        fill(&mut w, 0, 59, 2, 0);
        fill(&mut w, 60, 119, 0, 1);
        assert!(w.evaluate(119).is_some());
        assert!(w.evaluate(120).is_none(), "trong cooldown 60 phut -> im lang");
        assert_eq!(w.alerts(), 1);
    }

    #[test]
    fn bucket_co_tran_cung_khong_phinh_theo_thoi_gian() {
        let mut w = ClusterRateWatch::new();
        for m in 0..1000 {
            w.note(m, m % 2 == 0);
        }
        assert_eq!(w.len(), CLUSTER_RATE_BUCKET_CAP, "phai cat con dung tran 180 bucket");
    }

    #[test]
    fn window_cat_dung_bien_60_phut() {
        let mut w = ClusterRateWatch::new();
        fill(&mut w, 0, 119, 1, 0);
        let (cur, _) = w.window(119, 0, 59);
        let (prev, _) = w.window(119, 60, 119);
        assert_eq!(cur, 60, "cua so hien tai dung 60 phut");
        assert_eq!(prev, 60, "cua so truoc dung 60 phut, khong chong lan");
    }
}

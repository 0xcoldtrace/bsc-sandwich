// Cụm `rpc-probe`: module thật giờ sống trong lib crate (`src/lib.rs`) để
// `src/bin/rpc_probe.rs` dùng lại được `transport::parse_rpc_url_list`/
// `filter_read_urls`/`redact_rpc_url` — binary chính import qua
// `bsc_sandwich::...` thay vì tự `mod ...` như trước.
use bsc_sandwich::config::{Config, RiskGuard};
use bsc_sandwich::decoder::SwapVenue;
use bsc_sandwich::logger::BotLogger;
use bsc_sandwich::pairbook::{PairBook, RpcPairResolver};
use bsc_sandwich::pipeline::{self, PipelineOutcome, TxLogMeta};
use bsc_sandwich::sim_v2::PoolReserves;
use bsc_sandwich::state::{BotState, StateFiles};
use bsc_sandwich::tax::{self, TaxCache};
use bsc_sandwich::transport::{self, PendingTxRaw};
use bsc_sandwich::venues::{self, V2_FACTORY_ADDRESS};
use bsc_sandwich::victims::VictimBook;
use bsc_sandwich::web::{build_router, AppState, AppStateInner, FunnelCounters};

use alloy::primitives::Address;
use alloy::providers::Provider;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".to_string());

    let cfg = match Config::load(std::path::Path::new(&config_path)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("FAIL load config {config_path}: {e}");
            std::process::exit(1);
        }
    };

    println!(
        "bsc_sandwich boot: chain_id={} dry_run={} allow_live={} bot_armed={}",
        cfg.chain_id, cfg.dry_run, cfg.allow_live, cfg.bot_armed
    );

    let state_files = Arc::new(StateFiles::new("state")?);
    let logger = Arc::new(BotLogger::new("logs/bot.jsonl")?);

    // Cum `econ-truth-latency-vps` (muc 1) - hook panic GIU LAI VINH VIEN
    // (khong phai chan doan tam thoi): panic ben trong 1 task da
    // `tokio::spawn` (vd `handle_paper_tx`) KHONG lam sap tien trinh chinh,
    // chi lam CHET AM THAM rieng task do (JoinHandle bi bo qua, khong ai
    // `.await` de biet loi) - truoc cum nay, dieu do xay ra that (nguyen
    // nhan goc lech funnel.simulated vs sim.result, BAOCAO39: 27 vs 14, xem
    // pipeline::log_outcome_v2 - da fix) ma KHONG CO CACH NAO thay duoc tu
    // log binh thuong. Hook nay bien moi panic tuong lai (bat ky nguyen nhan
    // gi) thanh 1 dong `debug.panic` THAY VI bien mat im lang - re, khong
    // anh huong hanh vi binh thuong (chi chay khi that su panic).
    {
        let panic_logger = logger.clone();
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            panic_logger.log(
                "debug.panic",
                serde_json::json!({
                    "message": info.to_string(),
                    "location": info.location().map(|l| l.to_string()),
                }),
            );
            default_hook(info);
        }));
    }

    let mut book = VictimBook::new();
    let victims_path = PathBuf::from(&cfg.victims_path);
    if let Err(e) = book.load_from_file(&victims_path) {
        eprintln!("victims load loi (tiep tuc voi 0 victim, khong crash): {e}");
    }

    logger.log(
        "bot.start",
        serde_json::json!({ "chain_id": cfg.chain_id, "dry_run": cfg.dry_run }),
    );
    logger.log(
        "victim.reload",
        serde_json::json!({ "count": book.len(), "error_lines": book.error_lines }),
    );

    // Cum `5.2` muc 2 - canh bao BOOT (KHONG fail load) khi min_profit_bnb
    // thap hon tong gas 2 chieu front+back - xem Config::gas_warning_needed.
    // Config ship mac dinh (min_profit_bnb=0.001 < gas_total=0.006 BNB)
    // CHAC CHAN bat canh bao nay, dung de smoke-test khong can chinh tay.
    if cfg.gas_warning_needed() {
        let gas_total_wei = cfg.gas_wei();
        logger.log(
            "config.gas_warning",
            serde_json::json!({
                "min_profit_bnb": cfg.min_profit_bnb,
                "front_max_gas_bnb_wei": cfg.front_max_gas_bnb_wei,
                "back_max_gas_bnb_wei": cfg.back_max_gas_bnb_wei,
                "gas_total_wei": gas_total_wei.to_string(),
                "message": "min_profit_bnb thap hon tong gas front+back, chi la canh bao, khong fail load",
            }),
        );
        eprintln!(
            "CANH BAO: min_profit_bnb={} BNB thap hon tong gas front+back cap ({} wei) - xem logs/bot.jsonl config.gas_warning",
            cfg.min_profit_bnb, gas_total_wei
        );
    }

    let reload_interval = Duration::from_secs(cfg.victims_reload_sec.max(1));
    let config_reload_interval = Duration::from_secs(cfg.config_reload_sec.max(1));
    let pending_poll_interval = Duration::from_millis(cfg.pending_poll_ms.max(1));
    // Cum pair-mode - PairBook (pairs.txt), cung khuon victims_path o tren.
    let pairs_path = PathBuf::from(&cfg.pairs_path);
    let pairs_reload_interval = Duration::from_secs(cfg.pairs_reload_sec.max(1));
    let web_bind = cfg.web_bind.clone();
    let web_port = cfg.web_port;
    let config_path_buf = PathBuf::from(&config_path);

    // Cum `5.3` — pool nhieu URL HTTP doc (eth_call/getBlock/txpool_content):
    // uu tien BSC_HTTP_LIST (phay) hoac BSC_HTTP+BSC_HTTP_2..16, fallback
    // vps.json khi rong. Loc bo URL kenh gui/private (maxbackrun/fullprivacy/
    // privacy trong host) khoi pool DOC nay (CLAUDE.md lenh 5.3 muc A4) — cac
    // URL do van co the dung cho kenh gui live sau nay (7.x), chua lam o day.
    // Khong dua http_pool vao AppStateInner (web.rs) — truyen tay qua tham so
    // ham de khong phai sua struct dinh nghia o file khac ngoai pham vi lenh.
    let vps_fallback_boot = transport::VpsFallback::load(std::path::Path::new("vps.json"));
    let mut http_urls = transport::collect_rpc_urls_from_env("BSC_HTTP");
    if http_urls.is_empty() {
        if let Some(v) = transport::pick_url(None, &vps_fallback_boot.rpc_http) {
            http_urls.push(v);
        }
    }
    let http_urls_filtered = transport::filter_read_urls(http_urls);
    let http_pool = Arc::new(transport::RpcPool::new(http_urls_filtered.clone()));
    // Cum `econ-truth-latency-vps` (0.d) - pool RPC RIENG cho revm/vet/
    // validator (pairs_vet_task/gas_units_boot_task): mac dinh dung LAI danh
    // sach BSC_HTTP filtered (khong URL private) - Chu co the tro rieng
    // BSC_HTTP_SIM (vd node ho tro getStorageAt/state day du hon, tranh loi
    // "-32000 not supported" quan sat that tu bloXroute tren mot so RPC
    // public/private khong ho tro het state can cho revm fork).
    // Cum `bugfix-presign-and-contract-plan` (A5) - RPC NEN tach rieng khoi
    // duong nong: moi viec NEN (shadow pre-sign/ky, vet dinh ky, compete.check,
    // validator, recon) dung `BSC_HTTP_BG`; neu Chu khong dat bien do thi mac
    // dinh = 3 URL CUOI cua `BSC_HTTP` (danh sach da loc URL private). Duong
    // nong (`http_pool`) van bat dau tu URL DAU danh sach va chi doi URL khi
    // URL do chet (`RpcPool` luon connect tu index hien tai) - nen viec nen
    // khong con tranh chap cung ket noi voi `handle_paper_tx` (BAOCAO41: p95
    // seen_to_decision TANG 321->356ms sau khi them shadow task nen).
    let bg_urls_raw = transport::collect_rpc_urls_from_env("BSC_HTTP_BG");
    let bg_urls_raw_present = !bg_urls_raw.is_empty();
    let bg_urls = if !bg_urls_raw.is_empty() {
        transport::filter_read_urls(bg_urls_raw)
    } else if http_urls_filtered.len() >= 2 {
        // 3 URL cuoi (hoac it hon neu danh sach ngan) - KHONG bao gio lay URL
        // dau tien (duong nong giu rieng no) khi con >= 2 URL.
        let start = http_urls_filtered.len().saturating_sub(3).max(1);
        http_urls_filtered[start..].to_vec()
    } else {
        // Chi co 1 URL: khong the tach that su - dung chung, ghi ro trong log
        // boot ben duoi (khong bia "da tach").
        http_urls_filtered.clone()
    };
    let bg_http_pool = Arc::new(transport::RpcPool::new(bg_urls.clone()));
    logger.log(
        "rpc.bg_pool",
        serde_json::json!({
            "bg_url_count": bg_urls.len(),
            "hot_url_count": http_urls_filtered.len(),
            "separated": http_urls_filtered.len() >= 2,
            "source": if bg_urls_raw_present { "BSC_HTTP_BG" } else { "3 URL cuoi cua BSC_HTTP (mac dinh)" },
            "bg_urls": bg_urls.iter().map(|u| transport::redact_rpc_url(u)).collect::<Vec<_>>(),
        }),
    );
    let sim_urls_raw = transport::collect_rpc_urls_from_env("BSC_HTTP_SIM");
    // `BSC_HTTP_SIM` (revm fork) mac dinh dung LAI danh sach NEN (khong phai
    // toan bo BSC_HTTP nhu truoc A5) - cung ly do tach tren.
    // `revm` fork doi node co state cu (archive-ish). Danh sach NEN dung
    // TRUOC, nhung PHAI noi them cac URL con lai lam DU PHONG - quan sat that
    // (A5): `bsc-rpc.publicnode.com` trong nhom NEN tra `-32602 Archive
    // requests require a personal token` cho MOI lan doc storage, neu pool
    // sim chi co 3 URL nen thi co the khong con URL nao fork duoc, vet chet
    // hoan toan. `RpcPool` luon bat dau tu index 0 va chi di tiep khi loi ->
    // truong hop binh thuong van dung URL NEN, khong dung duong nong.
    let sim_urls = if sim_urls_raw.is_empty() {
        let mut v = bg_urls.clone();
        for u in &http_urls_filtered {
            if !v.contains(u) {
                v.push(u.clone());
            }
        }
        v
    } else {
        transport::filter_read_urls(sim_urls_raw)
    };
    let sim_http_pool = Arc::new(transport::RpcPool::new(sim_urls));
    let initial_gas_units = (cfg.gas_units_front, cfg.gas_units_back);

    // Cụm `competitor-recon-and-strategy` (mục 4, shadow mode) — load
    // `PRIVATE_KEY` THẬT CHỈ khi `live_mode="shadow"` (ship `"off"` — nhánh
    // này không chạy, hành vi y hệt trước cụm này). Lỗi load (thiếu key/key
    // rác) -> log rõ + `shadow_wallet=None` (KHÔNG panic, KHÔNG chặn boot —
    // bot vẫn chạy paper bình thường, chỉ mất khả năng ký shadow).
    let shadow_wallet: Option<(Address, alloy::network::EthereumWallet)> = if cfg.live_mode_is_shadow() {
        match bsc_sandwich::shadow::load_shadow_signer("PRIVATE_KEY", cfg.chain_id) {
            Ok(signer) => {
                let addr = bsc_sandwich::shadow::self_address(&signer);
                logger.log("shadow.signer_loaded", serde_json::json!({ "self_address": format!("{addr:#x}") }));
                Some((addr, alloy::network::EthereumWallet::from(signer)))
            }
            Err(e) => {
                logger.log("shadow.signer_load_failed", serde_json::json!({ "reason": e }));
                None
            }
        }
    } else {
        None
    };

    let app_state = Arc::new(AppStateInner {
        config: RwLock::new(cfg),
        victims: RwLock::new(book),
        pairbook: RwLock::new(PairBook::new()),
        state_files: state_files.clone(),
        logger: logger.clone(),
        bot_state: RwLock::new(BotState::Idle),
        start_time: Instant::now(),
        boot_wall_clock: chrono::Utc::now(),
        skip_counts: RwLock::new(HashMap::new()),
        last_block: RwLock::new(None),
        provider: RwLock::new(None),
        tax_cache: RwLock::new(TaxCache::new()),
        validate_log: RwLock::new(bsc_sandwich::web::ValidateStats::default()),
        pending_semaphore: Arc::new(Semaphore::new(4)),
        pending_source: RwLock::new(transport::PendingSource::InjectOnly),
        risk_guard: RwLock::new(RiskGuard::new()),
        nonce_cache: RwLock::new(transport::NonceCache::new()),
        funnel: FunnelCounters::new(),
        gas_oracle: transport::GasOracle::new(),
        gas_units: RwLock::new(initial_gas_units),
        reserve_cache: RwLock::new(transport::ReserveCache::new()),
        pairs_first_reload_done: Arc::new(tokio::sync::Notify::new()),
        sim_provider: RwLock::new(None),
        bg_provider: RwLock::new(None),
        mined_index: RwLock::new(transport::MinedTxIndex::new()),
        self_nonce: RwLock::new(transport::SelfNonceCache::new()),
        competitor_cluster: RwLock::new(bsc_sandwich::competitor::ClusterIndex::new()),
        candidate_seen: RwLock::new(HashMap::new()),
        seen_hashes: RwLock::new(transport::SeenHashSet::new()),
        compete_stats: bsc_sandwich::web::CompeteStats::new(),
        shadow_wallet,
    });

    {
        let mut st = app_state.bot_state.write().await;
        *st = BotState::Watching;
    }

    let reload_state = app_state.clone();
    let reload_path = victims_path.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(reload_interval);
        loop {
            interval.tick().await;
            let now = Instant::now();
            let mut book = reload_state.victims.write().await;
            if book.reload_if_due(&reload_path, reload_interval, now) {
                reload_state.logger.log(
                    "victim.reload",
                    serde_json::json!({ "count": book.len(), "error_lines": book.error_lines }),
                );
            }
        }
    });

    // Cum config-hot-reload: giong het khuon mau victims o tren
    // (Config::reload_if_due tu ghi log/giu config cu khi file loi, khong
    // panic) - chu doi min_profit_bnb/max_front_bnb/... trong luc bot dang
    // chay, lan decide_paper SAU dung ngay, khong can restart.
    let cfg_reload_state = app_state.clone();
    let cfg_reload_path = config_path_buf.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(config_reload_interval);
        loop {
            interval.tick().await;
            let now = Instant::now();
            let mut cfg = cfg_reload_state.config.write().await;
            if cfg.reload_if_due(&cfg_reload_path, config_reload_interval, now) {
                cfg_reload_state.logger.log(
                    "config.reload",
                    serde_json::json!({
                        "min_profit_bnb": cfg.min_profit_bnb,
                        "max_front_bnb": cfg.max_front_bnb,
                        "min_reserve_wbnb": cfg.min_reserve_wbnb,
                        "max_roundtrip_tax": cfg.max_roundtrip_tax,
                    }),
                );
            }
        }
    });

    // Cum pair-mode - PairBook hot-reload, cung khuon victims/config o tren,
    // KHAC o cho can provider RPC that de resolve getPair (V2 factory da pin)
    // - chua co provider (chua connect_rpc xong/dang failover) thi bo qua tick
    // nay, log skip, thu lai o tick sau (KHONG halt bot).
    let pair_reload_state = app_state.clone();
    let pair_reload_http_pool = http_pool.clone();
    tokio::spawn(async move {
        let factory =
            Address::from_str(V2_FACTORY_ADDRESS).expect("V2_FACTORY_ADDRESS da pin phai la address hop le");
        let mut interval = tokio::time::interval(pairs_reload_interval);
        loop {
            interval.tick().await;
            let now = Instant::now();
            let due = {
                let book = pair_reload_state.pairbook.read().await;
                match book.last_reload {
                    None => true,
                    Some(last) => now.saturating_duration_since(last) >= pairs_reload_interval,
                }
            };
            if !due {
                continue;
            }
            let provider_opt = pair_reload_state.provider.read().await.clone();
            let provider = match provider_opt {
                Some(p) => p,
                None => {
                    pair_reload_state.logger.log(
                        "pair.reload_skip",
                        serde_json::json!({ "reason": "chua co provider HTTP, thu lai tick sau" }),
                    );
                    let mut book = pair_reload_state.pairbook.write().await;
                    book.last_reload = Some(now); // tranh retry moi tick khi provider chua san sang
                    continue;
                }
            };
            let url_label = pair_reload_http_pool.current_url_label().await.unwrap_or_default();
            let resolver = RpcPairResolver { provider, factory, url_label };
            // Cum `strategy-lock-mode2` - doc `pairs_require_vetted` THAT tu
            // config hien hanh (hot-reload duoc, khong hardcode) truoc moi
            // lan reload - gate vet nam trong PairBook::reload chinh no.
            let require_vetted = pair_reload_state.config.read().await.pairs_require_vetted;
            let mut book = pair_reload_state.pairbook.write().await;
            let did_reload = book
                .reload_if_due(
                    &pairs_path,
                    &resolver,
                    &pair_reload_state.logger,
                    pairs_reload_interval,
                    now,
                    require_vetted,
                )
                .await;
            // Cum `econ-truth-latency-vps` (0.c) - neu bat ky dong pending nao
            // vua loi vi "method khong ho tro" (-32000/not supported/method
            // not found) tren URL HTTP hien tai - danh dau URL do, chuyen URL
            // HTTP KE trong pool ngay (khong doi health-check phat hien),
            // tranh ca 90+ dong pairs.txt lap lai cung 1 loi tren cung 1 URL
            // hong o tick sau.
            if did_reload {
                let hit_unsupported = book
                    .pending_entries()
                    .iter()
                    .any(|(_, _, _, err, _)| transport::is_unsupported_method_error(err));
                if hit_unsupported {
                    if let Some(new_provider) =
                        pair_reload_http_pool.mark_current_unsupported(&pair_reload_state.logger, "http").await
                    {
                        *pair_reload_state.provider.write().await = Some(new_provider);
                    }
                }
            }
            drop(book);
            // Cum B5 - bao hieu lan reload THAT DAU TIEN (co provider, thuc
            // su chay PairBook::reload) da xong - gas_units_boot_task cho tin
            // hieu nay thay vi doan thoi gian co dinh. `notify_one` luu 1
            // "permit" neu chua ai dang cho (dung ca 2 thu tu: reload xong
            // truoc hay gas_units_boot_task cho truoc deu dung).
            if did_reload {
                pair_reload_state.pairs_first_reload_done.notify_one();
            }
        }
    });

    // Cum `econ-truth-latency-vps` (0.d) - giu sim_http_pool song (provider
    // RIENG cho revm/vet/validator, tach khoi duong nong).
    tokio::spawn(sim_pool_health_check(app_state.clone(), sim_http_pool.clone(), Duration::from_secs(5)));
    // Cum `bugfix-presign-and-contract-plan` (A5) - giu `bg_provider` song
    // (RPC NEN: shadow/vet/compete/validator), doc lap voi duong nong.
    tokio::spawn(bg_pool_health_check(app_state.clone(), bg_http_pool.clone(), Duration::from_secs(5)));

    // Cum `strategy-lock-mode2` - vet NEN dinh ky (revm that, KHONG chan
    // duong nong) cho moi entry `pairs.txt` da co `vetted` - xem
    // `pairs_vet_task` duoi day.
    tokio::spawn(pairs_vet_task(app_state.clone(), sim_http_pool.clone()));

    // Cum `real-economics-mode2` (F-03) - do gas UNIT that 1 lan luc boot
    // bang revm tren 1 pair da vet trong pairs.txt (fallback config
    // gas_units_front/back neu do loi/khong co pair nao san sang) - xem
    // gas_units_boot_task duoi day.
    tokio::spawn(gas_units_boot_task(app_state.clone(), sim_http_pool.clone()));

    // Cum A6 - bo dem funnel gio nam trong `app_state.funnel`
    // (AppStateInner, src/web.rs) - moi ham spawn tx doc/ghi truc tiep qua
    // `app_state.funnel`, khong can truyen Arc rieng nua.
    connect_rpc(app_state.clone(), http_pool.clone(), pending_poll_interval).await;

    // Cum `5.3` — health-check dinh ky provider HTTP hien tai qua http_pool:
    // get_block_number() loi/chua co provider -> connect()/advance_and_reconnect()
    // sang URL ke trong pool (quay vong), cap nhat app_state.provider/last_block.
    // Bao ve MOI noi doc app_state.provider (bao gom pipeline::resolve_v2_reserves
    // qua handle_paper_tx) ma KHONG can doi chu ky pipeline.rs (giu tach loi
    // thuan/RPC that dung quy uoc docs/STATE.md).
    tokio::spawn(http_pool_health_check(app_state.clone(), http_pool.clone(), Duration::from_secs(5)));

    // Cum 5.1 - doc state/inject_tx.jsonl de bom tx test khi pending that
    // trong (khong co BSC_WS / node khong day pending) - chay song song voi
    // pending subscription that, khong loai tru lan nhau.
    tokio::spawn(watch_inject_file(app_state.clone()));

    // Cum tax-cache-inject - doc state/tax_inject.jsonl de chu/test dien
    // tax_cache luc bot dang chay, khong phai sua code/build lai.
    tokio::spawn(watch_tax_inject_file(app_state.clone()));

    // Cum A6 - log tong hop funnel moi 60 giay - KHONG phai nguong chu chinh
    // trong config.toml, cadence noi bo giong `http_pool_health_check`/
    // `watch_inject_file`.
    tokio::spawn(funnel_report_task(app_state.clone(), Duration::from_secs(60)));

    // Cum `exec-path-traps` (V-06) - theo doi state/halt.lock (log
    // halt.triggered/halt.cleared dung 1 lan moi lan chuyen trang thai + cap
    // nhat bot_state). Cadence 1s - phai du nhanh de paper_run.sh (cho toi da
    // 10s) thay duoc su kien nay som.
    tokio::spawn(halt_watch_task(app_state.clone(), Duration::from_secs(1)));

    let addr = format!("{web_bind}:{web_port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!(
        "web dashboard bind tai http://{addr} (dry_run={})",
        app_state.config.read().await.dry_run
    );

    let router = build_router(app_state.clone());

    tokio::select! {
        res = axum::serve(listener, router) => {
            if let Err(e) = res {
                eprintln!("web server loi: {e}");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("nhan Ctrl+C, dung bot");
            let mut st = app_state.bot_state.write().await;
            *st = BotState::Stopped;
        }
    }

    Ok(())
}

/// Cụm `5.3` (thay `2.1`): kết nối HTTP qua `http_pool` (đã build ở `main()`
/// từ `BSC_HTTP`/`BSC_HTTP_2..16`/`BSC_HTTP_LIST` + fallback `vps.json`) —
/// `RpcPool::connect` tự thử lần lượt, log `rpc.failover` cho URL lỗi,
/// `rpc.connect` cho URL thành công. Không URL nào connect được -> log
/// `rpc.skip`, boot vẫn tiếp tục — không panic, không halt. `BSC_WS` đọc
/// thành DANH SÁCH tương tự (`ws_urls`), truyền cho `subscribe_ws_heads`
/// (best-effort, block header) và `subscribe_pending_txs` (fallback
/// WSS -> WSS khác trong list -> `txpool_content` qua `http_pool`).
async fn connect_rpc(app_state: AppState, http_pool: Arc<transport::RpcPool>, pending_poll_interval: Duration) {
    match http_pool.connect(&app_state.logger, "http").await {
        Some(provider) => {
            if let Ok(block_number) = provider.get_block_number().await {
                *app_state.last_block.write().await = Some(block_number);
            }
            // Cum 5.1 - giu lai provider HTTP da verify de dung cho
            // pipeline::resolve_v2_reserves (eth_call getPair/getReserves
            // that) trong live paper loop, khong mo ket noi rieng moi tx.
            *app_state.provider.write().await = Some(provider);
        }
        None => {
            app_state.logger.log(
                "rpc.skip",
                serde_json::json!({
                    "transport": "http",
                    "reason": "khong URL HTTP nao trong pool ket noi duoc (rong hoac tat ca fail, xem rpc.failover)"
                }),
            );
        }
    }

    let vps_fallback = transport::VpsFallback::load(std::path::Path::new("vps.json"));
    let mut ws_urls = transport::collect_rpc_urls_from_env("BSC_WS");
    if ws_urls.is_empty() {
        if let Some(v) = transport::pick_url(None, &vps_fallback.rpc_ws) {
            ws_urls.push(v);
        }
    }
    if ws_urls.is_empty() {
        app_state.logger.log(
            "rpc.skip",
            serde_json::json!({
                "transport": "ws",
                "reason": "BSC_WS rong (.env chua dien) va vps.json chua co fallback hop le"
            }),
        );
    }

    tokio::spawn(subscribe_ws_heads(app_state.clone(), ws_urls.clone()));

    // Cum `econ-truth-latency-vps` (muc 3) - subscribe Sync event cho cac
    // pool trong PairBook, cap nhat ReserveCache TRUOC khi candidate nao cham
    // toi (giam do tre so voi cho eth_call getReserves tren duong nong).
    tokio::spawn(subscribe_sync_events(app_state.clone(), ws_urls.clone()));

    // Cum `bugfix-presign-and-contract-plan` (A4) - nap "mempool view" (hash
    // tx cua 3 block gan nhat) + prefetch nonce vi shadow moi block, tren RPC
    // NEN - de duong ky KHONG con goi RPC nao.
    tokio::spawn(mined_and_nonce_prefetch_task(app_state.clone()));

    // Cum `bugfix-presign-and-contract-plan` (A3) - theo doi Transfer quote
    // asset TU 3 dia chi seed cua cum doi thu -> nhan dien vi "burner" duoc
    // cap von tuc thi (co che THAT da verify on-chain, xem src/competitor.rs).
    tokio::spawn(subscribe_competitor_funding(app_state.clone(), ws_urls.clone()));

    // Cum 5.2+5.3 - fallback chain WSS (nhieu URL, lag/rot thi thu WSS KE
    // trong danh sach truoc) -> txpool_content (qua http_pool, failover URL
    // HTTP ke khi loi) -> chi con inject_only. Khong halt bot o bat ky nhanh
    // nao (CLAUDE.md).
    tokio::spawn(subscribe_pending_txs(app_state, ws_urls, http_pool, pending_poll_interval));
}

/// Cụm `econ-truth-latency-vps` (mục 3) — subscribe log `Sync(uint112,uint112)`
/// (topic0 `pool::sync_topic0()`) qua WSS cho ĐÚNG các pool đang có trong
/// `PairBook` — mỗi log nhận được cập nhật THẲNG `ReserveCache` tại block đó,
/// KHÔNG cần `eth_call getReserves` trên đường nóng (`handle_paper_tx` vẫn
/// giữ fallback `eth_call` khi cache miss/thiếu >2 block, xem
/// `resolve_reserves_cached`/`ReserveCache` — không đổi phần đó).
///
/// `token0` của mỗi pool chỉ cần biết 1 LẦN (không đổi theo thời gian, bất
/// biến on-chain) — cache riêng trong task này (`token0_cache`, không chia
/// `AppStateInner` vì chỉ dùng ở đây), tránh gọi `eth_call token0()` lặp lại
/// mỗi Sync event (chỉ 1 lần/pool trong suốt vòng đời task).
///
/// Danh sách pool theo dõi CHỈ cập nhật khi RESUBSCRIBE (mỗi
/// `RESYNC_INTERVAL`, mặc định 10 phút) — `pairs.txt` hiếm khi đổi giữa
/// phiên chạy (Chủ vet tay), đánh đổi chấp nhận được so với việc phải huỷ/
/// tạo lại subscription mỗi lần `pair.reload` để đổi bộ lọc theo thời gian
/// thực (phức tạp hơn nhiều, ngoài phạm vi cụm này).
async fn subscribe_sync_events(app_state: AppState, ws_urls: Vec<String>) {
    use alloy::rpc::types::eth::Filter;

    const RESYNC_INTERVAL: Duration = Duration::from_secs(600);
    const RECV_TIMEOUT: Duration = Duration::from_secs(5);
    let sync_topic = bsc_sandwich::pool::sync_topic0();
    let mut token0_cache: HashMap<Address, Address> = HashMap::new();

    loop {
        let pool_addrs_and_quote: Vec<(Address, Address)> =
            { app_state.pairbook.read().await.entries().map(|e| (e.pair_addr, e.quote)).collect() };
        if pool_addrs_and_quote.is_empty() {
            tokio::time::sleep(Duration::from_secs(10)).await;
            continue;
        }
        let quote_by_pair: HashMap<Address, Address> = pool_addrs_and_quote.iter().copied().collect();
        let addrs: Vec<Address> = pool_addrs_and_quote.iter().map(|(p, _)| *p).collect();

        let mut connected = false;
        for url in &ws_urls {
            let redacted = transport::redact_rpc_url(url);
            let provider = match transport::connect_and_verify(url).await {
                Ok(p) => p,
                Err(_) => continue,
            };
            let filter = Filter::new().address(addrs.clone()).event_signature(sync_topic);
            let mut sub = match provider.subscribe_logs(&filter).await {
                Ok(s) => s,
                Err(e) => {
                    app_state.logger.log(
                        "rpc.pending_unavailable",
                        serde_json::json!({ "transport": "sync_logs", "url": redacted, "reason": e.to_string() }),
                    );
                    continue;
                }
            };
            connected = true;
            app_state.logger.log(
                "sync.subscribed",
                serde_json::json!({ "url": redacted, "pools": addrs.len() }),
            );
            let deadline = Instant::now() + RESYNC_INTERVAL;
            'recv: loop {
                if Instant::now() >= deadline {
                    break 'recv; // dinh ky resubscribe de nhan pool MOI (neu pairs.txt doi)
                }
                match tokio::time::timeout(RECV_TIMEOUT, sub.recv()).await {
                    Ok(Ok(log)) => {
                        let pair_addr = log.address();
                        let Some(&quote) = quote_by_pair.get(&pair_addr) else { continue };
                        let Some(block) = log.block_number else { continue };
                        let Some((r0, r1)) = bsc_sandwich::pool::decode_sync_log_reserves(log.data().data.as_ref()) else { continue };
                        let token0 = match token0_cache.get(&pair_addr) {
                            Some(t) => *t,
                            None => match bsc_sandwich::pool::get_raw_reserves_and_token0(&provider, pair_addr).await {
                                Ok((t0, _, _)) => {
                                    token0_cache.insert(pair_addr, t0);
                                    t0
                                }
                                Err(_) => continue, // khong biet token0 -> khong the sap dung chieu, bo qua event nay
                            },
                        };
                        let (reserve_quote, reserve_token) = bsc_sandwich::pool::order_reserves_by_quote(token0, quote, r0, r1);
                        // Cum `bugfix-presign-and-contract-plan` (A1) - key cache
                        // PHAI kem `quote` (chinh quote cua entry PairBook dung de
                        // sap chieu ngay tren), neu khong 1 pool 2 chieu se de len
                        // nhau (xem doc-comment `transport::ReserveCache`).
                        app_state.reserve_cache.write().await.insert(pair_addr, quote, block, PoolReserves { reserve_wbnb: reserve_quote, reserve_token });
                    }
                    Ok(Err(_)) => break 'recv, // subscription roi - thu URL ke/resubscribe
                    Err(_) => continue,        // timeout doc, chi de kiem tra deadline
                }
            }
            break; // roi vong for URL, quay lai vong loop ngoai (doc lai PairBook, resubscribe)
        }
        if !connected {
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
}

/// Cụm `bugfix-presign-and-contract-plan` (A4) — task NỀN nạp 2 cache mà
/// đường ký shadow phụ thuộc, để đường ký đó **không gọi RPC nào**:
///
/// - `MinedTxIndex`: hash tx của block mới nhất (1 lời gọi
///   `eth_getBlockByNumber` KHÔNG-full mỗi block, ~1 lần/3 s) — thay cho
///   `eth_getTransactionReceipt(victim)` từng gọi trên đường ký.
/// - `SelfNonceCache`: `eth_getTransactionCount(self, pending)` mỗi block —
///   chỉ chạy khi `live_mode="shadow"` và đã load được ví.
///
/// Cả 2 đều đi qua **RPC NỀN** (`bg_provider`, cụm A5), không đụng kết nối
/// đường nóng. Poll `last_block` mỗi 300 ms (rẻ, chỉ đọc RwLock) và chỉ gọi
/// RPC khi số block ĐỔI.
async fn mined_and_nonce_prefetch_task(app_state: AppState) {
    let mut last_seen: u64 = 0;
    loop {
        tokio::time::sleep(Duration::from_millis(300)).await;
        let block = match *app_state.last_block.read().await {
            Some(b) => b,
            None => continue,
        };
        if block == last_seen {
            continue;
        }
        let Some((provider, _src)) = bg_provider_or_hot(&app_state).await else { continue };

        // (1) hash tx cua block moi nhat - KHONG lay full body (re hon nhieu).
        match provider.get_block_by_number(alloy::eips::BlockNumberOrTag::Number(block)).await {
            Ok(Some(b)) => {
                let hashes: std::collections::HashSet<alloy::primitives::B256> =
                    b.transactions.hashes().collect();
                let n = hashes.len();
                app_state.mined_index.write().await.insert_block(block, hashes);
                if last_seen == 0 {
                    app_state
                        .logger
                        .log("mined_index.ready", serde_json::json!({ "block": block, "tx_count": n }));
                }
                last_seen = block;
            }
            Ok(None) => continue, // block chua thay duoc tren node nay - thu lai tick sau
            Err(e) => {
                app_state
                    .logger
                    .log("mined_index.error", serde_json::json!({ "block": block, "reason": e.to_string() }));
                continue;
            }
        }

        // (2) nonce vi shadow (chi khi co vi).
        if let Some((self_addr, _)) = app_state.shadow_wallet.as_ref() {
            match provider.get_transaction_count(*self_addr).block_id(alloy::eips::BlockId::pending()).await {
                Ok(n) => app_state.self_nonce.write().await.insert(block, n),
                Err(e) => app_state
                    .logger
                    .log("nonce_prefetch.error", serde_json::json!({ "block": block, "reason": e.to_string() })),
            }
        }
    }
}

/// Cụm `bugfix-presign-and-contract-plan` (A3) — subscribe log `Transfer`
/// của WBNB+USDT có `topic1` (from) là 1 trong 3 địa chỉ SEED của cụm đối thủ
/// → ghi ví nhận (`topic2`) vào `ClusterIndex` tại đúng block đó.
///
/// Đây là cách DUY NHẤT nhận diện được các ví "burner" dùng-một-lần của cụm
/// (không thể liệt kê tĩnh — xem doc-comment `src/competitor.rs`). Dùng WS
/// filter theo `address` (2 token) + `topics` (Transfer + from ∈ seed) nên
/// node chỉ đẩy về đúng các log liên quan, không quét toàn chain.
async fn subscribe_competitor_funding(app_state: AppState, ws_urls: Vec<String>) {
    use alloy::rpc::types::eth::Filter;
    if ws_urls.is_empty() {
        return;
    }
    let seeds: Vec<alloy::primitives::B256> = bsc_sandwich::competitor::SEED_ADDRESSES
        .iter()
        .filter_map(|s| Address::from_str(s).ok())
        .map(|a| a.into_word())
        .collect();
    let quote_tokens = vec![venues::wbnb_addr(), venues::usdt_addr()];
    let topic0 = bsc_sandwich::competitor::transfer_topic0();
    loop {
        let mut connected = false;
        for url in &ws_urls {
            let redacted = transport::redact_rpc_url(url);
            let Ok(provider) = transport::connect_and_verify(url).await else { continue };
            let filter = Filter::new().address(quote_tokens.clone()).event_signature(topic0).topic1(seeds.clone());
            let mut sub = match provider.subscribe_logs(&filter).await {
                Ok(s) => s,
                Err(e) => {
                    app_state.logger.log(
                        "rpc.pending_unavailable",
                        serde_json::json!({ "transport": "competitor_funding", "url": redacted, "reason": e.to_string() }),
                    );
                    continue;
                }
            };
            connected = true;
            app_state.logger.log(
                "competitor.subscribed",
                serde_json::json!({ "url": redacted, "seeds": bsc_sandwich::competitor::SEED_ADDRESSES }),
            );
            loop {
                match tokio::time::timeout(Duration::from_secs(120), sub.recv()).await {
                    Ok(Ok(log)) => {
                        let (Some(block), Some(to_topic)) = (log.block_number, log.topics().get(2).copied()) else { continue };
                        let wallet = bsc_sandwich::competitor::address_from_topic(to_topic);
                        app_state.competitor_cluster.write().await.note_funded(block, wallet);
                        app_state.logger.log(
                            "competitor.funded",
                            serde_json::json!({
                                "block": block,
                                "wallet": format!("{wallet:#x}"),
                                "quote_token": format!("{:#x}", log.address()),
                                "from_seed": format!("{:#x}", log.topics().get(1).map(|t| bsc_sandwich::competitor::address_from_topic(*t)).unwrap_or_default()),
                            }),
                        );
                    }
                    Ok(Err(_)) => break,   // subscription roi - thu URL ke
                    Err(_) => continue,    // timeout doc (cum im lang la binh thuong)
                }
            }
            break;
        }
        if !connected {
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }
}

/// Cụm `5.3` — thử lần lượt từng URL trong `ws_urls` (log `rpc.failover` cho
/// URL lỗi connect/`subscribe_blocks`) tới khi 1 URL thành công; ở lại vòng
/// nhận block header tới khi lỗi/rớt (best-effort, KHÔNG tự động nhảy sang
/// URL khác giữa chừng — khác `subscribe_pending_txs` nơi pending-tx quan
/// trọng hơn). Hết `ws_urls` (rỗng hoặc mọi URL đều lỗi) -> log `rpc.skip`,
/// KHÔNG halt bot (CLAUDE.md: "Không halt vì WSS im (ship)") — `last_block`
/// vẫn có giá trị từ HTTP `get_block_number` lúc boot/health-check.
async fn subscribe_ws_heads(app_state: AppState, ws_urls: Vec<String>) {
    for url in &ws_urls {
        let redacted = transport::redact_rpc_url(url);
        let provider = match transport::connect_and_verify(url).await {
            Ok(p) => p,
            Err(e) => {
                app_state.logger.log(
                    "rpc.failover",
                    serde_json::json!({ "transport": "ws_heads", "url": redacted, "reason": e.to_string() }),
                );
                continue;
            }
        };
        let mut sub = match provider.subscribe_blocks().await {
            Ok(s) => s,
            Err(e) => {
                app_state.logger.log(
                    "rpc.failover",
                    serde_json::json!({ "transport": "ws_heads", "url": redacted, "reason": format!("subscribe_blocks that bai: {e}") }),
                );
                continue;
            }
        };
        app_state
            .logger
            .log("rpc.connect", serde_json::json!({ "transport": "ws_heads", "url": redacted }));

        loop {
            match sub.recv().await {
                Ok(header) => {
                    let block_number = header.number;
                    *app_state.last_block.write().await = Some(block_number);
                    app_state.logger.log("rpc.block", serde_json::json!({ "block": block_number }));
                }
                Err(e) => {
                    app_state.logger.log(
                        "rpc.skip",
                        serde_json::json!({ "transport": "ws_heads", "url": redacted, "reason": format!("subscription rot: {e}") }),
                    );
                    return;
                }
            }
        }
    }
    if !ws_urls.is_empty() {
        app_state.logger.log(
            "rpc.skip",
            serde_json::json!({ "transport": "ws_heads", "reason": "khong URL WSS nao trong danh sach connect/subscribe_blocks duoc" }),
        );
    }
}

/// Cụm `5.3` (mở rộng `5.2`) — orchestrate fallback: "WSS -> WSS KHÁC trong
/// danh sách -> `txpool_content` (HTTP pool)". Thử LẦN LƯỢT từng URL trong
/// `ws_urls`: connect lỗi -> log `rpc.failover`, thử URL kế; connect được
/// nhưng `subscribe_full_pending_transactions` lỗi (`PubsubUnavailable`...)
/// -> log `rpc.pending_unavailable`, thử URL kế; subscribe thành công ->
/// set `pending_source=Ws`, ở lại vòng `recv()` (buffer lớn qua
/// `channel_size(PENDING_WS_CHANNEL_SIZE)`, xem `transport.rs`) tới khi lỗi/
/// rớt (`channel lagged`...) -> log `rpc.pending_unavailable`, THỬ URL WSS KẾ
/// (khác `5.2`: trước đây rớt là rơi thẳng xuống txpool, giờ còn URL WSS nào
/// chưa thử thì thử tiếp trước). Hết TOÀN BỘ `ws_urls` mới rơi xuống
/// `poll_txpool_pending` (HTTP qua `http_pool`, tự failover URL kế khi lỗi).
/// Không nhánh nào halt bot (CLAUDE.md) — thất bại hết thì `pending_source`
/// giữ nguyên `InjectOnly`, bot vẫn nhận `state/inject_tx.jsonl`.
async fn subscribe_pending_txs(app_state: AppState, ws_urls: Vec<String>, http_pool: Arc<transport::RpcPool>, poll_interval: Duration) {
    for url in &ws_urls {
        let redacted = transport::redact_rpc_url(url);
        let provider = match transport::connect_and_verify(url).await {
            Ok(p) => p,
            Err(e) => {
                app_state.logger.log(
                    "rpc.failover",
                    serde_json::json!({ "transport": "ws", "url": redacted, "reason": e.to_string() }),
                );
                continue; // thu URL WSS ke trong danh sach
            }
        };
        let sub = provider
            .subscribe_full_pending_transactions()
            .channel_size(transport::PENDING_WS_CHANNEL_SIZE)
            .await;
        let mut sub = match sub {
            Ok(s) => s,
            Err(e) => {
                app_state.logger.log(
                    "rpc.pending_unavailable",
                    serde_json::json!({ "transport": "ws", "url": redacted, "reason": e.to_string() }),
                );
                continue; // thu URL WSS ke
            }
        };
        *app_state.pending_source.write().await = transport::PendingSource::Ws;
        app_state
            .logger
            .log("rpc.pending_subscribed", serde_json::json!({ "transport": "ws", "url": redacted }));
        loop {
            match sub.recv().await {
                Ok(tx) => {
                    let raw = transport::pending_tx_from_rpc(&tx);
                    app_state.funnel.record_seen();
                    // Cum A4 - gate (a) giong het poll_txpool_pending (xem doc-comment
                    // o do): `to` khong nam trong 5 router Pancake da pin -> khong
                    // log tx.seen tung dong, khong spawn, chi dem qua funnel.
                    if !passes_router_gate(raw.to, "pending_ws") {
                        app_state.funnel.record_not_pancake_router();
                    } else if app_state.state_files.is_halted() {
                        // Cum `exec-path-traps` (V-06) - halt.lock ton tai -
                        // KHONG spawn handle_paper_tx (paper loop dung THAT,
                        // khong chi hien thi tren dashboard). halt_watch_task
                        // lo viec log chuyen trang thai/cap nhat bot_state.
                    } else if !dedup_allows_processing(&app_state, raw.hash).await {
                        // Cum `econ-truth-latency-vps` (muc 1) - tx nay DA
                        // duoc xu ly qua nguon khac (vd truoc do qua WS, gio
                        // fallback txpool lay lai cung hash con trong mempool)
                        // - khong spawn lai, tranh dem trung funnel/log 2 lan
                        // sim.result cho cung 1 victim.
                    } else {
                        log_tx_seen(&app_state.logger, "pending_ws", &raw);
                        tokio::spawn(handle_paper_tx(app_state.clone(), raw));
                    }
                }
                Err(e) => {
                    app_state.logger.log(
                        "rpc.pending_unavailable",
                        serde_json::json!({ "transport": "ws", "url": redacted, "reason": format!("subscription rot: {e}") }),
                    );
                    break; // thu URL WSS ke trong danh sach (vong for ben ngoai)
                }
            }
        }
    }

    // Het danh sach WSS (rong, hoac moi URL deu fail/rot) -> fallback poll txpool_content.
    {
        let mut src = app_state.pending_source.write().await;
        if *src == transport::PendingSource::Ws {
            *src = transport::PendingSource::InjectOnly;
        }
    }
    poll_txpool_pending(app_state, http_pool, poll_interval).await;
}

/// Cụm `5.2` — kết quả tối thiểu của `txpool_content` (namespace `txpool`
/// chuẩn Geth, xem https://geth.ethereum.org/docs/rpc/ns-txpool#txpool_content)
/// chỉ đọc field `pending` (bỏ `queued` — tx chưa thể thực thi ngay do
/// nonce-gap, không phải candidate sandwich tức thời). Tự định nghĩa struct
/// này thay vì dùng `alloy::providers::ext::TxPoolApi`/`alloy-rpc-types-txpool`
/// vì bản `alloy-provider 2.4.2` đang pin (`docs/STATE.md`) khai lệch version
/// (`alloy-rpc-types-txpool = "2.4.2"`) — crate đó CHƯA có bản `2.4.2` trên
/// crates.io (chỉ tới `2.4.1`), bật feature `rpc-types-txpool` làm
/// `cargo build` fail resolve dependency ngay (đã verify lỗi thật, xem
/// docs/STATE.md mục "5.2"). Dùng thẳng `Provider::raw_request` (đã có sẵn
/// trong feature `provider-http` đang bật, không cần thêm feature nào) +
/// kiểu tối giản tự viết để tránh phụ thuộc crate bị lệch version đó.
#[derive(Debug, Default, serde::Deserialize)]
struct TxpoolContentPendingOnly {
    #[serde(default)]
    pending: std::collections::BTreeMap<String, std::collections::BTreeMap<String, alloy::rpc::types::eth::Transaction>>,
}

/// Cụm `5.2`+`5.3` — fallback HTTP: poll `txpool_content` mỗi `pending_poll_ms`
/// (config, ship `400`) qua `http_pool` (chain 56 đã xác nhận khi connect —
/// "Sai chain ≠ 56 không watch" tự động đúng). Dùng `txpool_content` (không
/// phải `txpool_inspect`) vì cần ĐỦ `input` calldata để
/// `pipeline::decode_and_prefilter` giải mã — `txpool_inspect` chỉ trả tóm
/// tắt dạng chuỗi, không có calldata.
///
/// KHÁC `5.2`: lỗi lần gọi (node không hỗ trợ namespace `txpool`, rate-limit,
/// timeout...) KHÔNG còn dừng task hẳn — log `rpc.pending_unavailable` rồi
/// `http_pool.advance_and_reconnect` sang URL HTTP KẾ trong pool (quay vòng,
/// cập nhật `app_state.provider`), thử lại ở vòng poll SAU. Chỉ khi TOÀN BỘ
/// pool không còn URL nào connect được (`advance_and_reconnect` trả `None`)
/// mới coi là hết đường — vẫn KHÔNG dừng task (health-check task riêng có
/// thể hồi phục pool sau), chỉ log rõ lý do — đúng "Không halt vì 1 node
/// chết" (CLAUDE.md `5.3`).
///
/// `pending_txpool_max_per_poll` (config, ship `32`) giới hạn số hash MỚI
/// (chưa `seen`) được xử lý mỗi vòng — `txpool_content` trả TOÀN BỘ pool
/// đang chờ mỗi lần gọi, không giới hạn sẽ spawn hàng trăm/ngàn
/// `handle_paper_tx` cùng lúc khi mempool đông (BAOCAO08:
/// `decode_fail=856` trong ~10s). Hash bị bỏ qua vì vượt cap KHÔNG được
/// đánh dấu `seen` — vẫn còn cơ hội được xử lý ở vòng poll sau nếu tx đó còn
/// trong pool. `seen` (dedup theo tx hash) chặn xử lý lặp lại CÙNG 1 tx qua
/// nhiều lần poll — cap kích thước để không phình vô hạn qua thời gian dài
/// chạy, chấp nhận đánh đổi hiếm khi xử lý lại 1 tx cũ ngay sau khi cap bị
/// xoá (KHÔNG sai logic, chỉ tốn thêm 1 lần `eth_call` hiếm gặp).
async fn poll_txpool_pending(app_state: AppState, http_pool: Arc<transport::RpcPool>, poll_interval: Duration) {
    use alloy::network::TransactionResponse;

    // Cum A4 - nang SEEN_CAP 5_000 -> 50_000 + doi tu "clear sach khi day"
    // sang "xoa theo tuoi" (VecDeque giu THU TU chen + HashSet tra cuu O(1)):
    // clear sach cu lam mat dau vet MOI tx da xu ly gan day (kha nang xu ly
    // lai tx cu tang dot bien ngay sau khi clear), xoa dan tu dau (tx CU
    // NHAT) it gay xu ly lap hon voi cung 1 dung luong bo nho.
    const SEEN_CAP: usize = 50_000;
    let mut seen_set: HashSet<alloy::primitives::TxHash> = HashSet::new();
    let mut seen_order: VecDeque<alloy::primitives::TxHash> = VecDeque::new();
    let mut interval = tokio::time::interval(poll_interval);
    loop {
        interval.tick().await;
        let provider = match http_pool.current().await {
            Some(p) => p,
            None => continue, // chua co URL HTTP nao trong pool connect duoc - cho health-check hoi phuc
        };
        let content: TxpoolContentPendingOnly =
            match provider.raw_request("txpool_content".into(), alloy::rpc::client::NoParams::default()).await {
                Ok(c) => c,
                Err(e) => {
                    app_state.logger.log(
                        "rpc.pending_unavailable",
                        serde_json::json!({ "transport": "txpool_content", "reason": e.to_string() }),
                    );
                    match http_pool.advance_and_reconnect(&app_state.logger, "http").await {
                        Some(new_provider) => {
                            *app_state.provider.write().await = Some(new_provider);
                        }
                        None => {
                            *app_state.provider.write().await = None;
                            app_state.logger.log(
                                "rpc.pending_unavailable",
                                serde_json::json!({ "transport": "txpool_content", "reason": "toan bo pool HTTP khong URL nao connect duoc" }),
                            );
                        }
                    }
                    continue; // thu lai o vong poll sau, khong dung han task
                }
            };
        {
            let mut src = app_state.pending_source.write().await;
            if *src != transport::PendingSource::Txpool {
                *src = transport::PendingSource::Txpool;
                app_state.logger.log("rpc.pending_subscribed", serde_json::json!({ "transport": "txpool_content" }));
            }
        }
        let max_new = app_state.config.read().await.pending_txpool_max_per_poll as usize;
        let mut new_count = 0usize;
        'outer: for by_nonce in content.pending.values() {
            for tx in by_nonce.values() {
                let hash = tx.tx_hash();
                if seen_set.contains(&hash) {
                    continue;
                }
                seen_set.insert(hash);
                seen_order.push_back(hash);
                if seen_order.len() > SEEN_CAP {
                    if let Some(oldest) = seen_order.pop_front() {
                        seen_set.remove(&oldest);
                    }
                }
                let raw = transport::pending_tx_from_rpc(tx);
                app_state.funnel.record_seen();

                // Cum A4 - gate (a) THUAN, 0 RPC, chay TRUOC decode: `to` khong
                // nam trong 5 router Pancake da pin -> khong dang de spawn task
                // nao (KHONG log tx.seen tung dong, chi dem qua funnel) - cap
                // `pending_txpool_max_per_poll` CHI ap dung cho tx da qua loc
                // nay (khong bi tieu ton boi rac khong lien quan Pancake).
                if !passes_router_gate(raw.to, "txpool") {
                    app_state.funnel.record_not_pancake_router();
                    continue;
                }
                if new_count >= max_new {
                    break 'outer;
                }
                new_count += 1;
                // Cum `exec-path-traps` (V-06) - halt.lock ton tai -> khong
                // spawn handle_paper_tx (van dem "seen"/qua gate router o
                // tren, chi dung LAI truoc buoc xu ly paper that).
                if app_state.state_files.is_halted() {
                    continue;
                }
                // Cum `econ-truth-latency-vps` (muc 1) - dedup CHUNG voi
                // nguon WS: tx nay co the DA duoc xu ly qua WS truoc do (WS
                // van song, hoac vua fallback xuong day) - `seen_set` CUC BO
                // o tren chi chan trung giua CAC LAN POLL txpool voi nhau,
                // KHONG biet gi ve WS - can lop thu 2 nay de dong khoang ho
                // xuyen-nguon (nghi van gop phan lech funnel.simulated vs
                // sim.result, BAOCAO39).
                if !dedup_allows_processing(&app_state, raw.hash).await {
                    continue;
                }
                log_tx_seen(&app_state.logger, "txpool", &raw);
                tokio::spawn(handle_paper_tx(app_state.clone(), raw));
            }
        }
    }
}

/// Cụm `5.3` — health-check định kỳ (mỗi `interval`, KHÔNG phải ngưỡng chủ
/// chỉnh trong `config.toml` — cadence nội bộ, cùng quy ước hằng số
/// `watch_inject_file`/`watch_tax_inject_file` 2s) cho provider HTTP hiện tại
/// trong `http_pool`: `get_block_number()` lỗi (node chết/timeout) hoặc chưa
/// có provider nào -> `connect()`/`advance_and_reconnect()` sang URL kế
/// trong pool (quay vòng), cập nhật `app_state.provider`/`last_block`. Bảo vệ
/// MỌI nơi đọc `app_state.provider` (bao gồm `pipeline::resolve_v2_reserves`
/// qua `handle_paper_tx`) mà KHÔNG cần đổi chữ ký `pipeline.rs` (giữ tách lõi
/// thuần/RPC thật đúng quy ước `docs/STATE.md`) — đây là điểm DUY NHẤT trong
/// cụm này chạm tới `eth_call getBlock` cho mọi consumer chung.
async fn http_pool_health_check(app_state: AppState, http_pool: Arc<transport::RpcPool>, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        let current = http_pool.current().await;
        let alive_block = match &current {
            Some(p) => p.get_block_number().await.ok(),
            None => None,
        };
        if let Some(bn) = alive_block {
            *app_state.last_block.write().await = Some(bn);
            continue;
        }
        // Chua co provider (current=None) -> connect() tu idx hien tai. Da co
        // provider nhung get_block_number loi (chet/timeout) -> advance_and_reconnect
        // sang URL KE (khac connect() se thu lai chinh URL vua chet truoc).
        let reconnected = if current.is_some() {
            http_pool.advance_and_reconnect(&app_state.logger, "http").await
        } else {
            http_pool.connect(&app_state.logger, "http").await
        };
        match reconnected {
            Some(new_provider) => {
                if let Ok(bn) = new_provider.get_block_number().await {
                    *app_state.last_block.write().await = Some(bn);
                }
                *app_state.provider.write().await = Some(new_provider);
            }
            None => {
                *app_state.provider.write().await = None;
            }
        }
    }
}

/// Cụm `econ-truth-latency-vps` (0.d) — health-check RIÊNG cho `sim_http_pool`
/// (`BSC_HTTP_SIM`/fallback `BSC_HTTP`), giữ `app_state.sim_provider` sống —
/// KHÔNG dùng chung `app_state.provider` (đường nóng) vì mục đích khác nhau
/// (revm fork cần state đầy đủ, một số node public/riêng KHÔNG hỗ trợ đủ
/// method cho việc đó, xem `pairs_vet_task`/`gas_units_boot_task`). Không đọc/
/// ghi `app_state.last_block` (đã có `http_pool_health_check` lo việc đó).
async fn sim_pool_health_check(app_state: AppState, sim_http_pool: Arc<transport::RpcPool>, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        let current = sim_http_pool.current().await;
        let alive = match &current {
            Some(p) => p.get_block_number().await.is_ok(),
            None => false,
        };
        if alive {
            continue;
        }
        let reconnected = if current.is_some() {
            sim_http_pool.advance_and_reconnect(&app_state.logger, "sim").await
        } else {
            sim_http_pool.connect(&app_state.logger, "sim").await
        };
        *app_state.sim_provider.write().await = reconnected;
    }
}

/// Cụm `bugfix-presign-and-contract-plan` (A5) — health-check cho pool RPC
/// **NỀN** (`BSC_HTTP_BG`, mặc định 3 URL cuối của `BSC_HTTP`): mọi việc nền
/// (`spawn_shadow_sign_task`, `spawn_post_simulated_tracker`/`compete.check`,
/// `spawn_victim_validator`) dùng `app_state.bg_provider` thay vì
/// `app_state.provider` — đường nóng `handle_paper_tx` giữ riêng kết nối đầu
/// danh sách. Cùng khuôn `sim_pool_health_check` (không đụng `last_block`).
async fn bg_pool_health_check(app_state: AppState, bg_http_pool: Arc<transport::RpcPool>, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        let current = bg_http_pool.current().await;
        let alive = match &current {
            Some(p) => p.get_block_number().await.is_ok(),
            None => false,
        };
        if alive {
            continue;
        }
        let reconnected = if current.is_some() {
            bg_http_pool.advance_and_reconnect(&app_state.logger, "bg").await
        } else {
            bg_http_pool.connect(&app_state.logger, "bg").await
        };
        *app_state.bg_provider.write().await = reconnected;
    }
}

/// Cụm `bugfix-presign-and-contract-plan` (A5) — provider cho việc NỀN:
/// `bg_provider` nếu có, nếu chưa kết nối được thì rơi về `provider` (đường
/// nóng) để KHÔNG mất hẳn chức năng nền — trả kèm nhãn nguồn để log nói THẬT
/// đang dùng cái nào (không giả vờ đã tách).
async fn bg_provider_or_hot(app_state: &AppState) -> Option<(alloy::providers::DynProvider, &'static str)> {
    if let Some(p) = app_state.bg_provider.read().await.clone() {
        return Some((p, "bg"));
    }
    app_state.provider.read().await.clone().map(|p| (p, "hot_fallback"))
}

/// Cụm `5.1` — đọc `state/inject_tx.jsonl` (mỗi dòng `from,value_wei,input_hex`)
/// để bơm tx giả lập vào ĐÚNG cùng `handle_paper_tx` như pending thật, dùng
/// khi node không đẩy pending (không có `BSC_WS`, giống máy phiên này) hoặc
/// để test theo ý chủ. Poll 2s/lần (không phải ngưỡng chủ chỉnh trong
/// `config.toml` — đây là cadence đọc file test nội bộ, không phải
/// `min_profit_bnb`/`max_front_bnb`/... nên không cần hot-reload qua
/// `config_reload_sec`). Chỉ đọc dòng MỚI (theo số dòng đã xử lý lần trước)
/// — file bị ghi đè ngắn hơn (chủ tạo lại file test) thì đọc lại từ đầu,
/// không panic, không bỏ sót dòng.
async fn watch_inject_file(app_state: AppState) {
    let path = PathBuf::from("state").join("inject_tx.jsonl");
    let mut last_len: usize = 0;
    let mut interval = tokio::time::interval(Duration::from_secs(2));
    loop {
        interval.tick().await;
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(_) => continue, // file chua ton tai - thu lai lan sau, khong panic
        };
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() < last_len {
            last_len = 0; // file bi ghi de ngan hon -> doc lai tu dau
        }
        for line in &lines[last_len..] {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match transport::parse_inject_line(line) {
                Ok(raw) => {
                    app_state.funnel.record_seen();
                    // Cum `exec-path-traps` (V-06) - halt.lock ton tai ->
                    // khong spawn handle_paper_tx (van dem "seen" cho tx da
                    // parse duoc, chi dung LAI truoc buoc xu ly paper that).
                    if app_state.state_files.is_halted() {
                        continue;
                    }
                    log_tx_seen(&app_state.logger, "inject", &raw);
                    tokio::spawn(handle_paper_tx(app_state.clone(), raw));
                }
                Err(e) => {
                    app_state.logger.log(
                        "inject.parse_error",
                        serde_json::json!({ "error": e, "line": line }),
                    );
                }
            }
        }
        last_len = lines.len();
    }
}

/// Cụm tax-cache-inject — đọc `state/tax_inject.jsonl` (mỗi dòng
/// `token,buy_bps,sell_bps`) để chủ/test điền `tax_cache` lúc bot đang chạy,
/// KHÔNG cần sửa code/build lại. `cfg.allow_tax_inject=false` -> dòng bị BỎ
/// QUA (log `tax.inject_skipped`, không ghi cache) — giữ nguyên luật "chưa đo
/// thì `honeypot_or_tax`" khi chủ tắt cờ này. Đọc `allow_tax_inject` MỖI dòng
/// (không cache 1 lần đầu vòng lặp) vì field này hot-reload qua
/// `config_reload_sec`, có thể đổi giữa 2 lần poll 2s. Cùng khuôn
/// `watch_inject_file` ở trên (poll 2s, chỉ đọc dòng MỚI, file ngắn hơn thì
/// đọc lại từ đầu, không panic).
async fn watch_tax_inject_file(app_state: AppState) {
    let path = PathBuf::from("state").join("tax_inject.jsonl");
    let mut last_len: usize = 0;
    let mut interval = tokio::time::interval(Duration::from_secs(2));
    loop {
        interval.tick().await;
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(_) => continue, // file chua ton tai - thu lai lan sau, khong panic
        };
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() < last_len {
            last_len = 0; // file bi ghi de ngan hon -> doc lai tu dau
        }
        for line in &lines[last_len..] {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match tax::parse_tax_inject_line(line) {
                Ok((token, buy_bps, sell_bps)) => {
                    let allow = app_state.config.read().await.allow_tax_inject;
                    if !allow {
                        app_state.logger.log(
                            "tax.inject_skipped",
                            serde_json::json!({ "reason": "allow_tax_inject=false", "token": format!("{:#x}", token) }),
                        );
                        continue;
                    }
                    let current_block = app_state.last_block.read().await.unwrap_or(0);
                    {
                        let mut cache = app_state.tax_cache.write().await;
                        cache.inject_from_buy_sell_bps(token, buy_bps, sell_bps, current_block);
                    }
                    app_state.logger.log(
                        "tax.inject",
                        serde_json::json!({
                            "token": format!("{:#x}", token),
                            "buy_bps": buy_bps,
                            "sell_bps": sell_bps,
                            "measured_at_block": current_block,
                            "source": "file",
                        }),
                    );
                }
                Err(e) => {
                    app_state.logger.log(
                        "tax.inject_parse_error",
                        serde_json::json!({ "error": e, "line": line }),
                    );
                }
            }
        }
        last_len = lines.len();
    }
}

/// Cụm `foundation-fix-then-real-sim` (A6) — bucket TERMINAL cho 1
/// `PipelineOutcome` cuối cùng (dù đến từ nhánh WBNB V2 hay nhánh USDT
/// fallback) vào ĐÚNG 1 field của `web::FunnelCounters` (không tính
/// `venue_v2`/`venue_v3`/`not_pancake_router`/`seen` — 4 field đó được ghi
/// TRỰC TIẾP tại điểm biết được, xem `handle_paper_tx`/3 hàm nguồn tx).
/// `NotQuotePair`/`SellDirection` (nhánh USDT-aware) gộp chung bucket
/// `not_wbnb_pair` — không tách riêng phiên này (cùng ý nghĩa "sai hướng/sai
/// cặp quote"). `NotInList` (lọc wallet/pairs.txt, không thuộc chuỗi
/// decode->venue->thanh_khoan->tax->sim) KHÔNG có bucket riêng trong danh
/// sách lệnh gốc — vẫn thấy đủ qua `/api/skips` (không đổi), chỉ không tính
/// vào funnel mới này (ghi rõ, không bịa bucket ngoài danh sách lệnh).
fn record_funnel_terminal(funnel: &bsc_sandwich::web::FunnelCounters, outcome: &PipelineOutcome) {
    match outcome {
        PipelineOutcome::Simulated(_) => funnel.record_simulated(),
        PipelineOutcome::Skip(reason) => match reason {
            pipeline::PipelineSkip::DecodeFail => funnel.record_decode_fail(),
            pipeline::PipelineSkip::NotWbnbPair | pipeline::PipelineSkip::NotQuotePair | pipeline::PipelineSkip::SellDirection => {
                funnel.record_not_wbnb_pair()
            }
            pipeline::PipelineSkip::VenueUnpinned => funnel.record_venue_v3(),
            pipeline::PipelineSkip::NoPool => funnel.record_no_pool(),
            // Cum `hotpath-fix-then-decoder-ur` (A3)
            pipeline::PipelineSkip::RpcError => funnel.record_rpc_error(),
            pipeline::PipelineSkip::BelowMin => funnel.record_below_min(),
            pipeline::PipelineSkip::ThinLiq => funnel.record_thin_liq(),
            pipeline::PipelineSkip::HoneypotOrTax => funnel.record_honeypot_or_tax(),
            // Cum `evm-validate-fixed-then-wire` (B3.2)
            pipeline::PipelineSkip::SimError => funnel.record_sim_error(),
            pipeline::PipelineSkip::Unprofitable => funnel.record_unprofitable(),
            pipeline::PipelineSkip::VictimWouldRevert => funnel.record_victim_would_revert(),
            // Cum `exec-path-traps` (F-14/F-13)
            pipeline::PipelineSkip::Deadline => funnel.record_deadline(),
            pipeline::PipelineSkip::NonceStale => funnel.record_nonce_stale(),
            pipeline::PipelineSkip::NonceFuture => funnel.record_nonce_future(),
            // Cum `real-economics-mode2` (F-03)
            pipeline::PipelineSkip::GasCap => funnel.record_gas_cap(),
            pipeline::PipelineSkip::SanityReject => funnel.record_sanity_reject(),
            // A3 - da dem rieng o handle_paper_tx luc GHI DE outcome (truoc khi
            // goi ham nay), khong dem lai lan 2.
            pipeline::PipelineSkip::CompetitorVictim => {}
            // NotInList/NotPancakeRouter khong roi vao day (NotPancakeRouter
            // bi chan truoc khi co PipelineOutcome nao duoc tao; NotInList
            // khong co bucket rieng trong danh sach lenh goc A6).
            pipeline::PipelineSkip::NotInList | pipeline::PipelineSkip::NotPancakeRouter => {}
        },
    }
}

/// Cụm `strategy-lock-mode2` — task nền VET (revm THẬT, ĐỘC LẬP với đường
/// nóng `handle_paper_tx` — không giữ khoá/không chặn bất kỳ tx nào). Chạy 1
/// vòng NGAY lúc gọi (lúc boot) rồi lặp lại mỗi `pairs_vet_interval_sec`: với
/// MỌI entry `pairs.txt` đã có `vetted` (`PairBook::tokens_to_vet`), gọi
/// `sim_evm::measure_tax_evm` TUẦN TỰ (sleep 200ms giữa 2 token — tránh dồn
/// RPC vào 1 nhịp), ghi kết quả vào `PairBook::set_vet_result` (tax > ngưỡng
/// `max_roundtrip_tax` HOẶC honeypot -> loại khỏi candidate ngay, log
/// `pair.vet_fail`, KHÔNG đụng `pairs.txt`) + đè `state/pairs_vetted.json`
/// (mảng đầy đủ, dùng cho Chủ/Grok đọc nhanh không cần đọc `logs/bot.jsonl`).
/// Bỏ qua cả vòng nếu chưa có provider HTTP hoặc chưa có `last_block` (boot
/// chưa xong) — thử lại ở vòng kế tiếp, không panic/không giả số block.
async fn pairs_vet_task(app_state: AppState, sim_http_pool: Arc<transport::RpcPool>) {
    loop {
        let (interval_sec, max_tax_bps) = {
            let cfg = app_state.config.read().await;
            (cfg.pairs_vet_interval_sec.max(1), cfg.max_roundtrip_tax_bps())
        };

        // Cum `econ-truth-latency-vps` (0.d) - dung provider RIENG
        // (sim_provider, tu BSC_HTTP_SIM/fallback BSC_HTTP) cho revm fork,
        // KHONG dung chung provider duong nong.
        let provider_opt = app_state.sim_provider.read().await.clone();
        let current_block = app_state.last_block.read().await.unwrap_or(0);
        if let (Some(provider), true) = (provider_opt, current_block > 0) {
            // Cum `hotpath-fix-then-decoder-ur` (A1) - tokens_to_vet gio tra
            // THEM quote THAT cua tung entry (truoc day hardcode WBNB cho MOI
            // token, khien 7 pool USDT trong pairs.txt bi do sai pool/loi).
            let all_targets = app_state.pairbook.read().await.tokens_to_vet();
            // Cum `bugfix-presign-and-contract-plan` (A4) - CHU KY VET KHAC
            // NHAU theo do "nong" cua pool: pool vua co candidate di toi buoc
            // sim trong HOT_CANDIDATE_WINDOW_SEC gan day duoc vet lai moi
            // HOT_VET_INTERVAL_SEC (300s), cac pool con lai giu
            // `pairs_vet_interval_sec` (600s ship). Ly do: ket qua vet TUOI la
            // dieu kien (a) cua duong ky shadow (`pre_sign_revet_fast`) -
            // pool dang co co hoi ma vet qua han se bi abort `vet_stale`.
            const HOT_VET_INTERVAL_SEC: u64 = 300;
            const HOT_CANDIDATE_WINDOW_SEC: u64 = 900;
            let hot: std::collections::HashSet<Address> = {
                let seen = app_state.candidate_seen.read().await;
                seen.iter()
                    .filter(|(_, t)| t.elapsed().as_secs() <= HOT_CANDIDATE_WINDOW_SEC)
                    .map(|(a, _)| *a)
                    .collect()
            };
            let targets: Vec<_> = {
                let book = app_state.pairbook.read().await;
                all_targets
                    .into_iter()
                    .filter(|(pair_addr, _, _)| {
                        let due_after = if hot.contains(pair_addr) { HOT_VET_INTERVAL_SEC.min(interval_sec) } else { interval_sec };
                        match book.vet_result(pair_addr) {
                            None => true, // chua vet lan nao -> vet ngay
                            Some((_, age_sec)) => age_sec >= due_after,
                        }
                    })
                    .collect()
            };
            if !targets.is_empty() {
                app_state.logger.log(
                    "pair.vet_cycle",
                    serde_json::json!({ "due": targets.len(), "hot_pools": hot.len(), "hot_interval_sec": HOT_VET_INTERVAL_SEC, "base_interval_sec": interval_sec }),
                );
            }
            let mut results = Vec::with_capacity(targets.len());
            for (pair_addr, token, quote) in targets {
                let quote_asset =
                    if quote == venues::usdt_addr() { pipeline::QuoteAsset::Usdt } else { pipeline::QuoteAsset::Wbnb };
                let probe_in = probe_in_for_quote(quote_asset);
                match bsc_sandwich::sim_evm::measure_tax_evm(provider.clone(), current_block, token, quote, probe_in)
                    .await
                {
                    Ok(m) => {
                        let bps = tax::combine_roundtrip_bps(m.buy_bps, m.sell_bps);
                        let ok = !m.honeypot && bps <= max_tax_bps;
                        if !ok {
                            app_state.logger.log(
                                "pair.vet_fail",
                                serde_json::json!({
                                    "pair": format!("{pair_addr:#x}"),
                                    "token": format!("{token:#x}"),
                                    "buy_bps": m.buy_bps,
                                    "sell_bps": m.sell_bps,
                                    "honeypot": m.honeypot,
                                    "block": current_block,
                                }),
                            );
                        }
                        app_state.pairbook.write().await.set_vet_result(
                            pair_addr,
                            bsc_sandwich::pairbook::VetResult {
                                buy_bps: m.buy_bps,
                                sell_bps: m.sell_bps,
                                honeypot: m.honeypot,
                                block: current_block,
                            },
                            ok,
                        );
                        results.push(serde_json::json!({
                            "pair": format!("{pair_addr:#x}"),
                            "token": format!("{token:#x}"),
                            "quote": format!("{quote:#x}"),
                            "buy_bps": m.buy_bps,
                            "sell_bps": m.sell_bps,
                            "honeypot": m.honeypot,
                            "block": current_block,
                            "ts": chrono::Utc::now().to_rfc3339(),
                        }));
                    }
                    Err(e) => {
                        let err_str = e.to_string();
                        app_state.logger.log(
                            "pair.vet_error",
                            serde_json::json!({
                                "pair": format!("{pair_addr:#x}"),
                                "token": format!("{token:#x}"),
                                "error": err_str,
                            }),
                        );
                        // Cum 0.c/0.d - loi "method khong ho tro" (quan sat
                        // that: bloXroute "-32000 not supported" khi revm fork
                        // can eth_getStorageAt/state day du) -> danh dau URL
                        // sim hien tai, chuyen URL ke NGAY (khong cho het het
                        // 90+ token cung loi tren cung 1 URL hong).
                        if transport::is_unsupported_method_error(&err_str) {
                            if let Some(new_provider) =
                                sim_http_pool.mark_current_unsupported(&app_state.logger, "sim").await
                            {
                                *app_state.sim_provider.write().await = Some(new_provider);
                            }
                        }
                    }
                }
                // Cum `econ-truth-latency-vps` (0.e) - 300ms/token (tang tu
                // 200ms), giu dung "1 sim dong thoi, tuan tu" (vong for tren
                // KHONG spawn song song, moi lan lap doi 1 lan `.await` het),
                // giam ap luc RPC tren pool sim khi pairs.txt co hang tram dong.
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
            if !results.is_empty() {
                let _ = tokio::fs::create_dir_all("state").await;
                let _ = tokio::fs::write(
                    "state/pairs_vetted.json",
                    serde_json::to_string_pretty(&results).unwrap_or_default(),
                )
                .await;
            }
        }

        // Nhip quet NGAN (30s) - viec vet that su co dien ra hay khong do
        // dieu kien `due_after` o tren quyet dinh, khong con do nhip ngu nay.
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

/// Cụm `real-economics-mode2` (F-03) — đo gas UNIT thật (KHÔNG phải wei) 1
/// LẦN lúc boot bằng revm, trên 1 pair đã vet trong `pairs.txt`
/// (`PairBook::tokens_to_vet` — chỉ entry `resolved_from=Token` đã có
/// `vetted`, xem `pairbook.rs`), rồi GHI ĐÈ `app_state.gas_units` (khởi tạo
/// sẵn = fallback config). Chờ tới khi có CẢ provider HTTP lẫn ≥1 pair đã vet
/// sẵn sàng (poll mỗi 5s, tối đa `MAX_ATTEMPTS` lần) — đo lỗi (revert/RPC lỗi)
/// thì thử lại; hết số lần thử vẫn giữ fallback config, log rõ, KHÔNG panic,
/// KHÔNG chặn boot (task nền độc lập, `main()` không `.await` task này).
async fn gas_units_boot_task(app_state: AppState, sim_http_pool: Arc<transport::RpcPool>) {
    // Cum `hotpath-fix-then-decoder-ur` (B5, no BAOCAO38) - cho pair.reload
    // LAN DAU xong THAT SU (event-driven qua Notify, khong doan thoi luong co
    // dinh) truoc khi bat dau vong lap do gas - BAOCAO38 ghi nhan giveup som
    // hon reload chi 700ms du co 60s ngan sach (reload ~90 dong pairs.txt qua
    // RPC thuc te co the mat >60s). Tran 120s la LUOI AN TOAN (phong khi
    // provider khong bao gio ket noi duoc) - khong chan boot vo han.
    let _ = tokio::time::timeout(Duration::from_secs(120), app_state.pairs_first_reload_done.notified()).await;
    const MAX_ATTEMPTS: u32 = 12; // 12 * 5s = 60s THEM sau khi reload lan dau xong (vong lap goc)
    let probe_in = alloy::primitives::U256::from(50_000_000_000_000_000u128); // 0.05 BNB
    for attempt in 1..=MAX_ATTEMPTS {
        // Cum 0.d - provider RIENG (sim_provider), khong dung chung duong
        // nong (giong pairs_vet_task).
        let provider_opt = app_state.sim_provider.read().await.clone();
        let current_block = app_state.last_block.read().await.unwrap_or(0);
        let target = app_state.pairbook.read().await.tokens_to_vet().into_iter().next();
        if let (Some(provider), true, Some((_pair_addr, token, _quote))) = (provider_opt, current_block > 0, target) {
            match bsc_sandwich::sim_evm::measure_gas_units(provider, current_block, token, probe_in).await {
                Ok((front, back)) => {
                    *app_state.gas_units.write().await = (front, back);
                    app_state.logger.log(
                        "gas.units_measured",
                        serde_json::json!({
                            "token": format!("{token:#x}"),
                            "block": current_block,
                            "gas_units_front": front,
                            "gas_units_back": back,
                            "source": "revm_boot",
                            "attempt": attempt,
                        }),
                    );
                    return;
                }
                Err(e) => {
                    let err_str = e.to_string();
                    app_state.logger.log(
                        "gas.units_measure_error",
                        serde_json::json!({ "token": format!("{token:#x}"), "error": err_str, "attempt": attempt }),
                    );
                    if transport::is_unsupported_method_error(&err_str) {
                        if let Some(new_provider) = sim_http_pool.mark_current_unsupported(&app_state.logger, "sim").await {
                            *app_state.sim_provider.write().await = Some(new_provider);
                        }
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    let (f, b) = *app_state.gas_units.read().await;
    app_state.logger.log(
        "gas.units_measure_giveup",
        serde_json::json!({
            "attempts": MAX_ATTEMPTS,
            "fallback_gas_units_front": f,
            "fallback_gas_units_back": b,
            "reason": "het so lan thu (chua co provider san sang, hoac pairs.txt chua co pair da vet nao, hoac do lien tuc loi)",
        }),
    );
}

/// Cụm A6 — task nền log 1 dòng `funnel.minute` mỗi `interval` (ship 60s)
/// rồi RESET bộ đếm về 0 (`FunnelCounters::snapshot_and_reset`) — "cộng dồn
/// trong phút đó", không phải tổng tích luỹ từ lúc boot.
async fn funnel_report_task(app_state: AppState, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        let snap = app_state.funnel.snapshot_and_reset();
        app_state.logger.log("funnel.minute", snap);
    }
}

/// Cụm `exec-path-traps` (V-06) — theo dõi `state/halt.lock` ĐỘC LẬP với 3
/// nguồn tx (`subscribe_pending_txs`/`poll_txpool_pending`/`watch_inject_file`,
/// mỗi nguồn tự kiểm `state_files.is_halted()` NGAY TRƯỚC khi spawn
/// `handle_paper_tx` — xem 3 hàm đó, đó là nơi paper loop THẬT SỰ dừng).
/// Task này CHỈ lo 2 việc quan sát được từ ngoài: log ĐÚNG 1 dòng
/// `halt.triggered`/`halt.cleared` mỗi lần CHUYỂN trạng thái (không lặp lại
/// mỗi tick khi vẫn đang halt) + cập nhật `bot_state` cho `/api/status`
/// (`STOPPED` khi halt, quay về `WATCHING` khi xoá file — đúng state machine
/// CLAUDE.md `STOPPED --reset--> IDLE`... thực tế ở đây coi xoá halt.lock là
/// "chạy lại bình thường", không phải nhánh `reset.req` riêng, xem
/// `state.rs`).
/// V-06 — logic THUẦN "trạng thái trước -> trạng thái hiện tại" ra quyết
/// định log gì, tách khỏi `halt_watch_task` để test được không cần dựng
/// `AppState` đầy đủ (state_files/logger/bot_state...).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HaltTransition {
    None,
    Triggered,
    Cleared,
}

fn halt_transition(was_halted: bool, now_halted: bool) -> HaltTransition {
    match (was_halted, now_halted) {
        (false, true) => HaltTransition::Triggered,
        (true, false) => HaltTransition::Cleared,
        _ => HaltTransition::None,
    }
}

async fn halt_watch_task(app_state: AppState, interval: Duration) {
    let mut was_halted = false;
    let mut ticker = tokio::time::interval(interval);
    loop {
        let now_halted = app_state.state_files.is_halted();
        match halt_transition(was_halted, now_halted) {
            HaltTransition::Triggered => {
                app_state.logger.log("halt.triggered", serde_json::json!({}));
                *app_state.bot_state.write().await = BotState::Stopped;
            }
            HaltTransition::Cleared => {
                app_state.logger.log("halt.cleared", serde_json::json!({}));
                *app_state.bot_state.write().await = BotState::Watching;
            }
            HaltTransition::None => {}
        }
        was_halted = now_halted;
        ticker.tick().await;
    }
}

/// Cụm `foundation-fix-then-real-sim` (A4) — gate rẻ tiền (0 RPC, chạy TRƯỚC
/// decode): `to=None` (tx inject định dạng cũ, không có cột `to`) coi là
/// "không rõ router" -> CHO QUA CHỈ khi `source == "inject"` (tx bơm từ
/// `state/inject_tx.jsonl` định dạng cũ, không có cột `to`, xem doc-comment
/// `PendingTxRaw::to`). `to=Some(addr)` -> phải khớp 1 trong 5 router Pancake
/// đã pin (`venues::venue_for_router`), sai thì `false` (`not_pancake_router`),
/// BẤT KỂ `source`.
///
/// Cụm `exec-path-traps` (F-15) — TRƯỚC bản sửa này, `to=None` LUÔN `true`
/// (không phân biệt nguồn), nghĩa là 1 tx WS/`txpool_content` thật hiếm khi
/// trả `to=None` (vd contract-creation lẫn vào path swap do lỗi decode
/// nguồn/node) sẽ lọt qua gate (a) dù không có cách nào biết nó có phải
/// router Pancake hay không — rủi ro "cho qua nhầm" đúng như audit F-15 chỉ
/// ra. `pending_tx_from_rpc` xác nhận tx pending THẬT (WS/`txpool_content`)
/// LUÔN có `Some(to)` (trừ trường hợp hiếm contract-creation không nằm trong
/// path swap nào cả) nên xiết chặt về `false` cho các nguồn đó không phá vỡ
/// hành vi thật — chỉ `source == "inject"` (định dạng cũ, cố ý không có cột
/// `to`) mới còn được CHO QUA khi `to=None`.
fn passes_router_gate(to: Option<Address>, source: &str) -> bool {
    match to {
        None => source == "inject",
        Some(addr) => venues::venue_for_router(addr).is_some(),
    }
}

/// Cụm `econ-truth-latency-vps` (mục 1) — dedup DÙNG CHUNG giữa 3 nguồn tx
/// (xem `transport::SeenHashSet`). Hash `B256::ZERO` (định dạng cũ
/// `state/inject_tx.jsonl`, không có cột hash thật) KHÔNG bị dedup — mỗi
/// dòng inject là 1 quyết định tường minh của Chủ/test, không phải tx thật
/// trùng lặp ngẫu nhiên.
async fn dedup_allows_processing(app_state: &AppState, hash: alloy::primitives::B256) -> bool {
    if hash == alloy::primitives::B256::ZERO {
        return true;
    }
    app_state.seen_hashes.write().await.insert_if_new(hash)
}

fn selector_hex_of(input: &[u8]) -> Option<String> {
    input.get(0..4).map(|s| format!("0x{}", s.iter().map(|b| format!("{b:02x}")).collect::<String>()))
}

/// Cụm A4 — dựng `TxLogMeta` (hash/to/venue-theo-router/selector) dùng chung
/// cho `tx.seen` (log thô, `log_tx_seen` dưới) VÀ `tx.skip`/`sim.result`
/// (`pipeline::log_outcome_v2`, gọi trong `handle_paper_tx`) — 1 nơi tính,
/// tránh 2 log lệch nhau.
fn build_tx_log_meta(raw: &PendingTxRaw) -> TxLogMeta {
    let router_venue = raw.to.and_then(venues::venue_for_router);
    TxLogMeta {
        hash: format!("{:#x}", raw.hash),
        to: raw.to.map(|a| format!("{a:#x}")),
        venue: router_venue.map(|v| v.as_str().to_string()),
        selector: selector_hex_of(&raw.input),
        fee: None,
        detail: None,
        // Cum `real-economics-mode2` (muc 2) - dien dan trong handle_paper_tx
        // khi tung gia tri co san (resolve pool/tinh gas that xong) - None
        // luc khoi tao la dung cho tx chua qua toi buoc do (decode_fail/
        // not_pancake_router...).
        amount_in: None,
        quote: None,
        pair: None,
        reserve_quote: None,
        gas_cost_wei: None,
        gas_price_gwei: None,
        seen_to_decision_ms: None,
        amount_in_bnb_equiv: None,
        victim_in_competitor_cluster: None,
        bribe_wei: None,
        net_pos_after_bribe_wei: None,
    }
}

fn log_tx_seen(logger: &BotLogger, source: &str, raw: &PendingTxRaw) {
    let meta = build_tx_log_meta(raw);
    logger.log(
        "tx.seen",
        serde_json::json!({
            "source": source,
            "from": format!("{:#x}", raw.from),
            "to": meta.to,
            "hash": meta.hash,
            "venue": meta.venue,
            "selector": meta.selector,
        }),
    );
}

/// Cụm `5.1` — lõi paper loop: nhận 1 tx thô (pending thật hoặc inject),
/// resolve reserve pool V2 THẬT qua RPC (chỉ khi qua được precheck rẻ tiền,
/// tránh tốn `eth_call` cho tx rõ ràng không phải candidate), gọi
/// `pipeline::decide_paper`, log kết quả, tăng `skip_counts`. Bọc bằng
/// `pending_semaphore` để giới hạn ≤ 4 tx xử lý đồng thời (CLAUDE.md lệnh
/// `5.1`) — mỗi lệnh gọi giữ ĐÚNG 1 permit tới khi xong (RAII qua
/// `OwnedSemaphorePermit`, tự trả khi hàm return ở bất kỳ nhánh nào).
///
/// Cụm `quote-live-wiring-funnel-diagnostics` (BAOCAO30) — Phase 1 nối
/// `decide_paper_quote` (BAOCAO29, trước đây có sẵn nhưng CHƯA nối) vào live
/// loop, cho CẢ WBNB lẫn USDT, KHÔNG đổi 1 dòng hành vi nhánh WBNB hiện có:
/// nhánh WBNB (`precheck_token_only` -> `resolve_v2_reserves` ->
/// `decide_and_build_paper_v2`, wallet|pair|universal) chạy Y HỆT TRƯỚC —
/// chỉ khi nhánh đó trả `NotWbnbPair` (tx không khớp WBNB ở path) VÀ
/// `cfg.scan_quote_usdt=true`, mới thử NHÁNH THỨ 2 (song song, không thay
/// thế): `precheck_quote_only` (thử lại WBNB rồi USDT) -> nếu nhận diện
/// đúng USDT (chiều MUA) -> `resolve_reserves_for_quote` -> `decide_paper_quote`.
/// `scan_quote_usdt=false` (ship mặc định) -> nhánh 2 không bao giờ chạy ->
/// hành vi tổng thể KHÔNG đổi 1 bit so với trước phiên này.
///
/// Cụm `hotpath-fix-then-decoder-ur` (A4) — resolve `(pair_addr, reserves)`
/// với 2 lớp giảm RPC, DÙNG CHUNG cho cả nhánh WBNB lẫn USDT:
/// (a) token đã có sẵn trong `PairBook` (`known_pair`, từ `pairs.txt` đã
///     resolve lúc `reload`) -> bỏ hẳn `eth_call getPair`, chỉ còn
///     `getReserves` (`resolve_v2_reserves_known_pair`).
/// (b) `(pair_addr, current_block)` đã có trong `ReserveCache` (candidate
///     KHÁC cùng pool, cùng block, tới trước) -> bỏ luôn `getReserves`, dùng
///     lại reserves đã đo.
/// Token KHÔNG có trong `PairBook` (chưa vet/ngoài `pairs.txt`) vẫn resolve
/// đầy đủ như cũ (`resolve_v2_reserves`/`resolve_reserves_for_quote`) — 2 lớp
/// trên KHÔNG đổi kết quả cuối cùng, chỉ đổi SỐ `eth_call` cần thiết để tới
/// được kết quả đó.
async fn resolve_reserves_cached(
    app_state: &AppState,
    provider: &dyn Provider,
    token: Address,
    quote_addr: Address,
    pairbook: &PairBook,
    current_block: u64,
) -> Result<(Address, PoolReserves), pipeline::PipelineSkip> {
    if let Some(pair_addr) = pairbook.known_pair(token, quote_addr) {
        if let Some(reserves) = app_state.reserve_cache.read().await.cached(pair_addr, quote_addr, current_block) {
            return Ok((pair_addr, reserves));
        }
        let reserves = pipeline::resolve_v2_reserves_known_pair(provider, pair_addr, quote_addr).await?;
        app_state.reserve_cache.write().await.insert(pair_addr, quote_addr, current_block, reserves);
        return Ok((pair_addr, reserves));
    }
    let (pair_addr, reserves) = if quote_addr == venues::wbnb_addr() {
        pipeline::resolve_v2_reserves(provider, token).await?
    } else {
        pipeline::resolve_reserves_for_quote(provider, token, pipeline::QuoteAsset::Usdt).await?
    };
    app_state.reserve_cache.write().await.insert(pair_addr, quote_addr, current_block, reserves);
    Ok((pair_addr, reserves))
}

async fn handle_paper_tx(app_state: AppState, raw: PendingTxRaw) {
    // Cum `real-economics-mode2` (muc 2) - moc thoi gian NHAN tx (proxy cho
    // luc "tx.seen" duoc log - do lech giua 2 moc nay la chi phi tokio::spawn,
    // khong dang ke o muc ms) - dung de tinh "seen_to_decision_ms" luc log
    // outcome CUOI CUNG.
    let handle_started = std::time::Instant::now();
    // Cum A4 - funnel.record_seen() da chuyen ra 3 ham goi (subscribe_pending_txs/
    // poll_txpool_pending/watch_inject_file) NGAY khi quan sat duoc raw tx, TRUOC
    // gate (a) - "seen" dem MOI tx quan sat duoc, khong phu thuoc co spawn task
    // nay hay khong (xem doc-comment cac ham do).
    let _permit = match app_state.pending_semaphore.clone().acquire_owned().await {
        Ok(p) => p,
        Err(_) => return, // semaphore dong (shutdown) - khong con gi de lam
    };

    let cfg = app_state.config.read().await.clone();
    if !cfg.dry_run {
        // "Paper loop (dry_run only)" - CLAUDE.md cam moi hanh vi ngoai
        // dry_run o cum nay (khong co logic gui tx that o day de tat, chi
        // dam bao khong chay nham logic paper khi chu da chuyen sang live).
        return;
    }
    // Cum `exec-path-traps` (V-06) - lop bao ve THU 2 (3 nguon tx da tu kiem
    // truoc khi spawn ham nay, xem subscribe_pending_txs/poll_txpool_pending/
    // watch_inject_file) - phong truong hop mot nguon tx tuong lai quen kiem.
    if app_state.state_files.is_halted() {
        return;
    }
    let current_block = app_state.last_block.read().await.unwrap_or(0);
    let mut meta = build_tx_log_meta(&raw);
    // Cum `real-economics-mode2` (muc 2) - gia tri MAC DINH cho nhanh WBNB
    // (da so tx hot path): amount_in = tx.value (dung cho swapExactETHForTokens/
    // UR V2_SWAP_EXACT_IN, noi amountIn nam trong msg.value, khong phai
    // calldata) - nhanh USDT (scan_quote_usdt=true) ghi de lai quote="usdt"
    // ben duoi, amount_in USDT (nam trong calldata, khong phai tx.value)
    // CHUA wire o day, ghi ro CON NO (scan_quote_usdt=false ship, khong phai
    // duong nong).
    meta.quote = Some("wbnb".to_string());
    meta.amount_in = Some(raw.value.to_string());
    // Cum `econ-truth-latency-vps` (muc 1) - nhanh WBNB: amount_in DA la BNB,
    // quy doi = chinh no (khong can reserve nao).
    meta.amount_in_bnb_equiv = Some(raw.value.to_string());

    // Cum pair-mode - precheck CHI decode + xac dinh chieu (KHONG loc theo
    // victims.txt som nhu 5.1, vi pair-mode khong quan tam dia chi `from` -
    // moi tx decode duoc deu can resolve pair_addr THAT truoc khi biet la
    // wallet-mode hay pair-mode, xem doc-comment pipeline::precheck_token_only).
    // Cum A4 - doi sang `precheck_token_and_venue` de biet THEM
    // `decoder::SwapVenue` that (V2 hay V3+fee) - gate (c) ngay duoi day chan
    // V3 KHONG duoc dua vao `resolve_v2_reserves` (bug cu: V3 bi sim nham
    // bang pool V2).
    let precheck = pipeline::precheck_token_and_venue(&raw.input, raw.value);
    // Cum "usdt-quote-asset" - FIX LOGGER (BAOCAO29): truoc phien nay
    // `log_outcome_v2` luon nhan `None` cho token_hint o day, du `precheck`
    // da tra `Ok(token)` (decode + resolve pool THANH CONG, chi bi skip o
    // buoc SAU nhu no_pool/honeypot_or_tax/thin_liq) - `tx.skip.token` vi vay
    // luon `null` sai (xem docs/TASKS.md/BAOCAO28 muc no). Sua: giu token
    // THAT ngay khi `precheck` tra `Ok`, CHI `None` khi chinh buoc decode nay
    // that bai (`decode_fail`/`not_wbnb_pair` - token chua tung biet duoc).
    let mut token_hint = token_hint_from_precheck(precheck.map(|(t, _, _)| t));

    // Cum `exec-path-traps` (F-16) - cross-check router THAT (tx.to, qua
    // venues::venue_for_router) voi selector da decode - selector V2 co dien
    // gui toi router V3/UR (hoac nguoc lai) la calldata bat thuong/decode
    // nham, coi nhu decode_fail (KHONG phai candidate hop le) thay vi cho
    // tiep tuc vao nhanh V2/V3 nhu binh thuong.
    let router_venue = raw.to.and_then(venues::venue_for_router);
    let precheck = precheck.and_then(|(token, venue, selector_name)| {
        if bsc_sandwich::decoder::venue_matches_router(selector_name, router_venue) {
            Ok((token, venue))
        } else {
            meta.detail = Some("venue_mismatch".to_string());
            Err(pipeline::PipelineSkip::DecodeFail)
        }
    });

    let (outcome, source) = match precheck {
        Err(skip) => (PipelineOutcome::Skip(skip), "none"),
        // Cum A4, gate (c) - venue V3 (exactInputSingle/exactInput/UR
        // V3_SWAP_EXACT_IN, ke ca khi router la SmartRouter/UR dang bat
        // gate a) chua co quoter/sim pin o TANG PIPELINE nay -> venue_unpinned,
        // KHONG goi resolve_v2_reserves (thay vi bi sim nham bang pool V2 nhu
        // truoc).
        Ok((_token, SwapVenue::V3 { fee })) => {
            meta.fee = Some(fee);
            app_state.funnel.record_venue_v3();
            (PipelineOutcome::Skip(pipeline::PipelineSkip::VenueUnpinned), "none")
        }
        Ok((token, SwapVenue::V2)) => {
            app_state.funnel.record_venue_v2();
            let provider_guard = app_state.provider.read().await;
            match provider_guard.as_ref() {
                None => (PipelineOutcome::Skip(pipeline::PipelineSkip::NoPool), "none"),
                Some(provider) => {
                    // Cum A4 - doc PairBook TRUOC de tan dung known_pair (bo
                    // eth_call getPair khi token da co san trong pairs.txt).
                    let pairbook_for_resolve = app_state.pairbook.read().await;
                    let resolve_result =
                        resolve_reserves_cached(&app_state, provider, token, venues::wbnb_addr(), &pairbook_for_resolve, current_block)
                            .await;
                    drop(pairbook_for_resolve);
                    match resolve_result {
                    Err(skip) => (PipelineOutcome::Skip(skip), "none"),
                    Ok((pair_addr, reserves)) => {
                        // Cum `real-economics-mode2` (F-03) - gas that: gia
                        // eth_gasPrice (cache theo block) x max(gia do, gia
                        // gas cua chinh victim) x tong gas unit front+back
                        // (do 1 lan luc boot bang revm, xem gas_units_boot_task).
                        let gas_price_wei = app_state.gas_oracle.gas_price_wei(provider, current_block, &app_state.logger).await;
                        let (units_front, units_back) = *app_state.gas_units.read().await;
                        let victim_gas_price: u128 = u128::try_from(raw.gas_price).unwrap_or(u128::MAX);
                        let gas_cost_wei = pipeline::compute_gas_cost_wei(
                            units_front,
                            units_back,
                            gas_price_wei,
                            victim_gas_price,
                            cfg.gas_price_max_wei(),
                        );
                        // Cum `real-economics-mode2` (muc 2) - dien cac field
                        // log moi ngay khi co du lieu (pool da resolve, gas da tinh).
                        meta.pair = Some(format!("{pair_addr:#x}"));
                        meta.reserve_quote = Some(reserves.reserve_wbnb.to_string());
                        meta.gas_cost_wei = Some(gas_cost_wei.to_string());
                        meta.gas_price_gwei = Some(gas_price_wei as f64 / 1e9);
                        let victims = app_state.victims.read().await;
                        let pairbook = app_state.pairbook.read().await;
                        // Cum `real-economics-mode2` (muc 0) - danh dau ro
                        // "vet_fail" trong log tx.skip khi pool DA BIET nhung
                        // vet NEN vua loai (giu visibility, khac im lang roi
                        // not_in_list) - xem pipeline::decide_paper_v2 nhanh
                        // pair moi tra honeypot_or_tax cho truong hop nay.
                        if pairbook.is_vet_failed(&pair_addr) {
                            meta.detail = Some("vet_fail".to_string());
                        }
                        let tax_cache = app_state.tax_cache.read().await;
                        let risk = app_state.risk_guard.read().await;
                        let input = pipeline::PaperDecisionV2 {
                            from: raw.from,
                            calldata: &raw.input,
                            tx_value: raw.value,
                            reserves,
                            pair_addr,
                            current_block,
                            gas_cost_wei,
                        };
                        let (v2_outcome, v2_source) =
                            pipeline::decide_and_build_paper_v2(&victims, &pairbook, &tax_cache, &cfg, &risk, &app_state.logger, &input);
                        // Cum `econ-truth-latency-vps` (muc 4, no nho) - gate
                        // nonce THUAN TU CACHE (khong them eth_call nao tren
                        // duong nong, ap dung SAU khi co outcome/source that de
                        // khong phai lap lai logic routing wallet/pair/none cua
                        // decide_paper_v2): cache CO du lieu (vd tu
                        // run_evm_decision neu tung chay, hoac tuong lai co
                        // task nen dien) -> ap dung nonce_stale/nonce_future
                        // that su, GHI DE outcome; cache MISS (thuc te hien tai
                        // LUON miss voi sim_engine="v2" vi chua co nguon nao
                        // dien no tren duong nong - ghi ro, khong bia hieu qua)
                        // -> giu nguyen outcome goc, khong chan.
                        let nonce_verdict =
                            app_state.nonce_cache.read().await.cached(raw.from, current_block).map(|expected| transport::compare_nonce(raw.nonce, expected));
                        match nonce_verdict {
                            Some(transport::NonceCheck::Stale) => (PipelineOutcome::Skip(pipeline::PipelineSkip::NonceStale), v2_source),
                            Some(transport::NonceCheck::Future) => (PipelineOutcome::Skip(pipeline::PipelineSkip::NonceFuture), v2_source),
                            Some(transport::NonceCheck::Ok) | None => (v2_outcome, v2_source),
                        }
                    }
                    }
                }
            }
        }
    };

    // Cum `quote-live-wiring-funnel-diagnostics` - nhanh 2 (USDT fallback),
    // CHI chay khi nhanh WBNB tren xac nhan "khong phai WBNB pair" VA co bat
    // scan_quote_usdt - khong dung lai nhanh nay cho moi tx (tranh ton them
    // eth_call/log cho tx da co ket qua ro rang tu nhanh WBNB, vd decode_fail/
    // victim dang mua bang WBNB da xu ly xong o tren).
    let (outcome, source) = if matches!(outcome, PipelineOutcome::Skip(pipeline::PipelineSkip::NotWbnbPair)) && cfg.scan_quote_usdt {
        match pipeline::precheck_quote_only(&raw.input, raw.value, true) {
            Ok((token, pipeline::QuoteAsset::Usdt)) => {
                token_hint = Some(token); // biet token THAT du buoc sau co skip vi ly do gi
                meta.quote = Some("usdt".to_string());
                // Cum B5 (no BAOCAO38) - amount_in USDT nam trong calldata
                // (khong phai tx.value nhu nhanh WBNB) - decode lai (thuan,
                // re, calldata da qua duoc precheck_quote_only nen chac chan
                // Ok) CHI de lay dung amount_in that cho log, khong dung ket
                // qua nay cho quyet dinh (decide_paper_quote tu decode rieng).
                meta.amount_in = bsc_sandwich::decoder::decode_swap_calldata(&raw.input, raw.value)
                    .ok()
                    .map(|d| d.amount_in.to_string());
                let provider_guard = app_state.provider.read().await;
                let usdt_outcome = match provider_guard.as_ref() {
                    None => PipelineOutcome::Skip(pipeline::PipelineSkip::NoPool),
                    Some(provider) => {
                        // Cum A4/A2 - doc PairBook 1 LAN, dung chung cho
                        // known_pair (giam RPC) VA cong tax_ok (decide_paper_quote).
                        let pairbook = app_state.pairbook.read().await;
                        match resolve_reserves_cached(&app_state, provider, token, venues::usdt_addr(), &pairbook, current_block).await {
                            Err(skip) => PipelineOutcome::Skip(skip),
                            Ok((pair_addr, reserves)) => {
                                // Cum `real-economics-mode2` (muc 1.d) - gas
                                // that (BNB) quy doi sang USDT qua reserve
                                // WBNB/USDT THAT tai block hien tai (KHONG
                                // price oracle) - resolve_v2_reserves(USDT)
                                // tra dung cap (reserve_wbnb, reserve_usdt)
                                // cua pool do.
                                meta.pair = Some(format!("{pair_addr:#x}"));
                                meta.reserve_quote = Some(reserves.reserve_wbnb.to_string());
                                // Cum `real-economics-mode2` (muc 0) - cung
                                // danh dau "vet_fail" nhu nhanh WBNB (giu
                                // visibility thay vi roi im lang qua not_in_list).
                                if pairbook.is_vet_failed(&pair_addr) {
                                    meta.detail = Some("vet_fail".to_string());
                                }
                                let gas_price_wei = app_state.gas_oracle.gas_price_wei(provider, current_block, &app_state.logger).await;
                                let (units_front, units_back) = *app_state.gas_units.read().await;
                                let victim_gas_price: u128 = u128::try_from(raw.gas_price).unwrap_or(u128::MAX);
                                let gas_cost_bnb_wei = pipeline::compute_gas_cost_wei(
                                    units_front,
                                    units_back,
                                    gas_price_wei,
                                    victim_gas_price,
                                    cfg.gas_price_max_wei(),
                                );
                                // Cum A4 - pool WBNB/USDT (dung de quy doi gas)
                                // cung di qua resolve_reserves_cached (chinh
                                // pool nay da co san trong pairs.txt, xem dong
                                // "USDT | vetted ... quote asset") - giam them
                                // 1 eth_call getPair moi candidate USDT.
                                let (gas_cost_usdt_wei, amount_in_bnb_equiv) =
                                    match resolve_reserves_cached(&app_state, provider, venues::usdt_addr(), venues::wbnb_addr(), &pairbook, current_block)
                                        .await
                                    {
                                        Ok((_p, wbnb_usdt_reserves)) => {
                                            let gas_usdt = pipeline::convert_gas_cost_bnb_to_usdt(
                                                gas_cost_bnb_wei,
                                                wbnb_usdt_reserves.reserve_wbnb,
                                                wbnb_usdt_reserves.reserve_token,
                                            );
                                            // Cum `econ-truth-latency-vps` (muc 1) - quy
                                            // doi amount_in USDT (da decode o tren) sang
                                            // BNB-equivalent CUNG 1 cap reserve vua lay,
                                            // khong ton them eth_call nao.
                                            let amount_bnb_equiv = meta.amount_in.as_deref().and_then(|s| s.parse::<u128>().ok()).map(
                                                |amt_usdt| {
                                                    pipeline::convert_usdt_to_bnb_wei(
                                                        amt_usdt,
                                                        wbnb_usdt_reserves.reserve_wbnb,
                                                        wbnb_usdt_reserves.reserve_token,
                                                    )
                                                },
                                            );
                                            (gas_usdt, amount_bnb_equiv)
                                        }
                                        Err(_) => (u128::MAX, None), // khong quy doi duoc -> coi gas vo cung dat, an toan
                                    };
                                meta.gas_cost_wei = Some(gas_cost_usdt_wei.to_string());
                                meta.amount_in_bnb_equiv = amount_in_bnb_equiv.map(|v| v.to_string());
                                meta.gas_price_gwei = Some(gas_price_wei as f64 / 1e9);
                                let tax_cache = app_state.tax_cache.read().await;
                                let (o, _tag) = pipeline::decide_paper_quote(
                                    &pairbook,
                                    pair_addr,
                                    &tax_cache,
                                    &cfg,
                                    &raw.input,
                                    raw.value,
                                    reserves,
                                    current_block,
                                    gas_cost_bnb_wei,
                                    gas_cost_usdt_wei,
                                );
                                o
                            }
                        }
                    }
                };
                (usdt_outcome, "usdt")
            }
            // WBNB da duoc nhanh 1 xu ly rieng (khong bao gio thuc su roi vao
            // day trong thuc te, giu day du kieu). Khong khop ca WBNB lan
            // USDT (hoac sell_direction) -> decode_and_classify_quote da tra
            // dung ly do (not_quote_pair/sell_direction), CHINH XAC HON
            // not_wbnb_pair cu khi da biet chac USDT dang duoc quet.
            Ok((_, pipeline::QuoteAsset::Wbnb)) => (PipelineOutcome::Skip(pipeline::PipelineSkip::NotWbnbPair), "none"),
            Err(skip) => (PipelineOutcome::Skip(skip), "none"),
        }
    } else {
        (outcome, source)
    };

    // ===== Cụm `evm-validate-fixed-then-wire` (C3 + B3.2) — QUYẾT ĐỊNH LẠI
    // bằng EVM THẬT khi `sim_engine="evm"`. Chỉ chạy khi bước sim công thức
    // đóng đã ra `Simulated` (số candidate tới đây rất ít sau các gate rẻ),
    // và ta biết `(token, quote)`. `decide_paper_v2` (công thức đóng) giờ chỉ
    // còn vai trò ƯỚC LƯỢNG KHOẢNG `front_in` (đúng CLAUDE.md).
    let (outcome, source) = if cfg.sim_engine_is_evm() {
        if let (PipelineOutcome::Simulated(_), Some(token)) = (&outcome, token_hint) {
            let quote = if source == "usdt" { pipeline::QuoteAsset::Usdt } else { pipeline::QuoteAsset::Wbnb };
            let evm_final = run_evm_decision(&app_state, &cfg, token, quote, &raw, current_block, outcome.clone()).await;
            (evm_final, source)
        } else {
            (outcome, source)
        }
    } else {
        (outcome, source)
    };

    // Cụm `bugfix-presign-and-contract-plan` (A3) — `from` có thuộc CỤM ĐỐI
    // THỦ không (3 seed tĩnh + ví vừa nhận quote-asset từ seed trong block
    // hiện tại/trước, xem `src/competitor.rs`). Đọc THUẦN từ bộ nhớ (0 RPC).
    let in_competitor_cluster = app_state.competitor_cluster.read().await.contains(raw.from, current_block);
    meta.victim_in_competitor_cluster = Some(in_competitor_cluster);
    // `allow_competitor_victims=false` (ship) + `live_mode != "off"` (đã có
    // khả năng KÝ thật) -> KHÔNG cho nhóm này thành `Simulated`. Ở
    // `live_mode="off"` (paper thuần) KHÔNG chặn — vẫn cần số liệu để Chủ
    // quyết định (xem doc-comment `Config::allow_competitor_victims`).
    let outcome = if in_competitor_cluster
        && !cfg.allow_competitor_victims
        && cfg.live_mode != "off"
        && matches!(outcome, PipelineOutcome::Simulated(_))
    {
        app_state.funnel.record_competitor_victim();
        PipelineOutcome::Skip(pipeline::PipelineSkip::CompetitorVictim)
    } else {
        outcome
    };
    record_funnel_terminal(&app_state.funnel, &outcome);
    meta.seen_to_decision_ms = Some(handle_started.elapsed().as_secs_f64() * 1000.0);
    // Cum `competitor-recon-and-strategy` (F-02) - tinh lai bribe/net_pos_after_bribe
    // CUNG 1 ham thuan (`compute_bribe_wei`) da dung de gate Simulated o
    // pipeline::evaluate_candidate*/, chi de LOG (khong anh huong quyet dinh -
    // quyet dinh da chot trong `outcome`).
    if let PipelineOutcome::Simulated(q) = &outcome {
        let clamp = if source == "usdt" { None } else { Some((cfg.bribe_min_wei(), cfg.bribe_max_wei())) };
        let bribe_wei = pipeline::compute_bribe_wei(q.profit_wei as u128, cfg.bribe_pct_of_profit, clamp);
        meta.bribe_wei = Some(bribe_wei.to_string());
        meta.net_pos_after_bribe_wei = Some((q.profit_wei - bribe_wei as i128).to_string());
    }
    // Cụm `bugfix-presign-and-contract-plan` (A4) — đánh dấu pool "đang nóng"
    // (candidate ĐI TỚI bước sim thật, không phải skip rẻ ở đầu chuỗi) để
    // `pairs_vet_task` rút chu kỳ vet xuống 300 s cho riêng pool đó.
    if matches!(
        outcome,
        PipelineOutcome::Simulated(_)
            | PipelineOutcome::Skip(pipeline::PipelineSkip::Unprofitable)
            | PipelineOutcome::Skip(pipeline::PipelineSkip::SanityReject)
    ) {
        if let Some(pair_addr) = meta.pair.as_deref().and_then(|p| Address::from_str(p).ok()) {
            app_state.candidate_seen.write().await.insert(pair_addr, Instant::now());
        }
    }
    pipeline::log_outcome_v2(&app_state.logger, raw.from, token_hint, source, &meta, &outcome);
    if let PipelineOutcome::Skip(skip) = &outcome {
        let mut counts = app_state.skip_counts.write().await;
        *counts.entry(skip.as_str().to_string()).or_insert(0) += 1;
    }
    // Cum `econ-truth-latency-vps` (muc 3) - do decision_vs_mined_block CHI
    // cho candidate Simulated (hiem sau cac gate re, xem BAOCAO39: 14/60
    // phut) - khong ton RPC them tren duong nong (task nen rieng, khong
    // .await trong ham nay).
    if matches!(outcome, PipelineOutcome::Simulated(_)) {
        spawn_post_simulated_tracker(app_state.clone(), raw.hash, meta.pair.clone(), current_block);
        // Cụm `competitor-recon-and-strategy` (mục 4, shadow mode) — CHỈ khi
        // `live_mode="shadow"` (ship "off", nhánh này không chạy) VÀ đã nạp
        // được signer lúc boot. Hỗ trợ CẢ 2 quote asset (WBNB VÀ USDT —
        // `calldata::encode_front_buy_usdt`/`encode_back_sell_usdt` đã có sẵn
        // từ cụm `usdt-quote-asset`, chỉ chưa có nơi gọi cho shadow trước cụm
        // này). Task NỀN riêng, không chặn `handle_paper_tx`.
        if cfg.live_mode_is_shadow() && app_state.shadow_wallet.is_some() {
            if let (PipelineOutcome::Simulated(q), Some(token), Some(pair_str)) = (&outcome, token_hint, &meta.pair) {
                if let Ok(pair_addr) = Address::from_str(pair_str) {
                    let quote_asset =
                        if source == "usdt" { pipeline::QuoteAsset::Usdt } else { pipeline::QuoteAsset::Wbnb };
                    let victim_gas_price_wei: u128 = u128::try_from(raw.gas_price).unwrap_or(u128::MAX);
                    spawn_shadow_sign_task(
                        app_state.clone(),
                        cfg.clone(),
                        *q,
                        token,
                        pair_addr,
                        quote_asset,
                        raw.clone(),
                        victim_gas_price_wei,
                        current_block,
                        in_competitor_cluster,
                    );
                }
            }
        }
    }
}

/// Cụm `competitor-recon-and-strategy` (mục 4, shadow mode) — KÝ THẬT
/// front-buy/back-sell (nếu pre-sign re-vet OK), log `bundle.shadow`. KHÔNG
/// BAO GIỜ gửi/broadcast (xem `shadow.rs`, không có `send_raw_transaction`
/// nào).
///
/// # Cụm `bugfix-presign-and-contract-plan` (A4) — đường ký giờ 0 RPC
///
/// Bản BAOCAO41 gọi trên đường ký: `eth_getTransactionCount` (nonce) +
/// `eth_gasPrice` (có thể) + `eth_getTransactionReceipt` (victim còn pending?)
/// + `measure_tax_evm` (fork revm, cold-fetch state). Đo thật 30 phút: **0/36
/// lần ký kịp**, 34/36 vì victim ĐÃ lên block trước khi chuỗi đó chạy xong.
///
/// Bản này đọc TOÀN BỘ từ cache trong bộ nhớ đã được task nền nạp sẵn:
/// nonce (`SelfNonceCache`), gas (`GasOracle::cached_price_any_block`, rơi về
/// `victim.gas_price` nếu chưa có), reserve (`ReserveCache` từ Sync-event),
/// victim còn pending (`MinedTxIndex`), tình trạng vet (`PairBook`). Mỗi bước
/// được bấm giờ và ghi vào `presign_ms` của `bundle.shadow`/`tx.abort`.
#[allow(clippy::too_many_arguments)]
fn spawn_shadow_sign_task(
    app_state: AppState,
    cfg: Config,
    quote: bsc_sandwich::sim_v2::SandwichQuote,
    token: Address,
    pair_addr: Address,
    quote_asset: pipeline::QuoteAsset,
    victim: transport::PendingTxRaw,
    victim_gas_price_wei: u128,
    current_block: u64,
    in_competitor_cluster: bool,
) {
    tokio::spawn(async move {
        let t_start = std::time::Instant::now();
        let victim_hash = victim.hash;
        let Some((self_addr, wallet)) = app_state.shadow_wallet.clone() else { return };
        let quote_addr = if quote_asset == pipeline::QuoteAsset::Usdt { venues::usdt_addr() } else { venues::wbnb_addr() };
        let min_reserve_wei =
            if quote_asset == pipeline::QuoteAsset::Usdt { cfg.min_reserve_usdt_wei() } else { cfg.min_reserve_wei() };

        // ---- (a) tinh trang vet (doc PairBook trong bo nho) ----
        let t0 = std::time::Instant::now();
        let (vet_ok, vet_age_sec) = {
            let book = app_state.pairbook.read().await;
            (book.is_tax_ok(&pair_addr), book.vet_result(&pair_addr).map(|(_, age)| age))
        };
        let ms_vet = t0.elapsed().as_secs_f64() * 1000.0;

        // ---- (b) reserve tu cache Sync-event, PHAI dung block hien tai ----
        let t1 = std::time::Instant::now();
        let reserves = app_state.reserve_cache.read().await.cached(pair_addr, quote_addr, current_block);
        let ms_reserve = t1.elapsed().as_secs_f64() * 1000.0;

        // ---- (c) victim con pending theo "mempool view" cua bot ----
        let t2 = std::time::Instant::now();
        let (victim_seen, blocks_loaded) = {
            let idx = app_state.mined_index.read().await;
            (idx.contains(victim_hash), idx.blocks_loaded())
        };
        let ms_mined = t2.elapsed().as_secs_f64() * 1000.0;

        // ---- (d) nonce vi bot tu cache prefetch ----
        let t3 = std::time::Instant::now();
        let nonce_cached = app_state.self_nonce.read().await.cached(current_block);
        let ms_nonce = t3.elapsed().as_secs_f64() * 1000.0;

        let revet = bsc_sandwich::shadow::pre_sign_revet_fast(
            vet_ok,
            vet_age_sec,
            cfg.pairs_vet_interval_sec.saturating_mul(2),
            reserves.map(|r| r.reserve_wbnb),
            reserves.map(|_| current_block),
            current_block,
            min_reserve_wei,
            victim_seen,
            blocks_loaded,
            nonce_cached.map(|(_, age)| age),
            transport::MINED_INDEX_DEPTH as u64,
        );

        // Gia gas: CHI doc cache (khong goi RPC tren duong ky) - chua co so do
        // nao thi dung thang gas_price cua chinh victim (luon biet).
        let oracle_price = app_state.gas_oracle.cached_price_any_block().await.map(|(_, p)| p).unwrap_or(0);
        let effective_gas_price = oracle_price.max(victim_gas_price_wei);
        let max_fee_per_gas = effective_gas_price.saturating_mul(2);
        let (units_front, units_back) = *app_state.gas_units.read().await;
        let bribe_clamp = if quote_asset == pipeline::QuoteAsset::Usdt { None } else { Some((cfg.bribe_min_wei(), cfg.bribe_max_wei())) };
        let bribe_wei = pipeline::compute_bribe_wei(quote.profit_wei.max(0) as u128, cfg.bribe_pct_of_profit, bribe_clamp);
        let total_units = units_front as u128 + units_back as u128;
        // BO SUNG GIUA PHIEN 2026-09-16 (docs BlockRazor + 48 Club): bribe
        // KHONG di toi block.coinbase. `bribe_mode="builder_transfer"` (ship)
        // = 1 TRANSFER BNB toi VI EOA CUA BUILDER dat trong CHAN BACK - can
        // contract executor de tra bribe SAU khi kiem lai (xem
        // docs/CONTRACT_DESIGN.md B2/B3), CHUA implement o shadow mode ->
        // priority = 0, bribe_wei van duoc TINH + LOG day du.
        // `bribe_mode="gaspriority"` = rai bribe qua maxPriorityFeePerGas
        // (2 chan chia deu theo tong gas unit). Luu y voi 48 Club: bribe tra
        // qua GAS chi duoc tinh 0.9x khi xep hang (relay::CLUB48_GAS_FEE_WEIGHT),
        // nen "gaspriority" luon kem hieu qua hon "builder_transfer" o relay do.
        let max_priority_fee_per_gas: u128 = if cfg.bribe_mode == "gaspriority" && total_units > 0 { bribe_wei / total_units } else { 0 };
        let nonce = nonce_cached.map(|(n, _)| n).unwrap_or(0);

        let deadline = bsc_sandwich::executor::compute_deadline(cfg.executor_deadline_buffer_sec);
        let front_out_min = bsc_sandwich::executor::apply_slippage(quote.front_out, cfg.front_slippage_bps);
        let back_out_min = bsc_sandwich::executor::apply_slippage(quote.back_out, cfg.back_slippage_bps);
        let router = Address::from_str(venues::V2_ROUTER_ADDRESS).expect("V2_ROUTER_ADDRESS da pin phai hop le");
        let (front_calldata, front_value, back_calldata) = if quote_asset == pipeline::QuoteAsset::Usdt {
            let front = bsc_sandwich::calldata::encode_front_buy_usdt(quote_addr, token, quote.front_in, front_out_min, self_addr, deadline);
            let back = bsc_sandwich::calldata::encode_back_sell_usdt(token, quote_addr, quote.front_out, back_out_min, self_addr, deadline);
            (front, alloy::primitives::U256::ZERO, back)
        } else {
            let front = bsc_sandwich::calldata::encode_front_buy(quote_addr, token, front_out_min, self_addr, deadline);
            let back = bsc_sandwich::calldata::encode_back_sell(token, quote_addr, quote.front_out, back_out_min, self_addr, deadline);
            (front, quote.front_in, back)
        };

        let presign_ms = serde_json::json!({
            "vet": ms_vet,
            "reserve": ms_reserve,
            "mined_index": ms_mined,
            "nonce": ms_nonce,
            "total_before_sign": t_start.elapsed().as_secs_f64() * 1000.0,
        });

        let signed = bsc_sandwich::shadow::build_and_log_shadow_bundle(
            &app_state.logger,
            &wallet,
            self_addr,
            cfg.chain_id,
            &revet,
            (router, front_value, front_calldata, nonce, max_fee_per_gas, max_priority_fee_per_gas, units_front),
            (router, alloy::primitives::U256::ZERO, back_calldata, nonce + 1, max_fee_per_gas, max_priority_fee_per_gas, units_back),
            victim_hash,
            token,
            presign_ms,
        )
        .await;

        if signed.is_none() {
            return;
        }
        // Ky xong -> ghi them so lieu kinh te cua chinh bundle nay (bribe,
        // net sau bribe, co doi thu) de doi chieu voi `shadow.sim` ben duoi.
        app_state.logger.log(
            "bundle.shadow_econ",
            serde_json::json!({
                "victim_hash": format!("{victim_hash:#x}"),
                "quote": if quote_asset == pipeline::QuoteAsset::Usdt { "usdt" } else { "wbnb" },
                "front_in_wei": quote.front_in.to_string(),
                "profit_net_wei": quote.profit_wei.to_string(),
                "bribe_wei": bribe_wei.to_string(),
                "net_after_bribe_wei": (quote.profit_wei - bribe_wei as i128).to_string(),
                // Ten field ghi ro DON VI theo quote asset cua chinh pool do
                // (nhanh USDT thi day la USDT, KHONG phai BNB) - dung bai hoc
                // A1: moi con so phai tu noi no dang o don vi nao.
                "bribe_native": bribe_wei as f64 / 1e18,
                "bribe_unit": if quote_asset == pipeline::QuoteAsset::Usdt { "usdt" } else { "bnb" },
                "victim_in_competitor_cluster": in_competitor_cluster,
                "decision_block": current_block,
                "presign_total_ms": t_start.elapsed().as_secs_f64() * 1000.0,
            }),
        );

        // ---- A7: mo phong bundle 3 chan bang revm NEN (khong chan duong ky) ----
        spawn_shadow_bundle_sim(app_state.clone(), victim, token, quote.front_in, current_block, quote_asset);
    });
}

/// Cụm `bugfix-presign-and-contract-plan` (A7) — 2 relay đã pin KHÔNG có
/// `eth_callBundle` (xác nhận cURL thật, xem `shadow.rs`), nên bundle 3 chân
/// được mô phỏng **tại chỗ** bằng `revm` (`sim_evm::simulate_sandwich`:
/// front-buy → victim replay THẬT → back-sell) trong 1 task NỀN chạy SAU khi
/// đã ký — không nằm trên đường ký, không ảnh hưởng `presign_ms`.
///
/// Chỉ chạy cho quote WBNB: `simulate_sandwich` dựng chân front bằng
/// `swapExactETHForTokens*` (native BNB). Nhánh USDT ghi
/// `shadow.sim{skipped:"usdt_not_supported_by_simulate_sandwich"}` — KHÔNG
/// bịa số cho nhánh chưa hỗ trợ.
fn spawn_shadow_bundle_sim(
    app_state: AppState,
    victim: transport::PendingTxRaw,
    token: Address,
    front_in: alloy::primitives::U256,
    fork_block: u64,
    quote_asset: pipeline::QuoteAsset,
) {
    tokio::spawn(async move {
        let victim_hash = victim.hash;
        if quote_asset == pipeline::QuoteAsset::Usdt {
            app_state.logger.log(
                "shadow.sim",
                serde_json::json!({
                    "victim_hash": format!("{victim_hash:#x}"),
                    "skipped": "usdt_not_supported_by_simulate_sandwich",
                }),
            );
            return;
        }
        // RPC rieng cho revm fork (BSC_HTTP_SIM/BSC_HTTP_BG) - khong dung
        // duong nong.
        let provider = match app_state.sim_provider.read().await.clone() {
            Some(p) => p,
            None => match bg_provider_or_hot(&app_state).await {
                Some((p, _)) => p,
                None => return,
            },
        };
        let t0 = std::time::Instant::now();
        match bsc_sandwich::sim_evm::simulate_sandwich(provider, fork_block, front_in, token, &victim).await {
            Ok(o) => app_state.logger.log(
                "shadow.sim",
                serde_json::json!({
                    "victim_hash": format!("{victim_hash:#x}"),
                    "token": format!("{token:#x}"),
                    "fork_block": fork_block,
                    "front_in_wei": o.front_in.to_string(),
                    "profit_sim_wei": o.profit_wei.to_string(),
                    "profit_sim_bnb": o.profit_wei as f64 / 1e18,
                    "victim_ok": o.victim_success,
                    "buy_tax_bps": o.buy_tax_bps,
                    "sell_tax_bps": o.sell_tax_bps,
                    "sim_ms": t0.elapsed().as_secs_f64() * 1000.0,
                }),
            ),
            Err(e) => app_state.logger.log(
                "shadow.sim",
                serde_json::json!({
                    "victim_hash": format!("{victim_hash:#x}"),
                    "error": e.to_string(),
                    "sim_ms": t0.elapsed().as_secs_f64() * 1000.0,
                }),
            ),
        }
    });
}

/// Cụm `econ-truth-latency-vps` (mục 3) — đo `decision_vs_mined_block` =
/// `decision_block` (block bot ĐANG thấy lúc quyết định `Simulated`) −
/// `mined_block` (block victim THẬT SỰ được đào) — âm nghĩa là bot quyết định
/// SỚM HƠN (kịp), dương nghĩa là quyết định TRỄ hơn lúc victim đã lên block
/// (không kịp front-run được nữa dù sim ra lãi). Chờ tối đa ~12s (8 lần x
/// 1.5s, cùng khuôn `spawn_victim_validator`) rồi bỏ cuộc — KHÔNG chặn/`await`
/// trong `handle_paper_tx` (task nền độc lập).
/// Cụm `econ-truth-latency-vps` (mục 2+3) — task nền chạy SAU MỖI candidate
/// `Simulated` (hiếm, không tốn RPC đường nóng): (a) đo `decision_vs_mined_block`
/// (mục 3 — độ trễ quyết định so với lúc victim lên block thật), (b)
/// `compete.check` (mục 2 — soi tx NGAY TRƯỚC/SAU victim trong CÙNG block có
/// chạm ĐÚNG pool `pair` không, dấu hiệu có bot khác giao dịch quanh thời
/// điểm đó). Gộp 2 việc vào 1 task để dùng CHUNG 1 lượt chờ + fetch block/
/// receipt (không lặp lại `get_transaction_receipt` cho cùng 1 hash 2 lần).
///
/// **Giới hạn ghi rõ (không phải "quét toàn bộ cạnh tranh")**: chỉ kiểm tra
/// ĐÚNG 1 vị trí liền kề trước và 1 vị trí liền kề sau victim trong block —
/// một sandwich thật với tx khác chen giữa (hiếm nhưng có thể) sẽ KHÔNG bị
/// phát hiện bởi kiểm tra này. Không tính `competitor_profit_bnb` từ Swap log
/// (yêu cầu decode thêm token0/token1 + amountOut của CHÍNH tx nghi ngờ đó —
/// ngoài phạm vi thời gian cụm này, ghi CÒN NỢ) — chỉ so `gas_price` của
/// candidate nghi ngờ với victim, đủ để trả lời câu hỏi cốt lõi "có ai khác
/// cũng đang giao dịch NGAY quanh victim, trả gas cao hơn hay không".
fn spawn_post_simulated_tracker(app_state: AppState, hash: alloy::primitives::B256, pair: Option<String>, decision_block: u64) {
    tokio::spawn(async move {
        // Cum A5 - viec NEN (compete.check + decision_vs_mined): RPC nen.
        let provider = match bg_provider_or_hot(&app_state).await {
            Some((p, _)) => p,
            None => return,
        };
        let mut receipt = None;
        for _ in 0..8u32 {
            if let Ok(Some(r)) = provider.get_transaction_receipt(hash).await {
                if r.block_number.is_some() {
                    receipt = Some(r);
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(1500)).await;
        }
        let Some(receipt) = receipt else { return };
        let Some(mined_block) = receipt.block_number else { return };

        let delta: i64 = decision_block as i64 - mined_block as i64;
        app_state.logger.log(
            "latency.decision_vs_mined",
            serde_json::json!({
                "hash": format!("{hash:#x}"),
                "pair": pair,
                "decision_block": decision_block,
                "mined_block": mined_block,
                "decision_vs_mined_block": delta,
            }),
        );

        // Cum 2 - compete.check: can biet vi tri (tx_index) + danh sach tx
        // CUNG block, va gas_price cua chinh victim (de so sanh).
        let Some(pair_addr) = pair.as_deref().and_then(|p| Address::from_str(p).ok()) else { return };
        let block = match provider.get_block_by_number(alloy::eips::BlockNumberOrTag::Number(mined_block)).full().await {
            Ok(Some(b)) => b,
            _ => return,
        };
        let txs: Vec<_> = block.transactions.txns().collect();
        let tx_index = match txs.iter().position(|t| <_ as alloy::network::TransactionResponse>::tx_hash(*t) == hash) {
            Some(i) => i,
            None => return,
        };
        let victim_gas_price: u128 = <_ as alloy::consensus::Transaction>::gas_price(txs[tx_index])
            .unwrap_or_else(|| <_ as alloy::consensus::Transaction>::max_fee_per_gas(txs[tx_index]));

        let mut competitor_addr: Option<String> = None;
        let mut competitor_gas_gwei: Option<f64> = None;
        let mut checked_positions = Vec::new();
        for (label, idx) in [("before", tx_index.checked_sub(1)), ("after", Some(tx_index + 1))] {
            let Some(idx) = idx else { continue };
            let Some(cand) = txs.get(idx) else { continue };
            let cand_hash = <_ as alloy::network::TransactionResponse>::tx_hash(*cand);
            let cand_from = <_ as alloy::network::TransactionResponse>::from(*cand);
            let cand_gas_price: u128 = <_ as alloy::consensus::Transaction>::gas_price(*cand)
                .unwrap_or_else(|| <_ as alloy::consensus::Transaction>::max_fee_per_gas(*cand));
            let touches_pair = match provider.get_transaction_receipt(cand_hash).await {
                Ok(Some(r)) => r.logs().iter().any(|l| l.address() == pair_addr),
                _ => false,
            };
            checked_positions.push(serde_json::json!({
                "label": label, "hash": format!("{cand_hash:#x}"), "from": format!("{cand_from:#x}"),
                "gas_price_gwei": cand_gas_price as f64 / 1e9, "touches_pair": touches_pair,
            }));
            if touches_pair && competitor_addr.is_none() {
                competitor_addr = Some(format!("{cand_from:#x}"));
                competitor_gas_gwei = Some(cand_gas_price as f64 / 1e9);
            }
        }
        let row = serde_json::json!({
            "hash": format!("{hash:#x}"),
            "pair": pair,
            "block": mined_block,
            "tx_index": tx_index,
            "victim_gas_price_gwei": victim_gas_price as f64 / 1e9,
            "competitor": competitor_addr,
            "competitor_gas_price_gwei": competitor_gas_gwei,
            "checked_positions": checked_positions,
        });
        app_state.logger.log("compete.result", row.clone());
        app_state.compete_stats.record(row, competitor_addr.as_deref(), competitor_gas_gwei);
    });
}

/// Cụm `evm-validate-fixed-then-wire` (C3 + B3.2) — mở fork EVM tại block hiện
/// tại, ĐO TAX bằng EVM (C3, trừ khi token nằm allowlist zero-tax), rồi QUYẾT
/// ĐỊNH LẠI `Simulated`/`victim_would_revert`/`unprofitable`/`sim_error` bằng
/// EVM thật (B3.2). Trả về outcome cuối.
///
/// **Ràng buộc `!Send` (ghi rõ)**: `BlockForkCache` chứa `revm::Evm` (`!Send`),
/// nên KHÔNG được giữ qua bất kỳ `.await` nào trong task đã `spawn`. Vì vậy:
/// (1) mở fork bằng `.await` rồi làm TOÀN BỘ việc EVM trong 1 block đồng bộ
/// (revm tự `block_on` fetch remote qua `WrapDatabaseAsync` — chặn 1 worker
/// thread, chấp nhận được vì semaphore giới hạn 4), (2) trích ra các giá trị
/// `Send` (outcome, số đo tax), (3) DROP fork TRƯỚC khi `.await` tiếp theo (ghi
/// cache/log). Fork mở lại mỗi candidate (rất ít sau gate) — chia sẻ xuyên tx
/// cần worker-thread riêng, ghi CÒN NỢ.
async fn run_evm_decision(
    app_state: &AppState,
    cfg: &Config,
    token: Address,
    quote: pipeline::QuoteAsset,
    raw: &PendingTxRaw,
    current_block: u64,
    prior: PipelineOutcome,
) -> PipelineOutcome {
    let provider = match app_state.provider.read().await.as_ref() {
        Some(p) => p.clone(),
        None => return PipelineOutcome::Skip(pipeline::PipelineSkip::SimError),
    };

    // Cum `exec-path-traps` (F-13) - nonce THAT cua victim, chay TRUOC ca
    // buoc do tax/mo fork EVM (re nhat, chan som candidate khong con y nghia
    // sandwich truoc khi ton tai nguyen EVM). `disable_nonce_check=true`
    // trong sim_evm.rs::build_evm la CAU HINH NOI BO cua revm (can thiet de
    // 3 tx gia cua attacker dung chung nonce=0 replay duoc, xem doc-comment
    // o do) - KHONG lien quan gate ngoai nay: nonce victim duoc xac minh THAT
    // qua RPC o day, doc lap voi revm, truoc khi fork ton tai.
    {
        let cached = app_state.nonce_cache.read().await.cached(raw.from, current_block);
        let expected = match cached {
            Some(n) => n,
            None => match transport::fetch_expected_nonce(&provider, raw.from).await {
                Ok(n) => {
                    app_state.nonce_cache.write().await.insert(raw.from, current_block, n);
                    n
                }
                Err(_) => return PipelineOutcome::Skip(pipeline::PipelineSkip::SimError),
            },
        };
        match transport::compare_nonce(raw.nonce, expected) {
            transport::NonceCheck::Ok => {}
            transport::NonceCheck::Stale => return PipelineOutcome::Skip(pipeline::PipelineSkip::NonceStale),
            transport::NonceCheck::Future => return PipelineOutcome::Skip(pipeline::PipelineSkip::NonceFuture),
        }
    }

    let quote_addr = match quote {
        pipeline::QuoteAsset::Wbnb => venues::wbnb_addr(),
        pipeline::QuoteAsset::Usdt => venues::usdt_addr(),
    };

    // C3 — TAX GATE: token allowlist zero-tax thì bỏ đo; ngược lại cache miss
    // (theo TTL) thì đo NGAY bằng EVM. Kết quả (Send) lấy ra khỏi khối fork.
    let is_allowlisted = venues::is_zero_tax_allowlisted(token);
    let cached = app_state.tax_cache.read().await.get_fresh_ttl(token, quote_addr, cfg.tax_cache_ttl());
    let max_tax_bps = cfg.max_roundtrip_tax_bps();

    // Toàn bộ EVM (đo tax + refine) nằm trong 1 khối đồng bộ, KHÔNG await, fork
    // drop cuối khối. Trả ra: (outcome_evm, tax_để_ghi_cache, số_liệu_validate).
    struct EvmProducts {
        outcome: PipelineOutcome,
        tax_to_cache: Option<bsc_sandwich::sim_evm::EvmTaxMeasurement>,
        evm_decision: pipeline::EvmDecision,
    }
    // Fork EVM (!Send) mở trực tiếp làm scrutinee của `match` — KHÔNG bind vào
    // 1 `let` riêng: nếu bind, state machine async coi binding đó "có thể còn
    // sống" tới `.await` ghi cache bên dưới -> future !Send (dù logic đã drop).
    // Là scrutinee, temporary bị drop ngay cuối `match`, trước mọi `.await`.
    let products: Result<EvmProducts, String> = match bsc_sandwich::sim_evm::BlockForkCache::open(provider, current_block).await {
        Err(e) => Err(e.to_string()),
        Ok(mut fork) => {
            // (a) tax
            let (tax_gate_skip, tax_to_cache) = if is_allowlisted || cached.is_some() {
                let tax = cached.map(|m| m.roundtrip_tax_bps).unwrap_or(0);
                (tax > max_tax_bps, None)
            } else {
                let (res, _ms) = fork.measure_tax_cached(token, quote_addr, probe_in_for_quote(quote));  // U256
                match res {
                    Ok(m) => {
                        let bps = bsc_sandwich::tax::combine_roundtrip_bps(m.buy_bps, m.sell_bps);
                        (m.honeypot || bps > max_tax_bps, Some(m))
                    }
                    // Khong do duoc tax bang EVM -> coi nhu honeypot_or_tax
                    // (an toan: chua chung minh duoc an toan thi khong sim).
                    Err(_) => (true, None),
                }
            };
            if tax_gate_skip {
                Ok(EvmProducts {
                    outcome: PipelineOutcome::Skip(pipeline::PipelineSkip::HoneypotOrTax),
                    tax_to_cache,
                    evm_decision: pipeline::EvmDecision { outcome: PipelineOutcome::Skip(pipeline::PipelineSkip::HoneypotOrTax), evm: None, tried: 0, total_ms: 0.0 },
                })
            } else {
                // (b) B3.2 — quyet dinh bang EVM that
                let d = pipeline::decide_with_evm(&mut fork, cfg, token, raw, prior, quote);
                Ok(EvmProducts { outcome: d.outcome.clone(), tax_to_cache, evm_decision: d })
            }
        }
    };

    match products {
        Err(_) => PipelineOutcome::Skip(pipeline::PipelineSkip::SimError),
        Ok(p) => {
            // Ghi cache tax (neu vua do duoc bang EVM) - await SAU khi fork da drop.
            if let Some(m) = p.tax_to_cache {
                let mut cache = app_state.tax_cache.write().await;
                cache.insert_for_quote(token, quote_addr, bsc_sandwich::tax::TaxMeasurement::from_evm(m, current_block));
            }
            pipeline::log_sim_evm(&app_state.logger, raw.from, token, current_block, &p.evm_decision, quote);
            // Cum `exec-path-traps` (F-26) - build CHI SAU KHI EVM da xac
            // nhan Simulated that (evm.is_some()) - diem build DUY NHAT cho
            // duong sim_engine=evm (decide_and_build_paper_v2 da NGUNG build
            // som cho duong nay, xem pipeline.rs).
            pipeline::build_paper_txs_from_evm_decision(&app_state.logger, cfg, raw.from, token, &p.evm_decision);
            // B3.4 - validator nhung: chi cho quote WBNB (predict_victim_swap_out
            // hien dung cho pool V2 WBNB; USDT ghi CON NO). Spawn khi EVM da
            // chay that (evm_decision co so lieu), de do do chinh xac song.
            if matches!(quote, pipeline::QuoteAsset::Wbnb) && p.evm_decision.evm.is_some() {
                spawn_victim_validator(app_state.clone(), raw.clone(), token);
            }
            p.outcome
        }
    }
}

/// Cụm `evm-validate-fixed-then-wire` (B3.4) — VALIDATOR NHÚNG: theo dõi 1
/// victim đã có `sim.evm`, chờ nó lên block (≤5 block), so `victim_out` DỰ
/// ĐOÁN (EVM replay tại block cha) với `victim_out` THẬT trong receipt, kiểm
/// tra cô lập (pair chỉ có đúng 1 Swap của tx đó trong block). Ghi
/// `validate.victim` + cập nhật `/api/validate`. Đây là bản THAY THẾ LÂU DÀI
/// cho gate B4''.3(b) (dùng CHÍNH cơ chế đã đạt 0% lệch ở B4''.2), chạy ở CẢ
/// paper lẫn live. Chỉ spawn cho quote WBNB (đường predict_victim_swap_out
/// hiện dựng cho pool V2 WBNB; USDT ghi CÒN NỢ).
fn spawn_victim_validator(app_state: AppState, victim: PendingTxRaw, token: Address) {
    tokio::spawn(async move {
        use bsc_sandwich::sim_evm::{decode_v2_swap_amount_out, predict_victim_swap_out, swap_topic0};
        // Cum A5 - viec NEN: dung RPC nen (BSC_HTTP_BG), khong tranh chap
        // ket noi voi duong nong.
        let provider = match bg_provider_or_hot(&app_state).await {
            Some((p, _)) => p,
            None => return,
        };
        let factory = match Address::from_str(V2_FACTORY_ADDRESS) {
            Ok(a) => a,
            Err(_) => return,
        };
        let pair = match bsc_sandwich::pool::resolve_v2_pair(&provider, factory, token).await {
            Ok(Ok(p)) => p,
            _ => return,
        };
        let (token0, _r0, _r1) = match bsc_sandwich::pool::get_raw_reserves_and_token0(&provider, pair).await {
            Ok(v) => v,
            Err(_) => return,
        };
        let token_is_token0 = token0 == token;
        let swap_topic = swap_topic0();

        // Cho toi da ~5 block (~5s BSC) de victim len block.
        let mut mined_block = None;
        for _ in 0..8u32 {
            if let Ok(Some(r)) = provider.get_transaction_receipt(victim.hash).await {
                mined_block = r.block_number;
                break;
            }
            tokio::time::sleep(Duration::from_millis(1500)).await;
        }
        let Some(block_n) = mined_block else { return };
        if block_n == 0 {
            return;
        }

        // victim_out_real tu receipt cua chinh victim.
        let receipt = match provider.get_transaction_receipt(victim.hash).await {
            Ok(Some(r)) => r,
            _ => return,
        };

        // Cum `exec-path-traps` (F-04, muc b) - victim tx REVERT that
        // (receipt.status()==false) la tin hieu "loss" NGAY, khong can doi
        // toi buoc so pred/real (tx revert thi khong sinh Swap log nao ca ->
        // nhanh duoi day se return som, mat dau vet neu khong bat o day
        // TRUOC). Log rieng 1 dong `validate.victim` rut gon (khong co
        // pred/real vi khong co Swap that de so) roi return, KHONG tiep tuc
        // xuong buoc kiem tra co lap/du doan (khong con y nghia gi voi tx da
        // revert).
        if !receipt.status() {
            app_state.risk_guard.write().await.record_result(true);
            app_state.logger.log(
                "validate.victim",
                serde_json::json!({
                    "hash": format!("{:#x}", victim.hash),
                    "pair": format!("{:#x}", pair),
                    "block": block_n,
                    "victim_reverted": true,
                    "risk_guard_is_loss": true,
                }),
            );
            return;
        }

        let mut victim_out_real = None;
        for log in receipt.inner.logs() {
            if log.address() == pair && log.topics().first() == Some(&swap_topic) {
                victim_out_real = decode_v2_swap_amount_out(log.data().data.as_ref(), token_is_token0);
                break;
            }
        }
        let Some(victim_out_real) = victim_out_real else { return };

        // Kiem tra co lap: DUNG 1 loi goi eth_getLogs.
        use alloy::rpc::types::eth::Filter;
        let filter = Filter::new().address(pair).event_signature(swap_topic).from_block(block_n).to_block(block_n);
        let isolated = match provider.get_logs(&filter).await {
            Ok(logs) => logs.len() == 1 && logs[0].transaction_hash == Some(victim.hash),
            Err(_) => return,
        };

        // Du doan bang EVM tai block cha.
        let pred = match predict_victim_swap_out(provider.clone(), block_n - 1, &victim, token, pair, token_is_token0).await {
            Ok(p) => p,
            Err(_) => return,
        };
        let Some(pred_out) = pred.swap_out else { return };

        let diff = if pred_out > victim_out_real { pred_out - victim_out_real } else { victim_out_real - pred_out };
        let base = victim_out_real.max(alloy::primitives::U256::from(1u64));
        let pct = (u128::try_from(diff).unwrap_or(u128::MAX) as f64) / (u128::try_from(base).unwrap_or(1) as f64) * 100.0;

        // Cum `exec-path-traps` (F-04, muc b) - wire RiskGuard::record_result
        // O DUONG PAPER: chua co giao dich that (dry_run=true, khong ky/gui
        // gi) nen KHONG co "lo that" theo nghia tien that mat - dung KET QUA
        // VALIDATOR (validate.victim, cung co che da dat 0% lech tren block
        // co lap o B4''.2) lam tin hieu thay the de bo dem RiskGuard con
        // SONG (F-04: "consecutive_loss vinh vien = 0 vi khong ai goi
        // record_result"). Nhanh victim revert that da bat rieng o tren
        // (return som) - o day chi con truong hop victim THANH CONG, "loss"
        // = sim lech qua nguong (>1%, khong khop thuc te dung theo lenh).
        let is_loss = pct > 1.0;
        app_state.risk_guard.write().await.record_result(is_loss);

        let row = serde_json::json!({
            "hash": format!("{:#x}", victim.hash),
            "pair": format!("{:#x}", pair),
            "block": block_n,
            "pred": pred_out.to_string(),
            "real": victim_out_real.to_string(),
            "lech_pct": (pct * 10000.0).round() / 10000.0,
            "isolated": isolated,
            "block_delta": block_n.saturating_sub(0),
            "victim_reverted": false,
            "risk_guard_is_loss": is_loss,
        });
        app_state.logger.log("validate.victim", row.clone());
        // Cum `real-economics-mode2` (F-27) - truyen rieng `isolated`/`pct`
        // (khong con gop san 1 co `ok`) de ValidateStats tu phan nhom dung -
        // fix bug audit (within_1pct cu chi dem dong isolated=true).
        app_state.validate_log.write().await.push(row, isolated, pct);
    });
}

/// C1/C3 — `probe_in` để đo tax: 0.05 BNB cho quote WBNB, 50 USDT cho quote
/// USDT (đủ lớn để đo tax ổn định, đủ nhỏ để không kẹt thanh khoản pool nhỏ).
fn probe_in_for_quote(quote: pipeline::QuoteAsset) -> alloy::primitives::U256 {
    match quote {
        pipeline::QuoteAsset::Wbnb => alloy::primitives::U256::from(50_000_000_000_000_000u128), // 0.05 BNB
        pipeline::QuoteAsset::Usdt => alloy::primitives::U256::from(50_000_000_000_000_000_000u128), // 50 USDT (18 dp)
    }
}

/// Cụm "usdt-quote-asset" (BAOCAO29) — hàm THUẦN tách riêng để unit-test
/// được (không cần dựng cả `handle_paper_tx`/`AppState`): `Ok(token)` từ
/// `precheck_token_only` nghĩa là decode + xác định chiều MUA thành công
/// (`token_a == WBNB`) — token đã BIẾT THẬT, dù bước SAU (resolve pool/tax
/// cache/sim) có skip vì lý do gì (`no_pool`/`honeypot_or_tax`/`thin_liq`/...)
/// thì vẫn nên log đúng token đó, không phải `null`. Chỉ giữ `None` khi
/// CHÍNH bước decode này thất bại (`decode_fail`/`not_wbnb_pair` — địa chỉ
/// token chưa từng xác định được).
fn token_hint_from_precheck(precheck: Result<Address, pipeline::PipelineSkip>) -> Option<Address> {
    precheck.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Cụm `exec-path-traps` (V-06) — halt_transition =====

    #[test]
    fn halt_transition_false_to_true_is_triggered() {
        assert_eq!(halt_transition(false, true), HaltTransition::Triggered);
    }

    #[test]
    fn halt_transition_true_to_false_is_cleared() {
        assert_eq!(halt_transition(true, false), HaltTransition::Cleared);
    }

    #[test]
    fn halt_transition_unchanged_is_none() {
        assert_eq!(halt_transition(false, false), HaltTransition::None);
        assert_eq!(halt_transition(true, true), HaltTransition::None);
    }

    // ===== Cụm `strategy-lock-mode2` — đường nóng KHÔNG đo tax bằng EVM khi
    // sim_engine="v2" =====

    /// ĐẠT CẦN DÁN (lệnh `strategy-lock-mode2`, mục 4) — ship `config.toml`
    /// PHẢI có `sim_engine="v2"`. Đây là gate DUY NHẤT trong `handle_paper_tx`
    /// quyết định có gọi `run_evm_decision` hay không
    /// (`if cfg.sim_engine_is_evm() { ... run_evm_decision(...).await ... }`,
    /// xem trên) — `run_evm_decision` là nơi DUY NHẤT trên đường nóng có thể
    /// dẫn tới đo tax bằng EVM (`fork.measure_tax_cached`). `sim_engine="v2"`
    /// -> nhánh đó KHÔNG BAO GIỜ chạy -> số lần gọi đo-tax-bằng-EVM trên
    /// đường nóng = 0 một cách CẤU TRÚC (không phụ thuộc dữ liệu/tx nào tới),
    /// khác hẳn `measure_tax_evm` (hàm độc lập) chỉ còn 2 call site: test và
    /// `pairs_vet_task` (nền, không phải đường nóng `handle_paper_tx`).
    #[test]
    fn ship_config_sim_engine_v2_means_hot_path_never_opens_evm_fork() {
        let cfg = Config::from_str(include_str!("../config.toml")).unwrap();
        assert_eq!(cfg.sim_engine, "v2", "cum strategy-lock-mode2: ship PHAI la \"v2\"");
        assert!(!cfg.sim_engine_is_evm(), "sim_engine_is_evm() la gate DUY NHAT truoc run_evm_decision trong handle_paper_tx");
    }

    /// ĐẠT CẦN DÁN (lệnh exec-path-traps, mục 12/V-06): `halt_watch_task`
    /// chạy thật với `interval` cực ngắn trên `StateFiles` tạm — tạo
    /// `halt.lock` giữa chừng, chờ task log ĐÚNG 1 dòng `halt.triggered`, xoá
    /// file, chờ ĐÚNG 1 dòng `halt.cleared` — không lặp lại dù nhiều tick
    /// trôi qua trong lúc file không đổi trạng thái.
    #[tokio::test]
    async fn halt_watch_task_logs_triggered_then_cleared_exactly_once_each() {
        let dir = tempfile::tempdir().unwrap();
        let logger = std::sync::Arc::new(BotLogger::new(dir.path().join("logs").join("bot.jsonl")).unwrap());
        let state_files = std::sync::Arc::new(StateFiles::new(dir.path().join("state")).unwrap());
        let cfg = Config::from_str(include_str!("../config.toml")).unwrap();
        let app_state: AppState = std::sync::Arc::new(AppStateInner {
            config: RwLock::new(cfg),
            victims: RwLock::new(VictimBook::new()),
            pairbook: RwLock::new(PairBook::new()),
            state_files: state_files.clone(),
            logger: logger.clone(),
            bot_state: RwLock::new(BotState::Watching),
            start_time: std::time::Instant::now(),
            boot_wall_clock: chrono::Utc::now(),
            skip_counts: RwLock::new(HashMap::new()),
            last_block: RwLock::new(None),
            provider: RwLock::new(None),
            tax_cache: RwLock::new(TaxCache::new()),
            validate_log: RwLock::new(bsc_sandwich::web::ValidateStats::default()),
            pending_semaphore: std::sync::Arc::new(Semaphore::new(4)),
            pending_source: RwLock::new(transport::PendingSource::InjectOnly),
            risk_guard: RwLock::new(RiskGuard::new()),
            nonce_cache: RwLock::new(transport::NonceCache::new()),
            funnel: FunnelCounters::new(),
            gas_oracle: transport::GasOracle::new(),
            gas_units: RwLock::new((160_000, 140_000)),
            reserve_cache: RwLock::new(transport::ReserveCache::new()),
        pairs_first_reload_done: Arc::new(tokio::sync::Notify::new()),
        sim_provider: RwLock::new(None),
        bg_provider: RwLock::new(None),
        mined_index: RwLock::new(transport::MinedTxIndex::new()),
        self_nonce: RwLock::new(transport::SelfNonceCache::new()),
        competitor_cluster: RwLock::new(bsc_sandwich::competitor::ClusterIndex::new()),
        candidate_seen: RwLock::new(HashMap::new()),
        seen_hashes: RwLock::new(transport::SeenHashSet::new()),
        compete_stats: bsc_sandwich::web::CompeteStats::new(),
        shadow_wallet: None,
        });

        let task_state = app_state.clone();
        let handle = tokio::spawn(async move {
            halt_watch_task(task_state, Duration::from_millis(20)).await;
        });

        // Cho vai tick troi qua khi CHUA halt - khong duoc log gi.
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(logger.tail(50).iter().all(|v| v["event"] != "halt.triggered"));

        state_files.request_halt().unwrap();
        // Cho toi da 2s de task nhan ra (interval 20ms, du du).
        let mut saw_triggered = false;
        for _ in 0..100u32 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            if logger.tail(50).iter().any(|v| v["event"] == "halt.triggered") {
                saw_triggered = true;
                break;
            }
        }
        assert!(saw_triggered, "phai thay halt.triggered sau khi tao halt.lock");
        assert_eq!(*app_state.bot_state.read().await, BotState::Stopped);

        // Cho them vai tick trong luc VAN halt - khong duoc log lap lai.
        tokio::time::sleep(Duration::from_millis(80)).await;
        let triggered_count = logger.tail(50).iter().filter(|v| v["event"] == "halt.triggered").count();
        assert_eq!(triggered_count, 1, "khong duoc log halt.triggered lap lai khi van dang halt");

        state_files.clear_halt().unwrap();
        let mut saw_cleared = false;
        for _ in 0..100u32 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            if logger.tail(50).iter().any(|v| v["event"] == "halt.cleared") {
                saw_cleared = true;
                break;
            }
        }
        assert!(saw_cleared, "phai thay halt.cleared sau khi xoa halt.lock");
        assert_eq!(*app_state.bot_state.read().await, BotState::Watching);

        handle.abort();
    }

    /// ĐẠT CẦN DÁN (lệnh usdt-quote-asset, mục 6 FIX LOGGER): token KHÔNG
    /// còn `null` sai khi decode/precheck đã biết token thật.
    #[test]
    fn token_hint_from_precheck_keeps_token_when_decode_succeeded() {
        let token = Address::from_str("0xcccccccccccccccccccccccccccccccccccccccc").unwrap();
        assert_eq!(token_hint_from_precheck(Ok(token)), Some(token));
    }

    /// Chỉ `None` khi CHÍNH bước decode thất bại — token chưa từng biết được.
    #[test]
    fn token_hint_from_precheck_is_none_only_when_decode_itself_failed() {
        assert_eq!(token_hint_from_precheck(Err(pipeline::PipelineSkip::DecodeFail)), None);
        assert_eq!(token_hint_from_precheck(Err(pipeline::PipelineSkip::NotWbnbPair)), None);
    }

    // ===== Cụm `foundation-fix-then-real-sim` (A4/A6) — `passes_router_gate`/
    // `record_funnel_terminal` =====

    fn addr(hex: &str) -> Address {
        Address::from_str(hex).unwrap()
    }

    /// Cụm `exec-path-traps` (F-15) — sửa lại ngữ nghĩa: `to=None` giờ CHỈ
    /// còn `true` khi `source=="inject"` (định dạng cũ `state/inject_tx.jsonl`,
    /// cố ý không có cột `to`) — tên test đổi từ "unknown_source" (KHÔNG phân
    /// biệt nguồn) sang "inject_source" (CHỈ 1 nguồn cụ thể) để khớp hành vi
    /// mới, đúng ĐẠT CẦN DÁN của lệnh này.
    #[test]
    fn passes_router_gate_true_for_none_only_when_source_is_inject() {
        assert!(passes_router_gate(None, "inject"));
    }

    /// F-15 — WS/`txpool_content` thật KHÔNG bao giờ nên nhận `to=None` mà
    /// được cho qua nữa: xiết chặt về `false` (`not_pancake_router`).
    #[test]
    fn passes_router_gate_false_for_none_when_source_is_ws_or_txpool() {
        assert!(!passes_router_gate(None, "pending_ws"));
        assert!(!passes_router_gate(None, "txpool"));
    }

    #[test]
    fn passes_router_gate_true_for_pinned_router_false_for_others() {
        assert!(passes_router_gate(Some(addr(bsc_sandwich::venues::V2_ROUTER_ADDRESS)), "pending_ws"));
        let biswap_like = addr("0x3a6d8cA21D1CF76F653A67577FA0D27453350dD8");
        assert!(!passes_router_gate(Some(biswap_like), "pending_ws"));
    }

    /// ĐẠT CẦN DÁN (lệnh A4, test bắt buộc) — tx tới router giả (không nằm
    /// trong 5 router Pancake đã pin) mang selector `swapExactETHForTokens`
    /// (`0x7ff36ab5`) thật vẫn phải bị gate (a) chặn — chứng minh gate CHỈ
    /// nhìn `to`, không quan tâm selector trông "hợp lệ" tới đâu.
    #[test]
    fn passes_router_gate_rejects_fake_router_even_with_real_v2_selector() {
        let fake_router = addr("0x3a6d8cA21D1CF76F653A67577FA0D27453350dD8"); // Biswap-style, chua pin
        assert!(!passes_router_gate(Some(fake_router), "txpool"));
    }

    /// ĐẠT CẦN DÁN — 1000 tx giả, đúng 5 tới V2 Router đã pin, phần còn lại
    /// tới địa chỉ random không thuộc registry -> ĐÚNG 5 tx "đáng spawn"
    /// (qua gate (a)) — dùng CHÍNH `passes_router_gate` mà
    /// `poll_txpool_pending`/`subscribe_pending_txs` gọi thật, nên kết quả
    /// đúng bằng số tx thực sự được `tokio::spawn(handle_paper_tx(...))`.
    #[test]
    fn poll_prefilter_1000_fake_tx_5_to_v2_router_exactly_5_pass_gate() {
        let v2_router = addr(bsc_sandwich::venues::V2_ROUTER_ADDRESS);
        let mut tos: Vec<Option<Address>> = Vec::with_capacity(1000);
        for i in 0..1000u32 {
            if i % 200 == 0 {
                // 0, 200, 400, 600, 800 -> dung 5 phan tu
                tos.push(Some(v2_router));
            } else {
                // dia chi gia, khong nam trong PANCAKE_ROUTERS (thay 1 byte
                // theo i de moi dia chi khac nhau, van chac chan khong trung
                // 5 router da pin).
                let hex = format!("0x{:040x}", 0x1000_0000u64 + i as u64);
                tos.push(Some(Address::from_str(&hex).unwrap()));
            }
        }
        let passed = tos.into_iter().filter(|to| passes_router_gate(*to, "txpool")).count();
        assert_eq!(passed, 5);
    }

    #[test]
    fn record_funnel_terminal_simulated_increments_simulated_bucket() {
        let funnel = bsc_sandwich::web::FunnelCounters::new();
        let quote = bsc_sandwich::sim_v2::SandwichQuote {
            front_in: alloy::primitives::U256::from(1u64),
            front_out: alloy::primitives::U256::ZERO,
            victim_out: alloy::primitives::U256::ZERO,
            back_out: alloy::primitives::U256::from(2u64),
            profit_wei: 1,
        };
        record_funnel_terminal(&funnel, &PipelineOutcome::Simulated(quote));
        let snap = funnel.snapshot();
        assert_eq!(snap["simulated"], 1);
        assert_eq!(snap["decode_fail"], 0);
    }

    #[test]
    fn record_funnel_terminal_venue_unpinned_increments_venue_v3() {
        let funnel = bsc_sandwich::web::FunnelCounters::new();
        record_funnel_terminal(&funnel, &PipelineOutcome::Skip(pipeline::PipelineSkip::VenueUnpinned));
        assert_eq!(funnel.snapshot()["venue_v3"], 1);
    }

    #[test]
    fn record_funnel_terminal_maps_each_terminal_skip_to_its_own_bucket() {
        let cases: &[(pipeline::PipelineSkip, &str)] = &[
            (pipeline::PipelineSkip::DecodeFail, "decode_fail"),
            (pipeline::PipelineSkip::NotWbnbPair, "not_wbnb_pair"),
            (pipeline::PipelineSkip::NotQuotePair, "not_wbnb_pair"),
            (pipeline::PipelineSkip::SellDirection, "not_wbnb_pair"),
            (pipeline::PipelineSkip::NoPool, "no_pool"),
            (pipeline::PipelineSkip::RpcError, "rpc_error"),
            (pipeline::PipelineSkip::BelowMin, "below_min"),
            (pipeline::PipelineSkip::ThinLiq, "thin_liq"),
            (pipeline::PipelineSkip::HoneypotOrTax, "honeypot_or_tax"),
            (pipeline::PipelineSkip::Unprofitable, "unprofitable"),
            (pipeline::PipelineSkip::VictimWouldRevert, "victim_would_revert"),
        ];
        for (reason, bucket) in cases {
            let funnel = bsc_sandwich::web::FunnelCounters::new();
            record_funnel_terminal(&funnel, &PipelineOutcome::Skip(*reason));
            assert_eq!(funnel.snapshot()[*bucket], 1, "{reason:?} phai roi dung bucket {bucket}");
        }
    }
}

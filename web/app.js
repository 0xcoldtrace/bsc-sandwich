async function getJSON(url) {
  const res = await fetch(url);
  return res.json();
}

function setBadge(status) {
  const badge = document.getElementById("mode-badge");
  if (status.dry_run) {
    badge.textContent = "DRY_RUN";
    badge.className = "badge dry";
  } else if (!status.live_gate.allow_live || !status.live_gate.bot_armed) {
    badge.textContent = "LIVE_BLOCKED";
    badge.className = "badge blocked";
  } else {
    badge.textContent = "LIVE_ARMED";
    badge.className = "badge armed";
  }
}

function renderBotKv(status) {
  const rows = [
    ["state", status.state],
    ["uptime_sec", status.uptime_sec],
    ["last_block", status.last_block === null ? "MISSING (chua co WSS)" : status.last_block],
    ["chain_id", status.chain_id],
    ["dry_run", status.dry_run],
    ["allow_live", status.allow_live],
    ["bot_armed", status.bot_armed],
    ["halt.lock", status.halt_lock],
    ["pending_source", status.pending_source],
    ["max_front_bnb", status.max_front_bnb],
    ["max_exposure_bnb", status.max_exposure_bnb === 0 ? "0 (tat cap)" : status.max_exposure_bnb],
    ["min_profit_bnb", status.min_profit_bnb],
  ];
  const tbl = document.getElementById("bot-kv");
  tbl.innerHTML = rows.map(([k, v]) => `<tr><td>${k}</td><td>${v}</td></tr>`).join("");
}

function renderGate(status) {
  const gate = status.live_gate;
  const list = document.getElementById("gate-list");
  list.innerHTML = Object.entries(gate)
    .map(([k, v]) => `<li class="${v ? "ok" : "bad"}">${k}: ${v ? "OK" : "THIEU"}</li>`)
    .join("");
}

function renderVenues(data) {
  const body = document.getElementById("venues-body");
  body.innerHTML = data.venues
    .map((v) => {
      const contracts = v.contracts && v.contracts.length ? v.contracts : [null];
      return contracts
        .map(
          (c) => `<tr>
        <td>${v.family}</td>
        <td>${c ? c.name : "-"}</td>
        <td>${c ? c.address : "-"}</td>
        <td>${v.pinned ? "yes" : "no"}</td>
        <td>${v.scan_enabled}</td>
        <td>${v.live_enabled}</td>
        <td>${c && c.get_code_len !== null ? c.get_code_len : "MISSING"}</td>
        <td>${v.status}</td>
        <td>${c ? c.source_url : "-"}</td>
      </tr>`
        )
        .join("");
    })
    .join("");
}

function renderVictims(data) {
  document.getElementById(
    "victims-meta"
  ).textContent = `count=${data.count} error_lines=${data.error_lines} last_reload_sec_ago=${data.last_reload_sec_ago ?? "MISSING"}`;
  const body = document.getElementById("victims-body");
  body.innerHTML = data.victims
    .map((v) => `<tr><td>${v.address}</td><td>${v.min_swap_bnb}</td></tr>`)
    .join("");
}

function renderPairs(data) {
  document.getElementById(
    "pairs-meta"
  ).textContent = `count=${data.count} error_lines=${data.error_lines} last_reload_sec_ago=${data.last_reload_sec_ago ?? "MISSING"}`;
  const body = document.getElementById("pairs-body");
  body.innerHTML = data.pairs
    .map((p) => `<tr><td>${p.pair_addr}</td><td>${p.source_line}</td><td>${p.resolved_from}</td></tr>`)
    .join("");
}

function renderHits(data) {
  const body = document.getElementById("hits-body");
  const rows = [...data.hits].reverse();
  body.innerHTML = rows
    .map((h) => {
      const { ts, event, ...rest } = h;
      return `<tr><td>${ts ?? ""}</td><td>${event ?? ""}</td><td>${JSON.stringify(rest)}</td></tr>`;
    })
    .join("");
}

function renderSkips(data) {
  const body = document.getElementById("skips-body");
  body.innerHTML = Object.entries(data)
    .map(([k, v]) => `<tr><td>${k}</td><td>${v}</td></tr>`)
    .join("");
}

function renderFunnel(data) {
  const order = [
    "seen",
    "not_pancake_router",
    "decode_fail",
    "not_wbnb_pair",
    "venue_v3",
    "venue_v2",
    "no_pool",
    "rpc_error",
    "below_min",
    "thin_liq",
    "honeypot_or_tax",
    "unprofitable",
    "victim_would_revert",
    "simulated",
    "sim_error",
  ];
  const body = document.getElementById("funnel-body");
  body.innerHTML = order.map((k) => `<tr><td>${k}</td><td>${data[k] ?? 0}</td></tr>`).join("");
}

function renderTaxCache(data) {
  document.getElementById(
    "tax-cache-meta"
  ).textContent = `allow_tax_inject=${data.allow_tax_inject} current_block=${data.current_block} tax_cache_ttl_sec=${data.tax_cache_ttl_sec ?? "?"} (don vi bps)`;
  const body = document.getElementById("tax-cache-body");
  body.innerHTML = data.entries
    .map(
      (e) =>
        `<tr><td>${e.token}</td><td>${e.quote ?? "?"}</td><td>${e.roundtrip_tax_bps}</td><td>${e.buy_bps ?? "-"}/${e.sell_bps ?? "-"}</td><td class="${e.honeypot ? "bad" : ""}">${e.honeypot ? "HONEYPOT" : ""}</td><td class="${e.fresh ? "ok" : "bad"}">${e.fresh ? "fresh" : "stale"}</td></tr>`
    )
    .join("");
}

// Cụm real-economics-mode2 (mục 3) — GET /api/econ.
function fmtBnbMaybe(v) {
  return v === null || v === undefined ? "-" : Number(v).toFixed(6);
}

function renderEcon(data) {
  const summary = document.getElementById("econ-summary");
  if (summary) {
    summary.textContent = data.summary_line || "";
  }
  const body = document.getElementById("econ-buckets-body");
  if (body) {
    body.innerHTML = (data.buckets_bnb || [])
      .map(
        (b) =>
          `<tr><td>${b.bucket}</td><td>${b.count}</td><td>${b.gross_pos}</td><td>${b.net_pos}</td><td>${fmtBnbMaybe(b.sum_net_pos_bnb)}</td><td>${fmtBnbMaybe(b.best_net_bnb)}</td><td>${fmtBnbMaybe(b.median_gas_cost_bnb)}</td></tr>`
      )
      .join("");
  }
  const decodeFailEl = document.getElementById("econ-decode-fail");
  if (decodeFailEl) {
    const byRouter = data.decode_fail_by_router || {};
    const parts = Object.entries(byRouter)
      .sort((a, b) => b[1] - a[1])
      .map(([name, count]) => `${name}=${count}`)
      .join(", ");
    const byQuote = Object.entries(data.by_quote || {})
      .map(([q, c]) => `${q}=${c}`)
      .join(", ");
    decodeFailEl.textContent = `decode_fail theo router: ${parts || "(chưa có)"} | by_quote: ${byQuote || "(chưa có)"}`;
  }
}

// Cụm evm-validate-fixed-then-wire (B3.4) — validator nhung song.
// Cụm real-economics-mode2 (F-27) — tach isolated/non_isolated (fix bug audit:
// within_1pct cu chi dem dong isolated:true).
function renderValidate(data) {
  const meta = document.getElementById("validate-meta");
  if (meta) {
    const ratio = (data.within_1pct_ratio * 100).toFixed(2);
    const iso = data.isolated || {};
    const nonIso = data.non_isolated || {};
    const isoRatio = ((iso.within_1pct_ratio || 0) * 100).toFixed(2);
    const nonIsoRatio = ((nonIso.within_1pct_ratio || 0) * 100).toFixed(2);
    meta.textContent =
      `total=${data.total} within_1pct=${data.within_1pct} (${ratio}%) | ` +
      `isolated: n=${iso.n || 0} within_1pct=${iso.within_1pct || 0} (${isoRatio}%) p50=${iso.p50_lech_pct ?? "-"}% p95=${iso.p95_lech_pct ?? "-"}% | ` +
      `non_isolated: n=${nonIso.n || 0} within_1pct=${nonIso.within_1pct || 0} (${nonIsoRatio}%) p50=${nonIso.p50_lech_pct ?? "-"}% p95=${nonIso.p95_lech_pct ?? "-"}%`;
  }
  const body = document.getElementById("validate-body");
  if (body) {
    body.innerHTML = (data.rows || [])
      .slice()
      .reverse()
      .map(
        (r) =>
          `<tr><td>${r.hash}</td><td>${r.block}</td><td>${r.lech_pct}%</td><td class="${r.isolated ? "ok" : "bad"}">${r.isolated ? "iso" : "multi"}</td></tr>`
      )
      .join("");
  }
}

async function refresh() {
  try {
    const status = await getJSON("/api/status");
    setBadge(status);
    renderBotKv(status);
    renderGate(status);
  } catch (e) {
    console.error("status fetch failed", e);
  }
  try {
    renderVenues(await getJSON("/api/venues"));
  } catch (e) {
    console.error("venues fetch failed", e);
  }
  try {
    renderVictims(await getJSON("/api/victims"));
  } catch (e) {
    console.error("victims fetch failed", e);
  }
  try {
    renderPairs(await getJSON("/api/pairs"));
  } catch (e) {
    console.error("pairs fetch failed", e);
  }
  try {
    renderHits(await getJSON("/api/hits?limit=50"));
  } catch (e) {
    console.error("hits fetch failed", e);
  }
  try {
    renderSkips(await getJSON("/api/skips"));
  } catch (e) {
    console.error("skips fetch failed", e);
  }
  try {
    renderTaxCache(await getJSON("/api/tax"));
  } catch (e) {
    console.error("tax cache fetch failed", e);
  }
  try {
    renderFunnel(await getJSON("/api/funnel"));
  } catch (e) {
    console.error("funnel fetch failed", e);
  }
  try {
    renderEcon(await getJSON("/api/econ"));
  } catch (e) {
    console.error("econ fetch failed", e);
  }
  try {
    renderValidate(await getJSON("/api/validate"));
  } catch (e) {
    console.error("validate fetch failed", e);
  }
}

refresh();
setInterval(refresh, 5000);

#!/usr/bin/env python3
"""Cụm `verify-cluster-as-victim` (mục 2) — BẢNG QUYẾT ĐỊNH: kẹp cụm đối thủ
có lãi không, đo bằng EVM.

Đọc `logs/bot.jsonl` của một lần chạy shadow (`live_mode="shadow"` +
`allow_competitor_victims=true`) và ghép 3 nguồn theo `victim_hash`:

  `sim.result`        — đường nóng V2-math: `amount_out_min_wei`,
                        `victim_out_no_front_wei` (KHÔNG có front — tử số của
                        `room`), `front_in_wei` (đã gated theo mục 2 của
                        BAOCAO44), `profit_net_wei`, `victim_ok_v2`, `pair`
  `shadow.sim`        — revm fork tại `decision_block − 1`, 3 chân THẬT:
                        `profit_sim_wei`, `victim_ok`
  `bundle.shadow_econ`— bribe + net sau bribe của chính bundle đã ký

Tách CỤM (`victim_in_competitor_cluster=true`) khỏi NGOÀI CỤM và in bảng theo
pool: `n`, `%victim_ok_evm`, p50/p90 `profit_sim`, tổng, quy ra mỗi giờ.

Đơn vị: `profit_sim_native` là đơn vị CỦA QUOTE pool đó (BNB hoặc USDT) —
KHÔNG cộng chéo 2 quote vào một con số.
"""
import argparse, json, collections, datetime as dt


def pct(v, q):
    if not v:
        return None
    s = sorted(v)
    return s[min(len(s) - 1, int(q * (len(s) - 1)))]


def ts(s):
    return dt.datetime.fromisoformat(s).timestamp()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--log", required=True)
    ap.add_argument("--label", default="")
    ap.add_argument("--cluster-from-scan", default="",
                    help="output cua scripts/cluster_tx_scan.py - NGUON SU THAT cho 'tx nay co phai cua cum'. "
                         "Co that nay vi co `victim_in_competitor_cluster` cua bot LUON false cho mau hinh "
                         "'cap von + swap trong CUNG block': luc bot quyet dinh, block do chua duoc dao nen "
                         "log Transfer cap von chua ton tai.")
    a = ap.parse_args()

    onchain_cluster = set()
    if a.cluster_from_scan:
        for line in open(a.cluster_from_scan):
            onchain_cluster.add(json.loads(line)["hash"].lower())
        print(f"nguon su that cum (on-chain): {len(onchain_cluster)} tx")
    sims, shadows, econs = {}, {}, {}
    t0 = t1 = None
    for line in open(a.log, errors="replace"):
        if '"sim.result"' not in line and '"shadow.sim"' not in line and '"bundle.shadow_econ"' not in line:
            continue
        try:
            d = json.loads(line)
        except Exception:  # noqa: BLE001
            continue
        e = d.get("event")
        t = ts(d["ts"])
        t0 = t if t0 is None else min(t0, t)
        t1 = t if t1 is None else max(t1, t)
        if e == "sim.result":
            sims[d["hash"].lower()] = d
        elif e == "shadow.sim":
            shadows[d["victim_hash"].lower()] = d
        elif e == "bundle.shadow_econ":
            econs[d["victim_hash"].lower()] = d
    hours = (t1 - t0) / 3600.0 if (t0 and t1 and t1 > t0) else 0.0
    print(f"== {a.label} ==  sim.result={len(sims)}  shadow.sim={len(shadows)}  "
          f"bundle.shadow_econ={len(econs)}  cua so={hours:.2f} h")

    rows = []
    for h, s in sims.items():
        sh = shadows.get(h)
        amin = int(s.get("amount_out_min_wei") or 0)
        vno = s.get("victim_out_no_front_wei")
        vno = int(vno) if vno else None
        rows.append({
            "hash": h,
            "cluster": (h in onchain_cluster) if onchain_cluster else bool(s.get("victim_in_competitor_cluster")),
            "cluster_bot_flag": bool(s.get("victim_in_competitor_cluster")),
            "pair": s.get("pair"),
            "quote": s.get("quote"),
            "amount_out_min": amin,
            "victim_out_no_front": vno,
            "room": (vno / amin) if (vno and amin) else None,
            "front_gated": int(s.get("front_in_wei") or 0) / 1e18,
            "profit_v2": int(s.get("profit_net_wei") or 0) / 1e18,
            "victim_ok_v2": s.get("victim_ok_v2"),
            "profit_sim": (sh.get("profit_sim_native") if sh and "profit_sim_native" in sh else None),
            "victim_ok_evm": (sh.get("victim_ok") if sh else None),
            "evm_err": (sh.get("error") or (sh.get("victim_revert_reason") if sh else None)) if sh else "KHONG_CO_shadow.sim",
        })

    if onchain_cluster:
        n_bot = sum(1 for r in rows if r["cluster_bot_flag"])
        n_chain = sum(1 for r in rows if r["cluster"])
        miss = sum(1 for r in rows if r["cluster"] and not r["cluster_bot_flag"])
        print(f"co CUM: bot nhan dien luc chay = {n_bot} ; on-chain that su = {n_chain} ; "
              f"bot BO SOT = {miss}")
    for grp, want in (("CUM DOI THU", True), ("NGOAI CUM", False)):
        sub = [r for r in rows if r["cluster"] == want]
        print(f"\n---- {grp}: {len(sub)} candidate Simulated ----")
        if not sub:
            print("   (khong co dong nao)")
            continue
        print(f"{'hash':20} {'quote':6} {'amountOutMin':>22} {'room':>9} {'front_gated':>12} "
              f"{'profit_v2':>12} {'profit_sim':>12} {'ok_evm':>7}")
        for r in sub[:40]:
            room = "-" if r["room"] is None else f"{r['room']:.4f}"
            psim = "-" if r["profit_sim"] is None else f"{r['profit_sim']:.6f}"
            print(f"{r['hash'][:18]:20} {str(r['quote']):6} {r['amount_out_min']:>22} {room:>9} "
                  f"{r['front_gated']:>12.6f} {r['profit_v2']:>12.6f} {psim:>12} "
                  f"{str(r['victim_ok_evm']):>7}")
        by_pool = collections.defaultdict(list)
        for r in sub:
            by_pool[(r["pair"], r["quote"])].append(r)
        print(f"\n  BANG THEO POOL ({grp}):")
        print(f"  {'pool':22} {'quote':6} {'n':>4} {'n_evm':>6} {'%ok_evm':>8} {'p50_sim':>11} "
              f"{'p90_sim':>11} {'tong_sim':>12} {'tong/gio':>11}")
        for (pair, quote), rs in sorted(by_pool.items(), key=lambda kv: -len(kv[1])):
            evm = [r for r in rs if r["victim_ok_evm"] is not None]
            ok = [r for r in evm if r["victim_ok_evm"]]
            profits = [r["profit_sim"] for r in ok if r["profit_sim"] is not None]
            tot = sum(profits) if profits else 0.0
            p50 = pct(profits, 0.5)
            p90 = pct(profits, 0.9)
            print(f"  {str(pair)[:20]:22} {str(quote):6} {len(rs):>4} {len(evm):>6} "
                  f"{('-' if not evm else f'{len(ok) * 100.0 / len(evm):.1f}'):>8} "
                  f"{('-' if p50 is None else f'{p50:.6f}'):>11} "
                  f"{('-' if p90 is None else f'{p90:.6f}'):>11} "
                  f"{tot:>12.6f} {(tot / hours if hours else 0.0):>11.6f}")
        evm = [r for r in sub if r["victim_ok_evm"] is not None]
        ok = [r for r in evm if r["victim_ok_evm"]]
        profits = [r["profit_sim"] for r in ok if r["profit_sim"] is not None]
        print(f"  TONG {grp}: n={len(sub)} co_shadow.sim={len(evm)} victim_ok_evm={len(ok)} "
              f"tong_profit_sim={sum(profits) if profits else 0.0:.6f} "
              f"({(sum(profits) / hours if (profits and hours) else 0.0):.6f}/gio)")
        errs = collections.Counter(str(r["evm_err"])[:60] for r in sub if not r["victim_ok_evm"])
        if errs:
            print(f"  ly do KHONG ok_evm: {dict(errs.most_common(6))}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

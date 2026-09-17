#!/usr/bin/env python3
"""Analyze sim.arb + skip counts for planB-B7 paper window. No network."""
from __future__ import annotations

import json
import sys
from pathlib import Path

WBNB = "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c"
USDT = "0x55d398326f99059ff775485246999027b3197955"
BNB = 10**18
CAP_BNB = 20 * BNB
CAP_USDT = 12000 * BNB


def pctile(sorted_vals, p):
    if not sorted_vals:
        return None
    if len(sorted_vals) == 1:
        return sorted_vals[0]
    i = int(round((p / 100.0) * (len(sorted_vals) - 1)))
    i = max(0, min(len(sorted_vals) - 1, i))
    return sorted_vals[i]


def main() -> int:
    if len(sys.argv) < 3:
        print("usage: baocao53_analyze_simarb.py <bot.jsonl> <start_line_1based> [end_line]")
        return 2
    path = Path(sys.argv[1])
    start = int(sys.argv[2])
    end = int(sys.argv[3]) if len(sys.argv) > 3 else 10**18

    skips = {}
    sim = []
    n_lines = 0
    with path.open() as f:
        for i, line in enumerate(f, 1):
            if i < start:
                continue
            if i > end:
                break
            n_lines += 1
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue
            ev = obj.get("event")
            if ev == "tx.skip":
                r = obj.get("reason") or "unknown"
                skips[r] = skips.get(r, 0) + 1
            elif ev == "sim.arb":
                sim.append(obj)

    by_dec = {}
    over_cap = []
    borrows_bnb = []
    borrows_usdt = []
    nets_bnb = []
    nets_usdt = []
    routes = {}
    for o in sim:
        dec = o.get("decision") or "unknown"
        by_dec[dec] = by_dec.get(dec, 0) + 1
        try:
            borrow = int(o.get("borrow") or "0")
        except ValueError:
            borrow = 0
        try:
            net = int(o.get("net_wei") or "0")
        except ValueError:
            net = 0
        bq = (o.get("borrow_quote") or "").lower()
        quote = (o.get("quote") or "").lower()
        is_usdt = False
        if bq:
            is_usdt = bq == USDT
            is_wbnb = bq == WBNB
        else:
            is_usdt = quote in ("usdt", USDT)
            is_wbnb = quote in ("wbnb", WBNB) or not is_usdt
        cap = CAP_USDT if is_usdt else CAP_BNB
        if borrow > cap:
            over_cap.append(o)
        if is_usdt:
            borrows_usdt.append(borrow)
            nets_usdt.append(net)
        else:
            borrows_bnb.append(borrow)
            nets_bnb.append(net)
        rk = o.get("route_kind") or "?"
        routes[rk] = routes.get(rk, 0) + 1

    def dist(vals, unit):
        if not vals:
            return f"n=0 {unit}"
        s = sorted(vals)
        return (
            f"n={len(s)} min={s[0]/BNB:.6f} p50={pctile(s,50)/BNB:.6f} "
            f"max={s[-1]/BNB:.6f} {unit}"
        )

    print(f"window_lines={n_lines} start={start}")
    print("skips_from_jsonl=" + json.dumps(skips, sort_keys=True))
    print(
        "sim.arb n={n} simulated={s} unprofitable={u} other={o}".format(
            n=len(sim),
            s=by_dec.get("simulated", 0),
            u=by_dec.get("unprofitable", 0),
            o=sum(v for k, v in by_dec.items() if k not in ("simulated", "unprofitable")),
        )
    )
    print("sim.arb_by_decision=" + json.dumps(by_dec, sort_keys=True))
    print("route_kind=" + json.dumps(routes, sort_keys=True))
    print("borrow_bnb " + dist(borrows_bnb, "BNB"))
    print("borrow_usdt " + dist(borrows_usdt, "USDT"))
    print(f"over_cap_lines={len(over_cap)}  (ky vong 0)")
    for o in over_cap[:20]:
        print(
            "OVER_CAP hash={h} borrow={b} quote={q} bq={bq} dec={d} route={r}".format(
                h=o.get("hash"),
                b=o.get("borrow"),
                q=o.get("quote"),
                bq=o.get("borrow_quote"),
                d=o.get("decision"),
                r=o.get("route_kind"),
            )
        )
    sim_ok = [o for o in sim if o.get("decision") == "simulated"]
    profit_bnb = []
    profit_usdt = []
    for o in sim_ok:
        try:
            net = int(o.get("net_wei") or "0")
        except ValueError:
            continue
        bq = (o.get("borrow_quote") or "").lower()
        quote = (o.get("quote") or "").lower()
        if bq == USDT or (not bq and quote in ("usdt", USDT)):
            profit_usdt.append(net)
        else:
            profit_bnb.append(net)
    print(
        "profit_simulated_bnb n={n} sum={s:.6f} p50={p} max={m}".format(
            n=len(profit_bnb),
            s=sum(profit_bnb) / BNB if profit_bnb else 0.0,
            p=f"{pctile(sorted(profit_bnb),50)/BNB:.6f}" if profit_bnb else "n/a",
            m=f"{max(profit_bnb)/BNB:.6f}" if profit_bnb else "n/a",
        )
    )
    print(
        "profit_simulated_usdt n={n} sum={s:.6f} p50={p} max={m}".format(
            n=len(profit_usdt),
            s=sum(profit_usdt) / BNB if profit_usdt else 0.0,
            p=f"{pctile(sorted(profit_usdt),50)/BNB:.6f}" if profit_usdt else "n/a",
            m=f"{max(profit_usdt)/BNB:.6f}" if profit_usdt else "n/a",
        )
    )
    key_skips = {
        k: skips.get(k, 0)
        for k in (
            "sanity_reject",
            "arb_no_second_venue",
            "below_min",
            "unprofitable",
            "arb_no_flash_source",
        )
    }
    print("key_skips=" + json.dumps(key_skips))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

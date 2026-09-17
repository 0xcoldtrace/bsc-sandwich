#!/usr/bin/env python3
"""Analyze sim.arb + skip counts for planB-B8d paper window. No network."""
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


def wei_to_dec(s):
    try:
        v = int(s)
    except (TypeError, ValueError):
        return None
    sign = "-" if v < 0 else ""
    v = abs(v)
    return f"{sign}{v / BNB:.6f}"


def main() -> int:
    if len(sys.argv) < 3:
        print("usage: baocao57_analyze_simarb.py <bot.jsonl> <start_line_1based> [end_line]")
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
    routes = {}
    reasons = {}
    for o in sim:
        dec = o.get("decision") or "unknown"
        by_dec[dec] = by_dec.get(dec, 0) + 1
        try:
            borrow = int(o.get("borrow") or "0")
        except ValueError:
            borrow = 0
        bq = (o.get("borrow_quote") or "").lower()
        quote = (o.get("quote") or "").lower()
        if bq:
            is_usdt = bq == USDT
        else:
            is_usdt = quote in ("usdt", USDT)
        cap = CAP_USDT if is_usdt else CAP_BNB
        if borrow > cap:
            over_cap.append(o)
        if is_usdt:
            borrows_usdt.append(borrow)
        else:
            borrows_bnb.append(borrow)
        rk = o.get("route_kind") or "?"
        routes[rk] = routes.get(rk, 0) + 1
        rsn = o.get("reason") or ""
        if rsn:
            reasons[rsn] = reasons.get(rsn, 0) + 1

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
        "sim.arb n={n} simulated={s} unprofitable={u} sim_error={e} other={o}".format(
            n=len(sim),
            s=by_dec.get("simulated", 0),
            u=by_dec.get("unprofitable", 0),
            e=by_dec.get("sim_error", 0),
            o=sum(
                v
                for k, v in by_dec.items()
                if k not in ("simulated", "unprofitable", "sim_error")
            ),
        )
    )
    print("sim.arb_by_decision=" + json.dumps(by_dec, sort_keys=True))
    print("sim.arb_reasons=" + json.dumps(reasons, sort_keys=True))
    print("route_kind=" + json.dumps(routes, sort_keys=True))
    print("borrow_bnb " + dist(borrows_bnb, "BNB"))
    print("borrow_usdt " + dist(borrows_usdt, "USDT"))
    print(f"over_cap_lines={len(over_cap)}  (ky vong 0)")
    key_skips = {
        k: skips.get(k, 0)
        for k in (
            "sanity_reject",
            "arb_no_second_venue",
            "below_min",
            "unprofitable",
            "sim_error",
            "arb_no_flash_source",
        )
    }
    print("key_skips=" + json.dumps(key_skips))

    sim_ok = [o for o in sim if o.get("decision") == "simulated"]
    print(f"simulated_rows={len(sim_ok)}")
    missing_quoter = 0
    quoter_le_0 = 0
    for o in sim_ok:
        qnet = o.get("size_quote_net_wei")
        applied = o.get("size_quote_applied")
        if qnet is None or qnet == "":
            missing_quoter += 1
            qnet_s = "MISSING"
            qnet_dec = None
        else:
            qnet_s = str(qnet)
            qnet_dec = wei_to_dec(qnet_s)
            try:
                if int(qnet_s) <= 0:
                    quoter_le_0 += 1
            except ValueError:
                missing_quoter += 1
        print(
            "SIM token={tok} route={rk} borrow={b} net_paper={np} net_quoter={nq} "
            "size_quote_applied={ap} hash={h}".format(
                tok=o.get("token"),
                rk=o.get("route_kind"),
                b=wei_to_dec(o.get("borrow")),
                np=wei_to_dec(o.get("net_wei")),
                nq=qnet_dec if qnet_dec is not None else qnet_s,
                ap=applied,
                h=o.get("hash"),
            )
        )
    print(f"simulated_missing_quoter={missing_quoter}")
    print(f"simulated_quoter_le_0={quoter_le_0}")
    if missing_quoter > 0:
        print("FAIL_RULE: simulated>0 without size_quote_net_wei")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

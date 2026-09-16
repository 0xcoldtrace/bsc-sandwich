#!/usr/bin/env python3
"""Cụm `verify-cluster-as-victim` — MỤC 3 + MỤC 4, đọc file, không gọi RPC.

Vào: `--scan` (output của `scripts/cluster_tx_scan.py`) + `--log`
(`logs/bot.jsonl` của LẦN CHẠY phủ đúng khoảng block đó).

Mục 3 — ĐƯỜNG GỬI CỦA CỤM: với N tx cụm gần nhất, bot có thấy tx đó trong
mempool (`tx.seen`) trước khi nó lên block không, và sớm hơn bao nhiêu ms?
Mốc "lên block" lấy từ chính `rpc.block` của bot (lúc bot NHÌN THẤY block
chứa tx) chứ không lấy `block.timestamp` — timestamp block BSC chỉ có độ phân
giải giây trong khi block ~0,45 s, dùng nó sẽ cho số ms vô nghĩa.

Mục 4 — AI ĐANG KẸP CỤM: trong ±3 vị trí quanh mỗi tx cụm, có địa chỉ nào
vừa đứng TRƯỚC vừa đứng SAU trên CÙNG pool không (dấu hiệu sandwich)?
"""
import argparse, json, collections, datetime as dt


def parse_ts(s):
    return dt.datetime.fromisoformat(s).timestamp()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--scan", required=True)
    ap.add_argument("--log", required=True)
    ap.add_argument("--muc3-n", type=int, default=30)
    ap.add_argument("--muc4-n", type=int, default=200)
    a = ap.parse_args()

    rows = [json.loads(l) for l in open(a.scan)]
    rows.sort(key=lambda r: (r["block"], r["tx_index"]))
    by_hash = {r["hash"]: r for r in rows}

    seen_ts, block_ts = {}, {}
    for line in open(a.log, errors="replace"):
        if '"tx.seen"' not in line and '"rpc.block"' not in line:
            continue
        try:
            d = json.loads(line)
        except Exception:  # noqa: BLE001
            continue
        ev = d.get("event")
        if ev == "tx.seen":
            h = (d.get("hash") or "").lower()
            if h in by_hash and h not in seen_ts:
                seen_ts[h] = parse_ts(d["ts"])
        elif ev == "rpc.block":
            b = d.get("block")
            if b is not None and b not in block_ts:
                block_ts[b] = parse_ts(d["ts"])

    lo, hi = min(block_ts) if block_ts else 0, max(block_ts) if block_ts else 0
    covered = [r for r in rows if lo <= r["block"] <= hi]
    print(f"== PHU SONG ==  bot thay block {lo}..{hi} ({len(block_ts)} block) ; "
          f"tx cum trong khoang do = {len(covered)}/{len(rows)}")

    # ---------------- MUC 3 ----------------
    print(f"\n== MUC 3: DUONG GUI CUA CUM ({a.muc3_n} tx cum gan nhat trong khoang bot phu song) ==")
    sub = covered[-a.muc3_n:]
    n_seen = n_blind = 0
    leads = []
    print(f"{'hash':20} {'block':>10} {'how':12} {'sel':11} {'gwei':>7} {'bot thay?':10} {'som hon (ms)':>13}")
    for r in sub:
        h = r["hash"]
        bt = block_ts.get(r["block"])
        if h in seen_ts:
            n_seen += 1
            lead = (bt - seen_ts[h]) * 1000.0 if bt else None
            if lead is not None:
                leads.append(lead)
            print(f"{h[:18]:20} {r['block']:>10} {r['how']:12} {r['selector']:11} "
                  f"{r['gas_price_gwei']:>7.3f} {'CO':10} {lead if lead is None else round(lead, 1):>13}")
        else:
            n_blind += 1
            print(f"{h[:18]:20} {r['block']:>10} {r['how']:12} {r['selector']:11} "
                  f"{r['gas_price_gwei']:>7.3f} {'KHONG':10} {'-':>13}")
    tot = len(sub)
    print(f"\n  tong                      = {tot}")
    if tot:
        print(f"  bot THAY pending          = {n_seen}  ({n_seen * 100.0 / tot:.1f}%)")
        print(f"  bot KHONG thay (private)  = {n_blind}  ({n_blind * 100.0 / tot:.1f}%)")
    if leads:
        leads.sort()
        p = lambda q: leads[min(len(leads) - 1, int(q * (len(leads) - 1)))]  # noqa: E731
        print(f"  som hon block (ms): n={len(leads)} p50={p(0.5):.0f} p90={p(0.9):.0f} "
              f"min={leads[0]:.0f} max={leads[-1]:.0f}")
    else:
        print("  som hon block (ms): MISSING (khong tx nao vua duoc thay pending vua co moc rpc.block)")

    # Tren TOAN BO khoang phu song (khong chi 30 dong) - so co y nghia thong ke hon.
    all_seen = sum(1 for r in covered if r["hash"] in seen_ts)
    if covered:
        print(f"  [toan bo {len(covered)} tx cum trong khoang] bot thay = {all_seen} "
              f"({all_seen * 100.0 / len(covered):.1f}%) ; KHONG thay = {len(covered) - all_seen} "
              f"({(len(covered) - all_seen) * 100.0 / len(covered):.1f}%)")
    gp = collections.Counter(round(r["gas_price_gwei"], 3) for r in covered)
    print(f"  gas_price cua tx cum (gwei): {dict(gp.most_common(6))}")

    # ---------------- MUC 4 ----------------
    print(f"\n== MUC 4: AI DANG KEP CUM (+-3 vi tri, {a.muc4_n} tx cum gan nhat) ==")
    sub4 = rows[-a.muc4_n:]
    n_nb = sandwiches = 0
    bots = collections.Counter()
    detail = []
    for r in sub4:
        nb = r["neighbors_same_pool"]
        if nb:
            n_nb += 1
        before = {x["from"] for x in nb if x["pos"] < 0}
        after = {x["from"] for x in nb if x["pos"] > 0}
        both = before & after
        if both:
            sandwiches += 1
            for b in both:
                bots[b] += 1
            detail.append((r, sorted(both), nb))
    print(f"  tx cum xet                       = {len(sub4)}")
    print(f"  co tx khac CHAM CUNG POOL trong +-3 = {n_nb} ({n_nb * 100.0 / max(1, len(sub4)):.1f}%)")
    print(f"  co dia chi dung CA TRUOC + SAU   = {sandwiches} ({sandwiches * 100.0 / max(1, len(sub4)):.1f}%)")
    if bots:
        print("  dia chi nghi kep (dem lan):")
        for adr, c in bots.most_common(10):
            print(f"    {adr}  x{c}")
        print("\n  chi tiet (toi da 10 vu):")
        for r, both, nb in detail[:10]:
            print(f"    victim-cum {r['hash'][:18]} block {r['block']} idx {r['tx_index']} "
                  f"gas {r['gas_price_gwei']:.3f} gwei pool {(r['pools'] or ['-'])[0][:12]}")
            for x in sorted(nb, key=lambda y: y["pos"]):
                mark = "<-- KEP" if x["from"] in both else ""
                print(f"       pos {x['pos']:+d}  {x['from'][:12]}  gas {x['gas_price_gwei']:.3f} gwei "
                      f"{x['hash'][:18]} {mark}")
    else:
        print("  KHONG tim thay dia chi nao dung ca truoc lan sau tren cung pool "
              "trong +-3 vi tri -> khong thay ai kep cum nay (trong pham vi do duoc).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

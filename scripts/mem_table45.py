#!/usr/bin/env python3
"""Cụm `verify-cluster-as-victim` (mục 1) — in bảng `mem.rss_mb` từ log.

`len` của container có thể là `null` (khoá đang bận, xem `mem::ContainerSize`)
— in "?" chứ KHÔNG in 0, vì 0 nghĩa là "rỗng thật" và hai thứ đó khác nhau.
"""
import sys, json

def f(x, w=9, d=2):
    return "?".rjust(w) if x is None else f"{x:{w}.{d}f}"

for line in open(sys.argv[1], errors="replace"):
    if '"mem.rss_mb"' not in line:
        continue
    d = json.loads(line)
    cs = {c["name"].split("::")[-1]: c["len"] for c in d.get("containers", [])}
    g = lambda k: "?" if cs.get(k) is None else cs[k]  # noqa: E731
    print(f"{d['uptime_sec']:6d}s rss={f(d.get('rss_mb'))} peak={f(d.get('rss_peak_mb'))} "
          f"d={f(d.get('delta_mb_since_last'), 8, 3)} | reserve={g('ReserveCache.entries')} "
          f"seen={g('SeenHashSet')} mined={g('MinedTxIndex')} "
          f"rate_buckets={g('ClusterRateWatch.buckets')}")

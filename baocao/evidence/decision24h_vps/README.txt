Bang chung cum `decision-data-24h` (BAOCAO43) — sinh tren VPS bang
`scripts/analyze_econ.sh` (bash+jq+awk, khong can Rust tren VPS), du lieu
`logs/bot.jsonl` tu dong 43138 (dau lan chay paper 24h).

Cua so THAT: 2026-09-15T17:45:20Z -> 2026-09-16T04:40:47Z = 10.92 h
(KHONG phai 24h — bot bi OOM-kill sau 656 phut, xem paper24h_vps.out).

Binary VPS luc sinh du lieu: git 5284bd376fb78ea0b450249fb2d09d4adfd7f812
sha256 4af44b29ef1adf729f416d624d18147030bf8d52f0c7f149bfaa0a0143b0feab

File:
  1a_pool_quote_hour_netpos_only.tsv  pool x quote x gio UTC (LOC: chi dong
                                      net_pos>0; ban day du 1438 pool nam o
                                      logs/vps_analysis/, khong commit vi
                                      logs/ bi gitignore)
  1a_front_p80.tsv                    p80/max front_in tren dong CO LAI
  1b_by_quote.tsv                     tong theo quote
  1b_top_pools.tsv                    top pool + % net_pos la vi cum doi thu
  1c_by_hour.tsv                      phan bo theo gio (UTC + gio VN)
  1d_summary.txt                      ket luan bang so (a)(b)(c)(d)
  paper24h_vps.out                    output paper_run.sh tren VPS (co dong
                                      "BOT DA CHET sau 656 phut")

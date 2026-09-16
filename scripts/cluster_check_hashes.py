#!/usr/bin/env python3
"""Cụm `verify-cluster-as-victim` (mục 2) — kiểm ĐÚNG một danh sách tx hash
xem có phải nạn nhân của cụm `0xB406` không, bằng on-chain, rẻ.

Khác `cluster_tx_scan.py` (quét cả khoảng block), script này chỉ nạp block
CHỨA từng hash đã cho: với mỗi hash, đọc receipt của cả block và tìm xem có
`Transfer` quote-asset **từ một seed** tới chính `tx.from` ở `tx_index` NHỎ HƠN
hay không. Có = ví burner được cấp vốn trong cùng block (mẫu hình của cụm).

Đây là nguồn sự thật thay cho cờ `victim_in_competitor_cluster` của bot — cờ
đó LUÔN `false` cho mẫu hình này, vì lúc bot quyết định thì block chứa lệnh
cấp vốn chưa được đào.
"""
import argparse, json, sys, urllib.request, time
from concurrent.futures import ThreadPoolExecutor

SEEDS = {
    "0xb406021e07b31e1f7850fcccd7076094f18d07ef",
    "0xa739dfab40ef6585f1174fce90ec96330669758c",
    "0x8180ad6a7c9f8f4864e9909480fba4123fce6c54",
}
TRANSFER = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
QUOTES = {"0x55d398326f99059ff775485246999027b3197955", "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c"}


def rpc(url, method, params, tries=5):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    last = None
    for i in range(tries):
        try:
            req = urllib.request.Request(
                url, data=body, headers={"content-type": "application/json", "user-agent": "curl/8.5.0"}
            )
            with urllib.request.urlopen(req, timeout=45) as r:
                out = json.loads(r.read())
            if "result" in out:
                return out["result"]
            last = out.get("error")
        except Exception as e:  # noqa: BLE001
            last = str(e)
        time.sleep(0.4 * (i + 1))
    raise RuntimeError(f"{method}: {last}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--hashes", required=True, help="file, moi dong 1 hash")
    ap.add_argument("--rpc", default="https://bsc-dataseed1.bnbchain.org")
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=6)
    a = ap.parse_args()
    url = a.rpc
    assert rpc(url, "eth_chainId", []) == "0x38"
    hashes = [l.strip().lower() for l in open(a.hashes) if l.strip()]
    print(f"kiem {len(hashes)} hash tren {url.split('//')[-1].split('/')[0]}", file=sys.stderr)

    def check(h):
        tx = rpc(url, "eth_getTransactionByHash", [h])
        if not tx or tx.get("blockNumber") is None:
            return {"hash": h, "tren_chain": False}
        blk = int(tx["blockNumber"], 16)
        idx = int(tx["transactionIndex"], 16)
        frm = tx["from"].lower()
        rcs = rpc(url, "eth_getBlockReceipts", [hex(blk)])
        funders = []
        for rc in rcs:
            if int(rc["transactionIndex"], 16) >= idx:
                continue
            for lg in rc.get("logs") or []:
                tps = lg.get("topics") or []
                if len(tps) < 3 or tps[0].lower() != TRANSFER or lg["address"].lower() not in QUOTES:
                    continue
                if "0x" + tps[2][-40:].lower() != frm:
                    continue
                src = "0x" + tps[1][-40:].lower()
                if src in SEEDS:
                    funders.append({
                        "tu_seed": src,
                        "tai_idx": int(rc["transactionIndex"], 16),
                        "so_luong": int(lg["data"], 16) / 1e18,
                        "token": lg["address"].lower(),
                    })
        return {
            "hash": h, "tren_chain": True, "block": blk, "tx_index": idx, "from": frm,
            "la_cum": bool(funders) or frm in SEEDS,
            "cap_von_cung_block": funders,
            "la_seed": frm in SEEDS,
            "gas_price_gwei": int(tx.get("gasPrice", "0x0"), 16) / 1e9,
        }

    rows = []
    with ThreadPoolExecutor(max_workers=a.workers) as ex:
        for r in ex.map(check, hashes):
            rows.append(r)
    with open(a.out, "w") as f:
        for r in rows:
            f.write(json.dumps(r) + "\n")
    on = [r for r in rows if r.get("tren_chain")]
    cum = [r for r in on if r.get("la_cum")]
    print(f"tren chain = {len(on)}/{len(rows)} ; LA CUM = {len(cum)} ({len(cum) * 100.0 / max(1, len(on)):.1f}%)",
          file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

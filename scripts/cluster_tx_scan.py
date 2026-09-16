#!/usr/bin/env python3
"""Cụm `verify-cluster-as-victim` (mục 3 + 4) — QUÉT ON-CHAIN, CHỈ ĐỌC.

Dựng danh sách tx THẬT của cụm đối thủ `0xB406` trong một khoảng block, kèm
(a) dấu thời gian block để đối chiếu `tx.seen` của bot (mục 3 — bot có thấy
tx đó trong mempool không, và sớm hơn lúc đào bao nhiêu ms), và (b) các tx
đứng LIỀN KỀ ±3 vị trí có chạm CÙNG pool không (mục 4 — có ai kẹp cụm không).

Cách tìm: `eth_getLogs` cho Transfer WBNB/USDT **từ** 1 trong 3 seed đã verify
on-chain (BAOCAO41) — mỗi log như vậy nằm trong đúng 1 tx của cụm, nên chỉ
cần nạp những block CÓ log đó thay vì quét mọi block (rẻ hơn ~100 lần).
Ví burner được cấp vốn cũng được ghi lại để bắt tx do CHÍNH ví đó gửi ở
block hiện tại/liền sau, đúng cửa sổ 2 block của `src/competitor.rs`.

KHÔNG ký, KHÔNG gửi tx, KHÔNG in URL RPC đầy đủ.
"""
import argparse, json, sys, time
import urllib.request
from concurrent.futures import ThreadPoolExecutor

SEEDS = {
    "0xb406021e07b31e1f7850fcccd7076094f18d07ef",
    "0xa739dfab40ef6585f1174fce90ec96330669758c",
    "0x8180ad6a7c9f8f4864e9909480fba4123fce6c54",
}
SEED_TOPICS = ["0x" + "0" * 24 + s[2:] for s in SEEDS]
TRANSFER = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
SWAP_V2 = "0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822"
WBNB = "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c"
USDT = "0x55d398326f99059ff775485246999027b3197955"
QUOTES = {WBNB, USDT}
ROUTERS = {
    "0x10ed43c718714eb63d5aa57b78b54704e256024e",
    "0x13f4ea83d0bd40e75c8222255bc855a974568dd4",
    "0x1b81d678ffb9c0263b24a97847620c99d213eb14",
    "0x1a0a18ac4becddbd6389559687d1a73d8927e416",
    "0x1906c1d672b88cd1b9ac7593301ca990f94eae07",
}


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
    raise RuntimeError(f"{method} that bai sau {tries} lan: {last}")


def topic_addr(t):
    return "0x" + t[-40:].lower()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--from-block", type=int, required=True)
    ap.add_argument("--to-block", type=int, required=True)
    ap.add_argument("--rpc", default="https://rpc-bsc.48.club", help="RPC cho eth_getLogs")
    ap.add_argument("--rpc-blocks", default="https://bsc-dataseed1.bnbchain.org",
                    help="RPC cho eth_getBlockByNumber/eth_getBlockReceipts (tach de khong dam 1 node)")
    ap.add_argument("--out", required=True)
    ap.add_argument("--workers", type=int, default=8)
    ap.add_argument("--chunk", type=int, default=1000)
    a = ap.parse_args()
    url = a.rpc
    host = url.split("//")[-1].split("/")[0]
    chain = rpc(url, "eth_chainId", [])
    print(f"rpc host = {host} eth_chainId = {chain}", file=sys.stderr)
    assert chain == "0x38", f"chain != 0x38: {chain}"

    # --- 1) eth_getLogs: mọi Transfer quote-asset TỪ seed trong khoảng ---
    logs = []
    b = a.from_block
    while b <= a.to_block:
        e = min(b + a.chunk - 1, a.to_block)
        logs += rpc(url, "eth_getLogs", [{
            "fromBlock": hex(b), "toBlock": hex(e),
            "address": [USDT, WBNB],
            "topics": [TRANSFER, SEED_TOPICS],
        }])
        print(f"  getLogs {b}..{e} -> tong {len(logs)} log", file=sys.stderr)
        b = e + 1
    blocks_of_interest = sorted({int(l["blockNumber"], 16) for l in logs})
    funded = {}  # block -> set(wallet)
    for l in logs:
        bn = int(l["blockNumber"], 16)
        funded.setdefault(bn, set()).add(topic_addr(l["topics"][2]))
    # Ví burner có thể swap ở block LIỀN SAU -> nạp thêm block đó.
    need = sorted(set(blocks_of_interest) | {x + 1 for x in blocks_of_interest})
    print(f"{len(logs)} log seed-transfer / {len(blocks_of_interest)} block co log / {len(need)} block can nap",
          file=sys.stderr)

    # --- 2) nạp block + receipts (song song) ---
    burl = a.rpc_blocks

    def fetch(bn):
        return bn, rpc(burl, "eth_getBlockByNumber", [hex(bn), True]), rpc(burl, "eth_getBlockReceipts", [hex(bn)])

    cache = {}
    with ThreadPoolExecutor(max_workers=a.workers) as ex:
        for i, (bn, blk, rcs) in enumerate(ex.map(fetch, need)):
            cache[bn] = (blk, rcs)
            if i % 100 == 0:
                print(f"  nap block {i}/{len(need)}", file=sys.stderr)

    # --- 3) phân loại + hàng xóm ±3 ---
    out = open(a.out, "w")
    n = 0
    for bn in need:
        blk, rcs = cache[bn]
        ts_sec = int(blk["timestamp"], 16)
        txs = blk["transactions"]
        pools_of = {}
        for rc in rcs:
            pools = {lg["address"].lower() for lg in (rc.get("logs") or [])
                     if (lg.get("topics") or [""])[0].lower() == SWAP_V2}
            if pools:
                pools_of[rc["transactionHash"].lower()] = pools
        window = funded.get(bn, set()) | funded.get(bn - 1, set())
        for t in txs:
            frm = t["from"].lower()
            to = (t.get("to") or "").lower()
            # 3 lối vào cụm, ghi rõ lối nào để không trộn số khi đọc:
            #   from_seed   — chính seed gửi tx
            #   from_funded — ví burner vừa được seed cấp vốn (cơ chế BAOCAO41)
            #   to_cluster  — EOA ngoài gọi THẲNG contract của cụm (`0x8180…`/
            #                 `0xa739…`) — dạng quan sát được 2026-09-16
            if frm in SEEDS:
                how = "from_seed"
            elif frm in window:
                how = "from_funded"
            elif to in SEEDS:
                how = "to_cluster"
            else:
                continue
            idx = int(t["transactionIndex"], 16)
            h = t["hash"].lower()
            pools = sorted(pools_of.get(h, []))
            nb = []
            for j in range(max(0, idx - 3), min(len(txs), idx + 4)):
                if j == idx:
                    continue
                nt = txs[j]
                shared = sorted(set(pools) & set(pools_of.get(nt["hash"].lower(), [])))
                if not shared:
                    continue
                nb.append({
                    "pos": j - idx,
                    "hash": nt["hash"].lower(),
                    "from": nt["from"].lower(),
                    "to": (nt.get("to") or "").lower(),
                    "gas_price_gwei": int(nt.get("gasPrice", "0x0"), 16) / 1e9,
                    "pools": shared,
                })
            out.write(json.dumps({
                "hash": h, "block": bn, "block_ts_sec": ts_sec, "tx_index": idx,
                "from": frm, "to": to, "to_is_pancake_router": to in ROUTERS,
                "gas_price_gwei": int(t.get("gasPrice", "0x0"), 16) / 1e9,
                "value_bnb": int(t.get("value", "0x0"), 16) / 1e18,
                "how": how, "selector": t.get("input", "0x")[:10],
                "pools": pools, "neighbors_same_pool": nb,
            }) + "\n")
            n += 1
    out.close()
    print(f"XONG: {n} tx cua cum trong {a.to_block - a.from_block + 1} block -> {a.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())

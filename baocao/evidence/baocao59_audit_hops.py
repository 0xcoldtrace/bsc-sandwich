#!/usr/bin/env python3
"""BAOCAO59 hop audit — RPC via BSC_HTTP_SIM (NodeReal). Never print URL/token."""
from __future__ import annotations

import json
import os
import time
import urllib.request
from pathlib import Path

ROOT = Path("/home/dmin/bsc-sandwich")
ENV = ROOT / ".env"
OUT = ROOT / "baocao/evidence/baocao59_audit.json"
LOG = ROOT / "baocao/evidence/baocao59_audit.log"

WBNB = "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c"
USDT = "0x55d398326f99059ff775485246999027b3197955"
TOKEN4 = "0x0a43fc31a73013089df59194872ecae4cae14444"
CAKE = "0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82"
V2_ROUTER = "0x10ed43c718714eb63d5aa57b78b54704e256024e"
QUOTER = "0xb048bbc1ee6b733fffcfb9e9cef7375518e25997"
BRIDGE = "0x16b9a82891338f9ba80e2d6970fdda79d1eb0dae"
SEL_QUOTE = "c6a5026a"
SEL_AMOUNTS = "d06ca61f"
SEL_RESERVES = "0902f1ac"
SEL_TOKEN0 = "0dfe1681"
SEL_TOKEN1 = "d21220a7"
SEL_FEE = "ddca3f43"

SNAP_WBNB = 53507083413731134572126
SNAP_USDT = 37996015600097767345798691


def log(msg: str) -> None:
    line = msg.rstrip() + "\n"
    with LOG.open("a") as f:
        f.write(line)
    print(line, end="", flush=True)


def host_only(url: str) -> str:
    try:
        from urllib.parse import urlparse

        return urlparse(url).hostname or "parse_fail"
    except Exception:
        return "parse_fail"


def load_sim_urls() -> list[str]:
    urls: list[str] = []
    for raw in ENV.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        line = line[7:].strip() if line.startswith("export ") else line
        if "=" not in line:
            continue
        k, v = line.split("=", 1)
        if k.strip() != "BSC_HTTP_SIM":
            continue
        v = v.strip().strip('"').strip("'")
        for part in v.split(","):
            u = part.strip()
            if u and u not in urls:
                urls.append(u)
    env_u = os.environ.get("BSC_HTTP_SIM", "").strip()
    if env_u:
        for part in env_u.split(","):
            u = part.strip()
            if u and u not in urls:
                urls.insert(0, u)
    prefer, rest = [], []
    for u in urls:
        h = host_only(u).lower()
        if "getblock" in h:
            continue
        if "nodereal" in h:
            prefer.append(u)
        else:
            rest.append(u)
    return prefer + rest


def word_addr(a: str) -> str:
    return a.lower().replace("0x", "").rjust(64, "0")


def word_u256(n: int) -> str:
    return f"{n:064x}"


def decode_u256(hexdata: str) -> int:
    h = hexdata.lower().replace("0x", "")
    if not h:
        return 0
    return int(h[:64], 16)


def v2_out(amount_in: int, rin: int, rout: int) -> int:
    if amount_in <= 0 or rin <= 0 or rout <= 0:
        return 0
    a = amount_in * 9975
    return (a * rout) // (rin * 10000 + a)


def rpc(url: str, method: str, params, timeout=60):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(url, data=body, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        data = json.loads(resp.read().decode())
    if "error" in data and data["error"]:
        raise RuntimeError(json.dumps(data["error"]))
    return data.get("result")


def rpc_retry(urls: list[str], method: str, params, tries=8):
    last = None
    for attempt in range(tries):
        for i, u in enumerate(urls):
            try:
                return rpc(u, method, params), host_only(u), i
            except Exception as e:
                last = f"host={host_only(u)} {e}"
                s = str(e)
                if "-32005" in s or "rate" in s.lower() or "429" in s:
                    time.sleep(5 * (attempt + 1))
                    continue
                time.sleep(0.4)
        time.sleep(1.5 * (attempt + 1))
    raise RuntimeError(last or "rpc_fail")


def blk_tag(n: int | str) -> str:
    if isinstance(n, str):
        return n
    return hex(n)


def call(urls, to: str, data: str, block: int | str):
    res, host, idx = rpc_retry(
        urls,
        "eth_call",
        [{"to": to, "data": data}, blk_tag(block)],
    )
    return res, host, idx


def quote_v3(urls, token_in, token_out, amount_in, fee, block):
    data = (
        "0x"
        + SEL_QUOTE
        + word_addr(token_in)
        + word_addr(token_out)
        + word_u256(amount_in)
        + word_u256(fee)
        + word_u256(0)
    )
    raw, host, idx = call(urls, QUOTER, data, block)
    return decode_u256(raw), host


def amounts_out(urls, amount_in, path, block):
    data = (
        "0x"
        + SEL_AMOUNTS
        + word_u256(amount_in)
        + word_u256(0x40)
        + word_u256(len(path))
        + "".join(word_addr(a) for a in path)
    )
    raw, host, idx = call(urls, V2_ROUTER, data, block)
    h = raw.lower().replace("0x", "")
    # offset, len, words...
    if len(h) < 192:
        raise RuntimeError(f"amounts_out short {len(h)}")
    n = int(h[64:128], 16)
    outs = [int(h[128 + 64 * i : 192 + 64 * i], 16) for i in range(n)]
    return outs, host


def reserves(urls, pair, block):
    raw, host, idx = call(urls, pair, "0x" + SEL_RESERVES, block)
    h = raw.lower().replace("0x", "").rjust(192, "0")
    r0 = int(h[0:64], 16)
    r1 = int(h[64:128], 16)
    return r0, r1, host


def addr_call(urls, to, sel, block):
    raw, host, idx = call(urls, to, "0x" + sel, block)
    h = raw.lower().replace("0x", "").rjust(64, "0")
    return "0x" + h[-40:], host


def u24_call(urls, to, sel, block):
    raw, host, idx = call(urls, to, "0x" + sel, block)
    return decode_u256(raw), host


def receipt(urls, txh):
    res, host, idx = rpc_retry(urls, "eth_getTransactionReceipt", [txh])
    return res, host


def tx(urls, txh):
    res, host, idx = rpc_retry(urls, "eth_getTransactionByHash", [txh])
    return res, host


def fmt(n: int | None) -> str:
    if n is None:
        return "MISSING"
    sign = "-" if n < 0 else ""
    v = abs(n)
    whole = v // 10**18
    frac = f"{v % 10**18:018d}".rstrip("0") or "0"
    return f"{sign}{whole}.{frac}"


def net_of(final_out, borrow, gas, bribe, flash=0):
    return final_out - borrow - flash - gas - bribe


CASES = [
    {
        "name": "CASE_V2V3",
        "hash": "0x657218fc2621df92676029346db880a163276d26d4baf15e88caeaf7d5990f53",
        "token": TOKEN4,
        "symbol": "4",
        "route": "v2_v3",
        "borrow": 2585473203347248311,
        "gas": 360000000000000,
        "bribe": 8940913652709132,
        "flash": 0,
        "net_paper": 13411370479063696,
        "net_quoter_log": 13411364926945322,
        "profit_revm_tsv": -49150000000000000,  # 6-dec TSV; refined later if exact
        "pair_buy": "0xf0a949d3d93b833c183a27ee067165b6f2c9625e",
        "pair_sell": "0xee04b2a82bab9efefcd626f5d66f51cc2b6fa12a",
        "pair_victim": "0xf0a949d3d93b833c183a27ee067165b6f2c9625e",
        "buy_kind": "v2",
        "sell_kind": "pcs_v3",
        "sell_fee": 2500,
        "buy_quote": WBNB,
        "sell_quote": USDT,
        "amount_in": 1047195215843787264,
        "victim_buys_token": False,
        "gross_paper": 22712284131772828,
    },
    {
        "name": "CASE_CAKE",
        "hash": "0x7ba67696c2fcd9c49cf921c5dd52097bedf24ce4c62d0c1a0cf854d9c928cc1d",
        "token": CAKE,
        "symbol": "Cake",
        "route": "v3_v3",
        "borrow": 20000000000000000000,
        "gas": 360000000000000,
        "bribe": 10000000000000000,
        "flash": 0,
        "net_paper": 319424347243675688,
        "net_quoter_log": 316063527379767828,
        "profit_revm_tsv": -179687000000000000,
        "pair_buy": "0xafb2da14056725e3ba3a30dd846b6bbbd7886c56",
        "pair_sell": "0x7f51c8aaa6b0599abd16674e2b17fec7a9f674a1",
        "pair_victim": "0x0ed7e52944161450477ee417de9cd3a859b14fd0",
        "buy_kind": "pcs_v3",
        "sell_kind": "pcs_v3",
        "sell_fee": 2500,
        "buy_fee": 500,
        "buy_quote": WBNB,
        "sell_quote": USDT,
        "amount_in": 1243817778628869916,
        "victim_buys_token": False,
        "gross_paper": 329784347243675688,
    },
]


def hop1(urls, c, block):
    if c["buy_kind"] == "v2":
        outs, host = amounts_out(urls, c["borrow"], [WBNB, c["token"]], block)
        return outs[-1], "v2_getAmountsOut", host
    out, host = quote_v3(urls, WBNB, c["token"], c["borrow"], c.get("buy_fee", 500), block)
    return out, "quoter_v3", host


def hop2(urls, c, token_in_amt, block):
    out, host = quote_v3(urls, c["token"], USDT, token_in_amt, c["sell_fee"], block)
    return out, "quoter_v3", host


def hop_bridge_chain(urls, usdt_in, block):
    outs, host = amounts_out(urls, usdt_in, [USDT, WBNB], block)
    return outs[-1], "v2_getAmountsOut", host


def main():
    if LOG.exists():
        LOG.unlink()
    urls = load_sim_urls()
    if not urls:
        raise SystemExit("FAIL no BSC_HTTP_SIM")
    log(f"rpc_hosts={[host_only(u) for u in urls]}")
    probe, host, _ = rpc_retry(
        urls,
        "eth_getStorageAt",
        [CAKE, "0x0", hex(122450112)],
    )
    log(f"PROBE_ARCHIVE host={host} result_len={len(probe or '')} head={(probe or '')[:10]}")
    if not probe or probe in ("0x", "0x0"):
        log("PROBE_VERDICT FAIL")
    else:
        log("PROBE_VERDICT PASS")

    all_out = {"snap_bridge_wbnb": SNAP_WBNB, "snap_bridge_usdt": SNAP_USDT, "cases": []}
    for c in CASES:
        log(f"\n===== {c['name']} {c['hash']} =====")
        rec, host = receipt(urls, c["hash"])
        if not rec:
            log("MISSING receipt")
            continue
        block = int(rec["blockNumber"], 16)
        tx_index = int(rec.get("transactionIndex") or "0x0", 16)
        status = rec.get("status")
        parent = block - 1
        txo, _ = tx(urls, c["hash"])
        to = (txo or {}).get("to")
        log(f"receipt host={host} block={block} tx_index={tx_index} status={status} to={to}")

        t0b, _ = addr_call(urls, c["pair_buy"], SEL_TOKEN0, block)
        t1b, _ = addr_call(urls, c["pair_buy"], SEL_TOKEN1, block)
        t0s, _ = addr_call(urls, c["pair_sell"], SEL_TOKEN0, block)
        t1s, _ = addr_call(urls, c["pair_sell"], SEL_TOKEN1, block)
        try:
            feeb, _ = u24_call(urls, c["pair_buy"], SEL_FEE, block)
        except Exception as e:
            feeb = f"MISSING {e}"
        fees, _ = u24_call(urls, c["pair_sell"], SEL_FEE, block)
        log(f"buy pool token0={t0b} token1={t1b} fee={feeb}")
        log(f"sell pool token0={t0s} token1={t1s} fee={fees}")

        br0_v, br1_v, _ = reserves(urls, BRIDGE, block)
        br0_p, br1_p, _ = reserves(urls, BRIDGE, parent)
        t0br, _ = addr_call(urls, BRIDGE, SEL_TOKEN0, block)
        t1br, _ = addr_call(urls, BRIDGE, SEL_TOKEN1, block)
        # pair 0x16b9a828... token0=USDT? token1=WBNB? classic is token0=WBNB token1=USDT
        log(f"bridge token0={t0br} token1={t1br}")
        log(f"bridge parent r0={br0_p} r1={br1_p}")
        log(f"bridge victim r0={br0_v} r1={br1_v}")
        log(f"bridge SNAP  wbnb={SNAP_WBNB} usdt={SNAP_USDT}")

        if t0br.lower() == WBNB:
            live_w, live_u = br0_v, br1_v
            par_w, par_u = br0_p, br1_p
        else:
            live_w, live_u = br1_v, br0_v
            par_w, par_u = br1_p, br0_p
        snap_rate = SNAP_USDT / SNAP_WBNB
        live_rate = live_u / live_w
        par_rate = par_u / par_w
        log(f"USDT_per_WBNB snap={snap_rate:.6f} parent={par_rate:.6f} victim={live_rate:.6f}")

        row = {
            "name": c["name"],
            "hash": c["hash"],
            "block": block,
            "parent": parent,
            "tx_index": tx_index,
            "status": status,
            "to": to,
            "buy_token0": t0b,
            "buy_token1": t1b,
            "buy_fee": feeb if isinstance(feeb, int) else str(feeb),
            "sell_token0": t0s,
            "sell_token1": t1s,
            "sell_fee": fees,
            "bridge_token0": t0br,
            "bridge_token1": t1br,
            "bridge_parent_wbnb": par_w,
            "bridge_parent_usdt": par_u,
            "bridge_victim_wbnb": live_w,
            "bridge_victim_usdt": live_u,
            "blocks": {},
        }

        paper_final = c["borrow"] + c["gross_paper"]
        log(f"PAPER implied final_out={paper_final} ({fmt(paper_final)}) net={c['net_paper']}")
        log(f"QUOTER_LOG net={c['net_quoter_log']} implied_final={c['net_quoter_log']+c['borrow']+c['gas']+c['bribe']}")

        for tag, b in [("parent", parent), ("victim", block)]:
            log(f"-- hops at {tag} block={b} --")
            try:
                tok, src1, _h1 = hop1(urls, c, b)
                usdt_out, src2, _h2 = hop2(urls, c, tok, b)
                wbnb_live, src3, _h3 = hop_bridge_chain(urls, usdt_out, b)
            except Exception as e:
                log(f"  HOP_FAIL {tag}: {e}")
                row["blocks"][tag] = {"block": b, "error": str(e)}
                continue
            wbnb_snap = v2_out(usdt_out, SNAP_USDT, SNAP_WBNB)
            if t0br.lower() == WBNB:
                wbnb_parent_res = v2_out(usdt_out, par_u, par_w)
                wbnb_victim_res = v2_out(usdt_out, live_u, live_w)
            else:
                wbnb_parent_res = v2_out(usdt_out, par_u, par_w)
                wbnb_victim_res = v2_out(usdt_out, live_u, live_w)
            net_live = net_of(wbnb_live, c["borrow"], c["gas"], c["bribe"])
            net_snap = net_of(wbnb_snap, c["borrow"], c["gas"], c["bribe"])
            net_parent_res = net_of(wbnb_parent_res, c["borrow"], c["gas"], c["bribe"])
            net_victim_res = net_of(wbnb_victim_res, c["borrow"], c["gas"], c["bribe"])
            log(f"  hop1 {src1} token_out={tok} ({fmt(tok)})")
            log(f"  hop2 {src2} usdt_out={usdt_out} ({fmt(usdt_out)})")
            log(f"  hop3 CHAIN {src3} wbnb={wbnb_live} ({fmt(wbnb_live)}) net={net_live} ({fmt(net_live)})")
            log(f"  hop3 SNAP_CPMM wbnb={wbnb_snap} ({fmt(wbnb_snap)}) net={net_snap} ({fmt(net_snap)})")
            log(f"  hop3 PARENT_RES_CPMM wbnb={wbnb_parent_res} ({fmt(wbnb_parent_res)}) net={net_parent_res}")
            log(f"  hop3 VICTIM_RES_CPMM wbnb={wbnb_victim_res} ({fmt(wbnb_victim_res)}) net={net_victim_res}")
            row["blocks"][tag] = {
                "block": b,
                "hop1_token": tok,
                "hop1_src": src1,
                "hop2_usdt": usdt_out,
                "hop2_src": src2,
                "hop3_chain_wbnb": wbnb_live,
                "hop3_snap_wbnb": wbnb_snap,
                "hop3_parent_res_wbnb": wbnb_parent_res,
                "hop3_victim_res_wbnb": wbnb_victim_res,
                "net_chain": net_live,
                "net_snap": net_snap,
                "net_parent_res": net_parent_res,
                "net_victim_res": net_victim_res,
            }

        # V2 buy reserves parent/victim if applicable
        if c["buy_kind"] == "v2":
            r0p, r1p, _ = reserves(urls, c["pair_buy"], parent)
            r0v, r1v, _ = reserves(urls, c["pair_buy"], block)
            t0, _ = addr_call(urls, c["pair_buy"], SEL_TOKEN0, block)
            if t0.lower() == WBNB:
                rq_p, rt_p, rq_v, rt_v = r0p, r1p, r0v, r1v
            else:
                rq_p, rt_p, rq_v, rt_v = r1p, r0p, r1v, r0v
            log(f"v2 buy parent reserve_quote={rq_p} reserve_token={rt_p}")
            log(f"v2 buy victim reserve_quote={rq_v} reserve_token={rt_v}")
            row["v2_buy_parent"] = {"rq": rq_p, "rt": rt_p}
            row["v2_buy_victim"] = {"rq": rq_v, "rt": rt_v}
            paper_h1_parent = v2_out(c["borrow"], rq_p, rt_p)
            paper_h1_victim = v2_out(c["borrow"], rq_v, rt_v)
            log(f"v2 hop1 CPMM parent={paper_h1_parent} ({fmt(paper_h1_parent)}) victim={paper_h1_victim} ({fmt(paper_h1_victim)})")
            row["v2_hop1_cpmm_parent"] = paper_h1_parent
            row["v2_hop1_cpmm_victim"] = paper_h1_victim

        all_out["cases"].append(row)

    OUT.write_text(json.dumps(all_out, indent=2))
    log(f"\nWROTE {OUT}")


if __name__ == "__main__":
    main()

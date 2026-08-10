#!/usr/bin/env python3
"""Verify stratum header packing + cgminer-compatible nonce submit."""
from __future__ import annotations

import hashlib
import sys


def dsha(b: bytes) -> bytes:
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def swab32(b: bytes) -> bytes:
    assert len(b) == 4
    return b[::-1]


def swab256(b: bytes) -> bytes:
    assert len(b) == 32
    out = bytearray()
    for i in range(8):
        out.extend(b[i * 4 : i * 4 + 4][::-1])
    return bytes(out)


def pack_wire_header(
    version_hex: str, prev_hex: str, merkle: bytes, ntime_hex: str, nbits_hex: str, nonce_le: bytes
) -> bytes:
    """Board hashes true Bitcoin wire headers (not cgminer getwork order)."""
    version = swab32(bytes.fromhex(version_hex))
    prev = swab256(bytes.fromhex(prev_hex))
    ntime = swab32(bytes.fromhex(ntime_hex))
    nbits = swab32(bytes.fromhex(nbits_hex))
    return version + prev + merkle + ntime + nbits + nonce_le


def main() -> int:
    merkle = bytes.fromhex("3ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a")
    hdr = pack_wire_header(
        "00000001",
        "0000000000000000000000000000000000000000000000000000000000000000",
        merkle,
        "495fab29",
        "1d00ffff",
        bytes.fromhex("1dac2b7c"),
    )
    expected = "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"
    got = dsha(hdr)[::-1].hex()
    print(f"genesis via stratum→wire packing: {got}")
    print(f"expected:                         {expected}")
    if got != expected:
        print("FAIL header endianness")
        return 1

    # Wire nonce bytes 1d ac 2b 7c → uint32 LE 0x7c2bac1d
    nonce_u = int.from_bytes(hdr[76:80], "little")
    if nonce_u != 0x7C2BAC1D:
        print(f"FAIL unexpected genesis nonce uint32 {nonce_u:#x}")
        return 1

    # cgminer stratum submit: bin2hex(work->data+76) in getwork order
    # = swab32(wire) = printf("%08x", nonce_u) on LE hosts.
    stratum_nonce = f"{nonce_u:08x}"
    wire_hex = hdr[76:80].hex()
    print(f"wire header nonce bytes: {wire_hex}")
    print(f"stratum submit nonce:    {stratum_nonce}")
    if stratum_nonce != "7c2bac1d":
        print("FAIL stratum nonce format")
        return 1
    if wire_hex == stratum_nonce:
        print("FAIL wire hex must differ from stratum submit for this nonce")
        return 1

    # Extranonce2: cgminer writes htole64(counter) truncated to n2size.
    counter = 0x01020304
    en2 = counter.to_bytes(8, "little")[:4]
    if en2.hex() != "04030201":
        print("FAIL extranonce2 LE packing", en2.hex())
        return 1

    print("PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Verify stratum header packing endianness matches Bitcoin block headers."""
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


def pack_header(version_hex: str, prev_hex: str, merkle: bytes, ntime_hex: str, nbits_hex: str, nonce_le: bytes) -> bytes:
    version = swab32(bytes.fromhex(version_hex))
    prev = swab256(bytes.fromhex(prev_hex))
    ntime = swab32(bytes.fromhex(ntime_hex))
    nbits = swab32(bytes.fromhex(nbits_hex))
    return version + prev + merkle + ntime + nbits + nonce_le


def main() -> int:
    # Genesis reconstructed as if stratum sent BE hex fields.
    merkle = bytes.fromhex("3ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a")
    hdr = pack_header(
        "00000001",
        "0000000000000000000000000000000000000000000000000000000000000000",
        merkle,
        "495fab29",
        "1d00ffff",
        bytes.fromhex("1dac2b7c"),
    )
    expected = "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"
    got = dsha(hdr)[::-1].hex()
    print(f"genesis via stratum packing: {got}")
    print(f"expected:                    {expected}")
    if got != expected:
        print("FAIL header endianness")
        return 1

    # Nonce submit must be the 4 LE header bytes as hex (cgminer bin2hex of data+76),
    # not printf("%08x") of the uint32 value.
    # Genesis header ends with bytes 1d ac 2b 7c → uint32 LE value 0x7c2bac1d.
    nonce_u = int.from_bytes(hdr[76:80], "little")
    if nonce_u != 0x7C2BAC1D:
        print(f"FAIL unexpected genesis nonce uint32 {nonce_u:#x}")
        return 1
    legacy = f"{nonce_u:08x}"  # 7c2bac1d — wrong for stratum submit
    correct = bytes(
        [
            nonce_u & 0xFF,
            (nonce_u >> 8) & 0xFF,
            (nonce_u >> 16) & 0xFF,
            (nonce_u >> 24) & 0xFF,
        ]
    ).hex()
    print(f"legacy submit hex:  {legacy}")
    print(f"correct submit hex: {correct}")
    if legacy == correct:
        print("FAIL expected legacy != correct for this nonce")
        return 1
    if correct != "1dac2b7c":
        print("FAIL nonce submit format")
        return 1
    if hdr[76:80].hex() != correct:
        print("FAIL nonce in header")
        return 1

    print("PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())

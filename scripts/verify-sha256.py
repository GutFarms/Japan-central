#!/usr/bin/env python3
"""Host-side check that Bitcoin SHA256d midstate path matches genesis header.

Also validates the same midstate word layout the ESP32 firmware uses
(custom compressor, not hashlib.copy()).
"""
from __future__ import annotations

import hashlib
import struct
import sys

# Bitcoin genesis block header (80 bytes)
HEADER = bytes.fromhex(
    "01000000"
    "0000000000000000000000000000000000000000000000000000000000000000"
    "3ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a"
    "29ab5f49"
    "ffff001d"
    "1dac2b7c"
)
# Display hash (byte-reversed SHA256d)
EXPECTED = "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"

K = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
    0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
    0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
    0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
    0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
    0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
    0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
    0xc67178f2,
]
IV = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
]


def rotr(x: int, n: int) -> int:
    return ((x >> n) | (x << (32 - n))) & 0xFFFFFFFF


def be32(data: bytes, off: int = 0) -> int:
    return struct.unpack(">I", data[off : off + 4])[0]


def transform(state: list[int], block: list[int]) -> list[int]:
    w = list(block) + [0] * 48
    for i in range(16, 64):
        s0 = rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >> 3)
        s1 = rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >> 10)
        w[i] = (w[i - 16] + s0 + w[i - 7] + s1) & 0xFFFFFFFF
    a, b, c, d, e, f, g, h = state
    for i in range(64):
        S1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)
        ch = (e & f) ^ ((~e) & g)
        t1 = (h + S1 + ch + K[i] + w[i]) & 0xFFFFFFFF
        S0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)
        maj = (a & b) ^ (a & c) ^ (b & c)
        t2 = (S0 + maj) & 0xFFFFFFFF
        h, g, f, e, d, c, b, a = g, f, e, (d + t1) & 0xFFFFFFFF, c, b, a, (t1 + t2) & 0xFFFFFFFF
    return [(state[i] + v) & 0xFFFFFFFF for i, v in enumerate([a, b, c, d, e, f, g, h])]


def state_to_bytes(st: list[int]) -> bytes:
    return b"".join(struct.pack(">I", x) for x in st)


def firmware_midstate_sha256d(header: bytes) -> bytes:
    """Mirror firmware-cpp/src/sha256_miner.cpp hot path."""
    assert len(header) == 80
    w1 = [be32(header, i * 4) for i in range(16)]
    mid = transform(list(IV), w1)
    nonce = struct.unpack("<I", header[76:80])[0]
    nonce_word = struct.unpack(">I", struct.pack("<I", nonce))[0]
    w2 = [0] * 16
    w2[0] = be32(header, 64)
    w2[1] = be32(header, 68)
    w2[2] = be32(header, 72)
    w2[3] = nonce_word
    w2[4] = 0x80000000
    w2[15] = 640
    first = state_to_bytes(transform(mid, w2))
    # Second SHA-256 of 32-byte digest
    w3 = [be32(first, i * 4) for i in range(8)] + [0] * 8
    w3[8] = 0x80000000
    w3[15] = 256
    return state_to_bytes(transform(list(IV), w3))


def sha256d(data: bytes) -> bytes:
    return hashlib.sha256(hashlib.sha256(data).digest()).digest()


def midstate_sha256d(header: bytes) -> bytes:
    assert len(header) == 80
    h = hashlib.sha256()
    h.update(header[:64])
    h2 = h.copy()
    h2.update(header[64:])
    first = h2.digest()
    return hashlib.sha256(first).digest()


def main() -> int:
    full = sha256d(HEADER)
    mid = midstate_sha256d(HEADER)
    fw = firmware_midstate_sha256d(HEADER)
    be_full = full[::-1].hex()
    be_mid = mid[::-1].hex()
    be_fw = fw[::-1].hex()
    print(f"genesis SHA256d (full):       {be_full}")
    print(f"genesis SHA256d (hashlib mid):{be_mid}")
    print(f"genesis SHA256d (firmware):   {be_fw}")
    print(f"expected:                     {EXPECTED}")
    if be_full != EXPECTED or be_mid != EXPECTED or be_fw != EXPECTED:
        print("FAIL")
        return 1

    # Mutate nonce and ensure firmware path == hashlib
    for nonce in (0, 1, 0x1DAC2B7C, 0xFFFFFFFF):
        hdr = bytearray(HEADER)
        hdr[76:80] = struct.pack("<I", nonce)
        a = sha256d(bytes(hdr))
        b = firmware_midstate_sha256d(bytes(hdr))
        if a != b:
            print(f"FAIL nonce={nonce:#x}")
            return 1
    print("PASS (genesis + nonce sweep)")
    return 0


if __name__ == "__main__":
    sys.exit(main())

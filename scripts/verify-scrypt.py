#!/usr/bin/env python3
"""Host-side check that Litecoin scrypt PoW + TMTO path matches known vectors."""
from __future__ import annotations

import hashlib
import struct
import sys

HEADER_HEX = (
    "01000000f615f7ce3b4fc6b8f61e8f89aedb1d0852507650533a9e3b10b9bbcc30639f27"
    "9fcaa86746e1ef52d3edb3c4ad8259920d509bd073605c9bf1d59983752a6b06b817bb4e"
    "a78e011d012d59d4"
)
EXPECTED_BE = "0000000110c8357966576df46f3b802ca897deb7ad18b12f1c24ecff6386ebd9"


def rotl(x: int, n: int) -> int:
    return ((x << n) | (x >> (32 - n))) & 0xFFFFFFFF


def qr(state, a, b, c, d) -> None:
    state[b] ^= rotl((state[a] + state[d]) & 0xFFFFFFFF, 7)
    state[c] ^= rotl((state[b] + state[a]) & 0xFFFFFFFF, 9)
    state[d] ^= rotl((state[c] + state[b]) & 0xFFFFFFFF, 13)
    state[a] ^= rotl((state[d] + state[c]) & 0xFFFFFFFF, 18)


def salsa20_8(block: bytearray) -> None:
    x = list(struct.unpack("<16I", block))
    orig = x[:]
    for _ in range(4):
        qr(x, 0, 4, 8, 12)
        qr(x, 5, 9, 13, 1)
        qr(x, 10, 14, 2, 6)
        qr(x, 15, 3, 7, 11)
        qr(x, 0, 1, 2, 3)
        qr(x, 5, 6, 7, 4)
        qr(x, 10, 11, 8, 9)
        qr(x, 15, 12, 13, 14)
    out = [(x[i] + orig[i]) & 0xFFFFFFFF for i in range(16)]
    block[:] = struct.pack("<16I", *out)


def blockmix(b: bytes) -> bytearray:
    x = bytearray(b[64:128])
    y = bytearray(128)
    for i in range(2):
        for j in range(64):
            x[j] ^= b[i * 64 + j]
        salsa20_8(x)
        dest = (i // 2) * 64 if i % 2 == 0 else (1 + (i - 1) // 2) * 64
        y[dest : dest + 64] = x
    return y


def romix_tmto(b: bytearray, slots: int = 64) -> None:
    n = 1024
    stride = n // slots
    v = [None] * slots
    x = bytearray(b)
    for i in range(n):
        if i % stride == 0:
            v[i // stride] = bytearray(x)
        x[:] = blockmix(x)
    for _ in range(n):
        j = struct.unpack_from("<Q", x, 64)[0] % n
        base = (j // stride) * stride
        slot = base // stride
        t = bytearray(v[slot])
        for _i in range(base, j):
            t = blockmix(t)
        for k in range(128):
            x[k] ^= t[k]
        x[:] = blockmix(x)
    b[:] = x


def scrypt_tmto(password: bytes, salt: bytes, dklen: int = 32) -> bytes:
    b = bytearray(hashlib.pbkdf2_hmac("sha256", password, salt, 1, dklen=128))
    romix_tmto(b)
    return hashlib.pbkdf2_hmac("sha256", password, bytes(b), 1, dklen=dklen)


def main() -> int:
    header = bytes.fromhex(HEADER_HEX)
    std = hashlib.scrypt(header, salt=header, n=1024, r=1, p=1, dklen=32)
    tmto = scrypt_tmto(header, header)
    be_std = std[::-1].hex()
    be_tmto = tmto[::-1].hex()
    ok = be_std == EXPECTED_BE and be_tmto == EXPECTED_BE and std == tmto
    print(f"Litecoin block #29255 PoW (stdlib): {be_std}")
    print(f"Litecoin block #29255 PoW (TMTO64): {be_tmto}")
    print(f"expected:                           {EXPECTED_BE}")
    print("PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Host-side check that Bitcoin SHA256d midstate path matches genesis header."""
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


def sha256d(data: bytes) -> bytes:
    return hashlib.sha256(hashlib.sha256(data).digest()).digest()


def midstate_sha256d(header: bytes) -> bytes:
    assert len(header) == 80
    h = hashlib.sha256()
    h.update(header[:64])
    # Finish with last 16 bytes
    h2 = h.copy()
    h2.update(header[64:])
    first = h2.digest()
    return hashlib.sha256(first).digest()


def main() -> int:
    full = sha256d(HEADER)
    mid = midstate_sha256d(HEADER)
    be_full = full[::-1].hex()
    be_mid = mid[::-1].hex()
    print(f"genesis SHA256d (full):     {be_full}")
    print(f"genesis SHA256d (midstate): {be_mid}")
    print(f"expected:                   {EXPECTED}")
    if be_full != EXPECTED or be_mid != EXPECTED:
        print("FAIL")
        return 1
    print("PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())

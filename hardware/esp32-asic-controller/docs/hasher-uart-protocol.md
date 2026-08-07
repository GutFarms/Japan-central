# Hasher UART protocol (JCHC-HUP v1)

Framing between ESP32 controller and hashboard. Little-endian multi-byte fields.  
Default: **1_500_000** 8N1. Bring-up: **115200** 8N1.

## Frame layout

```
  0     1     2        3..4       5..4+LEN     last
+-----+-----+-----+-----------+-------------+------+
| SOF | CMD | FLAGS| LEN u16LE | PAYLOAD     | CRC8 |
+-----+-----+-----+-----------+-------------+------+
```

| Field | Value |
|-------|-------|
| SOF | `0xA5` |
| CMD | see table |
| FLAGS | bit0 = expect ACK; bit1 = compressed (RFU) |
| LEN | payload length 0…1024 |
| CRC8 | Dallas/Maxim CRC8 over `CMD..PAYLOAD` (poly `0x31`, init `0x00`) |

Max frame ≈ 1 + 1 + 1 + 2 + 1024 + 1 = 1030 bytes.

## Commands (host → hasher)

| CMD | Name | Payload | Response |
|-----|------|---------|----------|
| `0x01` | HELLO | `u16` proto version (=1) | `0x81` INFO |
| `0x02` | GET_INFO | empty | `0x81` INFO |
| `0x10` | SET_JOB | job blob (see below) | `0x90` ACK or `0x91` NACK |
| `0x11` | ABORT | empty | `0x90` |
| `0x12` | SET_RANGE | `start_nonce u32`, `count u32` | `0x90` |
| `0x20` | SET_CLOCK | `mhz u16` | `0x90` / `0x91` |
| `0x21` | SET_VOLTAGE_MV | `mv u16` (hint only) | `0x90` |

## Commands (hasher → host)

| CMD | Name | Payload |
|-----|------|---------|
| `0x81` | INFO | `proto_u16`, `caps_u32`, `name[16]` UTF-8 padded |
| `0x90` | ACK | `ref_cmd u8` |
| `0x91` | NACK | `ref_cmd u8`, `err u8` |
| `0xA0` | SHARE | `nonce u32`, `hash[32]`, `job_tag u32` |
| `0xA1` | STATUS | `hashes_done u64`, `temp_c_x10 i16`, `faults u16` |
| `0xEE` | ERROR | `code u8` |

### Caps bits (`caps_u32`)

| Bit | Meaning |
|-----|---------|
| 0 | Scrypt N=1024 r=1 p=1 |
| 1 | Scrypt N=64 (lite) |
| 2 | Returns full hash in SHARE |
| 3 | Hardware midstate assist |
| 8 | FPGA simulator |
| 9 | Real ASIC |

## SET_JOB payload (112 bytes)

Matches this repo’s miner job shape:

| Offset | Size | Field |
|--------|------|-------|
| 0 | 80 | `header[80]` — nonce field at bytes 76..80 is start hint (hasher may overwrite) |
| 80 | 32 | `target[32]` — little-endian uint256; share if hash &lt; target |
| 112 | 4 | `job_tag u32` — echoed in SHARE (stratum job correlation) |
| 116 | 4 | `nbits_hint u32` — optional; 0 if unused |

Total LEN = 120.

Share rule: same as `ScryptMiner` — scrypt(header_with_nonce) LE compare against target.

## Sequence

```
Controller                         Hasher
    | -- HELLO ------------------> |
    | <- INFO -------------------- |
    | -- SET_RANGE ---------------> |
    | -- SET_JOB ----------------> |
    | <- ACK --------------------- |
    |                              | … hashing …
    | <- SHARE (maybe many) ------ |
    | -- ABORT (on new stratum) -> |
```

On new stratum `mining.notify`, controller sends `ABORT` then `SET_JOB` with new header/target/`job_tag`.

## Error codes (`0x91` / `0xEE`)

| Code | Meaning |
|------|---------|
| 1 | Bad CRC |
| 2 | Unknown CMD |
| 3 | Busy |
| 4 | Bad LEN |
| 5 | Unsupported params |
| 6 | Thermal shutdown |
| 7 | VCORE fault |

## CRC8 reference (Rust)

```rust
fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            if crc & 0x80 != 0 { crc = (crc << 1) ^ 0x31; }
            else { crc <<= 1; }
        }
    }
    crc
}
```

## Loopback / sim

Protocol bring-up without a hashboard: use the standalone `firmware-stub` crate
(`cargo test` there) or a logic analyzer on J1. Full CPU mining / stratum stays
in the **separate** miner firmware project — do not treat this hardware package
as an application firmware tree.

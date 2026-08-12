//! JCHC Hasher UART Protocol (HUP v1) — reference codec for the controller board.
//!
//! Framing matches `../docs/hasher-uart-protocol.md`.
//! This crate is part of the **hardware** package, not the miner application.

#![cfg_attr(not(test), no_std)]

pub const SOF: u8 = 0xA5;
pub const PROTO_VERSION: u16 = 1;
pub const MAX_PAYLOAD: usize = 1024;
pub const SET_JOB_LEN: usize = 120;
pub const HEADER_LEN: usize = 80;
pub const HASH_LEN: usize = 32;

pub const CMD_HELLO: u8 = 0x01;
pub const CMD_GET_INFO: u8 = 0x02;
pub const CMD_SET_JOB: u8 = 0x10;
pub const CMD_ABORT: u8 = 0x11;
pub const CMD_SET_RANGE: u8 = 0x12;
pub const CMD_INFO: u8 = 0x81;
pub const CMD_ACK: u8 = 0x90;
pub const CMD_NACK: u8 = 0x91;
pub const CMD_SHARE: u8 = 0xA0;
pub const CMD_STATUS: u8 = 0xA1;
pub const CMD_ERROR: u8 = 0xEE;

pub const CAP_SCRYPT_N1024: u32 = 1 << 0;
pub const CAP_SCRYPT_N64: u32 = 1 << 1;
pub const CAP_FULL_HASH: u32 = 1 << 2;
pub const CAP_FPGA_SIM: u32 = 1 << 8;
pub const CAP_REAL_ASIC: u32 = 1 << 9;

/// Dallas/Maxim CRC8 (poly 0x31, init 0x00) over CMD..PAYLOAD.
pub fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            if crc & 0x80 != 0 {
                crc = (crc << 1) ^ 0x31;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodecError {
    BufferTooSmall,
    BadSof,
    BadLen,
    BadCrc,
    Truncated,
    BadPayload,
}

/// Encode a frame into `out`. Returns bytes written.
pub fn encode_frame(cmd: u8, flags: u8, payload: &[u8], out: &mut [u8]) -> Result<usize, CodecError> {
    if payload.len() > MAX_PAYLOAD {
        return Err(CodecError::BadLen);
    }
    let total = 5 + payload.len() + 1;
    if out.len() < total {
        return Err(CodecError::BufferTooSmall);
    }
    out[0] = SOF;
    out[1] = cmd;
    out[2] = flags;
    let len = payload.len() as u16;
    out[3] = (len & 0xff) as u8;
    out[4] = (len >> 8) as u8;
    out[5..5 + payload.len()].copy_from_slice(payload);
    let crc = crc8(&out[1..5 + payload.len()]);
    out[5 + payload.len()] = crc;
    Ok(total)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub cmd: u8,
    pub flags: u8,
    pub payload_off: usize,
    pub payload_len: usize,
    pub frame_len: usize,
}

pub fn decode_frame(buf: &[u8]) -> Result<Frame, CodecError> {
    if buf.is_empty() {
        return Err(CodecError::Truncated);
    }
    if buf[0] != SOF {
        return Err(CodecError::BadSof);
    }
    if buf.len() < 5 {
        return Err(CodecError::Truncated);
    }
    let cmd = buf[1];
    let flags = buf[2];
    let len = u16::from_le_bytes([buf[3], buf[4]]) as usize;
    if len > MAX_PAYLOAD {
        return Err(CodecError::BadLen);
    }
    let total = 5 + len + 1;
    if buf.len() < total {
        return Err(CodecError::Truncated);
    }
    let expect = crc8(&buf[1..5 + len]);
    if buf[5 + len] != expect {
        return Err(CodecError::BadCrc);
    }
    Ok(Frame {
        cmd,
        flags,
        payload_off: 5,
        payload_len: len,
        frame_len: total,
    })
}

pub fn build_set_job(
    header: &[u8; HEADER_LEN],
    target: &[u8; HASH_LEN],
    job_tag: u32,
    nbits_hint: u32,
    out: &mut [u8; SET_JOB_LEN],
) {
    out[0..HEADER_LEN].copy_from_slice(header);
    out[HEADER_LEN..HEADER_LEN + HASH_LEN].copy_from_slice(target);
    out[112..116].copy_from_slice(&job_tag.to_le_bytes());
    out[116..120].copy_from_slice(&nbits_hint.to_le_bytes());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharePayload {
    pub nonce: u32,
    pub hash: [u8; HASH_LEN],
    pub job_tag: u32,
}

pub fn parse_share(payload: &[u8]) -> Result<SharePayload, CodecError> {
    if payload.len() < 4 + HASH_LEN + 4 {
        return Err(CodecError::BadPayload);
    }
    let mut hash = [0u8; HASH_LEN];
    hash.copy_from_slice(&payload[4..4 + HASH_LEN]);
    Ok(SharePayload {
        nonce: u32::from_le_bytes(payload[0..4].try_into().unwrap()),
        hash,
        job_tag: u32::from_le_bytes(payload[4 + HASH_LEN..8 + HASH_LEN].try_into().unwrap()),
    })
}

pub fn build_share(nonce: u32, hash: &[u8; HASH_LEN], job_tag: u32, out: &mut [u8; 40]) {
    out[0..4].copy_from_slice(&nonce.to_le_bytes());
    out[4..36].copy_from_slice(hash);
    out[36..40].copy_from_slice(&job_tag.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_roundtrip_frame() {
        let payload = b"hello";
        let mut buf = [0u8; 64];
        let n = encode_frame(CMD_HELLO, 1, payload, &mut buf).unwrap();
        let f = decode_frame(&buf[..n]).unwrap();
        assert_eq!(f.cmd, CMD_HELLO);
        assert_eq!(f.flags, 1);
        assert_eq!(&buf[f.payload_off..f.payload_off + f.payload_len], payload);
    }

    #[test]
    fn bad_crc_rejected() {
        let mut buf = [0u8; 64];
        let n = encode_frame(CMD_ABORT, 0, &[], &mut buf).unwrap();
        buf[n - 1] ^= 0xff;
        assert_eq!(decode_frame(&buf[..n]), Err(CodecError::BadCrc));
    }

    #[test]
    fn set_job_payload_layout() {
        let header = [0x11u8; HEADER_LEN];
        let target = [0x22u8; HASH_LEN];
        let mut job = [0u8; SET_JOB_LEN];
        build_set_job(&header, &target, 0xAABBCCDD, 0x100, &mut job);
        assert_eq!(&job[0..80], &header);
        assert_eq!(&job[80..112], &target);
        assert_eq!(&job[112..116], &0xAABBCCDDu32.to_le_bytes());
        let mut frame = [0u8; 256];
        let n = encode_frame(CMD_SET_JOB, 1, &job, &mut frame).unwrap();
        let f = decode_frame(&frame[..n]).unwrap();
        assert_eq!(f.cmd, CMD_SET_JOB);
        assert_eq!(f.payload_len, SET_JOB_LEN);
    }

    #[test]
    fn share_roundtrip() {
        let hash = [0x5Au8; HASH_LEN];
        let mut p = [0u8; 40];
        build_share(42, &hash, 7, &mut p);
        let s = parse_share(&p).unwrap();
        assert_eq!(s.nonce, 42);
        assert_eq!(s.job_tag, 7);
        assert_eq!(s.hash, hash);
    }
}

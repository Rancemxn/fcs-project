//! Little-endian primitive codec helpers (FCBC §1, §6).

use crate::error::{FcbcError, FcbcResult};

/// CRC-32/ISO-HDLC for section payloads (poly reflected `0xEDB88320`).
pub fn section_crc32_iso_hdlc(bytes: &[u8]) -> u32 {
    const ALGO: crc::Algorithm<u32> = crc::CRC_32_ISO_HDLC;
    let crc = crc::Crc::<u32>::new(&ALGO);
    crc.checksum(bytes)
}

pub fn encode_u8(value: u8) -> [u8; 1] {
    [value]
}

pub fn decode_u8(bytes: &[u8]) -> FcbcResult<(u8, &[u8])> {
    let (head, rest) = bytes
        .split_first()
        .ok_or_else(|| FcbcError::new("fcbc.invalid-record", "unexpected end while reading u8"))?;
    Ok((*head, rest))
}

pub fn encode_u16_le(value: u16) -> [u8; 2] {
    value.to_le_bytes()
}

pub fn decode_u16_le(bytes: &[u8]) -> FcbcResult<(u16, &[u8])> {
    if bytes.len() < 2 {
        return Err(FcbcError::new(
            "fcbc.invalid-record",
            "unexpected end while reading u16",
        ));
    }
    let mut raw = [0u8; 2];
    raw.copy_from_slice(&bytes[..2]);
    Ok((u16::from_le_bytes(raw), &bytes[2..]))
}

pub fn encode_u32_le(value: u32) -> [u8; 4] {
    value.to_le_bytes()
}

pub fn decode_u32_le(bytes: &[u8]) -> FcbcResult<(u32, &[u8])> {
    if bytes.len() < 4 {
        return Err(FcbcError::new(
            "fcbc.invalid-record",
            "unexpected end while reading u32",
        ));
    }
    let mut raw = [0u8; 4];
    raw.copy_from_slice(&bytes[..4]);
    Ok((u32::from_le_bytes(raw), &bytes[4..]))
}

pub fn encode_u64_le(value: u64) -> [u8; 8] {
    value.to_le_bytes()
}

pub fn decode_u64_le(bytes: &[u8]) -> FcbcResult<(u64, &[u8])> {
    if bytes.len() < 8 {
        return Err(FcbcError::new(
            "fcbc.invalid-record",
            "unexpected end while reading u64",
        ));
    }
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&bytes[..8]);
    Ok((u64::from_le_bytes(raw), &bytes[8..]))
}

pub fn encode_i64_le(value: i64) -> [u8; 8] {
    value.to_le_bytes()
}

pub fn decode_i64_le(bytes: &[u8]) -> FcbcResult<(i64, &[u8])> {
    let (raw, rest) = decode_u64_le(bytes)?;
    Ok((raw as i64, rest))
}

pub fn encode_f64_le(value: f64) -> FcbcResult<[u8; 8]> {
    if !value.is_finite() {
        return Err(FcbcError::new(
            "fcbc.invalid-float",
            "FCBC forbids NaN and Infinity in f64 fields",
        ));
    }
    Ok(value.to_le_bytes())
}

pub fn decode_f64_le(bytes: &[u8]) -> FcbcResult<(f64, &[u8])> {
    if bytes.len() < 8 {
        return Err(FcbcError::new(
            "fcbc.invalid-record",
            "unexpected end while reading f64",
        ));
    }
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&bytes[..8]);
    let value = f64::from_le_bytes(raw);
    if !value.is_finite() {
        return Err(FcbcError::new(
            "fcbc.invalid-float",
            "FCBC forbids NaN and Infinity in f64 fields",
        ));
    }
    Ok((value, &bytes[8..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_integers_and_finite_floats() {
        assert_eq!(decode_u16_le(&encode_u16_le(0xABCD)).unwrap().0, 0xABCD);
        assert_eq!(
            decode_u32_le(&encode_u32_le(0xDEAD_BEEF)).unwrap().0,
            0xDEAD_BEEF
        );
        assert_eq!(
            decode_u64_le(&encode_u64_le(0x0123_4567_89AB_CDEF))
                .unwrap()
                .0,
            0x0123_4567_89AB_CDEF
        );
        assert_eq!(decode_i64_le(&encode_i64_le(-42)).unwrap().0, -42);
        let encoded = encode_f64_le(-0.0).unwrap();
        let (value, _) = decode_f64_le(&encoded).unwrap();
        assert!(value.is_sign_negative());
        assert_eq!(value, 0.0);
    }

    #[test]
    fn rejects_non_finite_floats() {
        assert!(encode_f64_le(f64::NAN).is_err());
        assert!(encode_f64_le(f64::INFINITY).is_err());
        let mut nan = f64::NAN.to_le_bytes();
        // Force a quiet NaN encoding into the decoder path.
        nan[0] = 1;
        assert!(decode_f64_le(&nan).is_err());
    }

    #[test]
    fn empty_payload_crc_is_zero() {
        assert_eq!(section_crc32_iso_hdlc(&[]), 0);
    }
}

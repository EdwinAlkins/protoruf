//! Protobuf wire-format primitives: varints, zigzag, tags and length prefixes.

pub const WIRE_VARINT: u8 = 0;
pub const WIRE_I64: u8 = 1;
pub const WIRE_LEN: u8 = 2;
pub const WIRE_I32: u8 = 5;

/// Read a base-128 varint at `*pos`, advancing it.
#[inline]
pub fn read_varint(data: &[u8], pos: &mut usize) -> Result<u64, String> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    while *pos < data.len() {
        let byte = data[*pos];
        *pos += 1;
        if shift == 63 && byte > 1 {
            return Err("varint overflows 64 bits".into());
        }
        result |= u64::from(byte & 0x7f) << shift;
        if byte < 0x80 {
            return Ok(result);
        }
        shift += 7;
        if shift > 63 {
            return Err("varint too long".into());
        }
    }
    Err("truncated varint".into())
}

#[inline]
pub fn read_fixed32(data: &[u8], pos: &mut usize) -> Result<u32, String> {
    let end = *pos + 4;
    let bytes = data.get(*pos..end).ok_or("truncated fixed32")?;
    *pos = end;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

#[inline]
pub fn read_fixed64(data: &[u8], pos: &mut usize) -> Result<u64, String> {
    let end = *pos + 8;
    let bytes = data.get(*pos..end).ok_or("truncated fixed64")?;
    *pos = end;
    Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
}

/// Largest field number protobuf allows.
pub const MAX_FIELD_NUMBER: u64 = (1 << 29) - 1;

/// Read a field tag at `*pos`: its number (1 to 2^29 - 1) and wire type. Groups (wire
/// types 3 and 4) are not supported, so only 0, 1, 2 and 5 are accepted.
#[inline]
pub fn read_tag(data: &[u8], pos: &mut usize) -> Result<(u32, u8), String> {
    let tag = read_varint(data, pos)?;
    let number = tag >> 3;
    if number == 0 || number > MAX_FIELD_NUMBER {
        return Err(format!("invalid field number {number}"));
    }
    let wire_type = (tag & 7) as u8;
    if !matches!(wire_type, WIRE_VARINT | WIRE_I64 | WIRE_LEN | WIRE_I32) {
        return Err(format!("unsupported wire type {wire_type}"));
    }
    Ok((number as u32, wire_type))
}

/// Read a length prefix and return the delimited slice.
#[inline]
pub fn read_len<'a>(data: &'a [u8], pos: &mut usize) -> Result<&'a [u8], String> {
    let len = read_varint(data, pos)? as usize;
    let end = pos.checked_add(len).ok_or("length overflow")?;
    let slice = data
        .get(*pos..end)
        .ok_or("truncated length-delimited field")?;
    *pos = end;
    Ok(slice)
}

/// Skip a field whose number is unknown to the plan.
pub fn skip_field(data: &[u8], pos: &mut usize, wire_type: u8) -> Result<(), String> {
    match wire_type {
        WIRE_VARINT => read_varint(data, pos).map(|_| ()),
        WIRE_I64 => read_fixed64(data, pos).map(|_| ()),
        WIRE_LEN => read_len(data, pos).map(|_| ()),
        WIRE_I32 => read_fixed32(data, pos).map(|_| ()),
        other => Err(format!("unsupported wire type {other}")),
    }
}

#[inline]
pub fn write_varint(buf: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        buf.push((value as u8) | 0x80);
        value >>= 7;
    }
    buf.push(value as u8);
}

#[inline]
pub fn varint_len(value: u64) -> usize {
    // 1 byte per 7 significant bits, at least one byte.
    (64 - (value | 1).leading_zeros() as usize).div_ceil(7)
}

#[inline]
pub fn encode_tag(number: u32, wire_type: u8) -> u64 {
    (u64::from(number) << 3) | u64::from(wire_type)
}

#[inline]
pub fn zigzag32(value: i32) -> u64 {
    u64::from(((value << 1) ^ (value >> 31)) as u32)
}

#[inline]
pub fn zigzag64(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

#[inline]
pub fn unzigzag32(value: u64) -> i32 {
    let v = value as u32;
    ((v >> 1) as i32) ^ -((v & 1) as i32)
}

#[inline]
pub fn unzigzag64(value: u64) -> i64 {
    ((value >> 1) as i64) ^ -((value & 1) as i64)
}

/// Write a length-delimited payload produced by `body` in place.
///
/// One byte is reserved for the length; if the payload turns out longer than 127
/// bytes, it is shifted right to make room for the full varint. This avoids a
/// separate size pass over the (Python) source values.
#[inline]
pub fn write_len_delimited<E>(
    buf: &mut Vec<u8>,
    body: impl FnOnce(&mut Vec<u8>) -> Result<(), E>,
) -> Result<(), E> {
    let start = buf.len();
    buf.push(0);
    body(buf)?;
    let len = buf.len() - start - 1;
    let prefix = varint_len(len as u64);
    if prefix == 1 {
        buf[start] = len as u8;
    } else {
        buf.splice(start + 1..start + 1, std::iter::repeat_n(0, prefix - 1));
        let mut value = len as u64;
        for byte in &mut buf[start..start + prefix] {
            *byte = (value as u8) | 0x80;
            value >>= 7;
        }
        buf[start + prefix - 1] &= 0x7f;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_roundtrip() {
        for value in [
            0u64,
            1,
            127,
            128,
            300,
            16_383,
            16_384,
            u32::MAX as u64,
            u64::MAX,
        ] {
            let mut buf = Vec::new();
            write_varint(&mut buf, value);
            assert_eq!(buf.len(), varint_len(value));
            let mut pos = 0;
            assert_eq!(read_varint(&buf, &mut pos).unwrap(), value);
            assert_eq!(pos, buf.len());
        }
    }

    #[test]
    fn zigzag_roundtrip() {
        for value in [0i64, -1, 1, i64::MIN, i64::MAX] {
            assert_eq!(unzigzag64(zigzag64(value)), value);
        }
        for value in [0i32, -1, 1, i32::MIN, i32::MAX] {
            assert_eq!(unzigzag32(zigzag32(value)), value);
        }
    }

    #[test]
    fn len_delimited_shifts_long_payloads() {
        for size in [0usize, 1, 127, 128, 20_000] {
            let mut buf = vec![0xAA];
            write_len_delimited::<()>(&mut buf, |b| {
                b.extend(std::iter::repeat_n(7u8, size));
                Ok(())
            })
            .unwrap();
            let mut pos = 1;
            let payload = read_len(&buf, &mut pos).unwrap();
            assert_eq!(payload.len(), size);
            assert!(payload.iter().all(|&b| b == 7));
            assert_eq!(pos, buf.len());
        }
    }

    #[test]
    fn tags_are_checked() {
        let read = |tag: u64| {
            let mut buf = Vec::new();
            write_varint(&mut buf, tag);
            read_tag(&buf, &mut 0)
        };
        assert_eq!(read(encode_tag(1, WIRE_LEN)), Ok((1, WIRE_LEN)));
        let max = MAX_FIELD_NUMBER as u32;
        assert_eq!(read(encode_tag(max, WIRE_I32)), Ok((max, WIRE_I32)));
        assert!(read(0).is_err()); // field number 0
        assert!(read((MAX_FIELD_NUMBER + 1) << 3).is_err());
        assert!(read(((1u64 << 32) + 1) << 3).is_err()); // used to truncate to field 1
        for wire_type in [3, 4, 6, 7] {
            assert!(read((1 << 3) | wire_type).is_err());
        }
    }

    #[test]
    fn truncated_input_is_an_error() {
        let mut pos = 0;
        assert!(read_varint(&[0x80], &mut pos).is_err());
        let mut pos = 0;
        assert!(read_len(&[5, 1, 2], &mut pos).is_err());
    }
}

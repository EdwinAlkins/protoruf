//! Wire primitives on arbitrary bytes: no panic, positions only move forward and stay in
//! bounds, varints written back read the same.
#![no_main]

use libfuzzer_sys::fuzz_target;
use protoruf::native::wire::{
    read_len, read_tag, read_varint, skip_field, unzigzag32, unzigzag64, varint_len, write_varint,
    zigzag32, zigzag64,
};

fuzz_target!(|data: &[u8]| {
    // The input as a message whose fields are all unknown: every tag checked, every
    // field skipped, as the decoder does.
    let mut pos = 0;
    while pos < data.len() {
        let before = pos;
        let Ok((number, wire_type)) = read_tag(data, &mut pos) else {
            break;
        };
        assert!((1..1 << 29).contains(&number));
        if skip_field(data, &mut pos, wire_type).is_err() {
            break;
        }
        assert!(before < pos && pos <= data.len());
    }

    let mut pos = 0;
    if let Ok(value) = read_varint(data, &mut pos) {
        assert!(pos <= data.len() && pos <= 10);
        let mut buf = Vec::new();
        write_varint(&mut buf, value);
        assert_eq!(buf.len(), varint_len(value));
        assert_eq!(read_varint(&buf, &mut 0), Ok(value));
        assert_eq!(unzigzag64(zigzag64(value as i64)), value as i64);
        assert_eq!(unzigzag32(zigzag32(value as i32)), value as i32);
    }

    let mut pos = 0;
    if let Ok(slice) = read_len(data, &mut pos) {
        assert!(pos <= data.len());
        assert_eq!(&data[pos - slice.len()..pos], slice);
    }
});

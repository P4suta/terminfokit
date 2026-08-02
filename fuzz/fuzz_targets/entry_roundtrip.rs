#![no_main]

use arbitrary::Unstructured;
use libfuzzer_sys::fuzz_target;
use terminfokit::caps::{BooleanCap, NumericCap, StringCap};
use terminfokit::format::SourceFormatter;
use terminfokit::{EntryBuilder, Number};

fuzz_target!(|data: &[u8]| {
    let mut input = Unstructured::new(data);
    let mut entry = EntryBuilder::new("fuzz").unwrap().build();
    let operations = input.int_in_range::<u8>(0..=96).unwrap_or(0);
    for _ in 0..operations {
        let kind = input.int_in_range::<u8>(0..=2).unwrap_or(0);
        let state = input.int_in_range::<u8>(0..=2).unwrap_or(0);
        match kind {
            0 => {
                let index = usize::from(input.arbitrary::<u16>().unwrap_or(0))
                    % BooleanCap::COUNT;
                let cap = BooleanCap::ALL[index];
                match state {
                    0 => entry.set_boolean(cap),
                    1 => entry.cancel_boolean(cap),
                    _ => entry.remove_boolean(cap),
                }
            }
            1 => {
                let index = usize::from(input.arbitrary::<u16>().unwrap_or(0))
                    % NumericCap::COUNT;
                let cap = NumericCap::ALL[index];
                match state {
                    0 => {
                        let value = input.arbitrary::<u32>().unwrap_or(0) & 0x7fff_ffff;
                        let _ = entry.set_number(cap, value as i32);
                    }
                    1 => entry.cancel_number(cap),
                    _ => entry.remove_number(cap),
                }
            }
            _ => {
                let index = usize::from(input.arbitrary::<u16>().unwrap_or(0))
                    % StringCap::COUNT;
                let cap = StringCap::ALL[index];
                match state {
                    0 => {
                        let length = usize::from(input.int_in_range::<u8>(0..=64).unwrap_or(0));
                        if let Ok(bytes) = input.bytes(length) {
                            let value: Vec<_> = bytes.iter().copied().filter(|byte| *byte != 0).collect();
                            let _ = entry.set_string(cap, value);
                        }
                    }
                    1 => entry.cancel_string(cap),
                    _ => entry.remove_string(cap),
                }
            }
        }
    }
    if let Ok(bytes) = entry.to_bytes() {
        if let Ok(decoded) = terminfokit::binary::decode(&bytes) {
            let _ = SourceFormatter::default().format(decoded.entry());
            let _ = decoded.entry().to_bytes();
        }
    }
    let _ = Number::new(i64::from(input.arbitrary::<i32>().unwrap_or(0)));
});

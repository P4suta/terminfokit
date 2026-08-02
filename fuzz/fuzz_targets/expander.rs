#![no_main]
use libfuzzer_sys::fuzz_target;
use terminfokit::expand::Param;

fuzz_target!(|data: &[u8]| {
    let parameter_bytes = data.len().min(9 * 8);
    let mut numbers = [0i64; 9];
    for (index, chunk) in data[..parameter_bytes].chunks(8).enumerate() {
        let mut bytes = [0u8; 8];
        bytes[..chunk.len()].copy_from_slice(chunk);
        numbers[index] = i64::from_le_bytes(bytes);
    }
    let params: Vec<_> = numbers.iter().copied().map(Param::Number).collect();
    let _ = terminfokit::expand::expand(&data[parameter_bytes..], &params);
});

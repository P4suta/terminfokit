#![no_main]
use libfuzzer_sys::fuzz_target;
use terminfokit::format::SourceFormatter;

fuzz_target!(|data: &[u8]| {
    if let Ok(document) = terminfokit::binary::decode(data) {
        let _ = SourceFormatter::default().format(document.entry());
    }
});

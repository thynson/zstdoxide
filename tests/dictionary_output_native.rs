//! Pinned native destination cases, with explicit dictionary lifetime differences.
#![forbid(unsafe_code)]
use std::alloc::Global;
use zstdoxide::{
    DecodeOptions, Dictionary, Error, decompress_into_with_dictionary,
    decompress_into_with_dictionary_in,
};
#[test]
#[ignore = "requires the external pinned native dictionary destination stage"]
fn pinned_native_dictionary_destinations_and_guarded_errors() {
    let stage = std::path::PathBuf::from(
        std::env::var_os("ZSTDOXIDE_DICTIONARY_NATIVE_STAGE").expect("native stage"),
    );
    let manifest = std::fs::read_to_string(stage.join("rust-cases.tsv")).unwrap();
    let mut count = 0;
    for line in manifest.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 3);
        let name = fields[0];
        let capacity: usize = fields[1].parse().unwrap();
        let expected = fields[2];
        let frame = std::fs::read(stage.join("inputs").join(format!("{name}.zst"))).unwrap();
        let dictionary = Dictionary::from_bytes(
            &std::fs::read(stage.join("inputs").join(format!("{name}.dict"))).unwrap(),
        )
        .unwrap()
        .into_shared();
        let source = std::fs::read(stage.join("inputs").join(format!("{name}.raw"))).unwrap();
        for borrowed in [false, true] {
            let mut output = vec![0xa5; capacity + 64];
            let result = if borrowed {
                decompress_into_with_dictionary_in(
                    &frame,
                    &mut output[32..capacity + 32],
                    &dictionary.view(),
                    DecodeOptions::new(capacity),
                    Global,
                )
            } else {
                decompress_into_with_dictionary(
                    &frame,
                    &mut output[32..capacity + 32],
                    &dictionary,
                    DecodeOptions::new(capacity),
                )
            };
            match expected {
                "ok" => {
                    assert_eq!(result, Ok(source.len()), "{name} / {capacity}");
                    assert_eq!(&output[32..source.len() + 32], source);
                    assert!(output[source.len() + 32..].iter().all(|&b| b == 0xa5));
                }
                "limit" => assert_eq!(result, Err(Error::ResourceLimit), "{name} / {capacity}"),
                "corrupt" => assert_eq!(result, Err(Error::CorruptData), "{name}"),
                "dictionary" => assert_eq!(result, Err(Error::DictionaryMismatch), "{name}"),
                "checksum" => assert_eq!(result, Err(Error::ChecksumMismatch), "{name}"),
                "error" => assert!(result.is_err(), "{name}"),
                _ => panic!("unknown expectation"),
            }
            assert!(output[..32].iter().all(|&b| b == 0xa5));
            assert!(output[capacity + 32..].iter().all(|&b| b == 0xa5));
            count += 1;
        }
    }
    assert!(count > 100);
    eprintln!(
        "{count} dictionary destination calls, both shared and borrowed views, zero skipped cases"
    );
}

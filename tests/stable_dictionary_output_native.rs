//! Pinned native destination cases, with explicit dictionary lifetime differences.
#![forbid(unsafe_code)]
use std::alloc::Global;
use zstdoxide::{DecodeOptions, Dictionary, DictionaryHandle, Error, SliceDecoder, StreamStatus};
fn decode<D: DictionaryHandle>(
    frame: &[u8],
    output: &mut [u8],
    dictionary: D,
    chunk: usize,
) -> Result<usize, Error> {
    let limit = output.len();
    let mut decoder =
        SliceDecoder::with_dictionary_in(output, DecodeOptions::new(limit), dictionary, Global);
    for fragment in frame.chunks(chunk) {
        let mut offset = 0;
        while offset < fragment.len() {
            let p = decoder.process(&fragment[offset..])?;
            assert!(p.consumed > 0 || p.produced > 0);
            assert!(p.consumed <= fragment.len() - offset);
            assert!(matches!(
                p.status,
                StreamStatus::NeedInput | StreamStatus::FrameFinished
            ));
            offset += p.consumed;
        }
    }
    decoder.finish()?;
    assert_eq!(decoder.total_input(), frame.len() as u64);
    Ok(decoder.output()?.len())
}
#[test]
#[ignore = "requires the external pinned native dictionary destination stage"]
fn pinned_native_stable_dictionary_destinations_and_guarded_errors() {
    let stage = std::path::PathBuf::from(
        std::env::var_os("ZSTDOXIDE_STABLE_DICTIONARY_NATIVE_STAGE").expect("native stage"),
    );
    let manifest = std::fs::read_to_string(stage.join("rust-cases.tsv")).unwrap();
    let mut count = 0;
    for line in manifest.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 4);
        let name = fields[0];
        let capacity: usize = fields[1].parse().unwrap();
        let chunk: usize = fields[2].parse().unwrap();
        let expected = fields[3];
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
                decode(
                    &frame,
                    &mut output[32..capacity + 32],
                    dictionary.view(),
                    chunk,
                )
            } else {
                decode(
                    &frame,
                    &mut output[32..capacity + 32],
                    dictionary.clone(),
                    chunk,
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
        "{count} stable dictionary destination calls, both shared and borrowed views, zero skipped cases"
    );
}

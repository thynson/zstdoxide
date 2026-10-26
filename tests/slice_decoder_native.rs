//! External pinned native stable-buffer campaign adapter; normal tests stay offline.
#![forbid(unsafe_code)]

use std::path::PathBuf;
use zstdoxide::{DecodeOptions, Error, SliceDecoder, StreamStatus};

#[test]
#[ignore = "requires the hash-qualified external stable-output campaign"]
fn replay_native_stable_destinations() {
    let stage =
        PathBuf::from(std::env::var_os("ZSTDOXIDE_STABLE_NATIVE_STAGE").expect("campaign path"));
    let cases = std::fs::read_to_string(stage.join("rust-cases.tsv")).unwrap();
    let mut count = 0;
    for line in cases.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 4);
        let name = fields[0];
        assert!(!name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
        let capacity: usize = fields[1].parse().unwrap();
        let chunk: usize = fields[2].parse().unwrap();
        assert!(capacity <= 16 * 1024 * 1024 && chunk > 0);
        let encoded = std::fs::read(stage.join("inputs").join(format!("{name}.zst"))).unwrap();
        let original = std::fs::read(stage.join("inputs").join(format!("{name}.raw"))).unwrap();
        let mut destination = vec![0xa5; capacity + 64];
        let mut decoder = SliceDecoder::new(
            &mut destination[32..32 + capacity],
            DecodeOptions::new(capacity),
        );
        let result = (|| {
            let mut consumed = 0;
            while consumed < encoded.len() {
                let end = consumed + chunk.min(encoded.len() - consumed);
                while consumed < end {
                    let p = decoder.process(&encoded[consumed..end])?;
                    assert!(p.consumed > 0 || p.produced > 0);
                    assert!(p.consumed <= end - consumed);
                    assert!(matches!(
                        p.status,
                        StreamStatus::NeedInput | StreamStatus::FrameFinished
                    ));
                    consumed += p.consumed;
                }
            }
            decoder.finish()
        })();
        match fields[3] {
            "ok" => {
                assert_eq!(result, Ok(()), "{line}");
                assert_eq!(decoder.output().unwrap(), original, "{line}");
                assert_eq!(decoder.total_input(), encoded.len() as u64);
            }
            "small" => assert_eq!(result, Err(Error::ResourceLimit), "{line}"),
            "checksum" => assert_eq!(result, Err(Error::ChecksumMismatch), "{line}"),
            "incomplete" => assert_eq!(result, Err(Error::UnexpectedEof), "{line}"),
            other => panic!("unrecognized case kind {other}"),
        }
        drop(decoder);
        assert_eq!(&destination[..32], &[0xa5; 32]);
        assert_eq!(&destination[32 + capacity..], &[0xa5; 32]);
        if fields[3] == "ok" {
            assert!(
                destination[32 + original.len()..32 + capacity]
                    .iter()
                    .all(|&b| b == 0xa5)
            );
        }
        count += 1;
    }
    assert!(count > 1000, "complete campaign required");
    println!("replayed {count} stable destination cases with zero skips");
}

//! Formatted dictionary construction and independent codec interoperability.
//!
//! The cyclic case preserves tests/fuzzer.c:2665's exact sample layout and
//! success assertion, followed by its nonzero-ID/header and roundtrip checks.
//! Content selection and entropy construction are independent Rust algorithms;
//! these tests do not claim COVER/fastCover, shrink, threading, or level parity.
#![forbid(unsafe_code)]

use zstdoxide::{
    DecodeOptions, Dictionary, DictionaryMode, DictionaryTrainingOptions, Error, FrameHeader,
    RawTrainingOptions, compress, compress_with_dictionary, decompress_with_dictionary,
    finalize_dictionary, train_dictionary,
};

fn options(capacity: usize) -> DictionaryTrainingOptions {
    DictionaryTrainingOptions {
        selection: RawTrainingOptions {
            dictionary_size: capacity,
            max_sample_bytes: 32 * 1024,
            segment_size: 64,
        },
        dictionary_id: None,
    }
}

fn sample(index: usize) -> Vec<u8> {
    format!(
        "event=checkout;service=catalog;region=eu-west;version=stable;user={index:06};\
         product={:04};currency=EUR;status=accepted;delivery=standard;\
         trace=formatted-held-out-{index:06};\n",
        index % 19,
    )
    .into_bytes()
}

fn samples() -> Vec<Vec<u8>> {
    (0..64).map(sample).collect()
}

#[test]
fn upstream_cyclic_training_shape_builds_a_formatted_dictionary_with_a_nonzero_id() {
    let source: Vec<_> = (0..1024 * 1024).map(|index| index as u8).collect();
    let samples: Vec<_> = source.chunks_exact(8192).collect();
    assert_eq!(samples.len(), 128);
    let dictionary = train_dictionary(
        &samples,
        DictionaryTrainingOptions {
            selection: RawTrainingOptions {
                dictionary_size: 16 * 1024,
                max_sample_bytes: source.len(),
                segment_size: 64,
            },
            dictionary_id: None,
        },
    )
    .unwrap()
    .into_shared();
    assert!(dictionary.is_formatted());
    assert!((32768..1 << 31).contains(&dictionary.id()));
    assert!((8..16 * 1024).contains(&dictionary.content().len()));
    assert!(dictionary.as_bytes().len() <= 16 * 1024);
    assert!(dictionary.as_bytes().len() > dictionary.content().len());
    let encoded = compress_with_dictionary(samples[0], &dictionary).unwrap();
    assert_eq!(
        FrameHeader::parse(&encoded).unwrap().0.dictionary_id,
        dictionary.id()
    );
    assert_eq!(
        decompress_with_dictionary(&encoded, &dictionary, DecodeOptions::new(8192)).unwrap(),
        samples[0]
    );
}

#[test]
fn finalization_preserves_short_content_padding_and_long_content_suffixes() {
    for length in 0..=9 {
        let content: Vec<_> = (1..=length).map(|index| index as u8).collect();
        let dictionary = finalize_dictionary(&content, &[], options(256)).unwrap();
        assert_eq!(dictionary.content().len(), length.max(8));
        assert!(dictionary.content().ends_with(&content));
        assert!(
            dictionary.content()[..8_usize.saturating_sub(length)]
                .iter()
                .all(|&byte| byte == 0)
        );
        assert!(dictionary.as_bytes().len() <= 256);
    }
    let content: Vec<_> = (0..4096).map(|index| index as u8).collect();
    let dictionary = finalize_dictionary(&content, &[b"short"], options(256)).unwrap();
    assert_eq!(dictionary.as_bytes().len(), 256);
    assert_eq!(
        dictionary.content(),
        &content[content.len() - dictionary.content().len()..]
    );
    let metadata = dictionary.as_bytes().len() - dictionary.content().len();
    for length in [0, 1, 4, 8, metadata - 1, metadata, metadata + 7] {
        assert_eq!(
            Dictionary::from_bytes_with_mode(
                &dictionary.as_bytes()[..length],
                DictionaryMode::Formatted
            )
            .unwrap_err(),
            Error::InvalidDictionary
        );
    }
}

#[test]
fn identifiers_are_explicit_or_derived_from_original_content_before_padding() {
    let automatic = finalize_dictionary(&[], &[], options(256)).unwrap();
    assert_eq!(automatic.id(), 475_556_249); // Independent XXH64(empty) + public-range mapping.
    for id in [1, 255, 256, 65535, 65536, u32::MAX] {
        let options = DictionaryTrainingOptions {
            dictionary_id: Some(id),
            ..options(256)
        };
        let dictionary = finalize_dictionary(b"custom content", &[], options)
            .unwrap()
            .into_shared();
        assert_eq!(dictionary.id(), id);
        let encoded =
            compress_with_dictionary(b"custom contentcustom content", &dictionary).unwrap();
        assert_eq!(FrameHeader::parse(&encoded).unwrap().0.dictionary_id, id);
        assert_eq!(
            decompress_with_dictionary(&encoded, &dictionary, DecodeOptions::new(28)).unwrap(),
            b"custom contentcustom content"
        );
    }
}

#[test]
fn literal_statistics_include_short_samples_and_obey_proportional_budget_prefixes() {
    let first = finalize_dictionary(b"same content", &[&b"a"[..]; 128], options(256)).unwrap();
    let second = finalize_dictionary(b"same content", &[&b"b"[..]; 128], options(256)).unwrap();
    assert_eq!(first.id(), second.id());
    assert_eq!(first.content(), second.content());
    assert_ne!(first.as_bytes(), second.as_bytes());

    let samples = samples();
    let views: Vec<_> = samples.iter().map(Vec::as_slice).collect();
    let total: usize = views.iter().map(|sample| sample.len()).sum();
    let budget = 1024;
    let prefixes: Vec<_> = views
        .iter()
        .map(|sample| &sample[..sample.len() * budget / total])
        .collect();
    let limited = finalize_dictionary(
        b"custom suffix",
        &views,
        DictionaryTrainingOptions {
            selection: RawTrainingOptions {
                max_sample_bytes: budget,
                ..options(256).selection
            },
            ..options(256)
        },
    )
    .unwrap();
    let explicit = finalize_dictionary(b"custom suffix", &prefixes, options(256)).unwrap();
    assert_eq!(limited.as_bytes(), explicit.as_bytes());
}

#[test]
fn malformed_options_and_insufficient_selection_fail_with_specific_errors() {
    for options in [
        DictionaryTrainingOptions {
            dictionary_id: Some(0),
            ..options(256)
        },
        options(255),
        DictionaryTrainingOptions {
            selection: RawTrainingOptions {
                max_sample_bytes: 255,
                ..options(256).selection
            },
            ..options(256)
        },
        DictionaryTrainingOptions {
            selection: RawTrainingOptions {
                segment_size: 4097,
                ..options(256).selection
            },
            ..options(256)
        },
        DictionaryTrainingOptions {
            selection: RawTrainingOptions {
                dictionary_size: usize::MAX,
                ..options(256).selection
            },
            ..options(256)
        },
    ] {
        assert_eq!(
            train_dictionary(&[], options).unwrap_err(),
            Error::InvalidParameter
        );
        assert_eq!(
            finalize_dictionary(&[], &[], options).unwrap_err(),
            Error::InvalidParameter
        );
    }
    for samples in [
        &[][..],
        &[&b""[..]][..],
        &[&b"short"[..], &b"short"[..]][..],
    ] {
        assert_eq!(
            train_dictionary(samples, options(256)).unwrap_err(),
            Error::TrainingFailed
        );
        assert!(finalize_dictionary(b"content", samples, options(256)).is_ok());
    }
}

#[test]
fn large_caps_do_not_force_large_allocations_and_dictionary_bytes_own_their_sources() {
    let content = b"small content".to_vec();
    let dictionary = finalize_dictionary(
        &content,
        &[b"one"],
        DictionaryTrainingOptions {
            selection: RawTrainingOptions {
                dictionary_size: u32::MAX as usize,
                max_sample_bytes: u32::MAX as usize,
                segment_size: 64,
            },
            dictionary_id: None,
        },
    )
    .unwrap();
    drop(content);
    assert_eq!(dictionary.content(), b"small content");
    assert!(dictionary.as_bytes().len() < 1024);

    let samples = samples();
    let copied = samples.clone();
    let views: Vec<_> = samples.iter().map(Vec::as_slice).collect();
    let copied_views: Vec<_> = copied.iter().map(Vec::as_slice).collect();
    let first = train_dictionary(&views, options(2048)).unwrap();
    let second = train_dictionary(&copied_views, options(2048)).unwrap();
    assert_eq!(first.as_bytes(), second.as_bytes());
    drop(views);
    drop(copied_views);
    drop(samples);
    drop(copied);
    assert_eq!(first.as_bytes(), second.as_bytes());
}

#[test]
fn trained_formatted_content_improves_held_out_messages_and_roundtrips() {
    let samples = samples();
    let views: Vec<_> = samples.iter().map(Vec::as_slice).collect();
    let dictionary = train_dictionary(&views, options(2048))
        .unwrap()
        .into_shared();
    let mut ordinary_total = 0;
    let mut dictionary_total = 0;
    for index in 1000..1032 {
        let source = sample(index);
        ordinary_total += compress(&source).unwrap().len();
        let encoded = compress_with_dictionary(&source, &dictionary).unwrap();
        dictionary_total += encoded.len();
        assert_eq!(
            decompress_with_dictionary(&encoded, &dictionary, DecodeOptions::new(source.len()))
                .unwrap(),
            source
        );
    }
    assert!(
        dictionary_total * 4 < ordinary_total * 3,
        "held-out bytes: formatted={dictionary_total}, ordinary={ordinary_total}"
    );
}

#[test]
#[ignore = "requires official zstd v1.5.7 CLI; set ZSTD_BIN"]
fn official_codec_loads_learned_metadata_and_roundtrips_in_both_directions() {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let executable = std::env::var_os("ZSTD_BIN").unwrap_or_else(|| "zstd".into());
    let version = Command::new(&executable).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("1.5.7"));
    let path = std::env::temp_dir().join(format!(
        "zstdoxide-formatted-training-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    fs::create_dir(&path).unwrap();
    let directory = Directory(path);
    let dictionary_path = directory.0.join("trained.dict");
    let source_path = directory.0.join("source");
    let frame_path = directory.0.join("frame.zst");
    let mut cases = 0;
    let mut verify = |dictionary: &zstdoxide::SharedDictionary, source: &[u8]| {
        fs::write(&dictionary_path, dictionary.as_bytes()).unwrap();
        fs::write(&source_path, source).unwrap();
        let encoded = Command::new(&executable)
            .args(["-q", "-c", "-D"])
            .arg(&dictionary_path)
            .arg(&source_path)
            .output()
            .unwrap();
        assert!(
            encoded.status.success(),
            "C load/compress dictionary ID {}: {}",
            dictionary.id(),
            String::from_utf8_lossy(&encoded.stderr)
        );
        assert_eq!(
            decompress_with_dictionary(
                &encoded.stdout,
                dictionary,
                DecodeOptions::new(source.len())
            )
            .unwrap(),
            source
        );
        let rust_frame = compress_with_dictionary(source, dictionary).unwrap();
        assert_eq!(
            FrameHeader::parse(&rust_frame).unwrap().0.dictionary_id,
            dictionary.id()
        );
        fs::write(&frame_path, rust_frame).unwrap();
        let decoded = Command::new(&executable)
            .args(["-q", "-d", "-c", "-D"])
            .arg(&dictionary_path)
            .arg(&frame_path)
            .output()
            .unwrap();
        assert!(
            decoded.status.success(),
            "C decode dictionary ID {}: {}",
            dictionary.id(),
            String::from_utf8_lossy(&decoded.stderr)
        );
        assert_eq!(decoded.stdout, source);
        cases += 1;
        encoded.stdout.len()
    };

    let samples = samples();
    let views: Vec<_> = samples.iter().map(Vec::as_slice).collect();
    let trained = train_dictionary(&views, options(2048))
        .unwrap()
        .into_shared();
    let mut ordinary_total = 0;
    let mut trained_total = 0;
    for index in 2000..2016 {
        let source = sample(index);
        trained_total += verify(&trained, &source);
        let ordinary = Command::new(&executable)
            .args(["-q", "-c"])
            .arg(&source_path)
            .output()
            .unwrap();
        assert!(ordinary.status.success());
        ordinary_total += ordinary.stdout.len();
    }
    let binary: Vec<_> = (0..1024).map(|index| index as u8).collect();
    let held_out = sample(3000);
    for id in [1, 255, 256, 65535, 65536, u32::MAX] {
        let dictionary = finalize_dictionary(
            trained.content(),
            &views,
            DictionaryTrainingOptions {
                dictionary_id: Some(id),
                ..options(2048)
            },
        )
        .unwrap()
        .into_shared();
        for source in [&[][..], binary.as_slice(), held_out.as_slice()] {
            verify(&dictionary, source);
        }
    }
    let empty = finalize_dictionary(&[], &[], options(256))
        .unwrap()
        .into_shared();
    for source in [&[][..], binary.as_slice()] {
        verify(&empty, source);
    }
    assert_eq!(cases, 36);
    assert!(
        trained_total * 4 < ordinary_total * 3,
        "independent C held-out bytes: formatted={trained_total}, ordinary={ordinary_total}"
    );
    eprintln!(
        "C accepted 8 Rust-built formatted dictionaries; {cases} bidirectional frames; held-out bytes formatted={trained_total}, ordinary={ordinary_total}"
    );
}

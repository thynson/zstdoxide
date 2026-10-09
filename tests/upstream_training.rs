//! Independent quality and resource checks for the bounded raw trainer.
//!
//! These exercise the dictionary construction/held-out roundtrip contract of
//! upstream training tests. COVER/fastCover optimization remains separate.
#![forbid(unsafe_code)]

use zstdoxide::{
    DecodeOptions, RawTrainingOptions, compress, compress_with_dictionary,
    decompress_with_dictionary, train_raw_dictionary,
};

fn sample(index: usize) -> Vec<u8> {
    format!(
        "event=checkout;service=catalog;region=eu-west;version=stable;user={index:06};\
         product={:04};currency=EUR;status=accepted;delivery=standard;\
         trace=independent-sample-{index:06};\n",
        index % 19,
    )
    .into_bytes()
}

fn options() -> RawTrainingOptions {
    RawTrainingOptions {
        dictionary_size: 2048,
        max_sample_bytes: 32 * 1024,
        segment_size: 64,
    }
}

fn training_samples() -> Vec<Vec<u8>> {
    (0..64).map(sample).collect()
}

#[test]
fn trained_content_improves_held_out_messages_and_roundtrips_exactly() {
    let samples = training_samples();
    let slices: Vec<_> = samples.iter().map(Vec::as_slice).collect();
    let dictionary = train_raw_dictionary(&slices, options())
        .unwrap()
        .into_shared();
    assert!(dictionary.content().len() <= options().dictionary_size);
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
        "held-out compression should improve: dictionary={dictionary_total}, ordinary={ordinary_total}"
    );
}

#[test]
fn training_is_independent_of_allocation_addresses_and_releases_sample_borrows() {
    let samples = training_samples();
    let copies = samples.clone();
    let views: Vec<_> = samples.iter().map(Vec::as_slice).collect();
    let copied_views: Vec<_> = copies.iter().map(Vec::as_slice).collect();
    let expected = train_raw_dictionary(&views, options()).unwrap();
    let actual = train_raw_dictionary(&copied_views, options()).unwrap();
    assert_eq!(actual.as_bytes(), expected.as_bytes());
    drop(copied_views);
    drop(copies);
    assert_eq!(actual.as_bytes(), expected.as_bytes());
    assert!(!actual.is_formatted());
    assert_eq!(actual.id(), 0);
}

#[test]
fn sample_budget_is_applied_before_candidate_scoring() {
    let samples = training_samples();
    let views: Vec<_> = samples.iter().map(Vec::as_slice).collect();
    let total: usize = views.iter().map(|sample| sample.len()).sum();
    let budget = 2048;
    let selected: Vec<_> = views
        .iter()
        .map(|sample| &sample[..sample.len() * budget / total])
        .collect();
    let limited = train_raw_dictionary(
        &views,
        RawTrainingOptions {
            max_sample_bytes: budget,
            dictionary_size: 256,
            ..options()
        },
    )
    .unwrap();
    let explicit = train_raw_dictionary(
        &selected,
        RawTrainingOptions {
            dictionary_size: 256,
            ..options()
        },
    )
    .unwrap();
    assert_eq!(limited.as_bytes(), explicit.as_bytes());
}

#[test]
fn binary_samples_and_minimal_dictionary_caps_remain_usable() {
    let bytes: Vec<_> = (0..=255).collect();
    for capacity in [8, 9, 16, 63, 64, 65, 256, 512] {
        let dictionary = train_raw_dictionary(
            &[&bytes, &bytes[13..]],
            RawTrainingOptions {
                dictionary_size: capacity,
                ..options()
            },
        )
        .unwrap()
        .into_shared();
        assert!((8..=capacity).contains(&dictionary.content().len()));
        let encoded = compress_with_dictionary(&bytes, &dictionary).unwrap();
        assert_eq!(
            decompress_with_dictionary(&encoded, &dictionary, DecodeOptions::new(bytes.len()))
                .unwrap(),
            bytes
        );
    }
}

#[test]
#[ignore = "requires official zstd v1.5.7 CLI; set ZSTD_BIN"]
fn official_codec_uses_rust_trained_content_for_independent_held_out_roundtrips() {
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
    let directory = Directory(std::env::temp_dir().join(format!(
        "zstdoxide-training-{}-{}", std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
    )));
    fs::create_dir(&directory.0).unwrap();
    let samples = training_samples();
    let views: Vec<_> = samples.iter().map(Vec::as_slice).collect();
    let dictionary = train_raw_dictionary(&views, options())
        .unwrap()
        .into_shared();
    let dictionary_path = directory.0.join("trained.raw");
    let source_path = directory.0.join("source");
    let frame_path = directory.0.join("frame.zst");
    fs::write(&dictionary_path, dictionary.as_bytes()).unwrap();
    let mut ordinary_total = 0;
    let mut dictionary_total = 0;
    for index in 2000..2016 {
        let source = sample(index);
        fs::write(&source_path, &source).unwrap();
        let encoded = Command::new(&executable)
            .args(["-q", "-c", "-D"])
            .arg(&dictionary_path)
            .arg(&source_path)
            .output()
            .unwrap();
        assert!(
            encoded.status.success(),
            "{}",
            String::from_utf8_lossy(&encoded.stderr)
        );
        dictionary_total += encoded.stdout.len();
        assert_eq!(
            decompress_with_dictionary(
                &encoded.stdout,
                &dictionary,
                DecodeOptions::new(source.len())
            )
            .unwrap(),
            source
        );
        let ordinary = Command::new(&executable)
            .args(["-q", "-c"])
            .arg(&source_path)
            .output()
            .unwrap();
        assert!(ordinary.status.success());
        ordinary_total += ordinary.stdout.len();
        fs::write(
            &frame_path,
            compress_with_dictionary(&source, &dictionary).unwrap(),
        )
        .unwrap();
        let decoded = Command::new(&executable)
            .args(["-q", "-d", "-c", "-D"])
            .arg(&dictionary_path)
            .arg(&frame_path)
            .output()
            .unwrap();
        assert!(
            decoded.status.success(),
            "{}",
            String::from_utf8_lossy(&decoded.stderr)
        );
        assert_eq!(decoded.stdout, source);
    }
    assert!(
        dictionary_total * 4 < ordinary_total * 3,
        "independent held-out compression: dictionary={dictionary_total}, ordinary={ordinary_total}"
    );
    eprintln!(
        "C held-out bytes: ordinary={ordinary_total}, dictionary={dictionary_total}; 16 bidirectional frames"
    );
}

//! Public contracts of independent bounded search policies.
#![forbid(unsafe_code)]

use zstdoxide::{
    CompressionStrategy, DecodeOptions, Dictionary, Encoder, EncoderOptions, EndDirective,
    SharedDictionary, StreamStatus, decompress, decompress_with_dictionary,
};

fn append(
    encoder: &mut Encoder,
    source: &[u8],
    directive: EndDirective,
    output_chunk: usize,
    encoded: &mut Vec<u8>,
) {
    let mut consumed = 0;
    loop {
        let mut destination = vec![0xa5; output_chunk + 2];
        let progress = encoder
            .process(
                &source[consumed..],
                &mut destination[1..=output_chunk],
                directive,
            )
            .unwrap();
        assert_eq!(destination[0], 0xa5);
        assert_eq!(destination[output_chunk + 1], 0xa5);
        consumed += progress.consumed;
        encoded.extend_from_slice(&destination[1..1 + progress.produced]);
        match progress.status {
            StreamStatus::FrameFinished | StreamStatus::Flushed => {
                assert_eq!(consumed, source.len());
                return;
            }
            StreamStatus::NeedInput if directive == EndDirective::Continue => {
                assert_eq!(consumed, source.len());
                return;
            }
            _ => assert!(progress.consumed != 0 || progress.produced != 0),
        }
    }
}

fn frame(encoder: &mut Encoder, source: &[u8], input_chunk: usize, output_chunk: usize) -> Vec<u8> {
    let mut encoded = Vec::new();
    for part in source.chunks(input_chunk) {
        append(
            encoder,
            part,
            EndDirective::Continue,
            output_chunk,
            &mut encoded,
        );
    }
    append(
        encoder,
        &[],
        EndDirective::Finish,
        output_chunk,
        &mut encoded,
    );
    assert_eq!(encoder.total_input(), source.len() as u64);
    assert_eq!(encoder.total_output(), encoded.len() as u64);
    encoded
}

fn options(strategy: CompressionStrategy) -> EncoderOptions {
    EncoderOptions {
        strategy,
        ..EncoderOptions::default()
    }
}

fn older_match() -> (SharedDictionary, Vec<u8>) {
    let source: Vec<u8> = (0..128).collect();
    let mut content = source.clone();
    content.extend_from_slice(&[255; 19]);
    content.extend_from_slice(&source[..4]);
    content.extend_from_slice(&[254; 12]);
    (Dictionary::raw(&content).unwrap().into_shared(), source)
}

#[test]
fn chain_search_recovers_an_older_long_match_hidden_by_a_newer_short_match() {
    let (dictionary, source) = older_match();
    let mut sizes = Vec::new();
    for strategy in [CompressionStrategy::Greedy, CompressionStrategy::Chain] {
        let mut encoder = Encoder::with_dictionary(options(strategy), dictionary.clone()).unwrap();
        let encoded = frame(&mut encoder, &source, 7, 1);
        assert_eq!(
            decompress_with_dictionary(&encoded, &dictionary, DecodeOptions::new(source.len()))
                .unwrap(),
            source
        );
        sizes.push(encoded.len());
    }
    assert!(
        sizes[1] < sizes[0],
        "chain did not improve this older-match witness: {sizes:?}"
    );
}

#[test]
fn chain_preserves_fragmentation_reset_and_persistent_dictionary_contracts() {
    let (dictionary, pattern) = older_match();
    let source = pattern.repeat(2050);
    let mut encoder =
        Encoder::with_dictionary(options(CompressionStrategy::Chain), dictionary.clone()).unwrap();
    let mut expected = None;
    for (input_chunk, output_chunk) in [(4096, 4096), (17, 13), (1, 1)] {
        encoder.reset();
        let encoded = frame(&mut encoder, &source, input_chunk, output_chunk);
        assert_eq!(
            decompress_with_dictionary(&encoded, &dictionary, DecodeOptions::new(source.len()))
                .unwrap(),
            source
        );
        if let Some(expected) = &expected {
            assert_eq!(&encoded, expected);
        } else {
            expected = Some(encoded);
        }
        assert_eq!(encoder.options().strategy, CompressionStrategy::Chain);
    }
}

#[test]
fn chain_resolves_short_prefix_overlap_and_dictionary_window_expiration() {
    for prefix in [&b"ab"[..], &b"xyz"[..], &b"abcdefghijklmnopqrstuvwxyz"[..]] {
        let dictionary = Dictionary::raw(prefix).unwrap().into_shared();
        let mut encoder = Encoder::with_dictionary(
            EncoderOptions {
                window_log: 10,
                ..options(CompressionStrategy::Chain)
            },
            dictionary.clone(),
        )
        .unwrap();
        let blocks = [
            prefix.repeat(7),
            vec![0; 1024],
            prefix.repeat(11),
            prefix.repeat(17),
        ];
        let source = blocks.concat();
        let mut encoded = Vec::new();
        for block in &blocks {
            append(&mut encoder, block, EndDirective::Flush, 3, &mut encoded);
        }
        append(&mut encoder, &[], EndDirective::Finish, 3, &mut encoded);
        assert_eq!(
            decompress_with_dictionary(&encoded, &dictionary, DecodeOptions::new(source.len()))
                .unwrap(),
            source
        );
    }
}

#[test]
fn chain_workspace_is_lazy_bounded_and_retained_across_strategy_switches() {
    let source = b"bounded chain storage".repeat(16_384);
    let mut encoder = Encoder::new();
    assert_eq!(encoder.retained_capacity(), 0);
    let original = frame(&mut encoder, &source, 4096, 4096);
    let greedy_capacity = encoder.retained_capacity();
    assert!(greedy_capacity < 2 * 1024 * 1024);
    encoder
        .reset_with_options(options(CompressionStrategy::Chain))
        .unwrap();
    let encoded = frame(&mut encoder, &source, 4096, 4096);
    assert_eq!(decompress(&encoded, source.len()).unwrap(), source);
    let chain_capacity = encoder.retained_capacity();
    // Fixed heads/nodes are 9 MiB on 64-bit, 7.5 MiB on common 32-bit layouts;
    // allow their bound plus retained greedy, entropy and block workspaces.
    assert!(chain_capacity > greedy_capacity + 7 * 1024 * 1024);
    assert!(chain_capacity < 16 * 1024 * 1024);
    encoder.reset();
    assert_eq!(encoder.retained_capacity(), chain_capacity);
    assert_eq!(frame(&mut encoder, &source, 4096, 4096), encoded);
    assert_eq!(encoder.retained_capacity(), chain_capacity);
    encoder
        .reset_with_options(options(CompressionStrategy::Greedy))
        .unwrap();
    assert_eq!(frame(&mut encoder, &source, 4096, 4096), original);
    assert_eq!(encoder.retained_capacity(), chain_capacity);
}

#[test]
#[ignore = "requires official zstd v1.5.7 CLI; set ZSTD_BIN"]
fn official_codec_decodes_chain_frames_with_and_without_dictionary() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let executable = std::env::var_os("ZSTD_BIN").expect("set ZSTD_BIN");
    let (dictionary, pattern) = older_match();
    let path = std::env::temp_dir().join(format!("zstdoxide-chain-{}.dict", std::process::id()));
    std::fs::write(&path, dictionary.as_bytes()).unwrap();
    for use_dictionary in [false, true] {
        let source = pattern.repeat(2050);
        let mut encoder = if use_dictionary {
            Encoder::with_dictionary(options(CompressionStrategy::Chain), dictionary.clone())
                .unwrap()
        } else {
            Encoder::with_options(options(CompressionStrategy::Chain)).unwrap()
        };
        let encoded = frame(&mut encoder, &source, 19, 7);
        let mut command = Command::new(&executable);
        command.args(["-d", "-q", "-c"]);
        if use_dictionary {
            command.arg("-D").arg(&path);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let writer = std::thread::spawn(move || stdin.write_all(&encoded).unwrap());
        let result = child.wait_with_output().unwrap();
        writer.join().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, source);
    }
    std::fs::remove_file(path).unwrap();
}

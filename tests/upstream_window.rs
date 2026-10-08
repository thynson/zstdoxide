// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Adapted from facebook/zstd v1.5.7 tests/zstreamtest.c:1652 under BSD-3-Clause.
//! Exact shape of the upstream 10,000-sequence near-window offset sweep.

use zstdoxide::{DecodeOptions, Decoder, FrameHeader, StreamStatus};

fn replay(log: u8, output_chunk: usize) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "tests/upstream/window-sequences/window-{log:02}.zst"
    ));
    let encoded = std::fs::read(path).unwrap();
    let window = 1_usize << log;
    let expected_size = window + 10 * 9999;
    let header = FrameHeader::parse(&encoded).unwrap().0;
    assert_eq!(header.window_size, window as u64);
    assert_eq!(header.content_size, Some(expected_size as u64));
    assert!(header.has_checksum);
    let mut decoder = Decoder::new(DecodeOptions {
        max_output_size: expected_size,
        max_window_size: window as u64,
    });
    let mut input_position = 0;
    let mut produced = 0;
    let mut output = vec![0xa5; output_chunk + 8];
    let mut calls = 0;
    loop {
        calls += 1;
        assert!(calls <= expected_size + encoded.len() + 2);
        let end = (input_position + 257).min(encoded.len());
        output.fill(0xa5);
        let progress = decoder
            .process(&encoded[input_position..end], &mut output[..output_chunk])
            .unwrap();
        assert!(progress.consumed <= end - input_position);
        assert!(progress.produced <= output_chunk);
        assert!(output[..progress.produced].iter().all(|&byte| byte == 0));
        assert!(output[progress.produced..].iter().all(|&byte| byte == 0xa5));
        assert!(
            progress.consumed != 0
                || progress.produced != 0
                || progress.status == StreamStatus::FrameFinished
        );
        input_position += progress.consumed;
        produced += progress.produced;
        assert!(produced <= expected_size);
        // Both compressed and decoded staging stay bounded by one format block;
        // entropy tables are small independently of the advertised window.
        assert!(decoder.retained_capacity() <= window + 2 * 131072 + 16384);
        if progress.status == StreamStatus::FrameFinished {
            break;
        }
        assert!(input_position != encoded.len() || progress.status != StreamStatus::NeedInput);
    }
    decoder.finish().unwrap();
    assert_eq!(input_position, encoded.len());
    assert_eq!(produced, expected_size);
    assert_eq!(decoder.total_input(), encoded.len() as u64);
    assert_eq!(decoder.total_output(), expected_size as u64);
}

#[test]
fn near_window_distances_cover_every_small_window_and_tiny_output() {
    for log in 10..=20 {
        replay(log, 4093);
    }
    for log in 10..=12 {
        replay(log, 3);
    }
}

/// Completes the original normal log-26 and big-test log-29 window sweeps.
#[test]
#[ignore = "extended official window sweep; retains up to a 512 MiB history window"]
fn near_window_distances_cover_the_complete_large_window_sweep() {
    for log in 21..=29 {
        replay(log, 65536);
    }
}

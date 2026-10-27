//! Exact original fuzzer histories for size and bound assertions.
#![forbid(unsafe_code)]
use std::{fs, path::PathBuf};
use zstdoxide::{
    FrameKind, decompress, decompressed_size, decompression_bound, frame_compressed_size,
    frame_info, skippable_payload,
};
#[test]
#[ignore = "requires ZSTDOXIDE_FRAME_QUERY_HISTORY from the complete pinned fuzzer capture"]
fn original_single_and_multiframe_size_assertions_with_exact_input_history() {
    let root = PathBuf::from(std::env::var_os("ZSTDOXIDE_FRAME_QUERY_HISTORY").unwrap())
        .join("normal-fixtures");
    for (name, size) in [
        ("single-original", 5 * 1024 * 1024),
        ("multiple-original", 5 * 1024 * 1024 / 2),
        ("unknown-original", 5 * 1024 * 1024),
    ] {
        let frame = fs::read(root.join(format!("{name}.zst"))).unwrap();
        let expected = fs::read(root.join(format!("{name}.raw"))).unwrap();
        assert_eq!(expected.len(), size);
        assert_eq!(decompress(&frame, size).unwrap(), expected);
        if name == "unknown-original" {
            // This useful exact-frame-length bound is an explicit supplement.
            // The original group instead queried full compressedBufferSize,
            // obtained the native error sentinel and compared it numerically.
            assert_eq!(decompressed_size(&frame), Ok(None));
            assert!(decompression_bound(&frame).unwrap() >= size as u64);
            assert_eq!(frame_compressed_size(&frame), Ok(frame.len()));
        } else {
            assert_eq!(decompressed_size(&frame), Ok(Some(size as u64)));
            assert_eq!(decompression_bound(&frame), Ok(size as u64));
            assert!(decompression_bound(&frame[..frame.len() - 1]).is_err());
        }
        let mut remaining = frame.as_slice();
        let mut kinds = Vec::new();
        while !remaining.is_empty() {
            let info = frame_info(remaining).unwrap();
            match info.kind() {
                FrameKind::Standard(header) => {
                    if name == "multiple-original" {
                        assert_eq!(header.content_size, Some((size / 4) as u64));
                    }
                    kinds.push(false);
                }
                FrameKind::Skippable {
                    variant,
                    payload_size,
                } => {
                    assert_eq!(name, "multiple-original");
                    assert!(variant < 15);
                    assert_eq!(payload_size, 129 * 1024);
                    let (found, payload) = skippable_payload(remaining).unwrap();
                    assert_eq!(found, variant);
                    assert_eq!(payload.len(), 129 * 1024);
                    assert!(payload.iter().all(|&b| b == 0));
                    kinds.push(true);
                }
            }
            assert!(info.encoded_size() > 0 && info.encoded_size() <= remaining.len());
            remaining = &remaining[info.encoded_size()..];
        }
        if name == "multiple-original" {
            assert_eq!(kinds, [false, false, false, true, false]);
        } else {
            assert_eq!(kinds, [false]);
        }
    }
}

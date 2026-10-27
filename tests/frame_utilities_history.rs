//! Original writer/reader assertions use the captured preceding fuzzer seed.
#![forbid(unsafe_code)]
use std::{fs, path::PathBuf};
use zstdoxide::{
    decompress, frame_compressed_size, is_skippable_frame, skippable_payload, write_skippable_frame,
};
#[test]
#[ignore = "requires exact original writer history in ZSTDOXIDE_FRAME_UTILITY_HISTORY"]
fn original_skippable_writes_match_exact_history_and_preserve_reader_assertions() {
    let root = PathBuf::from(std::env::var_os("ZSTDOXIDE_FRAME_UTILITY_HISTORY").unwrap())
        .join("normal-fixtures");
    for (name, length) in [("writer-original", 129 * 1024), ("empty-original", 0)] {
        let native = fs::read(root.join(format!("{name}.zst"))).unwrap();
        let payload = fs::read(root.join(format!("{name}.raw"))).unwrap();
        let seed = fs::read_to_string(root.join(format!("{name}.seed")))
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        assert_eq!(payload.len(), length);
        assert!(
            payload
                .iter()
                .enumerate()
                .all(|(i, &b)| b == seed.wrapping_add(i as u32) as u8)
        );
        let mut guarded = vec![0xa5; length + 8 + 32];
        let written = write_skippable_frame(
            &mut guarded[16..16 + length + 8],
            &payload,
            (seed % 15) as u8,
        )
        .unwrap();
        assert_eq!(written, length + 8);
        let frame = &guarded[16..16 + written];
        assert_eq!(frame, native);
        assert!(is_skippable_frame(frame));
        let (variant, received) = skippable_payload(frame).unwrap();
        assert_eq!(written, received.len() + 8);
        assert_eq!(variant, (seed % 15) as u8);
        assert_eq!(received, payload);
        assert_eq!(frame_compressed_size(frame), Ok(written));
        assert!(decompress(frame, 0).unwrap().is_empty());
        assert!(
            guarded[..16]
                .iter()
                .chain(&guarded[16 + written..])
                .all(|&b| b == 0xa5)
        );
    }
}

//! Actual shared-buffer moves and split history on foreign interpreters.
#![forbid(unsafe_code)]
use zstdoxide::{DecodeOptions, Error, decompress_in_place_with_options, decompression_margin};

#[test]
fn overlapping_moves_disjoint_compressed_history_and_guards() {
    let raw: Vec<_> = [0x28, 0xb5, 0x2f, 0xfd, 0x20, 24, 193, 0, 0]
        .into_iter()
        .chain(0..24)
        .collect();
    // With input_start zero, destination [0,24) overlaps source [9,33).
    let mut guarded = vec![0xa5; raw.len() + 16];
    guarded[8..8 + raw.len()].copy_from_slice(&raw);
    let before = guarded.clone();
    assert_eq!(
        decompress_in_place_with_options(&mut guarded[8..8 + raw.len()], 0, DecodeOptions::new(24)),
        Ok(24)
    );
    assert_eq!(&guarded[8..32], &(0..24).collect::<Vec<_>>());
    assert_eq!(&guarded[..8], &before[..8]);
    assert_eq!(&guarded[32..], &before[32..]);

    let cases: &[(&[u8], &[u8])] = &[
        (
            include_bytes!("upstream/decodecorpus/compressed-02.zst"),
            include_bytes!("upstream/decodecorpus/compressed-02.raw"),
        ),
        (
            include_bytes!("upstream/decodecorpus/compressed-05.zst"),
            include_bytes!("upstream/decodecorpus/compressed-05.raw"),
        ),
    ];
    for &(frame, source) in cases {
        let mut combined = frame.to_vec();
        combined.extend_from_slice(frame);
        let capacity =
            (2 * source.len() + decompression_margin(&combined).unwrap()).max(combined.len());
        let start = capacity - combined.len();
        let mut buffer = vec![0xa5; capacity + 16];
        buffer[8 + start..8 + capacity].copy_from_slice(&combined);
        let before = buffer.clone();
        assert_eq!(
            decompress_in_place_with_options(
                &mut buffer[8..8 + capacity],
                start,
                DecodeOptions::new(2 * source.len())
            ),
            Ok(2 * source.len())
        );
        assert_eq!(&buffer[8..8 + source.len()], source);
        assert_eq!(&buffer[8 + source.len()..8 + 2 * source.len()], source);
        assert_eq!(&buffer[..8], &before[..8]);
        assert_eq!(
            &buffer[8 + 2 * source.len()..],
            &before[8 + 2 * source.len()..]
        );
        buffer.copy_from_slice(&before);
        let limit = source.len() - 1;
        assert_eq!(
            decompress_in_place_with_options(
                &mut buffer[8..8 + capacity],
                start,
                DecodeOptions::new(limit)
            ),
            Err(Error::ResourceLimit)
        );
        assert_eq!(&buffer[8 + limit..], &before[8 + limit..]);
    }
    let mut bytes = [0xa5; 8];
    assert_eq!(
        decompress_in_place_with_options(&mut bytes, usize::MAX, DecodeOptions::new(8)),
        Err(Error::ResourceLimit)
    );
    assert_eq!(bytes, [0xa5; 8]);
    assert_eq!(
        decompress_in_place_with_options(&mut [], 0, DecodeOptions::new(0)),
        Err(Error::UnexpectedEof)
    );
}

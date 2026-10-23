//! Small foreign-interpreter cases with independently generated compressed bytes.
#![forbid(unsafe_code)]
use zstdoxide::{DecodeOptions, Error, decompress_into, decompress_into_with_options};

#[test]
fn borrowed_compressed_history_and_limits() {
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
        let mut output = vec![0xa5; source.len() + 16];
        assert_eq!(
            decompress_into(frame, &mut output[8..8 + source.len()]),
            Ok(source.len())
        );
        assert_eq!(&output[8..8 + source.len()], source);
        assert_eq!(&output[..8], &[0xa5; 8]);
        assert_eq!(&output[8 + source.len()..], &[0xa5; 8]);
        output.fill(0xa5);
        let limit = source.len() - 1;
        assert_eq!(
            decompress_into_with_options(frame, &mut output[8..], DecodeOptions::new(limit)),
            Err(Error::ResourceLimit)
        );
        assert!(output[..8].iter().all(|b| *b == 0xa5));
        assert!(output[8 + limit..].iter().all(|b| *b == 0xa5));
        let mut combined = frame.to_vec();
        combined.extend_from_slice(frame);
        let mut output = vec![0xa5; 2 * source.len() + 8];
        assert_eq!(
            decompress_into(&combined, &mut output),
            Ok(2 * source.len())
        );
        assert_eq!(&output[..source.len()], source);
        assert_eq!(&output[source.len()..2 * source.len()], source);
        assert_eq!(&output[2 * source.len()..], &[0xa5; 8]);
    }
}

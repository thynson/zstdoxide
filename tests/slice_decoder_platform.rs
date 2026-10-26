//! Small strict-provenance cases using independently generated compressed frames.
#![forbid(unsafe_code)]
use zstdoxide::{DecodeOptions, Error, SliceDecoder};

fn feed(decoder: &mut SliceDecoder<'_>, input: &[u8]) -> Result<(), Error> {
    let mut offset = 0;
    while offset < input.len() {
        let end = offset + 7.min(input.len() - offset);
        let progress = decoder.process(&input[offset..end])?;
        assert!(progress.consumed > 0);
        offset += progress.consumed;
    }
    decoder.finish()
}

#[test]
fn stable_fragmented_compressed_history_and_reset() {
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
        let mut output = vec![0xa5; source.len() * 2 + 16];
        let mut decoder = SliceDecoder::new(&mut output[8..], DecodeOptions::new(source.len() * 2));
        feed(&mut decoder, &combined).unwrap();
        assert_eq!(&decoder.output().unwrap()[..source.len()], source);
        assert_eq!(&decoder.output().unwrap()[source.len()..], source);
        decoder.reset();
        feed(&mut decoder, frame).unwrap();
        assert_eq!(decoder.output().unwrap(), source);
        drop(decoder);
        assert_eq!(&output[..8], &[0xa5; 8]);
        assert_eq!(&output[8 + 2 * source.len()..], &[0xa5; 8]);
        output.fill(0xa5);
        let limit = source.len() - 1;
        let mut decoder = SliceDecoder::new(&mut output[8..], DecodeOptions::new(limit));
        assert_eq!(feed(&mut decoder, frame), Err(Error::ResourceLimit));
        assert_eq!(decoder.output(), Err(Error::InvalidState));
        drop(decoder);
        assert!(output[..8].iter().all(|&b| b == 0xa5));
        assert!(output[8 + limit..].iter().all(|&b| b == 0xa5));
    }
}

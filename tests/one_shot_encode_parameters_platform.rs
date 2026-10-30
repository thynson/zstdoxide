//! Concrete-platform checks for parameterized dictionary one-shot calls.
extern crate alloc;
use alloc::alloc::Global;
use zstdoxide::{
    CompressionParameterRequest, DecodeOptions, Dictionary, Error, FrameHeader, OneShotEncoder,
    ReferenceStrategy, compress_into_with_parameters, compress_into_with_parameters_and_dictionary,
    compress_with_parameters, compress_with_parameters_and_dictionary, decompress,
    decompress_with_dictionary_in,
};

#[test]
fn parameterized_dictionary_platform_paths_preserve_selection_and_wire_guards() {
    let mut prepared = include_bytes!("upstream/dictionaries/formatted.dict").to_vec();
    prepared[4..8].copy_from_slice(&0x0102_0304_u32.to_le_bytes());
    let text = b"platform pattern platform pattern platform pattern";
    for dictionary in [
        Dictionary::raw(b"platform pattern platform pattern").unwrap(),
        Dictionary::from_bytes(&prepared).unwrap(),
    ] {
        let before = dictionary.as_bytes().to_vec();
        let view = dictionary.view();
        for (level, input) in [(1, &text[..]), (4, &text[..]), (4, &b""[..])] {
            let request = CompressionParameterRequest::with_level(level);
            let expected = compress_with_parameters_and_dictionary(input, &request, &view).unwrap();
            let header = FrameHeader::parse(&expected).unwrap().0;
            assert_eq!(header.dictionary_id, dictionary.id());
            assert_eq!(header.content_size, Some(input.len() as u64));
            assert!(!header.has_checksum);
            let mut output = vec![0xa5; input.len() + 64];
            assert_eq!(
                compress_into_with_parameters_and_dictionary(
                    input,
                    &mut output[2..2 + expected.len()],
                    &request,
                    &view,
                ),
                Ok(expected.len())
            );
            assert_eq!(&output[2..2 + expected.len()], expected);
            assert_eq!((output[0], output[1]), (0xa5, 0xa5));
            assert_eq!(
                (output[2 + expected.len()], output[3 + expected.len()]),
                (0xa5, 0xa5)
            );
            assert_eq!(
                decompress_with_dictionary_in(
                    &expected,
                    &view,
                    DecodeOptions::new(input.len()),
                    Global
                )
                .unwrap(),
                input
            );
            // A short caller destination reports a resource limit and keeps
            // both guard bytes outside the slice unchanged.
            let mut short = vec![0xa5; expected.len() + 1];
            assert_eq!(
                compress_into_with_parameters_and_dictionary(
                    input,
                    &mut short[1..expected.len()],
                    &request,
                    &view,
                ),
                Err(Error::ResourceLimit)
            );
            assert_eq!((short[0], short[expected.len()]), (0xa5, 0xa5));

            let plain = compress_with_parameters(input, &request).unwrap();
            let mut encoder = OneShotEncoder::with_prefix(dictionary.view());
            output.fill(0xa5);
            assert_eq!(
                encoder.compress_into_with_parameters(
                    input,
                    &request,
                    &mut output[2..2 + expected.len()]
                ),
                Ok(expected.len())
            );
            assert_eq!(&output[2..2 + expected.len()], expected);
            assert!(encoder.dictionary().is_none());
            assert!(!encoder.prefix_pending());
            output.fill(0xa5);
            assert_eq!(
                encoder.compress_into_with_parameters(
                    input,
                    &request,
                    &mut output[2..2 + plain.len()]
                ),
                Ok(plain.len())
            );
            assert_eq!(&output[2..2 + plain.len()], plain);
            assert!(
                output[..2]
                    .iter()
                    .chain(&output[2 + plain.len()..])
                    .all(|&b| b == 0xa5)
            );
            encoder.set_prefix(Some(dictionary.view()));
            assert_eq!(
                encoder.compress_into_with_parameters(input, &request, &mut []),
                Err(Error::ResourceLimit)
            );
            assert!(!encoder.prefix_pending());
            assert!(encoder.dictionary().is_none());
            assert_eq!(dictionary.as_bytes(), before);
        }
    }
}

#[test]
fn parameterized_unsupported_profiles_reject_before_slice_writes() {
    let dictionary = Dictionary::raw(&[0x5a; 1024]).unwrap();
    let view = dictionary.view();
    let input = vec![0x5a; 20_000];
    let mut request = CompressionParameterRequest::with_level(11);
    request.set_strategy(Some(ReferenceStrategy::Optimal));
    request.set_window_log(16).unwrap();
    request.set_hash_log(12).unwrap();
    request.set_chain_log(12).unwrap();
    request.set_search_log(6).unwrap();
    request.set_min_match(3).unwrap();
    request.set_target_length(1).unwrap();
    assert_eq!(
        compress_with_parameters_and_dictionary(&input, &request, &view),
        Err(Error::UnsupportedFeature)
    );
    let mut output = [0xa5; 64];
    assert_eq!(
        compress_into_with_parameters_and_dictionary(&input, &mut output, &request, &view),
        Err(Error::UnsupportedFeature)
    );
    assert!(output.iter().all(|&byte| byte == 0xa5));
    // The dictionary-free slice path still produces the same bytes as the
    // allocating parameterized API within this platform scope.
    let plain = compress_with_parameters(&input[..64], &CompressionParameterRequest::with_level(1))
        .unwrap();
    let mut storage = vec![0u8; plain.len()];
    assert_eq!(
        compress_into_with_parameters(
            &input[..64],
            &mut storage,
            &CompressionParameterRequest::with_level(1),
        ),
        Ok(plain.len())
    );
    assert_eq!(storage, plain);
    assert_eq!(decompress(&storage, 64).unwrap(), &input[..64]);
}

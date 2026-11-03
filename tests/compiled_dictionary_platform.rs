//! Concrete-platform checks for compiled dictionary emission.
extern crate alloc;
use alloc::alloc::Global;
use zstdoxide::{
    CompiledDictionary, CompiledOneShotEncoder, CompressionParameterRequest, DecodeOptions,
    Dictionary, Error, ReferenceStrategy, compress_into_with_compiled, compress_with_compiled,
    decompress_with_dictionary_in,
};

#[test]
fn compiled_platform_paths_preserve_guards_and_selection() {
    let content = b"platform pattern platform pattern platform pattern".repeat(64);
    let dictionary = Dictionary::raw(&content).unwrap();
    let text = b"platform pattern platform pattern";
    for level in [1, 4, 9] {
        let mut request = CompressionParameterRequest::with_level(level);
        request
            .set_source_hint(u32::try_from(text.len()).unwrap())
            .unwrap();
        let compiled = CompiledDictionary::new(dictionary.view(), &request, Global).unwrap();
        let frame = compress_with_compiled(text, &request, &compiled).unwrap();
        let mut output = vec![0xa5; frame.len() + 4];
        assert_eq!(
            compress_into_with_compiled(text, &mut output[2..2 + frame.len()], &request, &compiled),
            Ok(frame.len())
        );
        assert_eq!(&output[2..2 + frame.len()], &frame);
        assert_eq!((output[0], output[1]), (0xa5, 0xa5));
        assert_eq!(
            (output[2 + frame.len()], output[3 + frame.len()]),
            (0xa5, 0xa5)
        );
        assert_eq!(
            decompress_with_dictionary_in(
                &frame,
                &compiled.view(),
                DecodeOptions::new(text.len()),
                Global
            )
            .unwrap(),
            text
        );
        let encoder = CompiledOneShotEncoder::new(compiled);
        let mut again = vec![0xa5; frame.len() + 2];
        for _ in 0..2 {
            assert_eq!(
                encoder.compress_into(text, &request, &mut again[1..1 + frame.len()]),
                Ok(frame.len())
            );
            assert_eq!(&again[1..1 + frame.len()], &frame);
            assert_eq!((again[0], again[1 + frame.len()]), (0xa5, 0xa5));
        }
    }
}

#[test]
fn compiled_platform_rejects_mismatch_and_unsupported_before_writes() {
    let dictionary = Dictionary::raw(&[0x5a; 1024]).unwrap();
    let input = &[0x5a; 256][..];
    let request = CompressionParameterRequest::with_level(1);
    let compiled = CompiledDictionary::new(dictionary.view(), &request, Global).unwrap();
    let mut output = [0xa5; 64];
    let mut different = request;
    different.set_window_log(10).unwrap();
    assert_eq!(
        compress_into_with_compiled(input, &mut output, &different, &compiled),
        Err(Error::InvalidParameter)
    );
    assert!(output.iter().all(|&byte| byte == 0xa5));
    let mut optimal = CompressionParameterRequest::with_level(11);
    optimal.set_strategy(Some(ReferenceStrategy::Optimal));
    assert_eq!(
        CompiledDictionary::new(dictionary.view(), &optimal, Global).err(),
        Some(Error::UnsupportedFeature)
    );
    let empty = Dictionary::raw(b"").unwrap();
    assert_eq!(
        CompiledDictionary::new(empty.view(), &request, Global).err(),
        Some(Error::InvalidParameter)
    );
}

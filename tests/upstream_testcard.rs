//! Exact fixtures from official v1.5.7 single-file examples and kernel tests.
//!
//! Testcard data: Carl Woffenden, Numfum GmbH, CC0. Kernel fixture: Meta
//! Platforms, Inc. and affiliates, BSD-3-Clause. See upstream/testcard/NOTICE.

use zstdoxide::{Error, compress, decompress};

const TESTCARD: &[u8] = include_bytes!("upstream/testcard/testcard.dxt1");
const COMPRESSED: &[u8] = include_bytes!("upstream/testcard/testcard.zst");
const KERNEL_EMPTY: &[u8] = include_bytes!("upstream/testcard/kernel-empty.zst");

#[test]
fn decodes_single_file_example_testcard_exactly() {
    // simple.c::main compares the supplied compressed texture with rawDxt1.
    assert_eq!(TESTCARD.len(), 32 * 1024);
    assert_eq!(decompress(COMPRESSED, TESTCARD.len()).unwrap(), TESTCARD);
}

#[test]
fn round_trips_single_file_example_testcard() {
    // roundtrip.c::main uses its maximum compression level. Rust currently
    // has a single strategy, so this ports its data/round-trip contract only.
    let encoded = compress(TESTCARD).unwrap();
    assert_eq!(decompress(&encoded, TESTCARD.len()).unwrap(), TESTCARD);
}

#[test]
fn testcard_rejects_truncation_and_insufficient_output() {
    assert_eq!(
        decompress(COMPRESSED, TESTCARD.len() - 1),
        Err(Error::ResourceLimit)
    );
    for end in (0..16).chain(COMPRESSED.len() - 16..COMPRESSED.len()) {
        assert!(decompress(&COMPRESSED[..end], TESTCARD.len()).is_err());
    }
}

#[test]
fn decodes_kernel_empty_frame_with_zero_output_budget() {
    // contrib/linux-kernel/test/static_test.c::test_decompress_unzstd.
    assert_eq!(KERNEL_EMPTY.len(), 13);
    assert!(decompress(KERNEL_EMPTY, 0).unwrap().is_empty());
}

#[test]
fn kernel_empty_frame_still_validates_checksum() {
    let mut changed = KERNEL_EMPTY.to_vec();
    *changed.last_mut().unwrap() ^= 1;
    assert_eq!(decompress(&changed, 0), Err(Error::ChecksumMismatch));
    for end in 0..KERNEL_EMPTY.len() {
        assert!(decompress(&KERNEL_EMPTY[..end], 0).is_err());
    }
}

//! Bounded deterministic mutation checks, complementary to format fixtures.

use zstdoxide::{DecodeOptions, decompress_with_options};

fn bounded_decode(input: &[u8]) {
    let options = DecodeOptions {
        max_output_size: 512 * 1024,
        max_window_size: 8 * 1024 * 1024,
    };
    // A panic fails the test. Valid mutations are allowed, but never over limit.
    if let Ok(output) = decompress_with_options(input, options) {
        assert!(output.len() <= options.max_output_size);
    }
}

#[test]
fn deterministic_noise_and_structured_header_mutations_never_panic() {
    let mut random = 0xa8d2_e175_c039_6bf1_u64;
    for length in 0..512 {
        let mut input = Vec::with_capacity(length);
        for _ in 0..length {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            input.push(random as u8);
        }
        bounded_decode(&input);
        if length >= 4 {
            input[..4].copy_from_slice(&[0x28, 0xb5, 0x2f, 0xfd]);
            bounded_decode(&input);
        }
    }
    for descriptor in 0..=255_u8 {
        let mut input = [0; 32];
        input[..4].copy_from_slice(&[0x28, 0xb5, 0x2f, 0xfd]);
        input[4] = descriptor;
        bounded_decode(&input);
        input[5..].fill(255);
        bounded_decode(&input);
    }
}

#[test]
fn entropy_mutations_and_truncations_are_bounded() {
    for fixture in [
        &include_bytes!("fixtures/skewed-3.zst")[..],
        &include_bytes!("fixtures/text-19.zst")[..],
        &include_bytes!("fixtures/runs-3.zst")[..],
    ] {
        // Sample the full encoded body, rather than only the frame prefix.
        for offset in (0..fixture.len()).step_by((fixture.len() / 48).max(1)) {
            bounded_decode(&fixture[..offset]);
            for mask in [1, 0x80, 0xff] {
                let mut changed = fixture.to_vec();
                changed[offset] ^= mask;
                bounded_decode(&changed);
            }
        }
    }
}

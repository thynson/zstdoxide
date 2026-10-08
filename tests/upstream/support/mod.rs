// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Derived from facebook/zstd v1.5.7 tests/fuzzer.c and programs/datagen.c.
// Used under the BSD-3-Clause option; see LICENSE.zstd.
//! Safe, deterministic equivalents of upstream's test-data generators.

/// Scalar state corresponding to upstream `FUZ_rand`.
#[derive(Clone, Copy, Debug)]
pub struct FuzzerRng(u32);

impl FuzzerRng {
    pub const fn new(seed: u32) -> Self {
        Self(seed)
    }

    pub const fn state(self) -> u32 {
        self.0
    }

    pub fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(2_654_435_761)
            .wrapping_add(2_246_822_519)
            .rotate_left(13);
        self.0 >> 5
    }

    /// Corresponds to `FUZ_randomLength`/`FUZ_rLogLength`.
    pub fn log_length(&mut self, maximum_log: u32) -> usize {
        assert!((1..usize::BITS).contains(&maximum_log));
        let log = self.next() % maximum_log;
        let base = 1_usize << log;
        base + ((self.next() as usize) & (base - 1))
    }

    /// Preserve upstream's weighting of the five compressibility profiles.
    pub fn noise_index(&mut self) -> usize {
        let value = self.next() & 0x7f;
        if value & 7 != 0 {
            2
        } else {
            let value = value >> 3;
            if value & 7 != 0 {
                [1, 3][(value >> 3) as usize]
            } else {
                [0, 4][(value >> 3) as usize]
            }
        }
    }
}

struct NoiseGenerator {
    state: u32,
    literals: [u8; 8192],
}

impl NoiseGenerator {
    fn new(seed: u32, literal_probability: f64) -> Self {
        let weight = (literal_probability * 256.0 + 0.001) as usize;
        let (first, last, mut character) = if weight == 0 {
            (0, 255, 0_u16)
        } else {
            (b'(' as u16, b'}' as u16, b'0' as u16)
        };
        let mut literals = [b'0'; 8192];
        let mut offset = 0;
        while offset < literals.len() {
            let count = (((literals.len() - offset) * weight) >> 8) + 1;
            let end = (offset + count).min(literals.len());
            literals[offset..end].fill(character as u8);
            offset = end;
            character += 1;
            if character > last {
                character = first;
            }
        }
        Self {
            state: seed,
            literals,
        }
    }

    /// `RDG_rand` uses XOR, unlike `FUZ_rand`'s addition.
    fn random(&mut self) -> u32 {
        self.state = (self.state.wrapping_mul(2_654_435_761) ^ 2_246_822_519).rotate_left(13);
        self.state >> 5
    }

    fn character(&mut self) -> u8 {
        let index = (self.random() & 8191) as usize;
        self.literals[index]
    }

    fn length(&mut self) -> usize {
        if self.random() & 7 != 0 {
            (self.random() & 15) as usize
        } else {
            (self.random() & 511) as usize + 15
        }
    }
}

/// Corresponds to `RDG_genBuffer` with an owned output and checked indices.
///
/// All nonempty inputs preserve upstream's exact byte sequence. An empty input
/// returns an empty allocation, avoiding upstream's unconditional first write.
pub fn noise(
    length: usize,
    match_probability: f64,
    literal_probability: f64,
    seed: u32,
) -> Vec<u8> {
    assert!((0.0..=1.0).contains(&match_probability));
    assert!((0.0..=1.0).contains(&literal_probability));
    let literal_probability = if literal_probability == 0.0 {
        match_probability / 4.5
    } else {
        literal_probability
    };
    let mut generator = NoiseGenerator::new(seed, literal_probability);
    let mut output = vec![0; length];
    if length == 0 {
        return output;
    }
    let mut position = 0;
    if match_probability == 1.0 {
        loop {
            let base = 1 << (16 + (generator.random() & 3) * 2);
            let run = base + (generator.random() as usize & (base - 1));
            if run > length - position {
                return output;
            }
            position += run;
            output[position - 1] = generator.character();
        }
    }
    let threshold = (32768.0 * match_probability) as u32;
    output[0] = generator.character();
    position = 1;
    let mut previous_offset = 1;
    while position < length {
        if generator.random() & 0x7fff < threshold {
            let count = generator.length() + 4;
            let end = (position + count).min(length);
            let repeated = generator.random() & 15 == 2;
            let random_offset = (generator.random() & 0x7fff) as usize + 1;
            let offset = if repeated {
                previous_offset
            } else {
                random_offset.min(position)
            };
            while position < end {
                output[position] = output[position - offset];
                position += 1;
            }
            previous_offset = offset;
        } else {
            let end = (position + generator.length()).min(length);
            while position < end {
                output[position] = generator.character();
                position += 1;
            }
        }
    }
    output
}

/// Upstream's `NB3BYTESSEQ`/`_3BYTESTESTLENGTH` corpus, including its final
/// partial triple; the C test allocated spare bytes past that logical end.
pub fn three_byte_sequences(length: usize) -> Vec<u8> {
    let mut random = FuzzerRng::new(1);
    let mut sequences = [[0; 3]; 512];
    for sequence in &mut sequences {
        for byte in sequence {
            *byte = random.next() as u8;
        }
    }
    let mut output = Vec::with_capacity(length);
    while output.len() < length {
        let sequence = &sequences[(random.next() & 511) as usize];
        let count = (length - output.len()).min(3);
        output.extend_from_slice(&sequence[..count]);
    }
    output
}

pub fn skippable_frame(magic_variant: u8, payload: &[u8]) -> Vec<u8> {
    assert!(magic_variant < 16);
    let length = u32::try_from(payload.len()).expect("skippable payload fits in u32");
    let mut frame = Vec::with_capacity(payload.len() + 8);
    frame.extend_from_slice(&(0x184d_2a50 + u32::from(magic_variant)).to_le_bytes());
    frame.extend_from_slice(&length.to_le_bytes());
    frame.extend_from_slice(payload);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzer_rng_matches_upstream_vectors() {
        let mut random = FuzzerRng::new(0x1234_5678);
        for expected in [
            38_498_172,
            88_772_490,
            87_273_912,
            109_962_871,
            15_594_053,
            50_966_326,
            46_760_884,
            111_544_936,
        ] {
            assert_eq!(random.next(), expected);
        }
    }

    #[test]
    fn noise_matches_upstream_generated_vectors() {
        // FNV-1a digests of exact RDG_genBuffer outputs, independently generated
        // by compiling v1.5.7 programs/datagen.c; see FUZZER.md for provenance.
        let vectors = [
            (1, 0.00, 0xaf63ee4c86020b22),
            (1, 0.05, 0xaf63a04c86018698),
            (1, 0.50, 0xaf63a84c86019430),
            (1, 0.95, 0xaf63af4c8601a015),
            (1, 1.00, 0xaf63bd4c8601b7df),
            (32, 0.00, 0xc24ebe079b99bf16),
            (32, 0.05, 0xc2cd7e3a97c29c14),
            (32, 0.50, 0x9fe6d86d6762043d),
            (32, 0.95, 0x2353955fbc22b925),
            (32, 1.00, 0x0c8210784d8af5a5),
            (65536, 0.00, 0x982f15db54e29d65),
            (65536, 0.05, 0x7078b395de563dbf),
            (65536, 0.50, 0x5e4d550575ff4757),
            (65536, 0.95, 0xfb2a65db406a378a),
            (65536, 1.00, 0xeb05052ea5b62325),
            (262144, 0.00, 0xdc628387deadb3cd),
            (262144, 0.05, 0xa999d8d90e62d551),
            (262144, 0.50, 0x71507e4bbe1209bd),
            (262144, 0.95, 0xab39c94ba44645ac),
            (262144, 1.00, 0x9c735bed0a722325),
        ];
        for (length, probability, expected) in vectors {
            let output = noise(length, probability, 0.0, 0x1234_5678);
            let digest = output.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
            });
            assert_eq!(
                digest, expected,
                "length={length}, probability={probability}"
            );
        }
        assert!(noise(0, 0.5, 0.0, 1).is_empty());
    }
}

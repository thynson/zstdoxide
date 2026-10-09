//! Seed-zero XXH64 for Zstandard frame content checksums.
//!
//! Algorithm and constants: facebook/zstd v1.5.7, `lib/common/xxhash.h`.
//! Copyright (c) Yann Collet - Meta Platforms, Inc. Used under the BSD license;
//! see the upstream attribution and license retained with this project.

const PRIME_1: u64 = 0x9e37_79b1_85eb_ca87;
const PRIME_2: u64 = 0xc2b2_ae3d_27d4_eb4f;
const PRIME_3: u64 = 0x1656_67b1_9e37_79f9;
const PRIME_4: u64 = 0x85eb_ca77_c2b2_ae63;
const PRIME_5: u64 = 0x27d4_eb2f_1656_67c5;

const INITIAL_LANES: [u64; 4] = [
    PRIME_1.wrapping_add(PRIME_2),
    PRIME_2,
    0,
    PRIME_1.wrapping_neg(),
];

/// Allocation-free incremental XXH64 with seed zero.
///
/// Corresponds to `XXH64_state_t`. The state owns its lanes and incomplete
/// stripe; it never retains a reference to an input passed to `update`.
#[derive(Clone, Debug)]
pub(crate) struct XxHash64 {
    lanes: [u64; 4],
    total_len: u64,
    tail: [u8; 32],
    tail_len: usize,
}

impl XxHash64 {
    /// Start a seed-zero stream; corresponds to `XXH64_reset(state, 0)`.
    pub(crate) const fn new() -> Self {
        Self {
            lanes: INITIAL_LANES,
            total_len: 0,
            tail: [0; 32],
            tail_len: 0,
        }
    }

    /// Hash an arbitrary next slice; corresponds to `XXH64_update`.
    ///
    /// Empty slices are no-ops. The byte count wraps modulo 2^64, matching
    /// upstream's unsigned counter. No temporary allocation is performed.
    pub(crate) fn update(&mut self, mut input: &[u8]) {
        self.total_len = self.total_len.wrapping_add(input.len() as u64);
        if self.tail_len != 0 {
            let needed = self.tail.len() - self.tail_len;
            if input.len() < needed {
                let end = self.tail_len + input.len();
                self.tail[self.tail_len..end].copy_from_slice(input);
                self.tail_len = end;
                return;
            }
            self.tail[self.tail_len..].copy_from_slice(&input[..needed]);
            mix_stripe(&mut self.lanes, &self.tail);
            self.tail_len = 0;
            input = &input[needed..];
        }
        let (stripes, remainder) = input.as_chunks::<32>();
        for stripe in stripes {
            mix_stripe(&mut self.lanes, stripe);
        }
        self.tail[..remainder.len()].copy_from_slice(remainder);
        self.tail_len = remainder.len();
    }

    /// Return the current full-width hash without consuming or changing state.
    ///
    /// Corresponds to `XXH64_digest`. More input may follow a digest call.
    pub(crate) fn digest(&self) -> u64 {
        let hash = if self.total_len >= 32 {
            combine_lanes(self.lanes)
        } else {
            // For ordinary short inputs lane 2 is the seed (zero). Using the
            // lane also preserves upstream's modular-length overflow behavior.
            self.lanes[2].wrapping_add(PRIME_5)
        };
        finish(
            hash.wrapping_add(self.total_len),
            &self.tail[..self.tail_len],
        )
    }

    /// Discard the previous stream; corresponds to `XXH64_reset(state, 0)`.
    pub(crate) fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for XxHash64 {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns XXH64 with seed zero; the frame stores its low 32 bits.
///
/// Corresponds to upstream `XXH64` / `XXH64_endian_align` with `seed = 0`.
/// Works on arbitrary, possibly unaligned slices without allocation. Hash
/// arithmetic wraps intentionally, including when release overflow checks run.
pub(crate) fn checksum(data: &[u8]) -> u64 {
    let (stripes, remainder) = data.as_chunks::<32>();
    let hash = if data.len() >= 32 {
        let mut lanes = INITIAL_LANES;
        for stripe in stripes {
            mix_stripe(&mut lanes, stripe);
        }
        combine_lanes(lanes)
    } else {
        PRIME_5
    };
    finish(hash.wrapping_add(data.len() as u64), remainder)
}

/// Mix one complete 32-byte stripe; corresponds to the four `XXH64_round`
/// calls in the stripe loops of `XXH64_update` and `XXH64_endian_align`.
fn mix_stripe(lanes: &mut [u64; 4], stripe: &[u8]) {
    for (lane, word) in lanes.iter_mut().zip(stripe.as_chunks::<8>().0) {
        *lane = round(*lane, read_word(word));
    }
}

/// Combine completed lanes; corresponds to the long-input prefix of
/// `XXH64_digest` and `XXH64_endian_align`.
fn combine_lanes(lanes: [u64; 4]) -> u64 {
    let mut hash = lanes[0]
        .rotate_left(1)
        .wrapping_add(lanes[1].rotate_left(7))
        .wrapping_add(lanes[2].rotate_left(12))
        .wrapping_add(lanes[3].rotate_left(18));
    for lane in lanes {
        hash = merge_round(hash, lane);
    }
    hash
}

/// Finalize a sub-stripe tail; corresponds to `XXH64_finalize`.
fn finish(mut hash: u64, mut tail: &[u8]) -> u64 {
    while tail.len() >= 8 {
        hash ^= round(0, read_word(&tail[..8]));
        hash = hash
            .rotate_left(27)
            .wrapping_mul(PRIME_1)
            .wrapping_add(PRIME_4);
        tail = &tail[8..];
    }
    if tail.len() >= 4 {
        let word = u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]);
        hash ^= u64::from(word).wrapping_mul(PRIME_1);
        hash = hash
            .rotate_left(23)
            .wrapping_mul(PRIME_2)
            .wrapping_add(PRIME_3);
        tail = &tail[4..];
    }
    for &byte in tail {
        hash ^= u64::from(byte).wrapping_mul(PRIME_5);
        hash = hash.rotate_left(11).wrapping_mul(PRIME_1);
    }
    avalanche(hash)
}

/// Reads an eight-byte little-endian chunk; corresponds to `XXH_readLE64`.
fn read_word(bytes: &[u8]) -> u64 {
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

/// Mixes one lane and input word; corresponds to `XXH64_round`.
fn round(lane: u64, word: u64) -> u64 {
    lane.wrapping_add(word.wrapping_mul(PRIME_2))
        .rotate_left(31)
        .wrapping_mul(PRIME_1)
}

/// Combines a completed lane; corresponds to `XXH64_mergeRound`.
fn merge_round(hash: u64, lane: u64) -> u64 {
    (hash ^ round(0, lane))
        .wrapping_mul(PRIME_1)
        .wrapping_add(PRIME_4)
}

/// Finalizes the hash; corresponds to `XXH64_avalanche`.
fn avalanche(mut hash: u64) -> u64 {
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(PRIME_2);
    hash ^= hash >> 29;
    hash = hash.wrapping_mul(PRIME_3);
    hash ^ (hash >> 32)
}

#[cfg(test)]
mod tests {
    use super::{XxHash64, checksum};
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn known_text_vectors() {
        // Standard public text inputs, independently confirmed with the pinned
        // upstream XXH64 implementation and seed zero.
        let vectors: &[(&[u8], u64)] = &[
            (b"", 0xef46_db37_51d8_e999),
            (b"a", 0xd24e_c4f1_a98c_6e5b),
            (b"abc", 0x44bc_2cf5_ad77_0999),
            (b"message digest", 0x066e_d728_fcee_b3be),
            (b"abcdefghijklmnopqrstuvwxyz", 0xcfe1_f278_fa89_835c),
            (
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
                0xaaa4_6907_d304_7814,
            ),
            (
                b"12345678901234567890123456789012345678901234567890123456789012345678901234567890",
                0xe04a_477f_19ee_145d,
            ),
        ];
        for &(input, expected) in vectors {
            assert_eq!(checksum(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn official_vectors_cover_stripes_and_every_tail_path() {
        // Input byte i is (i * 37 + 11) modulo 256. Expected values were
        // generated by XXH64 in the pinned facebook/zstd v1.5.7 source.
        let vectors = [
            (0, 0xef46_db37_51d8_e999),
            (1, 0xf592_c0c7_639c_4cb6),
            (3, 0x22c0_8528_601d_4f27),
            (4, 0xfb1e_5cf2_f1ae_4d95),
            (7, 0x5613_ac51_0496_c04e),
            (8, 0x57cb_2b75_21f3_e21a),
            (15, 0x90a9_714e_b00e_8d29),
            (16, 0xc684_3cef_9972_1174),
            (31, 0xe4a0_e629_e519_a4ae),
            (32, 0xcc6b_8aaa_da79_0b2d),
            (33, 0x35ec_4985_0475_a832),
            (63, 0xbf9f_0ba3_cf95_b28a),
            (64, 0x155c_cce4_bf32_befc),
            (65, 0xda5e_e441_a595_cfb8),
            (95, 0xeb8f_a2ed_c955_0675),
            (96, 0x7c87_4b79_5fa7_ab0c),
            (97, 0x665a_ae9f_f640_96d9),
            (255, 0x178e_a4a7_d231_9abe),
            (256, 0x43c9_2f09_cb3e_28cf),
            (257, 0x5f0f_93c2_b91f_a7db),
            (1024, 0x3c8b_642b_9e1d_662f),
            (131_085, 0x5d67_94f3_cc36_40bb),
            (1_048_576, 0x8993_fec9_be75_c186),
        ];
        let input: Vec<_> = (0..1_048_576usize)
            .map(|index| (index.wrapping_mul(37).wrapping_add(11)) as u8)
            .collect();
        for (length, expected) in vectors {
            assert_eq!(checksum(&input[..length]), expected, "length: {length}");
        }
    }

    #[test]
    fn alignment_and_surrounding_bytes_do_not_affect_hash() {
        let input: Vec<_> = (0..257usize).map(|index| (index * 37 + 11) as u8).collect();
        for offset in 0..16 {
            let mut backing = vec![0xa5; input.len() + 32];
            backing[offset..offset + input.len()].copy_from_slice(&input);
            assert_eq!(
                checksum(&backing[offset..offset + input.len()]),
                0x5f0f_93c2_b91f_a7db,
                "offset: {offset}",
            );
        }
    }

    // Independently generated by seed-zero XXH64 in the pinned C reference.
    // Input byte i is (i * 37 + 11) modulo 256; each element hashes that prefix.
    const PREFIX_DIGESTS: [u64; 513] = [
        0xef46db3751d8e999,
        0xf592c0c7639c4cb6,
        0x78f1ae91e3b05e99,
        0x22c08528601d4f27,
        0xfb1e5cf2f1ae4d95,
        0x9d213eca1f68f273,
        0x0f28996d1449561e,
        0x5613ac510496c04e,
        0x57cb2b7521f3e21a,
        0xc3f1d09775257b66,
        0x920f407f474903ff,
        0x1bef4539443a2d5a,
        0x2f53b00266039e64,
        0xaed0293a7bb13c7a,
        0x393648aa0a97d46f,
        0x90a9714eb00e8d29,
        0xc6843cef99721174,
        0xf72e5e622b272ad6,
        0xade42c5775a43c96,
        0xc3bdab4626258cea,
        0xa62c18328d2d838d,
        0x2f04ff79d78f8827,
        0x1e5b0540defc3911,
        0x5e6a1fe05d7c06f8,
        0xfa2d58b357e1565e,
        0x8adefb8c854b47a5,
        0x2b085f5f9da33210,
        0x87b80516e95cbbbc,
        0xff5c77ef1446f7e5,
        0x8b15b1001a84dbdb,
        0x6077f1ac1f080751,
        0xe4a0e629e519a4ae,
        0xcc6b8aaada790b2d,
        0x35ec49850475a832,
        0xf56a57787fd5fe4e,
        0x59de01e65b64555b,
        0xbebb6d8c24fb3cfa,
        0x5f313a6790502101,
        0x9ffd421ba9cd1bab,
        0x22984e41b53c1210,
        0x3b21ed23eea85c2d,
        0x25c2b9e2f81a6a9d,
        0x126f6f74a01c7118,
        0x03c47edf3964bb63,
        0xa9a8f93ce6f588d3,
        0xfc003d355fa58a61,
        0x2af21f2c679a3b30,
        0x16fdd9ca22942dda,
        0xf6c2a2407a06c116,
        0x1951edf80e0e88a1,
        0x8966cf42dc674cb9,
        0xca011706caf89aa3,
        0xc97263946e120e53,
        0xf540a55131f40746,
        0xab9a21d36f37bce2,
        0x66a3eed6214e51ff,
        0xd44977c91550cf3b,
        0x52f38f5975796217,
        0x9996904218108a75,
        0xd8213a1477e2e240,
        0x2bafa54dfa33d5a4,
        0xcdc4536663934e01,
        0x57b94e18c6b9173f,
        0xbf9f0ba3cf95b28a,
        0x155ccce4bf32befc,
        0xda5ee441a595cfb8,
        0x4b4fc775ab6cf0fb,
        0x5e65106afea9135e,
        0x775582f16a463143,
        0x122f5c97b04ed270,
        0x0d74d36bafb8d9c7,
        0x81e0a52b9c0e1651,
        0x657fc372b173aae2,
        0x228f4f53c6795c5f,
        0xedc247c39e343f66,
        0x63cda54c687966f2,
        0x53eeb7aa7ae1400f,
        0x531f82751546d548,
        0x3327a98236e094b4,
        0x87a81e99c6449abd,
        0x300315c0f7a0f0b0,
        0x5619f4db22b243ea,
        0x65ac6e53a063f323,
        0x55d8dc41d1396942,
        0x60702868c96e3a02,
        0xf430229a7aecf3b7,
        0x1c8d22476fe60724,
        0xfec57d71abc854a6,
        0x628311773e689fb4,
        0x4d6b4b10c1e8b188,
        0x54d428d7dc5ed523,
        0xc5cbef261c0d081a,
        0x2986290ac486b8dc,
        0x48244111593dd776,
        0xc741b3e62620b360,
        0xeb8fa2edc9550675,
        0x7c874b795fa7ab0c,
        0x665aae9ff64096d9,
        0x7d4e4c0f94ce3a1c,
        0xd5fd0ba4e8ed1ac5,
        0x4826e367566ea023,
        0x30383413a6f6646a,
        0x4f7ef82142ceb6ca,
        0xd72237cdfe49bde2,
        0x1cc638a7d3855855,
        0xf407d12ecb4ec4b1,
        0x21cb22c11938dbce,
        0x11fa4659b9d27283,
        0x71d52be0d8d53347,
        0xfda4c06b100b3f0d,
        0x844f09baf0364ef7,
        0xf6b573a6263b393c,
        0x3d24792ce0ee489b,
        0x0187137a76721a5f,
        0x836fc9917991ce7e,
        0x04c1b83b21b8cd40,
        0x50f283cecfc6a0a5,
        0x902d3b9462a43073,
        0x1dee7e408515048b,
        0xafe9820f98347138,
        0xc83079df999b4ad3,
        0xfdbdfcf1e60d7649,
        0xcfc4badc404cc1f6,
        0x611e00d43e9236de,
        0x301f9c177ce89452,
        0x14074547197c6feb,
        0xf8e6f27fffbe1414,
        0xf648961449461a69,
        0x01166e04da0ea4a8,
        0x46f0e2b6282dd4fb,
        0x1b5f3684d1f66923,
        0xa52d21d1e22532ea,
        0x0d23cf9382b84558,
        0xcf817a6467a67275,
        0x5e4c6ed7b88b9ca1,
        0xa9d959e870dd4ae9,
        0x5e869fbf18083ba2,
        0x0c6227ca26909c66,
        0xa8b836168c0bff7a,
        0x5333ade6e927598a,
        0xaff841f16143f5ca,
        0xd96d3651b22020a5,
        0xb8dce62dee474893,
        0xaa39b4de43baa2d9,
        0xf4fe1c3430cdae2c,
        0x7813fe628febed86,
        0x1a9557cfc090a56e,
        0x752aacf458bff3b1,
        0xa901961ead399e04,
        0x3f3fec70c622b996,
        0x3abe85e03211d63c,
        0x7a3a61271af19bfd,
        0x46cb490a641689d8,
        0x7e8ea9f69daaad5e,
        0x6539f184406b5ba5,
        0xc9641a93f9e51f11,
        0x674cbdb6243fec2a,
        0x7f1a2c2e8d4d9a49,
        0x2e0e295fc32ab601,
        0xa1d04002fd7cd939,
        0x600f0d7f184e4f0e,
        0x38612fb5c0c6d315,
        0xa9459323b945787e,
        0x58be45b231593d15,
        0xf2cfcb124f407e96,
        0x3aca49f0de3b9c21,
        0x66da4c9ce672f157,
        0x22f9ab0b80529c87,
        0x5c3e1e77904005b3,
        0x665dbd378f56ba76,
        0x98b5081549676d25,
        0xc4990ee058001fe2,
        0x1658a54be4c466ba,
        0x3880d3769c919af7,
        0x7dd950fa1a45755a,
        0xfc3ae81ece6541b0,
        0x00e6d8e9b810ebfc,
        0x6f3622a48ff52b7a,
        0x646eed8b73bfb1ff,
        0xf6f67df542b34cbf,
        0xc5ec6c9c284654e3,
        0x0144d2adc578b623,
        0x5efc2e29cd1bb1dd,
        0x230576d2cd05e94b,
        0xb1229db286a406e6,
        0xbeca2323c4b94a8a,
        0x6e73800b27a6fa20,
        0xb377fa9e6a293e7a,
        0xc4542688e6ca92d8,
        0x962afc9eda98460c,
        0xfecabbad3405224c,
        0xd725f2d4fe3a1755,
        0xf1bcae4f926ac087,
        0x803462d514b0679c,
        0xa186e87b962cf263,
        0x1eab109b3020978c,
        0x2a53c6281138d094,
        0x54a1479937d66ac7,
        0x1e12cbb5e5b11000,
        0xc2f64bc669817a94,
        0x2f074b6dd9094e34,
        0xf252e1788649a253,
        0xed96fd8257714ba6,
        0xbfbfaedb1bed830e,
        0xd044c04b4d2bb991,
        0x43224ca27d023e62,
        0xe6d6f6a25a5d4883,
        0x18fa66c131e3cfbb,
        0x9c47e533cc8e5a58,
        0xe969dd3f19bd8425,
        0x9bafded19d595afb,
        0x8ecac3f745983847,
        0xa64151de7c7a54e5,
        0xe4fd7d1037ee740d,
        0xffdc3fedc180166a,
        0x4e1d436775cc18bc,
        0xd40c45d76b1eeed8,
        0x085b6dfd859c18c8,
        0x77e6725f9ed656b8,
        0x36baa3b687affc2f,
        0xc7c0b9977603a4c1,
        0xd2a8437fa8ac7a42,
        0xf9ce30e0eafd7013,
        0x52e13dbe42c59854,
        0x5d68b6b4496e6909,
        0x58ed0a3234072c7c,
        0x2f7e503dc8f179d1,
        0xca6972bc85b768cc,
        0x1055781d1d3f647d,
        0x4e7e04fbacdf3ff4,
        0xb374fda63bc15698,
        0x29519c06405694f0,
        0x14ca2beba02d3bf5,
        0x02c2e5541cb5faf9,
        0x9bc912aaa5ea6cad,
        0x776fc1b330d28221,
        0x20127393168463f0,
        0x9b24f1a4f9705013,
        0x0def8cf224950714,
        0xf8c96e4a400b252e,
        0x6a0f66faa2413a03,
        0xf84d8d02776ed350,
        0xa0e6d9df0495e77d,
        0x8f2bb1570c548abc,
        0x617365b283deb73d,
        0x2c89f1c11c215b22,
        0x71bf3c2ed7307a56,
        0x14d05fd086cc81e7,
        0xa6fb8a5d10be3ac8,
        0xb55802d64913ccfe,
        0x6e6b876c3792ca26,
        0x5a394cc14eed8af6,
        0x92935c54ee25ca33,
        0xb53f7110a2fca526,
        0xf62d695f12a36818,
        0x178ea4a7d2319abe,
        0x43c92f09cb3e28cf,
        0x5f0f93c2b91fa7db,
        0x707c43f705053840,
        0xac83d89f977a89fc,
        0x221d7e0ed4f54ab9,
        0x1485aa68e948c396,
        0x28ecc121f7a354e5,
        0x5e844823d7d6cc44,
        0x5b1ade9bb02b0788,
        0x571aa77c1a21ac6a,
        0x8a030f106784e0b2,
        0xa83388a806dd0627,
        0x8c0f3157efac9229,
        0x0b5179b56a544e9c,
        0xd315d7abdf135a8d,
        0x1b955da868745d0a,
        0x51c57287e1970a17,
        0x0e69a44aa36d530d,
        0x9b61fbd2a42d2455,
        0x11f369643a83809a,
        0x46e80acf31f7e417,
        0xa2683740cb4a084d,
        0xa96635063431a197,
        0xf6e73835dcf57537,
        0x506f695e9a9c5f2d,
        0xcde81b9bdab5a599,
        0xfab5d95ec5371abf,
        0x4515d89807e36265,
        0x5b0f62f277f4f9ea,
        0xaaf9d89c91243cad,
        0x881711c586dc589b,
        0x117a145c212453ab,
        0xe19cfaf7292fda86,
        0x40a6f04b1eeb36a4,
        0x3fe6f63297697ed0,
        0x3c471fbe55c26013,
        0xdf1c78cbe383d3ce,
        0xcd30e2c51249f6dd,
        0xff2fcfe4061a909e,
        0xea64a5add6da21f2,
        0x539cb95e1de6315d,
        0x6a5c42fc051273a1,
        0x41c60612cb389ab1,
        0x080cf089b14caf7a,
        0x8923b28a8498a506,
        0x51a95c085d01840a,
        0xc5e9ffb878f78008,
        0xed0d956fe5629a9f,
        0x62d647b694c41229,
        0x1d17b3f9dd771277,
        0x8f72e48bd7930d85,
        0xca9ae1d6c2e32364,
        0x5b6a7754cb5f94c2,
        0x7f890fc330cd3c42,
        0x86d37cce13437f4e,
        0x41b058d46c09cba0,
        0x31878bdbb3bc84c6,
        0x44096ad41b7ba144,
        0xe2057f600f66713c,
        0xc2ebab1436a1dc85,
        0xac777ea885f7c8a2,
        0x4ec4bda6ad93d542,
        0x3ec956a46e8c4216,
        0xbdd3b702bcbfb666,
        0x568c4eef0d8f9234,
        0x4e4cd8b4578e7973,
        0xd98d7601020cf41c,
        0x5fb7f879425ea8ba,
        0xbed2734be64f6fc1,
        0x290aa938dfdb61a7,
        0xd4459b2a7883ff7c,
        0x403b6f2c4513ec34,
        0xc67e286809a3f036,
        0x2273da96cd042d77,
        0x6bb61ccecb26a465,
        0xfe7e54aca716333d,
        0xb96adfd1b1ad20b4,
        0xacda7a868c375c43,
        0x06d12a572a21c8a4,
        0xfbc5e2f3236d4d60,
        0x3d02cc75a2fd9261,
        0xc018d0b231df1834,
        0xd73f3310c6690b95,
        0x4b9a8cc9625cf8e0,
        0xd8a85934042acc3d,
        0x939bb110d171307d,
        0xff97448b3787e35d,
        0x05347eceb78b0c03,
        0xe02857c21086ab33,
        0xf2d69312009b8d24,
        0x9ef7b238059a8b5b,
        0xc419a9dc3a0cdac3,
        0xb8d6a1a33b6ceccb,
        0xc488778a98c21b67,
        0xbe4337792fae3d8c,
        0x4eacbc675410be8a,
        0xe8efd2637c3a3ebb,
        0x7dcff831fb8338e6,
        0x08f79b5966f36840,
        0xbb522ba1c174400a,
        0x3f588a1859e3e4c7,
        0x694a136ab245a5a4,
        0x7562f44202000c45,
        0x18c25d2735ce13cb,
        0x4062fde4d98a2f4b,
        0x767847be7c24a15d,
        0x74493b2be131019f,
        0x65e3aae76978df23,
        0x8fc939661916e65e,
        0x81f0d2fc8ef78657,
        0x9d118d232f405e4c,
        0x5e09b94bf1a3bc16,
        0x80857db1f7c395c8,
        0x5af92432f1416329,
        0x1a2ad13ea8d31857,
        0x00d145ae98254a32,
        0x48b04527236951ed,
        0xf30f0093777602f7,
        0x9351582add7302d3,
        0xf584ac5a7ebb508d,
        0x2c041cccd10fb536,
        0xa8f5d354ce71ea93,
        0xa639dbc256a66633,
        0x764f1121f06d4265,
        0x6ad12c4a24b81303,
        0xefc08b652e1954b2,
        0x7261330f7fc33277,
        0xc16c0b65658c3870,
        0xb3ee2058189fb518,
        0xee9d9c51523bb347,
        0x2cc09dacfc575178,
        0xa31e7598fbc7d6f1,
        0x2914a762b69167d6,
        0x9cc48813d0e123b4,
        0x5b4e468d5104c8a4,
        0x69d2452581d564c7,
        0x5b9eb0bf718b3419,
        0x37771766262df0a1,
        0x9d52e0ba6131419e,
        0x4ba7bf671551ad95,
        0xc0aae71cd2e55582,
        0xadce5f6b9c8db6e8,
        0x05ec7914a083487e,
        0xecf6ada6457a9c70,
        0x0c7418c3eaca7ee6,
        0x41d2c71a728e76c2,
        0x442ad1e445c21f0b,
        0x3e3e5442a1cb22d5,
        0x42457af3f14a320d,
        0x94ca11aca7168465,
        0xde15be693c359ed4,
        0x5f21c3d3c751c468,
        0x2b0063169f27c6de,
        0xf768877b1e60b04b,
        0x21bd82837dcdd2b4,
        0x2fef65f92ca4ad0e,
        0xdcf5523543a886fb,
        0x626400289dd8518d,
        0x4e7142e57056286a,
        0x8f48c5b84e528e3f,
        0xa2774f5e3998dbb3,
        0x652c1e15f3353fb8,
        0x76b12b9174cc5d05,
        0x476a053741a90285,
        0xe8444b275be4c22e,
        0xb565599a6cf72152,
        0x21aad74982a600c9,
        0x6be39165bf783e98,
        0x733d228eca187109,
        0x9ffabfdb27ac0c35,
        0x59496ea64e447c7c,
        0xcc45e99fd7996c82,
        0x7e47ddd6d326e4d1,
        0x6d87b52911f856a0,
        0xa8907625de4df612,
        0x84b65575d4162f77,
        0xea105c2f865383d4,
        0xde8da4ceab756536,
        0x908a9e67453d34ea,
        0x770a9436ce4b6f1f,
        0x743fca477d4e0e9d,
        0x4c8ab48bb4f8b147,
        0x8e6d65de731e84f7,
        0x65b468bbb92d21a3,
        0x606d47c2d9fa5138,
        0x3347bae04fd4fcec,
        0xf4defdc234b561fd,
        0x3cd89b23fed8abfa,
        0x12c8fa3e15073ca4,
        0x6bee5ae5e51424af,
        0x0209012dadc41147,
        0xfc712ddc8eef4577,
        0xa24eab404f833b42,
        0xd9f12f226f300d9d,
        0x566c07b9145569a6,
        0xe4d1159be937d297,
        0x012dc7f2e0ecaa96,
        0x86efb7b671192f36,
        0x9f33319fcd3941c3,
        0xd4aa76327cbcb4c7,
        0x0eb4d39a3f66f78b,
        0x3150fc591d589f85,
        0xca9e210cc04193e4,
        0xfa8de09a02b7ae38,
        0x17ed184343376391,
        0x8fdae9f891b130ae,
        0x4cbfbf1d4cce947f,
        0xdb448908b659a5e8,
        0x3fe238e281fb284e,
        0x68efed40fb930f52,
        0x0f406785732f728e,
        0x98c6bf734b2d0041,
        0x597faca4072b2b96,
        0xc62c2fa5e00163cf,
        0x8f96f580bb886a39,
        0xafce5e7ee17a71b9,
        0x698faaec1cee5d20,
        0x35de85d441819a7c,
        0xd04c873414607cb5,
        0x6db9a667a9f4ade6,
        0xb2f361d85a8c51e6,
        0x33c1b03069017f13,
        0x993b6696d8dbc8bc,
        0xd606f1d7c3633ff2,
        0xa9d810863ef2bbe4,
        0xcb4f87c8fe40866b,
        0x95a7632ca981c62e,
        0x4daa977dac61add8,
        0x39b4b878cb2ca075,
        0xd9a08223230499a4,
        0x1c7c90ce35392c1e,
        0x6d7c3fe62bac83fd,
        0xff5eabac061917e5,
        0xdca48ba8ed0a1291,
        0x1c1c0fe71a7ea181,
        0x446c1b5e36d828a8,
        0x1bedf6bbf5eb02d5,
        0x99c59e78ef80a5ba,
        0xa2111ccd10c58241,
        0x1d888f60142560a4,
        0x5f739d6252bafec7,
        0x60d02eef34c4cda9,
        0xf704d7949cad7b2e,
        0x6bff815bfe7b0345,
        0x1a8416b2899fa5a3,
        0xf6f309bb33c4a433,
        0xa9792cf5929953d6,
        0x1b240d1862ad077f,
        0xb47a3934dbbc1aea,
        0x257f71dc2b741ed4,
        0x934dd874f35479c1,
        0x46456f6c3ebaed3f,
        0x487f48e3e7bc4964,
        0x8959e8760f03bddd,
        0x3cd9a789748f7576,
        0x9f6d68b8b71d72df,
        0x0056740c319f2c83,
    ];

    #[test]
    fn streaming_every_prefix_and_two_way_split_matches_upstream() {
        let input: Vec<_> = (0..512).map(|index| (index * 37 + 11) as u8).collect();
        for (length, &expected) in PREFIX_DIGESTS.iter().enumerate() {
            assert_eq!(
                checksum(&input[..length]),
                expected,
                "one-shot length={length}"
            );
            for split in 0..=length {
                let mut state = XxHash64::new();
                state.update(&[]);
                state.update(&input[..split]);
                assert_eq!(
                    state.digest(),
                    PREFIX_DIGESTS[split],
                    "prefix split={split}"
                );
                state.update(&[]);
                state.update(&input[split..length]);
                assert_eq!(state.digest(), expected, "length={length}, split={split}");
                state.update(&[]);
                assert_eq!(
                    state.digest(),
                    expected,
                    "empty update length={length}, split={split}"
                );
            }
        }
    }

    #[test]
    fn streaming_large_regular_and_irregular_chunks_match_upstream() {
        // C XXH64 and C XXH64_update both independently produce this digest.
        let expected = 0x2203_bab4_7f5a_29b7;
        let mut random = 0x6a09_e667_u32;
        let input: Vec<_> = (0..1_000_003)
            .map(|_| {
                random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (random >> 24) as u8
            })
            .collect();
        assert_eq!(checksum(&input), expected);
        for chunk_size in [1, 7, 31, 32, 33, 127, 1024, 65535] {
            let mut state = XxHash64::new();
            for chunk in input.chunks(chunk_size) {
                state.update(chunk);
            }
            assert_eq!(state.digest(), expected, "chunk size={chunk_size}");
        }
        let sizes = [0, 1, 5, 0, 31, 4096, 17, 65536, 3];
        let mut offset = 0;
        let mut step = 0;
        let mut state = XxHash64::new();
        while offset < input.len() {
            let count = sizes[step % sizes.len()].min(input.len() - offset);
            state.update(&input[offset..offset + count]);
            offset += count;
            step += 1;
        }
        assert_eq!(state.digest(), expected);
    }

    #[test]
    fn digest_is_repeatable_and_reset_discards_dirty_stripes_and_tail() {
        let input: Vec<_> = (0..512).map(|index| (index * 37 + 11) as u8).collect();
        let mut state = XxHash64::default();
        for old_length in [0, 1, 7, 31, 32, 33, 63, 64, 255, 511, 512] {
            state.reset();
            state.update(&input[..old_length]);
            let snapshot = state.clone();
            assert_eq!(state.digest(), PREFIX_DIGESTS[old_length]);
            assert_eq!(state.digest(), snapshot.digest());
            state.update(&input[old_length..]);
            assert_eq!(state.digest(), PREFIX_DIGESTS[512]);
            // The clone retains its earlier prefix when the original advances.
            assert_eq!(snapshot.digest(), PREFIX_DIGESTS[old_length]);
            for new_length in [0, 1, 31, 32, 33, 127] {
                state.reset();
                assert_eq!(state.digest(), PREFIX_DIGESTS[0]);
                state.update(&input[..new_length]);
                assert_eq!(state.digest(), PREFIX_DIGESTS[new_length]);
            }
        }
    }

    #[test]
    fn streaming_unaligned_slices_match_the_independent_oracle() {
        let input: Vec<_> = (0..512).map(|index| (index * 37 + 11) as u8).collect();
        for offset in 0..16 {
            let mut backing = vec![0xa5; input.len() + 32];
            backing[offset..offset + input.len()].copy_from_slice(&input);
            for chunk_size in [1, 7, 31, 32, 33, 63, 64, 65] {
                let mut state = XxHash64::new();
                for chunk in backing[offset..offset + input.len()].chunks(chunk_size) {
                    state.update(chunk);
                }
                assert_eq!(
                    state.digest(),
                    PREFIX_DIGESTS[512],
                    "offset={offset}, chunk={chunk_size}"
                );
            }
        }
    }
}

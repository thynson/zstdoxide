//! Exact incremental source generator for facebook/zstd v1.5.7 `tests/seqgen.c`.
//!
//! The recurrence and source layouts follow revision
//! f8745da6ff1ad1e7bab384bd1f9d742439278e99 (BSD option, LICENSE.zstd).
//! Owned state contains only typed phases, scalar indices/counts, random states,
//! and the existing allocation-free XXH64. A call borrows its destination and
//! never retains it. This test helper needs no independent unsafe backend.

use crate::xxhash::XxHash64;
use crate::{Error, Result};

pub(crate) const INITIAL_SEED: u32 = 0x8765_4321;
pub(crate) const ORIGINAL_SOURCE_BYTES: u64 = 34_639_743;
pub(crate) const ORIGINAL_DIGEST: u64 = 0xd6a9_522c_4b14_b2a5;
pub(crate) const ORIGINAL_FINAL_SEED: u32 = 0x0f90_0e80;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SequenceKind {
    MatchLength,
    LiteralLength,
    Offset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SequenceRequest {
    pub(crate) kind: SequenceKind,
    pub(crate) value: u32,
}

/// `remaining` is C's current-phase pending count, not the number of all bytes
/// left in the request. Zero alone means this request has completed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GenerationProgress {
    pub(crate) length: usize,
    pub(crate) remaining: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Start,
    MatchRun,
    LastGuard,
    Prefix,
    Body,
    Replay,
}

/// An unfinished request retains its kind and value, so callers cannot silently
/// switch C state-machine interpretations between partial output slices.
#[derive(Clone, Debug)]
pub(crate) struct SequenceGenerator {
    active: Option<SequenceRequest>,
    phase: Phase,
    seed: u32,
    saved: u32,
    remaining: usize,
    hash: XxHash64,
}

impl SequenceGenerator {
    pub(crate) const fn new(seed: u32) -> Self {
        Self {
            active: None,
            phase: Phase::Start,
            seed,
            saved: 0,
            remaining: 0,
            hash: XxHash64::new(),
        }
    }

    pub(crate) fn seed(&self) -> u32 {
        self.seed
    }

    pub(crate) fn digest(&self) -> u64 {
        self.hash.digest()
    }

    /// Initialize at most output.len() bytes and preserve every unused byte.
    ///
    /// Corresponds to `SEQ_gen` and its three source kinds. Empty output may
    /// advance zero-length phases, but consumes no random bytes. An unfinished
    /// call must be continued with the same kind/value; violation returns
    /// `InvalidState` without modifying output or generator state. Match length
    /// uses the original unsigned `value + 1` wrap: `u32::MAX` has no run bytes
    /// and emits only the two guards. Digest queries do not finalize the state.
    pub(crate) fn generate(
        &mut self,
        kind: SequenceKind,
        value: u32,
        output: &mut [u8],
    ) -> Result<GenerationProgress> {
        let request = SequenceRequest { kind, value };
        if self.active.is_some_and(|active| active != request) {
            return Err(Error::InvalidState);
        }
        let value_size = usize::try_from(value).map_err(|_| Error::ResourceLimit)?;
        self.active = Some(request);
        let mut position = 0;
        loop {
            match self.phase {
                Phase::Start => {
                    if kind == SequenceKind::MatchLength {
                        if position == output.len() {
                            self.remaining = 1;
                            break;
                        }
                        let guard = random_byte(&mut self.seed);
                        output[position] = guard;
                        loop {
                            self.saved = u32::from(random_byte(&mut self.seed));
                            if self.saved != u32::from(guard) {
                                break;
                            }
                        }
                        position += 1;
                        self.remaining = value.wrapping_add(1) as usize;
                        self.phase = Phase::MatchRun;
                    } else {
                        self.saved = self.seed;
                        self.remaining = value_size.min(128);
                        self.phase = Phase::Prefix;
                    }
                }
                Phase::MatchRun => {
                    let length = self.remaining.min(output.len() - position);
                    output[position..position + length].fill(self.saved as u8);
                    position += length;
                    self.remaining -= length;
                    if self.remaining != 0 {
                        break;
                    }
                    self.phase = Phase::LastGuard;
                }
                Phase::LastGuard => {
                    if position == output.len() {
                        self.remaining = 1;
                        break;
                    }
                    loop {
                        let guard = random_byte(&mut self.seed);
                        if u32::from(guard) != self.saved {
                            output[position] = guard;
                            break;
                        }
                    }
                    position += 1;
                    self.complete();
                    break;
                }
                Phase::Prefix => {
                    while self.remaining != 0 && position < output.len() {
                        output[position] = random_byte(&mut self.seed) | 0x80;
                        position += 1;
                        self.remaining -= 1;
                    }
                    if self.remaining != 0 {
                        break;
                    }
                    self.phase = Phase::Body;
                    self.remaining = value_size - value_size.min(128);
                }
                Phase::Body => {
                    if kind == SequenceKind::Offset {
                        let length = self.remaining.min(output.len() - position);
                        output[position..position + length].fill(0);
                        position += length;
                        self.remaining -= length;
                    } else {
                        while self.remaining != 0 && position < output.len() {
                            output[position] = random_byte(&mut self.seed) & 0x7f;
                            position += 1;
                            self.remaining -= 1;
                        }
                    }
                    if self.remaining != 0 {
                        break;
                    }
                    self.phase = Phase::Replay;
                    self.remaining = value_size.min(128);
                }
                Phase::Replay => {
                    while self.remaining != 0 && position < output.len() {
                        output[position] = random_byte(&mut self.saved) | 0x80;
                        position += 1;
                        self.remaining -= 1;
                    }
                    if self.remaining != 0 {
                        break;
                    }
                    self.complete();
                    break;
                }
            }
        }
        self.hash.update(&output[..position]);
        Ok(GenerationProgress {
            length: position,
            remaining: self.remaining,
        })
    }

    fn complete(&mut self) {
        self.active = None;
        self.phase = Phase::Start;
        self.remaining = 0;
    }

    /// Numeric C state only for independent per-call observations in tests.
    fn phase_number(&self) -> u32 {
        match self.phase {
            Phase::Start => 0,
            Phase::MatchRun | Phase::Prefix => 1,
            Phase::LastGuard | Phase::Body => 2,
            Phase::Replay => 3,
        }
    }
}

fn random_byte(seed: &mut u32) -> u8 {
    *seed = (seed.wrapping_mul(2_654_435_761) ^ 2_246_822_519).rotate_left(13);
    (*seed >> 5) as u8
}

/// Exact `zstreamtest.c:1574-1589` order, including repeated boundary values.
/// The C loop is exclusive at 2^17: its last power is 2^16. Offset-only requests
/// then extend through (2^24)-1. No vector or simultaneous source corpus is owned.
pub(crate) fn original_schedule() -> impl ExactSizeIterator<Item = SequenceRequest> {
    (0..971).map(|index| {
        let (kind, value) = if index < 900 {
            (index % 3, (index / 3) as u32)
        } else if index < 954 {
            let slot = index - 900;
            let power = 1_u32 << (8 + slot / 6);
            ((slot % 6) / 2, power + (slot % 2) as u32 * (power >> 2))
        } else {
            (2, (1_u32 << (8 + index - 954)) - 1)
        };
        SequenceRequest {
            kind: [
                SequenceKind::MatchLength,
                SequenceKind::LiteralLength,
                SequenceKind::Offset,
            ][kind],
            value,
        }
    })
}

#[test]
fn original_schedule_has_exact_order_count_and_total_length() {
    let requests: alloc::vec::Vec<_> = original_schedule().collect();
    assert_eq!(requests.len(), 971);
    assert_eq!(
        requests[899],
        SequenceRequest {
            kind: SequenceKind::Offset,
            value: 299
        }
    );
    assert_eq!(
        requests[900],
        SequenceRequest {
            kind: SequenceKind::MatchLength,
            value: 256
        }
    );
    assert_eq!(requests[901].value, 320);
    assert_eq!(
        requests[953],
        SequenceRequest {
            kind: SequenceKind::Offset,
            value: 81920
        }
    );
    assert_eq!(requests[954].value, 255);
    assert_eq!(requests[970].value, 16_777_215);
    let total: u64 = requests
        .iter()
        .map(|request| {
            u64::from(request.value)
                + if request.kind == SequenceKind::MatchLength {
                    3
                } else {
                    u64::from(request.value.min(128))
                }
        })
        .sum();
    assert_eq!(total, ORIGINAL_SOURCE_BYTES);
}

fn kind_from_number(number: u32) -> SequenceKind {
    match number {
        0 => SequenceKind::MatchLength,
        1 => SequenceKind::LiteralLength,
        2 => SequenceKind::Offset,
        _ => panic!("invalid stored/oracle source kind"),
    }
}

#[test]
fn stored_c_schedule_matches_every_request_hash_seed_and_complete_source_length() {
    let mut generator = SequenceGenerator::new(INITIAL_SEED);
    let mut lines = SCHEDULE_VECTORS.lines();
    let mut total = 0_u64;
    let mut count = 0;
    for request in original_schedule() {
        let fields: alloc::vec::Vec<_> = lines.next().unwrap().split_whitespace().collect();
        assert_eq!(fields.len(), 6);
        assert_eq!(kind_from_number(fields[0].parse().unwrap()), request.kind);
        assert_eq!(fields[1].parse::<u32>().unwrap(), request.value);
        let mut hash = XxHash64::new();
        let mut length = 0_u64;
        loop {
            let mut guarded = [0xa5; 1026];
            let progress = generator
                .generate(request.kind, request.value, &mut guarded[1..1025])
                .unwrap();
            assert_eq!(guarded[0], 0xa5);
            assert!(
                guarded[1 + progress.length..]
                    .iter()
                    .all(|&byte| byte == 0xa5)
            );
            hash.update(&guarded[1..1 + progress.length]);
            length += progress.length as u64;
            if progress.remaining == 0 {
                break;
            }
            assert_eq!(progress.length, 1024);
        }
        assert_eq!(length, fields[2].parse::<u64>().unwrap(), "{request:?}");
        assert_eq!(
            hash.digest(),
            u64::from_str_radix(fields[3], 16).unwrap(),
            "{request:?}"
        );
        assert_eq!(
            generator.digest(),
            u64::from_str_radix(fields[4], 16).unwrap(),
            "{request:?}"
        );
        assert_eq!(
            generator.seed(),
            u32::from_str_radix(fields[5], 16).unwrap(),
            "{request:?}"
        );
        assert_eq!(generator.phase, Phase::Start);
        assert_eq!(generator.active, None);
        count += 1;
        total += length;
    }
    assert_eq!(
        lines.next().unwrap(),
        "TOTAL 971 34639743 d6a9522c4b14b2a5 0f900e80"
    );
    assert!(lines.next().is_none());
    assert_eq!(count, 971);
    assert_eq!(total, ORIGINAL_SOURCE_BYTES);
    assert_eq!(generator.digest(), ORIGINAL_DIGEST);
    assert_eq!(generator.seed(), ORIGINAL_FINAL_SEED);
}

#[test]
fn stored_c_call_vectors_match_every_byte_phase_remaining_count_and_random_state() {
    let mut generator = SequenceGenerator::new(INITIAL_SEED);
    for &(kind, value, capacity, pending, seed, phase, saved, digest, bytes) in CALL_VECTORS {
        let mut guarded = alloc::vec![0xa5; capacity + 2];
        let progress = generator
            .generate(kind_from_number(kind), value, &mut guarded[1..=capacity])
            .unwrap();
        assert_eq!(
            progress,
            GenerationProgress {
                length: bytes.len(),
                remaining: pending
            }
        );
        assert_eq!(&guarded[1..1 + progress.length], bytes);
        assert_eq!(guarded[0], 0xa5);
        assert!(
            guarded[1 + progress.length..]
                .iter()
                .all(|&byte| byte == 0xa5)
        );
        assert_eq!(generator.seed(), seed);
        assert_eq!(generator.saved, saved);
        assert_eq!(generator.phase_number(), phase);
        assert_eq!(generator.digest(), digest);
    }
    assert!(generator.active.is_none());
}

#[test]
fn unfinished_requests_reject_changed_kind_or_value_without_mutating_state_or_output() {
    for kind in [
        SequenceKind::MatchLength,
        SequenceKind::LiteralLength,
        SequenceKind::Offset,
    ] {
        let mut generator = SequenceGenerator::new(INITIAL_SEED);
        let progress = generator.generate(kind, 129, &mut []).unwrap();
        assert!(progress.remaining > 0);
        let snapshot = generator.clone();
        for request in [
            SequenceRequest { kind, value: 130 },
            SequenceRequest {
                kind: if kind == SequenceKind::Offset {
                    SequenceKind::LiteralLength
                } else {
                    SequenceKind::Offset
                },
                value: 129,
            },
        ] {
            let mut output = [0xa5; 128];
            assert_eq!(
                generator.generate(request.kind, request.value, &mut output),
                Err(Error::InvalidState)
            );
            assert_eq!(output, [0xa5; 128]);
            assert_eq!(generator.active, snapshot.active);
            assert_eq!(generator.phase, snapshot.phase);
            assert_eq!(generator.remaining, snapshot.remaining);
            assert_eq!(generator.seed(), snapshot.seed());
            assert_eq!(generator.saved, snapshot.saved);
            assert_eq!(generator.digest(), snapshot.digest());
        }
        let mut expected_generator = snapshot;
        let mut actual = [0; 1024];
        let mut expected = [0; 1024];
        assert_eq!(
            generator.generate(kind, 129, &mut actual).unwrap(),
            expected_generator
                .generate(kind, 129, &mut expected)
                .unwrap()
        );
        assert_eq!(actual, expected);
        assert!(generator.active.is_none());
    }
}

#[test]
fn zero_length_phases_and_wrapping_match_length_preserve_c_continuations() {
    let mut generator = SequenceGenerator::new(INITIAL_SEED);
    for kind in [SequenceKind::LiteralLength, SequenceKind::Offset] {
        assert_eq!(
            generator.generate(kind, 0, &mut []).unwrap(),
            GenerationProgress {
                length: 0,
                remaining: 0
            }
        );
        assert_eq!(generator.seed(), INITIAL_SEED);
        assert_eq!(generator.digest(), XxHash64::new().digest());
        assert!(generator.active.is_none());
    }
    assert_eq!(
        generator
            .generate(SequenceKind::MatchLength, u32::MAX, &mut [])
            .unwrap(),
        GenerationProgress {
            length: 0,
            remaining: 1
        }
    );
    let mut first = [0; 1];
    assert_eq!(
        generator
            .generate(SequenceKind::MatchLength, u32::MAX, &mut first)
            .unwrap(),
        GenerationProgress {
            length: 1,
            remaining: 1
        }
    );
    assert_eq!(generator.phase, Phase::LastGuard);
    let seed = generator.seed();
    assert_eq!(
        generator
            .generate(SequenceKind::MatchLength, u32::MAX, &mut [])
            .unwrap(),
        GenerationProgress {
            length: 0,
            remaining: 1
        }
    );
    assert_eq!(generator.seed(), seed);
    let mut last = [0; 1];
    assert_eq!(
        generator
            .generate(SequenceKind::MatchLength, u32::MAX, &mut last)
            .unwrap(),
        GenerationProgress {
            length: 1,
            remaining: 0
        }
    );
    assert_ne!(u32::from(first[0]), generator.saved);
    assert_ne!(u32::from(last[0]), generator.saved);
    let mut expected = XxHash64::new();
    expected.update(&first);
    expected.update(&last);
    assert_eq!(generator.digest(), expected.digest());
}

#[test]
fn fragmented_layouts_preserve_guard_bytes_alphabets_and_saved_prefix_replay() {
    let capacities = [0, 1, 2, 127, 128, 129, 1023, 1024];
    for kind in [
        SequenceKind::MatchLength,
        SequenceKind::LiteralLength,
        SequenceKind::Offset,
    ] {
        for value in (0..=300).chain([1023, 1024, 4097]) {
            let mut reference = SequenceGenerator::new(INITIAL_SEED);
            let mut expected = alloc::vec![0; value as usize + 129];
            let progress = reference.generate(kind, value, &mut expected).unwrap();
            assert_eq!(progress.remaining, 0);
            expected.truncate(progress.length);
            let mut fragmented = SequenceGenerator::new(INITIAL_SEED);
            let mut actual = alloc::vec::Vec::new();
            let mut step = 0;
            loop {
                let capacity = capacities[step % capacities.len()];
                let mut output = alloc::vec![0xa5; capacity + 2];
                let progress = fragmented
                    .generate(kind, value, &mut output[1..=capacity])
                    .unwrap();
                actual.extend_from_slice(&output[1..1 + progress.length]);
                assert_eq!(output[0], 0xa5);
                assert!(
                    output[1 + progress.length..]
                        .iter()
                        .all(|&byte| byte == 0xa5)
                );
                if progress.remaining == 0 {
                    break;
                }
                step += 1;
                assert!(step < 100);
            }
            assert_eq!(actual, expected);
            assert_eq!(fragmented.seed(), reference.seed());
            assert_eq!(fragmented.digest(), reference.digest());
            if kind == SequenceKind::MatchLength {
                assert_eq!(actual.len(), value as usize + 3);
                assert!(
                    actual[1..actual.len() - 1]
                        .iter()
                        .all(|&byte| byte == actual[1])
                );
                assert_ne!(actual[0], actual[1]);
                assert_ne!(actual[actual.len() - 1], actual[1]);
            } else {
                let prefix = (value as usize).min(128);
                assert_eq!(actual.len(), value as usize + prefix);
                assert!(actual[..prefix].iter().all(|&byte| byte >= 128));
                assert_eq!(&actual[..prefix], &actual[value as usize..]);
                assert!(actual[prefix..value as usize].iter().all(|&byte| {
                    if kind == SequenceKind::Offset {
                        byte == 0
                    } else {
                        byte < 128
                    }
                }));
            }
        }
    }
}

// Captured by the unchanged pinned C seqgen.c with this repository's bounded
// scripts/oracles/sequence_generator.c helper in schedule mode. Each row is:
// kind, value, request bytes, request XXH64, cumulative XXH64, random seed.
// seqgen.c SHA-256: f57190da11bca49821e099be7e516c674974cedab886b95670eabccedcae3a03
// seqgen.h SHA-256: e85dfeeffecce5d77310a6efecc6a49303f2e5781327f9004cbb231c00220a23
// Full oracle text SHA-256: cf01566caefc87d75cc5bc0d301a2676a2cca413d53aa2d6966d3972973e82c7
const SCHEDULE_VECTORS: &str = "0 0 3 a2bd78555db15c09 a2bd78555db15c09 653ddf8a
1 0 0 ef46db3751d8e999 a2bd78555db15c09 653ddf8a
2 0 0 ef46db3751d8e999 a2bd78555db15c09 653ddf8a
0 1 4 9d2566f473545e8d f2c9037db98fdfb4 369d2d95
1 1 2 423cefbde404a84c 6aeabd2de11cb06b 276e497a
2 1 2 d6e35ddd47f55202 fabfb0c67fdf1ee4 57a5bf9b
0 2 5 078cd295cbdb92db c35d5270db84e32f bef5a2de
1 2 4 950294a267f88392 2d10b585ead82816 d59947db
2 2 4 9bc03a35e22136b9 bd04b2750f235257 48aab808
0 3 6 2076ca0ae7830ef6 f7a24b4ca0f641d7 b1e8e3d9
1 3 6 0b050550b1b1461e 466adbb596b838f6 c4c3c6d9
2 3 6 e2a353344638271d be0fb71288eedf8d 25d8e633
0 4 7 0d24574b584c4430 b27470e55f968dac 375bd297
1 4 8 9a58b9582b417008 0a6e8e0770da78c0 2e5f8d07
2 4 8 abf3c0f5ebd6f79d 957a2df5a43699c7 4ed01f7e
0 5 8 d6ac24301b0a06d8 b4fa5d3857739d7e 32a4a1dd
1 5 10 37c4f20f6d77c7d7 123d5d61f3158991 fb121479
2 5 10 ddf58ff365aa2419 1bc4ec274ef03bab d7073680
0 6 9 ac311bf3c46411e3 7fe5bcb2cb2a31b0 dd15e800
1 6 12 326f6911eb823edb 3423060cf8217eb3 ddf16ff8
2 6 12 694285d63c6c8b3e 2f1025d5a56c19b3 079f7f04
0 7 10 5febc68a468a1c18 86811e1c47839cc2 47f9ac3c
1 7 14 8999d5d0194e0500 48e058ac06108925 59a29905
2 7 14 10be70526eb026dc 171facc4f653f554 d27c7056
0 8 11 46b2e231d9b7d768 31b797fb19a14734 ac38807d
1 8 16 ea718fe0b8521149 b739d2d4f92e1926 6d60a1e2
2 8 16 b6c581cc8c055a3a 2d9f2354f80f0fdd cbd6e99c
0 9 12 48647306f5b24d33 3c58b0df6a9ffbd8 1c48736f
1 9 18 8b1f4bac5f121d34 3a954e5ad59165e8 3d3308f0
2 9 18 99ee0d9bb8d699da 313ff7ebf08f4aa6 da644e4b
0 10 13 b6e0028bed1bdfaf e2ea85f5fe1e75cd 500082e6
1 10 20 e214509718e21ffe 27550e1bcb4c15c0 63586fe1
2 10 20 4a9606ac7eee4dbb 35898e9a63da35bc 3614250a
0 11 14 8e44cc2971405adb 2d21b56256b66b80 80e08e44
1 11 22 044ca7934e3b552f 99a9afbd4b5aba93 612fc64f
2 11 22 d319780492afc719 0fe52dd5eb330851 fd90b38a
0 12 15 30b587866568fe40 333e7084e9edad83 820feb80
1 12 24 96df08caa31208f9 bfb7f82956a9e957 c6be2900
2 12 24 7c93963b2ebbdc72 acdd29d7bc362c3e f13be2c2
0 13 16 9bf7a2e8ded113dc 205dbf4efc549bf0 abb6fcd0
1 13 26 90d7d1204ab06f9c af59335026695eec 6e3f896a
2 13 26 954d17c66d06e3da be71de331eb3d0ae 16158498
0 14 17 07a51b25784aea6c 63a3062bd3e4c532 6edd51e5
1 14 28 aae4eb82cf9feb38 efcc1b4c474f6f74 72661e33
2 14 28 5f4b71ef7e3d268a 87c7338c2d88fe5f 87d733b9
0 15 18 687ed422af11344d 77058b3f02d02fb8 b21bca56
1 15 30 866a4db30991045f f1b70c5c29e02d59 149fcfed
2 15 30 875a211b15bcad16 de53f9aa6bbb0a4a 2e2ce640
0 16 19 e3581ab2d6bac993 d90b54cbb4e3a97f 20299d61
1 16 32 3ee43c3351752a5c b63590a87869a959 52574eb1
2 16 32 56577731e7b68864 6cdd8e673487128a 2b63fbac
0 17 20 92ecae9723692e9a b6577bfa04c0683c 0976c38c
1 17 34 0bd852f979771dc4 9c62d6c4ed165671 f9338bf3
2 17 34 9fd6f4d2e3aa1e17 b353ca889d0212ce f06e9285
0 18 21 33a4f565b25cd336 ba083c3460b2448f 7fae4870
1 18 36 91a7993c8a5199a3 3a3036813c7a2f94 e531db43
2 18 36 bc7e6e2f8cc3b9f6 e6cb134e4e76e968 2a235fa8
0 19 22 0a94e7909c42adb7 05f524563a5d6b32 55c7a5cf
1 19 38 f4163a814e72aa8a 7fdc99acc1b6cb2a aa0823c0
2 19 38 144890ed0d5a9b32 822b088291e831a9 0c542e3e
0 20 23 41d7b32336b0f53b c231106437f6e692 73a94b75
1 20 40 21138da7ae70527f 6bdfb59a9f8a8340 6742b497
2 20 40 596ce20e591d58d4 1d6a798fb64c3853 e851fc1a
0 21 24 15ad586af1e496cb 41b59fb4de1288dc f74fed57
1 21 42 9f76cbcfea94241f bd4a9c01f2b72e09 02373bad
2 21 42 e71a5a0ac8384fe4 97184706d6af5747 52900f96
0 22 25 874040d4a73b5695 7bddd7abf515fa41 ecf6b55c
1 22 44 114e2e14d853da86 e17845e7272860ac 12d22812
2 22 44 bc1a874c653f3f53 ea24ec9196afb527 c5c166fc
0 23 26 e585b7978873d43a 4b9f8814ea0e94fe bcbaf7cf
1 23 46 71b2559e988f095a 294f8553b906e00a 73d7c817
2 23 46 0adb4db7f370b925 5ed683365d3e8888 031a8b35
0 24 27 e782901591569898 26f3755c7b0d6909 3e955275
1 24 48 bc847470ef615a0d a22b749946cd7068 f14e1bad
2 24 48 84521506e06208cd e5c2143e0b4aa0c6 a311c619
0 25 28 a633b6a040504f17 85edd5902e5a377a 232a7450
1 25 50 3866b660d5841b9a b87d5d57e4baafa3 57f11694
2 25 50 fe568f1d83e17664 e99bb4e5548ff33d 87e25bc8
0 26 29 b921728121ca33d6 26684278e3fd9a4f 44bb8aa5
1 26 52 134e9af867a8bb7a c0f91652e1d915f7 3433f5e9
2 26 52 1f46c9163ca2ec9b 0afa03b4b68ae24a f75af449
0 27 30 828a1b5350594f9c 69e71087b1c71f6e 4bfbc5dc
1 27 54 cb5fadcf679f0725 b607f5b4566440a5 7d2ea8dc
2 27 54 6584ea84ac123055 ac0e64ebcc57a821 779bd0d9
0 28 31 85e5d70b6657fd7d 86183b16b37ecc01 caeca761
1 28 56 754b14a88f31ccf6 bf85c3d71f4823ce 12fd443b
2 28 56 09f440bcdd716519 05561d9ba2f1fbe9 8709c830
0 29 32 82779c8d95421546 a6c6703d63ceb33d 0c83063d
1 29 58 ab90dabe8511b09e 21856ac76a543ff4 8a845726
2 29 58 4cf661073e433b36 4a7c9453e01e9cfc 2f4aef53
0 30 33 261ff5c4861b297c 5fb40f082249fe0d 3dcc4f53
1 30 60 abe97bb6bb5e663a 7306793c1dc15a62 eaf7776d
2 30 60 7ee4457d71e3e7d8 bd0ccd8f6ce5f312 00b6270d
0 31 34 662ee4fa4d6c1867 774835f0ecdb91d7 64dd7e46
1 31 62 b5fbcca51b392908 3db59ba85ef10289 0eee2fa8
2 31 62 ffc5a1cd5847f323 b900d9d1fdf8a12e e16168c9
0 32 35 3ce9aca015d009d4 6cab80b057643306 d13bd8e6
1 32 64 0761a937a5f858b8 ff6d58889042fd0d c2c72d32
2 32 64 e20e335f53cfa7e1 6b9a992384ec1aad 2d449a00
0 33 36 0b6d3766c5c5fb6e 912e7fc4a26649e2 bf108148
1 33 66 ccc9ff67155a2e6d 9da3b636e6097762 fee770fd
2 33 66 cfd925418c821642 f9132cf4636dbe95 59a1ccc6
0 34 37 75dd1146205c38ad d940d433d52a9132 49a4e63a
1 34 68 48c73552fc021352 ac7e8a3ea8564e8e efb2c1e8
2 34 68 037661874d3433fe f99d593e4d1322d2 c480ab7b
0 35 38 09628485a4bbd584 8258fc6097c17cad 8285328c
1 35 70 0b63adb929d7fe69 eca2f720e91cb695 d3f387bb
2 35 70 bbf72f4369406704 690ee0a1599e7ef9 f18ba4a0
0 36 39 469706275119c1a2 62ef505c4aac6f36 defe64a9
1 36 72 4b4fa3253be8defa d4901f1b13e8dad2 2c10882d
2 36 72 c92f735fd4cd2e1f 430e83067a50d8f4 e0f1abc0
0 37 40 bf592577ab197d25 a11834b1274cc432 3c1d54f4
1 37 74 22bc27d6e9b3c300 cf41a431ae7c7591 17698339
2 37 74 8f2cf9a623ec0299 10ad0d37e8830073 55dbe448
0 38 41 d0f0424856085f04 0f7e79b84fe7dc61 c04b3041
1 38 76 373de876e5fb525a 61ae67e1dd31666d eacbab94
2 38 76 7fbaf10701862d53 cd606ed6ef192c18 435f89dc
0 39 42 c54dfa5bd07b74d0 0c221d3e0bafaca6 ab714640
1 39 78 479ca804c2661c76 091150d2151f4ff1 a18ea033
2 39 78 13fef7d4e8de7e09 3a209e21ba73906b 098b55c7
0 40 43 2d43347a3d4df882 4166d3f0fc9beada 769aa394
1 40 80 3704627ccb7c9a34 a974fce061c29ee4 fc2dbdb2
2 40 80 22f1ea5917f91e80 c334c0b10b3c5585 e979a847
0 41 44 12eab1b157057069 13a5223e71537dc7 310e5d65
1 41 82 a4b7286b0aff891a 40a142ba4b0c7bf5 0cbcbf9d
2 41 82 b414d2c7a9967ed7 293ba4c593f1f72a aa8cdeeb
0 42 45 a12030bef5357ec9 eda0e79ddcec8849 1b663beb
1 42 84 14d6187e2b1e848b 02d9ed36ec3b4cce e02821f9
2 42 84 6fe70fe394c13e1d 300359db861c0b2c eeb8f7d6
0 43 46 0a6e37fb703782b2 4748c4585830f725 6100dc62
1 43 86 b26627051e77970e a985db338d59ffe3 a43e6649
2 43 86 19438a3655f56734 509bfa311a93bd68 1f93dfa9
0 44 47 6050703c8b9772dd 95e797394288e38e dfe9daa9
1 44 88 6566d85c1c0c0330 2150a8dce207cc13 8f65c3c2
2 44 88 5b32dd0cd3567827 4c2d528b67f7944d 06cca152
0 45 48 5d40f3e7bed435ae 20b52aad97904ae4 f6eb185c
1 45 90 2c728ddc1f380039 6e213349f50fd1c7 ee1cf0cd
2 45 90 d361f1e8ebcf1b9d efb07a014b206703 f1162025
0 46 49 199335d596505767 5272c1356651e4ce 85d1866d
1 46 92 2c885106541bf5c1 bb1b3ff4bb05aefb d7a19d1f
2 46 92 584df87cd39eaf03 60373dd13226acda 4cbeaac7
0 47 50 e034a37063abf927 7b3f555f922e5dee d98469d0
1 47 94 144766b773180c27 9c08f74b9757dd68 98d3c0de
2 47 94 a5160821cb149899 a86bed73ae1c0089 ecd12264
0 48 51 380034ccfa060be9 57e08e87e23a8095 6dedbf15
1 48 96 bcce47291541ca7d c9e2dae3e7683eb5 aed54f4f
2 48 96 4742c28f3d0bc66e 3fb15066096ffc44 6b277bf6
0 49 52 b6aec097dc6666a7 da0d7afd68c8f768 1ec25149
1 49 98 0d9ff209c4aff32d 2401e2b81316c933 95dc9f7d
2 49 98 46d1cdbace69fef5 98df6920a5111f16 fb219e17
0 50 53 fd974db758693c02 8f8b830ee153ca41 3cc6de1c
1 50 100 888b6e1f1d3ead8d 49ab45ff124bed0c 177fa5e0
2 50 100 b3b7af3f095601ed 7f9bf67b2480dd9b d305143a
0 51 54 2c36e41fe26b36fb 503009622d1981cb a7bbc958
1 51 102 d20df375e70c0fe3 eefa77446679d1cd 7d46df5b
2 51 102 45b59113628ee99b 48ab25869bfca616 02bdea43
0 52 55 456d22e8ffc4d4a4 98d017c71c622f28 5410fa7d
1 52 104 720fa36382d91518 4357ec0347f1fa95 b054d880
2 52 104 b45561f08e232bcf d91b3cd9313aeab9 303b6215
0 53 56 6512b6c93b31289d 3ce47845c94b507f d8f81756
1 53 106 713f66ee956449c9 dafd6638ee6da7af bbbc0e7d
2 53 106 34f1f151598a505c 25e9114f5bf57432 d5bf6e59
0 54 57 7536edb2f966bc17 d1d9023e7d81aad6 66a2ac4a
1 54 108 fc4ffd0631e5ccce 59d4538f3dc588fb 4aac5cd0
2 54 108 049874fee30e2248 e7764aeb94a0cff9 f91d9d67
0 55 58 4ca1680eb085d397 ab41384972513fa4 88a15cc9
1 55 110 16d5dd973f0f056e 90aa1e8d3bc32e85 a25d6d70
2 55 110 248bf8d016d0022e c5acf447543ec80f 7acf078e
0 56 59 245f6a0df71b2ea4 d4049ed2b3331af9 ee6b7dd0
1 56 112 5ee2724b808c1a25 70808cb439a538de a7240b05
2 56 112 b1a4055805860ba9 54a49c58095fcdfb d8654bbe
0 57 60 c0f5fdd8ebedba80 7dee4a56bfd29301 4840ccac
1 57 114 e297cc7b81d21586 43eff4b9a9cd7dd8 34f8a6f2
2 57 114 510799cef80e0e3b e106243c4ad088fa fbc5b1c1
0 58 61 3eea44726cd8347b d5c121fd0099e0e1 7f6a940f
1 58 116 0716cd7d638c82c0 404c75b710247cae df9bc257
2 58 116 62b2b9c6c0e3bf1a f759d57c723ca9bb db7a7980
0 59 62 6307ee6f3404f8ca 874b5e26d7d43377 0e349f71
1 59 118 06dca2a185649aae 3fbb11f0a83c2c1e 2b689dc3
2 59 118 9f87279f94e78ffb ee58bac747dd5740 a68ce16f
0 60 63 a5e287786d50d03f 32e7da32aa5688a8 13e3019c
1 60 120 e48607bed0ffa51e 23eca42a6ef8068a 0ce07652
2 60 120 4ab40f6c7035f53c 59ac7010e394c37f 1ab1193c
0 61 64 2940f781a1495488 c37841d789efefe8 4ee2641d
1 61 122 c8b286089e9ee7fa b3e6a981a390318b 27f981f6
2 61 122 d59763d8323e43ed fe36a48ff582caf7 32a9f607
0 62 65 468f3276a4941c45 fec79a875061823a 9d637ee3
1 62 124 210cbc5fafaa27a5 c5656b56e4e9a83f b51e7238
2 62 124 39c420894d12c775 7d457e4163b41517 032e09c8
0 63 66 ee772c0b6dbd9528 287a150150d6d190 0b114cec
1 63 126 2578de26e226597a fcb94bfcc32c58cd b88edb8e
2 63 126 683f6bf10afe02f3 c21558e1d9430854 917f8b50
0 64 67 75c211be91d79330 10b815607f215db1 bfa754d3
1 64 128 c8d517237ea5ab00 b27bcd395d7122ae 7f78529d
2 64 128 10906ad0798e103f ead1c7a0625694c3 773d3f19
0 65 68 f6178e457c134232 5732de407a9647d6 1f9b154d
1 65 130 c6dc6bc6f18aa51e 2b6bdddc7d4f65a1 52b4806b
2 65 130 36f94989fe095f31 27148908266be508 454c34aa
0 66 69 9a8b1397615f145d 01733e3fb53c72f8 2aa99918
1 66 132 d7361514309d1baf 75066330689592c1 21581ee7
2 66 132 3d9c7f408570899a 957534a5bdfa9284 0f1a0147
0 67 70 4f29cc985bb4a9e1 c49a87fea6b87b71 61c779ef
1 67 134 eca3d62945d56233 22703dc9916e5c4a 0417a578
2 67 134 4a1a8dd0c8211ae4 d12a8cae9570055a 462efa0f
0 68 71 3bcb588d3e769d64 409410d947e6d5f5 b721c7f4
1 68 136 abfb0664c3356247 c59749b14c71aa45 71f69f3d
2 68 136 6061ca4d1205bb2f f3d4f29d9e50165b 016595b7
0 69 72 526d66b48bb7a82a e354711f7078ef8a b8dfdb17
1 69 138 038be908e27be144 56e9f4b8f09c9c8f 59a67b74
2 69 138 c51b031f52dfe21e 3e2cd660e49d6a6d 0c87e870
0 70 73 87734f03396ab744 d109816edac35695 41834761
1 70 140 ccf29bd147e7bd93 75e5e04d4439a7f4 ee8e112b
2 70 140 32212215d55108c1 827e4bd6f4fef92f b2b64a1e
0 71 74 f9bfdba6b6cc3a3d 67e017112a777164 169e70d9
1 71 142 826b999d3e5c3039 71a860ce2fa41516 3937b03e
2 71 142 c8977d28a85e559e bfb7baf5a20d17c0 0e87dbc3
0 72 75 9951d6316c3a8202 23dc064f650462a0 acf1edcb
1 72 144 d2c3cac944dadc19 c1aa3f8186378899 42bb6752
2 72 144 4b8c2ff7fb4c4594 b48f9f7ff1afc427 6936321b
0 73 76 86103e6c6e58c01e edfba12047cd4466 a34e64ee
1 73 146 02443a0899f58494 92b51c8dc6241fea ed09bf41
2 73 146 9196dc62e068dc58 1d1767c97af7e581 79d3d815
0 74 77 df846f71402aa5c1 6941b22a2b6ee9ca eab6a9c7
1 74 148 4aff13119e30c1a2 7dc976f0fedfff32 f4e2fc26
2 74 148 1fbe3e951026f495 a12f8ffc3b21696b 085f5675
0 75 78 da5c3c9eb9f9e216 19a0e53d2c915d49 61c8efb5
1 75 150 98ee5dcaf518ec09 6d564bc930585980 bb8c6681
2 75 150 6f3c816a124cb99b e13221dcef612b28 8c38ecab
0 76 79 623af54f58095ff9 ff22d18b9a1048e2 b12d5fa5
1 76 152 7b00d810e778749f 1bdb757a6f4d9652 fbec9d00
2 76 152 d75245e80ad22d00 badf9bf4e63b0a35 70cdbb74
0 77 80 9b1dc5b868323f07 e8ab5a2d99072ae0 ca9dd64a
1 77 154 750ddb2993d2996d 46b599a0a4c906eb 5f5b4216
2 77 154 609872954d55dfda 135157ee94cff472 7924d403
0 78 81 3d780bf1a25ea88f 2e098f2538ced4a0 9d2f94ba
1 78 156 996c8fd04cff4512 75aefe2b17ce91b8 46806e38
2 78 156 2a8b0aa24509766d dc3059d596a7782a cc458466
0 79 82 93795ba04bc4fcb2 aaa4a5eea3c75964 b2976412
1 79 158 b142d6263faa3139 a15807cacbaefbf4 7f6accf3
2 79 158 171b5b20771d4524 f576ba353e1bbca1 5f07953e
0 80 83 4a66d98d2b72f317 6f902f40a9c1ea02 f6a2058f
1 80 160 5997064c7b8c11a5 f54a927251812fff 2e3c5b46
2 80 160 15b20b701130243d 128089f3d3fdfa49 2d80e785
0 81 84 90c8f60785ea8851 5981c8005fc5ff70 7f5a2765
1 81 162 6936572f3bdb7976 83f943aba836ece5 b735a216
2 81 162 95384ed593d0a756 aaf83d8425a6aebf c05c3420
0 82 85 d8c68b8af0f6b6e7 f10fe78c4a8cb0d9 69ec0e89
1 82 164 5d5c7bd50af627eb ca59052ca1606d02 30605c79
2 82 164 9eac59a6a670c99f 0893f31b3780d669 2d2ca07c
0 83 86 92700fdf33bce60b 7e45936b7afe60ca 7311a300
1 83 166 b266f6808ee5ac3c d8dafc5c552fb048 9beb9ea9
2 83 166 48d09e4c5f74190b 2ce05b04245dd1a3 6587164f
0 84 87 f22b5fa9e0139650 bf6a4523e0a328e5 f2ae5403
1 84 168 90fd06ea5fa6ee65 4d0964b2bdef40b7 ac518062
2 84 168 5b9eeb56952da214 d122900cbc85b4a2 79e84ea3
0 85 88 869e72acc86dcab3 a1969f0ce205bba2 6eb1df78
1 85 170 c84c875d70a7ce6f 01a2921e4e7acd43 f6d52f50
2 85 170 c05a406d290f54b5 b652de3323882d10 9debbe04
0 86 89 1fc0d3037eea26e6 965299d4bfc2f907 bf08c8db
1 86 172 5d30655fc86f8715 9e9996ed693c1aa7 b18974ff
2 86 172 ba442b654ec60e98 5db9b44d0f7339de 4eecbb40
0 87 90 c9d7214e0b437428 382f13b3767c9bc9 6746d2c8
1 87 174 2d8aa079bbaed701 5a372fdf14e04b0a 60e15811
2 87 174 ca9cd9aa43ae5608 38ebf3f954370904 37abd2f6
0 88 91 b8cc5c732857ce73 6e696deee8e208fb e3d9305f
1 88 176 3b2f5f0628588dd8 7cd5e8c456a28957 20115b57
2 88 176 e480561aa97aa80e db6d144c82887fe5 ef2dc64a
0 89 92 5ec6f0d225659c89 df8a9536a30802b3 a1c322b4
1 89 178 45b5ab8510dd15f7 d25330fb319510f3 a82c59dc
2 89 178 31ad7ef4e1d5a2bf a3eb122a25aebdd2 183f9c1e
0 90 93 2a77a62213df20f8 5423fc7656fb69ea 0195ec1c
1 90 180 639de62906ea6c9d 1571e52ec69468a2 52ad439d
2 90 180 2b62fcfb3266b498 dc7fc6864ff7d161 8a0dc749
0 91 94 af77ad88ac7b2e86 774e4f3bcbddcd21 2da6c265
1 91 182 10ba10a1a8f5c55d 06650324275db7e0 49dd3983
2 91 182 26877881919263db 27901178c88c98f9 ac83008c
0 92 95 cdd8b2c169b1efff f5dd32cf8b9dabd7 8d2b512a
1 92 184 fba7555b66488f1c 1465f2fa80b544cf 7e2e3f67
2 92 184 a64dc0587832f2be ddeac2860cf33100 eecc3315
0 93 96 578b742d9522779f dcac834aa3f1fb26 258298cb
1 93 186 3862e16cf6bddf2a 5485b13b7ca14e9f 041f7048
2 93 186 ef77863c750b78e4 f1f0e3dea9448ab3 68f7dd30
0 94 97 3e705e3a4194a742 6eec88c836f1d6ea 537d4592
1 94 188 4cce00b4bfd6707d fa8163c7949decba 84eeab85
2 94 188 e729ced636629284 f110a47f4fb0d6fa 4071a73a
0 95 98 f8fdc923d0ec000e faa409cd92f553fa 5964e89b
1 95 190 4d25e5ba1c36eb35 fd9afeb76a4980a9 76bc7b20
2 95 190 d318b7307eec8b89 17207a8c7430f77b 3b3d1c29
0 96 99 6732ad2589e00a35 ab46e33a99c5f54b 79aa255e
1 96 192 d735eff0c71a0a81 c0fec0405dc4ba7f 179f6559
2 96 192 58c3d6797fda8bbe f75ec85920720b74 c98dec86
0 97 100 144872f51b763054 46a80cfb8cd91c8b b70ff324
1 97 194 82cbdf7fbc0cfb5d 231a74268f4a28b0 9a9bf32f
2 97 194 3f9f7270919722c1 3e8884990dd7ee8f f08617fe
0 98 101 cd73855bced81607 4c63f194666d4368 29839efe
1 98 196 592faa3167c9f681 01da0856dd5f794e eb121746
2 98 196 bc327fe0c96c5bee 35eaf8e10aab9a4c f7497e2d
0 99 102 fa36a652c6e24e84 b7a60e13fb40165f 4a550a1a
1 99 198 653990f882d25162 d50b2d0dd08c9c28 d1987644
2 99 198 a04ab96e87080f1b 493a19bbd563cd75 ae24263d
0 100 103 8b3868590cdbfc46 c05349131704cbd3 ca9ca006
1 100 200 01b4f721f464d099 191929284bd5292e a037a088
2 100 200 c4afd1c3a42a40ed 483afaef5c949fee fd4f8933
0 101 104 46a44172c97fec37 f298f47c55f4eb6c f11fa5a7
1 101 202 a013fdfd71c938de d1fcee1134a6b123 75d7ce64
2 101 202 dc25287a6771165d a6af002593d9f698 28b429b7
0 102 105 534bbe9194977b67 9edefdb261a0e201 9d0becb2
1 102 204 613a9668fbc900bd 879113b55dbed908 8320a84f
2 102 204 33b5534ced416be6 be1a21c8381d4323 d173688d
0 103 106 663ce8348f4cdce9 29517d1e5ca0a324 3fdec71d
1 103 206 ec899aa1cf6a81c4 c6018bd8ef116c51 6a1262f9
2 103 206 dde888221253846d ab78999c4715ee65 87d31879
0 104 107 d2f68e8155b97716 1dad6ce630ece609 4bf2604e
1 104 208 bef25699b22e196d 469d8fcd1d78a744 f31c647e
2 104 208 aafd9156aac43a0a 678c7818d7e16947 9f23dedc
0 105 108 887925b8252bfa0b 0e236382ae6c97e0 3e4f8f51
1 105 210 c982f80f2e577a6f bc2c49899eb6c7bd f51b9b1c
2 105 210 86a5e23348d19e5f f845fd569cbc37a1 c58a4198
0 106 109 eaf49b9208a2432b 45dff83dd27a116d 903c50b0
1 106 212 bf793316c2edba15 82b8796842916ba9 29bf340a
2 106 212 452e6c74bacfc611 8990398830e1e6d0 3915693f
0 107 110 f91cf065e370fc65 ed548a6e02972a22 b7e417cb
1 107 214 6c4c332b5238f976 315be91c32c042df f004a286
2 107 214 c6f278af2f0b9211 d33796552a72d7d3 e16a42c7
0 108 111 b2a452bb5375efdc b8b4216054173062 a5608244
1 108 216 1fbcd8862a38512e 360a62ad2f105f6b 7e977f18
2 108 216 6631d2049ee25133 6f74b6f057086997 82e6d54b
0 109 112 66f682e532ca0fa7 09eb16edc55c39c8 b2503852
1 109 218 95c9c303f457e61f 0079b16a8b05386c 40e1a10d
2 109 218 f0fc104e8844b7be 0f41a306343e3586 afb46915
0 110 113 de775db900e18ec4 5d943795570b07ec 55ce1a7e
1 110 220 733f8518199f3abb 8db698a831a00b62 0ab5c0ce
2 110 220 23dd0396d308f9ba 16e3b957e056fc09 961c1a74
0 111 114 6ae1e9cdcf49c2a8 9df8d175d030d62f 0ff8ac8c
1 111 222 9e8d6f6da6e2e8c2 5e4e54b769544d8b 8716a09f
2 111 222 b24a59f5f02895e8 35d84a0a79373873 deecec29
0 112 115 9fb04a254e256e6a 3d06bfe1328b4e1f 569ec1bd
1 112 224 a1ca2be3b63a3a12 e977cd36afe9f1e1 e5caa325
2 112 224 d6f712f0e1552311 aca4162e5eda7e00 793cdef7
0 113 116 5d494fd1c21d10e6 9a1e05454a530472 4c4c0e4a
1 113 226 ecfe726195a76f37 4e9d374d276adb2c 1647c59a
2 113 226 1893a3f8527a7322 d5b627a78af908d3 3da41897
0 114 117 89771249f9449ddd 93aa8ce9738eff47 29863d8c
1 114 228 cf9bcb6135845c87 ac5e86cb1ba04ffc 4e647a30
2 114 228 bbdcf3c9eb379402 bf5bf9c205fc44b6 596b3d76
0 115 118 4b1366e2a24ba253 4051f8dd93ef07f4 67fb437a
1 115 230 d4a7908109597efb f18ddc8b9d8b2cca f5bb7d79
2 115 230 05864f40338bf75b 1a5eb8dfa263edea 17dc1666
0 116 119 11d6f2fb94773dee 3f868af7410e4432 66246f89
1 116 232 05cfa24b84465633 b6acb0101c0067fe c924d1e4
2 116 232 e1792379b5bfee77 5e4ac4fd47feeceb 1549ad9f
0 117 120 922b31af6299cc56 c0a0664b235568c6 de4edf9c
1 117 234 cf63b1edacf8e29b a96a34549203891d 2030cf1d
2 117 234 f01bd119d2dfaf9c 65a1bc4ee19f54b2 b6decf48
0 118 121 77b1c9f815b75e32 5903c97d8dc170c6 b5fb635a
1 118 236 98ca3b4c9216a608 0e096cb8e593eb48 558627cd
2 118 236 06b0a982a9db0f1c 228110fdae02dd82 f93f4cb1
0 119 122 bc2cfcd6e26a1d3d c5c79c905f456731 dccbc7a7
1 119 238 151cc9e79effd6a6 5230912d6ea22e1e f972d51f
2 119 238 cb39f8a53cdb5f4f ae676f898a698076 7ca36e50
0 120 123 9f197b6d7e18acd2 b0338bf57a0d7a7d c7659593
1 120 240 e8262afc789ac1c0 e0dc14e4e99eae42 6ff7b0ca
2 120 240 0947b64823658e67 5369d6fcac2b55e6 d72f9b66
0 121 124 323102ce6323f5f9 98641ba86d66f33d 9888e39b
1 121 242 2c1651be197e0e46 b8a248c0ae0a428d 2a37696f
2 121 242 8865947ee70bb326 7e7d8797f05de974 e6a5f3d0
0 122 125 d48ebd87304bd259 3d41a189b9d306d4 76988e44
1 122 244 c405a09dfc37a492 98114b3a090bf75d 33afbd0a
2 122 244 d2842b69b26c8020 0d58a7413d95585d 6bc1aa55
0 123 126 83d091b6e024501f 03fa4da3031027ea bc2280fd
1 123 246 fde9197991fcb921 c022b91b434e9f30 cfd38cc0
2 123 246 ae1ffc4f0be9d498 894b1c0df4075a3c 88abc506
0 124 127 9e0f29fe1068de99 7268d307d010624d a3b3fcf7
1 124 248 a1ff8631950d94ab fb4d8cb9d6fa37ac d2b819ef
2 124 248 9de6e81cfede001b e51e97b815b68054 e518a670
0 125 128 ec6ee21c30829c66 f7cb1424e73aca13 c633e69e
1 125 250 b38897e8946bf543 2a322bb5768620ca e1245c43
2 125 250 165ec71402fba9c3 2c6817200140b36f 50dea435
0 126 129 f0518e28d60ebc1f 1dd97f40f8955599 43260238
1 126 252 58e72d810e50cba5 e563d588579004b2 ca3a0fa2
2 126 252 d9c8a7cd3a9d9670 c77d92412597173e 6c4adecb
0 127 130 ea6385cc889f226a fb8d96baebfc2fca 43e1b881
1 127 254 300b712d2d27ba85 9da5738fec8e1e90 e6a5da6a
2 127 254 c66cb2f6d6b3f46b 008251c5af933d2f 36fad396
0 128 131 9b405a52e3a70bc8 f3ecf3d16d3d1814 0a4851e4
1 128 256 9df5ff12e6aced66 356bc74cb8d9415a f7b72c3f
2 128 256 af8c98d43178628b 4278153c30956240 26d50837
0 129 132 b7075e0e92ba2019 cd1fca29a0daaf38 d4127182
1 129 257 2ce240e9350f467c f87bb31b0d31e10b 1243f792
2 129 257 57e16e8064b4beb6 ea9b4496f0e0e6eb f55b34bb
0 130 133 791f158fb1c366a3 e281608dcccfd5e7 cb4bfe8d
1 130 258 4a39d279d86486ea 4482084413e1cad4 6b2b78db
2 130 258 959335c87339f346 44659c29feb65195 e04810dd
0 131 134 043dd7ccc5546a7a 3cf0332299d288ba 87d3daca
1 131 259 25432dc258c5d0d5 ae1c01783d8a5ccc 3b3a2f7e
2 131 259 2f7563e4510f80c1 5bd684d6d447e6ba 8548ffb3
0 132 135 10720cf14b69bc95 e6ad0db22384c174 ba6314dc
1 132 260 5f7fac124cad1e35 23e2585046052075 94932edf
2 132 260 6afc0996a357622f 5526d4e9e45cbd53 5c423922
0 133 136 215b9e8943baf246 c2633eb407e61c5b 42ba3505
1 133 261 dfe50307c5f4bf60 c28d39eec8ee0469 de03d4ee
2 133 261 cd81bfe8436104ab 165d3da0ec28d80b 72416ce7
0 134 137 871901fe456b72b9 fcaa1cc91c330bbc 52a4c407
1 134 262 950a8a39109b7a0f e4d86c998d5404e9 e8bda6b7
2 134 262 d924f5eddb3da188 7f4ae61188a517f8 418d2d78
0 135 138 ae654d638157d30b f54f91fa31c7f42f 9be55881
1 135 263 1e1f9d3a66c71a7f e0166a937b6a833b c5644682
2 135 263 674b65c93845a50c 4cb56397005bea53 da708e9d
0 136 139 a9d8b82e18966246 a673172cbc84db27 69de2ff0
1 136 264 a2e5de9fc2349cb1 6d86d56c755a9ffc 99117311
2 136 264 7d3a50149fceff01 6cb1c0805bdbb4ea f554d83d
0 137 140 6ed6b3e84a6dbf70 503967276ac96ff2 458c5c7d
1 137 265 cd9bf70363925f54 d68022c4c55cbc06 05fe8fee
2 137 265 45e33b36c57a5225 018c158d6e1d5f43 cd055f3a
0 138 141 18bc0241f64066e4 5e8d7f700685469b e53f5676
1 138 266 93649a83e2843eed e9c6715dbb1378c9 78d46c88
2 138 266 d093ecb8be49b217 10f3738e8269d815 7c1ada6a
0 139 142 188d7ab58ad53037 bf9a66135ac54a0b 7962fb0b
1 139 267 95c7f2a29faf63f5 f8a1fc99fae31c6f 6c5c6104
2 139 267 42bda7ebc20dae2f e5cdf2eec0e27a90 7b54a183
0 140 143 a22c8a067191e813 87100014f215cc46 0b64e2ac
1 140 268 04a559e94e47430d de895ec3658514a0 7f43dd98
2 140 268 ca626d04ab80be5b f7fb0bf551db1f2a 112b8603
0 141 144 596f0245ff292987 ef5fa9aec0cc4d9d f67c37e8
1 141 269 eea60be28f89d5a2 ce4f6395568fb44a e4949b21
2 141 269 4c84b8eee9200824 0e7e2bd7c38653e2 86916c5e
0 142 145 cfdd56fed7a67f83 e55bb6841b5d1d3f 4e959b03
1 142 270 ad92bfe31f9dc15e 4828fcec28973fe6 1e34112c
2 142 270 d1f5008e9a53ec68 9e14114b2deb0534 6e0a111d
0 143 146 1d179d75b4168ede 7bbf142eb515ed5b 5c67ebae
1 143 271 0b4a04e88315ed0c d6e85d73ce23815d 6841fefe
2 143 271 3f2ed52834b0d6ef f90fa46b637fe121 9de5af52
0 144 147 ee353b3ce6dd01e6 1f79f439d76c2741 81015727
1 144 272 6a9e28b31bad19be 77c9d7c75894593c 017a221d
2 144 272 2bdb46d905284336 b2403e5e105b1259 689a87d6
0 145 148 5b02263bb334b448 b28fb45094a64b73 47cadf41
1 145 273 58968d8ebb8a64a6 c953b3782e7c6a0b 2696c148
2 145 273 c59046365449dff2 3f703a6b4912410f f3ec5bc3
0 146 149 ed3df3db9e2c9216 92a51100a7c4b94e f18fb9fb
1 146 274 a39ff37338c9cf17 ef33c4842a72dc92 e2e6b36a
2 146 274 6146a1b909a31d3c 75770fd9819bae22 cbf947db
0 147 150 134aa1c36ef8dad6 dea2270e5909c72c bfa64427
1 147 275 fbb6b55469408ece cb5d96df68d7f48b e34d9b05
2 147 275 26af3d1bff5fb374 ffd947f00e5c8143 5f967e23
0 148 151 81d3002e0d2249cd 015dc9a6c8de7a3e 4e1d8d31
1 148 276 8536130888fe7fb0 7832bac23a06c395 344cc66b
2 148 276 fd2fa8acddb9cd17 084c70a3cfc9979e b7728448
0 149 152 fb94cc5f04a9f9c2 82a3c1307496d824 60845b9e
1 149 277 69053c1fb58ed90e fe35a650cfca402d 031c75d1
2 149 277 d8ecc000ba32a4be 259f488d624c0599 c16c94ab
0 150 153 c98a399854cfbb45 01172b4a5b0b5455 ea462d3c
1 150 278 39cb393eff37b3c6 36f73d853dfb40d0 ecfcb7fb
2 150 278 4f0bb8aab67da4f6 bcaa712231be83d4 1171b9e4
0 151 154 8ba6af966f60f56e c4e46901031c1565 f396f195
1 151 279 1f2e986c2732ce8a 41930c20b78530c5 4d63aca3
2 151 279 1488733a1a21f8d9 ee90fadb8c158c08 44b76de6
0 152 155 961ec4c37e83982e 29b5f69f656e48e4 c08599c3
1 152 280 1070c5a727c0a427 e62bb435d30c7f11 7c1a0097
2 152 280 420e17b591def440 b8e29c5d2e3b1bf7 c9db3aaf
0 153 156 675163ae62dfa5b1 831c548d91715c86 1ca2072c
1 153 281 b78820aa46d9b888 6dc2c982693f35df dca25c60
2 153 281 4e073e929f316b1c 6dcc4a9917b0e7a2 524af73d
0 154 157 91a2bdafb15588aa 1f27e3bf6918701d 0635396a
1 154 282 93fbc97764d97c4b 09a7ddea1f93b245 62432f08
2 154 282 04d547ba25e60c59 71327e3bc31a601f d8845f3a
0 155 158 f93bb9eeba601aca 3b3d0170fe5955f6 37fa3f49
1 155 283 7094e1cc2e05d487 9c847db02a85cc6b aa54ae02
2 155 283 5acae140fa50e931 d945cd5eaf2c8306 eadad021
0 156 159 1d0e276ff0a1f9f0 488f197514029bb1 5d4d67fb
1 156 284 3f28d892ab69a245 2b2273706d0e2b29 83e9815d
2 156 284 6c6064aa5fae937a 9a97b58da193fb65 3840fe9b
0 157 160 b63953a499a60d4a 5d69e9191583007b cb7d20c1
1 157 285 8dc8cdce1b22a4f0 2dd7e7f7e4738c5e 98864365
2 157 285 48f73a0e08e87e18 adceaaa6ea53d22f db5e025f
0 158 161 04e54a74d26afff3 5b8cbff585774806 488070b4
1 158 286 ca447f26c9eb9824 c22568f95d3d09a5 022cbd0b
2 158 286 5888b074f059da16 3163b431e424de5f 3621822f
0 159 162 e16842505c605c78 50e54684dff92863 b57e3224
1 159 287 7a6c6c34101e2d66 e854c2fb73c13700 20c83bc3
2 159 287 8dcee66afd365cf0 4597daeb41615b92 baae3a94
0 160 163 455aadd4f8d5fbe8 01880d2c79e826bf c4f42889
1 160 288 66fbc02eb92adb4e 08b027b109d3a1e1 4e2d0aa8
2 160 288 ad706b27f7a30c1c 3953454ba45e916e 22005d03
0 161 164 04e4aff01169c26b b194b3a21fe75750 09597fe5
1 161 289 d8856c2f297e4241 c4f2fcc5896636f6 ade1c183
2 161 289 313c59cf501722ce 0dad1c8f8b511156 d09a48db
0 162 165 a41eb02e69017afd 9e2dfadbdbe466c8 5f42f1e8
1 162 290 ba91848cf4e17499 625441b6fca3b5c5 c8f437fc
2 162 290 f00b729352dd0065 54eaac03471ef1e6 bb25c87e
0 163 166 e4f54bb908e0ac52 7eef81ee19f4e4d0 cfafccf6
1 163 291 fe54f246eafd8a00 c183bd11b88152c4 55867bc4
2 163 291 92212a756ea9bf62 319a52edbbbe6a3e 2c2ebbba
0 164 167 96ec9e488c844784 0d72db7899e48c52 c0834ac5
1 164 292 bfa0b014f59edc97 6badfb8a5164e973 4ad0a74b
2 164 292 7b5c3264bb2a22ca b4627611f5ac30e4 2a9f3347
0 165 168 aa2831a30f181837 b2a6640ba9f496e6 3513debe
1 165 293 ab1437d0177882bf 1a0afcc8b9710e19 34888727
2 165 293 8e4f9b90657d38c1 012a279006adc72f e2c8a65d
0 166 169 7dc3e5c76dc3b892 fb6817c5e90ac532 14dd7253
1 166 294 421c6a65be7eb89a 5d0cdeab83dd94e2 ec232e0b
2 166 294 31f50d06fc2cc604 1d47a06f81cc9332 944d49a1
0 167 170 03e6b92d0cf608c9 d126580e0ba74166 5227461d
1 167 295 6816c5e2d349a638 f4db77faff4579dd aa9aaa6b
2 167 295 eeb38207ea1bff8d f0fcb67ed57d42e6 5fb2aefa
0 168 171 673bc3a3d6a02e7b 6cd0c75da641d6f3 07cd4861
1 168 296 fa1561a9f18f2794 7a8d84c95100a2c0 b1338160
2 168 296 fe23d8659c81885b b9a753faee6902da 36caa1dd
0 169 172 c00a8d3b85db9a83 8047aa1c614c3d39 69bc7d4e
1 169 297 415a24906720780b 1df6d6b4a71cff8e 4a5bfe5a
2 169 297 594fa086a791900d 24ac5aa681626308 0e306793
0 170 173 93e719c86cd4e68c c755616c0f5893f2 537d8c5c
1 170 298 91fb33f01add5046 db0834687bfd9343 112d8ca4
2 170 298 4882aa947cb5613f 63d40f931077bcf6 d0a049e1
0 171 174 265a661a7ab872f8 099b23a4decfd559 99a8f499
1 171 299 de6873e223888e49 e053e882c09207ee d9863943
2 171 299 545c93da8f864796 54a478b34d163197 ab865a8b
0 172 175 cd213f1fd75a324c a423cb59e20f5916 ced8a355
1 172 300 a1034fae8cc126b8 c1f89b71382a9959 06966acf
2 172 300 a241d045de1bdb1f 17978a5a4ac1ff83 feba5a09
0 173 176 9222f75194841a06 f3fca5ed3bb57765 b1af2531
1 173 301 cfbd40ccb56cd8c0 a700f241c0b25c7d ec0f284f
2 173 301 a608803211d25f4f 30b44440de45661f ff41ae57
0 174 177 21227c0e49046ba7 c735685e8ff3269c 6a5863a3
1 174 302 249c4bf40bcce110 e9f2ab3f0312b371 e87e5fe9
2 174 302 04410cc42b08cbdf 33e08337614389e1 53c003c0
0 175 178 aeb98231e03d118d 01b1aac85f71139a d4e98f89
1 175 303 792c1f629c13c68f afc1c84843dbc2d0 2f8d2fff
2 175 303 390ddff05f5a40fc c8be04f48ba1a09f fc6cb407
0 176 179 2bef07e1acbc1f0c 4af8a009aded5ac5 e2e52239
1 176 304 8205c3ec18a0b79d 3aa2ee05264f7782 2ca22fcd
2 176 304 73458af2ae99b313 efb45c2a35b7f5eb ffc13485
0 177 180 13ed6ac7b8ea8fa2 becc9b062e4ddcc7 d41b39fa
1 177 305 93a4b3fd94f920a5 2c21397ee9ebfd74 171d7bce
2 177 305 c444d2d7ec555959 5f1fc6eb64420ebf a8780cac
0 178 181 fe76269169db7bf4 d51a85ab8bd31b1c cdc1891e
1 178 306 5f4740900cc5d290 25ce30eeb2a8e354 eba0ed9e
2 178 306 af8756e05d0b6b22 e17a9e23d3a88706 32c95fac
0 179 182 083f2cbbe845de10 996dd422f8a102e2 d8d9ec5f
1 179 307 3aebcc805b0932f9 4e5fcf87ec12bebd 68a4ad39
2 179 307 17db13825bb7d308 166dd0cb535559d2 7263e609
0 180 183 3765693f1abddcd6 fcb4e9da8510c774 7f730d31
1 180 308 f2b9a884e8dab80e 21e57e1c461bdcea b0b6f16e
2 180 308 d72fa64d83c780d6 f6dcbb5d83c318ec bf8e6705
0 181 184 837b0e15c6ec9c94 1097c52f079a6d45 213ca673
1 181 309 5c9ed2e6c4e0402e 2c2ca06521aed954 afaa259a
2 181 309 54bb256d36c97f18 4e5d3cbaf6373d09 ae41eca8
0 182 185 c6c5ab844fecfc4a c16208212f331560 e7023a7b
1 182 310 eeefa57e8aa6c6af a819cf0d96e18b11 e228a33f
2 182 310 2b9d6ae6e94d3171 71c87dbb38748cf3 41c01531
0 183 186 1d868ac8e36285aa 7dcae5660ddeb4eb 44c67408
1 183 311 73363c0abbc89abd aae71bbf11dbcc80 c28efda1
2 183 311 da79078f5c458886 ff3aad0afba32285 47794054
0 184 187 7e327d73aca4357e 764f63ce8e214ad3 471749ab
1 184 312 761be00772b02f97 e2957cdd17bf43f9 1384e221
2 184 312 be6116acd348e927 589f35725b82b8b7 74b63fb2
0 185 188 7aede13bd5697f89 8840767ad36ffc57 5002f3e4
1 185 313 b2966ad10d561ff8 77e122d8f6dbb9f7 f063ff2a
2 185 313 2baae8849831ab20 c9350e8a9ea547a1 78f8b4b5
0 186 189 73f60e493fd7ff6d 0190259e6de3dc7a c39e5e93
1 186 314 dd3d9745405f6bec d8381ccaff9c642a 648f0243
2 186 314 8bfa84324a11739a c2ccbaf8b5ea404d 9fd6a01a
0 187 190 a167e70881bb1cd4 5fe12c5b35032281 d3c6a97d
1 187 315 6b89c67c7ae1d8c0 ecca491cbb2d17f1 df381b33
2 187 315 a335a6c77d3624b1 5127d803559e3c2d 7079a4bc
0 188 191 b730b82f584b89ce d5d5e0273c1e1015 1b905f84
1 188 316 8bbd1abafe827deb 9befde7d680fee90 65913462
2 188 316 a443881cfba6679e df7d90bbe2ae5329 dca39267
0 189 192 2b0234c332d57750 d8a8391f1492c2ce 4a836412
1 189 317 de1d6388bd4c530e 050d23c63234bc01 0ddc75e4
2 189 317 6ceed61ef65fb4bf 1eb20cdbf0c9f156 e695ea30
0 190 193 35c15dced563f82d 6b0a09b508826a5b aab85a70
1 190 318 fd9a0c41fc651858 7fb1fae1d0440bc5 46a910ee
2 190 318 d36701f4716c79f8 87fdea940369143a 2143090f
0 191 194 275fb8aa73274d67 c079545d1beee4f4 48735743
1 191 319 5d7a95b4d0083e5b d472c9cf4462d750 c4aaf92d
2 191 319 339a0f02bd3fd288 f104c863cb3d8700 ec022399
0 192 195 acc33a2662c676ad 82ba2306eef351fa f69103c0
1 192 320 ced18d1f96d9b61a 4ac642a30af730a2 93f5f1d3
2 192 320 53631ab95fe54461 653c782c463fb9a4 7e1bf2a5
0 193 196 c13eceed13d85c59 7867631b60646ae5 5fae2df5
1 193 321 e1e4f511c4c224a9 e9031b0719609611 1e666f49
2 193 321 0c6da78e0fe74c2d 03439247b6db2f09 7f1a5412
0 194 197 8b7d1553ff6a1961 50f13b187b127658 913e81e2
1 194 322 68d0f0ce075813e7 2b664eef0b1c4239 5440716b
2 194 322 3fd4d4dfca9868de 8a2bd931434c2743 1e0524c0
0 195 198 c6404d37e0d873f0 8b262318e18ec005 b83f10f3
1 195 323 aa16a0833870dd67 030223fca20a6a28 f5715f37
2 195 323 af4a54370a6aeaf8 bdecde2b752318bb 7c7e2ee5
0 196 199 1eb100f6ff157810 3ed6141269b6023a 72e44b89
1 196 324 3bb045cf10b4b04b 48652021aa210613 c0b38320
2 196 324 1e644bcc761cef88 d7046b0bc0db1854 b56f96d2
0 197 200 c4abc8eccfe50e35 e6f35a3cbaec0d61 f945caec
1 197 325 1913f3e01cbfde4b 9ce4d4ef8cfd5c40 e1ed5d84
2 197 325 b3627a0151d5ce3b 252d39e1c0017bfc f5d87ad7
0 198 201 a9a619ff7eeabe58 d7e3c1125a2ddb58 44e98291
1 198 326 9928b9f5316556fc 0ece8c22ecec763d d70fe176
2 198 326 0b86b9eca971164e 3b6705835b18fbb1 d761b2ff
0 199 202 2cb16c0a5450204e 0ad3d5ac0438251f 325e2e1d
1 199 327 89702a109db1164d cfaff7a24d11c970 baea5206
2 199 327 f52664174d6ba291 62313586a815f360 411ff2fa
0 200 203 8adfb5faf9a4951a b0b5e8e36b0f3216 313424ee
1 200 328 e53cbf07787d0be5 76f6939464a44089 625a7ddf
2 200 328 7be63699b0b2d72d 5ef73ffafbda35cc 8cdf0ed4
0 201 204 242bab219920ad20 de03d705a7522991 50a7e88a
1 201 329 23f09745ed6f9bfb d59df0cdca38560c 028878ae
2 201 329 e7dde1d373b54eec d664d6f9c7b9f8ad c45db281
0 202 205 9ccea3e9f9938db6 c997c6a55c3e50b5 c7229dba
1 202 330 c3623641ca2c7af8 27bb7c7b3d1d5cc1 e1920452
2 202 330 aae41baadbfe3dd5 c164c663e5ab5590 feed839b
0 203 206 c31e0b3be3704cdc 7b7c9b95ec1812b2 ceee42c6
1 203 331 0a29b349e93cb655 40af4012d1515442 0973b560
2 203 331 4ae9185adb262fc2 d8224c26849061e6 43895108
0 204 207 abe5c454bf99be6b 3d4c5e00caf95fb2 21fe85f2
1 204 332 30d4dcf01de91147 85cb9f87d3bc836f 5d051196
2 204 332 c4501e4468da365c e26ee36a47743ad0 8d428788
0 205 208 0255c8b763aaae76 d1f008658e20a26c 9eea28c5
1 205 333 3a1035ff03f555e7 891f34faee8af17e a7a8c690
2 205 333 ce1a60c7a7765f12 3103ab5eef34d7ad d957eda7
0 206 209 de5f7a285af214b8 8243bf05ab57ac52 fbfc2e0b
1 206 334 a8da2833deba510e 384f6d9093c0d70d cb65281d
2 206 334 1728ab5b2128d44d 77993d13a6d3fd01 f804b5ce
0 207 210 a1b7b54847c7e9ee 8dac1488cabe4cfb 0711d928
1 207 335 8ce7dd46a16cd6f2 ee7b287226bd03a0 9aeed858
2 207 335 7488c46d942f7033 558d3e8fb9023382 6d881619
0 208 211 04546ee5cc484383 e82744bf2e7f96ac 8544191c
1 208 336 3bd3c5685fa6aaf7 9a2bfbd1beec3981 d5304e85
2 208 336 68810e8380ef6b9d 63b310827902176c d886518b
0 209 212 97144cd851398cc0 434d25ae96161e86 a8ee7eb0
1 209 337 f2598d6e871f18d3 3853df6d03550d06 ce2737b1
2 209 337 cd0c48989361a755 979835534d1c9a37 9fb7a863
0 210 213 8def251efeb2f6f0 5fb979fa190f3db0 de732256
1 210 338 023a428b683d16c4 29ac6e54c040bd02 06462a71
2 210 338 027007f781d3c9bb 7a9c7fb7fb3a3bce 2253f700
0 211 214 ac7cb5910c6fc75d ff7c93d19dc38815 f542093b
1 211 339 0926a083bb2dcaa5 281ef6c14660d5e1 9b34a6d3
2 211 339 52c11d367e028bb3 34b400242ad60b3c 8fbcdb28
0 212 215 ce83ea3ce028a1b9 3095845813adf566 4f05770c
1 212 340 6ee806f2de4fbee3 f3c079602319e90f 842515dc
2 212 340 b1cbe7c1f6b47b7d e15d780e339d7c4d a803dd8a
0 213 216 07a6fada948849d1 758fd401cf3ae5b8 009d34a1
1 213 341 f7a52590ef9b0230 a1a81014bec07f5b ed173d64
2 213 341 74a0f5e6bcf07b87 44f5a63b90c8b07d f077b57d
0 214 217 3adf19f17d1169e9 1bd1263f50bc0ce4 2f71e67b
1 214 342 a44aa37dc1901cad 136f6cc9d323ce31 aba1362b
2 214 342 727893cae78ff2c5 c7b349ab001c5b65 51adbdea
0 215 218 c139e7d309b9a83a 6c503e126b3a3d1a a34cca5e
1 215 343 5947f8a6e7933b6f 75cf3e6f8232fc7d b5a2f7b7
2 215 343 f711fccb8d653f67 844ce4c34dd8940d 3b3b3c12
0 216 219 0a138ee59c2f710e 805085033009bf90 3390ef6b
1 216 344 7a5dfea756912c89 beae833faefe97d2 69cd956d
2 216 344 d9779d0ba9829412 f799030714edd90b f28482d5
0 217 220 0e9bf33169cdd72e 32a47fc0cdab6e8b 7a05d5df
1 217 345 9a2c81f1acad0255 74fa54e1ece16b4f 3b6917c9
2 217 345 abb342d5961d49dc 65093094ca0b6290 975fbb9b
0 218 221 33d7fdf82ac8a02a 90d1190ccfeb7395 2a04aed5
1 218 346 a7c5cd51bd96ef5d 18f7cbda92c69287 e28ccda7
2 218 346 f508df510ef15670 89f2ab53ebda8b24 4366409e
0 219 222 7529e2126466a03f a2803e17bdcfcac8 310577cd
1 219 347 460992159cf526d6 54c2fc2eb6428abf 6dca4ee0
2 219 347 70ab8664b6f6315e 1047d68ebb5e8d4f be5a5dff
0 220 223 3cbd4dec8ebc88b6 7d757fd703055eca b6fcbec3
1 220 348 dc6270aa9cd22a8f 9913af166c07112d 9b4a3ab0
2 220 348 411b4950cae8ee61 0fa1d352fffb59d4 0a49ae4b
0 221 224 7a2f617ca78fb842 5b65a440e6e4e13c fb40cd41
1 221 349 b76ff0bdfbcda36a 9d5bdeb6c9ad25b7 4bbf5c7c
2 221 349 b7895cd604ca374a ec23d615bf59fc09 e212509d
0 222 225 0c6c94092eef4648 c81896f5b36e1bb3 e18eed2e
1 222 350 fd1e08e84ea9ea3f 4c27a10749ed2000 5f20a029
2 222 350 a8ad8fb0dae5c9fc 5b3183c4bd724574 d75786d0
0 223 226 6e3d4e70ceb9bdb7 da8942e40d001e08 2ea47661
1 223 351 fe32ff7d5580c63f b2d473f11c6a35b9 dafbbcb3
2 223 351 7909e17e6720ec28 2272516d3c2e831b 1e418d20
0 224 227 0b2448cd42744530 77469e673abc3a0d 85d777d1
1 224 352 012678c71691fe9a 7af4f59c55c4b355 ff48e897
2 224 352 44d192e4aef31a3e f1a7316e8c3f0c2b 73610884
0 225 228 bd0ec452ed24caf1 9b927e7ec4e00cf3 c4fc2fb4
1 225 353 a56cc83e093c7e35 be4ed437539e59a8 3e574764
2 225 353 9ff1ac19e5eeb40c 7aaaa606a7ee40c5 ab56001e
0 226 229 374a6a27be7a9a9c c49d0d8e91d4b90e 2ce2399f
1 226 354 cc087b4144cd5f17 decd76503ec73b25 5516fb79
2 226 354 c0e6f65fd7d17c2b ccbfedacb4c2c999 bc03b890
0 227 230 8feae16cb1a9baaa 781f789f3f299c2b 36d4f324
1 227 355 7064cdf7b5152a18 701135043566e1d1 895b0e6e
2 227 355 fc3039a78a589785 371a87fd3ed5c587 b02795a8
0 228 231 ee97eb8f0f369804 45f9e8b0683a7706 64a006f4
1 228 356 c7220db8668b1fa1 442072db09b804ac 27c5ff29
2 228 356 213313092c5377a1 66f11dba7cc4957c 498adeab
0 229 232 f309ccae7bc8fe67 69ac9e81e3e5fd9c 94a46482
1 229 357 d434cfa97348f06c 2a3293098789443d de6fccc8
2 229 357 c47abb5579f3b468 18c7d5bc9a17c618 ea02298b
0 230 233 0e2477411565c1c6 bd638d55bad8475b 5d98df48
1 230 358 008ef39c34054860 1c0ce6b8181e4009 089ebc7d
2 230 358 a300cae208aa5f24 f2bb57223f76c6e8 d5887926
0 231 234 1a5c2308fa2435aa 48b1c3aa0e269ea3 934cc157
1 231 359 3e2d4c8c9eefc60e e85e71ebad0c7750 53833d6c
2 231 359 7c3f7880efbd916a 02c399c5a239b4b6 d17ae9c6
0 232 235 12f9aee5d05deed9 8228cadb0018c181 701e17d2
1 232 360 08c1b205acee52fb 6409e5db54b7cf30 9ccc814b
2 232 360 f1ddb19e47a03a4d bb479da99eed75f1 484821a2
0 233 236 f966840322815e33 fba0ee4ab84bd6ba e57ecb3d
1 233 361 98a679532a266ccc b3ae7cfc62e28f29 2c6127a7
2 233 361 625dded670b96033 471262aa04641e70 8b7103b9
0 234 237 455c1a4808344f34 511ff859f6bdb38d 418b67a4
1 234 362 fa419ff31db7dede 9abb8fdc1edd7cda 9c6ba0cb
2 234 362 515cce3efb5f8f09 83e5ab0fc7023079 c5b24adc
0 235 238 ceb7e59dc65cfffc f0df29503838e84e ea8cf6b8
1 235 363 b7b7fc2df5dadaf8 fc1a277df6c66ad0 18f71f09
2 235 363 35e7c5f57ac10a15 ab13058ac19b47be d90f3abb
0 236 239 b60b64e64764b79c 8a5fed90c718d37b 7c9ebd1f
1 236 364 afd63ec1907a696b 647e2e919ef2b13d ef4724f5
2 236 364 82dd473d50a85f43 4064bf16210a32bf 05fe31ab
0 237 240 6ef4f5a5c09a4d60 30d65783d6e2c9e1 88fe089e
1 237 365 8fbf741f12a4109a 6f6246f2e92b091d 969e8e5a
2 237 365 250e8d0ebd9ba3ab b847d4cdb0f5f80a 63ff84af
0 238 241 be46a387d109a2dc 4a7bb6e83bee0230 6dd96b4e
1 238 366 e3817d4c85abddc5 10be3037b052bcaa 23f9d390
2 238 366 b5e21d47f95ddb42 792fa0f609943b48 aa019936
0 239 242 61503cfd16f8277b 9eb3b6af4030d548 2a946e28
1 239 367 4d0685a6e2f8003e 77f55548f2b1e460 3197455a
2 239 367 9f559bf966de76a2 6c47655b58b5f8ab 5703e089
0 240 243 2348736af4ec2caa 5f49ed12a17827b3 4aa1c50a
1 240 368 0511643dde5e06d1 c4d33fee8765bd6e 492ddd7e
2 240 368 84adc82dacca2bab e5533158952c967e b4047856
0 241 244 9b509ea6616c9da0 16b7101b7fb43639 2f15a9d9
1 241 369 570c80db324adcc9 36a5a593f4bf1579 80ba09eb
2 241 369 d17d9b723e0e2cc0 e2244b52788de964 172853e8
0 242 245 833f614f54f87147 36df20456e234844 d0b2b162
1 242 370 497e2201c2dbf7f7 c78581393d53b0c0 6966eadf
2 242 370 33405685bb3dde58 c9dd36424e68e1a7 7245e3bc
0 243 246 129e7074b18eb33c 19c6ff81494a3c2e dd3bea67
1 243 371 482b02260ded68af 7a356a3ceaae50ed 4fd727c3
2 243 371 507bc01740a73f4b 346110818beb3348 19995086
0 244 247 c10b99220afc3bd4 e9253b32d16f9193 2f675db6
1 244 372 7495d6ae84c8d8c8 d728ac98c498742c 86dfaf16
2 244 372 949984ece2ccac8b 15d7531338c5a1c6 45073c8c
0 245 248 2ab1cd754967dd1e f73f38ec2f3e8d66 dd15588b
1 245 373 af99ea260ec1d73a 30f80c93a630cd7e a05621cb
2 245 373 ca4fdd766224c43d 8262bf932293f5db 33483e8f
0 246 249 750fe74c8911a519 872f689602877b91 c76ff586
1 246 374 5d16ff7d831bb350 0c0352b855fc822b 4cb771a3
2 246 374 23b716f53ddbe055 996d5051c835d3ce 762cd221
0 247 250 3869b9418c705d65 7fcfac3a1d243de2 dd520016
1 247 375 10f3aba83cd624e0 463a8e5c27569f6f 99a7a613
2 247 375 ba895c86a0e22c42 48935130e62f64d5 563a6e0f
0 248 251 bec61616dbfe88ee 1ab1745165230249 39cbcaac
1 248 376 21ce06ec26d39894 3d019aeb8f02ffae 017b22fc
2 248 376 f04c2cbd1a522dd2 96dbe9c8c59fb0fa 17bcb6bb
0 249 252 b94aed0776e19483 4fcfffcf144fb233 733d5518
1 249 377 be42f45ceb6f863f a64869866bd3b18d aa7c0633
2 249 377 56720bec99232450 2ad8244772ce6fa8 8aa7a027
0 250 253 ddb4b397688f11c6 2998674e9eb0b419 61282061
1 250 378 690c05ea853ddacf 3fe7740db13fee48 064f3619
2 250 378 edd5c7926329ec85 a51eebfc12d1d657 31b3c5a3
0 251 254 9013e2556b20e1a2 a25dc020a84cc097 db69fb18
1 251 379 52f9862309a993da 62b19ddc065fa799 536ba96e
2 251 379 426acbbdbf9ed1bb b380f4eeadfc38dc 21a98c90
0 252 255 83021cc5c87fb7bd 91157f6fa69466af f639ff7b
1 252 380 e73d4d967b37244e 9032ec746b1d4091 ab738dc7
2 252 380 8b67ad84bcbeac22 13b58beee9a40647 c24d1667
0 253 256 1c19e59aa5dbeb60 ff51bf73172bdd21 731c7cbd
1 253 381 f61631fbd2b11ba0 2ff37456895a0563 acc7a861
2 253 381 7ac5d46b6c6ddc8d c0050e32537b9329 19453977
0 254 257 a761a972b9085571 934cf805e6b17ef5 c9bfdd70
1 254 382 29fb648731080621 6f06ed0c9368f862 9c358598
2 254 382 0bf6fb7e714c6998 d14555aab3c3f048 2bfdc776
0 255 258 abc9fbfe5f09a6b5 dd7d0129ce8bb72f 4ea67bf8
1 255 383 d4ce64e25880f512 97e534bef2e46cce 4b29dda2
2 255 383 d722b00458ad5c9b d5779013c49b24d3 9001f8e3
0 256 259 d02d30fe704e8422 958b1e3a78f5e056 1cd19fa6
1 256 384 7012d23029a4ef2d d63839f998078fc1 13305301
2 256 384 2ba4f7e861b9a8fa b054d52a6880ed3d 854e2542
0 257 260 5bd92a4b538b4242 1dea3126d98ee7b0 1666ffe3
1 257 385 3e10a8f9cce10149 dafe9d4342ff3e42 99b37f67
2 257 385 c8f02055af6b8280 c74ba5ee309b8ca1 703ad60d
0 258 261 8282823b56699580 bf956d25d34c803c 9fdd1360
1 258 386 6ef2716fed041e37 a9589b193ba39e8d 22e1c0a2
2 258 386 ef920087acb248bd 75e167638e0cc341 524fc4da
0 259 262 0965bedaf9c4c96d 6b8dfd6c6637fd7c c6e2b8ac
1 259 387 ec7f8d5cffd975ac e04ee8083b557180 47b73ddb
2 259 387 c404b3177c23986e b6bfaa4757269ae6 3dd829c1
0 260 263 466c6134aae0415d aab35a3ac5e8d481 cbddf7b8
1 260 388 9b700b28004c8b9d 3aa5c04e830bd0e0 c69ccf95
2 260 388 5061721ab4c80e56 631541ca0e988e75 76095e30
0 261 264 1ee621b64ee937f3 2331efe0e59f740f 4150b5b0
1 261 389 aa76fcb9a7ee3e46 792366bb77c864b3 f1a8616c
2 261 389 9956b8eefde2eb7e 0fc71d3e55eb1847 ef5c9fdf
0 262 265 d4e872ddfcd7878c af91379335277c87 36f40ba1
1 262 390 96a1a3effa782342 28a5da23d2f5a533 ffbf1318
2 262 390 ff8eb195d979c510 54ac160e18c6047c ed7c276b
0 263 266 8d415edc50be7574 2192f665940c56fd 9aa8426f
1 263 391 92948924907626e1 97bf641717fb7b4c 3b8e665d
2 263 391 e7eb59513848b511 5295bcfc7cda71f1 f4fe12a0
0 264 267 cb6faa8d32064339 983ef19848755dd6 94a0a42e
1 264 392 083805b42f02eecf e43ddffe200df982 b311c877
2 264 392 f996d206c0752567 217ee82e6be54fc7 ed3f75cf
0 265 268 3ac313af17e62ed9 4cb68c7ec2e336a1 aed4965a
1 265 393 421cee7a1f9c7ee9 1963a3b072841e37 3e10b7fe
2 265 393 db38ae3e9bfd252a 78333e6bd82940cf 5de43d27
0 266 269 cb9927e7e4564aae 3fc973485ddf56dd e0af3aeb
1 266 394 5be3041fd33d888b 46341cdadc9f1fb5 2ef4a52e
2 266 394 9b8887de7e8ad12e 0662940a25291f1e dda15172
0 267 270 d040e8375fdb6cbf f76a4d999c9364eb a7206a70
1 267 395 42223865598da249 89bb1bd509d536d0 8905c88d
2 267 395 ca022d53080fb562 c336e986dc8aaf44 ed18cec0
0 268 271 b61a8e4f828ee693 aaa2f1ad95dff7e3 ae63aa18
1 268 396 8e1873a7986e0615 c1cb63e41c5eaacc 2ceabd50
2 268 396 a083a3e419c0b0b0 3f439e4ef5fe359b fcb3697e
0 269 272 4f4350a608f8f6a7 e49227fc56e5d301 77a216d4
1 269 397 c077bbf83c7568aa 228f6e1749544791 821ebdd9
2 269 397 b8ff8a70f975db6c a3215a0d3f6d01a9 5dd0d1f8
0 270 273 81372b9a6ca2bfc7 0af177eca0d42d1c 17aecd6e
1 270 398 b04ba637a644cdd9 f7c889541c56f79a 7fbf1d07
2 270 398 03aa24665ffb3d42 16aff1a963395eb0 6ab6bb3d
0 271 274 649469b3f259d834 e8a6c32e5438ee8e 066f0e8d
1 271 399 33ce10cbf0599c94 4c43dcf8a33cbe02 fe9c26c5
2 271 399 e239c63d409f86e7 43bf6bb4a008d768 d7e137c6
0 272 275 1e26f1770923f372 293fa452d892d610 7872595f
1 272 400 8f9c0ae3e2b7fb7e 3d018306bc9fb0e3 fbfa992c
2 272 400 adc44cbda049ac1c fff0ebe9b6740f1c d5fc3d78
0 273 276 b786a8c0f8d68139 8688f4beeeb45399 5a84a36f
1 273 401 a7f1eee681968be8 33455c07bab2fa07 4c94eee4
2 273 401 e29c4cfd8abed7ac 1cbec47975ec3811 35c885a8
0 274 277 04569a73d6fb0200 2466af941c374f82 8ef07cb1
1 274 402 5ec8e38c99ee4ba1 04734bd32a495f00 d0d4e992
2 274 402 c178fbd8df20ed22 d1f937c56b481c82 8a0b8844
0 275 278 243b61981759095b 2f995e27acc4f1bd 01eea4e6
1 275 403 e1b74ba3de68d5ee 4232ada176f334e4 4fb19440
2 275 403 280c6bbe3758d7fb e72676971fa59e3b b45aadf6
0 276 279 e294eadba23c75ec 86747aa6b6c69c4d 17c67b74
1 276 404 4c1246f4c0e47cf1 aaffa1f4610a5ab8 06e6658b
2 276 404 bebffc6015d54d64 a56da8d538e444d5 9e68987a
0 277 280 b872a8d07ee13833 7dba631deb67f82d 95a512df
1 277 405 419024d928b75635 34f31b4cced63bf8 d9bdf5a1
2 277 405 cdff2cc66359d1a2 17d465923bc4542a 1188e6d7
0 278 281 f921d9af8cb2b0c8 026e1a002d4ee74b 8c641135
1 278 406 8a73ffc05fd436c1 e00af61f90992562 46c7e908
2 278 406 d0fdeec94dfe6bad 8e81364f8b2fb31f 4cbaf915
0 279 282 22e5015ec6514aa3 3ff66736cccbe190 72b5a4eb
1 279 407 c43c551318448c19 ed677b6a046c144c 1fc1895f
2 279 407 c2640a13ec146587 7c76b954c7f1aa75 0a267901
0 280 283 f2e7167b1f395b2c 8365620edb16d5bf 5db1d5c6
1 280 408 1f2c8b7449f409a4 5598bdbb1bae8ec1 c0032bec
2 280 408 d283b0e5f076abf8 18631c778c50e96a 6decd5ef
0 281 284 df8fb8cc2aef5a4b cf931ae163394ad3 2c584730
1 281 409 4616d6c0871c7776 ca5b2aaf12d27d81 9d333852
2 281 409 18005b8cee9756cd 171d651387073799 bfe472f4
0 282 285 5a828cead7d26321 a7d36f584c72dde4 8fe94ade
1 282 410 31ef614439ef4935 51b7dcb7e5ddb482 1b0090a5
2 282 410 71325008cec3cb42 bc5a90739b995c76 3c157816
0 283 286 20f85c6717ac353a a36a843d1723b86f 2934c0f0
1 283 411 718be10a9c4e87cc 12db85eb2cf63a58 22f40309
2 283 411 5de685767a3b9b49 51557f3b007e0b42 8766766b
0 284 287 a211eee307a7ecef 61631de16961633f 8bf251c8
1 284 412 d07589262354563a a72ae034e6bdf728 6ae5665c
2 284 412 da8bd53a0f21f1ee 5e8a72f170ecca55 842bfe67
0 285 288 2f463589319edd50 3becafa47dae9427 6a1204a9
1 285 413 cfba73088a00b16b e99a82e66477e71a 9e3c51a6
2 285 413 9ddace0e4bddfdd3 2c0e977ff5ef927b 9b0e3765
0 286 289 f0c2a65086c775ea 861584bf0445c0fc 9d0ba5f2
1 286 414 e0c17d7e7a15f1b1 0c002e6339c3eec9 54cf4c9d
2 286 414 297ca11a37fa0c14 ec07aac41f38a345 31b5b640
0 287 290 c20dea283a17a363 70280b707e1e5600 38d0293c
1 287 415 5309fc7d6cf77e21 ac637f6409f1a7c9 52f35c74
2 287 415 a600aded0d04845b 60b2630c187cfc20 2012ab83
0 288 291 8c291185d0fff447 1540667587ac2db2 3ab0eb6c
1 288 416 525b09c5f9a9051a aef846c9ec44c20b ec184383
2 288 416 fdec1aa71b004011 6fd6a8eb4ca8404f 3aacae11
0 289 292 8900ae9c892e11fe 28ee66330dec1293 4b45c798
1 289 417 f581776c638f3140 40892f0b7b2fb042 872f2c74
2 289 417 e10265a2c049ca7e 4f3b7f8e0d383ef2 638c535c
0 290 293 73ba00b56f98dee8 98cd343f33e20d8c 13aee009
1 290 418 c394241bcb53f8bc 0fb4f229470c33f3 26f6ce55
2 290 418 1571c06abc597280 0f4ab2fdf5e818f7 20a5c668
0 291 294 6a54c52eb43e51a6 db5723b266761717 4306b22b
1 291 419 260a8de678a4e370 2bffc6f29608c4ec a82ac26b
2 291 419 07004b725ffe83b2 03d0e2111c7bc75c 007e76c0
0 292 295 7c03d7a949404c4a e751b17ec6568275 52f6debf
1 292 420 b07d3c9ac2032c4e 113a1fdc1d8a6615 93b90ab0
2 292 420 9ad9e7316a46b65c a5f539a3edcdf39a ea668687
0 293 296 9dd61e9d374bb008 850833334a2e33bb 80323a8b
1 293 421 3eaca41ad392b060 441f2d93eed4ac3a adb1be42
2 293 421 cdec49247f941386 039921b98c89da28 909e4d4c
0 294 297 b8e7166ed4cdca62 a212aa576da59232 06e8cae9
1 294 422 9d33d95e29e34109 9874acb7f879d261 c353e7df
2 294 422 800957a471453720 112fa5a8110c3423 17588b33
0 295 298 72bf30bd12c7f339 bd6d2e541f51784b 6ca95f3e
1 295 423 ee37eed241f17572 8fb5a153daac775e 568267d8
2 295 423 4e39c5907c626251 abdd39b3924727ce 5e48067e
0 296 299 7782c8a245d58c6f 6dd58b1ea1a779f7 f3c4d2b9
1 296 424 57472dcb052766c6 68e3cbae9612b06f dd3e4dde
2 296 424 de8ca9b077ac3b92 529f215c716aad6c dff4063a
0 297 300 348ec239583550ce 79e2ce18d67d717a 4d94cb84
1 297 425 bab11dff6744ada0 e513a11c32cb2c9b dff15bc1
2 297 425 1e29983d43ba5d05 9c8d9311176cc797 0f34ffcf
0 298 301 3dfe2492d0e25ee1 3db72a3dd4fd6735 2aad8158
1 298 426 cbe32b3a555bd79a e483eeec6e118cee a2943fc2
2 298 426 1ee5860da172a460 f1b2e842bb760025 6580bc08
0 299 302 234ed09eb01aa65c fde8d8f1ef45666d f7131cdf
1 299 427 beb256458f52e617 9c41345a8a8bcfd4 00a0d9b3
2 299 427 be49ca846fbb0e58 5e1aae6cc6ee8e0d 0d48409b
0 256 259 8d9d80b9daffe12f aa8eee6c0d3e5a11 092b9616
0 320 323 2b6e4e37ff8d4859 891cec07bb57dbc9 5ea7df7e
1 256 384 a82533644f435c18 d36be30c54639c51 f8f1da90
1 320 448 4ba43fc1f492e5b1 7df806825ccb2f8c 3cfdb6be
2 256 384 0f05c44b64f4a123 5b63c6cff84fb240 fc208cb5
2 320 448 3982c9a27c27eea8 9f4fadc11d15de5c 60dee276
0 512 515 39d0cce088ebc7c2 2dec76309edad6be 21f2fce9
0 640 643 d09db9dbd498b8f4 1f5c848da3eaf1d0 de922fca
1 512 640 0d49c043cdb08c81 387e10a856bcf402 1227f151
1 640 768 20b57a469e0077bf 244083443473d230 f8f1942b
2 512 640 7e08f850fbc5ce94 3c99126930052326 3df9647c
2 640 768 7629b96553478a58 df5040836877797d 5c2924bd
0 1024 1027 0d640286ce77151a c367a69747ffa6fa 2ab361ae
0 1280 1283 272d0360ce766dd8 6240836528668e3e 4c05f07c
1 1024 1152 a25ae57f0d4faa00 2a88676716a8b6b9 eed1638b
1 1280 1408 5870e96faf2212fd 66cbf472a3d4f1bc 3c8814d0
2 1024 1152 9d719803c2601d4b 46e4fdc8dfcf62ee 91f52efd
2 1280 1408 1c746583c31ab536 1523a0a81d19f648 c6c7d222
0 2048 2051 2a335e84c441d71f 32c3ff5aa6813860 553f5f52
0 2560 2563 6770cba77654fa85 8f6bb661b4a2b99c 8d5bfa70
1 2048 2176 81d15a0973234dca 4756fb58a3d359b7 a3e856ed
1 2560 2688 003524e2c1525752 d8c2cd41ae3f8831 07e09dfc
2 2048 2176 c48b81d1deb1c1b7 e66c3967a7e728c4 fee9db9c
2 2560 2688 e7663ea5494bc8dd fee2adeda1721bf2 bfa36289
0 4096 4099 eea51c0fe0068a74 73d390e158b69f37 f3a1fdfb
0 5120 5123 faf10cf4225011b1 9ca00887733f2a31 4c8fd242
1 4096 4224 7122492ca5e0f4ad 1720fd4ae65e6c3b f7216714
1 5120 5248 725f130193fc9af0 9961ba4ffce4de79 a6a90433
2 4096 4224 ab51b8b3ecb8f71d 16fd2a5ba5cd192b 7739a27f
2 5120 5248 506d63cc61157b23 4227df682cc39b6b 7d99e51c
0 8192 8195 9a4b76e9ffe04509 26e4fcc421526190 3cfa7805
0 10240 10243 f0af89e80f8fa8d9 168d2e0a3fadae4e 8ab0e19f
1 8192 8320 51bc3e930e5cb4ea bff7a5c6c3cdb9fb 3e456439
1 10240 10368 ac6d8a5755356c02 25609056996d11d1 2f33f6e0
2 8192 8320 78d814b038dc28a3 3a0dc55ddfc90cde e8630d54
2 10240 10368 381a93f044b63f5c e1f9a65123fd94f0 7c88372f
0 16384 16387 95a24fac8b438b32 be182d7b8bf72914 a6954446
0 20480 20483 f19da64639ef48ab 0612660baecb420e 5e4500ff
1 16384 16512 1aafcbf69877cb83 53d7fe42f9be8a19 3f3f7f6c
1 20480 20608 f5333b92cae90200 eec22e0629a5ae0c 99bde526
2 16384 16512 bdd983bc71686827 2aaf97dba862db97 0ea7ae62
2 20480 20608 7552410990d7b884 57ac4f7b76908aed a172495a
0 32768 32771 d1b919973268b136 8906dc26e2bae1f9 77bb8235
0 40960 40963 e6e1738e0d3bcad0 86e3474f1a631a0d 0110b32b
1 32768 32896 f1141bd447435229 13c83e31e9721b2f ef787d8f
1 40960 41088 2bf69735e3341ac3 f98fb76e622c2c56 b0222aa1
2 32768 32896 b4893326e9850610 c5aeea6e2e053b6f b8770b33
2 40960 41088 03022c74782d9c78 cdc7f3d9208b0445 3945d3d9
0 65536 65539 4f43e290f526a7ef 888fc0e83321d629 aa9fbe91
0 81920 81923 0cc986405c03ae26 5626d49b8e6c63f9 bf17de3d
1 65536 65664 93be27215c34525a a578b67d87a18b21 a6eb98bd
1 81920 82048 640c47d898c1d786 ccf0652b4f9fffb1 0e1eab4b
2 65536 65664 921e3ee3a83b4cce c055d184d961f33d 820d329c
2 81920 82048 6892c7142f56a997 a17ee7162692a5db 11c5b156
2 255 383 02f07a8f3f813be7 c54673902bfb1ec1 bd654eaf
2 511 639 844e7da1b59c3e6c 2a23a883a64e7a98 daeb8333
2 1023 1151 8e8c1e4ebe657b47 320ad6a34dfc87f6 2a5ebad3
2 2047 2175 a41edef6924ee6f7 b5acd0abd3f6665b d60410d7
2 4095 4223 aa548083b8909641 d426fd53e9fc3762 0c0c9c64
2 8191 8319 c591666cef37d974 4535e9fc5f5159c7 1159a0c3
2 16383 16511 a55d6f894b7d9b6c f1c9fa60fea6a602 ff2365f9
2 32767 32895 7438e3d700490ec7 f5061c9dd24ed75a 4c2dc9dd
2 65535 65663 b43e09edb1ecb521 d92bf62c98519e65 3f80629d
2 131071 131199 dfddce36b706bd14 500a18ae59e87e0a 782ef363
2 262143 262271 ed68c8bf52526a06 07b54c7da4f59d66 de129091
2 524287 524415 b1c6bd584c2b6a9d 62569bf9c323906e a499f1f8
2 1048575 1048703 bd161ca774008507 7b568d8cda895c52 86e3b399
2 2097151 2097279 403ef71e817960a7 8259dd3f6389c449 39110ee7
2 4194303 4194431 84a0ea050f503043 287bf703bc27df80 643c8227
2 8388607 8388735 a59593b4c3676c39 f53e9a59f3da66a0 35f71805
2 16777215 16777343 dcec44ca97c865ec d6a9522c4b14b2a5 0f900e80
TOTAL 971 34639743 d6a9522c4b14b2a5 0f900e80
";

// Focused per-call C vectors include exact initialized bytes. Zero runs use
// ordinary constant slices; all other bytes are recorded explicitly.
// Tuple fields: kind, value, capacity, pending, seed, C phase, saved, cumulative
// XXH64, initialized bytes. The first empty ML call observes saved=0 only because
// the oracle initializes C's inactive field before invoking unchanged SEQ_gen.
type CallVector = (u32, u32, usize, usize, u32, u32, u32, u64, &'static [u8]);
const CALL_VECTORS: &[CallVector] = &[
    (
        0,
        3,
        0,
        1,
        0x87654321,
        0,
        0x00000000,
        0xef46db3751d8e999,
        &[],
    ),
    (
        0,
        3,
        1,
        4,
        0x675c1a69,
        1,
        0x000000d3,
        0xd2da77930f69647c,
        &[0xc6],
    ),
    (
        0,
        3,
        0,
        4,
        0x675c1a69,
        1,
        0x000000d3,
        0xd2da77930f69647c,
        &[],
    ),
    (
        0,
        3,
        2,
        2,
        0x675c1a69,
        1,
        0x000000d3,
        0x9374fb588da16650,
        &[0xd3, 0xd3],
    ),
    (
        0,
        3,
        0,
        2,
        0x675c1a69,
        1,
        0x000000d3,
        0x9374fb588da16650,
        &[],
    ),
    (
        0,
        3,
        2,
        1,
        0x675c1a69,
        2,
        0x000000d3,
        0x2df091c99ce857db,
        &[0xd3, 0xd3],
    ),
    (
        0,
        3,
        0,
        1,
        0x675c1a69,
        2,
        0x000000d3,
        0x2df091c99ce857db,
        &[],
    ),
    (
        0,
        3,
        1,
        0,
        0x653ddf8a,
        0,
        0x000000d3,
        0x8de3582dceed952c,
        &[0xfc],
    ),
    (
        1,
        129,
        0,
        128,
        0x653ddf8a,
        1,
        0x653ddf8a,
        0x8de3582dceed952c,
        &[],
    ),
    (
        1,
        129,
        1,
        127,
        0xc043be9f,
        1,
        0x653ddf8a,
        0x299f88bb7c9321b4,
        &[0xf4],
    ),
    (
        1,
        129,
        2,
        125,
        0x369d2d95,
        1,
        0x653ddf8a,
        0xdd3ffa6fc30f14d2,
        &[0x8f, 0xec],
    ),
    (
        1,
        129,
        0,
        125,
        0x369d2d95,
        1,
        0x653ddf8a,
        0xdd3ffa6fc30f14d2,
        &[],
    ),
    (
        1,
        129,
        127,
        127,
        0xd12f14f4,
        3,
        0xc043be9f,
        0xe321cd33069a40ab,
        &[
            0xcb, 0xfc, 0xc0, 0xbf, 0x96, 0xa6, 0xbe, 0xbe, 0xc0, 0xed, 0xa1, 0x9e, 0xf2, 0xa1,
            0xb6, 0xbf, 0xdd, 0xb1, 0x9b, 0xe3, 0x94, 0x98, 0xb2, 0xc7, 0xe8, 0xdd, 0x92, 0x99,
            0xfb, 0xe1, 0xb7, 0x8e, 0x85, 0xa0, 0xfe, 0xa0, 0xa3, 0xb2, 0xd3, 0xec, 0xe5, 0xb4,
            0xcf, 0xf2, 0xc0, 0x8b, 0xaa, 0xa7, 0x9a, 0xc5, 0xff, 0xb5, 0xc1, 0xc5, 0xf4, 0xb2,
            0xf8, 0xef, 0xae, 0xe1, 0xcb, 0xfd, 0xe8, 0x8a, 0xdb, 0x9a, 0xc8, 0xcc, 0xda, 0xd8,
            0xde, 0xa9, 0x9e, 0x82, 0x8a, 0x85, 0x83, 0x8d, 0x8a, 0x86, 0xae, 0xb0, 0xd4, 0x98,
            0x8f, 0x8a, 0xe3, 0xcb, 0xb2, 0xbc, 0xed, 0x86, 0xcc, 0xf9, 0xa3, 0x9b, 0x9e, 0xa9,
            0x81, 0xfc, 0xbc, 0xc1, 0xd2, 0xfc, 0xc7, 0xfe, 0xb1, 0x90, 0xc6, 0xe2, 0xed, 0x94,
            0xf7, 0xf2, 0x81, 0xdb, 0x97, 0xa4, 0x9b, 0x87, 0xee, 0x8d, 0xb7, 0x8a, 0xc5, 0x27,
            0xf4,
        ],
    ),
    (
        1,
        129,
        0,
        127,
        0xd12f14f4,
        3,
        0xc043be9f,
        0xe321cd33069a40ab,
        &[],
    ),
    (
        1,
        129,
        128,
        0,
        0xd12f14f4,
        0,
        0x44e6f8bf,
        0x00d338e79c589920,
        &[
            0x8f, 0xec, 0xcb, 0xfc, 0xc0, 0xbf, 0x96, 0xa6, 0xbe, 0xbe, 0xc0, 0xed, 0xa1, 0x9e,
            0xf2, 0xa1, 0xb6, 0xbf, 0xdd, 0xb1, 0x9b, 0xe3, 0x94, 0x98, 0xb2, 0xc7, 0xe8, 0xdd,
            0x92, 0x99, 0xfb, 0xe1, 0xb7, 0x8e, 0x85, 0xa0, 0xfe, 0xa0, 0xa3, 0xb2, 0xd3, 0xec,
            0xe5, 0xb4, 0xcf, 0xf2, 0xc0, 0x8b, 0xaa, 0xa7, 0x9a, 0xc5, 0xff, 0xb5, 0xc1, 0xc5,
            0xf4, 0xb2, 0xf8, 0xef, 0xae, 0xe1, 0xcb, 0xfd, 0xe8, 0x8a, 0xdb, 0x9a, 0xc8, 0xcc,
            0xda, 0xd8, 0xde, 0xa9, 0x9e, 0x82, 0x8a, 0x85, 0x83, 0x8d, 0x8a, 0x86, 0xae, 0xb0,
            0xd4, 0x98, 0x8f, 0x8a, 0xe3, 0xcb, 0xb2, 0xbc, 0xed, 0x86, 0xcc, 0xf9, 0xa3, 0x9b,
            0x9e, 0xa9, 0x81, 0xfc, 0xbc, 0xc1, 0xd2, 0xfc, 0xc7, 0xfe, 0xb1, 0x90, 0xc6, 0xe2,
            0xed, 0x94, 0xf7, 0xf2, 0x81, 0xdb, 0x97, 0xa4, 0x9b, 0x87, 0xee, 0x8d, 0xb7, 0x8a,
            0xc5,
        ],
    ),
    (
        2,
        1024,
        0,
        128,
        0xd12f14f4,
        1,
        0xd12f14f4,
        0x00d338e79c589920,
        &[],
    ),
    (
        2,
        1024,
        1,
        127,
        0x63586fe1,
        1,
        0xd12f14f4,
        0x96ae3547d3d1154a,
        &[0xff],
    ),
    (
        2,
        1024,
        2,
        125,
        0x01a426a5,
        1,
        0xd12f14f4,
        0x4462f2b00791b7a6,
        &[0xe1, 0xb5],
    ),
    (
        2,
        1024,
        127,
        894,
        0x3cae0a7d,
        2,
        0xd12f14f4,
        0xcc07b41d8991a933,
        &[
            0xd9, 0x80, 0x92, 0xd0, 0x84, 0xda, 0xbd, 0xa8, 0x8f, 0xf3, 0xf2, 0xc2, 0x81, 0xce,
            0x82, 0xf5, 0xb9, 0x85, 0xd5, 0xc4, 0xe6, 0xb2, 0xa6, 0xf6, 0xd3, 0xf1, 0x90, 0xed,
            0xdd, 0xff, 0x99, 0x84, 0x9c, 0xf1, 0xa4, 0xdc, 0x8e, 0x97, 0xb4, 0x8e, 0xe2, 0xb0,
            0xc7, 0xba, 0xac, 0xa3, 0x9b, 0xc8, 0x9c, 0xb0, 0xb5, 0xba, 0xcf, 0xb9, 0x94, 0xfc,
            0xf4, 0xcf, 0x81, 0x96, 0x89, 0x9e, 0xe6, 0xe2, 0xb5, 0xf1, 0xf2, 0xf6, 0xd4, 0x98,
            0xd5, 0xe1, 0xa0, 0x8f, 0xdf, 0xcb, 0xb6, 0xb9, 0xd1, 0x90, 0xb0, 0xc4, 0xd2, 0xdc,
            0xd3, 0xfd, 0x83, 0xa2, 0xa4, 0xb8, 0x85, 0x8f, 0xb5, 0xd6, 0xa4, 0xa3, 0xd8, 0xd7,
            0xf6, 0x83, 0xc4, 0xba, 0x9e, 0xad, 0xc3, 0xf1, 0xd6, 0xe4, 0xdc, 0x95, 0x94, 0xac,
            0x85, 0xdc, 0xff, 0x8c, 0xef, 0x8c, 0xb9, 0x9d, 0xe8, 0xb3, 0xd2, 0xd1, 0xd3, 0x00,
            0x00,
        ],
    ),
    (
        2,
        1024,
        128,
        766,
        0x3cae0a7d,
        2,
        0xd12f14f4,
        0x742d572441cc682d,
        &[0; 128],
    ),
    (
        2,
        1024,
        129,
        637,
        0x3cae0a7d,
        2,
        0xd12f14f4,
        0x0c27f653ab309bf1,
        &[0; 129],
    ),
    (
        2,
        1024,
        0,
        637,
        0x3cae0a7d,
        2,
        0xd12f14f4,
        0x0c27f653ab309bf1,
        &[],
    ),
    (
        2,
        1024,
        1023,
        0,
        0x3cae0a7d,
        0,
        0x3cae0a7d,
        0x1f8cca2c2c2c1945,
        &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xe1, 0xb5, 0xd9, 0x80, 0x92, 0xd0,
            0x84, 0xda, 0xbd, 0xa8, 0x8f, 0xf3, 0xf2, 0xc2, 0x81, 0xce, 0x82, 0xf5, 0xb9, 0x85,
            0xd5, 0xc4, 0xe6, 0xb2, 0xa6, 0xf6, 0xd3, 0xf1, 0x90, 0xed, 0xdd, 0xff, 0x99, 0x84,
            0x9c, 0xf1, 0xa4, 0xdc, 0x8e, 0x97, 0xb4, 0x8e, 0xe2, 0xb0, 0xc7, 0xba, 0xac, 0xa3,
            0x9b, 0xc8, 0x9c, 0xb0, 0xb5, 0xba, 0xcf, 0xb9, 0x94, 0xfc, 0xf4, 0xcf, 0x81, 0x96,
            0x89, 0x9e, 0xe6, 0xe2, 0xb5, 0xf1, 0xf2, 0xf6, 0xd4, 0x98, 0xd5, 0xe1, 0xa0, 0x8f,
            0xdf, 0xcb, 0xb6, 0xb9, 0xd1, 0x90, 0xb0, 0xc4, 0xd2, 0xdc, 0xd3, 0xfd, 0x83, 0xa2,
            0xa4, 0xb8, 0x85, 0x8f, 0xb5, 0xd6, 0xa4, 0xa3, 0xd8, 0xd7, 0xf6, 0x83, 0xc4, 0xba,
            0x9e, 0xad, 0xc3, 0xf1, 0xd6, 0xe4, 0xdc, 0x95, 0x94, 0xac, 0x85, 0xdc, 0xff, 0x8c,
            0xef, 0x8c, 0xb9, 0x9d, 0xe8, 0xb3, 0xd2, 0xd1, 0xd3,
        ],
    ),
    (
        0,
        4294967295,
        0,
        1,
        0x3cae0a7d,
        0,
        0x3cae0a7d,
        0x1f8cca2c2c2c1945,
        &[],
    ),
    (
        0,
        4294967295,
        1,
        1,
        0xe3bbd3bd,
        2,
        0x0000009d,
        0xc6b768322fc11154,
        &[0x1b],
    ),
    (
        0,
        4294967295,
        0,
        1,
        0xe3bbd3bd,
        2,
        0x0000009d,
        0xc6b768322fc11154,
        &[],
    ),
    (
        0,
        4294967295,
        1,
        0,
        0xee1b5b4c,
        0,
        0x0000009d,
        0x0a609858fc17042e,
        &[0xda],
    ),
];

#[cfg(feature = "std")]
mod oracle {
    use super::*;
    use crate::cursor::ByteCursor;
    use alloc::vec::Vec;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};

    const REVISION: &str = "f8745da6ff1ad1e7bab384bd1f9d742439278e99";
    static NEXT_ORACLE: AtomicUsize = AtomicUsize::new(0);

    struct Oracle(PathBuf);

    impl Drop for Oracle {
        fn drop(&mut self) {
            // Only this test-created temporary directory is removed.
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[derive(Clone, Copy)]
    struct Call {
        kind: SequenceKind,
        value: u32,
        capacity: usize,
    }

    impl Oracle {
        fn compile() -> Self {
            let upstream = PathBuf::from(
                std::env::var_os("ZSTD_UPSTREAM").expect("set ZSTD_UPSTREAM to pinned v1.5.7"),
            );
            let revision = Command::new("git")
                .arg("-C")
                .arg(&upstream)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap();
            assert!(revision.status.success());
            assert_eq!(
                std::str::from_utf8(&revision.stdout).unwrap().trim(),
                REVISION
            );
            assert!(
                Command::new("git")
                    .arg("-C")
                    .arg(&upstream)
                    .args([
                        "diff",
                        "--quiet",
                        "HEAD",
                        "--",
                        "lib",
                        "tests/seqgen.c",
                        "tests/seqgen.h"
                    ])
                    .status()
                    .unwrap()
                    .success()
            );
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory = std::env::temp_dir().join(alloc::format!(
                "zstdoxide-sequence-generator-{}-{stamp}-{}",
                std::process::id(),
                NEXT_ORACLE.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&directory).unwrap();
            let oracle = Self(directory);
            let output = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
                .args(["-std=c99", "-O2", "-Wall", "-Wextra", "-Werror"])
                .arg("-I")
                .arg(upstream.join("tests"))
                .arg("-I")
                .arg(upstream.join("lib/common"))
                .arg("-I")
                .arg(upstream.join("lib"))
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/scripts/oracles/sequence_generator.c"
                ))
                .arg(upstream.join("tests/seqgen.c"))
                .arg(upstream.join("lib/libzstd.a"))
                .arg("-o")
                .arg(oracle.0.join("oracle"))
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                std::string::String::from_utf8_lossy(&output.stderr)
            );
            oracle
        }

        fn calls(&self, seed: u32, calls: &[Call]) -> usize {
            assert!((1..=4096).contains(&calls.len()));
            let mut request = Vec::from(&b"ZSEQ0001"[..]);
            request.extend_from_slice(&seed.to_le_bytes());
            request.extend_from_slice(&(calls.len() as u32).to_le_bytes());
            for call in calls {
                assert!(call.capacity <= 4096);
                for value in [call.kind as u32, call.value, call.capacity as u32] {
                    request.extend_from_slice(&value.to_le_bytes());
                }
            }
            let mut child = Command::new(self.0.join("oracle"))
                .arg("calls")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let written = child.stdin.take().unwrap().write_all(&request);
            let response = child.wait_with_output().unwrap();
            assert!(
                response.status.success(),
                "{}",
                std::string::String::from_utf8_lossy(&response.stderr)
            );
            written.unwrap();
            assert!(response.stdout.len() <= 12 + calls.len() * (28 + 4096));
            let mut cursor = ByteCursor::new(&response.stdout);
            assert_eq!(cursor.take(8).unwrap(), b"ZSQO0001");
            assert_eq!(cursor.little_endian(4).unwrap() as usize, calls.len());
            let mut generator = SequenceGenerator::new(seed);
            let mut total = 0;
            for call in calls {
                let length = cursor.little_endian(4).unwrap() as usize;
                let remaining = cursor.little_endian(4).unwrap() as usize;
                let seed = cursor.little_endian(4).unwrap() as u32;
                let phase = cursor.little_endian(4).unwrap() as u32;
                let saved = cursor.little_endian(4).unwrap() as u32;
                let digest = cursor.little_endian(8).unwrap();
                assert!(length <= call.capacity);
                let bytes = cursor.take(length).unwrap();
                let mut guarded = alloc::vec![0xa5; call.capacity + 2];
                let actual = generator
                    .generate(call.kind, call.value, &mut guarded[1..=call.capacity])
                    .unwrap();
                assert_eq!(actual, GenerationProgress { length, remaining });
                assert_eq!(&guarded[1..1 + length], bytes);
                assert_eq!(guarded[0], 0xa5);
                assert!(guarded[1 + length..].iter().all(|&byte| byte == 0xa5));
                assert_eq!(generator.seed(), seed);
                assert_eq!(generator.phase_number(), phase);
                assert_eq!(generator.saved, saved);
                assert_eq!(generator.digest(), digest);
                total += length;
            }
            assert!(cursor.remaining().is_empty());
            total
        }
    }

    #[test]
    #[ignore = "requires ZSTD_UPSTREAM pinned v1.5.7, built lib/libzstd.a, and a C compiler"]
    fn unchanged_c_generator_reproduces_all_971_request_vectors_and_full_digest() {
        let oracle = Oracle::compile();
        let response = Command::new(oracle.0.join("oracle"))
            .arg("schedule")
            .output()
            .unwrap();
        assert!(
            response.status.success(),
            "{}",
            std::string::String::from_utf8_lossy(&response.stderr)
        );
        assert!(response.stdout.len() <= 100 * 1024);
        assert_eq!(response.stdout, SCHEDULE_VECTORS.as_bytes());
        stored_c_schedule_matches_every_request_hash_seed_and_complete_source_length();
        std::eprintln!(
            "Unchanged C seqgen: 971 exact per-request length/hash/seed vectors, {ORIGINAL_SOURCE_BYTES} source bytes, cumulative XXH64 {ORIGINAL_DIGEST:016x}, final seed {ORIGINAL_FINAL_SEED:08x}"
        );
    }

    #[test]
    #[ignore = "requires ZSTD_UPSTREAM pinned v1.5.7, built lib/libzstd.a, and a C compiler"]
    fn unchanged_c_generator_matches_fragmented_calls_empty_destinations_and_wrapping_lengths() {
        let oracle = Oracle::compile();
        let capacities = [0, 1, 2, 127, 128, 129, 1023, 1024];
        let mut compared = 0;
        let mut generated = 0;
        for seed in [0, INITIAL_SEED, u32::MAX] {
            for kind in [
                SequenceKind::MatchLength,
                SequenceKind::LiteralLength,
                SequenceKind::Offset,
            ] {
                for value in [0, 1, 2, 3, 127, 128, 129, 1023, 1024, 4097] {
                    let mut reference = SequenceGenerator::new(seed);
                    let mut calls = Vec::new();
                    loop {
                        let capacity = capacities[calls.len() % capacities.len()];
                        let mut output = alloc::vec![0; capacity];
                        calls.push(Call {
                            kind,
                            value,
                            capacity,
                        });
                        let progress = reference.generate(kind, value, &mut output).unwrap();
                        if progress.remaining == 0 {
                            break;
                        }
                        assert!(calls.len() < 4096);
                    }
                    generated += oracle.calls(seed, &calls);
                    compared += calls.len();
                }
                // At unsigned extrema, only bounded prefixes are requested;
                // the large pending count is compared without a multi-GiB run.
                for value in [u32::MAX - 1, u32::MAX] {
                    let calls: Vec<_> = capacities
                        .iter()
                        .copied()
                        .map(|capacity| Call {
                            kind,
                            value,
                            capacity,
                        })
                        .collect();
                    // A wrapping ML request completes after the two guards.
                    // Stop its observation there rather than start a new one.
                    let calls = if kind == SequenceKind::MatchLength && value == u32::MAX {
                        &calls[..3]
                    } else {
                        &calls[..]
                    };
                    generated += oracle.calls(seed, calls);
                    compared += calls.len();
                }
            }
        }
        for kind in [
            SequenceKind::MatchLength,
            SequenceKind::LiteralLength,
            SequenceKind::Offset,
        ] {
            for value in [127, 128, 129] {
                for capacity in [1, 2] {
                    let mut reference = SequenceGenerator::new(INITIAL_SEED);
                    let mut calls = Vec::new();
                    loop {
                        calls.push(Call {
                            kind,
                            value,
                            capacity: 0,
                        });
                        reference.generate(kind, value, &mut []).unwrap();
                        calls.push(Call {
                            kind,
                            value,
                            capacity,
                        });
                        if reference
                            .generate(kind, value, &mut [0; 2][..capacity])
                            .unwrap()
                            .remaining
                            == 0
                        {
                            break;
                        }
                    }
                    generated += oracle.calls(INITIAL_SEED, &calls);
                    compared += calls.len();
                }
            }
        }
        let stored: Vec<_> = CALL_VECTORS
            .iter()
            .map(|&(kind, value, capacity, ..)| Call {
                kind: kind_from_number(kind),
                value,
                capacity,
            })
            .collect();
        generated += oracle.calls(INITIAL_SEED, &stored);
        compared += stored.len();
        std::eprintln!(
            "Unchanged C seqgen: {compared} exact per-call byte/phase/pending/seed/hash records, {generated} initialized bytes; capacities 0/1/2/127/128/129/1023/1024, three seeds and unsigned extrema"
        );
    }
}

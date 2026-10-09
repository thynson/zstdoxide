//! Decode unchanged zstd v1.5.7 ordinary Fast frames with the public Rust API.
//!
//! The independently captured C frames, source recipes, state observations and
//! hashes are documented in `upstream/fast-frame-publication/README.md`. These
//! tests require neither Python/C execution nor JSON/gzip dependencies. They do
//! not assert that a Rust Fast parser exists or reproduces C candidate choices.
//! Eleven abandoned header-only captures are deliberately not completed frames.
//!
//! Source counterparts include `ZSTD_compressContinue`, `ZSTD_compressEnd`, and
//! normal block publication in pinned `lib/compress/zstd_compress.c:4437-4445`.
//! Tiny-input and first-block exceptions remain observed frame properties.

use zstdoxide::{FrameHeader, decompress};

mod captured {
    include!("upstream/fast-frame-publication/cases.rs");
}

use captured::{CASES, Case, FRAME_BYTES, FrameExtent, SourceRecipe};

/// Read one positive, consecutive extent; errors preserve the reader cursor.
fn take_frame<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    extent: FrameExtent,
) -> Result<&'a [u8], &'static str> {
    if extent.offset != *cursor || extent.size == 0 {
        return Err("nonconsecutive or empty frame extent");
    }
    let end = extent
        .offset
        .checked_add(extent.size)
        .ok_or("frame extent overflow")?;
    let frame = bytes
        .get(extent.offset..end)
        .ok_or("frame extent outside pack")?;
    *cursor = end;
    Ok(frame)
}

fn noise(mut seed: u32, length: usize) -> Vec<u8> {
    (0..length)
        .map(|_| {
            seed ^= seed.wrapping_shl(13);
            seed ^= seed >> 17;
            seed ^= seed.wrapping_shl(5);
            seed as u8
        })
        .collect()
}

/// Independent Rust expression of the frozen Python input recipes. Recurrence
/// shifts operate on u32; all copies use the complete stated source ranges.
fn source(recipe: SourceRecipe) -> Vec<u8> {
    let periodic = || {
        let period = noise(0x1357_9bdf, 257);
        (0..4096).map(|i| period[i % 257]).collect::<Vec<_>>()
    };
    let a = || noise(0x2468_ace1, 131_072);
    match recipe {
        SourceRecipe::Empty => Vec::new(),
        SourceRecipe::Uniform => vec![0x5a; 4096],
        SourceRecipe::PeriodicUniformPeriodic => {
            [periodic(), vec![0xa6; 4096], periodic()].concat()
        }
        SourceRecipe::RawThenCopy => [a(), a()].concat(),
        SourceRecipe::Mixed => [
            periodic(),
            noise(0x7654_321f, 4096),
            vec![0x37; 4096],
            periodic(),
        ]
        .concat(),
        SourceRecipe::Tiny => [
            periodic(),
            vec![0x99; 6],
            noise(0x1020_3041, 7),
            vec![0xee; 5],
            periodic(),
        ]
        .concat(),
        SourceRecipe::LiteralTree => {
            let alphabet = |seed| noise(seed, 4096).into_iter().map(|b| b & 7).collect();
            [
                alphabet(0x5566_7789),
                noise(0x1928_3745, 1024),
                alphabet(0x89ab_cdef),
            ]
            .concat()
        }
        SourceRecipe::Window => [a(), noise(0xfedc_ba91, 131_072), a()].concat(),
        SourceRecipe::RawCandidate {
            length,
            destination,
        } => {
            let mut middle = noise(0xabcd_ef01, 1024);
            middle.copy_within(32..32 + length, destination);
            [periodic(), middle, periodic()].concat()
        }
    }
}

fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, &b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

// These are the C begin-record source_fnv64 observations captured before this
// Rust recipe implementation, rather than newly generated expected values.
const SOURCE_FINGERPRINTS: [(&str, u64); 23] = [
    ("empty", 0xcbf29ce484222325),
    ("first_uniform", 0x8ab9e262b32da325),
    ("empty_then_first_uniform", 0x8ab9e262b32da325),
    ("compressed_uniform_repeat", 0xdabba5de12d7d1b1),
    ("raw_then_copy", 0xce9e8fd57a7b2075),
    ("mixed_fallback", 0x19945576dd79254d),
    ("tiny_stale", 0xb71cebde9ce80cb9),
    ("literal_tree", 0x66ebdb12a22c5ef1),
    ("negative_literal_policy", 0x66ebdb12a22c5ef1),
    ("window_expiry_17", 0x5ee511aa26aaabbd),
    ("window_retained_19", 0x5ee511aa26aaabbd),
    ("raw_candidate_k4_d128", 0x93f9f56f6038295e),
    ("raw_candidate_k4_d192", 0x5544373ce0f5b01b),
    ("raw_candidate_k4_d256", 0x7f9751942fb92547),
    ("raw_candidate_k4_d320", 0xdd6d8d60d271ffad),
    ("raw_candidate_k8_d128", 0xd137331ce5c1300e),
    ("raw_candidate_k8_d192", 0xa6e7c7dbb43bafab),
    ("raw_candidate_k8_d256", 0x7d1b6044e254b21c),
    ("raw_candidate_k8_d320", 0xd3eb408db54ccbc1),
    ("raw_candidate_k12_d128", 0x29646f983caaf9d2),
    ("raw_candidate_k12_d192", 0x05fc33aff2f44790),
    ("raw_candidate_k12_d256", 0x759e4221e5ec037b),
    ("raw_candidate_k12_d320", 0x1e2d11ee9fed2a05),
];

#[test]
fn rust_source_recipes_match_all_frozen_c_input_observations() {
    assert_eq!(CASES.len(), SOURCE_FINGERPRINTS.len());
    let mut bytes = 0;
    for (case, &(name, expected)) in CASES.iter().zip(&SOURCE_FINGERPRINTS) {
        let input = source(case.recipe);
        assert_eq!(case.name, name);
        assert_eq!(input.len(), case.source_size, "{name}");
        assert_eq!(fingerprint(&input), expected, "{name}");
        bytes += input.len();
    }
    assert_eq!(bytes, 1_222_674);
}

#[test]
fn decodes_all_twenty_seven_completed_c_frames_byte_for_byte() {
    let mut cursor = 0;
    let mut count = 0;
    let mut decoded_bytes = 0;
    for case in CASES {
        let input = source(case.recipe);
        assert_eq!(input.len(), case.source_size, "{}", case.name);
        for extent in [Some(case.frame), case.whole_frame].into_iter().flatten() {
            let frame = take_frame(FRAME_BYTES, &mut cursor, extent).unwrap();
            let actual = decompress(frame, input.len())
                .unwrap_or_else(|error| panic!("{} at {}: {error:?}", case.name, extent.offset));
            assert_eq!(actual, input, "{} at {}", case.name, extent.offset);
            count += 1;
            decoded_bytes += actual.len();
        }
    }
    assert_eq!(count, 27);
    assert_eq!(decoded_bytes, 2_275_346);
    assert_eq!(cursor, FRAME_BYTES.len());
    assert_eq!(cursor, 1_607_342);
}

#[derive(Debug, PartialEq)]
struct WireBlock<'a> {
    kind: usize,
    field: usize,
    payload: &'a [u8],
}

fn blocks<'a>(case: &Case, frame: &'a [u8]) -> Vec<WireBlock<'a>> {
    let (header, mut cursor) = FrameHeader::parse(frame).unwrap();
    assert_eq!(header.content_size, Some(case.source_size as u64));
    assert_eq!(header.dictionary_id, 0);
    assert!(!header.has_checksum);
    let window = if case.name == "window_expiry_17" {
        1 << 17
    } else {
        1 << 19
    };
    assert_eq!(header.window_size, (case.source_size as u64).min(window));
    let mut result = Vec::new();
    loop {
        let bytes = frame.get(cursor..cursor + 3).unwrap();
        let word = u32::from(bytes[0]) | (u32::from(bytes[1]) << 8) | (u32::from(bytes[2]) << 16);
        let kind = ((word >> 1) & 3) as usize;
        let field = (word >> 3) as usize;
        assert!(kind < 3 && field <= 131_072);
        cursor += 3;
        let size = if kind == 1 { 1 } else { field };
        let payload = frame.get(cursor..cursor + size).unwrap();
        result.push(WireBlock {
            kind,
            field,
            payload,
        });
        cursor += size;
        if word & 1 != 0 {
            break;
        }
    }
    assert_eq!(cursor, frame.len(), "{}: exact single frame", case.name);
    result
}

fn observed_kinds(case: &Case) -> &'static [usize] {
    match case.name {
        "empty" => &[],
        "first_uniform" | "empty_then_first_uniform" => &[2],
        "compressed_uniform_repeat" => &[2, 1, 2],
        "raw_then_copy" => &[0, 2],
        "mixed_fallback" => &[2, 0, 1, 2],
        "tiny_stale" => &[2, 0, 0, 0, 2],
        "literal_tree" => &[2, 0, 2],
        "negative_literal_policy" | "window_expiry_17" => &[0, 0, 0],
        "window_retained_19" => &[0, 0, 2],
        _ if matches!(case.recipe, SourceRecipe::RawCandidate { .. }) => &[2, 0, 2],
        _ => panic!("unclassified frozen recipe {}", case.name),
    }
}

#[test]
fn completed_headers_and_observed_block_transitions_survive_import() {
    let mut cursor = 0;
    let mut kinds = [0; 3];
    let mut whole_count = 0;
    for case in CASES {
        let frame = take_frame(FRAME_BYTES, &mut cursor, case.frame).unwrap();
        let mut main = blocks(case, frame);
        let terminal = main.pop().unwrap();
        assert_eq!(terminal.kind, 0, "{}", case.name);
        assert_eq!(terminal.field, 0, "{}", case.name);
        assert!(terminal.payload.is_empty());
        assert_eq!(
            main.iter().map(|b| b.kind).collect::<Vec<_>>(),
            observed_kinds(case),
            "{}",
            case.name
        );
        for block in &main {
            kinds[block.kind] += 1;
        }
        if let Some(extent) = case.whole_frame {
            let frame = take_frame(FRAME_BYTES, &mut cursor, extent).unwrap();
            // These four schedules were explicitly compared by C; arbitrary
            // Continue boundaries have no whole-buffer-equivalence assertion.
            assert_eq!(blocks(case, frame), main, "{}", case.name);
            whole_count += 1;
        }
    }
    assert_eq!(kinds, [26, 2, 36]);
    assert_eq!(kinds.iter().sum::<usize>(), 64);
    assert_eq!(whole_count, 4);
    assert_eq!(cursor, FRAME_BYTES.len());
}

#[test]
fn binary_extent_reader_rejects_holes_overlaps_and_overflow_without_advancing() {
    let bytes = [7, 8, 9, 10];
    let mut cursor = 1;
    for extent in [
        FrameExtent { offset: 0, size: 1 },
        FrameExtent { offset: 2, size: 1 },
        FrameExtent { offset: 1, size: 0 },
        FrameExtent { offset: 1, size: 4 },
        FrameExtent {
            offset: 1,
            size: usize::MAX,
        },
    ] {
        assert!(take_frame(&bytes, &mut cursor, extent).is_err());
        assert_eq!(cursor, 1);
    }
    assert_eq!(
        take_frame(&bytes, &mut cursor, FrameExtent { offset: 1, size: 3 }),
        Ok(&bytes[1..])
    );
    assert_eq!(cursor, bytes.len());
    cursor = usize::MAX;
    assert!(
        take_frame(
            &bytes,
            &mut cursor,
            FrameExtent {
                offset: usize::MAX,
                size: 1,
            }
        )
        .is_err()
    );
    assert_eq!(cursor, usize::MAX);
}

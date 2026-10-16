//! Structural qualification of the independent borrowed scalar fixture reader.
#![forbid(unsafe_code)]
#[path = "support/ultra_scalar_fixture.rs"]
mod fixture;
use fixture::{Fixture, PACK};

#[test]
fn reads_all_native_rows_and_signed_boundaries() {
    let rows = Fixture::read(PACK).unwrap();
    let mut modes = [0; 2];
    let mut tags = [0; 5];
    let mut negative = 0;
    for row in rows.rows() {
        let row = row.unwrap();
        modes[usize::from(row.words[0] == 2)] += 1;
        tags[row.words[1] as usize] += 1;
        negative += usize::from(row.value < 0);
    }
    assert_eq!(modes, [1244, 1226]);
    assert_eq!(tags, [161, 876, 924, 151, 358]);
    assert_eq!(negative, 70);
}

fn changed_word(row: usize, word: usize, value: u32) -> Vec<u8> {
    let mut bytes = PACK.to_vec();
    let start = 12 + row * 48 + word * 4;
    bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
    bytes
}

#[test]
fn rejects_bad_extents_header_modes_and_scalar_domains() {
    for length in [0, 7, 11, 12, 13, PACK.len() - 1] {
        assert!(Fixture::read(&PACK[..length]).is_err());
    }
    let mut appended = PACK.to_vec();
    appended.push(0);
    assert!(Fixture::read(&appended).is_err());
    let mut header = PACK.to_vec();
    header[0] ^= 1;
    assert!(Fixture::read(&header).is_err());
    for count in [0u32, 2469, 2471, 4097, u32::MAX] {
        header.copy_from_slice(PACK);
        header[8..12].copy_from_slice(&count.to_le_bytes());
        assert!(Fixture::read(&header).is_err());
    }
    for (word, value) in [(0, 1), (1, 5), (2, u32::MAX), (3, 1)] {
        assert!(Fixture::read(&changed_word(0, word, value)).is_err());
    }
    let rows: Vec<_> = Fixture::read(PACK)
        .unwrap()
        .rows()
        .map(Result::unwrap)
        .collect();
    let fractional = rows.iter().position(|r| r.words[..2] == [2, 0]).unwrap();
    assert!(Fixture::read(&changed_word(fractional, 2, 8_388_607)).is_err());
    let ll = rows.iter().position(|r| r.words[1] == 1).unwrap();
    assert!(Fixture::read(&changed_word(ll, 2, 131_073)).is_err());
    let matched = rows.iter().position(|r| r.words[1] == 2).unwrap();
    assert!(Fixture::read(&changed_word(matched, 2, 0)).is_err());
    assert!(Fixture::read(&changed_word(matched, 3, 2)).is_err());
    let increment = rows.iter().position(|r| r.words[1] == 4).unwrap();
    assert!(Fixture::read(&changed_word(increment, 2, 0)).is_err());
}

#[test]
fn rejects_inconsistent_counts_and_invalid_signed_results() {
    let rows: Vec<_> = Fixture::read(PACK)
        .unwrap()
        .rows()
        .map(Result::unwrap)
        .collect();
    for (i, row) in rows.iter().enumerate() {
        if row.words[1] == 3 && row.words[6] == 0 && row.words[7] == 0 && row.words[3] != 0 {
            assert!(Fixture::read(&changed_word(i, 4, row.words[3] + 1)).is_err());
            break;
        }
    }
    for result in [-1i64, 1 << 30, i64::MIN, i64::MAX] {
        let mut bytes = PACK.to_vec();
        bytes[52..60].copy_from_slice(&result.to_le_bytes());
        assert!(Fixture::read(&bytes).is_err());
    }
    let index = rows.iter().position(|r| r.value < 0).unwrap();
    for result in [-(1i64 << 30), 1 << 30] {
        let mut bytes = PACK.to_vec();
        bytes[52 + index * 48..60 + index * 48].copy_from_slice(&result.to_le_bytes());
        assert!(Fixture::read(&bytes).is_err());
    }
}

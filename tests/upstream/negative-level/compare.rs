//! Independent safe source replay using the existing Rust RDG implementation.
//! No Rust codec or Fast parser is linked or called.
#![forbid(unsafe_code)]
#[allow(dead_code)]
#[path = "support.rs"]
mod support;
use std::path::Path;
const SIZE: usize = 5 * 1024 * 1024;
fn check(directory: &Path, name: &str, source: &[u8]) {
    let path = directory.join(format!("{name}.raw"));
    assert_eq!(std::fs::metadata(&path).unwrap().len(), SIZE as u64);
    let recorded = std::fs::read(path).unwrap();
    assert_eq!(recorded.len(), source.len());
    assert_eq!(recorded, source, "complete {name} source state differs");
    println!("{{\"stage\":\"{name}\",\"bytes_compared\":{},\"equal\":true}}", source.len());
}
fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let directory = arguments.next().expect("capture directory");
    assert!(arguments.next().is_none(), "exact argument count");
    let directory = Path::new(&directory);
    let mut source = support::noise(SIZE, 0.5, 0.0, 0);
    check(directory, "restored", &source);
    for (index, byte) in source[..20].iter_mut().enumerate() {
        *byte = index as u8;
    }
    source.copy_within(..10, 20);
    check(directory, "initial-match", &source);
    let flat = support::noise(31 * 1024, 0.5, 0.0, 0);
    source[..flat.len()].copy_from_slice(&flat);
    check(directory, "flat-dictionary", &source);
    source[0] = 1;
    check(directory, "bad-magic", &source);
    let prefix_path = directory.join("source.raw");
    assert_eq!(std::fs::metadata(&prefix_path).unwrap().len(), 1 << 20);
    assert_eq!(std::fs::read(prefix_path).unwrap(), source[..1 << 20]);
    println!("{{\"prefix_bytes_compared\":1048576,\"equal\":true}}");
}

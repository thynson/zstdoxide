//! Bounded subprocess adapter for scripts/check_reference_fast_interop.py.
//! This is test infrastructure; the library never invokes or links the C oracle.

use std::io::{Read, Write};
use zstdoxide::{CompressionParameterRequest, Error, FrameHeader, ReferenceStrategy, SourceSize};

const MAX_SOURCE: usize = 1_048_577;
const MAX_FRAME: usize = 2 << 20;
const HEADER: usize = 56;

fn parameters(fields: &[u32; 12]) -> zstdoxide::Result<CompressionParameterRequest> {
    // The protocol carries the exact signed level's two's-complement bits.
    let mut request = CompressionParameterRequest::with_level(fields[3] as i32);
    let byte = |value| u8::try_from(value).map_err(|_| Error::InvalidParameter);
    request.set_window_log(byte(fields[4])?)?;
    request.set_chain_log(byte(fields[5])?)?;
    request.set_hash_log(byte(fields[6])?)?;
    request.set_search_log(byte(fields[7])?)?;
    request.set_min_match(byte(fields[8])?)?;
    request.set_target_length(fields[9])?;
    request.set_strategy(if fields[10] == 0 {
        None
    } else {
        Some(ReferenceStrategy::try_from(fields[10])?)
    });
    request.set_source_hint(fields[11])?;
    Ok(request)
}

// The library decoder intentionally permits concatenation. This adapter's
// protocol is one frame, so independently walk bounded outer block extents.
fn single_frame_extent(frame: &[u8], mut cursor: usize) -> Result<(), &'static str> {
    loop {
        let header = frame
            .get(cursor..)
            .and_then(|tail| tail.get(..3))
            .ok_or("truncated block header")?;
        let value = u32::from_le_bytes([header[0], header[1], header[2], 0]);
        let kind = (value >> 1) & 3;
        let size = (value >> 3) as usize;
        if kind == 3 || size > 131072 {
            return Err("invalid block kind or size");
        }
        let stored = if kind == 1 { 1 } else { size };
        cursor = cursor
            .checked_add(3)
            .and_then(|start| start.checked_add(stored))
            .filter(|&end| end <= frame.len())
            .ok_or("truncated block payload")?;
        if value & 1 != 0 {
            return if cursor == frame.len() {
                Ok(())
            } else {
                Err("trailing frame bytes")
            };
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut packet = Vec::new();
    std::io::stdin()
        .lock()
        .take((HEADER + MAX_SOURCE + MAX_FRAME + 1) as u64)
        .read_to_end(&mut packet)?;
    if packet.len() < HEADER || packet[..8] != *b"ZFOI0001" {
        return Err("invalid request header".into());
    }
    let fields: [u32; 12] = core::array::from_fn(|index| {
        u32::from_le_bytes(packet[8 + index * 4..12 + index * 4].try_into().unwrap())
    });
    let source_len = fields[1] as usize;
    let frame_len = fields[2] as usize;
    if fields[0] > 1
        || source_len > MAX_SOURCE
        || frame_len > MAX_FRAME
        || (fields[0] == 0 && frame_len != 0)
        || (fields[0] == 1 && frame_len == 0)
        || packet.len() != HEADER + source_len + frame_len
    {
        return Err("invalid request extent or mode".into());
    }
    let source = &packet[HEADER..HEADER + source_len];
    let frame = &packet[HEADER + source_len..];
    let mut status = 0_u32;
    let mut resolved = [0_u32; 7];
    let payload = if fields[0] == 0 {
        let request = parameters(&fields).map_err(|e| format!("invalid parameters: {e:?}"))?;
        let original = request;
        let raw = request
            .resolve(SourceSize::Known(source_len as u64), 0)
            .map_err(|e| format!("resolution: {e:?}"))?
            .to_raw();
        resolved = [
            raw.window_log,
            raw.chain_log,
            raw.hash_log,
            raw.search_log,
            raw.min_match,
            raw.target_length,
            raw.strategy,
        ];
        let result = zstdoxide::compress_with_parameters(source, &request);
        if request != original {
            return Err("request mutated".into());
        }
        match result {
            Ok(frame) => {
                if frame.len() > MAX_FRAME {
                    return Err("encoded frame exceeds bound".into());
                }
                frame
            }
            Err(Error::UnsupportedFeature) => {
                status = 1;
                Vec::new()
            }
            Err(Error::ResourceLimit) => {
                status = 2;
                Vec::new()
            }
            Err(error) => return Err(format!("unexpected encode failure: {error:?}").into()),
        }
    } else {
        let (header, header_size) =
            FrameHeader::parse(frame).map_err(|e| format!("header: {e:?}"))?;
        if header.content_size != Some(source_len as u64)
            || header.dictionary_id != 0
            || header.has_checksum
        {
            return Err("frame metadata differs from known no-dictionary request".into());
        }
        single_frame_extent(frame, header_size)?;
        let decoded =
            zstdoxide::decompress(frame, source_len).map_err(|e| format!("decode: {e:?}"))?;
        if decoded != source {
            return Err("decoded bytes differ".into());
        }
        decoded
    };
    let mut output = std::io::stdout().lock();
    output.write_all(b"ZFOR0001")?;
    output.write_all(&status.to_le_bytes())?;
    for value in resolved {
        output.write_all(&value.to_le_bytes())?;
    }
    output.write_all(&u32::try_from(payload.len())?.to_le_bytes())?;
    output.write_all(&payload)?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("reference Fast adapter: {error}");
        std::process::exit(2);
    }
}

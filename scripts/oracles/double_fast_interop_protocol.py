#!/usr/bin/env python3
"""Bounded wire checks and negative packets for the broader public-frame oracle."""
from pathlib import Path
import hashlib
import struct

MAX_SOURCE = 4 << 20
MAX_FRAME = 5 << 20
MAX_REQUEST = 56 + MAX_SOURCE + MAX_FRAME
MAX_RESPONSE = 44 + MAX_FRAME
FRAME_READER_SHA256 = 'd1b8ee20ae3542df2a1d25748988cd6cba4a5c8c2b57c0c685b632e92dd94553'


def require(value, reason):
    if not value:
        raise ValueError(reason)


def packet(source, controls, frame=b'', mode=0):
    require(type(source) is bytes and len(source) <= MAX_SOURCE
            and type(frame) is bytes and len(frame) <= MAX_FRAME, 'packet payload bounds')
    require(type(controls) is tuple and len(controls) == 9 and all(type(x) is int for x in controls),
            'exact request fields')
    require(-(1 << 31) <= controls[0] < 1 << 31
            and all(0 <= x <= 0xffffffff for x in controls[1:]), 'wire scalar extents')
    require(type(mode) is int and mode in (0, 1) and bool(frame) == bool(mode), 'packet mode')
    return b'ZDOI0001' + struct.pack('<12I', mode, len(source), len(frame),
                                    controls[0] & 0xffffffff, *controls[1:]) + source + frame


def response(data, magic):
    require(magic in (b'ZDOR0001', b'ZDOC0001'), 'response protocol')
    require(type(data) is bytes and 44 <= len(data) <= MAX_RESPONSE and data[:8] == magic,
            'bounded response header')
    values = struct.unpack('<9I', data[8:44])
    require(values[0] in (0, 1, 2) and values[8] <= MAX_FRAME and len(data) == 44 + values[8],
            'exact response extent/status')
    require(values[0] == 0 or values[8] == 0, 'rejected output payload')
    require(magic != b'ZDOC0001' or values[:8] == (0,) * 8, 'C has no scalar observation')
    return values[0], list(values[1:8]), data[44:]


def frame_info(data, source_len):
    # Reuse only the immutable independent outer-frame reader; no execution path
    # or historical recipe policy is called from this helper.
    path = Path(__file__).resolve().parents[1] / 'check_reference_fast_interop.py'
    require(hashlib.sha256(path.read_bytes()).hexdigest() == FRAME_READER_SHA256,
            'frame reader identity before import')
    from check_reference_fast_interop import frame_header
    require(len(data) <= MAX_FRAME and 0 <= source_len <= MAX_SOURCE, 'frame/source bounds')
    info = frame_header(data, source_len)
    require(info['window_size'] <= 1 << 21, 'bounded advertised window')
    return info


def negative_packets():
    controls = (3,) + (0,) * 8
    empty = bytes.fromhex('28b52ffd2000010000')
    encode = packet(b'x', controls)
    decode = packet(b'', controls, empty, 1)
    rows = [('empty-request', b''), ('short-header', encode[:55]),
            ('truncated-source', encode[:-1]), ('extra-input', encode + b'x'),
            ('wrong-magic', b'ZFOI0001' + encode[8:]),
            ('concatenated', packet(b'', controls, empty + empty, 1)),
            ('skippable-tail', packet(b'', controls, empty + bytes.fromhex('502a4d1800000000'), 1)),
            ('skippable-only', packet(b'', controls, bytes.fromhex('502a4d1800000000'), 1)),
            ('content-size', packet(b'x', controls, empty, 1)),
            ('unknown-content', packet(b'', controls, bytes.fromhex('28b52ffd0000010000'), 1)),
            ('checksum', packet(b'', controls, bytes.fromhex('28b52ffd240001000000000000'), 1)),
            ('dictionary-id', packet(b'', controls, bytes.fromhex('28b52ffd210100010000'), 1)),
            ('large-window', packet(b'', controls, bytes.fromhex('28b52ffd806000000000010000'), 1)),
            ('reserved-block', packet(b'', controls, empty[:-3] + b'\x07\0\0', 1)),
            ('truncated-block', packet(b'', controls, empty[:-3] + b'\x09\0\0', 1)),
            ('wrong-decoded-byte', packet(b'A', controls, bytes.fromhex('28b52ffd200109000042'), 1)),
            ('reader-cap', packet(b'', controls) + bytes(MAX_REQUEST + 1 - 56))]

    def changed(name, original, index, value):
        result = bytearray(original)
        struct.pack_into('<I', result, 8 + 4 * index, value)
        rows.append((name, bytes(result)))

    changed('unknown-mode', encode, 0, 2)
    changed('oversized-source', encode, 1, MAX_SOURCE + 1)
    changed('oversized-frame', decode, 2, MAX_FRAME + 1)
    changed('encode-with-frame', decode, 0, 0)
    changed('decode-without-frame', encode, 0, 1)
    for mode, original in [('encode', encode), ('decode', decode)]:
        for index, value in [(4, 9), (5, 5), (6, 5), (7, 31), (8, 2), (9, 131073), (10, 10), (11, 1 << 31)]:
            changed(f'invalid-{mode}-field{index}', original, index, value)
    require(len(rows) == len({name for name, _ in rows}) == 38, 'negative packet count')
    require(all(len(data) <= MAX_REQUEST + 1 for _, data in rows), 'negative request bound')
    return rows

"""Independent bounded scalar requests/model for pinned zstd optLevel 0 and 2.

The model uses integer interpolation/division, not the native shift expression.
Format width tables derive from zstd_internal.h v1.5.7; see NOTICE/LICENSE.zstd.
No subprocess, native codec, production Rust, or observed output is consulted.
"""
import bisect
import hashlib
import json
import struct

REVISION = 'f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAGIC = b'ZUSCAL01'
BLOCK = 131072
FRACTION_MAX = 8388606
ROWS_MAX = 4096
WORDS = 10
PACKET_MAX = 12 + ROWS_MAX * WORDS * 4
LL_WIDTHS = (0,) * 16 + (1, 1, 1, 1, 2, 2, 3, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
ML_WIDTHS = (0,) * 32 + (1, 1, 1, 1, 2, 2, 3, 3, 4, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)


def need(value, reason):
    if not value:
        raise ValueError(reason)


def integer(value, low, high, reason):
    need(type(value) is int and low <= value <= high, reason)
    return value


def starts(widths, first):
    result = []
    for width in widths:
        result.append(first)
        first += 2 ** width
    return tuple(result)


LL_START = starts(LL_WIDTHS, 0)
ML_START = starts(ML_WIDTHS, 3)


def code(length, match=False):
    integer(length, 3 if match else 0, BLOCK, 'length')
    return bisect.bisect_right(ML_START if match else LL_START,
                              length if match else min(length, BLOCK - 1)) - 1


def maximum(mode):
    need(type(mode) is int and mode in (0, 2), 'coupled mode')
    return 0xfffffffe if mode == 0 else FRACTION_MAX


def weight(mode, raw):
    integer(raw, 0, maximum(mode), 'weight domain')
    value = raw + 1
    exponent = value.bit_length() - 1
    return exponent * 256 + (0 if mode == 0 else value * 256 // (2 ** exponent))


def validate(row):
    need(type(row) is tuple and len(row) == WORDS, 'ten immutable words')
    for word in row:
        integer(word, 0, 0xffffffff, 'u32 word')
    mode, tag, a, b, c, d, e, f, g, h = row
    bound = maximum(mode)
    integer(tag, 0, 4, 'query tag')
    if tag == 0:
        integer(a, 0, bound, 'weight statistic')
        need(not any((b, c, d, e, f, g, h)), 'weight reserved')
    elif tag == 1:
        code(a)
        integer(d, 0, 1, 'LL predefined')
        need(not any((e, f, g, h)), 'LL reserved')
        integer(b, 0, bound, 'LL sum'); integer(c, 0, b, 'LL frequency')
        need(not d or b == c == 0, 'unused LL statistics')
    elif tag == 2:
        integer(a, 1, 0xffffffff, 'offset'); code(b, True)
        integer(g, 0, 1, 'match predefined'); need(h == 0, 'match reserved')
        integer(c, 0, bound, 'offset sum'); integer(d, 0, c, 'offset frequency')
        integer(e, 0, bound, 'ML sum'); integer(f, 0, e, 'ML frequency')
        need(not g or c == d == e == f == 0, 'unused match statistics')
    elif tag == 3:
        integer(a, 0, BLOCK, 'literal count')
        integer(e, 0, 1, 'disabled literals'); integer(f, 0, 1, 'literal predefined')
        need(g == h == 0, 'literal reserved')
        integer(b, 0, bound, 'literal sum'); integer(c, 0, b, 'literal frequency zero')
        integer(d, 0, b - c, 'literal frequency one')
        need(not (e or f) or b == c == d == 0, 'unused literal statistics')
        need(not a or e or f or weight(mode, b) >= 256, 'positive dynamic literal base')
    else:
        integer(a, 1, BLOCK, 'increment length'); integer(e, 0, 1, 'increment predefined')
        need(f == g == h == 0, 'increment reserved')
        integer(b, 0, bound, 'increment sum'); integer(c, 0, b, 'previous frequency')
        integer(d, 0, b, 'next frequency')
        need(not e or b == c == d == 0, 'unused increment statistics')
        if code(a - 1) == code(a):
            need(c == d, 'same LL symbol frequency')
        else:
            need(c + d <= b, 'two LL symbols fit sum')
    return row


def packet(rows):
    need(type(rows) is tuple, 'immutable rows')
    integer(len(rows), 1, ROWS_MAX, 'row count')
    return MAGIC + struct.pack('<I', len(rows)) + b''.join(struct.pack('<10I', *validate(r)) for r in rows)


def parse(data):
    need(type(data) is bytes and 12 <= len(data) <= PACKET_MAX, 'packet bounds')
    need(data[:8] == MAGIC, 'magic')
    count = struct.unpack_from('<I', data, 8)[0]
    integer(count, 1, ROWS_MAX, 'row count')
    need(len(data) == 12 + count * WORDS * 4, 'exact packet extent')
    return tuple(validate(struct.unpack_from('<10I', data, 12 + i * 40)) for i in range(count))


def ll_price(mode, length, total, frequency, predefined):
    if predefined:
        return weight(mode, length)
    return LL_WIDTHS[code(length)] * 256 + weight(mode, total) - weight(mode, frequency) + (256 if length == BLOCK else 0)


def result(row):
    mode, tag, a, b, c, d, e, f, g, h = validate(row)
    if tag == 0:
        value = weight(mode, a)
    elif tag == 1:
        value = ll_price(mode, a, b, c, d)
    elif tag == 2:
        offset_bits = a.bit_length() - 1
        if g:
            value = weight(mode, b - 3) + (16 + offset_bits) * 256
        else:
            value = (offset_bits + ML_WIDTHS[code(b, True)]) * 256 + weight(mode, c) - weight(mode, d) + weight(mode, e) - weight(mode, f) + 51
            if mode == 0:
                value += max(0, offset_bits - 19) * 512
    elif tag == 3:
        if e or f:
            value = a * (2048 if e else 1536)
        else:
            first = max(256, weight(mode, b) - weight(mode, c))
            second = max(256, weight(mode, b) - weight(mode, d))
            value = ((a + 1) // 2) * first + (a // 2) * second
    else:
        value = ll_price(mode, a, b, d, e) - ll_price(mode, a - 1, b, c, e)
    integer(value, -(1 << 30) + 1 if tag == 4 else 0, (1 << 30) - 1, 'finite result')
    return value


def records(rows):
    packet(rows)
    yield dict(record='request', revision=REVISION, schema_version=1, rows=len(rows))
    for index, row in enumerate(rows):
        yield dict(record='scalar', index=index, mode=row[0], tag=row[1], value=result(row))
    yield dict(record='end', rows=len(rows), guards_unchanged=True)


def unique(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, 'duplicate JSON key'); result[key] = value
    return result


def load(data):
    def invalid(value):
        raise ValueError('invalid JSON constant: ' + value)
    return json.loads(data, object_pairs_hook=unique, parse_constant=invalid)


def validate_output(data, rows):
    need(type(data) is bytes and len(data) <= 1 << 20, 'bounded output')
    actual = [load(line) for line in data.splitlines()]
    expected = list(records(rows))
    need(json.dumps(actual, sort_keys=True) == json.dumps(expected, sort_keys=True), 'complete scalar observations')
    return len(rows)


def sha(data):
    return hashlib.sha256(data).hexdigest()

"""Deterministic source-only scalar boundary recipes; no observed results."""
from . import ultra_scalar as m


def neighbors(values, low, high):
    return sorted({x + delta for x in values for delta in (-1, 0, 1) if low <= x + delta <= high})


def recipes():
    rows = []
    seen = set()
    def add(mode, tag, *args):
        row = (mode, tag, *args, *([0] * (8 - len(args))))
        m.validate(row)
        if row not in seen:
            rows.append(row); seen.add(row)
    for mode in (0, 2):
        for value in neighbors([0, *[2 ** n - 1 for n in range(1, 33)], m.maximum(mode)], 0, m.maximum(mode)):
            add(mode, 0, value)
        lengths = neighbors([*m.LL_START, m.BLOCK], 0, m.BLOCK)
        for length in lengths:
            add(mode, 1, length, 0, 0, 1)
            for total, frequency in [(0, 0), (1, 1), (17, 3), (m.FRACTION_MAX, 0), (m.FRACTION_MAX, m.FRACTION_MAX)]:
                add(mode, 1, length, total, frequency, 0)
        offsets = neighbors([2 ** k for k in range(32)] + [0xffffffff], 1, 0xffffffff)
        matches = neighbors([*m.ML_START, m.BLOCK], 3, m.BLOCK)
        for i, length in enumerate(matches):
            offset = offsets[i % len(offsets)]
            add(mode, 2, offset, length, 0, 0, 0, 0, 1)
            for total, frequency in [(0, 0), (17, 3), (m.FRACTION_MAX, m.FRACTION_MAX)]:
                add(mode, 2, offset, length, total, frequency, total, frequency, 0)
        for offset in offsets:
            add(mode, 2, offset, 3, 17, 3, 25, 5, 0)
        for length in [0, 1, 2, 3, 17, 255, 256, 257, m.BLOCK - 1, m.BLOCK]:
            for disabled, predefined in [(1, 0), (1, 1), (0, 1)]:
                add(mode, 3, length, 0, 0, 0, disabled, predefined)
            for total, first, second in [(1, 1, 0), (2, 1, 1), (17, 3, 7), (m.FRACTION_MAX, m.FRACTION_MAX, 0)]:
                add(mode, 3, length, total, first, second)
            if mode == 2 or length == 0:
                add(mode, 3, length, 0, 0, 0)
        for length in neighbors([*m.LL_START, m.BLOCK], 1, m.BLOCK):
            add(mode, 4, length, 0, 0, 0, 1)
            if m.code(length - 1) == m.code(length):
                add(mode, 4, length, 17, 3, 3)
            else:
                add(mode, 4, length, 17, 1, 15)
                add(mode, 4, length, 17, 15, 1)
    return tuple(rows)


def manifest():
    rows = recipes()
    return dict(schema_version=1, revision=m.REVISION, packet_sha256=m.sha(m.packet(rows)),
                rows=[dict(index=i, mode=r[0], tag=r[1], words=list(r)) for i, r in enumerate(rows)],
                qualification='Source-only synthetic scalar boundary inputs. No C observations or expected results are used to select requests.')

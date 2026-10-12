#!/usr/bin/env python3
"""Broader source-only Greedy recipes; no parser is used to construct expectations."""
from dataclasses import replace
from .greedy_direct import Recipe, key_hash, require

SEED = 0x5eed_6eed_2026_1012
FRESH_SALT = 0x835892b4bb2cae74
UNAVAILABLE = (0xfffffffd, 0xfffffffe, 0xffffffff)


def noise(size, seed=SEED):
    result = bytearray()
    state = seed
    for _ in range(size):
        state ^= (state << 13) & 0xffffffffffffffff
        state ^= state >> 7
        state ^= (state << 17) & 0xffffffffffffffff
        result.append(state & 255)
    return bytes(result)


def collision_keys(bits, salt):
    """Find distinct four-byte keys sharing a checked hash, not a match result."""
    first = bytes(4)
    target = key_hash(first, bits, salt)
    for word in range(1, 1 << 20):
        candidate = word.to_bytes(4, 'little')
        if key_hash(candidate, bits, salt) == target:
            return first, candidate
    raise ValueError('bounded collision search exhausted')


def row_wrap(count, search_log, name, key=b'abcd', salt=FRESH_SALT):
    pieces = [key + i.to_bytes(4, 'little') + noise(12, SEED ^ (i + 1)) for i in range(count)]
    prefix = b''.join(pieces)
    block = key + b'\xff' * 4 + noise(56, SEED ^ 0xabc)
    return Recipe(name, prefix + block, branch=1, prefix=len(prefix), window_log=12,
                  hash_log=8, search_log=search_log, next=len(prefix),
                  repeats=UNAVAILABLE, seeds=tuple(20 * i for i in range(count)), salt=salt)


def recipes():
    result = []
    # Effective width and row-width/search-budget dispatch use common sources.
    text = noise(512) + b'abcdefgh01234567' * 32 + noise(256, SEED ^ 1)
    for branch, label in ((0, 'chain'), (1, 'row')):
        for minimum in (3, 4, 5, 6, 7):
            result.append(Recipe(f'{label}-width-{minimum}', text, branch=branch, window_log=11,
                                 hash_log=10, chain_log=10, min_match=minimum,
                                 salt=FRESH_SALT if branch else 0))
        for h, c in ((6, 6), (7, 6), (6, 7), (18, 6), (6, 18), (19, 18)):
            result.append(Recipe(f'{label}-layout-h{h}-c{c}', text, branch=branch, window_log=11,
                                 hash_log=h, chain_log=c, salt=FRESH_SALT if branch else 0))
        for w in (14, 15, 21):
            result.append(Recipe(f'{label}-explicit-window-{w}', text, branch=branch, window_log=w,
                                 hash_log=8, salt=FRESH_SALT if branch else 0))
        for size in (63, 64, 65, 127, 128, 129):
            prefix = noise(size, SEED ^ size)
            result.append(Recipe(f'{label}-cyclic-prefix-{size}', prefix + prefix[-32:] * 2,
                                 branch=branch, prefix=size, window_log=12, next=size,
                                 seeds=tuple(range(size - 3)), repeats=UNAVAILABLE,
                                 salt=FRESH_SALT if branch else 0))
        for gap in (383, 384, 385, 575, 576, 577):
            prefix = noise(gap, SEED ^ 2)
            block = noise(17 if branch else 9, SEED ^ 3)
            result.append(Recipe(f'{label}-pending-gap-{gap}', prefix + block, branch=branch,
                                 prefix=gap, window_log=12, hash_log=10, chain_log=10,
                                 repeats=UNAVAILABLE, salt=FRESH_SALT if branch else 0))
        for size in (2047, 2048, 2049, 2055, 2064, 4096):
            result.append(Recipe(f'{label}-skip-tail-{size}', noise(size), branch=branch,
                                 window_log=13, hash_log=12, chain_log=12, repeats=UNAVAILABLE,
                                 salt=FRESH_SALT if branch else 0))
        result.append(Recipe(f'{label}-skip-then-period', noise(4096) + b'abcd' * 64,
                             branch=branch, window_log=13, hash_log=12, chain_log=12,
                             repeats=UNAVAILABLE, salt=FRESH_SALT if branch else 0))
        for distance in (1023, 1024, 1025):
            key = b'0123456789abcdefghijklmnopqrstuv'
            prefix = key + noise(distance - len(key), SEED ^ 4)
            result.append(Recipe(f'{label}-moving-floor-{distance}', prefix + key,
                                 branch=branch, prefix=len(prefix), window_log=10, next=len(prefix),
                                 repeats=UNAVAILABLE, seeds=(0,), salt=FRESH_SALT if branch else 0))
        for floor in (0, 4):
            prefix = b'abcdefghijklmnopqrstuvwxyz012345'
            result.append(Recipe(f'{label}-backward-floor-{floor}', prefix * 3, branch=branch,
                                 prefix=len(prefix), next=len(prefix), floor=floor, seeds=(4,),
                                 repeats=UNAVAILABLE, salt=FRESH_SALT if branch else 0))
        for period in (1, 2, 3, 4, 7, 8):
            pattern = bytes(range(65, 65 + period))
            result.append(Recipe(f'{label}-repeat-period-{period}', pattern * 128, branch=branch,
                                 window_log=12, salt=FRESH_SALT if branch else 0))
        for size in (65535, 65536, 65537, 65541, 65542, 65543, 65544, 131071, 131072):
            result.append(Recipe(f'{label}-long-match-{size}', b'abcd' * (size // 4) + b'abcd'[:size % 4],
                                 branch=branch, window_log=17, hash_log=8, chain_log=8,
                                 salt=FRESH_SALT if branch else 0))
        for literal_prefix in (65500, 65535, 65536):
            result.append(Recipe(f'{label}-long-literals-{literal_prefix}',
                                 noise(literal_prefix) + b'abcd' * 1024, branch=branch,
                                 window_log=17, hash_log=12, chain_log=12,
                                 repeats=UNAVAILABLE, salt=FRESH_SALT if branch else 0))
        # At the 2-MiB window boundary a valid head may predate chain capacity.
        prefix = b'Far history remains a valid first hash head even outside its chain ring.\n'
        block = prefix[:64]
        prefix += noise((2 << 20) - len(prefix), SEED ^ 5)
        result.append(Recipe(f'{label}-maximum-history-head', prefix + block, branch=branch,
                             prefix=len(prefix), next=len(prefix), window_log=21, hash_log=19,
                             chain_log=18, seeds=(0,), repeats=UNAVAILABLE,
                             salt=FRESH_SALT if branch else 0))
    for log, counts in ((4, (15, 16, 17)), (5, (31, 32, 33)), (6, (63, 64, 65))):
        for count in counts:
            result.append(row_wrap(count, log, f'row-width-{1 << log}-insertions-{count}'))
    result.append(row_wrap(17, 4, 'row-zero-tag-and-sentinel', key=bytes(4), salt=0))
    row = row_wrap(33, 5, 'row-capped-search-5')
    # Search log 6 switches to 64-cell rows; logs 7/29 retain that same layout.
    for log in (5, 6, 7, 29):
        result.append(replace(row, name=f'row-capped-search-{log}', search_log=log))
    for log in (1, 2, 4, 8):
        prefix = b''.join(b'abcd' + bytes([i]) + noise(15, SEED ^ i) for i in range(12))
        result.append(Recipe(f'chain-tie-budget-{1 << log}', prefix + b'abcdZ' + noise(59),
                             prefix=len(prefix), next=len(prefix), seeds=tuple(20 * i for i in range(12)),
                             repeats=UNAVAILABLE, window_log=12, hash_log=6, chain_log=8, search_log=log))
    a, b = collision_keys(6, 0)
    prefix = a + noise(60, SEED ^ 6) + b + noise(60, SEED ^ 7)
    for c in (6, 7):
        result.append(Recipe(f'chain-distinct-keys-c{c}', prefix + prefix[:32], prefix=128,
                             next=128, seeds=(0, 64), repeats=UNAVAILABLE, window_log=12,
                             hash_log=6, chain_log=c, search_log=8))
    for salt, name in ((0, 'zero'), (FRESH_SALT, 'fresh')):
        a, b = collision_keys(10, salt)
        prefix = a + noise(28, SEED ^ 8) + b + noise(28, SEED ^ 9)
        result.append(Recipe(f'row-distinct-keys-{name}-salt', prefix + prefix[:32], branch=1,
                             prefix=64, next=64, seeds=(0, 32), repeats=UNAVAILABLE,
                             hash_log=6, search_log=4, salt=salt))
    result.append(replace(next(c for c in result if c.name == 'row-skip-tail-4096'),
                          name='row-entropy-wrap', entropy=0xfffffff0))
    require(len(result) == len({c.name for c in result}), 'unique broader recipe names')
    for case in result:
        case.packet()
    return result

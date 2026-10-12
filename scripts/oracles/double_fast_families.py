"""Source-only DoubleFast hypotheses; no parser or expected-output generator."""

from dataclasses import dataclass
from pathlib import Path
import hashlib

DIRECT_SHA256 = "5280b94e6e10e34a263b7ae1b31a24026adfd92327767b776e81344362604065"
if hashlib.sha256(Path(__file__).with_name("double_fast_direct.py").read_bytes()).hexdigest() != DIRECT_SHA256:
    raise ValueError("frozen direct recipe/validator identity changed")
from . import double_fast_direct as direct

require = direct.require
Recipe = direct.Recipe
UNAVAILABLE = (4096, 8192, 16384)
FAMILY_COUNTS = {"width-dispatch": 30, "table-shape": 20, "tail": 34,
                 "sampling-tail": 25, "sampling-plant": 10, "conditional-store": 2,
                 "lookahead-length": 3, "current-long-priority": 2, "collision": 2,
                 "seed-order": 3, "repeat-transition": 12, "floor": 11,
                 "long-marker": 12, "envelope": 2}


@dataclass(frozen=True)
class Case:
    recipe: Recipe
    family: str
    hypothesis: str

    def metadata(self):
        return self.recipe.metadata() | {"family": self.family, "hypothesis": self.hypothesis}


def noise(size, seed=0x31415926):
    output = bytearray()
    for _ in range(size):
        seed = (seed * 1664525 + 1013904223) & 0xffffffff
        output.append(((seed >> 24) ^ (seed >> 13)) & 255)
    return bytes(output)


def unique_words():
    """Base16 order4 de Bruijn cycle, with an independent uniqueness check."""
    work, output = [0] * 64, bytearray()

    def visit(depth, period):
        if depth > 4:
            if 4 % period == 0:
                output.extend(work[1:period + 1])
        else:
            work[depth] = work[depth - period]
            visit(depth + 1, period)
            for symbol in range(work[depth - period] + 1, 16):
                work[depth] = symbol
                visit(depth + 1, depth)

    visit(1, 1)
    cycle = bytes(output)
    require(len(cycle) == 65536, "de Bruijn extent")
    wrapped = cycle + cycle[:3]
    require(len({wrapped[i:i + 4] for i in range(65536)}) == 65536, "cyclic four-byte uniqueness")
    return cycle


def adjacent_collision():
    for attempt in range(65536):
        word = (0x17293b4d5f718395 + attempt * 0x9e3779b97f4a7c15) & 0xffffffffffffffff
        value = word.to_bytes(8, "little") + bytes([attempt & 255])
        if value[:8] != value[1:] and direct.bucket(value[:8], 6) == direct.bucket(value[1:], 6):
            return value
    raise ValueError("bounded adjacent long-key collision unavailable")


def conditional_input(unique, position, step):
    prefix = bytes(range(224, 256))
    for start in range(0, 4096, 64):
        block = bytearray(unique[start:start + position + step + 8])
        block[position:position + 4] = prefix[:4]
        require(block[position + 4] != prefix[4], "short-only match continuation")
        source = prefix + block
        target = len(prefix) + position + step
        selected = direct.bucket(source[target:target + 8], 18)
        possible_prior = range(len(prefix), len(prefix) + position + 3)
        if all(direct.bucket(source[p:p + 8], 18) != selected for p in possible_prior):
            return source
    raise ValueError("bounded isolated deferred bucket unavailable")


def recipes():
    result = []

    def add(family, hypothesis, recipe):
        recipe.packet()
        result.append(Case(recipe, family, hypothesis))

    unique = unique_words()
    sources = {"period": b"0123456789ABCDEF" * 16,
               "split": b"".join(b"abcd" + bytes([17 + i, 37 + i, 83 + i, 121 + i]) + b"01234567" for i in range(16)),
               "asymmetric": noise(192) + noise(192)[32:80] + noise(15, 7)}
    for label, source in sources.items():
        for width in range(3, 8):
            for window in (18, 21):
                add("width-dispatch", "Compare selected short widths and equal effective bounds at windows18/21.",
                    Recipe(f"width-{label}-m{width}-w{window}", source, min_match=width, window_log=window))
    for long_log, short_log in ((6, 9), (9, 6), (10, 18), (18, 10), (18, 18)):
        for width in range(4, 8):
            add("table-shape", "Observe independent table dimensions with the same source.",
                Recipe(f"tables-h{long_log}-c{short_log}-m{width}", noise(512) + noise(512)[17:193],
                       long_log=long_log, short_log=short_log, min_match=width, window_log=20))
    for size in (*range(7, 18), 23, 24, 25, 31, 32, 33):
        add("tail", "Fresh search tail boundary.", Recipe(f"fresh-tail-{size}", noise(size)))
        prefix = bytes(range(32, 96))
        add("tail", "Seeded prefix changes entry-position availability.",
            Recipe(f"prefix-tail-{size}", prefix + prefix[:size], prefix=64, repeats=UNAVAILABLE,
                   long_seeds=(0,), short_seeds=(0,)))
    for size in (*range(253, 261), *range(509, 517), *range(765, 774)):
        add("sampling-tail", "Unique four-byte words isolate no-match table sampling around growth thresholds.",
            Recipe(f"sampling-tail-{size}", unique[4096:4096 + size]))
    prefix = bytes(range(224, 256))
    for position in (255, 256, 257, 511, 512, 513, 767, 768, 770, 771):
        block = bytearray(unique[4096:5196])
        block[position:position + 16] = prefix[:16]
        add("sampling-plant", "Planted high-byte prefix key; sampled outcome remains unobserved.",
            Recipe(f"sampling-plant-{position}", prefix + block, prefix=32, window_log=11,
                   long_log=18, short_log=18, repeats=UNAVAILABLE, long_seeds=(0,), short_seeds=(0,)))
    for position, step in ((512, 3), (770, 4)):
        add("conditional-store", "Isolated deferred bucket and short-only match hypothesize the step3/4 store distinction.",
            Recipe(f"conditional-store-step{step}", conditional_input(unique, position, step), prefix=32,
                   long_log=18, short_log=18, repeats=UNAVAILABLE, short_seeds=(0,)))
    block = bytes(range(64, 104))
    require(direct.bucket(block[:8], 9) != direct.bucket(block[1:9], 9), "isolated lookahead hashes")
    for length in (11, 12, 13):
        prefix = bytearray([200] * 64)
        prefix[:12] = block[:12]
        prefix[12] = block[12] ^ 255
        prefix[32:32 + length] = block[1:1 + length]
        prefix[32 + length] = block[1 + length] ^ 255
        prefix[31] = block[0] ^ 255
        add("lookahead-length", "Short length12 versus later long length11/12/13; no backward equality.",
            Recipe(f"lookahead-length-{length}", bytes(prefix) + block, prefix=64, long_log=9, short_log=9,
                   repeats=UNAVAILABLE, long_seeds=(32,), short_seeds=(0,)))
    prefix = bytearray([200] * 64)
    prefix[:16] = block[:16]
    prefix[16] = block[16] ^ 255
    prefix[32:52] = block[:20]
    prefix[52] = block[20] ^ 255
    for label, seeds in (("seeded", (0,)), ("empty", ())):
        add("current-long-priority", "Current long length16 competes with current short length20.",
            Recipe(f"current-long-priority-{label}", bytes(prefix) + block, prefix=64,
                   repeats=UNAVAILABLE, long_seeds=seeds, short_seeds=(32,)))
    nine = adjacent_collision()
    prefix = bytearray(noise(48, 73))
    prefix[1:9] = nine[1:]
    block = bytearray(nine + noise(23, 71))
    for label in ("colliding", "control"):
        if label == "control":
            for byte in range(256):
                block[0] = byte
                if direct.bucket(block[:8], 6) != direct.bucket(block[1:9], 6):
                    break
            else:
                raise ValueError("noncolliding first-byte control unavailable")
        prefix[16:20] = block[:4]
        prefix[20] = block[4] ^ 255
        add("collision", "Later long candidate is above floor; adjacent current write may replace its bucket.",
            Recipe(f"adjacent-long-{label}", bytes(prefix + block), prefix=48, repeats=UNAVAILABLE,
                   long_seeds=(1,), short_seeds=(16,)))
    prefix = b"ABCDEFGH01234567ABCDEFGHqrstuvwx" + noise(16, 21)
    block = b"ABCDEFGH01234567" + noise(16, 29)
    for label, seeds in (("repeat-first", (0, 16, 0)), ("last-second", (0, 16)), ("last-first", (16, 0))):
        add("seed-order", "Equal prefix keys with distinct continuations preserve ordered initial overwrites.",
            Recipe(f"seed-order-{label}", prefix + block, prefix=48, repeats=UNAVAILABLE,
                   long_seeds=seeds, short_seeds=seeds))
    a, b = b"ABCDEFGHIJKLMNO!", b"qrstuvwxyz01234?"
    prefix = a + b + noise(64, 55)
    blocks = {"none": noise(96, 44), "one": a * 4 + noise(32, 88), "alternating": (a + b) * 3,
              "two": a * 2 + b * 2 + noise(32, 17), "loop": (a + b) * 2 + prefix[64:96] * 2}
    for label, block in blocks.items():
        for availability, repeats in (("available", (32, 16, 8)), ("disabled", UNAVAILABLE)):
            add("repeat-transition", "Classify observed explicit offsets, immediate repeats and saved-offset restoration.",
                Recipe(f"repeats-{label}-{availability}", prefix + block, prefix=96, repeats=repeats,
                       long_seeds=(0, 16), short_seeds=(0, 16)))
    add("repeat-transition", "Single candidate followed by different data may expose saved-first restoration.",
        Recipe("repeats-single-disabled", prefix + a + noise(80, 123), prefix=96, repeats=UNAVAILABLE,
               long_seeds=(0, 16), short_seeds=(0, 16)))
    add("repeat-transition", "Duplicate initial repeat distances remain positive wire cells.",
        Recipe("repeats-duplicate", b"abcd" * 48, prefix=32, repeats=(4, 4, 8), long_seeds=(0, 4), short_seeds=(0, 4)))
    for position in (539, 540, 541):
        prefix, block = bytearray(noise(1500, 83)), bytearray(noise(64, 79))
        block[:16] = prefix[position:position + 16]
        for role in ("long", "short"):
            add("floor", "Candidate around ordinary block-end floor540; equality differs from later strict probe.",
                Recipe(f"floor-{role}-{position}", bytes(prefix + block), prefix=1500, repeats=UNAVAILABLE,
                       long_seeds=(position,) if role == "long" else (), short_seeds=(position,) if role == "short" else ()))
    for distance in (1024, 1025):
        prefix, block = bytearray(noise(1500, 83)), bytearray(noise(64, 79))
        for index in range(3, 32):
            block[index] = prefix[1500 + index - distance]
        add("floor", "Repeat entry availability uses window1024 independently of the ordinary end floor.",
            Recipe(f"repeat-bound-{distance}", bytes(prefix + block), prefix=1500, repeats=(distance, 4096, 8192)))
    for floor in (0, 16, 32):
        add("floor", "Explicit retained-prefix floor can expire seeds and initial repeats independently.",
            Recipe(f"retained-floor-{floor}", b"abcd" * 40, prefix=64, floor=floor, repeats=(32, 64, 128),
                   long_seeds=(0, 16, 32), short_seeds=(0, 16, 32)))
    for size in (*range(65534, 65543), 65700, 131072):
        add("long-marker", "Observe raw match-base overflow and marker position, rather than inferring from source size.",
            Recipe(f"long-match-{size}", b"A" * size, window_log=17))
    add("long-marker", "Unique literal candidate followed by high-byte matching tail; inspect actual LL marker.",
        Recipe("long-literal-candidate", b"A" * 64 + unique + b"A" * 128, prefix=64, window_log=17,
               long_log=18, short_log=18, repeats=UNAVAILABLE, long_seeds=(0,), short_seeds=(0,)))
    prefix_size = 2 << 20
    add("envelope", "Maximum prefix with seven-byte cleanup and both old/retained prefix seed keys.",
        Recipe("maximum-prefix-cleanup", b"A" * (prefix_size + 7), prefix=prefix_size, window_log=21,
               floor=prefix_size - 16, long_seeds=(0, prefix_size - 8), short_seeds=(0, prefix_size - 4)))
    add("envelope", "Maximum prefix/block/table envelope with overlap; this is a test-helper bound.",
        Recipe("maximum-envelope-match", b"A" * (prefix_size + (128 << 10)), prefix=prefix_size,
               window_log=21, long_log=18, short_log=18, long_seeds=(0, prefix_size - 8), short_seeds=(0, prefix_size - 4)))
    require(len(result) == len({case.recipe.name for case in result}) == 168, "unique recipe cardinality")
    counts = {family: sum(case.family == family for case in result) for family in FAMILY_COUNTS}
    require(direct.exact(counts, FAMILY_COUNTS), "complete family decomposition")
    return result

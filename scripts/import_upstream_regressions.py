#!/usr/bin/env python3
"""Extract exact inline binary regressions from the pinned zstd v1.5.7 tests."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "tests" / "upstream" / "inline-regressions"
SOURCES = ["tests/legacy.c", "tests/invalidDictionaries.c", "tests/fuzzer.c"]
STRING_TOKEN = re.compile(r'"(?:[^"\\]|\\.)*"', re.DOTALL)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def decode_c_string(token):
    """Evaluate C byte escapes without Python eval or a C compiler."""
    text = token[1:-1]
    output = bytearray()
    cursor = 0
    named = {"a": 7, "b": 8, "f": 12, "n": 10, "r": 13, "t": 9, "v": 11,
             "\\": 92, "'": 39, '"': 34, "?": 63}
    while cursor < len(text):
        char = text[cursor]
        cursor += 1
        if char != "\\":
            output.extend(char.encode("utf-8"))
            continue
        escape = text[cursor]
        cursor += 1
        if escape == "x":
            digits = re.match(r"[0-9a-fA-F]+", text[cursor:])
            if not digits:
                raise ValueError("empty C hexadecimal escape")
            value = int(digits.group(), 16)
            if value > 255:
                raise ValueError("C hexadecimal escape exceeds one byte")
            output.append(value)
            cursor += len(digits.group())
        elif escape in "01234567":
            digits = re.match(r"[0-7]{0,2}", text[cursor:]).group()
            output.append(int(escape + digits, 8))
            cursor += len(digits)
        elif escape in named:
            output.append(named[escape])
        else:
            raise ValueError(f"unsupported C escape: {escape!r}")
    return bytes(output)


def declaration(text, symbol):
    match = re.search(r"\b" + re.escape(symbol) + r"\s*(?:\[[^]]*\])?\s*=\s*(.*?);", text, re.DOTALL)
    if match is None:
        raise ValueError(f"missing declaration: {symbol}")
    initializer = match.group(1)
    source_lines = [text.count("\n", 0, match.start()) + 1, text.count("\n", 0, match.end()) + 1]
    if initializer.lstrip().startswith("{"):
        numbers = initializer.strip()[1:-1].split(",")
        data = bytes(int(number.strip(), 0) for number in numbers if number.strip())
        metadata = {"syntax": "C byte-array initializer", "source_initializer": initializer}
    else:
        literals = STRING_TOKEN.findall(initializer)
        if STRING_TOKEN.sub("", initializer).strip():
            raise ValueError(f"unexpected tokens in string initializer: {symbol}")
        data = b"".join(map(decode_c_string, literals))
        metadata = {"syntax": "C adjacent string literals", "source_literals": literals,
                    "implicit_trailing_nul_included": False}
    return data, {"symbol": symbol, "source_lines": source_lines, **metadata}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream", type=Path, help="external pinned zstd git checkout")
    parser.add_argument("--zstd", default=os.environ.get("ZSTD_BIN", "zstd"))
    parser.add_argument("--check", action="store_true", help="verify generated assets without writing")
    args = parser.parse_args()
    revision = subprocess.check_output(["git", "-C", str(args.upstream), "rev-parse", "HEAD"], text=True).strip()
    if revision != REVISION:
        parser.error(f"expected {REVISION}; found {revision}")
    subprocess.run(["git", "-C", str(args.upstream), "diff", "--quiet", "HEAD", "--", *SOURCES], check=True)
    version = subprocess.check_output([args.zstd, "--version"], text=True)
    if "v1.5.7" not in version:
        parser.error(f"expected zstd v1.5.7; found {version.strip()}")
    sources = {path: (args.upstream / path).read_bytes() for path in SOURCES}
    texts = {path: data.decode("utf-8") for path, data in sources.items()}
    outputs = {}
    manifest = {"upstream_revision": REVISION,
                "source_sha256": {path: sha256(data) for path, data in sources.items()},
                "fixtures": []}

    def record(path, data, **metadata):
        outputs[path] = data
        manifest["fixtures"].append({"path": path, "size": len(data), "sha256": sha256(data), **metadata})

    def extract(source, symbol, path, status):
        data, metadata = declaration(texts[source], symbol)
        record(path, data, source=source, status=status, **metadata)
        return data

    def oracle(data):
        return subprocess.check_output([args.zstd, "-q", "-d", "-c"], input=data)

    legacy = extract("tests/legacy.c", "COMPRESSED", "legacy-all.zst", "blocked: legacy v0.4-v0.7 decoders")
    expected = extract("tests/legacy.c", "EXPECTED", "legacy-all.raw", "expected bytes for five concatenated frames")
    if len(legacy) != 917 or len(expected) % 5:
        raise RuntimeError("unexpected legacy fixture dimensions")
    if oracle(legacy) != expected:
        raise RuntimeError("official legacy concatenation output mismatch")
    expected_frame = expected[:len(expected) // 5]
    if expected != expected_frame * 5:
        raise RuntimeError("legacy expectation is not five identical outputs")
    offsets = [legacy.index(bytes([magic, 0xB5, 0x2F, 0xFD])) for magic in range(0x24, 0x29)]
    offsets.append(len(legacy))
    for index, (start, end) in enumerate(zip(offsets, offsets[1:])):
        version_name = ["v0.4.3", "v0.5.0", "v0.6.0", "v0.7.0", "v0.8.0"][index]
        encoded = legacy[start:end]
        if oracle(encoded) != expected_frame:
            raise RuntimeError(f"independent legacy component mismatch: {version_name}")
        record(
            f"legacy-{version_name}.zst", encoded, derived_from="legacy-all.zst", byte_range=[start, end],
            original_encoder_version=version_name, expected_output="legacy-frame.raw",
            status="active candidate: current standard frame" if index == 4 else "blocked: legacy decoder",
        )
    record("legacy-frame.raw", expected_frame, derived_from="legacy-all.raw", byte_range=[0, len(expected_frame)])
    extract("tests/invalidDictionaries.c", "invalidRepCode", "invalid-repeat-code.dict", "blocked: dictionary construction APIs")
    extract("tests/fuzzer.c", "dictBufferFixed", "fixed-training.dict", "blocked: dictionary training and use APIs")
    skippable_bad = extract("tests/fuzzer.c", "skippable8", "skippable-size-minus-eight.zst", "active candidate: reject truncated skippable frame")
    result = subprocess.run([args.zstd, "-q", "-d", "-c"], input=skippable_bad, capture_output=True)
    if result.returncode == 0:
        raise RuntimeError("official CLI accepted malformed skippable frame")
    skippable = extract("tests/fuzzer.c", "skippableFrame", "skippable-variant-two.zst", "active candidate: skip five payload bytes")
    if oracle(skippable) != b"":
        raise RuntimeError("official CLI produced output for skippable frame")
    # The dictionary-header-only input is an inline function argument rather
    # than a named array, but it is still a complete hard-coded regression input.
    token = '"\\x37\\xa4\\x30\\xec\\x11\\x22\\x33\\x44"'
    text = texts["tests/fuzzer.c"]
    if text.count(token) != 1:
        raise RuntimeError("dictionary header literal no longer unique")
    line = text.count("\n", 0, text.index(token)) + 1
    record("header-only.dict", decode_c_string(token), source="tests/fuzzer.c", source_lines=[line, line],
           syntax="C string literal function argument", source_literals=[token],
           implicit_trailing_nul_included=False, status="blocked: dictionary corruption API")
    outputs["manifest.json"] = (json.dumps(manifest, indent=2, ensure_ascii=False) + "\n").encode()
    for relative, data in outputs.items():
        path = DESTINATION / relative
        if args.check:
            if path.read_bytes() != data:
                raise RuntimeError(f"fixture mismatch: {relative}")
        else:
            path.write_bytes(data)
    print(f"{'Verified' if args.check else 'Imported'} {len(manifest['fixtures'])} exact or derived inline fixtures")


if __name__ == "__main__":
    main()

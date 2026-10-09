#!/usr/bin/env python3
"""Bounded, bidirectional interoperability for the public reference Fast APIs.

No C byte-identity claim is made: only Rust safe/all equality and exact decoded
source equality are required. --check-plan is entirely offline and runs no child.
"""
from pathlib import Path
import argparse
import datetime
import hashlib
import json
import os
import shutil
import struct
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
TOOLCHAIN = "nightly-2026-10-08"
MAX_SOURCE = 1_048_577
MAX_FRAME = 2 << 20
MAX_RESPONSE = 44 + MAX_FRAME
MAX_TOTAL_SOURCE = 32 << 20
OWNED = ("examples/reference_fast_oracle.rs", "scripts/check_reference_fast_interop.py",
         "scripts/oracles/reference_fast_frames.c")
FIELDS = ("window", "chain", "hash", "search", "minimum", "target", "strategy")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def file_sha(path):
    return sha(Path(path).read_bytes())


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def recipes():
    result = []

    def add(name, size, pattern="periodic", level=1, expected="success", hint=0, **overrides):
        require(not (set(overrides) - set(FIELDS)), "unknown recipe override")
        result.append(dict(id=name, size=size, pattern=pattern, seed=0x12345678,
                           level=level, expected=expected, hint=hint,
                           overrides=[overrides.get(key, 0) for key in FIELDS]))

    for size in (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 12, 15, 16, 63, 64, 127, 128,
                 255, 256, 257, 65791, 65792):
        add(f"extent-{size}", size, "uniform" if size % 2 else "periodic")
    for size in (16383, 16384, 16385, 131071, 131072, 131073, 262143, 262144, 262145):
        for level in (1, 2, -3):
            expected = "unsupported" if level == 2 and 131072 < size <= 262144 else "success"
            add(f"tier-{size}-level-{level}", size, "skewed", level, expected)
    for width in range(3, 8):
        add(f"minimum-{width}", 65536, "key-pattern", minimum=width)
    for target in (0, 1, 2, 3, 4, 7, 131072):
        add(f"acceleration-{target}", 65536, "skewed", target=target)
    for log in (6, 10, 14, 18):
        add(f"hash-{log}", 262145, "periodic", hash=log, minimum=7)
    add("chain-search-request", 65536, "key-pattern", chain=16, search=5, minimum=6, target=2)
    for hint in (1, 16384, 2147483647):
        add(f"known-source-hint-{hint}", 32768, "skewed", hint=hint)
    for window in (17, 19):
        add(f"aba-window-{window}", 393216, "aba", window=window, hash=16, minimum=4)
    add("small-window", 1025, "periodic", window=10, hash=6, minimum=4)
    add("mixed-blocks", 524288, "mixed", window=20, hash=16, minimum=4)
    add("raw-then-copy", 262144, "raw-copy", window=19, hash=16, minimum=4)
    add("long-match", 131072, "uniform", window=17, minimum=4)
    add("long-literals", 69632, "long-literals", window=17, hash=16, minimum=4)
    for level in (-1, -7, -131072, -2147483648):
        add(f"negative-{level}", 16385, "skewed", level)
    for level in (0, 3, 22):
        add(f"unsupported-level-{level}", 65, level=level, expected="unsupported")
    for strategy in (2, 3, 5, 9):
        add(f"unsupported-strategy-{strategy}", 4096, strategy=strategy, expected="unsupported")
    add("resource-hash-19", 262144, hash=19, expected="resource")
    add("resource-window-21", MAX_SOURCE, window=21, hash=16, expected="resource")
    add("strategy-before-resource", MAX_SOURCE, window=21, hash=19, strategy=2, expected="unsupported")
    # Pinned clevels.h:29 / presets.tsv tier0 row1: level1 uses window19.
    # A larger known source does not increase that preset to window21.
    add("default-window-large-source", MAX_SOURCE)
    add("explicit-fast-level-22", 65536, "key-pattern", 22, strategy=1, window=17,
        hash=14, minimum=4, target=7)
    require(len(result) == 92 and len({r["id"] for r in result}) == 92, "exact unique recipe count")
    require(all(0 <= r["size"] <= MAX_SOURCE for r in result), "source size bound")
    require(sum(r["size"] for r in result) <= MAX_TOTAL_SOURCE, "aggregate source bound")
    return result


def noise(seed, size):
    output = bytearray(size)
    for index in range(size):
        seed ^= (seed << 13) & 0xffffffff
        seed ^= seed >> 17
        seed ^= (seed << 5) & 0xffffffff
        output[index] = seed & 255
    return bytes(output)


def source(recipe):
    size, pattern, seed = recipe["size"], recipe["pattern"], recipe["seed"]
    phrase = b"Reference Fast: repeated source text and literal entropy.\n"
    periodic = lambda n: (phrase * ((n + len(phrase) - 1) // len(phrase)))[:n]
    if pattern == "periodic":
        result = periodic(size)
    elif pattern == "uniform":
        result = b"x" * size
    elif pattern == "key-pattern":
        word = b"abcdEFGHabcdIJKLabcdMNOPabcdQRST"
        result = (word * ((size + len(word) - 1) // len(word)))[:size]
    elif pattern == "skewed":
        random = noise(seed, size)
        result = bytes(value if index % 23 == 0 else ord('a') + index % 3
                       for index, value in enumerate(random))
    elif pattern == "aba":
        first = noise(seed, 131072)
        result = first + noise(0x10203040, 131072) + first
    elif pattern == "raw-copy":
        first = noise(seed, 131072)
        result = first + first
    elif pattern == "mixed":
        first = periodic(131072)
        result = first + noise(seed, 131072) + b"z" * 131072 + first
    elif pattern == "long-literals":
        first = noise(seed, 65536)
        result = first + first[:4096]
    else:
        raise ValueError("unknown source recipe")
    require(len(result) == size, "recipe source extent")
    return result


def packet(recipe, data, frame=b"", mode=0):
    require(len(data) <= MAX_SOURCE and len(frame) <= MAX_FRAME, "packet bounds")
    words = [mode, len(data), len(frame), recipe["level"] & 0xffffffff,
             *recipe["overrides"], recipe["hint"]]
    return b"ZFOI0001" + struct.pack("<12I", *words) + data + frame


def response(data, magic):
    require(44 <= len(data) <= MAX_RESPONSE and data[:8] == magic, "bounded response magic")
    values = struct.unpack("<9I", data[8:44])
    require(values[0] in (0, 1, 2) and values[8] <= MAX_FRAME and len(data) == 44 + values[8],
            "response extent/status")
    require(values[0] == 0 or values[8] == 0, "rejected request returned output")
    return values[0], list(values[1:8]), data[44:]


def frame_header(frame, size):
    require(len(frame) >= 6 and frame[:4] == b"\x28\xb5\x2f\xfd", "standard frame magic")
    descriptor = frame[4]
    require(descriptor & 0x1c == 0, "reserved/unused/checksum frame flags")
    cursor = 5
    single = bool(descriptor & 32)
    if single:
        window = None
    else:
        require(cursor < len(frame), "window descriptor extent")
        wd = frame[cursor]
        cursor += 1
        base = 1 << (10 + (wd >> 3))
        window = base + (base >> 3) * (wd & 7)
    dict_width = (0, 1, 2, 4)[descriptor & 3]
    require(cursor + dict_width <= len(frame), "dictionary extent")
    dictionary = int.from_bytes(frame[cursor:cursor + dict_width], "little")
    cursor += dict_width
    content_width = (1 if single else 0, 2, 4, 8)[descriptor >> 6]
    require(content_width and cursor + content_width <= len(frame), "known content size extent")
    content = int.from_bytes(frame[cursor:cursor + content_width], "little")
    if content_width == 2:
        content += 256
    cursor += content_width
    require(content == size and dictionary == 0, "known source / dictionary metadata")
    if single:
        window = content
    kinds = []
    while True:
        require(cursor + 3 <= len(frame), "block header extent")
        value = int.from_bytes(frame[cursor:cursor + 3], "little")
        cursor += 3
        kind, encoded_size, last = (value >> 1) & 3, value >> 3, bool(value & 1)
        require(kind < 3 and encoded_size <= 131072, "block kind/size")
        stored = 1 if kind == 1 else encoded_size
        require(cursor + stored <= len(frame), "block payload extent")
        kinds.append({"kind": ("raw", "rle", "compressed")[kind], "size": encoded_size, "last": last})
        cursor += stored
        if last:
            break
    require(cursor == len(frame), "single frame exact exhaustion")
    return dict(content_size=content, window_size=window, dictionary_id=dictionary,
                checksum=False, single_segment=single, header_bytes=5 + (not single) + dict_width + content_width,
                blocks=kinds)


def environment():
    selected = {k: os.environ[k] for k in ("PATH", "HOME", "TMPDIR", "RUSTUP_HOME", "CARGO_HOME",
                                         "DEVELOPER_DIR", "SDKROOT") if k in os.environ}
    selected.update(LANG="C", LC_ALL="C", RUSTUP_TOOLCHAIN=TOOLCHAIN,
                    ASAN_OPTIONS="detect_leaks=0:halt_on_error=1", UBSAN_OPTIONS="halt_on_error=1")
    return selected


def source_manifest():
    paths = list((ROOT / "src").rglob("*.rs"))
    paths += [ROOT / p for p in (*OWNED, "Cargo.toml", "Cargo.lock", "rust-toolchain.toml")]
    return {str(p.relative_to(ROOT)): file_sha(p) for p in sorted(set(paths))}


def disjoint(left, right):
    return not (left == right or left.is_relative_to(right) or right.is_relative_to(left))


def execute(args):
    reference = args.upstream.resolve(strict=True)
    output = args.output.resolve()
    require(not output.exists(), "output must not exist")
    for protected in (ROOT, reference, Path(__file__).resolve()):
        require(disjoint(output, protected), "output overlaps protected input")
    require((reference / "lib/libzstd.a").is_file(), "pinned static library is required")
    cc = shutil.which(args.compiler or "cc")
    require(cc is not None, "C compiler unavailable")
    env = environment()
    before = source_manifest()
    output.mkdir(parents=True)
    logs = output / "logs"
    logs.mkdir()
    records = []
    start = time.monotonic()
    result = dict(schema_version=1, scope="Rust safe/all exact frames; bidirectional decoded-byte equality; no C entropy identity",
                  started_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(), source_manifest=before,
                  environment=env, source_limit=MAX_SOURCE, frame_limit=MAX_FRAME, commands=records,
                  sanitizer_scope="Helper only is ASan/UBSan instrumented; the pinned prebuilt static library is not. Guard and byte-equality checks remain active. LeakSanitizer disabled.")
    evidence = output / "evidence.json"

    def save():
        write_json(evidence, result)

    def command(name, argv, data=None, timeout=120, maximum=4 << 20, expected=0):
        require(time.monotonic() - start < 900, "campaign wall-time budget")
        stdout, stderr = logs / (name + ".out"), logs / (name + ".err")
        require(not stdout.exists() and not stderr.exists(), "duplicate command name")
        begun = time.monotonic()
        completed = None
        failure = None
        try:
            with stdout.open("wb") as out, stderr.open("wb") as err:
                completed = subprocess.run([str(x) for x in argv], input=data, stdout=out, stderr=err,
                                           cwd=ROOT, env=env, timeout=timeout, check=False)
        except (subprocess.TimeoutExpired, OSError) as error:
            failure = error
        record = dict(name=name, argv=[str(x) for x in argv], cwd=str(ROOT), environment=env,
                      exit_code=None if completed is None else completed.returncode,
                      exception=None if failure is None else repr(failure),
                      seconds=time.monotonic() - begun, timeout_seconds=timeout,
                      stdin_bytes=0 if data is None else len(data), stdin_sha256=None if data is None else sha(data),
                      stdout=str(stdout.relative_to(output)), stderr=str(stderr.relative_to(output)),
                      stdout_bytes=stdout.stat().st_size, stderr_bytes=stderr.stat().st_size,
                      stdout_sha256=file_sha(stdout), stderr_sha256=file_sha(stderr))
        records.append(record)
        save()
        if failure is not None:
            raise failure
        require(stdout.stat().st_size <= maximum and stderr.stat().st_size <= 4 << 20, "bounded process output")
        require(completed.returncode == expected,
                "command exit: " + name)
        require(before == source_manifest(), "Rust/helper/recipe source changed")
        return stdout.read_bytes()

    def git(*words):
        name = "git-" + str(len(records))
        return command(name, ["git", "-C", reference, *words], maximum=8 << 20)

    require(git("rev-parse", "HEAD").decode().strip() == REVISION, "wrong pinned reference revision")
    require(not git("status", "--porcelain", "--untracked-files=no"), "tracked upstream changes")
    upstream_files = {}
    for relative in git("ls-files", "lib").decode().splitlines():
        p = reference / relative
        if p.suffix in (".c", ".h", ".S", ".mk") or p.name == "Makefile":
            data = p.read_bytes()
            require(data == git("show", f"{REVISION}:{relative}"), "modified reference " + relative)
            upstream_files[relative] = sha(data)
    require({"lib/zstd.h", "lib/compress/zstd_compress.c", "lib/compress/zstd_fast.c"} <= set(upstream_files),
            "missing required reference sources")
    result["upstream"] = dict(path=str(reference), revision=REVISION, source_sha256=upstream_files,
                              library_sha256=file_sha(reference / "lib/libzstd.a"))
    result["compiler"] = dict(path=str(Path(cc).resolve()), sha256=file_sha(Path(cc).resolve()),
                              version=command("cc-version", [cc, "--version"]).decode(),
                              target=command("cc-target", [cc, "-dumpmachine"]).decode())
    command("cc-predefines", [cc, "-dM", "-E", "-x", "c", "/dev/null"])
    result["rustc"] = command("rustc-version", ["rustc", "+" + TOOLCHAIN, "-vV"]).decode()
    compiler_path = command("rustc-path", ["rustup", "which", "--toolchain", TOOLCHAIN, "rustc"]).decode().strip()
    result["rustc_sha256"] = file_sha(compiler_path)
    common = [cc, "-std=c11", "-O2", "-g", "-Wall", "-Wextra", "-Werror", "-I", reference / "lib"]
    helpers = {}
    for mode, flags in (("normal", []), ("sanitized", ["-fsanitize=address,undefined", "-fno-sanitize-recover=all", "-fno-omit-frame-pointer"])):
        executable = output / ("c-" + mode)
        command("build-c-" + mode, common + flags + [ROOT / OWNED[2], reference / "lib/libzstd.a", "-o", executable])
        helpers[mode] = executable
    rust = {}
    for mode, flags in (("safe", []), ("all", ["--all-features"])):
        target = output / ("rust-" + mode)
        command("build-rust-" + mode, ["cargo", "+" + TOOLCHAIN, "build", "--locked", "--offline", "--release",
                                      "--example", "reference_fast_oracle", "--target-dir", target, *flags])
        rust[mode] = target / "release/examples/reference_fast_oracle"
    result["executables"] = {str(p.relative_to(output)): file_sha(p) for p in [*helpers.values(), *rust.values()]}
    cases = recipes()
    write_json(output / "recipes.json", cases)
    result["recipes_sha256"] = file_sha(output / "recipes.json")
    result["cases"] = []
    totals = dict(recipes=len(cases), source_bytes=sum(r["size"] for r in cases), admitted=0,
                  unsupported=0, resource=0, rust_frames_c_decoded=0, c_frames_rust_decoded=0,
                  c_frames=0, decoded_bytes=0)
    statuses = {"success": 0, "unsupported": 1, "resource": 2}
    for index, recipe in enumerate(cases):
        prefix = f"case-{index:03d}-" + recipe["id"]
        data = source(recipe)
        entry = dict(recipe=recipe, source_sha256=sha(data), rust={}, c={})
        encoded = {}
        for mode, executable in rust.items():
            raw = command(prefix + "-rust-encode-" + mode, [executable], packet(recipe, data), timeout=30, maximum=MAX_RESPONSE)
            status, resolved, frame = response(raw, b"ZFOR0001")
            require(status == statuses[recipe["expected"]], "unexpected Rust admission: " + recipe["id"])
            entry["rust"][mode] = dict(status=recipe["expected"], resolved=resolved,
                                        frame_bytes=len(frame), frame_sha256=sha(frame))
            encoded[mode] = (status, resolved, frame)
        require(encoded["safe"] == encoded["all"], "safe/all Rust output differs")
        if recipe["expected"] != "success":
            totals[recipe["expected"]] += 1
            entry["c"] = {"status": "not-run-for-rejected-Rust-profile"}
        else:
            totals["admitted"] += 1
            for mode, (_, resolved, frame) in encoded.items():
                header = frame_header(frame, len(data))
                require(header["window_size"] <= 1 << resolved[0], "Rust window exceeds resolved profile")
                entry["rust"][mode]["header"] = header
            c_frames = []
            for mode, executable in helpers.items():
                for rust_mode, (_, _, frame) in encoded.items():
                    raw = command(prefix + f"-c-{mode}-decode-{rust_mode}", [executable],
                                  packet(recipe, data, frame, 1), timeout=30, maximum=MAX_RESPONSE)
                    status, fields, decoded = response(raw, b"ZFOC0001")
                    require(status == 0 and fields == [0] * 7 and decoded == data, "C did not decode exact Rust source")
                    totals["rust_frames_c_decoded"] += 1
                    totals["decoded_bytes"] += len(data)
                raw = command(prefix + "-c-encode-" + mode, [executable], packet(recipe, data), timeout=30, maximum=MAX_RESPONSE)
                status, fields, frame = response(raw, b"ZFOC0001")
                require(status == 0 and fields == [0] * 7, "C response scalar contract")
                header = frame_header(frame, len(data))
                entry["c"][mode] = dict(frame_bytes=len(frame), frame_sha256=sha(frame), header=header,
                                         parameters="fresh requested setters plus compress2 with exact known input; no applied-scalar claim")
                c_frames.append(frame)
                totals["c_frames"] += 1
                for rust_mode, executable in rust.items():
                    raw = command(prefix + f"-rust-{rust_mode}-decode-c-{mode}", [executable],
                                  packet(recipe, data, frame, 1), timeout=30, maximum=MAX_RESPONSE)
                    status, fields, decoded = response(raw, b"ZFOR0001")
                    require(status == 0 and fields == [0] * 7 and decoded == data, "Rust did not decode exact C source")
                    totals["c_frames_rust_decoded"] += 1
                    totals["decoded_bytes"] += len(data)
            require(c_frames[0] == c_frames[1], "helper instrumentation changed C frames")
        result["cases"].append(entry)
        result["totals"] = totals
        save()
        print(f"{index + 1}/{len(cases)} {recipe['id']} {recipe['expected']}", flush=True)
    # Protocol negatives are independently rejected by every executable. No
    # malformed compressed payload is needed to exercise the bounded loader.
    baseline = packet(cases[1], source(cases[1]))
    bad_magic = b"badmagic" + baseline[8:]
    oversized = bytearray(baseline)
    struct.pack_into("<I", oversized, 12, MAX_SOURCE + 1)
    empty_frame = bytes.fromhex("28b52ffd2000010000")
    concatenated = packet(cases[0], b"", empty_frame + empty_frame, 1)
    skippable_tail = packet(cases[0], b"", empty_frame + bytes.fromhex("502a4d1800000000"), 1)
    negatives = [b"", baseline[:55], baseline[:-1], baseline + b"x", bad_magic, bytes(oversized),
                 concatenated, skippable_tail]
    for mode, executable in {**rust, **helpers}.items():
        for index, malformed in enumerate(negatives):
            command(f"negative-{mode}-{index}", [executable], malformed, timeout=30,
                    maximum=MAX_RESPONSE, expected=2)
    result["negative_rejections"] = len(negatives) * 4
    require(before == source_manifest(), "source changed at completion")
    require(result["upstream"]["library_sha256"] == file_sha(reference / "lib/libzstd.a"), "reference library changed")
    for relative, expected in upstream_files.items():
        require(file_sha(reference / relative) == expected, "reference source changed")
    result.update(result="passed", source_unchanged=True, completed_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  elapsed_seconds=time.monotonic() - start)
    save()
    print(json.dumps(totals, sort_keys=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--compiler")
    parser.add_argument("--check-plan", action="store_true")
    args = parser.parse_args()
    if args.check_plan:
        require(args.upstream is None and args.output is None and args.compiler is None,
                "--check-plan excludes execution arguments")
        cases = recipes()
        print(json.dumps(dict(recipes=len(cases), source_bytes=sum(r["size"] for r in cases),
                              max_source=max(r["size"] for r in cases),
                              statuses={name:sum(r["expected"] == name for r in cases) for name in ("success", "unsupported", "resource")}), sort_keys=True))
    else:
        require(args.upstream is not None and args.output is not None, "--upstream and fresh --output are required")
        execute(args)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"reference Fast interoperability: {error}", file=sys.stderr)
        raise SystemExit(1)

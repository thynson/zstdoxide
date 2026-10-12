#!/usr/bin/env python3
"""Capture fifteen constructed DoubleFast recipes with bounded, retained logs."""

from pathlib import Path
import argparse
import datetime
import json
import os
import selectors
import shutil
import subprocess
import sys
import time
import traceback

from oracles import double_fast_direct as oracle

ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / "scripts/oracles/double_fast_direct.c"
CATALOG = ROOT / "tests/upstream/manifest.json"
ARCHIVE_SHA256 = "2a6a6b22f5c98389d4c3cac3ab65ac77386ae529aa6fcdb8dbdbe267210a640d"
require = oracle.require


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def write_json(path, value):
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n")


def disjoint(left, right):
    return not left.is_relative_to(right) and not right.is_relative_to(left)


def run_command(argv, output, label, env, timeout=120, cap=oracle.MAX_RECORDS):
    """Drain both pipes with a hard per-stream byte cap and a bounded deadline."""
    row = {"label": label, "argv": argv, "cwd": str(output), "environment": env,
           "started_at": utc(), "timeout_seconds": timeout, "per_stream_cap": cap}
    paths = {stream: output / (label + "." + stream) for stream in ("stdout", "stderr")}
    process = None
    with paths["stdout"].open("wb") as stdout, paths["stderr"].open("wb") as stderr:
        try:
            process = subprocess.Popen(argv, cwd=output, env=env, stdin=subprocess.DEVNULL,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            sizes = {stdout: 0, stderr: 0}
            deadline = time.monotonic() + timeout
            with selectors.DefaultSelector() as selector:
                selector.register(process.stdout, selectors.EVENT_READ, stdout)
                selector.register(process.stderr, selectors.EVENT_READ, stderr)
                while selector.get_map():
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        raise TimeoutError("command exceeded bounded deadline")
                    for key, _ in selector.select(min(remaining, 0.1)):
                        chunk = os.read(key.fileobj.fileno(), 65536)
                        if not chunk:
                            selector.unregister(key.fileobj)
                            continue
                        destination = key.data
                        available = cap - sizes[destination]
                        destination.write(chunk[:available])
                        sizes[destination] += min(len(chunk), available)
                        if len(chunk) > available:
                            raise ValueError("command exceeded bounded output cap")
                row["exit_code"] = process.wait(timeout=max(0.001, deadline - time.monotonic()))
        except (OSError, ValueError, TimeoutError, subprocess.TimeoutExpired) as error:
            row["error"] = type(error).__name__ + ": " + str(error)
        finally:
            if process is not None:
                if process.poll() is None:
                    process.kill()
                row["exit_code"] = process.wait()
                process.stdout.close()
                process.stderr.close()
    row["finished_at"] = utc()
    for stream, path in paths.items():
        row[stream] = {"path": path.name, "bytes": path.stat().st_size, "sha256": oracle.sha(path)}
    return row


def run(args):
    output, upstream = args.output.resolve(), args.upstream.resolve()
    # Validate every destination before mkdir, compiler invocation or report I/O.
    for protected in (ROOT, upstream, HELPER, CATALOG):
        require(disjoint(output, protected), "output overlaps protected source: " + str(protected))
    require(not output.exists(), "output must be a fresh directory")
    cc = shutil.which(args.cc)
    require(cc is not None, "compiler not found")
    cc = str(Path(cc).resolve())
    catalog = oracle.decode_json(CATALOG.read_bytes())
    require(catalog["upstream"]["revision"] == oracle.REVISION, "catalog revision")
    expected_sources = {row["path"]: row["sha256"] for row in catalog["sources"] + catalog["excluded_sources"]
                        if row["path"].startswith("lib/") and Path(row["path"]).suffix in (".c", ".h")}
    require("lib/compress/zstd_double_fast.c" in expected_sources, "pinned parser source")
    for name, digest in expected_sources.items():
        require(oracle.sha(upstream / name) == digest, "reference source differs: " + name)
    archive = upstream / "lib/libzstd.a"
    require(oracle.sha(archive) == ARCHIVE_SHA256, "pinned support archive")
    cases = oracle.recipes()
    output.mkdir(parents=True)
    packets = output / "packets"
    packets.mkdir()
    for case in cases:
        (packets / (case.name + ".packet")).write_bytes(case.packet())
    write_json(output / "recipes.json", {"schema_version": 1, "status": "pre-execution source-only hypotheses",
               "recipes": [case.metadata() for case in cases], "case_count": len(cases)})
    inputs = [Path(__file__).resolve(), Path(oracle.__file__).resolve(), HELPER, CATALOG, archive,
              *[upstream / name for name in sorted(expected_sources)], output / "recipes.json", *sorted(packets.iterdir())]

    def source_state():
        return [{"path": str(path), "bytes": path.stat().st_size, "sha256": oracle.sha(path)} for path in inputs]

    before = source_state()
    env = {"PATH": os.environ.get("PATH", os.defpath), "LC_ALL": "C", "LANG": "C"}
    if "TMPDIR" in os.environ:
        env["TMPDIR"] = os.environ["TMPDIR"]
    evidence = {"schema_version": 1, "status": "running", "started_at": utc(), "commands": [],
                "source_before": before, "reference_revision": oracle.REVISION,
                "reference_source_sha256": expected_sources, "support_archive_sha256": ARCHIVE_SHA256,
                "compiler": cc, "compiler_sha256": oracle.sha(Path(cc)), "results": {}}

    def save():
        write_json(output / "evidence.json", evidence)

    def command(label, argv, overrides=None):
        require(oracle.exact(source_state(), before), "immutable inputs changed before command")
        row = run_command(argv, output, label, env | (overrides or {}))
        evidence["commands"].append(row)
        save()
        require("error" not in row and row.get("exit_code") == 0, "failed command: " + label)
        require(oracle.exact(source_state(), before), "immutable inputs changed after command")
        return output / row["stdout"]["path"]

    save()
    try:
        revision = command("reference-revision", ["git", "-C", str(upstream), "rev-parse", "HEAD"])
        require(revision.read_text().strip() == oracle.REVISION, "reference checkout revision")
        status = command("reference-status", ["git", "-C", str(upstream), "status", "--porcelain"])
        require(status.read_bytes() == b"", "reference checkout changed")
        command("compiler-version", [cc, "--version"])
        for mode in ("normal", "sanitized"):
            executable = output / ("double-fast-" + mode)
            options = ["-O2"] if mode == "normal" else ["-O1", "-g", "-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
            compiler = [cc, "-std=c11", "-Wall", "-Wextra", "-Werror", "-I", str(upstream / "lib"),
                        "-I", str(upstream / "lib/common"), "-I", str(upstream / "lib/compress"), *options]
            sources = [str(HELPER), str(upstream / "lib/compress/zstd_double_fast.c")]
            # In BOTH modes explicitly compile the unchanged parser, rather than
            # trusting the archive's object for the function being measured.
            command(mode + "-dependencies", [*compiler, "-MM", *sources])
            command(mode + "-compile", [*compiler, *sources, str(archive), "-o", str(executable)])
            overrides = {"ASAN_OPTIONS": "halt_on_error=1:detect_leaks=0",
                         "UBSAN_OPTIONS": "halt_on_error=1:print_stacktrace=1"} if mode == "sanitized" else {}
            results = []
            for case in cases:
                label = mode + "-" + case.name
                path = command(label, [str(executable), str(packets / (case.name + ".packet"))], overrides)
                require((output / (label + ".stderr")).stat().st_size == 0, "capture stderr is not empty")
                rows = oracle.validate(case, path)
                if mode == "sanitized":
                    require(path.read_bytes() == (output / ("normal-" + case.name + ".stdout")).read_bytes(),
                            "normal/instrumented complete observations differ")
                results.append({"id": case.name, "observations": 2, "sequence_count": rows[1]["sequence_count"],
                                "metadata": rows[0], "stdout_sha256": oracle.sha(path)})
            evidence["results"][mode] = {"recipes": results, "executable_sha256": oracle.sha(executable),
                                        "validated_cases": len(cases), "validated_observations": 2 * len(cases)}
            save()
        evidence["source_after"] = source_state()
        require(oracle.exact(evidence["source_after"], before), "inputs changed during capture")
        evidence["status"] = "passed"
        evidence["finished_at"] = utc()
        evidence["instrumentation"] = {"helper_and_unchanged_double_fast_and_inline_headers": True,
                                       "remaining_archive_instrumented": False, "leak_sanitizer": False}
        evidence["scope"] = "Constructed direct states; sequences and final tables, not chronological tracing, collectors or ordinary frames."
        save()
        print("Validated 15 recipes / 30 observations in each of two builds: " + str(output))
    except BaseException:
        evidence["status"] = "failed"
        evidence["failure"] = traceback.format_exc()
        evidence["finished_at"] = utc()
        save()
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cc", default="clang")
    run(parser.parse_args())


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError) as error:
        print("ERROR: " + str(error), file=sys.stderr)
        raise SystemExit(1)

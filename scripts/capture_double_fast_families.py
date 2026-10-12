#!/usr/bin/env python3
"""Capture the fixed wider DoubleFast families using the unchanged tiny helper."""

from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import sys
import traceback

ROOT = Path(__file__).resolve().parents[1]
PINS = {"scripts/check_double_fast_recipes.py": "5ea9dbaa4aac38d6eb64b7379a9684c61247bbb9a560a2ae3728d392bdca710a",
        "scripts/capture_double_fast_tiny.py": "5dea723222db1171e6302411d4d59870e01a09dfd7df74bf47c51cc6978bef19",
        "scripts/oracles/double_fast_direct.py": "5280b94e6e10e34a263b7ae1b31a24026adfd92327767b776e81344362604065"}


def require(valid, reason):
    if not valid:
        raise ValueError(reason)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(args):
    # Verify immutable imported recipe/command code before executing it.
    for name, digest in PINS.items():
        require(sha(ROOT / name) == digest, "import identity: " + name)
    import check_double_fast_recipes as recipes
    import capture_double_fast_tiny as bounded
    cases = recipes.check_manifest(recipes.MANIFEST)
    oracle = recipes.load_families().direct
    output, upstream = args.output.resolve(), args.upstream.resolve()
    for protected in (ROOT, upstream, recipes.MANIFEST):
        require(bounded.disjoint(output, protected), "output overlaps protected input")
    require(not output.exists(), "output must be fresh")
    compiler = shutil.which(args.cc)
    require(compiler is not None, "compiler not found")
    compiler = str(Path(compiler).resolve())
    catalog = oracle.decode_json(bounded.CATALOG.read_bytes())
    require(catalog["upstream"]["revision"] == oracle.REVISION, "reference inventory revision")
    sources = {row["path"]: row["sha256"] for row in catalog["sources"] + catalog["excluded_sources"]
               if row["path"].startswith("lib/") and Path(row["path"]).suffix in (".c", ".h")}
    for name, digest in sources.items():
        require(sha(upstream / name) == digest, "reference source: " + name)
    archive = upstream / "lib/libzstd.a"
    require(sha(archive) == bounded.ARCHIVE_SHA256, "pinned support archive")
    output.mkdir(parents=True)
    packets = output / "packets"
    packets.mkdir()
    for case in cases:
        recipe = case.recipe
        (packets / (recipe.name + ".packet")).write_bytes(recipe.packet())
    inputs = [Path(__file__).resolve(), bounded.CATALOG, archive, recipes.MANIFEST,
              *[ROOT / name for name in sorted(set(PINS) | set(recipes.PINS))],
              *[upstream / name for name in sorted(sources)], *sorted(packets.iterdir())]

    def state():
        return [{"path": str(p), "bytes": p.stat().st_size, "sha256": sha(p)} for p in inputs]

    before = state()
    env = {"PATH": os.environ.get("PATH", os.defpath), "LANG": "C", "LC_ALL": "C"}
    if "TMPDIR" in os.environ:
        env["TMPDIR"] = os.environ["TMPDIR"]
    evidence = {"schema_version": 1, "status": "running", "started_at": bounded.utc(),
                "reference_revision": oracle.REVISION, "source_sha256": sources,
                "support_archive_sha256": bounded.ARCHIVE_SHA256, "source_before": before,
                "recipe_manifest_sha256": recipes.MANIFEST_SHA256, "recipes": 168,
                "families": 14, "observations_per_build": 336, "compiler": compiler,
                "compiler_sha256": sha(Path(compiler)), "commands": [], "results": {}}

    def save():
        bounded.write_json(output / "evidence.json", evidence)

    def command(label, argv, overrides=None):
        require(oracle.exact(state(), before), "source changed before command")
        record = bounded.run_command(argv, output, label, env | (overrides or {}))
        evidence["commands"].append(record)
        save()
        require(record.get("exit_code") == 0 and "error" not in record, "failed command: " + label)
        require(oracle.exact(state(), before), "source changed after command")
        return output / record["stdout"]["path"]

    save()
    try:
        require(command("revision", ["git", "-C", str(upstream), "rev-parse", "HEAD"]).read_text().strip() == oracle.REVISION, "checkout revision")
        require(command("status", ["git", "-C", str(upstream), "status", "--porcelain"]).read_bytes() == b"", "clean reference checkout")
        command("compiler", [compiler, "--version"])
        for mode in ("normal", "sanitized"):
            executable = output / ("double-fast-families-" + mode)
            options = ["-O2"] if mode == "normal" else ["-O1", "-g", "-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
            cc = [compiler, "-std=c11", "-Wall", "-Wextra", "-Werror", "-I", str(upstream / "lib"),
                  "-I", str(upstream / "lib/common"), "-I", str(upstream / "lib/compress"), *options]
            compile_sources = [str(bounded.HELPER), str(upstream / "lib/compress/zstd_double_fast.c")]
            command(mode + "-dependencies", [*cc, "-MM", *compile_sources])
            command(mode + "-compile", [*cc, *compile_sources, str(archive), "-o", str(executable)])
            overrides = {"ASAN_OPTIONS": "halt_on_error=1:detect_leaks=0", "UBSAN_OPTIONS": "halt_on_error=1:print_stacktrace=1"} if mode == "sanitized" else {}
            results = []
            for index, case in enumerate(cases):
                recipe = case.recipe
                label = mode + "-" + recipe.name
                path = command(label, [str(executable), str(packets / (recipe.name + ".packet"))], overrides)
                require((output / (label + ".stderr")).read_bytes() == b"", "nonempty parser/sanitizer diagnostic")
                rows = oracle.validate(recipe, path)
                if mode == "sanitized":
                    require(path.read_bytes() == (output / ("normal-" + recipe.name + ".stdout")).read_bytes(), "entire normal/sanitized observation equality")
                results.append({"id": recipe.name, "family": case.family, "sequence_count": rows[1]["sequence_count"],
                                "long_length_type": rows[1]["long_length_type"], "observations": 2, "stdout_sha256": sha(path)})
                evidence["results"][mode] = {"cases": results, "executable_sha256": sha(executable), "metadata": rows[0]}
                save()
                if index % 8 == 7 or index + 1 == len(cases):
                    print(f"{mode}: {index + 1}/{len(cases)} independently validated recipes", flush=True)
        evidence["source_after"] = state()
        require(oracle.exact(before, evidence["source_after"]), "final source/packet identity")
        evidence["status"] = "passed"
        evidence["finished_at"] = bounded.utc()
        evidence["instrumentation"] = {"helper_and_unchanged_double_fast_and_inline_headers": True,
                                       "remaining_archive_instrumented": False, "leak_sanitizer": False}
        evidence["scope"] = "Frozen constructed direct states; full sequences and final table maps, not chronological tracing or ordinary frames."
        save()
        print("Validated 168 recipes in both builds; 672 C padding-case executions: " + str(output))
    except BaseException:
        evidence["status"] = "failed"
        evidence["failure"] = traceback.format_exc()
        evidence["finished_at"] = bounded.utc()
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

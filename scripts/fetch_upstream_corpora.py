#!/usr/bin/env python3
"""Fetch SHA-256-pinned upstream corpora into a user-selected external cache.

Design: compressed archives own byte streams in files; immutable metadata owns
URL/hash/size records. New temporary directories isolate extraction. Every
member must be a regular file or directory with a safe relative name; links,
special files, duplicates, oversized members, and oversized aggregate output
are rejected before publishing a directory. No corpus archive enters the repo.

Work sequence: verify archive identity, inspect declared expansion bounds,
extract with bounded copies, verify observed member counts/sizes, publish only
complete directories. The official CLI is an import oracle for .zst archives,
never a fallback inside the Rust implementation. Corpora are rolling release
snapshots and are not claimed to be versioned with zstd v1.5.7.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath, PureWindowsPath
import stat
import subprocess
import tarfile
import tempfile
from urllib.request import Request, urlopen
import zipfile

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "tests" / "upstream" / "external-corpora.json"
MAX_UNPACKED = 1 << 30
CHUNK = 1 << 20


def digest_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(CHUNK):
            digest.update(chunk)
    return digest.hexdigest()


def safe_relative(name):
    if "\\" in name or "\x00" in name or PureWindowsPath(name).drive:
        raise ValueError(f"unsafe archive name: {name!r}")
    path = PurePosixPath(name)
    if path.is_absolute() or ".." in path.parts:
        raise ValueError(f"unsafe archive name: {name!r}")
    return Path(*path.parts)


def copy_bounded(source, destination, declared_size, maximum):
    if declared_size < 0 or declared_size > maximum:
        raise ValueError(f"member expansion exceeds remaining bound: {declared_size}")
    count = 0
    with destination.open("xb") as output:
        while chunk := source.read(min(CHUNK, declared_size - count + 1)):
            count += len(chunk)
            if count > declared_size:
                raise ValueError("member expanded beyond its declared size")
            output.write(chunk)
    if count != declared_size:
        raise ValueError("member size differs from its declaration")
    return count


def write_member(source, name, size, destination, seen, remaining):
    relative = safe_relative(name)
    if relative == Path(".") or relative in seen:
        raise ValueError(f"empty or duplicate file name: {name!r}")
    seen.add(relative)
    target = destination / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    return copy_bounded(source, target, size, remaining)


def extract_zip(archive, destination, maximum):
    seen = set()
    total = 0
    largest = 0
    with zipfile.ZipFile(archive) as packed:
        infos = packed.infolist()
        if sum(info.file_size for info in infos) > maximum:
            raise ValueError("ZIP declared expansion exceeds unpacked-byte bound")
        for info in infos:
            relative = safe_relative(info.filename)
            mode = stat.S_IFMT(info.external_attr >> 16)
            if info.flag_bits & 1 or mode not in (0, stat.S_IFREG, stat.S_IFDIR):
                raise ValueError(f"encrypted, linked, or special ZIP entry: {info.filename!r}")
            if info.is_dir():
                if info.file_size:
                    raise ValueError("directory entry has file contents")
                (destination / relative).mkdir(parents=True, exist_ok=True)
                continue
            if mode == stat.S_IFDIR:
                raise ValueError("inconsistent ZIP directory mode")
            with packed.open(info) as source:
                total += write_member(source, info.filename, info.file_size, destination, seen, maximum - total)
            largest = max(largest, info.file_size)
    return {"member_count": len(infos), "file_count": len(seen), "unpacked_bytes": total, "largest_member_bytes": largest}


def decode_zstd(archive, output, executable, maximum):
    count = 0
    # Spool diagnostics to avoid a child blocking on an unread stderr pipe.
    with tempfile.TemporaryFile() as diagnostics, output.open("xb") as decoded:
        process = subprocess.Popen([executable, "-q", "-d", "-c", "--", str(archive)], stdout=subprocess.PIPE, stderr=diagnostics)
        try:
            while chunk := process.stdout.read(CHUNK):
                count += len(chunk)
                if count > maximum:
                    raise ValueError("Zstandard archive expansion exceeds unpacked-byte bound")
                decoded.write(chunk)
            if process.wait() != 0:
                diagnostics.seek(0)
                raise ValueError(f"official archive decoder failed: {diagnostics.read(4096).decode(errors='replace')}")
        finally:
            process.stdout.close()
            if process.poll() is None:
                process.kill()
            process.wait()
    return count


def extract_tar(archive, destination, maximum):
    seen = set()
    total = 0
    largest = 0
    count = 0
    with tarfile.open(archive, mode="r:") as packed:
        for info in packed:
            count += 1
            relative = safe_relative(info.name)
            if info.isdir():
                (destination / relative).mkdir(parents=True, exist_ok=True)
                continue
            if not info.isfile() or info.issparse():
                raise ValueError(f"linked, sparse, or special TAR entry: {info.name!r}")
            with packed.extractfile(info) as source:
                total += write_member(source, info.name, info.size, destination, seen, maximum - total)
            largest = max(largest, info.size)
    return {"member_count": count, "file_count": len(seen), "unpacked_bytes": total, "largest_member_bytes": largest}


def ensure_archive(record, directory, offline):
    archive = directory / record["name"]
    if not archive.exists():
        if offline:
            raise FileNotFoundError(f"missing cached archive: {archive}")
        request = Request(record["source_url"], headers={"User-Agent": "zstdoxide-pinned-test-corpora"})
        with tempfile.NamedTemporaryFile(dir=directory, prefix="download-", delete=False) as temporary:
            temp_path = Path(temporary.name)
            try:
                count = 0
                with urlopen(request, timeout=60) as response:
                    while chunk := response.read(CHUNK):
                        count += len(chunk)
                        if count > record["compressed_bytes"]:
                            raise ValueError("download exceeds pinned compressed size")
                        temporary.write(chunk)
                temporary.close()
                if count != record["compressed_bytes"] or digest_file(temp_path) != record["sha256"]:
                    raise ValueError(f"archive identity changed: {record['name']}")
                temp_path.replace(archive)
            finally:
                temp_path.unlink(missing_ok=True)
    if archive.is_symlink() or not archive.is_file():
        raise ValueError(f"cache archive must be a regular file: {archive}")
    if archive.stat().st_size != record["compressed_bytes"] or digest_file(archive) != record["sha256"]:
        raise ValueError(f"cached archive fails pin: {archive}")
    return archive


def directory_inventory(directory):
    result = {}
    for path in sorted(directory.rglob("*")):
        if path.is_symlink():
            raise ValueError(f"existing extracted cache contains a symlink: {path}")
        if path.is_file():
            result[str(path.relative_to(directory))] = (path.stat().st_size, digest_file(path))
        elif not path.is_dir():
            raise ValueError(f"existing extracted cache contains a special file: {path}")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", required=True, type=Path, help="external cache, outside this repository")
    parser.add_argument("--target", action="append", help="manifest ID; repeat to choose multiple (default: every pinned archive)")
    parser.add_argument("--offline", action="store_true", help="require cached archives; never access the network")
    parser.add_argument("--max-unpacked-bytes", type=int, default=MAX_UNPACKED, help="aggregate extraction limit, at most 1 GiB")
    parser.add_argument("--zstd", default=os.environ.get("ZSTD_BIN", "zstd"), help="official v1.5.7 archive decoder")
    args = parser.parse_args()
    cache = args.cache.expanduser().resolve()
    if cache.is_relative_to(ROOT):
        parser.error("corpus cache must be outside the repository")
    if not 0 < args.max_unpacked_bytes <= MAX_UNPACKED:
        parser.error("max-unpacked-bytes must be between 1 and 1073741824")
    manifest = json.loads(MANIFEST.read_text())
    records = manifest["archives"]
    if args.target:
        requested = set(args.target)
        unknown = requested - {record["id"] for record in records}
        if unknown:
            parser.error(f"unknown corpus IDs: {sorted(unknown)}")
        records = [record for record in records if record["id"] in requested]
    if sum(record["unpacked_bytes"] for record in records) > args.max_unpacked_bytes:
        parser.error("selected corpora exceed aggregate unpacked-byte bound")
    if any(record["format"] != "zip" for record in records):
        version = subprocess.check_output([args.zstd, "--version"], text=True)
        if "v1.5.7" not in version:
            parser.error("official zstd v1.5.7 is required to unpack regression archives")
    archives = cache / "archives"
    extracted = cache / "extracted"
    for path in (archives, extracted):
        if path.is_symlink():
            parser.error(f"cache subdirectory must not be a symlink: {path}")
        path.mkdir(parents=True, exist_ok=True)
    total = 0
    for record in records:
        archive = ensure_archive(record, archives, args.offline)
        destination = extracted / record["id"]
        with tempfile.TemporaryDirectory(dir=cache, prefix="extract-") as temporary:
            scratch = Path(temporary)
            work = scratch / "files"
            work.mkdir()
            remaining = args.max_unpacked_bytes - total
            if record["format"] == "zip":
                observed = extract_zip(archive, work, remaining)
            else:
                decoded = scratch / "decoded"
                decoded_size = decode_zstd(archive, decoded, args.zstd, min(MAX_UNPACKED, record["decoded_bytes"]))
                if decoded_size != record["decoded_bytes"]:
                    raise ValueError("decoded archive size differs from pin")
                if record["format"] == "tar.zst":
                    observed = extract_tar(decoded, work, remaining)
                else:
                    if decoded_size > remaining:
                        raise ValueError("decoded file exceeds remaining aggregate byte bound")
                    decoded.replace(work / record["output_name"])
                    observed = {"member_count": 1, "file_count": 1, "unpacked_bytes": decoded_size, "largest_member_bytes": decoded_size}
            for key in ("member_count", "file_count", "unpacked_bytes", "largest_member_bytes"):
                if observed[key] != record[key]:
                    raise ValueError(f"{record['id']}: {key} differs from pinned observation")
            total += observed["unpacked_bytes"]
            if destination.exists():
                if destination.is_symlink() or directory_inventory(destination) != directory_inventory(work):
                    raise ValueError(f"existing extracted corpus differs; choose a fresh cache: {destination}")
            else:
                work.replace(destination)
        print(f"{record['id']}: {observed['file_count']} files, {observed['unpacked_bytes']} bytes, {destination}")
    print(f"Verified {len(records)} archive pins; {total} total extracted bytes")


if __name__ == "__main__":
    main()

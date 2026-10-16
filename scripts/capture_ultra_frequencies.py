#!/usr/bin/env python3
"""Execute a reviewed, bounded Ultra frequency C observation campaign."""
import argparse
import datetime
import json
import os
from pathlib import Path
import selectors
import shutil
import subprocess
import time
import sys
sys.dont_write_bytecode = True
import ultra_frequency_capture_plan as specification
from oracles.ultra_scalar import REVISION, need as require

ROOT = specification.ROOT


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def prepare(upstream, output, compiler, fresh=True):
    require(not upstream.is_symlink() and not output.is_symlink(), 'regular external roots')
    upstream = upstream.resolve(strict=True)
    output = output.resolve()
    for a, b in ((ROOT, upstream), (ROOT, output), (upstream, output)):
        require(not a.is_relative_to(b) and not b.is_relative_to(a), 'disjoint roots')
    if fresh:
        require(not output.exists(), 'fresh capture output')
    resolved = shutil.which(str(compiler))
    require(resolved is not None, 'available compiler')
    compiler = Path(resolved).resolve()
    require(compiler.is_file() and os.access(compiler, os.X_OK), 'compiler executable')
    source = specification.source_manifest()
    upstream_sources, units = specification.reference_sources(upstream)
    rows = specification.plan(upstream, output, compiler, units)
    specification.validate_plan(rows, upstream, output, compiler, units)
    return upstream, output, compiler, source, upstream_sources, rows


def execute(upstream, output, compiler):
    upstream, output, compiler, source, upstream_sources, rows = prepare(upstream, output, compiler)
    output.mkdir()
    for directory in ('logs', 'requests', 'c-normal', 'c-sanitized', 'source'):
        (output / directory).mkdir()
    for name in source:
        dest = output / 'source' / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, dest)
    for name, packet in specification.requests().items():
        (output / 'requests' / (name + '.bin')).write_bytes(packet)
    env = {k: os.environ[k] for k in ('PATH', 'HOME', 'TMPDIR', 'DEVELOPER_DIR', 'SDKROOT') if k in os.environ}
    env.update(LANG='C', LC_ALL='C', TZ='UTC', ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',
               UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1')
    evidence = dict(schema_version=1, status='running', started_at=utc(), cwd=str(ROOT),
                    upstream=str(upstream), output=str(output), compiler=str(compiler),
                    compiler_sha256=specification.sha(compiler.read_bytes()), source=source,
                    upstream_revision=REVISION, upstream_source_sha256=upstream_sources,
                    environment=env, plan=rows, commands=[], artifacts={})

    def save():
        (output / 'evidence.json').write_text(json.dumps(evidence, sort_keys=True, indent=2) + '\n')

    total_output = 0
    start = time.monotonic()
    save()
    try:
        for row in rows:
            require(time.monotonic() - start <= 1800, 'bounded campaign duration')
            label = row['label']
            paths = {s: output / 'logs' / (label + '.' + s) for s in ('stdout', 'stderr')}
            record = dict(row, started_at=utc(), stdin=None)
            process = None
            begin = time.monotonic()
            with paths['stdout'].open('xb') as stdout, paths['stderr'].open('xb') as stderr:
                try:
                    process = subprocess.Popen(row['argv'], cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
                                               stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                    deadline = time.monotonic() + row['timeout_seconds']
                    sizes = {stdout: 0, stderr: 0}
                    caps = {stdout: row['stdout_limit'], stderr: row['stderr_limit']}
                    with selectors.DefaultSelector() as selector:
                        selector.register(process.stdout, selectors.EVENT_READ, stdout)
                        selector.register(process.stderr, selectors.EVENT_READ, stderr)
                        while selector.get_map():
                            remaining = deadline - time.monotonic()
                            require(remaining > 0, 'command timeout')
                            for key, _ in selector.select(min(remaining, 0.1)):
                                chunk = os.read(key.fileobj.fileno(), 65536)
                                if not chunk:
                                    selector.unregister(key.fileobj)
                                    continue
                                destination = key.data
                                available = caps[destination] - sizes[destination]
                                destination.write(chunk[:available])
                                sizes[destination] += min(len(chunk), available)
                                total_output += len(chunk)
                                require(len(chunk) <= available and total_output <= 256 << 20, 'bounded output')
                        record['exit_code'] = process.wait(timeout=max(0.001, deadline - time.monotonic()))
                except BaseException as error:
                    record['error'] = repr(error)
                    raise
                finally:
                    if process is not None:
                        if process.poll() is None:
                            process.kill()
                        record['exit_code'] = process.wait()
                        process.stdout.close()
                        process.stderr.close()
                    stdout.flush()
                    stderr.flush()
                    record['seconds'] = time.monotonic() - begin
                    for stream, path in paths.items():
                        record[stream] = dict(path=str(path.relative_to(output)), bytes=path.stat().st_size,
                                              sha256=specification.sha(path.read_bytes()))
                    evidence['commands'].append(record)
                    save()
            require(record['exit_code'] == row['expected_exit'], 'command exit: ' + label)
            if '-compile-' in label or label.endswith('-link'):
                artifact = Path(row['argv'][-1])
                evidence['artifacts'][str(artifact.relative_to(output))] = dict(bytes=artifact.stat().st_size,
                    sha256=specification.sha(artifact.read_bytes()))
            if label.endswith('-link'):
                print('Built all used codec units:', label, flush=True)
        require(specification.source_manifest() == source, 'unchanged source')
        require(specification.reference_sources(upstream)[0] == upstream_sources, 'unchanged pinned reference')
        evidence.update(status='captured', completed_at=utc(), source_unchanged=True)
        save()
        import check_ultra_frequency_capture as auditor
        result = auditor.audit(output)
        evidence['audit'] = result
        evidence['status'] = 'passed'
        save()
        print(json.dumps(result, sort_keys=True), flush=True)
    except BaseException as error:
        evidence.update(status='failed', error=repr(error), completed_at=utc())
        save()
        raise


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--cc', default='clang')
    parser.add_argument('--preflight', action='store_true')
    args = parser.parse_args()
    if args.preflight:
        prepared = prepare(args.upstream, args.output, args.cc)
        print(json.dumps(dict(status='passed', commands=len(prepared[-1]), source=prepared[3]), indent=2))
    else:
        execute(args.upstream, args.output, args.cc)

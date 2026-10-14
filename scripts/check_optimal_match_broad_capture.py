#!/usr/bin/env python3
"""Independently audit complete Optimal match captures without child execution."""
import argparse
import functools
import json
from pathlib import Path
import sys
sys.dont_write_bytecode = True
import optimal_match_broad_capture_plan as specification
from oracles import optimal_match_broad_validation as model
from oracles import tree_validation as strict
from oracles.optimal_matches_broad import REVISION, parse_packet, require


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'))


def load(data):
    return json.loads(data, object_pairs_hook=strict.unique, parse_constant=strict.bad_constant)


def read(root, name, limit):
    p = root / name
    require(p.is_file() and not p.is_symlink() and p.resolve().is_relative_to(root), 'regular contained artifact: ' + name)
    require(p.stat().st_size <= limit, 'bounded artifact: ' + name)
    data = p.read_bytes()
    require(len(data) <= limit, 'artifact grew: ' + name)
    return data


def audit(root):
    root = root.resolve(strict=True)
    e = load(read(root, 'evidence.json', 8 << 20))
    keys = {'schema_version','status','started_at','cwd','upstream','output','compiler','compiler_sha256',
            'source','upstream_revision','upstream_source_sha256','environment','plan','commands','artifacts',
            'completed_at','source_unchanged'}
    if e.get('status') == 'passed':
        keys.add('audit')
    require(type(e) is dict and set(e) == keys and type(e['schema_version']) is int, 'strict envelope')
    require(all(type(e[k]) is str for k in ('started_at','completed_at','cwd','upstream','output','compiler','compiler_sha256','upstream_revision')), 'envelope strings')
    require(e['schema_version'] == 2 and e['status'] in ('captured', 'passed')
            and e['source_unchanged'] is True and e['upstream_revision'] == REVISION,
            'completed capture status')
    require(e['cwd'] == str(specification.ROOT), 'reviewed working directory')
    # Copies used for offline controls retain the original command/output root.
    output = Path(e['output'])
    upstream = Path(e['upstream'])
    compiler = Path(e['compiler'])
    require(output.is_absolute() and upstream.is_absolute() and compiler.is_absolute(), 'absolute execution roots')
    for a, b in ((specification.ROOT, upstream), (specification.ROOT, output), (upstream, output)):
        require(not a.is_relative_to(b) and not b.is_relative_to(a), 'disjoint execution roots')
    require(specification.sha(compiler.read_bytes()) == e['compiler_sha256'], 'compiler identity')
    source = specification.source_manifest()
    require(canonical(source) == canonical(e['source']), 'exact local source closure')
    for name, digest in source.items():
        require(specification.sha(read(root, 'source/' + name, 8 << 20)) == digest, 'retained source identity')
    sources, units = specification.reference_sources(upstream)
    require(canonical(sources) == canonical(e['upstream_source_sha256']), 'complete reference identity')
    env = e['environment']
    require(set(env) <= {'PATH', 'HOME', 'TMPDIR', 'DEVELOPER_DIR', 'SDKROOT', 'LANG', 'LC_ALL', 'TZ',
                         'ASAN_OPTIONS', 'UBSAN_OPTIONS'}, 'execution environment closure')
    require(all(type(k) is str and type(v) is str for k, v in env.items()), 'environment types')
    for key, value in {'LANG': 'C', 'LC_ALL': 'C', 'TZ': 'UTC', 'ASAN_OPTIONS': 'detect_leaks=0:halt_on_error=1',
                       'UBSAN_OPTIONS': 'halt_on_error=1:print_stacktrace=1'}.items():
        require(env.get(key) == value, 'required environment: ' + key)
    plan = specification.plan(upstream, output, compiler, units)
    specification.validate_plan(e['plan'], upstream, output, compiler, units)
    require(len(e['commands']) == len(plan), 'complete exact command count')
    packets = specification.requests()
    require({p.name for p in (root / 'requests').iterdir()} == {n + '.bin' for n in packets}, 'request inventory')
    for name, packet in packets.items():
        require(read(root, 'requests/' + name + '.bin', 524716) == packet, 'exact request source and schedule')
    require({p.name for p in (root / 'logs').iterdir()} ==
            {r['label'] + '.' + stream for r in plan for stream in ('stdout', 'stderr')}, 'complete log inventory')
    artifacts = {}
    total = operations = negative = 0
    transcripts = {}
    for expected, command in zip(plan, e['commands']):
        require(set(command) == set(expected) | {'started_at', 'stdin', 'seconds', 'exit_code', 'stdout', 'stderr'}, 'command record closure')
        require(canonical({k: command[k] for k in expected}) == canonical(expected), 'exact command/flags/order/bounds')
        require(type(command['exit_code']) is int and command['exit_code'] == expected['expected_exit']
                and command['stdin'] is None and type(command['seconds']) in (float, int)
                and 0 <= command['seconds'] <= expected['timeout_seconds'] + 5, 'command completion')
        logs = {}
        label = expected['label']
        for stream in ('stdout', 'stderr'):
            name = 'logs/' + label + '.' + stream
            data = read(root, name, expected[stream + '_limit'])
            require(canonical(command[stream]) == canonical(dict(path=name, bytes=len(data), sha256=specification.sha(data))), 'exact log metadata')
            total += len(data)
            require(total <= 256 << 20, 'aggregate logs')
            logs[stream] = data
        out, err = logs['stdout'], logs['stderr']
        if expected['expected_exit'] == 1:
            require(not out and err.startswith(b'optimal match oracle: ') and err.count(b'\n') == 1,
                    'malformed packet rejected before output')
            negative += 1
        else:
            require(not err, 'successful command stderr')
        if label == 'reference-revision':
            require(out.decode().strip() == REVISION, 'actual upstream revision')
        elif label == 'reference-status':
            require(not out, 'clean reference')
        elif label.endswith('-dependencies'):
            dependencies = set()
            for line in out.decode().replace('\\\n', ' ').splitlines():
                require(':' in line, 'dependency grammar')
                for text in line.split(':', 1)[1].split():
                    p = Path(text).resolve()
                    require(p == specification.ROOT / specification.HELPER or
                            p.is_relative_to(upstream) and str(p.relative_to(upstream)) in sources,
                            'pinned dependency closure')
                    dependencies.add(p)
            require(upstream / 'lib/compress/zstd_opt.c' in dependencies and
                    specification.ROOT / specification.HELPER in dependencies, 'embedded source dependency')
        elif label.endswith('-predefines'):
            require(b'#define ZSTD_MULTITHREAD ' not in out and b'#define ZSTD_DISABLE_ASM' in out
                    and b'#define DEBUGLEVEL 2' in out, 'effective compilation switches')
        elif '-compile-' in label or label.endswith('-link'):
            require(not out, 'quiet compilation')
            name = str(Path(expected['argv'][-1]).relative_to(output))
            data = read(root, name, 64 << 20)
            artifacts[name] = dict(bytes=len(data), sha256=specification.sha(data))
        elif '-run-om_' in label:
            mode, name = label.split('-run-', 1)
            result = model.validate(parse_packet(packets[name]), out)
            operations += result['operations']
            if mode == 'normal':
                transcripts[name] = out
            else:
                require(transcripts[name] == out, 'normal/instrumented complete equality')
    require(canonical(artifacts) == canonical(e['artifacts']) and len(artifacts) == 54, 'all objects and executables')
    require(len(transcripts) == 37 and operations == 164 and negative == 44, 'exact capture totals')
    result = dict(status='passed', recipes=37, operations_per_build=82, commands=len(plan),
                  malformed_rejections_per_build=22, instrumented_codec_units=26,
                  normal_instrumented_equal=True, log_bytes=total,
                  qualification='Independent complete broader short-key geometry, distance and repeat replay with two padding passes. No ordinary parser startup, Rust implementation or public Optimal claim.')
    if 'audit' in e:
        require(canonical(e['audit']) == canonical(result), 'saved audit equals independent replay')
    return result



if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    args = parser.parse_args()
    print(json.dumps(audit(args.capture), sort_keys=True, indent=2))

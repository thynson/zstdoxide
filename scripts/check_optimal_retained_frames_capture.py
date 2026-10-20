#!/usr/bin/env python3
"""Independently audit complete retained ordinary Optimal captures without child execution."""
import argparse
import functools
import json
from pathlib import Path
import sys
sys.dont_write_bytecode = True
import optimal_retained_frames_capture_plan as specification
from oracles import optimal_retained_frames_validation as model
from oracles import tree_validation as strict
from oracles.fast_frame_publication_support import frame_blocks
from oracles.optimal_retained_frames import REVISION, parse_packet, require


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
    for name,data in specification.generated(upstream).items():
        require(read(root,'generated/'+name,1<<20)==data,'exact original/generated instrumentation')
    source = specification.source_manifest()
    require(canonical(source) == canonical(e['source']), 'exact local source closure')
    for name, digest in source.items():
        require(specification.sha(read(root, 'source/' + name, 8 << 20)) == digest, 'retained source identity')
    sources, units = specification.reference_sources(upstream)
    require(canonical(sources) == canonical(e['upstream_source_sha256']), 'complete reference identity')
    specification.environment(e['environment'])
    plan = specification.plan(upstream, output, compiler, units)
    specification.validate_plan(e['plan'], upstream, output, compiler, units)
    require(len(e['commands']) == len(plan), 'complete exact command count')
    packets = specification.requests()
    require({p.name for p in (root / 'requests').iterdir()} == {n + '.bin' for n in packets}, 'request inventory')
    for name, packet in packets.items():
        require(read(root, 'requests/' + name + '.bin', 32828) == packet, 'exact request source and schedule')
    require({p.name for p in (root / 'logs').iterdir()} ==
            {r['label'] + '.' + stream for r in plan for stream in ('stdout', 'stderr')}, 'complete log inventory')
    predictions = {r["name"]: r for r in load(specification.manifest_bytes())["cases"]}
    artifacts = {}
    total = operations = negative = 0
    transcripts = {}
    frames = {}
    ledgers = {}
    kinds = {}
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
            require(total <= 512 << 20, 'aggregate logs')
            logs[stream] = data
        out, err = logs['stdout'], logs['stderr']
        if expected['expected_exit'] == 1:
            require(not out and err.startswith(b'Optimal retained observer: ') and err.count(b'\n') == 1,
                    'malformed packet rejected before output')
            negative += 1
        else:
            require(not err, 'successful command stderr')
        if expected['expected_exit'] == 0:
            specification.command_gate(label, out, err, upstream, output, sources)
        if '-run-negative-' in label:
            mode, name = label.split('-run-', 1)
            require(all(not (root/p).exists() for p in specification.artifact_paths(mode,name)),
                    'malformed request has no artifacts')
        if '-compile-' in label or label.endswith('-link'):
            require(not out, 'quiet compilation')
            name = str(Path(expected['argv'][-1]).relative_to(output))
            data = read(root, name, 64 << 20)
            artifacts[name] = dict(bytes=len(data), sha256=specification.sha(data))
        elif '-run-wide_' in label:
            mode, name = label.split('-run-', 1)
            request = parse_packet(packets[name])
            validate_model_output(out,request,specification.retained_recipes()[name].kinds)
            operations += predictions[name]['events']
            if mode == 'normal':
                transcripts[name] = out
            else:
                require(transcripts[name] == out, 'normal/instrumented complete equality')
            paths = specification.artifact_paths(mode,name)
            for path in paths:
                data = read(root,path,32832)
                artifacts[path] = dict(bytes=len(data),sha256=specification.sha(data))
            first, second = (read(root,path,32832) for path in paths[:2])
            require(first == second, 'padding frame equality')
            frame_kind = validate_frame(first,request,specification.retained_recipes()[name].kinds)
            ledger_first, ledger_second = (load(read(root,path,32832)) for path in paths[2:])
            validate_ledger(ledger_first)
            strict.same(ledger_first,ledger_second,'padding allocation equality')
            if mode == 'normal':
                frames[name] = first
                ledgers[name] = ledger_first
                kinds[name] = frame_kind
            else:
                require(frames[name] == first,'normal/instrumented complete frame equality')
                validate_ledger_pair(ledgers[name],ledger_first)
    require(canonical(artifacts) == canonical(e['artifacts']) and len(artifacts) == 54 + 8 * len(predictions),
            'all objects, executables, frames and ledgers')
    for mode in ('normal','sanitized'):
        require({str(p.relative_to(root)) for p in (root/'artifacts'/mode).iterdir()} ==
                {p for name in specification.recipes() for p in specification.artifact_paths(mode,name)},
                'exact generated artifact inventory')
    require(len(transcripts) == len(predictions) == 19 and operations == 2 * sum(r['events'] for r in predictions.values()) and negative == 2 * len(specification.malformed_packets()), 'exact capture totals')
    result = dict(status='passed', recipes=len(predictions), events_per_build=operations//2, commands=len(plan),
                  malformed_rejections_per_build=negative//2, instrumented_codec_units=26,
                  normal_instrumented_equal=True, log_bytes=total, frame_kinds=kinds,
                  frames=4*len(predictions), allocation_ledgers=4*len(predictions),
                  qualification='Larger sparse retained ordinary unchanged-provider CCtx with complete independent search/path/frequency and committed-repeat publication replay, frame structure and allocator audit. Independent Rust complete-frame decoding remains a separate gate.')
    if 'audit' in e:
        require(canonical(e['audit']) == canonical(result), 'saved audit equals independent replay')
    return result


# Cache independently predicted immutable canonical lines and exact validated
# transcript identities. Forged bytes cannot reuse a prior identity; failed
# validation never enters the verified set. No child execution is involved.
_verified = set()
@functools.lru_cache(maxsize=24)
def expected_lines(request,kinds):
    lines=[];size=0
    for row in model.observations(request,kinds):
        line=canonical(row);size+=len(line)+1
        require(len(line)<=model.MAX_LINE and size<=model.MAX_OUTPUT,'bounded predicted output')
        lines.append(line)
    return tuple(lines)

def validate_model_output(data,request,kinds):
    import io
    require(type(data)is bytes and 0<len(data)<=model.MAX_OUTPUT and data.endswith(b'\n'),'bounded model transcript')
    key=(request,kinds,specification.sha(data))
    if key in _verified:return
    stream=io.BytesIO(data)
    for expected in expected_lines(request,kinds):
        line=stream.readline(model.MAX_LINE+1)
        require(0<len(line)<=model.MAX_LINE and line.endswith(b'\n'),'bounded native line')
        require(canonical(load(line))==expected,'independent wider sparse state/publication')
    require(stream.read(1)==b'','complete sparse transcript EOF')
    _verified.add(key)

def validate_ledger(value):
    require(type(value) is dict and set(value)=={'allocations','peak_bytes','released'},'ledger schema')
    values=value['allocations']
    require(type(values) is list and 1 <= len(values) <= 32 and
            all(type(v) is int and 0 < v <= 8<<20 for v in values),'allocation request domain')
    require(type(value['peak_bytes']) is int and max(values) <= value['peak_bytes'] <= min(sum(values),8<<20),
            'allocation high-water domain')
    require(value['released'] is True,'complete owner release')


def validate_ledger_pair(normal,sanitized):
    validate_ledger(normal);validate_ledger(sanitized)
    a,b=normal['allocations'],sanitized['allocations']
    require(len(a)==len(b)==2 and a[0]==b[0], 'fresh CCtx and workspace owners')
    # Pinned ZSTD_estimateCCtxSize_usingCCtxParams_internal adds redzones to
    # 6 optimal buffers, 5 token buffers, 1 temporary and 2 block states.
    # Match tables, absent stream/LDM buffers and the separate CCtx add none.
    require(b[1]==a[1]+14*2*128,'fourteen upstream workspace redzone pairs')
    require(normal['peak_bytes']==sum(a) and sanitized['peak_bytes']==sum(b),
            'two live owners high-water mark')


def validate_frame(data,request,kinds):
    n=len(request.source)
    header = bytes.fromhex('28b52ffd') + (bytes((0x20,n)) if n<256 else bytes((0x60,))+(n-256).to_bytes(2,'little'))
    require(data.startswith(header),'exact single-segment source-size header')
    blocks=frame_blocks(data)
    require(len(blocks)==len(request.blocks)+1 and blocks[-1]==dict(kind=0,field=0,last=1,payload=b''),
            'complete scheduled blocks and empty final block')
    position=0
    for block,length,kind in zip(blocks,request.blocks,kinds):
        source=request.source[position:position+length]
        require(block['last']==0 and block['kind']==kind,'ordinary publication kind agrees with wire frame')
        if kind==0:require(block['payload']==source,'raw frame source')
        elif kind==1:require(block['field']==length and block['payload']*length==source,'RLE frame source')
        else:require(0<block['field']<length,'bounded beneficial compressed block')
        position+=length
    require(position==n,'complete frame source extent')
    return list(kinds)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    args = parser.parse_args()
    print(json.dumps(audit(args.capture), sort_keys=True, indent=2))

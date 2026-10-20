#!/usr/bin/env python3
"""Independently audit the complete source-bound private Ultra2 lifecycle capture."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
sys.dont_write_bytecode = True
import ultra2_capture_plan as specification
from oracles import ultra2_protocol as model
from oracles import tree_validation as strict
from oracles.ultra2_protocol import REVISION, parse_packet
from oracles.ultra_scalar import need as require


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
        require(read(root, 'requests/' + name + '.bin', len(packet)) == packet, 'exact request source and schedule')
    require({p.name for p in (root / 'logs').iterdir()} ==
            {r['label'] + '.' + stream for r in plan for stream in ('stdout', 'stderr')}, 'complete log inventory')
    predictions = {r["name"]: r for r in load((specification.ROOT/specification.MANIFEST).read_bytes())["cases"]}
    require(len(predictions)==53, 'complete frozen recipe inventory')
    artifacts = {}
    total = operations = negative = 0
    transcripts = {}
    ledgers = {}
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
            require(total <= 5 << 30, 'aggregate logs')
            logs[stream] = data
        out, err = logs['stdout'], logs['stderr']
        if expected['expected_exit'] == 1:
            require(not out and type(expected['expected_diagnostic']) is str and err == expected['expected_diagnostic'].encode(),
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
        elif '-run-u2_' in label:
            mode, name = label.split('-run-', 1)
            request = parse_packet(packets[name])
            canonical_digest=validate_model_output(out,request)
            require(canonical_digest==predictions[name]['wire']['canonical_sha256'] and len(out)==predictions[name]['wire']['canonical_bytes'], 'source-predicted complete wire bytes and digest')
            completion = load(out.splitlines()[-1])
            require(completion['events'] == 2 * predictions[name]['events'], 'observed predicted event count')
            operations += completion['events']
            if mode == 'normal':
                transcripts[name] = specification.sha(out)
            else:
                require(transcripts[name] == specification.sha(out), 'normal/instrumented complete equality')
            paths = specification.artifact_paths(mode,name)
            for path in paths:
                data = read(root,path,32768)
                artifacts[path] = dict(bytes=len(data),sha256=specification.sha(data))
            ledger_first, ledger_second = (load(read(root,path,32768)) for path in paths)
            validate_ledger(ledger_first)
            strict.same(ledger_first,ledger_second,'padding allocation equality')
            if mode == 'normal':ledgers[name]=ledger_first
            else:validate_ledger_pair(ledgers[name],ledger_first)
    require(canonical(artifacts) == canonical(e['artifacts']) and len(artifacts) == 54 + 4 * len(predictions),
            'all objects, executables and allocation ledgers')
    for mode in ('normal','sanitized'):
        require({str(p.relative_to(root)) for p in (root/'artifacts'/mode).iterdir()} ==
                {p for name in specification.recipes() for p in specification.artifact_paths(mode,name)},
                'exact generated artifact inventory')
    require(len(transcripts) == len(predictions) == 53 and operations == 4 * sum(r['events'] for r in predictions.values()) and negative == 2 * (len(specification.malformed_packets())+len(specification.special_inputs())), 'exact capture totals')
    result = dict(status='passed', recipes=len(predictions), logical_events_per_padding_pass=operations//4,
                  observed_events_per_build=operations//2, commands=len(plan),
                  malformed_rejections_per_build=negative//2, instrumented_codec_units=26,
                  normal_instrumented_equal=True, log_bytes=total, allocation_ledgers=4*len(predictions),
                  qualification='Private unchanged Ultra2 wrapper and original first-pass/reset/rebase operations. '
                  'Complete independent pass-tagged search/path/frequency, copied repeats, source guards and '
                  'allocation replay. No frame publication, Rust implementation or public strategy-9 admission.')
    if 'audit' in e:
        require(canonical(e['audit']) == canonical(result), 'saved audit equals independent replay')
    return result


# Every complete canonical expected transcript is computed from source and sealed
# before native execution. Hashing every parsed native record checks the same
# full equality without rerunning a costly model on each copied-evidence forgery.
# The manifest's own digest is a literal in the reviewed capture specification.
_verified = {}

def validate_model_output(data,request):
    import io
    require(type(data)is bytes and 0<len(data)<=model.MAX_OUTPUT and data.endswith(b'\n'),'bounded model transcript')
    prediction=specification.prediction(request)
    wanted=prediction['wire']
    key=(request,specification.sha(data),wanted['canonical_sha256'])
    if key in _verified:return _verified[key]
    require(len(data)==wanted['canonical_bytes'],'exact source-predicted transcript extent')
    stream=io.BytesIO(data);digest=hashlib.sha256();count=events=maximum=0
    while True:
        line=stream.readline(model.MAX_LINE+1)
        if not line:break
        require(len(line)<=model.MAX_LINE and line.endswith(b'\n'),'bounded complete native line')
        row=load(line);require(type(row)is dict,'observation object')
        encoded=(canonical(row)+'\n').encode()
        digest.update(encoded);count+=1;events+=row.get('record')=='event';maximum=max(maximum,len(encoded))
    require(count==wanted['records']and events==wanted['events']
            and maximum==wanted['maximum_line_bytes'],'complete source-predicted record inventory')
    require(digest.hexdigest()==wanted['canonical_sha256'],'every canonical native state equals frozen independent model')
    _verified[key]=digest.hexdigest()
    return _verified[key]

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


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = audit(args.capture)
    if args.output:
        require(not args.output.exists(), 'fresh independent audit sidecar')
        sidecar = dict(status='passed', capture_evidence_sha256=specification.sha((args.capture/'evidence.json').read_bytes()),
            auditor_sha256=specification.sha(Path(__file__).read_bytes()), audit=result)
        args.output.write_text(json.dumps(sidecar,sort_keys=True,indent=2)+'\n')
    print(json.dumps(result, sort_keys=True, indent=2))

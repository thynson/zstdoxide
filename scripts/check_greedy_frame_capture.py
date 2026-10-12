#!/usr/bin/env python3
"""Audit complete ordinary Greedy capture provenance and replay offline."""
from pathlib import Path, PurePosixPath
import argparse
import hashlib
import json
import os
import re

ROOT = Path(__file__).resolve().parents[1]
CONTROLLER = 'scripts/capture_greedy_frames_v2.py'
CONTROLLER_SHA = '6f4e3ec00d8f2679e4b8bf15be8c837d879e360dc541d11109be593d8e66dddb'


def require(value, reason):
    if not value:
        raise ValueError(reason)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def exact(left, right):
    return json.dumps(left, sort_keys=True, allow_nan=False) == json.dumps(right, sort_keys=True, allow_nan=False)


def read(root, name, cap=64 << 20):
    path = PurePosixPath(name)
    require(not path.is_absolute() and '..' not in path.parts and str(path) == name and '\\' not in name,
            'canonical artifact path')
    p = root / name
    require(p.is_file() and not p.is_symlink() and p.resolve().is_relative_to(root)
            and p.stat().st_size <= cap, 'bounded regular artifact: ' + name)
    data = p.read_bytes()
    require(len(data) <= cap, 'artifact grew beyond cap')
    return data


def inputs():
    require(sha(read(ROOT, CONTROLLER, 1 << 20)) == CONTROLLER_SHA, 'frozen capture controller')
    import capture_greedy_frames_v2 as capture
    recipes, floor_recipes, _, validator, generator, cases = capture.load_inputs()
    return capture, recipes, floor_recipes, validator, generator, cases


def audit(directory):
    capture, recipes, floor_recipes, v, generator, cases = inputs()
    root = directory.resolve()
    e = v.parse_json(read(root, 'evidence.json', 8 << 20))
    require(e['status'] == 'passed' and exact(e['schema_version'], 1)
            and e['reference_revision'] == generator.REVISION and e['support_archive_used'] is False
            and e['normal_sanitized_equal'] is True, 'completed capture scope')
    require(exact(e['source_before'], e['source_after']), 'immutable capture inputs')
    commands = {r['label']: r for r in e['commands']}
    require(len(commands) == len(e['commands']) == 440, 'unique complete command count')
    command = commands['normal-dependencies']['argv']
    helper = Path(command[command.index('-MM') + 1]); original_root = helper.parents[2]
    output = Path(commands['reference-revision']['cwd'])
    upstream = Path(commands['reference-revision']['argv'][2]); cc = e['compiler']
    require(all(p.is_absolute() for p in (helper, original_root, output, upstream, Path(cc))), 'absolute captured locations')
    require(helper == original_root / 'scripts/oracles/greedy_frame_v3.c', 'reviewed helper location')
    inventory = v.parse_json(read(ROOT, 'tests/upstream/manifest.json', 2 << 20))
    sources = {r['path']: r['sha256'] for r in inventory['sources'] + inventory['excluded_sources']
               if r['path'].startswith('lib/') and Path(r['path']).suffix in ('.c', '.h')}
    units = sorted(n for n in sources if n.endswith('.c') and PurePosixPath(n).parent.as_posix()
                   in ('lib/common', 'lib/compress', 'lib/decompress'))
    require(len(units) == 26 and exact(e['reference_source_sha256'], sources), 'complete upstream closure')
    local_names = [CONTROLLER, 'tests/upstream/manifest.json', str(recipes.GENERATOR.relative_to(ROOT)),
                   str(recipes.MANIFEST.relative_to(ROOT)), 'scripts/oracles/fast_frame_publication_support.py', *capture.PINS]
    input_data = {c.name + '.source': c.source for c in cases}
    input_data |= {'recipes.json': read(ROOT, str(recipes.MANIFEST.relative_to(ROOT))),
                   'floor-recipes.json': read(ROOT, str(floor_recipes.MANIFEST.relative_to(ROOT))),
                   'oversize.source': bytes((4 << 20) + 1)}
    expected_paths = [str(original_root / n) for n in local_names] + [str(upstream / n) for n in sorted(sources)]
    expected_paths += [str(output / 'inputs' / n) for n in sorted(input_data)]
    require([r['path'] for r in e['source_before']] == expected_paths, 'complete ordered input snapshot')
    for name, row in zip(local_names, e['source_before']):
        data = read(ROOT, name)
        require(exact(row, {'path':str(original_root / name),'bytes':len(data),'sha256':sha(data)}), 'repository input identity')
    offset = len(local_names)
    for name, row in zip(sorted(sources), e['source_before'][offset:]):
        require(row['sha256'] == sources[name] and type(row['bytes']) is int and 0 < row['bytes'] <= 2 << 20,
                'upstream input identity')
    offset += len(sources)
    for name, row in zip(sorted(input_data), e['source_before'][offset:]):
        data = read(root, 'inputs/' + name)
        require(data == input_data[name] and exact(row, {'path':str(output/'inputs'/name), 'bytes':len(data), 'sha256':sha(data)}),
                'independently regenerated input bytes')
    require({p.name for p in (root/'inputs').iterdir()} == set(input_data), 'exact generated input inventory')
    raw = {}
    baseline_env = commands['reference-revision']['environment']
    require(set(baseline_env) in ({'PATH','LC_ALL','LANG','TZ'}, {'PATH','LC_ALL','LANG','TZ','TMPDIR'})
            and all(type(s) is str for s in baseline_env.values())
            and baseline_env['LC_ALL'] == baseline_env['LANG'] == 'C' and baseline_env['TZ'] == 'UTC', 'clean capture environment')
    sanitizer = {'ASAN_OPTIONS':'detect_leaks=0:halt_on_error=1', 'UBSAN_OPTIONS':'halt_on_error=1:print_stacktrace=1'}
    for label, row in commands.items():
        negative = '-negative-' in label
        require('error' not in row and exact(row['exit_code'], 1 if negative else 0)
                and row['cwd'] == str(output) and exact(row['timeout_seconds'], 30 if negative else 180)
                and exact(row['per_stream_cap'], 64 << 20), 'bounded command outcome')
        is_runtime = label.startswith('sanitized-') and not (label.startswith('sanitized-compile-') or label in ('sanitized-link','sanitized-dependencies','sanitized-predefines'))
        require(exact(row['environment'], baseline_env | (sanitizer if is_runtime else {})), 'command environment')
        for stream in ('stdout','stderr'):
            name = label + '.' + stream
            data = read(root, name); meta = row[stream]
            require(exact(meta, {'path':name,'bytes':len(data),'sha256':sha(data)}), 'retained log bytes/hash')
            raw[label,stream] = data
        error = raw[label,'stderr']
        if negative:
            require(error.startswith(b'frame oracle [') and b'ERROR: AddressSanitizer' not in error and b'runtime error:' not in error,
                    'clean helper rejection')
        else:
            require(not error, 'unexpected diagnostics')
    expected = {}
    def expect(label, argv):
        require(label not in expected and commands[label]['argv'] == argv, 'exact invocation: ' + label)
        expected[label] = argv
    expect('reference-revision', ['git','-C',str(upstream),'rev-parse','HEAD'])
    expect('reference-status', ['git','-C',str(upstream),'status','--porcelain','--untracked-files=no'])
    expect('compiler-version', [cc,'--version']); expect('compiler-target',[cc,'-dumpmachine'])
    require(raw['reference-revision','stdout'].decode().strip() == generator.REVISION and not raw['reference-status','stdout'], 'reference checkout identity')
    require(re.fullmatch('[0-9a-f]{64}',e['compiler_sha256']), 'compiler identity syntax')
    require(set(e['results']) == {'normal','sanitized'}, 'build inventory')
    common = [cc,'-std=c11','-g','-I',str(upstream/'lib'),'-I',str(upstream/'lib/common'),'-I',str(upstream/'lib/compress'),
              '-UZSTD_MULTITHREAD','-DZSTD_DISABLE_ASM','-DZSTD_LEGACY_SUPPORT=0','-DXXH_NAMESPACE=ZSTD_']
    accepted = {}; summaries = {}; decoded_bytes = sum(len(c.source)*(1+int(c.whole_compare)) for c in cases)
    def records(label):
        return [v.parse_json(line) for line in raw[label,'stdout'].splitlines()]
    for mode in ('normal','sanitized'):
        result = e['results'][mode]; build = output/('build-'+mode); frames = output/('frames-'+mode)
        require(exact(result['compiled_codec_sources'], units) and exact(result['instrumented_codec_sources'], units if mode == 'sanitized' else [])
                and result['helper_instrumented'] is (mode == 'sanitized') and result['leak_sanitizer'] is False and result['assembly'] is False,
                'all-unit instrumentation scope')
        flags = ['-O2'] if mode == 'normal' else ['-O1','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer']
        expect(mode+'-dependencies',common+flags+['-MM',str(helper)]+[str(upstream/n) for n in units])
        expect(mode+'-predefines',common+flags+['-dM','-E',str(helper)])
        predefines = raw[mode+'-predefines','stdout']
        require(b'#define ZSTD_DISABLE_ASM ' in predefines and b'#define ZSTD_LEGACY_SUPPORT 0\n' in predefines
                and b'#define ZSTD_MULTITHREAD ' not in predefines, 'effective assembly/legacy/worker controls')
        dependencies = set()
        for line in raw[mode+'-dependencies','stdout'].decode().replace('\\\n',' ').splitlines():
            dependencies.update(os.path.normpath(name) for name in line.split(':',1)[1].split())
        allowed = {str(helper):capture.PINS['scripts/oracles/greedy_frame_v3.c']} | {str(upstream/n):s for n,s in sources.items()}
        require(dependencies <= allowed.keys() and str(helper) in dependencies and all(str(upstream/n) in dependencies for n in units), 'used source/header closure')
        require(exact(result['dependencies'],{n:allowed[n] for n in sorted(dependencies)}), 'dependency identity map')
        objects = []
        for i,n in enumerate(units):
            obj = str(build/('upstream-'+str(i)+'.o')); objects.append(obj)
            expect(mode+'-compile-'+str(i), common+flags+['-c',str(upstream/n),'-o',obj])
        exe = str(build/'greedy-frame-v3')
        expect(mode+'-link',common+flags+['-Wall','-Wextra','-Werror','-Wno-deprecated-declarations',str(helper)]+objects+['-o',exe])
        require(sha(read(root,'build-'+mode+'/greedy-frame-v3')) == result['executable_sha256'], 'retained executable identity')
        expect(mode+'-metadata',[exe,'metadata'])
        meta = records(mode+'-metadata'); require(len(meta)==1 and exact(result['metadata'],meta[0]),'metadata observation'); v.validate_metadata(meta[0])
        if mode == 'normal': accepted['metadata.stdout'] = raw[mode+'-metadata','stdout']
        else: require(raw[mode+'-metadata','stdout']==accepted['metadata.stdout'],'metadata build equality')
        mode_summaries=[]; frame_names=set()
        for case in cases:
            name=case.name; label=mode+'-'+name; frame_name=name+'.zst'; whole_name=name+'.whole.zst'
            argv=[exe,'frame',name,str(output/'inputs'/(name+'.source')),','.join(map(str,case.schedule)) or '-',str(frames/frame_name),
                  str(frames/whole_name) if case.whole_compare else '-', *map(str,(case.window_log,case.hash_log,case.chain_log,case.search_log,case.min_match,case.target))]
            expect(label,argv); frame=read(root,'frames-'+mode+'/'+frame_name, (4 << 20)+256); whole=read(root,'frames-'+mode+'/'+whole_name,(4 << 20)+256) if case.whole_compare else None
            summary=v.validate_case(case,records(label),frame,whole)
            summary.update(stdout_sha256=sha(raw[label,'stdout']),frame_sha256=sha(frame),frame_bytes=len(frame),whole_sha256=sha(whole) if whole else None,whole_bytes=len(whole) if whole else None)
            mode_summaries.append(summary); frame_names.add(frame_name)
            artifacts={name+'.stdout':raw[label,'stdout'],frame_name:frame}
            if whole is not None: artifacts[whole_name]=whole; frame_names.add(whole_name)
            for n,data in artifacts.items():
                if mode=='normal': accepted[n]=data
                else: require(accepted[n]==data,'complete normal/instrumented artifact equality')
        require(exact(mode_summaries,result['cases']),'independently replayed summaries'); summaries[mode]=mode_summaries
        require({p.name for p in (root/('frames-'+mode)).iterdir()}==frame_names,'exact frame inventory')
        headers=[]
        for i,(pledge,window) in enumerate(generator.HEADERS):
            label=mode+'-header-'+str(i); expect(label,[exe,'header',str(pledge),str(window)])
            v.validate_partial(records(label),pledge,window)
            headers.append({'pledge':pledge,'window_log':window,'stdout_sha256':sha(raw[label,'stdout'])})
            name='header-'+str(i)+'.stdout'
            if mode=='normal': accepted[name]=raw[label,'stdout']
            else: require(accepted[name]==raw[label,'stdout'],'incomplete-header build equality')
        require(exact(headers,result['headers']),'header summary')
        negative=[]
        for name,argv,forbidden in capture.malformed(output/'inputs',frames):
            label=mode+'-negative-'+name; expect(label,[exe]+argv)
            require(forbidden is None or not (root/forbidden.relative_to(output)).exists(),'rejected command wrote frame')
            negative.append({'id':name,'stdout_sha256':sha(raw[label,'stdout'])})
        require(exact(negative,result['malformed']),'rejection summary')
    require(set(commands)==set(expected),'complete exact invocation inventory')
    require({p.name for p in root.iterdir() if p.suffix in ('.stdout','.stderr')}=={label+'.'+stream for label in commands for stream in ('stdout','stderr')},'complete log inventory')
    kinds=[k for s in summaries['normal'] for k in s['block_kinds']]
    rollback=[r['kind'] for s in summaries['normal'] for r in s['rollback_witnesses']]
    coverage={'recipes':len(cases),'source_bytes':sum(len(c.source) for c in cases),'scheduled_calls':sum(len(c.schedule) for c in cases),
              'positive_blocks':len(kinds),'candidate_generations':sum(s['candidate_generations'] for s in summaries['normal']),
              'block_kinds':{k:kinds.count(i) for i,k in enumerate(('raw','rle','compressed'))},
              'discarded_repeat_witnesses':{k:rollback.count(i) for i,k in enumerate(('raw','rle'))},
              'complete_frame_decodes_per_build':len(cases)+sum(c.whole_compare for c in cases),'decoded_bytes_per_build':decoded_bytes,
              'incomplete_headers_per_build':len(generator.HEADERS),'negative_rejections_per_build':35,'commands':len(commands),
              'collector':False,'public_parameter_resolution':False}
    return coverage, accepted, summaries['normal']


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory',type=Path)
    args=parser.parse_args()
    coverage,_,_=audit(args.directory)
    print(json.dumps(coverage,sort_keys=True))


if __name__=='__main__':
    main()

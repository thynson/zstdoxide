"""Explicit external C regeneration only; imported by the fixture CLI on request."""
from pathlib import Path
import hashlib
import json
import os
import platform
import shutil
import subprocess
import time
from fast_frame_publication_support import *

ROOT = Path(__file__).resolve().parents[2]
HERE = Path.cwd()
COLLECTION = ROOT / "tests/upstream/fast-frame-publication"
INPUT_HASHES = None
ENV_KEYS = ("PATH","HOME","TMPDIR","TMP","TEMP","SYSTEMROOT","SDKROOT",
            "MACOSX_DEPLOYMENT_TARGET","DEVELOPER_DIR")


def file_sha(path):
    return sha(Path(path).read_bytes())


def write_json(path, value):
    Path(path).write_bytes(canonical(value))


def env():
    selected = {key: os.environ[key] for key in ENV_KEYS if key in os.environ}
    selected.update(LC_ALL="C", LANG="C", TZ="UTC")
    return selected


def source_inputs():
    return {
        'frame_oracle.c': file_sha(COLLECTION/'frame_oracle.c'),
        'live': file_sha(Path(__file__)),
        'support': file_sha(ROOT/'scripts/oracles/fast_frame_publication_support.py'),
        'importer': file_sha(ROOT/'scripts/import_fast_frame_publication.py'),
    }


def command(argv, directory, name, environment=None):
    need(INPUT_HASHES == source_inputs(), "helper/runner/design changed before command")
    directory.mkdir(parents=True, exist_ok=True)
    out, err = directory / (name + ".stdout"), directory / (name + ".stderr")
    need(not out.exists() and not err.exists(), f"refusing to overwrite command logs: {name}")
    effective = env() if environment is None else environment
    start = time.monotonic()
    with out.open("wb") as stdout, err.open("wb") as stderr:
        result = subprocess.run([str(x) for x in argv], stdout=stdout, stderr=stderr,
                                env=effective, cwd=HERE, check=False, timeout=120)
    record = dict(argv=[str(x) for x in argv], cwd=str(HERE), env=effective,
                  stdout=out.name, stderr=err.name, exit_code=result.returncode,
                  elapsed_seconds=time.monotonic()-start,
                  stdout_sha256=file_sha(out), stderr_sha256=file_sha(err))
    write_json(directory / (name + ".command.json"), record)
    need(INPUT_HASHES == source_inputs(), "helper/runner/design changed after command")
    need(result.returncode == 0, f"command failed ({result.returncode}): {argv}; logs {out}, {err}")
    return out


def git(reference, *arguments):
    return subprocess.check_output(["git", "-C", str(reference), *arguments], env=env())


def reference_proof(reference):
    need(git(reference, "rev-parse", "HEAD").decode().strip() == REVISION, "wrong pinned revision")
    need(not git(reference, "status", "--porcelain", "--untracked-files=no"), "modified tracked upstream files")
    paths = git(reference, "ls-files", "lib").decode().splitlines()
    hashes = {}
    for relative in paths:
        p = reference / relative
        if p.is_file() and (p.suffix in ('.c', '.h', '.S', '.mk') or p.name == 'Makefile'):
            data = p.read_bytes()
            need(data == git(reference, "show", f"{REVISION}:{relative}"), f"pinned source mismatch: {relative}")
            hashes[relative] = sha(data)
    required = {'lib/compress/zstd_fast.c', 'lib/compress/zstd_compress.c',
                'lib/compress/zstd_compress_internal.h', 'lib/common/zstd_internal.h',
                'lib/common/huf.h', 'lib/common/fse.h', 'lib/zstd.h', 'lib/libzstd.mk'}
    need(required <= set(hashes), "missing required pinned source inputs")
    return dict(revision=REVISION, tracked_clean=True, source_sha256=hashes,
                archive_sha256=file_sha(reference / "lib/libzstd.a"))


def build(reference, destination, mode, compiler):
    need(not destination.exists(), "build destination must be fresh")
    destination.mkdir(parents=True)
    proof=reference_proof(reference)
    cc=shutil.which(compiler)
    need(cc is not None, "compiler unavailable")
    version=command([cc,'--version'],destination,'compiler-version').read_text()
    target=command([cc,'-dumpmachine'],destination,'compiler-target').read_text()
    command([cc,'-dM','-E','-x','c','/dev/null'],destination,'compiler-predefines')
    common=[cc,'-std=c11','-O2','-g','-I',str(reference/'lib'),'-I',str(reference/'lib/common'),'-I',str(reference/'lib/compress')]
    executable=destination/'frame_oracle'
    if mode=='normal':
        argv=common+['-Wall','-Wextra','-Werror','-Wno-deprecated-declarations',str(COLLECTION/'frame_oracle.c'),str(reference/'lib/libzstd.a'),'-o',str(executable)]
        command(argv,destination,'compile-normal')
        instrumented=[]
    else:
        sources=sorted(p for folder in ('common','compress','decompress') for p in (reference/'lib'/folder).glob('*.c'))
        need(len(sources)>=25, "complete used sanitizer source list")
        flags=['-DZSTD_DISABLE_ASM','-DZSTD_LEGACY_SUPPORT=0','-DXXH_NAMESPACE=ZSTD_',
               '-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer']
        objects=[]
        for index,source in enumerate(sources):
            obj=destination/f'upstream-{index:02d}.o'
            command(common+flags+['-c',str(source),'-o',str(obj)],destination,f'compile-upstream-{index:02d}')
            objects.append(obj)
        command(common+flags+['-Wall','-Wextra','-Werror','-Wno-deprecated-declarations',str(COLLECTION/'frame_oracle.c')]+objects+['-o',str(executable)],destination,'link-sanitized')
        instrumented=[str(p.relative_to(reference)) for p in sources]
    effective_flags = [] if mode=='normal' else flags
    dep=destination/'dependencies.d'
    command(common+effective_flags+['-MM','-MF',str(dep),str(COLLECTION/'frame_oracle.c')],destination,'dependencies')
    command(common+effective_flags+['-dM','-E',str(COLLECTION/'frame_oracle.c')],destination,'effective-predefines')
    dependency_hashes={}
    dependency_text=dep.read_text().replace('\\\n',' ')
    for name in dependency_text.split(':',1)[1].split():
        dependency=Path(name).resolve()
        if dependency==COLLECTION/'frame_oracle.c':
            need(file_sha(dependency)==INPUT_HASHES['frame_oracle.c'], 'helper dependency changed')
        else:
            need(dependency.is_relative_to(reference), 'unexpected non-system dependency')
            relative=str(dependency.relative_to(reference))
            need(relative in proof['source_sha256'] and file_sha(dependency)==proof['source_sha256'][relative],
                 'untracked/changed included reference header')
        dependency_hashes[str(dependency)]=file_sha(dependency)
    need(proof==reference_proof(reference), "reference changed during build")
    evidence=dict(mode=mode,reference=proof,compiler=cc,compiler_sha256=file_sha(cc),compiler_version=version,target=target,
                  leak_sanitizer=False,source_inputs=INPUT_HASHES,
                  helper_sha256=file_sha(COLLECTION/'frame_oracle.c'),runner_sha256=file_sha(__file__),
                  design_sha256=file_sha(ROOT/'docs/reference-fast-oracle-design.md'),executable_sha256=file_sha(executable),
                  instrumented_sources=instrumented,dependency_file_sha256=file_sha(dep),dependency_sha256=dependency_hashes,
                  scope='unchanged ordinary frame mode, not sequence collector')
    write_json(destination/'build.json',evidence)


def capture(reference, build_dir, destination):
    need(not destination.exists(), "capture destination must be fresh")
    build_info=parse_json((build_dir/'build.json').read_text())
    need(build_info['helper_sha256']==file_sha(COLLECTION/'frame_oracle.c') and build_info['runner_sha256']==file_sha(__file__), "build source changed")
    need(build_info['reference']==reference_proof(reference), "build reference changed")
    executable=build_dir/'frame_oracle'
    need(build_info['executable_sha256']==file_sha(executable), "executable changed")
    destination.mkdir(parents=True)
    artifacts=destination/'artifacts'; artifacts.mkdir()
    logs=destination/'logs'
    rows=recipes(); envelope=[]
    for row in rows:
        data=row['source']; metadata={k:v for k,v in row.items() if k!='source'}
        metadata.update(source_size=len(data),source_sha256=sha(data))
        source_path=artifacts/(row['id']+'.source'); source_path.write_bytes(data)
        write_json(artifacts/(row['id']+'.recipe.json'),metadata); envelope.append(metadata)
    write_json(artifacts/'recipes.json',envelope)
    initial={p.name:file_sha(p) for p in artifacts.iterdir()}
    observations=[]
    run_env=env()
    if build_info['mode']=='sanitized':
        run_env.update(ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1')
    for row in rows:
        name=row['id']; output=artifacts/(name+'.zst'); whole=artifacts/(name+'.whole.zst')
        schedule=','.join(str(n) for n in row['schedule']) or '-'
        argv=[executable,'frame',name,artifacts/(name+'.source'),schedule,output,
              whole if row['whole_compare'] else '-',row['window'],row['hash'],row['min_match'],row['target']]
        log=command(argv,logs,name,run_env)
        need(log.stat().st_size<=32<<20, "case record cap")
        records=[parse_json(line) for line in log.read_text().splitlines()]
        summary=validate_case(row,records,output.read_bytes())
        write_json(artifacts/(name+'.records.json'),records)
        observations.append(summary)
    for index,(pledge,window) in enumerate(HEADERS):
        log=command([executable,'header',pledge,window],logs,f'header-{index:02d}',run_env)
        records=[parse_json(line) for line in log.read_text().splitlines()]
        need(len(records)==2 and records[0]['type']=='header' and records[1]['type']=='partial', "partial header records")
        need(records[0]['pledge']==pledge and records[0]['window_log']==window and records[1]['completed_frame'] is False and records[1]['decoded'] is False, "partial header scope")
        need(records[1]['pointer_bits'] in (32,64) and records[1]['int_bits']==32, "capture host width scope")
        write_json(artifacts/f'header-{index:02d}.json',records)
    need(all(file_sha(artifacts/name)==value for name,value in initial.items()), "recipe inputs changed during run")
    need(build_info['reference']==reference_proof(reference), "reference changed during capture")
    need(len(observations)==23 and len(HEADERS)==11, "positive exact case count")
    summary=dict(full_frame_cases=len(observations),partial_header_cases=len(HEADERS),
                 decoded_bytes=sum(row['decoded_bytes'] for row in observations),
                 whole_buffer_decodes=sum(row['whole_compare'] for row in observations),cases=observations,
                 qualifications=['candidate outcomes observed; recipes not adapted after capture',
                                 'no FSE valid-tag branch claim without observed valid tag',
                                 'whole-buffer equality only explicitly aligned cases',
                                 'Fast raw repeat cell 2 differs from authoritative semantic triple'])
    write_json(artifacts/'summary.json',summary)
    evidence=dict(build=build_info,host=dict(system=platform.system(),machine=platform.machine()),
                  runner_sha256=file_sha(__file__),helper_sha256=file_sha(COLLECTION/'frame_oracle.c'),
                  artifacts={p.name:dict(size=p.stat().st_size,sha256=file_sha(p)) for p in sorted(artifacts.iterdir())},
                  logs={p.name:dict(size=p.stat().st_size,sha256=file_sha(p)) for p in sorted(logs.iterdir())},summary=summary)
    write_json(destination/'capture.json',evidence)


def compare(original, regenerated):
    left=parse_json((original/'capture.json').read_text())
    right=parse_json((regenerated/'capture.json').read_text())
    for directory,evidence in ((original,left),(regenerated,right)):
        for group in ('artifacts','logs'):
            need(set(evidence[group])=={p.name for p in (directory/group).iterdir()}, "captured file inventory changed")
            for name,item in evidence[group].items():
                path=directory/group/name
                need(path.stat().st_size==item['size'] and file_sha(path)==item['sha256'], "captured artifact hash mismatch")
    need(left['artifacts']==right['artifacts'], "deterministic observations differ")
    need(left['summary']['full_frame_cases']==right['summary']['full_frame_cases']==23
         and left['summary']['partial_header_cases']==right['summary']['partial_header_cases']==11, "complete nonzero comparison")
    print(json.dumps(dict(equal=True,full_frame_cases=23,partial_header_cases=11,
                          original=str(original),regenerated=str(regenerated)),sort_keys=True))

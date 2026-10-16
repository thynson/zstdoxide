"""Immutable predecessor requests and source-only alternative branch witnesses."""
import json
from pathlib import Path
from .optimal_path_recipes import recipes as small
from .optimal_path_wide_recipes import recipes as wide
from .optimal_paths_wide import Request as Original
from .ultra_paths import Request
from .ultra_scalar import sha

def originals():
    result=[]
    for kind,cases in [(0,small()),(1,wide())]:
        for name,old in cases.items():result.append((name,Original(old.source,old.scripts,old.min_match,old.target,old.literal_mode),kind))
    return result

def recipes():
    cases={}
    for policy in (0,2):
        for name,original,_ in originals():cases[f'up_p{policy}_'+name[5:]]=Request(policy,original)
        raw=json.loads(Path(__file__).with_name('ultra_path_script_inputs.json').read_text())
        for row in raw['cases']:
            scripts=tuple((pos,tuple(tuple(pair)for pair in candidates))for pos,candidates in row['scripts'])
            original=Original(b'a'*row['source_bytes'],scripts,row['min_match'],row['target'],row['literal_mode'])
            cases[f'up_p{policy}_'+row['name']]=Request(policy,original)
    return cases

def manifest():
    result=[]
    for name,request in recipes().items():
        request.validate();original=request.original
        result.append(dict(id=name,policy=request.policy,packet_sha256=sha(request.packet()),original_packet_sha256=sha(original.packet()),source_sha256=sha(original.source),source_bytes=len(original.source),min_match=original.min_match,target=original.target,literal_mode=original.literal_mode,scripts=[[pos,[list(pair)for pair in candidates]]for pos,candidates in original.scripts]))
    return dict(schema_version=1,recipes=result,qualification='Source-only coupled policy wrappers; original27requests unchanged, additional alternative branches chosen before native execution.')

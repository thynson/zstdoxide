"""Source-only wider ordinary block and horizon publication hypotheses."""
from dataclasses import dataclass
from .optimal_retained_wide import Request,require

@dataclass(frozen=True)
class Recipe:
    request:Request
    kinds:tuple


def noise(size,seed=0x516aa393):
    state=seed;out=bytearray()
    for _ in range(size):
        state^=(state<<13)&0xffffffff;state^=state>>17;state^=(state<<5)&0xffffffff
        out.append(state&255)
    return bytes(out)


def recipes():
    history=b'a'*2050+b'b'*2045+b'b'*2045+b'c'*100+b'd'*1952
    current=b'a'*2050+b'b'*2045+b'c'*100+b'e'*3997
    require(len(history)==len(current)==8192,'exact horizon blocks')
    anchor=noise(4095,1)
    horizon_history=anchor+anchor[2050:]+b'Z'*100+b'D'*1952
    horizon_current=anchor+b'Z'*100+b'E'*3997
    random=noise(8192)
    short=(b'abcXabcYabcZabcd'*1093)[:16384]
    cases={
        'wide_uniform':Recipe(Request(b'a'*16384,(8192,8192),window_log=14,hash_log=12,chain_log=12,search_log=6,target=4095),(2,1)),
        'wide_noise_copy':Recipe(Request(random*2,(8192,8192),window_log=14,hash_log=12,chain_log=12,search_log=6,target=4095),(0,2)),
        'wide_runs_disabled':Recipe(Request(history+current,(8192,8192),window_log=14,hash_log=12,chain_log=12,search_log=6,target=4095,literal_mode=1),(2,2)),
        'wide_runs_priced':Recipe(Request(history+current,(8192,8192),window_log=14,hash_log=12,chain_log=12,search_log=6,target=4095),(2,2)),
        'wide_horizon':Recipe(Request(horizon_history+horizon_current,(8192,8192),window_log=14,hash_log=12,chain_log=12,search_log=6,target=4095,literal_mode=1),(2,2)),
        'wide_short':Recipe(Request(short,(8192,8192),window_log=14,hash_log=12,chain_log=12,search_log=6,min_match=3,target=4095),(2,2)),
        'wide_window16':Recipe(Request((b'abcdEFGH'*2048),(4096,4096,4096,4096),window_log=16,hash_log=12,chain_log=12,search_log=6,min_match=3,target=4095),(2,2,2,2)),
    }
    for recipe in cases.values():recipe.request.validate()
    return cases

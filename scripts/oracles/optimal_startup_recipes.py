"""Deterministic fresh ordinary hypotheses frozen before any new C execution."""
from .optimal_startup import Request, require


def recipes():
    random=0x5eed1014
    noise=bytearray()
    for _ in range(512):
        random=(random*1664525+1013904223)&0xffffffff
        noise.append(random>>24)
    periodic=(b'abcdefghijklmno!'+bytes(noise[:16]))*16
    mixed=bytes(noise[:64])+b'abcd'*24+bytes(noise[64:128])+b'abcd'*24+bytes(noise[128:192])+b'abcd'*32
    require(len(mixed)==512,'fixed mixed-source extent')
    result={
        'startup_uniform':Request(b'a'*64),
        'startup_noise':Request(bytes(noise)),
        'startup_periodic':Request(periodic),
        'startup_mixed':Request(mixed,target=4095),
        'startup_disabled_literals':Request(mixed,target=4095,literal_mode=1),
        'startup_three_short':Request((b'abcXabcYabcZabcd'*35)[:512],min_match=3,target=4095),
        'startup_larger_geometry':Request(mixed,window_log=12,hash_log=9,chain_log=9,search_log=4,min_match=3,target=16),
    }
    for minimum in range(3,8):
        result['startup_minimum_'+str(minimum)]=Request(periodic,min_match=minimum,target=4095)
    return result

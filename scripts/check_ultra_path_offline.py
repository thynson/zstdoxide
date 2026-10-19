#!/usr/bin/env python3
"""Read-only complete scripted Ultra reconstruction using the frozen auditor."""
import json
import sys
sys.dont_write_bytecode = True
import check_ultra_path_fixture as fixture


def main():
    children = []

    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.fork', 'os.posix_spawn', 'os.exec', 'os.spawn'):
            children.append(event)
            raise ValueError('child execution forbidden')

    sys.addaudithook(guard)
    data = (fixture.ROOT / 'tests/fixtures/reference-ultra-paths/cases.bin').read_bytes()
    expected, layouts = fixture.reconstruct()
    fixture.check(data, expected)
    fixture.need(not children, 'read-only no-child reconstruction')
    print(json.dumps(dict(status='passed', cases=len(layouts), bytes=len(data),
                          sha256=fixture.sha(data), child_attempts=len(children)), sort_keys=True))


if __name__ == '__main__':
    main()

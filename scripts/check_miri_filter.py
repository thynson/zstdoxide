#!/usr/bin/env python3
"""Run a focused Miri check and reject filters that execute no passing tests."""

import re
import subprocess
import sys


def main():
    if len(sys.argv) == 1:
        print("usage: check_miri_filter.py CARGO_TEST_ARGUMENTS...", file=sys.stderr)
        return 2
    result = subprocess.run(
        ["cargo", "+nightly", "miri", "test", *sys.argv[1:]],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        errors="replace",
        check=False,
    )
    print(result.stdout, end="", flush=True)
    if result.returncode:
        return result.returncode
    passed = sum(
        int(count)
        for count in re.findall(r"test result: ok\. (\d+) passed;", result.stdout)
    )
    if passed == 0:
        print("ERROR: Miri filter executed no passing tests; check the selector.", file=sys.stderr)
        return 1
    print(f"Verified {passed} passing Miri test(s) for the requested filter.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

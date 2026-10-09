/* Copyright (c) zstdoxide contributors.
 * Independent scalar-depth oracle for facebook/zstd v1.5.7.
 * The adjusted-depth contract derives from tests/fuzz/huf_round_trip.c;
 * select the BSD option in LICENSE.zstd. No Rust implementation links to C.
 */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "common/huf.h"
#include "common/bits.h"
#include "zstd.h"

static int read_u32(uint32_t* result) {
    unsigned char bytes[4];
    if (fread(bytes, 1, 4, stdin) != 4) return 0;
    *result = (uint32_t)bytes[0] | (uint32_t)bytes[1] << 8 |
              (uint32_t)bytes[2] << 16 | (uint32_t)bytes[3] << 24;
    return 1;
}

static int write_u32(uint32_t value) {
    unsigned char bytes[4];
    for (unsigned i = 0; i < 4; ++i) bytes[i] = (unsigned char)(value >> (8 * i));
    return fwrite(bytes, 1, 4, stdout) == 4;
}

int main(void) {
    char magic[8];
    uint32_t cases;
    if (ZSTD_versionNumber() != 10507 || fread(magic, 1, 8, stdin) != 8 ||
        memcmp(magic, "ZHDEP001", 8) || !read_u32(&cases) || cases > 4096) return 2;
    if (fwrite("ZHDOUT01", 1, 8, stdout) != 8 || !write_u32(cases)) return 3;
    for (uint32_t test = 0; test < cases; ++test) {
        uint32_t requested;
        unsigned counts[256] = {0}, highest = 0, cardinality = 0, total = 0;
        uint64_t workspace[HUF_WORKSPACE_SIZE_U64];
        HUF_CElt table[HUF_CTABLE_SIZE_ST(255)];
        if (!read_u32(&requested) || requested < 1 || requested > 12) return 4;
        for (unsigned symbol = 0; symbol < 256; ++symbol) {
            uint32_t count;
            if (!read_u32(&count) || count > 262144 - total) return 5;
            counts[symbol] = count;
            total += count;
            if (count) { highest = symbol; ++cardinality; }
        }
        if (total < 2 || cardinality < 2) return 6;
        /* The target uses the highest symbol, including holes in the alphabet. */
        unsigned const alphabet = highest + 1;
        unsigned minimum = ZSTD_highbit32(alphabet) + 1;
        if (alphabet & (alphabet - 1)) ++minimum;
        unsigned const adjusted = requested < minimum ? minimum : requested;
        unsigned const cheap = HUF_optimalTableLog(adjusted, total, highest,
            workspace, sizeof(workspace), table, counts, 0);
        if (cheap < 1 || cheap > 12 || !write_u32(adjusted) || !write_u32(cheap)) return 7;
    }
    return ferror(stdin) || fgetc(stdin) != EOF || ferror(stdout);
}

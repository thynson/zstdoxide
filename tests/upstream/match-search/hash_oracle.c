/*
 * Test-only observations of facebook/zstd v1.5.7 hash pointer wrappers.
 * Copyright (c) zstdoxide contributors.
 * The included upstream definitions retain Meta's BSD-3-Clause/GPLv2 notices;
 * this work selects the BSD-3-Clause option; see LICENSE.zstd at the project
 * root. No hash algorithm is copied here.
 */
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "zstd_compress_internal.h"

#define PATTERNS 64
_Static_assert(CHAR_BIT == 8, "eight-bit bytes required");
_Static_assert(sizeof(U32) == 4, "32-bit U32 required");
_Static_assert(sizeof(U64) == 8, "64-bit U64 required");

static void check(int condition, const char* message)
{
    if (!condition) {
        fprintf(stderr, "hash oracle: %s\n", message);
        exit(1);
    }
}

/* Every caller passes eight initialized readable bytes. In particular, widths
 * 5..7 may physically read all eight bytes in the unchanged C pointer wrappers.
 * This does not require a later Rust implementation to overread its key slice.
 */
static size_t observe(const unsigned char bytes[8], unsigned width, unsigned log)
{
    size_t direct;
    size_t generic;
    size_t salted;
    check(width >= 4 && width <= 8, "width outside campaign");
    check(log >= 10 && log <= 18, "log outside campaign");
    switch (width) {
    case 4: direct = ZSTD_hash4Ptr(bytes, log); break;
    case 5: direct = ZSTD_hash5Ptr(bytes, log); break;
    case 6: direct = ZSTD_hash6Ptr(bytes, log); break;
    case 7: direct = ZSTD_hash7Ptr(bytes, log); break;
    case 8: direct = ZSTD_hash8Ptr(bytes, log); break;
    default: exit(1);
    }
    generic = ZSTD_hashPtr(bytes, log, width);
    salted = ZSTD_hashPtrSalted(bytes, log, width, 0);
    check(direct == generic, "direct and generic wrappers disagree");
    check(direct == salted, "zero-salt and unsalted wrappers disagree");
    check(direct < ((size_t)1 << log), "bucket outside requested bits");
    return direct;
}

static void hex8(const unsigned char bytes[8])
{
    unsigned i;
    for (i = 0; i < 8; ++i) printf("%02x", bytes[i]);
}

int main(int argc, char** argv)
{
    unsigned char patterns[PATTERNS][8] = {{0}};
    unsigned width, log, id;
    FILE* input;
    check(ZSTD_versionNumber() == 10507, "linked archive is not v1.5.7");
    if (argc == 2 && strcmp(argv[1], "--meta") == 0) {
        printf("version\tsize_t_bits\tint_bits\tbyte_order\n");
        printf("%s\t%u\t%u\t%s\n", ZSTD_versionString(),
               (unsigned)(sizeof(size_t) * CHAR_BIT),
               (unsigned)(sizeof(int) * CHAR_BIT),
               MEM_isLittleEndian() ? "little" : "big");
        return 0;
    }
    check(argc == 3, "usage: oracle --vectors|--invariance patterns.bin");
    input = fopen(argv[2], "rb");
    check(input != NULL, "cannot open pattern file");
    check(fread(patterns, 1, sizeof(patterns), input) == sizeof(patterns),
          "pattern file is not 64 complete eight-byte records");
    check(fgetc(input) == EOF && !ferror(input), "unexpected pattern suffix/read error");
    check(fclose(input) == 0, "cannot close pattern file");
    if (strcmp(argv[1], "--vectors") == 0) {
        printf("width\tlog\tpattern\tbytes\tbucket\n");
        for (width = 4; width <= 8; ++width) {
            for (log = 10; log <= 18; ++log) {
                for (id = 0; id < PATTERNS; ++id) {
                    size_t const bucket = observe(patterns[id], width, log);
                    printf("%u\t%u\t%u\t", width, log, id);
                    hex8(patterns[id]);
                    printf("\t%zu\n", bucket);
                }
            }
        }
    } else if (strcmp(argv[1], "--invariance") == 0) {
        printf("width\tlog\tpattern\tbase\tchanged\tbase_bucket\tchanged_bucket\tfull8_base\tfull8_changed\n");
        for (width = 4; width <= 7; ++width) {
            for (log = 10; log <= 18; ++log) {
                for (id = 0; id < PATTERNS; ++id) {
                    unsigned char changed[8];
                    size_t first, second;
                    unsigned byte;
                    memcpy(changed, patterns[id], sizeof(changed));
                    for (byte = width; byte < 8; ++byte) changed[byte] ^= 0xff;
                    first = observe(patterns[id], width, log);
                    second = observe(changed, width, log);
                    check(first == second, "unused high bytes affected selected-width hash");
                    printf("%u\t%u\t%u\t", width, log, id);
                    hex8(patterns[id]);
                    printf("\t");
                    hex8(changed);
                    printf("\t%zu\t%zu\t%zu\t%zu\n", first, second,
                           observe(patterns[id], 8, log), observe(changed, 8, log));
                }
            }
        }
    } else {
        check(0, "unknown mode");
    }
    check(!ferror(stdout), "cannot write observations");
    return 0;
}

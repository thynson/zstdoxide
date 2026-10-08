/* Copyright (c) Meta Platforms, Inc. and affiliates.
 * Copyright (c) zstdoxide contributors.
 * Adapted from facebook/zstd v1.5.7 tests/zstreamtest.c:1652 under BSD-3-Clause.
 * This generator is an external test oracle, never linked into the Rust codec.
 */
#define ZSTD_STATIC_LINKING_ONLY
#include "zstd.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void checked(size_t result) {
    if (ZSTD_isError(result)) {
        fprintf(stderr, "%s\n", ZSTD_getErrorName(result));
        exit(1);
    }
}
static uint32_t random_word(uint32_t* state) {
    uint32_t value = *state;
    value *= 2654435761U;
    value += 2246822519U;
    value = (value << 13) | (value >> 19);
    *state = value;
    return value >> 5;
}
int main(int argc, char** argv) {
    unsigned log;
    size_t const count = 10000;
    size_t size, encoded_size, total;
    size_t i;
    uint32_t seed = 0x12345678U;
    char* source;
    unsigned char* encoded;
    unsigned char output[65536];
    ZSTD_Sequence* sequences;
    ZSTD_CCtx* encoder;
    ZSTD_DCtx* decoder;
    ZSTD_inBuffer input;
    if (argc != 2) return 2;
    log = (unsigned)strtoul(argv[1], NULL, 10);
    if (log < 10 || log > 29) return 2;
    size = ((size_t)1 << log) + 10 * (count - 1);
    source = (char*)calloc(size, 1);
    encoded = (unsigned char*)malloc(1 << 20);
    sequences = (ZSTD_Sequence*)calloc(count, sizeof(*sequences));
    encoder = ZSTD_createCCtx();
    decoder = ZSTD_createDCtx();
    if (!source || !encoded || !sequences || !encoder || !decoder) return 1;
    sequences[0].offset = 32;
    sequences[0].litLength = 32;
    sequences[0].matchLength = (1U << log) - 32;
    for (i = 1; i < count; ++i) {
        sequences[i].offset = (1U << log) - random_word(&seed) % 8;
        sequences[i].litLength = random_word(&seed) & 7;
        sequences[i].matchLength = 10 - sequences[i].litLength;
    }
    checked(ZSTD_CCtx_setParameter(encoder, ZSTD_c_checksumFlag, 1));
    checked(ZSTD_CCtx_setParameter(encoder, ZSTD_c_minMatch, 3));
    checked(ZSTD_CCtx_setParameter(encoder, ZSTD_c_validateSequences, 1));
    checked(ZSTD_CCtx_setParameter(encoder, ZSTD_c_windowLog, (int)log));
    encoded_size = ZSTD_compressSequences(encoder, encoded, 1 << 20, sequences, count, source, size);
    checked(encoded_size);
    checked(ZSTD_DCtx_setParameter(decoder, ZSTD_d_windowLogMax, (int)log));
    input.src = encoded; input.size = encoded_size; input.pos = 0;
    total = 0;
    for (;;) {
        ZSTD_outBuffer out = {output, sizeof(output), 0};
        size_t result = ZSTD_decompressStream(decoder, &out, &input);
        checked(result);
        if (out.pos > size - total || memcmp(output, source + total, out.pos)) return 1;
        total += out.pos;
        if (!result) break;
        if (!out.pos && input.pos == input.size) return 1;
    }
    if (total != size || input.pos != input.size) return 1;
    if (fwrite(encoded, 1, encoded_size, stdout) != encoded_size) return 1;
    ZSTD_freeCCtx(encoder); ZSTD_freeDCtx(decoder);
    free(source); free(encoded); free(sequences);
    return 0;
}

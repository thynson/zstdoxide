/* Copyright (c) Meta Platforms, Inc. and affiliates.
 * Copyright (c) zstdoxide contributors.
 * Derived selector order: facebook/zstd v1.5.7 tests/fuzz/simple_round_trip.c
 * :145-159, :61, :79, :93. BSD-3-Clause option; see root LICENSE.zstd.
 * Compile with unchanged external fuzz_data_producer.c and fuzz_helpers.c.
 * No compression, decompression, context mutation or advanced branch replay.
 *
 * Input: "ZRFZIN01", LE u32 count=81, then (LE u32 length, initialized bytes).
 * Output: "ZRFZOUT1", LE u32 count,size_t bits,int bits,version; then twelve
 * LE u32 fields per case: N,prefix,remaining,bound,reduction,remaining,branch,
 * remaining,level bits,remaining,decoder toggle,remaining. For advanced cases
 * level is INT32_MIN and the final three fields are UINT32_MAX (unconsumed).
 */
#include "fuzz_data_producer.h"
#include "zstd.h"
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CASES 81u
#define INPUT_LIMIT (1024u * 1024u)
#define BATCH_LIMIT (16u * INPUT_LIMIT)
#define GUARD 64u
/* Exact constants from tests/fuzz/zstd_helpers.c:23-24; importer verifies them. */
static const int minimum_level = -3;
static const int maximum_level = 19;

static void require(int condition, const char* message) {
    if (!condition) { fprintf(stderr, "selector oracle: %s\n", message); exit(1); }
}
static void read_bytes(void* data, size_t size) {
    require(fread(data, 1, size, stdin) == size, "truncated input");
}
static uint32_t read_number(void) {
    unsigned char b[4];
    read_bytes(b, sizeof(b));
    return (uint32_t)b[0] | (uint32_t)b[1] << 8 | (uint32_t)b[2] << 16 | (uint32_t)b[3] << 24;
}
static void write_number(uint32_t value) {
    unsigned char b[4];
    unsigned i;
    for (i = 0; i < 4; ++i) b[i] = (unsigned char)(value >> (8 * i));
    require(fwrite(b, 1, sizeof(b), stdout) == sizeof(b), "output write");
}
int main(void) {
    unsigned char magic[8];
    uint32_t count, index, total = 0;
    require(CHAR_BIT == 8 && sizeof(int) == 4 && (sizeof(size_t) == 4 || sizeof(size_t) == 8), "supported scalar widths");
    require(ZSTD_versionNumber() == 10507, "pinned library version");
    read_bytes(magic, sizeof(magic));
    require(memcmp(magic, "ZRFZIN01", 8) == 0, "input magic");
    count = read_number();
    require(count == CASES, "exact 81 cases");
    require(fwrite("ZRFZOUT1", 1, 8, stdout) == 8, "output magic");
    write_number(count); write_number((uint32_t)(sizeof(size_t) * CHAR_BIT));
    write_number((uint32_t)(sizeof(int) * CHAR_BIT)); write_number(ZSTD_versionNumber());
    for (index = 0; index < count; ++index) {
        uint32_t values[12];
        uint32_t size = read_number(), i;
        size_t bound;
        unsigned char *allocation, *source;
        FUZZ_dataProducer_t* producer;
        require(size <= INPUT_LIMIT && size <= BATCH_LIMIT - total, "input/batch cap");
        total += size;
        allocation = (unsigned char*)malloc((size_t)size + 2 * GUARD);
        require(allocation != NULL, "input allocation");
        memset(allocation, 0xa7, (size_t)size + 2 * GUARD);
        source = allocation + GUARD;
        read_bytes(source, size);
        /* Original N determines both capacities before prefix reservation. */
        bound = ZSTD_compressBound(size);
        require(bound <= UINT32_MAX && bound > 1, "bounded compression bound");
        producer = FUZZ_dataProducer_create(source, size);
        values[0] = size;
        values[1] = (uint32_t)FUZZ_dataProducer_reserveDataPrefix(producer);
        values[2] = (uint32_t)FUZZ_dataProducer_remainingBytes(producer);
        values[3] = (uint32_t)bound;
        values[4] = FUZZ_dataProducer_uint32Range(producer, 0, 1);
        values[5] = (uint32_t)FUZZ_dataProducer_remainingBytes(producer);
        values[6] = FUZZ_dataProducer_uint32Range(producer, 0, 1);
        values[7] = (uint32_t)FUZZ_dataProducer_remainingBytes(producer);
        values[8] = UINT32_C(0x80000000);
        values[9] = values[10] = values[11] = UINT32_MAX;
        if (values[6] == 0) {
            values[8] = (uint32_t)FUZZ_dataProducer_int32Range(producer, minimum_level, maximum_level);
            values[9] = (uint32_t)FUZZ_dataProducer_remainingBytes(producer);
            values[10] = FUZZ_dataProducer_uint32Range(producer, 0, 1);
            values[11] = (uint32_t)FUZZ_dataProducer_remainingBytes(producer);
        }
        FUZZ_dataProducer_free(producer);
        for (i = 0; i < GUARD; ++i) {
            require(allocation[i] == 0xa7 && allocation[GUARD + size + i] == 0xa7, "input guards");
        }
        free(allocation);
        for (i = 0; i < 12; ++i) write_number(values[i]);
    }
    require(getchar() == EOF && !ferror(stdin), "extra input or read failure");
    require(fflush(stdout) == 0, "output flush");
    return 0;
}

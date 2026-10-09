/* Independent facebook/zstd v1.5.7 Huffman fuzz-selector oracle.
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 * Copyright (c) zstdoxide contributors.
 * Derived selector order: tests/fuzz/huf_round_trip.c:43-61.
 * Used under the BSD-3-Clause option; see LICENSE.zstd.
 *
 * Compile with the unchanged external fuzz_data_producer.c and fuzz_helpers.c.
 * This test-only executable performs no entropy compression or decompression.
 * Request: "ZHPRM001", LE u32 count, then count records containing LE u32
 * input length, LE u32 BMI availability (0/1), and the complete input bytes.
 * Response: "ZHPOUT01", LE u32 count, then twelve LE u32 values per record:
 * stream selector, decoder selector, BMI flag, optimal-depth, prefer-repeat,
 * suspect-uncompressible, disable-assembly, disable-fast, destination capacity,
 * requested table log, remaining bytes before capping, capped source length.
 * Exactly one bounded batch and EOF are required. BMI availability is explicit
 * so both original CPU-dependent byte-consumption paths run on every host.
 */
#include "fuzz_data_producer.h"
#include "huf.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define INPUT_LIMIT (1024u * 1024u)
#define SOURCE_LIMIT (256u * 1024u)
#define RECORD_LIMIT 4096u
#define BATCH_LIMIT (32u * 1024u * 1024u)

static void require(int condition, const char* message) {
    if (!condition) {
        fprintf(stderr, "%s\n", message);
        exit(1);
    }
}

static void read_bytes(void* output, size_t length) {
    require(fread(output, 1, length, stdin) == length, "truncated selector request");
}

static uint32_t read_number(void) {
    unsigned char bytes[4];
    read_bytes(bytes, sizeof(bytes));
    return (uint32_t)bytes[0] | (uint32_t)bytes[1] << 8 |
           (uint32_t)bytes[2] << 16 | (uint32_t)bytes[3] << 24;
}

static void write_number(uint32_t number) {
    unsigned char bytes[4];
    unsigned index;
    for (index = 0; index < 4; ++index)
        bytes[index] = (unsigned char)(number >> (8 * index));
    require(fwrite(bytes, 1, sizeof(bytes), stdout) == sizeof(bytes), "selector response write failed");
}

int main(void) {
    unsigned char magic[8];
    uint32_t count, index, total = 0;
    read_bytes(magic, sizeof(magic));
    require(memcmp(magic, "ZHPRM001", 8) == 0, "wrong selector request magic");
    count = read_number();
    require(count <= RECORD_LIMIT, "too many selector records");
    require(fwrite("ZHPOUT01", 1, 8, stdout) == 8, "selector response write failed");
    write_number(count);
    for (index = 0; index < count; ++index) {
        uint32_t values[12];
        uint32_t const size = read_number();
        uint32_t const bmi_available = read_number();
        unsigned field;
        int flags;
        unsigned char* source;
        FUZZ_dataProducer_t* producer;
        require(size <= INPUT_LIMIT, "selector input exceeds one MiB");
        require(size <= BATCH_LIMIT - total, "selector batch exceeds bound");
        require(bmi_available <= 1, "invalid BMI availability");
        total += size;
        source = (unsigned char*)malloc(size == 0 ? 1 : size);
        require(source != NULL, "selector allocation failed");
        read_bytes(source, size);
        producer = FUZZ_dataProducer_create(source, size);
        /* Keep these calls in the original target's order. In particular,
         * the unavailable-BMI path does not consume its conditional selector. */
        values[0] = (uint32_t)FUZZ_dataProducer_int32Range(producer, 0, 1);
        values[1] = (uint32_t)FUZZ_dataProducer_int32Range(producer, 0, 1);
        flags = 0
            | (bmi_available && FUZZ_dataProducer_int32Range(producer, 0, 1) ? HUF_flags_bmi2 : 0)
            | (FUZZ_dataProducer_int32Range(producer, 0, 1) ? HUF_flags_optimalDepth : 0)
            | (FUZZ_dataProducer_int32Range(producer, 0, 1) ? HUF_flags_preferRepeat : 0)
            | (FUZZ_dataProducer_int32Range(producer, 0, 1) ? HUF_flags_suspectUncompressible : 0)
            | (FUZZ_dataProducer_int32Range(producer, 0, 1) ? HUF_flags_disableAsm : 0)
            | (FUZZ_dataProducer_int32Range(producer, 0, 1) ? HUF_flags_disableFast : 0);
        values[2] = (flags & HUF_flags_bmi2) != 0;
        values[3] = (flags & HUF_flags_optimalDepth) != 0;
        values[4] = (flags & HUF_flags_preferRepeat) != 0;
        values[5] = (flags & HUF_flags_suspectUncompressible) != 0;
        values[6] = (flags & HUF_flags_disableAsm) != 0;
        values[7] = (flags & HUF_flags_disableFast) != 0;
        values[8] = FUZZ_dataProducer_uint32Range(producer, 0, 4 * size);
        values[9] = FUZZ_dataProducer_uint32Range(producer, 1, 12);
        values[10] = (uint32_t)FUZZ_dataProducer_remainingBytes(producer);
        values[11] = values[10] < SOURCE_LIMIT ? values[10] : SOURCE_LIMIT;
        for (field = 0; field < 12; ++field) write_number(values[field]);
        FUZZ_dataProducer_free(producer);
        free(source);
    }
    require(getchar() == EOF && !ferror(stdin), "extra selector request bytes");
    require(fflush(stdout) == 0, "selector response flush failed");
    return 0;
}

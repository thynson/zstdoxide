/* Bounded facebook/zstd v1.5.7 Huffman destination-capacity oracle.
 * Compiled in an external temporary directory for tests only; never linked
 * into the Rust implementation. Copyright (c) zstdoxide contributors.
 * BSD-3-Clause; see LICENSE. Upstream code uses LICENSE.zstd's BSD option.
 *
 * Request: "ZHCAP001", five LE u32 fields (description length, source length,
 * operation 0=write table/1=1X/4=4X, flags, capacity count), description bytes,
 * source bytes, then count LE u32 capacities. One request and EOF are required.
 * Response: "ZHCO0001", native word bytes, BMI2 availability, count; each record
 * contains capacity, outcome (0=zero,1=success,2=error), error code, result size,
 * modified destination bytes, and successful result bytes. Error/zero records
 * never publish partial bytes, but the modification count records C behavior.
 */
#define HUF_STATIC_LINKING_ONLY
#include "huf.h"
#include "zstd.h"
#include "zstd_errors.h"
#include "cpu.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define SOURCE_LIMIT (256u * 1024u)
#define DESTINATION_LIMIT (4u * 1024u * 1024u)
#define COUNT_LIMIT 128u
#define GUARD 16u

static void require(int condition, const char* message) {
    if (!condition) { fprintf(stderr, "%s\n", message); exit(1); }
}

static void read_bytes(void* output, size_t size) {
    require(fread(output, 1, size, stdin) == size, "truncated request");
}

static uint32_t read_number(void) {
    unsigned char bytes[4];
    read_bytes(bytes, sizeof(bytes));
    return (uint32_t)bytes[0] | (uint32_t)bytes[1] << 8 |
           (uint32_t)bytes[2] << 16 | (uint32_t)bytes[3] << 24;
}

static void write_bytes(const void* input, size_t size) {
    require(fwrite(input, 1, size, stdout) == size, "response write failed");
}

static void write_number(uint32_t value) {
    unsigned char bytes[4];
    unsigned index;
    for (index = 0; index < 4; ++index) bytes[index] = (unsigned char)(value >> (8 * index));
    write_bytes(bytes, sizeof(bytes));
}

int main(void) {
    unsigned char magic[8], description[512];
    HUF_CElt table[HUF_CTABLE_SIZE_ST(255)];
    unsigned workspace[HUF_WORKSPACE_SIZE / sizeof(unsigned)];
    uint32_t capacities[COUNT_LIMIT];
    uint32_t description_size, source_size, operation, flags, count, index;
    unsigned maximum = 255, zeros = 0;
    int const bmi2 = ZSTD_cpuid_bmi2(ZSTD_cpuid()) != 0;
    size_t consumed, capacity_max = 0;
    unsigned char *source, *guarded;
    require(ZSTD_versionNumber() == 10507, "expected zstd v1.5.7");
    read_bytes(magic, sizeof(magic));
    require(memcmp(magic, "ZHCAP001", 8) == 0, "request magic");
    description_size = read_number(); source_size = read_number();
    operation = read_number(); flags = read_number(); count = read_number();
    require(description_size > 0 && description_size <= sizeof(description), "description bound");
    require(source_size <= SOURCE_LIMIT, "source bound");
    require(operation == 0 || operation == 1 || operation == 4, "operation");
    require(flags <= 63 && (!(flags & HUF_flags_bmi2) || bmi2), "unsupported flags");
    require(count > 0 && count <= COUNT_LIMIT, "capacity count bound");
    source = (unsigned char*)malloc(source_size ? source_size : 1);
    require(source != NULL, "source allocation");
    read_bytes(description, description_size); read_bytes(source, source_size);
    for (index = 0; index < count; ++index) {
        capacities[index] = read_number();
        require(capacities[index] <= DESTINATION_LIMIT, "destination bound");
        if (capacities[index] > capacity_max) capacity_max = capacities[index];
    }
    require(fgetc(stdin) == EOF && !ferror(stdin), "trailing input or read error");
    memset(table, 0, sizeof(table));
    consumed = HUF_readCTable(table, &maximum, description, description_size, &zeros);
    require(!HUF_isError(consumed) && consumed == description_size, "invalid complete table description");
    require(HUF_readCTableHeader(table).tableLog <= 12, "table log bound");
    for (index = 0; index < source_size; ++index)
        require(HUF_getNbBitsFromCTable(table, source[index]) != 0, "source symbol absent");
    guarded = (unsigned char*)malloc(capacity_max + 2 * GUARD);
    require(guarded != NULL, "destination allocation");
    write_bytes("ZHCO0001", 8); write_number((uint32_t)sizeof(size_t));
    write_number((uint32_t)bmi2); write_number(count);
    for (index = 0; index < count; ++index) {
        size_t const capacity = capacities[index];
        unsigned char* const output = guarded + GUARD;
        size_t result, byte, modified = 0;
        uint32_t outcome, error = 0, size = 0;
        memset(guarded, 0xa5, capacity + 2 * GUARD);
        if (operation == 0) {
            result = HUF_writeCTable_wksp(output, capacity, table, maximum,
                HUF_readCTableHeader(table).tableLog, workspace, sizeof(workspace));
        } else if (operation == 1) {
            result = HUF_compress1X_usingCTable(output, capacity, source, source_size, table, (int)flags);
        } else {
            result = HUF_compress4X_usingCTable(output, capacity, source, source_size, table, (int)flags);
        }
        for (byte = 0; byte < GUARD; ++byte)
            require(guarded[byte] == 0xa5 && output[capacity + byte] == 0xa5, "C wrote outside destination");
        for (byte = 0; byte < capacity; ++byte) modified += output[byte] != 0xa5;
        if (HUF_isError(result)) { outcome = 2; error = (uint32_t)ZSTD_getErrorCode(result); }
        else if (result == 0) { outcome = 0; }
        else {
            require(result <= capacity && result <= 393240, "bounded successful result");
            outcome = 1; size = (uint32_t)result;
        }
        write_number((uint32_t)capacity); write_number(outcome); write_number(error);
        write_number(size); write_number((uint32_t)modified);
        if (size != 0) write_bytes(output, size);
    }
    free(guarded); free(source);
    require(fflush(stdout) == 0, "response flush failed");
    return 0;
}

/* Independent facebook/zstd v1.5.7 Huffman stream oracle.
 * This program is compiled outside the repository for tests only. The Rust
 * implementation neither links this program nor calls the C codec at runtime.
 * Copyright (c) zstdoxide contributors. BSD-3-Clause; see LICENSE.
 * Upstream functions are provided under LICENSE.zstd's BSD option.
 *
 * Request: "ZHENC001", four little-endian u32 fields (description bytes,
 * source bytes, stream count 1 or 4, provided compressed bytes), then the three
 * byte strings. Exactly one request and EOF are required.
 * Response: "ZHOK0001", four little-endian u32 fields (table log, symbol count,
 * compressed bytes, flags), 256 weights, 256 code lengths, and C stream bytes.
 * Flag 1 records X2-to-X1 table-log fallback. Flag 2 records that an empty 1X
 * stream's marker was verified but C decoding was bypassed: C's output-size
 * contract does not establish successful zero-byte decoding.
 */
#define HUF_STATIC_LINKING_ONLY
#include "huf.h"
#include "zstd.h"
#include "zstd_errors.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define SOURCE_LIMIT (128u * 1024u)
#define DESCRIPTION_LIMIT 512u
/* Four twelve-bit streams require at most 4 * 49153 + 6 = 196618 bytes. */
#define STREAM_LIMIT 196640u

static void require(int condition, const char* message) {
    if (!condition) {
        fprintf(stderr, "%s\n", message);
        exit(1);
    }
}

static void codec_ok(size_t result, const char* operation) {
    if (HUF_isError(result)) {
        fprintf(stderr, "%s: %s\n", operation, HUF_getErrorName(result));
        exit(1);
    }
}

static void read_bytes(void* destination, size_t size) {
    require(fread(destination, 1, size, stdin) == size, "truncated request");
}

static uint32_t read_number(void) {
    unsigned char bytes[4];
    read_bytes(bytes, sizeof(bytes));
    return (uint32_t)bytes[0] | (uint32_t)bytes[1] << 8 |
           (uint32_t)bytes[2] << 16 | (uint32_t)bytes[3] << 24;
}

static void write_bytes(const void* source, size_t size) {
    require(fwrite(source, 1, size, stdout) == size, "response write failed");
}

static void write_number(uint32_t number) {
    unsigned char bytes[4];
    unsigned i;
    for (i = 0; i < 4; ++i) bytes[i] = (unsigned char)(number >> (i * 8));
    write_bytes(bytes, sizeof(bytes));
}

static unsigned char* allocate(size_t size) {
    unsigned char* bytes = (unsigned char*)malloc(size ? size : 1);
    require(bytes != NULL, "allocation failed");
    return bytes;
}

static size_t encode_one(unsigned char* destination, size_t capacity,
                         const unsigned char* source, size_t size,
                         const HUF_CElt* table) {
    size_t result = HUF_compress1X_usingCTable(destination, capacity, source, size, table, 0);
    codec_ok(result, "HUF_compress1X_usingCTable");
    require(result > 0 && result <= capacity, "C 1X encoder returned no bounded stream");
    if (size == 0) require(result == 1 && destination[0] == 1, "empty stream marker mismatch");
    return result;
}

static size_t encode_four(unsigned char* destination, const unsigned char* source,
                          size_t size, const HUF_CElt* table) {
    size_t quarter = (size + 3) / 4;
    size_t position = 6;
    unsigned stream;
    require(size >= 6 && quarter * 3 <= size, "invalid four-stream partition");
    for (stream = 0; stream < 4; ++stream) {
        size_t source_start = quarter * stream;
        size_t source_size = stream == 3 ? size - source_start : quarter;
        size_t encoded;
        require(position < STREAM_LIMIT, "four-stream capacity exhausted");
        encoded = encode_one(destination + position, STREAM_LIMIT - position,
                             source + source_start, source_size, table);
        if (stream < 3) {
            require(encoded <= UINT16_MAX, "jump-table field overflow");
            destination[2 * stream] = (unsigned char)encoded;
            destination[2 * stream + 1] = (unsigned char)(encoded >> 8);
        }
        position += encoded;
    }
    if (size >= 12) {
        unsigned char* native = allocate(STREAM_LIMIT);
        size_t result = HUF_compress4X_usingCTable(native, STREAM_LIMIT, source, size, table, 0);
        codec_ok(result, "HUF_compress4X_usingCTable");
        require(result == position && memcmp(native, destination, position) == 0,
                "independent quarters differ from native C 4X stream");
        free(native);
    }
    return position;
}

static unsigned verify_decoders(const unsigned char* description, size_t description_size,
                                 const unsigned char* source, size_t source_size,
                                 const unsigned char* encoded, size_t encoded_size,
                                 unsigned streams) {
    HUF_DTable table[HUF_DTABLE_SIZE(12)];
    unsigned workspace[HUF_WORKSPACE_SIZE / sizeof(unsigned)];
    unsigned char* restored = allocate(source_size);
    unsigned flags = 0, decoder;
    for (decoder = 0; decoder < 2; ++decoder) {
        size_t consumed, decoded;
        memset(table, 0, sizeof(table));
        table[0] = 12 * 0x01000001;
        consumed = decoder == 0 ?
            HUF_readDTableX1_wksp(table, description, description_size, workspace, sizeof(workspace), 0) :
            HUF_readDTableX2_wksp(table, description, description_size, workspace, sizeof(workspace), 0);
        if (decoder == 1 && HUF_isError(consumed) &&
            ZSTD_getErrorCode(consumed) == ZSTD_error_tableLog_tooLarge) {
            flags |= 1;
            memset(table, 0, sizeof(table));
            table[0] = 12 * 0x01000001;
            consumed = HUF_readDTableX1_wksp(table, description, description_size,
                                            workspace, sizeof(workspace), 0);
        }
        codec_ok(consumed, "Huffman decoder table load");
        require(consumed == description_size, "decoder table consumed wrong description size");
        if (source_size == 0) {
            require(streams == 1 && encoded_size == 1 && encoded[0] == 1,
                    "invalid empty stream");
            flags |= 2;
            continue;
        }
        decoded = streams == 1 ?
            HUF_decompress1X_usingDTable(restored, source_size, encoded, encoded_size, table, 0) :
            HUF_decompress4X_usingDTable(restored, source_size, encoded, encoded_size, table, 0);
        codec_ok(decoded, "Huffman stream decode");
        require(decoded == source_size && memcmp(restored, source, source_size) == 0,
                "C decoder output mismatch");
    }
    free(restored);
    return flags;
}

int main(void) {
    unsigned char magic[8], description[DESCRIPTION_LIMIT], weights[256] = {0}, lengths[256] = {0};
    HUF_CElt table[HUF_CTABLE_SIZE_ST(255)];
    unsigned ranks[HUF_TABLELOG_MAX + 1] = {0};
    unsigned symbols = 0, log = 0, maximum = 255, has_zero = 0, flags;
    uint32_t description_size, source_size, streams, provided_size;
    unsigned char *source, *provided, *expected;
    size_t consumed, encoded, i;
    require(ZSTD_versionNumber() == 10507, "expected zstd v1.5.7");
    read_bytes(magic, sizeof(magic));
    require(memcmp(magic, "ZHENC001", sizeof(magic)) == 0, "invalid request magic");
    description_size = read_number(); source_size = read_number();
    streams = read_number(); provided_size = read_number();
    require(description_size > 0 && description_size <= DESCRIPTION_LIMIT, "description limit");
    require(source_size <= SOURCE_LIMIT, "source limit");
    require(streams == 1 || (streams == 4 && source_size >= 6), "stream count or partition");
    require(provided_size > 0 && provided_size <= STREAM_LIMIT, "provided stream limit");
    source = allocate(source_size); provided = allocate(provided_size); expected = allocate(STREAM_LIMIT);
    read_bytes(description, description_size); read_bytes(source, source_size); read_bytes(provided, provided_size);
    require(fgetc(stdin) == EOF && !ferror(stdin), "trailing input or read error");
    consumed = HUF_readStats(weights, sizeof(weights), ranks, &symbols, &log, description, description_size);
    codec_ok(consumed, "HUF_readStats");
    require(consumed == description_size && symbols >= 2 && symbols <= 256 && log <= 12,
            "unexpected Huffman description metadata");
    memset(table, 0, sizeof(table));
    consumed = HUF_readCTable(table, &maximum, description, description_size, &has_zero);
    codec_ok(consumed, "HUF_readCTable");
    require(consumed == description_size && maximum + 1 == symbols && HUF_readCTableHeader(table).tableLog == log,
            "encoding-table metadata mismatch");
    for (i = 0; i < 256; ++i) {
        unsigned bits = HUF_getNbBitsFromCTable(table, (unsigned)i);
        require(bits <= 12 && bits == (weights[i] ? log + 1 - weights[i] : 0),
                "weight/code-length mismatch");
        lengths[i] = (unsigned char)bits;
    }
    for (i = 0; i < source_size; ++i) require(lengths[source[i]] != 0, "source symbol absent from table");
    encoded = streams == 1 ? encode_one(expected, STREAM_LIMIT, source, source_size, table) :
                             encode_four(expected, source, source_size, table);
    require(encoded == provided_size && memcmp(expected, provided, encoded) == 0,
            "provided stream differs from same-table C encoding");
    flags = verify_decoders(description, description_size, source, source_size, provided, provided_size, streams);
    write_bytes("ZHOK0001", 8); write_number(log); write_number(symbols);
    write_number((uint32_t)encoded); write_number(flags);
    write_bytes(weights, sizeof(weights)); write_bytes(lengths, sizeof(lengths)); write_bytes(expected, encoded);
    free(source); free(provided); free(expected);
    require(fflush(stdout) == 0, "response flush failed");
    return 0;
}

/* Offline oracle only; never compiled or linked into the Rust crate.
 * Test distributions follow facebook/zstd v1.5.7 fse_read_ncount.c.
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 * Copyright (c) zstdoxide contributors. BSD-3-Clause; see LICENSE.zstd.
 */
#define FSE_STATIC_LINKING_ONLY
#define HUF_STATIC_LINKING_ONLY
#include "fse.h"
#include "huf.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void require(int valid, const char *message) {
    if (!valid) { fprintf(stderr, "%s\n", message); exit(1); }
}

static void bytes(const void *data, size_t size) {
    require(fwrite(data, 1, size, stdout) == size, "output failed");
}

static void number(uint32_t value, unsigned width) {
    unsigned i;
    for (i = 0; i < width; ++i) {
        unsigned char byte = (unsigned char)(value >> (8 * i));
        bytes(&byte, 1);
    }
}

static uint32_t random_value(uint32_t *state) {
    *state = *state * UINT32_C(1664525) + UINT32_C(1013904223);
    return *state;
}

static void fse_vectors(void) {
    const unsigned maxima[] = {0, 1, 2, 15, 31, 63, 127, 255};
    unsigned log, maximum_index, variant;
    bytes("ZFSE0001", 8);
    number(8 * 8 * 8, 4);
    for (log = 5; log <= 12; ++log) {
        for (maximum_index = 0; maximum_index < 8; ++maximum_index) {
            unsigned maximum = maxima[maximum_index];
            for (variant = 0; variant < 8; ++variant) {
                short counts[256] = {0}, decoded[256] = {0};
                unsigned char header[512];
                uint32_t state = 1 + log * 10000 + maximum * 100 + variant;
                unsigned remaining = (1u << log) - 1, symbol;
                unsigned decoded_maximum = 255, decoded_log = 0;
                size_t size, read;
                for (symbol = 0; symbol < maximum && remaining; ++symbol) {
                    int count = 0;
                    if (variant == 1 && symbol == 0) count = 1;
                    if (variant == 2) count = -1;
                    if (variant == 3 && symbol % 17 == 0) count = 1;
                    if (variant == 4) count = remaining / (maximum - symbol + 1);
                    if (variant == 5) count = (int)(random_value(&state) % 5) - 1;
                    if (variant == 6) count = (int)(random_value(&state) % (remaining + 2)) - 1;
                    if (variant == 7 && symbol % 2 == 0) count = -1;
                    if (count > (int)remaining) count = (int)remaining;
                    counts[symbol] = (short)count;
                    remaining -= (unsigned)abs(count);
                }
                counts[maximum] = (short)(remaining + 1);
                if (counts[maximum] == 1 && (variant & 1)) counts[maximum] = -1;
                size = FSE_writeNCount(header, sizeof(header), counts, maximum, log);
                require(!FSE_isError(size), "FSE oracle encoding failed");
                read = FSE_readNCount(decoded, &decoded_maximum, &decoded_log, header, size);
                require(read == size && decoded_maximum == maximum && decoded_log == log,
                        "FSE oracle metadata mismatch");
                require(memcmp(counts, decoded, (maximum + 1) * sizeof(short)) == 0,
                        "FSE oracle count mismatch");
                number(log, 1); number(maximum, 1); number((uint32_t)size, 2);
                for (symbol = 0; symbol <= maximum; ++symbol) number((uint16_t)counts[symbol], 2);
                bytes(header, size);
            }
        }
    }
}

static void huffman_vectors(void) {
    const unsigned lengths[] = {384, 511, 1024, 4096, 16384, 65536};
    const unsigned alphabets[] = {2, 4, 16, 64, 256};
    unsigned length_index, alphabet_index;
    bytes("ZHUF0001", 8);
    number(6 * 5, 4);
    for (length_index = 0; length_index < 6; ++length_index) {
        for (alphabet_index = 0; alphabet_index < 5; ++alphabet_index) {
            unsigned length = lengths[length_index], alphabet = alphabets[alphabet_index];
            unsigned count[256] = {0}, workspace[HUF_WORKSPACE_SIZE / sizeof(unsigned)];
            HUF_CElt table[HUF_CTABLE_SIZE_ST(255)];
            unsigned char original[65536], header[512], single[131072], four[131072];
            unsigned char restored[65536];
            HUF_DTable decode_table[HUF_DTABLE_SIZE(12)];
            uint32_t state = 17 + length * 100 + alphabet;
            unsigned i;
            size_t log, header_size, single_size, four_size, decoded;
            memset(table, 0, sizeof(table));
            for (i = 0; i < length; ++i) {
                uint32_t value = random_value(&state);
                original[i] = i < alphabet ? (unsigned char)i :
                    (value & 3) ? 0 : (unsigned char)((value >> 16) % alphabet);
                ++count[original[i]];
            }
            log = HUF_buildCTable_wksp(table, count, alphabet - 1, 11, workspace, sizeof(workspace));
            require(!HUF_isError(log), "Huffman table build failed");
            header_size = HUF_writeCTable_wksp(header, sizeof(header), table, alphabet - 1,
                                             (unsigned)log, workspace, sizeof(workspace));
            if (HUF_isError(header_size)) fprintf(stderr, "length=%u alphabet=%u: %s\n", length, alphabet, HUF_getErrorName(header_size));
            require(!HUF_isError(header_size) && header_size > 0, "Huffman table write failed");
            single_size = HUF_compress1X_usingCTable(single, sizeof(single), original, length, table, 0);
            four_size = HUF_compress4X_usingCTable(four, sizeof(four), original, length, table, 0);
            require(single_size && four_size && !HUF_isError(single_size) && !HUF_isError(four_size),
                    "Huffman oracle compression failed");
            decode_table[0] = 12 * 0x01000001;
            decoded = HUF_readDTableX1_wksp(decode_table, header, header_size, workspace, sizeof(workspace), 0);
            require(decoded == header_size, "Huffman table read failed");
            decoded = HUF_decompress1X_usingDTable(restored, length, single, single_size, decode_table, 0);
            require(decoded == length && memcmp(restored, original, length) == 0, "Huffman 1X mismatch");
            decoded = HUF_decompress4X_usingDTable(restored, length, four, four_size, decode_table, 0);
            require(decoded == length && memcmp(restored, original, length) == 0, "Huffman 4X mismatch");
            number(length, 4); number((uint32_t)header_size, 4);
            number((uint32_t)single_size, 4); number((uint32_t)four_size, 4);
            bytes(original, length); bytes(header, header_size);
            bytes(single, single_size); bytes(four, four_size);
        }
    }
}

int main(int argc, char **argv) {
    require(argc == 2, "expected fse or huffman");
    if (strcmp(argv[1], "fse") == 0) fse_vectors();
    else if (strcmp(argv[1], "huffman") == 0) huffman_vectors();
    else require(0, "unknown corpus");
    return fflush(stdout) != 0;
}

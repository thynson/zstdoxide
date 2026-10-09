/* Copyright (c) zstdoxide contributors.
 * Test-only driver for facebook/zstd v1.5.7 RDG generation and decompression.
 * Source schedule: tests/fuzzer.c:279-298. The codec never links this helper.
 */
#include "datagen.h"
#include "zstd.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define SOURCE_SIZE (64U * 1024U * 1024U + 16U * 1024U * 1024U + \
                     4U * 1024U * 1024U + 1U * 1024U * 1024U + \
                     256U * 1024U + 64U * 1024U)

int main(int argc, char** argv) {
    unsigned char* output;
    int failed;
    if (ZSTD_versionNumber() != 10507 || argc != 3) return 2;
    output = (unsigned char*)malloc(SOURCE_SIZE);
    if (output == NULL) return 3;
    if (!strcmp(argv[1], "generate")) {
        unsigned seed = (unsigned)strtoul(argv[2], NULL, 10);
        RDG_genBuffer(output, SOURCE_SIZE, 0.50, 0.0, seed);
    } else if (!strcmp(argv[1], "decode")) {
        size_t capacity = ZSTD_compressBound(SOURCE_SIZE);
        unsigned char* frame = (unsigned char*)malloc(capacity);
        FILE* file = fopen(argv[2], "rb");
        size_t size, decoded;
        if (frame == NULL || file == NULL) {
            free(frame);
            free(output);
            if (file != NULL) fclose(file);
            return 4;
        }
        size = fread(frame, 1, capacity, file);
        failed = ferror(file) || fgetc(file) != EOF;
        if (fclose(file) != 0) failed = 1;
        if (failed) {
            free(frame);
            free(output);
            return 5;
        }
        decoded = ZSTD_decompress(output, SOURCE_SIZE, frame, size);
        free(frame);
        if (ZSTD_isError(decoded) || decoded != SOURCE_SIZE) {
            free(output);
            return 6;
        }
    } else {
        free(output);
        return 7;
    }
    failed = fwrite(output, 1, SOURCE_SIZE, stdout) != SOURCE_SIZE;
    free(output);
    return failed;
}

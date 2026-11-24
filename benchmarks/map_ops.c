#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

// Open-addressing hash table with linear probing, the same algorithm class as
// the native Vantyx scalar-key index (ADR-0016). Insertion order is irrelevant
// to this workload, so the C version does not maintain a separate entry array.
#define CAP 8192

static int64_t keys[CAP];
static int64_t values[CAP];
static uint8_t used[CAP];
static int64_t len = 0;

static uint64_t hash_key(int64_t key) {
    uint64_t value = (uint64_t)key;
    value ^= value >> 30;
    value *= UINT64_C(0xbf58476d1ce4e5b9);
    value ^= value >> 27;
    value *= UINT64_C(0x94d049bb133111eb);
    value ^= value >> 31;
    return value;
}

static void set_key(int64_t key, int64_t value) {
    uint64_t slot = hash_key(key) & (CAP - 1);
    while (used[slot]) {
        if (keys[slot] == key) {
            values[slot] = value;
            return;
        }
        slot = (slot + 1) & (CAP - 1);
    }
    used[slot] = 1;
    keys[slot] = key;
    values[slot] = value;
    len += 1;
}

static int64_t get_key(int64_t key) {
    uint64_t slot = hash_key(key) & (CAP - 1);
    while (used[slot]) {
        if (keys[slot] == key) return values[slot];
        slot = (slot + 1) & (CAP - 1);
    }
    fputs("missing key\n", stderr);
    exit(2);
}

int main(void) {
    int64_t i = 0;
    while (i < 3000) {
        set_key(i, i * 2);
        i = i + 1;
    }
    int64_t total = 0;
    i = 0;
    while (i < 3000) {
        total = total + get_key(i);
        i = i + 1;
    }
    printf("%" PRId64 " %" PRId64 "\n", len, total);
    return 0;
}

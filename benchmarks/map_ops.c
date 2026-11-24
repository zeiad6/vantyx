#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static int64_t find_key(const int64_t* keys, int64_t len, int64_t key) {
    for (int64_t i = 0; i < len; i++) {
        if (keys[i] == key) return i;
    }
    return -1;
}

static void set_key(int64_t** keys, int64_t** values, int64_t* len, int64_t* cap,
                    int64_t key, int64_t value) {
    int64_t found = find_key(*keys, *len, key);
    if (found >= 0) {
        (*values)[found] = value;
        return;
    }
    if (*len == *cap) {
        *cap = *cap == 0 ? 4 : *cap * 2;
        *keys = realloc(*keys, (size_t)*cap * sizeof(int64_t));
        *values = realloc(*values, (size_t)*cap * sizeof(int64_t));
        if (*keys == NULL || *values == NULL) exit(2);
    }
    (*keys)[*len] = key;
    (*values)[*len] = value;
    *len += 1;
}

int main(void) {
    int64_t* keys = NULL;
    int64_t* values = NULL;
    int64_t len = 0;
    int64_t cap = 0;
    int64_t i = 0;
    while (i < 3000) {
        set_key(&keys, &values, &len, &cap, i, i * 2);
        i = i + 1;
    }
    int64_t total = 0;
    i = 0;
    while (i < 3000) {
        int64_t found = find_key(keys, len, i);
        total = total + values[found];
        i = i + 1;
    }
    printf("%" PRId64 " %" PRId64 "\n", len, total);
    free(keys);
    free(values);
    return 0;
}

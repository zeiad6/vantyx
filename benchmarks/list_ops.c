#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    int64_t cap = 0;
    int64_t len = 0;
    int64_t* xs = NULL;
    int64_t i = 0;
    while (i < 150000) {
        if (len == cap) {
            cap = cap == 0 ? 4 : cap * 2;
            xs = realloc(xs, (size_t)cap * sizeof(int64_t));
            if (xs == NULL) return 2;
        }
        xs[len++] = i;
        i = i + 1;
    }
    int64_t total = 0;
    i = 0;
    while (i < len) {
        xs[i] = xs[i] + 1;
        total = total + xs[i];
        i = i + 1;
    }
    printf("%" PRId64 " %" PRId64 "\n", len, total);
    free(xs);
    return 0;
}

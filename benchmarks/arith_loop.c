#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>

int main(void) {
    int64_t i = 0;
    int64_t total = 0;
    while (i < 1000000) {
        total = total + (i * 3 + 1) % 1000003;
        i = i + 1;
    }
    printf("%" PRId64 "\n", total);
    return 0;
}

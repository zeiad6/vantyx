#include <stdio.h>

int main(void) {
    long long total = 0;
    for (long long i = 0; i < 200000; i++) {
        char text[64];
        int written = snprintf(text, sizeof text, "item-%lld-%lld", i, i + 1);
        total += written;
    }
    printf("%lld\n", total);
    return 0;
}

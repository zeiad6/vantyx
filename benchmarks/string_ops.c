#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(void) {
    int64_t len = 2;
    int64_t cap = 3;
    char* text = malloc((size_t)cap);
    if (text == NULL) return 2;
    memcpy(text, "ab", 3);
    int64_t i = 0;
    while (i < 15) {
        if (len * 2 + 1 > cap) {
            cap = len * 2 + 1;
            text = realloc(text, (size_t)cap);
            if (text == NULL) return 2;
        }
        memcpy(text + len, text, (size_t)len);
        len = len * 2;
        text[len] = '\0';
        i = i + 1;
    }
    int64_t count = 0;
    for (i = 0; i < len; i++) {
        if (text[i] == 'a') count = count + 1;
    }
    printf("%" PRId64 " %" PRId64 "\n", len, count);
    free(text);
    return 0;
}

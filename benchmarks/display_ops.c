#include <stdio.h>
#include <stdlib.h>

int main(void) {
    char* text = (char*)malloc(100000);
    size_t length = 0;
    text[length++] = '[';
    for (long long i = 0; i < 10000; i++) {
        if (i > 0) {
            text[length++] = ',';
            text[length++] = ' ';
        }
        length += (size_t)snprintf(text + length, 100000 - length, "%lld", i);
    }
    text[length++] = ']';
    text[length] = '\0';
    printf("%zu\n", length);
    free(text);
    return 0;
}

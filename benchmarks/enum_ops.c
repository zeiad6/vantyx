#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>

typedef enum { OP_ADD, OP_MUL, OP_NEG, OP_KEEP } OpTag;

typedef struct {
    OpTag tag;
    int64_t value;
} Op;

static Op choose(int64_t state) {
    Op op;
    switch (state % 4) {
        case 0:
            op.tag = OP_ADD;
            op.value = (state % 7) + 1;
            break;
        case 1:
            op.tag = OP_MUL;
            op.value = (state % 5) + 1;
            break;
        case 2:
            op.tag = OP_NEG;
            op.value = 0;
            break;
        default:
            op.tag = OP_KEEP;
            op.value = 0;
            break;
    }
    return op;
}

static int64_t apply(Op op, int64_t acc) {
    switch (op.tag) {
        case OP_ADD: return acc + op.value;
        case OP_MUL: return acc * op.value;
        case OP_NEG: return -acc;
        case OP_KEEP: return acc;
    }
    return acc;
}

int main(void) {
    int64_t state = 12345;
    int64_t acc = 1;
    for (int64_t i = 0; i < 200000; i++) {
        state = (state * 31 + 7) % 1000003;
        Op op = choose(state);
        acc = apply(op, acc);
        acc = (acc + 1000003) % 1000003;
    }
    printf("%" PRId64 " %" PRId64 "\n", state, acc);
    return 0;
}

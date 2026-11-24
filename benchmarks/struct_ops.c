#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

/* Vantyx native stores each struct behind its own heap allocation, so this
 * implementation does the same: a growable pointer array plus one malloc per
 * Point. Using a contiguous Point array here would benchmark a different
 * representation rather than the language implementation. */
typedef struct {
    int64_t x;
    int64_t y;
} Point;

int main(void) {
    int64_t cap = 0;
    int64_t len = 0;
    Point** points = NULL;
    int64_t i = 0;
    while (i < 80000) {
        if (len == cap) {
            cap = cap == 0 ? 4 : cap * 2;
            points = realloc(points, (size_t)cap * sizeof(Point*));
            if (points == NULL) return 2;
        }
        Point* point = malloc(sizeof(Point));
        if (point == NULL) return 2;
        point->x = i;
        point->y = i + 1;
        points[len] = point;
        len += 1;
        i = i + 1;
    }
    int64_t total = 0;
    i = 0;
    while (i < len) {
        points[i]->x = points[i]->x + 1;
        total = total + points[i]->x + points[i]->y;
        i = i + 1;
    }
    printf("%" PRId64 " %" PRId64 "\n", len, total);
    for (i = 0; i < len; i++) free(points[i]);
    free(points);
    return 0;
}

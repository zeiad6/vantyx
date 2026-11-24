/* Minimal measurement helper.
 *
 * The benchmark runner is a Python process with a relatively large resident
 * set. A child forked directly from it inherits that resident set as its
 * initial ru_maxrss, which hides the real peak RSS of small benchmark
 * binaries. This helper has a tiny resident set, forks/execs the target,
 * redirects the target's stdout/stderr to /dev/null, and reports the child's
 * wall time and ru_maxrss from wait4(2).
 *
 * Output (stderr): VANTYX_MEASURE <elapsed_ms> <maxrss_kib> <exit_code>
 */
#define _GNU_SOURCE

#include <fcntl.h>
#include <stdio.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

int main(int argc, char** argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: measure <program> [args...]\n");
        return 2;
    }

    struct timespec start;
    struct timespec end;
    if (clock_gettime(CLOCK_MONOTONIC, &start) != 0) {
        return 2;
    }

    pid_t pid = fork();
    if (pid < 0) {
        return 2;
    }
    if (pid == 0) {
        int devnull = open("/dev/null", O_WRONLY);
        if (devnull >= 0) {
            dup2(devnull, STDOUT_FILENO);
            dup2(devnull, STDERR_FILENO);
        }
        execv(argv[1], &argv[1]);
        _exit(127);
    }

    int status = 0;
    struct rusage usage;
    if (wait4(pid, &status, 0, &usage) < 0) {
        return 2;
    }
    if (clock_gettime(CLOCK_MONOTONIC, &end) != 0) {
        return 2;
    }

    double elapsed_ms = (double)(end.tv_sec - start.tv_sec) * 1000.0 +
                        (double)(end.tv_nsec - start.tv_nsec) / 1e6;
    int code;
    if (WIFEXITED(status)) {
        code = WEXITSTATUS(status);
    } else if (WIFSIGNALED(status)) {
        code = 128 + WTERMSIG(status);
    } else {
        code = 255;
    }
    fprintf(stderr, "VANTYX_MEASURE %.3f %ld %d\n", elapsed_ms, usage.ru_maxrss,
            code);
    return code;
}

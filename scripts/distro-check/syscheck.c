/*
 * syscheck: the C side of distro-check.sh.
 *
 * Checks what only the C library can answer: POSIX.1 and XSI versions,
 * the mandatory and optional POSIX options (sysconf), limits against the
 * minimums POSIX requires (sysconf, pathconf), the utility search path
 * (confstr), the POSIX and UTF-8 locales, and the clocks.
 *
 * Each result is printed as one line, "LEVEL<TAB>MESSAGE", where LEVEL is
 * PASS, FAIL, WARN or INFO; distro-check.sh reads and counts them. Build it
 * with any C17 compiler (`just distro-check` uses clang):
 *
 *     cc -std=c17 -O2 -o syscheck syscheck.c
 *
 * Link it statically (-static) to also run it inside an unbooted root.
 */

#define _XOPEN_SOURCE 700

#include <langinfo.h>
#include <limits.h>
#include <locale.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/utsname.h>
#include <time.h>
#include <unistd.h>

/* Prints one result line: LEVEL, a tab, then the formatted message. */
__attribute__((format(printf, 2, 3)))
static void result(const char *level, const char *fmt, ...)
{
    va_list args;
    printf("%s\t", level);
    va_start(args, fmt);
    vprintf(fmt, args);
    va_end(args);
    putchar('\n');
}

/* A sysconf() limit with the minimum POSIX requires. -1 means "no fixed
 * limit", which meets any minimum. */
struct limit {
    const char *name;
    int key;
    long minimum;
    /* Why falling short is expected, or NULL when it is a failure. */
    const char *known;
};

/* A sysconf() option, mandatory since POSIX.1-2008 or optional. */
struct option {
    const char *name;
    int key;
    int mandatory;
};

static void check_limit(const struct limit *l)
{
    long value = sysconf(l->key);
    if (value == -1) {
        result("PASS", "%s: no fixed limit (minimum %ld)", l->name,
               l->minimum);
    } else if (value >= l->minimum) {
        result("PASS", "%s = %ld (POSIX minimum %ld)", l->name, value,
               l->minimum);
    } else if (l->known != NULL) {
        result("INFO", "%s = %ld, below the POSIX minimum %ld (%s)",
               l->name, value, l->minimum, l->known);
    } else {
        result("FAIL", "%s = %ld, below the POSIX minimum %ld", l->name,
               value, l->minimum);
    }
}

static void check_option(const struct option *o)
{
    long value = sysconf(o->key);
    if (value > 0) {
        result("PASS", "%s supported (%ld)", o->name, value);
    } else if (o->mandatory) {
        result("FAIL", "%s missing (mandatory since POSIX.1-2008)",
               o->name);
    } else {
        result("INFO", "%s not supported (optional)", o->name);
    }
}

static void check_path_limit(const char *name, int key, long minimum)
{
    long value = pathconf("/", key);
    if (value == -1) {
        result("PASS", "%s on /: no fixed limit (minimum %ld)", name,
               minimum);
    } else if (value >= minimum) {
        result("PASS", "%s on / = %ld (POSIX minimum %ld)", name, value,
               minimum);
    } else {
        result("FAIL", "%s on / = %ld, below the POSIX minimum %ld", name,
               value, minimum);
    }
}

static void check_versions(void)
{
    long posix = sysconf(_SC_VERSION);
    if (posix >= 202405L) {
        result("PASS", "POSIX.1 version %ld (POSIX.1-2024)", posix);
    } else if (posix >= 200809L) {
        result("PASS", "POSIX.1 version %ld (POSIX.1-2008)", posix);
    } else {
        result("FAIL", "POSIX.1 version %ld, older than POSIX.1-2008", posix);
    }
    long posix2 = sysconf(_SC_2_VERSION);
    if (posix2 >= 200809L) {
        result("PASS", "POSIX.2 (shell and utilities) version %ld", posix2);
    } else {
        result("WARN", "POSIX.2 (shell and utilities) version %ld", posix2);
    }
#ifdef _SC_XOPEN_VERSION
    long xopen = sysconf(_SC_XOPEN_VERSION);
    if (xopen >= 700) {
        result("PASS", "X/Open (SUS) version %ld", xopen);
    } else {
        result("WARN", "X/Open (SUS) version %ld, older than SUSv4", xopen);
    }
#endif
}

static void check_path(void)
{
    char path[PATH_MAX];
    size_t n = confstr(_CS_PATH, path, sizeof path);
    if (n == 0 || n > sizeof path) {
        result("FAIL", "confstr(_CS_PATH) gives no utility path");
        return;
    }
    result("PASS", "Standard utility path: %s", path);
    char *save = NULL;
    for (char *dir = strtok_r(path, ":", &save); dir != NULL;
         dir = strtok_r(NULL, ":", &save)) {
        struct stat st;
        if (stat(dir, &st) != 0 || !S_ISDIR(st.st_mode)) {
            result("WARN", "%s from _CS_PATH does not exist", dir);
        }
    }
}

static void check_locales(void)
{
    if (setlocale(LC_ALL, "POSIX") != NULL) {
        result("PASS", "The POSIX locale loads");
    } else {
        result("FAIL", "The POSIX locale does not load");
    }
    const char *utf8[] = {"C.UTF-8", "C.utf8", "en_US.UTF-8"};
    for (size_t i = 0; i < sizeof utf8 / sizeof utf8[0]; i++) {
        if (setlocale(LC_ALL, utf8[i]) != NULL) {
            const char *codeset = nl_langinfo(CODESET);
            if (strcmp(codeset, "UTF-8") == 0) {
                result("PASS", "UTF-8 locale %s loads", utf8[i]);
                setlocale(LC_ALL, "C");
                return;
            }
        }
    }
    setlocale(LC_ALL, "C");
    result("WARN", "No UTF-8 locale loads (C.UTF-8, en_US.UTF-8)");
}

static void check_clocks(void)
{
    struct timespec res;
    if (clock_getres(CLOCK_REALTIME, &res) == 0) {
        result("PASS", "CLOCK_REALTIME resolution %ld ns",
               res.tv_sec * 1000000000L + res.tv_nsec);
    } else {
        result("FAIL", "CLOCK_REALTIME unavailable");
    }
#ifdef CLOCK_MONOTONIC
    if (clock_getres(CLOCK_MONOTONIC, &res) == 0) {
        result("PASS", "CLOCK_MONOTONIC resolution %ld ns",
               res.tv_sec * 1000000000L + res.tv_nsec);
    } else {
        result("WARN", "CLOCK_MONOTONIC unavailable");
    }
#endif
}

int main(void)
{
    struct utsname u;
    if (uname(&u) == 0) {
        result("INFO", "Kernel: %s %s %s", u.sysname, u.release, u.machine);
    }

    check_versions();

    static const struct option options[] = {
        /* Mandatory in POSIX.1-2008 and later. */
        {"_POSIX_ASYNCHRONOUS_IO", _SC_ASYNCHRONOUS_IO, 1},
        {"_POSIX_BARRIERS", _SC_BARRIERS, 1},
        {"_POSIX_CLOCK_SELECTION", _SC_CLOCK_SELECTION, 1},
        {"_POSIX_JOB_CONTROL", _SC_JOB_CONTROL, 1},
        {"_POSIX_MAPPED_FILES", _SC_MAPPED_FILES, 1},
        {"_POSIX_MEMORY_PROTECTION", _SC_MEMORY_PROTECTION, 1},
        {"_POSIX_READER_WRITER_LOCKS", _SC_READER_WRITER_LOCKS, 1},
        {"_POSIX_REALTIME_SIGNALS", _SC_REALTIME_SIGNALS, 1},
        {"_POSIX_REGEXP", _SC_REGEXP, 1},
        {"_POSIX_SAVED_IDS", _SC_SAVED_IDS, 1},
        {"_POSIX_SEMAPHORES", _SC_SEMAPHORES, 1},
        {"_POSIX_SHELL", _SC_SHELL, 1},
        {"_POSIX_SPIN_LOCKS", _SC_SPIN_LOCKS, 1},
        {"_POSIX_THREAD_SAFE_FUNCTIONS", _SC_THREAD_SAFE_FUNCTIONS, 1},
        {"_POSIX_THREADS", _SC_THREADS, 1},
        {"_POSIX_TIMEOUTS", _SC_TIMEOUTS, 1},
        {"_POSIX_TIMERS", _SC_TIMERS, 1},
        /* Optional. */
        {"_POSIX_ADVISORY_INFO", _SC_ADVISORY_INFO, 0},
        {"_POSIX_CPUTIME", _SC_CPUTIME, 0},
        {"_POSIX_FSYNC", _SC_FSYNC, 0},
        {"_POSIX_IPV6", _SC_IPV6, 0},
        {"_POSIX_MESSAGE_PASSING", _SC_MESSAGE_PASSING, 0},
        {"_POSIX_MONOTONIC_CLOCK", _SC_MONOTONIC_CLOCK, 0},
        {"_POSIX_PRIORITY_SCHEDULING", _SC_PRIORITY_SCHEDULING, 0},
        {"_POSIX_RAW_SOCKETS", _SC_RAW_SOCKETS, 0},
        {"_POSIX_SHARED_MEMORY_OBJECTS", _SC_SHARED_MEMORY_OBJECTS, 0},
        {"_POSIX_SPAWN", _SC_SPAWN, 0},
        {"_POSIX_THREAD_PRIORITY_SCHEDULING",
         _SC_THREAD_PRIORITY_SCHEDULING, 0},
        {"_POSIX2_C_DEV", _SC_2_C_DEV, 0},
        {"_POSIX2_LOCALEDEF", _SC_2_LOCALEDEF, 0},
        {"_POSIX2_SW_DEV", _SC_2_SW_DEV, 0},
        {"_POSIX2_UPE", _SC_2_UPE, 0},
        {"_XOPEN_CRYPT", _SC_XOPEN_CRYPT, 0},
        {"_XOPEN_ENH_I18N", _SC_XOPEN_ENH_I18N, 0},
        {"_XOPEN_REALTIME", _SC_XOPEN_REALTIME, 0},
        {"_XOPEN_SHM", _SC_XOPEN_SHM, 0},
        {"_XOPEN_UNIX", _SC_XOPEN_UNIX, 0},
    };
    for (size_t i = 0; i < sizeof options / sizeof options[0]; i++) {
        check_option(&options[i]);
    }

    /* The minimums of <limits.h> (_POSIX_* and _POSIX2_* values). */
    static const struct limit limits[] = {
        {"ARG_MAX", _SC_ARG_MAX, 4096, NULL},
        {"ATEXIT_MAX", _SC_ATEXIT_MAX, 32, NULL},
        {"BC_BASE_MAX", _SC_BC_BASE_MAX, 99, NULL},
        {"BC_DIM_MAX", _SC_BC_DIM_MAX, 2048, NULL},
        {"BC_SCALE_MAX", _SC_BC_SCALE_MAX, 99, NULL},
        {"BC_STRING_MAX", _SC_BC_STRING_MAX, 1000, NULL},
        {"CHILD_MAX", _SC_CHILD_MAX, 25, NULL},
        {"COLL_WEIGHTS_MAX", _SC_COLL_WEIGHTS_MAX, 2, NULL},
        {"DELAYTIMER_MAX", _SC_DELAYTIMER_MAX, 32, NULL},
        {"EXPR_NEST_MAX", _SC_EXPR_NEST_MAX, 32, NULL},
        {"HOST_NAME_MAX", _SC_HOST_NAME_MAX, 255,
         "Linux limits host names to 64 bytes on every system"},
        {"IOV_MAX", _SC_IOV_MAX, 16, NULL},
        {"LINE_MAX", _SC_LINE_MAX, 2048, NULL},
        {"LOGIN_NAME_MAX", _SC_LOGIN_NAME_MAX, 9, NULL},
        {"NGROUPS_MAX", _SC_NGROUPS_MAX, 8, NULL},
        {"OPEN_MAX", _SC_OPEN_MAX, 20, NULL},
        {"PAGESIZE", _SC_PAGESIZE, 1, NULL},
        {"RE_DUP_MAX", _SC_RE_DUP_MAX, 255, NULL},
        {"RTSIG_MAX", _SC_RTSIG_MAX, 8, NULL},
        {"SEM_NSEMS_MAX", _SC_SEM_NSEMS_MAX, 256, NULL},
        {"SEM_VALUE_MAX", _SC_SEM_VALUE_MAX, 32767, NULL},
        {"SIGQUEUE_MAX", _SC_SIGQUEUE_MAX, 32, NULL},
        {"STREAM_MAX", _SC_STREAM_MAX, 8, NULL},
        {"SYMLOOP_MAX", _SC_SYMLOOP_MAX, 8, NULL},
        {"THREAD_KEYS_MAX", _SC_THREAD_KEYS_MAX, 128, NULL},
        {"THREAD_THREADS_MAX", _SC_THREAD_THREADS_MAX, 64, NULL},
        {"TIMER_MAX", _SC_TIMER_MAX, 32, NULL},
        {"TTY_NAME_MAX", _SC_TTY_NAME_MAX, 9, NULL},
        {"TZNAME_MAX", _SC_TZNAME_MAX, 6, NULL},
    };
    for (size_t i = 0; i < sizeof limits / sizeof limits[0]; i++) {
        check_limit(&limits[i]);
    }

    check_path_limit("LINK_MAX", _PC_LINK_MAX, 8);
    check_path_limit("NAME_MAX", _PC_NAME_MAX, 14);
    check_path_limit("PATH_MAX", _PC_PATH_MAX, 256);
    check_path_limit("PIPE_BUF", _PC_PIPE_BUF, 512);

    check_path();
    check_locales();
    check_clocks();
    return 0;
}

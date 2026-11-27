// Native fixture for the device profiles of runtime/cfg.c: the real settings code over the C library, and one line per
// row of the settings table for tests/hwaoi/test_device_profile.py, which writes the settings file next to <exe>.
//   device_profile_fixture <path of a hwaoi.exe that need not exist>
// Prints "profile <deck|desktop>", then "row <KEY> <value in effect> <1 when the row accepts it, else 0>" for every row
// with a value, then "preset <1 when the game resolution in effect is auto or one of the game's presets>".
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "cfg.h"
#include "game_target.h"
#include "hwaoi_display.h"
#include "platform/platform.h"

void rt_log(const char *fmt, ...) { (void)fmt; }
void rt_fatal(const char *fmt, ...) {
    va_list a;
    va_start(a, fmt);
    vfprintf(stderr, fmt, a);
    va_end(a);
    exit(70);
}

static const char *exe;
int plat_exe_path(char *out, size_t n) { return snprintf(out, n, "%s", exe) < (int)n; }
uint8_t *plat_file_read(const char *path, uint64_t *size) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    uint8_t *buf = malloc(1 << 16);
    size_t n = buf ? fread(buf, 1, (1 << 16) - 1, f) : 0;
    fclose(f);
    if (!buf || !n) { free(buf); return NULL; }
    buf[n] = 0;
    *size = n;
    return buf;
}
int plat_file_exists(const char *path) { FILE *f = fopen(path, "rb"); if (f) fclose(f); return f != NULL; }
int64_t plat_file_size(const char *path) { (void)path; return -1; }
int plat_file_write_atomic(const char *path, const void *data, size_t n) { (void)path; (void)data; (void)n; return 0; }
void plat_mutex_lock(PlatMutex *m) { (void)m; }
void plat_mutex_unlock(PlatMutex *m) { (void)m; }

// The row's list ("a|b|c") or range ("lo..hi") takes value; a row without one takes any.
static int accepted(const CfgMeta *m, const char *value) {
    if (!m->values) return 1;
    const char *dots = strstr(m->values, "..");
    if (dots && !strchr(m->values, '|')) {
        char *end;
        double v = strtod(value, &end);
        return *value && !*end && v >= strtod(m->values, NULL) && v <= strtod(dots + 2, NULL);
    }
    size_t n = strlen(value);
    for (const char *p = m->values; p; p = strchr(p, '|') ? strchr(p, '|') + 1 : NULL)
        if (!strncmp(p, value, n) && (p[n] == '|' || !p[n])) return 1;
    return 0;
}

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    exe = argv[1];
    cfg_init();
    printf("profile %s\n", cfg_device_profile());
    for (int i = 0; cfg_meta_at(i); i++) {
        const CfgMeta *m = cfg_meta_at(i);
        char name[80];
        snprintf(name, sizeof name, "TOTK_%s", m->key);
        const char *v = cfg_value(name);
        if (*v) printf("row %s %s %d\n", m->key, v, accepted(m, v));
    }
    unsigned size[2];
    const char *out = cfg_value("TOTK_OUTPUT_RES");
    printf("preset %d\n", !RT_TARGET_HWAOI || !strcmp(out, "auto") || hwaoi_display_size(out, size));
    return 0;
}

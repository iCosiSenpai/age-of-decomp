// GPU breadcrumbs for a lost device (vk.h vk_crumb). VK_AMD_buffer_marker writes a 32-bit code into a buffer from
// the command stream: at TOP_OF_PIPE once the command processor reaches the marker, at BOTTOM_OF_PIPE once every
// command before it has finished. Each crumb writes both, into two words of one host-coherent buffer, so after a
// device loss word 0 names the newest point the GPU reached and word 1 the newest point it had finished: the hang lies
// between them. A code is the crumb kind in the top 4 bits and its number in the low 28. The markers cost two small
// writes per crumb, and crumbs mark submissions, upload batches, ASTC chunks and presents.
//
// TOTK_DRAW_CRUMBS=1 marks every draw too (vk_exec_draw.c prepare_draw): draws are numbered from 1 (the number wraps
// at 28 bits), and a note of what each one is stays in a ring of the last 65536. After a lost device the report then
// lists the draws the GPU had been given and had not finished, each with its frame, its pass and the NVN 2 names of
// its shaders, which are the same on every graphics card: the same draw can be looked at on another PC. Two more
// marker writes per draw, so it is off unless asked for. TOTK_DRAW_CRUMBS_TEST=<frame> writes that list once, at the
// first draw of that frame, as if the GPU had stopped six draws before it, and then says when a code the GPU itself
// wrote names a draw. Each draw's line also says what came between it and the draw before (a feedback barrier, a pass
// that ended and began again, a batch of GPU QUADS conversions, which has crumbs of its own) and which of its pass's
// targets its fragment shader samples: a hang right after a draw can lie in those commands (GitHub #16).
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "cfg.h"
#include "host.h"
#include "vk.h"

static PFN_vkCmdWriteBufferMarkerAMD write_marker;
static VkBuffer buf;
static VmaAllocation alloc;
static volatile uint32_t *words;

enum { NOTES = 65536, DRAW_MAX = 0x0fffffff };
static VkDrawNote *notes;      // notes[n % NOTES] is draw n while its no is n
static uint32_t draw_no;
static uint64_t test_frame;
static uint32_t test_watch;    // the test: draws left in which a draw code of the GPU's own is looked for

void vk_crumbs_init(VkCtx *c) {
    if (!c->has_buffer_marker) {
        if (cfg_bool("TOTK_DRAW_CRUMBS"))
            rt_log("vk: GPU breadcrumbs: draw_crumbs = 1 does nothing here: this graphics card's driver has no "
                   "VK_AMD_buffer_marker");
        return;
    }
    write_marker = (PFN_vkCmdWriteBufferMarkerAMD)vkGetDeviceProcAddr(c->device, "vkCmdWriteBufferMarkerAMD");
    VkBufferCreateInfo bi = {.sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO, .size = 16,
                             .usage = VK_BUFFER_USAGE_TRANSFER_DST_BIT};
    VmaAllocationCreateInfo ai = {.usage = VMA_MEMORY_USAGE_AUTO_PREFER_HOST,
                                  .flags = VMA_ALLOCATION_CREATE_HOST_ACCESS_RANDOM_BIT | VMA_ALLOCATION_CREATE_MAPPED_BIT,
                                  .requiredFlags = VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT};
    VmaAllocationInfo info;
    if (!write_marker || vmaCreateBuffer(c->vma, &bi, &ai, &buf, &alloc, &info) != VK_SUCCESS) {
        rt_log("vk: GPU breadcrumbs unavailable");
        write_marker = NULL;
        return;
    }
    words = info.pMappedData;
    words[0] = words[1] = 0;
    rt_log("vk: GPU breadcrumbs on (VK_AMD_buffer_marker)");
    if (cfg_bool("TOTK_DRAW_CRUMBS") && (notes = calloc(NOTES, sizeof *notes))) {
        const char *test = cfg_get("TOTK_DRAW_CRUMBS_TEST");
        test_frame = test && *test ? strtoull(test, NULL, 10) : 0;
        rt_log("vk: GPU breadcrumbs: every draw is marked too (draw_crumbs = 1)");
    }
}

void vk_crumbs_release(void) {
    VkCtx *c = vk_ctx();
    if (buf && c) vmaDestroyBuffer(c->vma, buf, alloc);
    buf = VK_NULL_HANDLE, alloc = NULL, words = NULL, write_marker = NULL;
    free(notes);
    notes = NULL, draw_no = 0, test_frame = 0, test_watch = 0;
}

void vk_crumb(VkCommandBuffer cb, int kind, uint64_t n) {
    if (!write_marker || !cb) return;
    uint32_t code = (uint32_t)kind << 28 | (uint32_t)(n & 0x0fffffff);
    write_marker(cb, VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT, buf, 0, code);
    write_marker(cb, VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT, buf, 4, code);
}

static void describe(uint32_t code, char *o, size_t size) {
    static const char *const kinds[] = {"nothing", "the start of a part of submission", "the end of submission",
                                        "the start of upload batch", "the end of upload batch",
                                        "ASTC decode chunk", "the start of present",
                                        "the end of present", "draw",
                                        "the start of a QUADS conversion after draw",
                                        "the end of a QUADS conversion after draw"};
    unsigned k = code >> 28;
    if (!code) snprintf(o, size, "nothing yet");
    else if (k < sizeof kinds / sizeof *kinds) snprintf(o, size, "%s %u", kinds[k], code & 0x0fffffff);
    else snprintf(o, size, "unknown code %#x", code);
}

// ---- a crumb per draw

int vk_crumbs_draws(void) { return notes != NULL; }

static const VkDrawNote *note_of(uint32_t no) {
    const VkDrawNote *n = &notes[no % NOTES];
    return n->no == no ? n : NULL;   // NULL: never recorded, or the ring has gone round since
}

static void draw_line(const VkDrawNote *n) {
    char targets[160], sampled[48], after[112];
    size_t at = 0, sat = 0;
    targets[0] = sampled[0] = after[0] = 0;
    for (int i = 0; i < 9; i++) {
        if (!n->fmt[i]) continue;
        int w = snprintf(targets + at, sizeof targets - at, "%s%c%d fmt %u", at ? ", " : "", i < 8 ? 'c' : 'd',
                         i < 8 ? i : 0, n->fmt[i]);
        if (w < 0 || (size_t)w >= sizeof targets - at) break;
        at += (size_t)w;
    }
    for (int i = 0; i < 9; i++) {
        if (!(n->sampled >> i & 1)) continue;
        int w = snprintf(sampled + sat, sizeof sampled - sat, "%s%c%d", sat ? " " : "", i < 8 ? 'c' : 'd', i < 8 ? i : 0);
        if (w < 0 || (size_t)w >= sizeof sampled - sat) break;
        sat += (size_t)w;
    }
    static const struct { uint8_t bit; const char *what; } afters[] = {   // in the order they are recorded
        {VK_DRAW_AFTER_QUADS, "a QUADS conversion"}, {VK_DRAW_AFTER_NEW_PASS, "a new pass or submission"},
        {VK_DRAW_AFTER_FB_RESTART, "a pass restart for a feedback loop"}, {VK_DRAW_AFTER_FB_BARRIER, "a feedback barrier"}};
    for (size_t i = 0, aat = 0; i < sizeof afters / sizeof *afters; i++) {
        if (!(n->after & afters[i].bit)) continue;
        int w = snprintf(after + aat, sizeof after - aat, "%s%s", aat ? ", " : "; after ", afters[i].what);
        if (w < 0 || (size_t)w >= sizeof after - aat) break;
        aat += (size_t)w;
    }
    rt_log("vk: GPU breadcrumbs: draw %u: frame %llu, submission %u, %u vertices, pass %u %ux%u, %s, shaders vs %016llx "
           "fs %016llx%s%s%s", n->no, (unsigned long long)n->frame, n->sub, n->vertices, n->pass, n->w, n->h,
           at ? targets : "no targets", (unsigned long long)n->vs, (unsigned long long)n->fs,
           sat ? "; samples its targets " : "", sampled, after);
}

// The draw a code names: a draw's own number, or for a QUADS conversion the draw recorded before it. 0: none.
static uint32_t draw_named(uint32_t code) {
    unsigned k = code >> 28;
    return k == VK_CRUMB_DRAW || k == VK_CRUMB_QUADS || k == VK_CRUMB_QUADS_END ? code & DRAW_MAX : 0;
}

// The draws the GPU had been given and had not finished: from the one after the last finished (when that crumb is not
// a draw: from the first draw of the submission the last reached draw is in) to the last reached. The first eight are
// written, where the one that hangs is, and the last.
static void report_draws(uint32_t reached, uint32_t finished) {
    if (!notes) return;
    unsigned kind = reached >> 28;
    if (kind == VK_CRUMB_QUADS || kind == VK_CRUMB_QUADS_END) {   // in or after a conversion batch between two draws
        uint32_t before = reached & DRAW_MAX;
        const VkDrawNote *b = before ? note_of(before) : NULL, *next = note_of(before < DRAW_MAX ? before + 1 : 1);
        if (!b) {
            rt_log("vk: GPU breadcrumbs: the hang is %s a QUADS conversion recorded before any draw of the notes",
                   kind == VK_CRUMB_QUADS ? "in or after" : "after");
            return;
        }
        rt_log("vk: GPU breadcrumbs: the hang is %s the QUADS conversion recorded after draw %u; that draw and the next:",
               kind == VK_CRUMB_QUADS ? "in or after" : "after", before);
        draw_line(b);
        if (next) draw_line(next);
        return;
    }
    if (kind != VK_CRUMB_DRAW) return;
    uint32_t last = reached & DRAW_MAX;
    uint32_t stop = draw_named(finished);   // the newest draw known to be finished
    const VkDrawNote *end = note_of(last);
    if (!end) {
        rt_log("vk: GPU breadcrumbs: no note of draw %u is left", last);
        return;
    }
    if (stop == last) {   // nothing between the two: the hang is in what follows this draw
        const VkDrawNote *next = note_of(last < DRAW_MAX ? last + 1 : 1);
        rt_log("vk: GPU breadcrumbs: draw %u had finished and no later one was reached; it and the one after it:", last);
        draw_line(end);
        if (next) draw_line(next);
        return;
    }
    uint32_t first = last, count = 1;
    while (count < NOTES - 1) {
        uint32_t before = first > 1 ? first - 1 : DRAW_MAX;
        const VkDrawNote *b = note_of(before);
        if (!b || before == stop || (!stop && b->sub != end->sub)) break;
        first = before, count++;
    }
    rt_log("vk: GPU breadcrumbs: %u draw%s not finished, %u to %u%s", count, count == 1 ? "" : "s", first, last,
           count > 8 ? "; the first eight and the last:" : ":");
    uint32_t d = first;
    for (uint32_t shown = 0; shown < 8; shown++) {
        const VkDrawNote *n = note_of(d);
        if (n) draw_line(n);
        if (d == last) return;
        d = d < DRAW_MAX ? d + 1 : 1;
    }
    draw_line(end);
}

uint32_t vk_crumbs_last_draw(void) { return notes ? draw_no : 0; }

uint32_t vk_crumbs_draw(const VkDrawNote *n) {
    const VkDrawNote *prev = draw_no ? note_of(draw_no) : NULL;
    if (++draw_no > DRAW_MAX) draw_no = 1;
    VkDrawNote *slot = &notes[draw_no % NOTES];
    *slot = *n;
    slot->no = draw_no;
    if (!prev || prev->pass != n->pass || prev->sub != n->sub) slot->after |= VK_DRAW_AFTER_NEW_PASS;
    if (test_frame && n->frame >= test_frame && draw_no > 6) {
        test_frame = 0;
        rt_log("vk: GPU breadcrumbs: TOTK_DRAW_CRUMBS_TEST: the report of a lost device, with the GPU's own two codes "
               "first and then as if it had stopped six draws before draw %u", draw_no);
        vk_crumbs_report();
        report_draws((uint32_t)VK_CRUMB_DRAW << 28 | draw_no, (uint32_t)VK_CRUMB_DRAW << 28 | (draw_no - 6));
        test_watch = 100000;
    }
    if (test_watch) {   // the test goes on until a code the GPU itself wrote names a draw: the markers do run there
        uint32_t reached = words[0];
        if (reached >> 28 == VK_CRUMB_DRAW) {
            rt_log("vk: GPU breadcrumbs: test: the GPU's own code named draw %u while draw %u was recorded",
                   reached & DRAW_MAX, draw_no);
            test_watch = 0;
        } else if (!--test_watch) {
            rt_log("vk: GPU breadcrumbs: test: no code of the GPU's own named a draw in 100000 draws");
        }
    }
    return draw_no;
}

void vk_crumbs_report(void) {
    if (!words) return;
    char reached[96], finished[96];
    uint32_t r = words[0], f = words[1];
    describe(r, reached, sizeof reached);
    describe(f, finished, sizeof finished);
    rt_log("vk: GPU breadcrumbs: the GPU last reached %s; everything before %s had finished", reached, finished);
    report_draws(r, f);
}

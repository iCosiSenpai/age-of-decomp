// GPU breadcrumbs for a lost device (vk.h vk_crumb). VK_AMD_buffer_marker writes a 32-bit code into a buffer from
// the command stream: at TOP_OF_PIPE once the command processor reaches the marker, at BOTTOM_OF_PIPE once every
// command before it has finished. Each crumb writes both, into two words of one host-coherent buffer, so after a
// device loss word 0 names the newest point the GPU reached and word 1 the newest point it had finished: the hang lies
// between them. A code is the crumb kind in the top 4 bits and its number in the low 28. The markers cost two small
// writes per crumb, and crumbs mark submissions, upload batches, ASTC chunks and presents.
//
// TOTK_DRAW_CRUMBS=1 marks every draw too (vk_exec_draw.c prepare_draw), and every compute dispatch of the game
// (op_dispatch, op_dispatch_ind): they are numbered from 1 in one sequence (the number wraps at 28 bits), and a note of
// what each one is stays in a ring of the last 65536. After a lost device the report then lists the draws and
// dispatches the GPU had been given and had not finished, each with its frame, its pass and the NVN 2 names of its
// shaders, which are the same on every graphics card: the same draw can be looked at on another PC. A crumb is recorded
// right before its draw or dispatch, so a code that names draw N says that the GPU reached the point before draw N
// (word 0) or that everything before draw N had finished (word 1): when both name N, the hang lies in draw N or in what
// was recorded after it and before the next crumb. Two more marker writes per draw, so it is off unless asked for.
// TOTK_DRAW_CRUMBS_TEST=<frame> writes that list once, at the first draw of that frame, as if the GPU had stopped six
// draws before it, then how many dispatches were marked and the line of the last one (a frame of this game begins with
// a draw, so the six before it are draws), and then says when a code the GPU itself wrote names a draw. Each draw's line also says what came
// between it and the draw before (a feedback barrier, a pass that ended and began again, a batch of GPU QUADS
// conversions, which has crumbs of its own) and which of its pass's targets its fragment shader samples: a hang right
// after a draw can lie in those commands (GitHub #16). A dispatch's line gives its work groups and its compute stage.
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
static uint32_t dispatches, last_dispatch;   // dispatches marked, and the number of the last one (for the test)

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
    notes = NULL, draw_no = 0, test_frame = 0, test_watch = 0, dispatches = 0, last_dispatch = 0;
}

void vk_crumb(VkCommandBuffer cb, int kind, uint64_t n) {
    if (!write_marker || !cb) return;
    uint32_t code = (uint32_t)kind << 28 | (uint32_t)(n & 0x0fffffff);
    write_marker(cb, VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT, buf, 0, code);
    write_marker(cb, VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT, buf, 4, code);
}

static const VkDrawNote *note_of(uint32_t no) {
    if (!notes) return NULL;
    const VkDrawNote *n = &notes[no % NOTES];
    return n->no == no ? n : NULL;   // NULL: never recorded, or the ring has gone round since
}

static void describe(uint32_t code, char *o, size_t size) {
    static const char *const kinds[] = {"nothing", "the start of a part of submission", "the end of submission",
                                        "the start of upload batch", "the end of upload batch",
                                        "ASTC decode chunk", "the start of present",
                                        "the end of present", "draw",
                                        "the start of a QUADS conversion after draw",
                                        "the end of a QUADS conversion after draw", "dispatch"};
    unsigned k = code >> 28;
    const VkDrawNote *before = k == VK_CRUMB_QUADS || k == VK_CRUMB_QUADS_END ? note_of(code & DRAW_MAX) : NULL;
    if (!code) snprintf(o, size, "nothing yet");
    else if (before && before->dispatch)   // a QUADS conversion recorded after a dispatch
        snprintf(o, size, "the %s of a QUADS conversion after dispatch %u", k == VK_CRUMB_QUADS ? "start" : "end",
                 code & 0x0fffffff);
    else if (k < sizeof kinds / sizeof *kinds) snprintf(o, size, "%s %u", kinds[k], code & 0x0fffffff);
    else snprintf(o, size, "unknown code %#x", code);
}

// ---- a crumb per draw

int vk_crumbs_draws(void) { return notes != NULL; }


static const char *note_kind(const VkDrawNote *n) { return n->dispatch ? "dispatch" : "draw"; }

static void draw_line(const VkDrawNote *n) {
    if (n->dispatch) {
        char groups[48];
        if (n->dispatch == 2) snprintf(groups, sizeof groups, "work groups read by the GPU (indirect)");
        else snprintf(groups, sizeof groups, "%ux%ux%u work groups", n->groups[0], n->groups[1], n->groups[2]);
        rt_log("vk: GPU breadcrumbs: dispatch %u: frame %llu, submission %u, %s, shader cs %016llx", n->no,
               (unsigned long long)n->frame, n->sub, groups, (unsigned long long)n->vs);
        return;
    }
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

// The draw or dispatch a code names: its own number, or for a QUADS conversion the one recorded before it. 0: none.
static uint32_t draw_named(uint32_t code) {
    unsigned k = code >> 28;
    return k == VK_CRUMB_DRAW || k == VK_CRUMB_DISPATCH || k == VK_CRUMB_QUADS || k == VK_CRUMB_QUADS_END
               ? code & DRAW_MAX : 0;
}

// The draws and dispatches the GPU had been given and had not finished: from the last finished crumb's (everything
// before it had finished; when that crumb is not a draw's: from the first draw of the submission the last reached one
// is in) to the last reached. The first eight are written, where the one that hangs is, and the last.
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
        rt_log("vk: GPU breadcrumbs: the hang is %s the QUADS conversion recorded after %s %u; it and the next:",
               kind == VK_CRUMB_QUADS ? "in or after" : "after", note_kind(b), before);
        draw_line(b);
        if (next) draw_line(next);
        return;
    }
    if (kind != VK_CRUMB_DRAW && kind != VK_CRUMB_DISPATCH) return;
    uint32_t last = reached & DRAW_MAX;
    uint32_t stop = draw_named(finished);   // everything before this one had finished; it may not have
    const VkDrawNote *end = note_of(last);
    if (!end) {
        rt_log("vk: GPU breadcrumbs: no note of draw or dispatch %u is left", last);
        return;
    }
    if (stop == last) {   // both crumbs are this one's, recorded before it: the hang is in it or in what follows it
        const VkDrawNote *next = note_of(last < DRAW_MAX ? last + 1 : 1);
        rt_log("vk: GPU breadcrumbs: everything before %s %u had finished and the GPU never reached the crumb after "
               "it: the hang is in it or in the commands recorded after it (a barrier, the end of its pass, a copy); "
               "it and the one after it:", note_kind(end), last);
        draw_line(end);
        if (next) draw_line(next);
        return;
    }
    uint32_t first = last, count = 1;
    while (count < NOTES - 1) {
        uint32_t before = first > 1 ? first - 1 : DRAW_MAX;
        const VkDrawNote *b = note_of(before);
        if (!b || (!stop && b->sub != end->sub)) break;
        first = before, count++;
        if (before == stop) break;   // its own crumb was the last finished: it is the oldest that may not have
    }
    rt_log("vk: GPU breadcrumbs: %u draw%s or dispatch%s not finished, %u to %u%s", count, count == 1 ? "" : "s",
           count == 1 ? "" : "es", first, last, count > 8 ? "; the first eight and the last:" : ":");
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
    if (!n->dispatch && (!prev || prev->pass != n->pass || prev->sub != n->sub)) slot->after |= VK_DRAW_AFTER_NEW_PASS;
    if (n->dispatch) dispatches++, last_dispatch = draw_no;
    if (test_frame && n->frame >= test_frame && draw_no > 6) {
        test_frame = 0;
        rt_log("vk: GPU breadcrumbs: TOTK_DRAW_CRUMBS_TEST: the report of a lost device, with the GPU's own two codes "
               "first and then as if it had stopped six draws or dispatches before %s %u", note_kind(slot), draw_no);
        vk_crumbs_report();
        report_draws((uint32_t)VK_CRUMB_DRAW << 28 | draw_no, (uint32_t)VK_CRUMB_DRAW << 28 | (draw_no - 6));
        const VkDrawNote *d = last_dispatch ? note_of(last_dispatch) : NULL;
        rt_log("vk: GPU breadcrumbs: test: %u dispatch%s marked so far%s", dispatches, dispatches == 1 ? "" : "es",
               d ? "; the last one:" : "");
        if (d) draw_line(d);
        test_watch = 100000;
    }
    if (test_watch) {   // the test goes on until a code the GPU itself wrote names a draw: the markers do run there
        uint32_t reached = words[0];
        if (reached >> 28 == VK_CRUMB_DRAW || reached >> 28 == VK_CRUMB_DISPATCH) {
            rt_log("vk: GPU breadcrumbs: test: the GPU's own code named %s %u while %s %u was recorded",
                   reached >> 28 == VK_CRUMB_DRAW ? "draw" : "dispatch", reached & DRAW_MAX, note_kind(slot), draw_no);
            test_watch = 0;
        } else if (!--test_watch) {
            rt_log("vk: GPU breadcrumbs: test: no code of the GPU's own named a draw or dispatch in 100000 of them");
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

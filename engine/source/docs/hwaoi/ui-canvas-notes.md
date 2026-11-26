# 2D layer canvas above 1080p (patch set hwaoi_ui_canvas)

## How the game shows its 2D layer

The game (KT "kids" engine, screen layouts) lays the 2D layer out on a 1920x1080 canvas, in coordinates centred on
the screen (quads of -960..960 x -540..540). Three kinds of draws reach the window image:

1. **Perspective canvas** (frames, panels, menus, title/splash logos, movies' quads, HUD frames, minimap outline,
   map image): a screen camera with fov 45 deg, near 0.1, far 20000, standing at `size.h/2 / tan(22.5 deg)` from the
   canvas. In the UBO (vertex stage, binding 0) the combined matrix row `0 0 -1.00001 -1 | 0 0 d' d` shows `d`:
   1303.68 at 1080p, 1738.23 at 1440p, 869.1 at 720p. With `size` = output the canvas covers 1080/output_h of the
   screen: centred 0.75 at 1440p, cropped 1.5x at 720p (the Map screen at 720p is zoomed and loses its frame).
2. **Text** (glyph quads built on the CPU, vertices in output pixels): anchor * (output / layer size) + output/2.
   With layer size = output they sit 1:1 in a centred 1920x1080 box above 1080p.
3. **2D primitives** (map icons/diamonds/tower/green square, minimap marker and enemy dots): quads in canvas units
   from the top-left, drawn with the pixel ortho of the target. Above 1080p they shrink towards the upper left (the
   minimap ones fall outside the minimap and are clipped away); at 720p they spread to the lower right. The UBO rows
   `1.2 0 0 tx | 0 1 0 ty` seen next to these draws are stale contents of the constant ring; the vertex shader of these
   draws (sh 0e099825d5f3bbf0) only reads the ortho vec4: `pos = v.xy * c3[0].xy + c3[0].zw`.

The pixel ortho of every 2D draw is computed on the render thread from the bound target's size:
`(2*s/W, -2*t/H, -1, 1)` with s, t at ctx+0x28f4/0x28f8 (1.0 observed), in six copies at main+7194c8, 71cea4,
71de6c, 71ede0, 71f400, 726090. Forcing it to 2/1920 for the window target (host experiment) fixes the icons but
moves the text (output pixels) - the two use different spaces, so it is not a fix by itself.

## The patch

The layer setup at main+576f80 reads the layer's screen size from its config (`ldr x25, [x8]` at 576ff8, x8 =
config+0x88; 1920x1080 when absent), derives the perspective camera (inlined at 5770d0..57712c, tanf at 57711c),
stores the size at layer+0x690 and passes it to main+add610(camera, mode, size) for its cameras. hwaoi_ui_canvas
loads {1920,1080} from patch page +0xa8 there instead, and in add610's perspective path (add844/add874) for its
other callers. Result: the perspective canvas fills the output at every size and text scales with it.
HWAOI_UI_CANVAS=0 (TOTK_UI_CANVAS) restores the original code.

## The 2D primitive recorder (fixed)

Widgets draw sprites through main+15f2ca4(widget, const f32 corners[8], uv, color): it calls
`widget+0x28 -> vtable[1]` = main+af38c0, a method of the game's 2D primitive recorder (vtable at main+275fb28;
methods af3788..af42d8 record typed 0x60-byte items into batch = renderer+8: 0 quad, 3/5/7 state, ...). af38c0
converts the corners with main+af42dc and appends a type-0 item. af42dc copies the corners unchanged when the batch
has a target (batch+0x428, always set here: both batches seen, the Map and the battle HUD/minimap one); the other
branch maps them through batch+0xf0 (s16 screen size) and the anchor at +0x10c. Over a whole battle-slow2 replay at
1440p the only geometry method called was af38c0 (via 15f2ca4); the rest were state methods (af4074, af414c).

hwaoi_ui_canvas replaces the copy at af42e4..af42f0 by: `ldp q0,q1,[x2]; adrp x9,page; ldr s2,[x9,#0xb8];
fmul v0.4s/v1.4s by v2.s[0]; stp q0,q1,[x1]` (the original ldp/stp pairs become nops). Page +0xb8 = f32
output_w / 1920 (res_init). The corners reach the vertex buffer in output pixels, consistent with text.

### The recorder's other shapes (2026-11-23, Age of Decomp; not yet seen in a battle)

A report from 2560x1440: the enemies' dots are missing in the zoomed minimap and there in the whole-map view. The
recorder's geometry methods, read in the generated C (items of 0x60 bytes in batch = renderer+8):

| Method | Item | Positions | Above 1080p |
|---|---|---|---|
| main+af38c0, af3ea4 | 0: quad, four corners | through main+af42dc | scaled (the sites above) |
| main+af3788 (one colour), af3d7c (item 6) | quad from a rectangle x0 y0 x1 y1 | through main+af43d0 | was not scaled |
| main+af3b24 | 1: line, two points, colour, width (s0..s4) | stored as given for a batch with a target | was not scaled |
| main+af39f0 | 2: triangle, three points | stored as given, no branch on the target | not scaled, left alone |

main+af43d0, for a batch with a target, copies the rectangle's four numbers into four corners with eight `ldr s0` /
`str s0` pairs (af43d8..af4414). hwaoi_ui_canvas now follows each load with `fmul s0, s0, s2`, s2 = page +0xb8 loaded
before the first one (x9 and s2 are free there: both callers load them again after the call). main+af3b24 gets its
five kept arguments s12..s8 multiplied at af3b74, the first instruction of the path for a batch with a target (the
width too). The world map and the training field's HUD at 2560x1440 look the same with and without the nine sites.
If the zoomed minimap still lacks something, the triangles are next. resolution.c also writes the canvas (1920x1080)
and a scale of 1.0 to the page on the game's own 3D path (`RENDER_SCALE=game`), where the page was left at zero.

Since 2026-11-26 these nine sites are a set of their own, `hwaoi_ui_shapes` (`TOTK_UI_SHAPES`, on by default and only
together with `hwaoi_ui_canvas`, runtime/patches.c): GitHub #23 shows white bars behind each line of description text
(the Music menu, a character's unlock card) at 3440x1440 and none at 1920x1080, where every site of both sets is a
no-op. `ui_shapes = 0` turns off only the rectangles and lines, `ui_canvas = 0` the whole canvas.

A test on Windows the same day (RTX 5070 Ti, the Gallery): at 2560x1440 the character cards and the Music list are
clean; at 3840x2160 the Music list's lines are a mix of each other's titles (the first four rows show the start of their
title and then the end of the second row's, every row below the same mix), with `ui_shapes = 0` too, and clean with
`ui_canvas = 0`. So the four quad sites at af42e4 became a set of their own as well, `hwaoi_ui_quads`
(`TOTK_UI_QUADS`, `ui_quads = 0`), to tell the quads from the layer's camera size. With `ui_quads = 0` the list is as
mixed up as with every set on (row frames, icons and panels stay scaled; the character list's portraits move), so the
quads are not the cause: the layer size of 576ff8 / add844 / add874 is. Text is placed at anchor * (output / layer
size) + output/2, so above 1080p it is drawn larger than the layer believes; the idea to check next is that the list
draws its long titles (the scrolling text) into a target of its own sized in layer units, where text at twice the size
runs into the next row's slot. At 2560x1440 (x 1.33) it still fits, at 3440x1440 (x 1.79 across) and 3840x2160 (x 2)
it does not. That was it, with one shared texture rather than one per list: see *The shared text texture* below (set
`hwaoi_ui_text`).

How it was found: hardware write breakpoints (perf_event_open PERF_TYPE_BREAKPOINT, one event per thread, signal
handler logs host rip and the guest frame-pointer chain via kern_cpu()) on the corner values of one icon found in
memory (scan of rw mappings for the vertex buffer's first x,y), which pointed at main+15f2ca4; then a temporary hook in
rt_dispatch logging indirect calls by return address (c->x[30]) gave the target af38c0 and the batch fields.
(PROT_NONE + single-step watchpoints are too slow on the hot frame-allocator pages.)

### Earlier findings (kept for reference)

- The scale-policy sites of hwaoi_display (01432eb4...) are dynamic resolution, not UI. The 1920/1080 float pairs near
  1e9xxxx are DRS UV clamps. add610 mode 1 (ortho camera) forced to 1920x1080 changes nothing visible.
- Forcing the window draws' ortho to 2/1920 fixes the primitives but breaks text (output pixels): one producer has to
  change, which is what the af42dc patch does.

## The shared text texture (set hwaoi_ui_text, fixed 2026-11-26)

Some text panes are not drawn straight to the window but into one texture the whole UI shares, and copied from it to
the screen (the Gallery's Music list titles, which scroll when they are long; Options rows draw there too). The game's
settings object `[main+28a7e60]` turns it on (+0x37c) and gives its size (+0x380: 1800x1024, RGBA8): the layer setup
makes it at main+577cc4 (through main+700f00, kept at the layer's render object +0x38), and main+b2ce14 lays the panes
out in it before each frame. That layout reads the output's size (the render context's target +0x58) and the layer's
(the layer's screen object +0x150 / +0x154), computes k = output / layer size per axis (fdiv at main+b2cedc into s8, at
main+b2cee4 into s9), stores the texture's size at +0x248 as its capacity and keeps k in each pane (+0x13c / +0x140),
whose glyphs are then built k times their canvas size. On the console the layer is the output's size, so k = 1. With
the canvas patch the layer is 1920x1080, so k = 2 at 3840x2160 (1.33 at 2560x1440, 0.67 at 1280x720): the nine
titles of the Music list, 63 canvas pixels apart, need 1134 of the texture's 1024 lines and are all laid out at its
top, on top of each other, and each row's copy shows that mix. At 2560x1440 they still fit.

Frame dumps of the Music list (`TOTK_DUMP_TRIGGER`; 2026-11-26, RTX 5070 Ti): at 1920x1080 the frame clears the
1800x1024 texture, draws the nine titles into it at 1x, stacked from the top-left 63 px apart (one render pass, nine
DrawElementsBaseVertex, viewport and scissor 0 0 1800 1024), then copies each row's strip to the window in the row's own
pass. At 3840x2160 the same nine draws put every title at twice the size into the same top strip. A temporary probe in
SetRenderTargets (the guest chain of a target of a given size) and in TextureInitialize (who makes a texture of a given
size) found the binding on the render thread (main+719198, main+71a3dc) and the texture's maker (main+577cf8); a scan for
the settings object's +0x380 found the only other reader, the layout at main+b2ce94. The pane boxes with effects
(300x60 for a header, 1500x60 for the button guide, 480x270 for the glow) are other textures, drawn at 1x at every
output. A first attempt that scaled the layer's own targets at main+576fa0 (config +0xb0) changed nothing: that is not
the texture the titles go through.

hwaoi_ui_text replaces both divisions by `fmov s8, #1.0` / `fmov s9, #1.0`: the shared texture is laid out in canvas
units, as at 1920x1080, and the layer's camera scales the copies to the output with the rest of the canvas (the titles
are drawn at canvas resolution and scaled, like the header and the button guide). On only with hwaoi_ui_canvas
(runtime/patches.c), `ui_text = 0` (`TOTK_UI_TEXT`) turns it off alone. At 1920x1080 both divisions give 1 already.
Checked on 2026-11-26 (RTX 5070 Ti): the Music list clean at 3840x2160 and back to the mix with `ui_text = 0`; the
world map the same with and without it at 3840x2160; character cards clean at 3840x2160, 2560x1440 and 1920x1080;
2560x1440 compared with 0.3.5 (Music list, cards, world map, the training field's HUD) the same; 1920x1080 and
1280x720 clean. 3440x1440 is not an output the game takes (`OUTPUT_RES` has 1280x720, 1920x1080, 2560x1440 and
3840x2160: hwaoi_display.h), so it was not tried.

Not done: sharper titles would keep k and give the texture and its capacity k times the size instead (scale
`ldr x8, [x8, #0x380]` at main+577cc4 and `ldr x8, [x10, #0x380]` at main+b2ce94 by page +0x88 / page +0xa8 per axis;
x9 and v0..v2, resp. x9 and v2..v3 are free there). It depends on every reader of the copy's UVs taking the texture's
own size, which was not checked.

## Method

Temporary probe in nvn_api_cmd.c: on draws into the window image (SetRenderTargets' color 0 one of the window's textures)
log the guest frame-pointer chain, the first 256 bytes of each bound UBO and vertex buffer 0; a host-side rewrite of
the UBO's distance/ortho confirmed each hypothesis before patching. A scan of rw mappings for the camera distance
found the camera objects (near 0.1, far 20000, fov 0.785398), whose constants led to the 577xxx setup.

## Host experiment: origin-scaling instanced window draws (reverted)

With the canvas patch on, draws into the window image with more than one instance were given a viewport and scissor
scaled about the TOP-LEFT by pass_w / 1920 (record_fixed), to test the idea that the icons are instanced sprites in
canvas units. Window captures of the Map screen and the minimap at 3840x2160 against 1920x1080:
- The tower glyph on the Map screen landed exactly where it is at 1080p.
- The orange diamond markers and the green square still land at about 0.5-0.8 of their 1080p positions (up-left), so they
  are not drawn through the same instanced path (or are scaled by a second factor).
- The minimap objective marker and the orange enemy dots were not visible inside the minimap at 3840x2160 with or without
  the experiment; at 1080p they are. So at least two icon classes exist and the host cannot tell them apart by instance
  count. The fix has to be at the game-thread 2D batcher that writes the icon rows (see the notes above), not in the
  window pass. Capture method used: a temporary swapchain capture in present_image (menu_shot path), every 150th frame,
  with battle-slow2.txt (the Map screen appears when the replay loses sync at 4K).

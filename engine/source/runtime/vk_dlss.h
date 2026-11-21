// NVIDIA DLSS as a backend of the temporal path (vk_temporal.c): DLAA at the scene's own size (TEMPORAL=dlss with
// DLSS_QUALITY=native, or the older TEMPORAL=dlaa) and DLSS Super Resolution below it. Streamline itself is in
// aod_dlss.cpp; this file holds the settings, where the files are and the per-frame inputs.
#pragma once
#include "vk.h"

int vk_dlss_wanted(void);            // TEMPORAL = dlss or dlaa at start (the device may still lack DLSS)
// DLSS_QUALITY for the scene size: "native" (DLAA; always with TEMPORAL=dlaa), "quality", "balanced", "performance"
const char *vk_dlss_quality(void);
void vk_dlss_started(int asked);     // vk_device.c: whether DLSS was asked for when the device was made
int vk_dlss_available(void);         // Streamline runs DLSS on this device
int vk_dlss_failed(void);            // asked for at start but not running (no RTX card, a DLL missing, Streamline failed)
int vk_dlss_files_present(void);     // Streamline's DLLs and the player's nvngx_dlss.dll are in place
int vk_dlss_offered(void);           // the menu offers DLSS: an NVIDIA GPU and the files in place
// Where Streamline's DLLs are: TOTK_DLSS_DIR, else the streamline folder beside the executable. 0 when no path fits.
int vk_dlss_dir(char *dir, size_t cap);
// Where the player's nvngx_dlss.dll is, when not with Streamline's: TOTK_DLSS_NGX (the file or its folder), else the
// ngx folder beside the executable if it has the DLL (RecompLauncher's guide names <game folder>/ngx). 0: none.
int vk_dlss_ngx_dir(char *dir, size_t cap);

typedef struct {   // an image in VK_IMAGE_LAYOUT_GENERAL, level 0 and layer 0 of view
    VkImage image;
    VkImageView view;
    VkFormat format;
    uint32_t width, height;
    VkImageUsageFlags usage;
} VkDlssImage;

typedef struct {
    VkDlssImage color, depth, mv, out;   // depth: the R32F copy of the scene depth (reversed Z); mv: R16G16F
    float jx, jy;                        // this frame's jitter, render pixels (window x right, y down)
    const double *vp, *prev_vp;          // view-projections (row major, row vectors); NULL when unknown
    int reset;                           // a new history
    int hdr;                             // color is linear HDR, else the tone-mapped picture
    int moving;                          // mv holds the camera's motion (else zeros)
} VkDlssDispatch;

// Records DLSS into cb, outside any render pass. 0 on failure (logged). Streamline binds pipelines and descriptors of
// its own.
int vk_dlss_dispatch(VkCommandBuffer cb, const VkDlssDispatch *d);

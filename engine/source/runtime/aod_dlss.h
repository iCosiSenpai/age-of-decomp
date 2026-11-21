// NVIDIA DLSS through Streamline, loaded at run time: DLAA (DLSS at the scene's own size) and DLSS Super Resolution
// (a smaller scene scaled to the output). C interface of aod_dlss.cpp; vk_device.c calls the set-up part,
// gfx_present.c the present, vk_dlss.c the evaluation for the temporal path (vk_temporal.c).
#pragma once
#include <stddef.h>
#include <stdint.h>
#include <vulkan/vulkan.h>

#ifdef __cplusplus
extern "C" {
#endif

// An image DLSS reads or writes. It is in VK_IMAGE_LAYOUT_GENERAL whenever an evaluation runs.
typedef struct {
    VkImage image;
    VkImageView view;            // depth: a view of the depth aspect only
    VkFormat format;
    uint32_t width, height;
    VkImageUsageFlags usage;
    VkImageAspectFlags aspect;
} AodDlssImage;

// One frame's inputs. Matrices are row major for row vectors (Streamline's convention: p' = p * M) and carry no jitter.
typedef struct {
    AodDlssImage color_in, color_out, depth, mvec;   // color_in, depth and mvec at the render size, color_out the output's
    double view_to_clip[16], clip_to_view[16], clip_to_prev[16], prev_to_clip[16];
    double cam_pos[3], cam_up[3], cam_right[3], cam_fwd[3];
    double cam_near, cam_far, cam_fov, cam_aspect;
    float jitter[2];             // sub-pixel offset of this frame's scene draws, render pixels (x right, y down)
    int reset;                   // the previous frame has no connection to this one
    int depth_inverted;          // reversed Z: near is 1
    int hdr;                     // color_in is linear HDR (before the tone map), else display-referred
    // The motion vectors: last frame's window position minus this frame's, in render pixels, the camera's motion
    // included and no jitter in them. 0: the image is all zeros and Streamline derives the camera's motion itself.
    int mvec_camera;
    uint32_t frame;              // increases by one per evaluated frame
} AodDlssFrame;

// Before vkCreateInstance. Loads sl.interposer.dll from dir after checking NVIDIA's signature on it and on the
// plugins, and starts Streamline for DLSS (manual hooking, Vulkan, no over-the-air downloads). nvngx_dlss.dll is in
// ngx (a folder; NULL: dir), which Streamline searches after dir. 1 = loaded; 0 = DLSS is off for this run and why
// says why.
int aod_dlss_init(const char *dir, const char *ngx, char *why, size_t cap);
// After aod_dlss_init: the extensions DLSS needs (names stay valid until aod_dlss_shutdown).
int aod_dlss_instance_extensions(const char **names, int max);
int aod_dlss_device_extensions(const char **names, int max);
// After vkCreateDevice with those extensions: hands the device to Streamline and checks DLSS on this GPU. queues is
// the number of queues created in family (Streamline's own queues would follow them). 1 = DLSS is ready.
int aod_dlss_device(VkInstance instance, VkPhysicalDevice phys, VkDevice device, uint32_t family, uint32_t queues);
int aod_dlss_ready(void);
// vkQueuePresentKHR through Streamline once DLSS is ready (its once-per-frame bookkeeping), else the loader's.
VkResult aod_dlss_present(VkQueue queue, const VkPresentInfoKHR *info);
// Records DLSS into cb outside any render pass: DLAA when color_in and color_out have one size, else the DLSS mode
// nearest to their ratio. Streamline binds pipelines and descriptor sets of its own: the caller forgets the state it
// had bound. 1 = recorded; 0 = nothing was recorded (logged once per reason).
int aod_dlss_evaluate(VkCommandBuffer cb, const AodDlssFrame *f);
// Before vkDestroyDevice (the device is idle).
void aod_dlss_shutdown(void);

#ifdef __cplusplus
}
#endif

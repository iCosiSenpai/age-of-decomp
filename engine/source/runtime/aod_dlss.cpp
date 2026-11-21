// DLSS through NVIDIA Streamline v2.14.16 (aod_dlss.h), Age of Decomp's own code.
// Streamline's headers are MIT (third_party/streamline, LICENSE.txt), and so are its DLLs: sl.interposer.dll,
// sl.common.dll and sl.dlss.dll, signed by NVIDIA, are loaded at run time (llvm-mingw cannot link the SDK's MSVC
// import library) after their signature is checked with sl_security.h. nvngx_dlss.dll (NVIDIA RTX SDK licence) is
// never shipped: Streamline looks for the player's copy in the same folder.
// Manual hooking (the SDK's docs/ProgrammingGuideManualHooking.md): vk_device.c creates the instance and the device
// with the extensions slGetFeatureRequirements lists and hands them over with slSetVulkanInfo; gfx_present.c presents
// through Streamline's vkQueuePresentKHR, which runs its per-frame bookkeeping; after slEvaluateFeature the caller
// restores its own command buffer state. Over-the-air updates of Streamline and its plugins stay off.
#include "aod_dlss.h"

#include <stdio.h>
#include <string.h>

#ifdef _WIN32
#include <windows.h>

#include <string>
#include <vector>

#include "third_party/streamline/sl.h"
#include "third_party/streamline/sl_dlss.h"
#include "third_party/streamline/sl_helpers_vk.h"
#include "third_party/streamline/sl_security.h"

extern "C" {
#include "cfg.h"
#include "host.h"
}

namespace {

HMODULE g_mod;
PFun_slInit *sl_init;
PFun_slShutdown *sl_shutdown;
PFun_slIsFeatureSupported *sl_supported;
PFun_slGetFeatureRequirements *sl_requirements;
PFun_slSetVulkanInfo *sl_set_vulkan;
PFun_slGetFeatureFunction *sl_feature_fn;
PFun_slGetNewFrameToken *sl_frame_token;
PFun_slSetConstants *sl_constants;
PFun_slSetTagForFrame *sl_tag;
PFun_slEvaluateFeature *sl_evaluate;
PFun_slFreeResources *sl_free;
PFun_slDLSSSetOptions *dlss_options;
PFN_vkQueuePresentKHR g_present;   // Streamline's vkQueuePresentKHR once the device is handed over

std::vector<std::string> g_inst_ext, g_dev_ext;
std::wstring g_dir, g_ngx;
const wchar_t *g_paths[2];
int g_ready, g_verbose;            // g_verbose: TOTK_DLSS_LOG=verbose, Streamline's info messages too
uint32_t g_out_w, g_out_h, g_in_w, g_in_h;   // the sizes the DLSS options were set for
int g_hdr = -1;                              // ... and whether for HDR colour
const sl::ViewportHandle g_view{0u};
// A fixed project id for Streamline's engine identification (Preferences::projectId): a random UUID, not NVIDIA's.
const char *const k_project = "6b0d5a52-61a4-4b7e-9f5c-6f2d3e8c9a17";

void sl_log(sl::LogType type, const char *msg) {
    static volatile LONG logged;
    if (type == sl::LogType::eInfo && !g_verbose) return;
    if (InterlockedIncrement(&logged) > 300 && type != sl::LogType::eError) return;
    size_t n = strlen(msg);
    while (n && (msg[n - 1] == '\n' || msg[n - 1] == '\r')) n--;
    rt_log("dlss: streamline %s: %.*s", type == sl::LogType::eError ? "error" : type == sl::LogType::eWarn ? "warning" : "info",
           (int)n, msg);
}

const char *result_name(sl::Result r) {
    switch (r) {
    case sl::Result::eOk: return "ok";
    case sl::Result::eErrorIO: return "I/O error";
    case sl::Result::eErrorDriverOutOfDate: return "the NVIDIA driver is too old";
    case sl::Result::eErrorOSOutOfDate: return "Windows is too old";
    case sl::Result::eErrorOSDisabledHWS: return "hardware-accelerated GPU scheduling is off";
    case sl::Result::eErrorDeviceNotCreated: return "no device yet";
    case sl::Result::eErrorNoSupportedAdapterFound: return "no supported NVIDIA GPU";
    case sl::Result::eErrorAdapterNotSupported: return "this GPU does not support it";
    case sl::Result::eErrorNoPlugins: return "the Streamline plugins were not found";
    case sl::Result::eErrorVulkanAPI: return "Vulkan error";
    case sl::Result::eErrorFeatureMissing: return "the feature was not loaded";
    case sl::Result::eErrorFeatureNotSupported: return "not supported on this PC";
    case sl::Result::eErrorFeatureFailedToLoad: return "the feature failed to load (nvngx_dlss.dll missing?)";
    case sl::Result::eErrorMissingInputParameter: return "an input is missing";
    case sl::Result::eErrorNotInitialized: return "not initialised";
    default: return "error";
    }
}

std::wstring widen(const char *s) {
    int n = MultiByteToWideChar(CP_UTF8, 0, s, -1, NULL, 0);
    std::wstring w(n > 0 ? (size_t)n : 1, L'\0');
    if (n > 0) MultiByteToWideChar(CP_UTF8, 0, s, -1, &w[0], n);
    w.resize(wcslen(w.c_str()));
    return w;
}

template <class F> bool get(F *&f, const char *name) {
    f = reinterpret_cast<F *>(reinterpret_cast<void *>(GetProcAddress(g_mod, name)));
    return f != nullptr;
}

void unload() {
    if (g_mod) FreeLibrary(g_mod);
    g_mod = nullptr;
    g_ready = 0;
    g_present = nullptr;
    dlss_options = nullptr;
    g_inst_ext.clear();
    g_dev_ext.clear();
}

void put(sl::float4x4 &m, const double *a) {
    for (int r = 0; r < 4; r++)
        m.row[r] = sl::float4((float)a[4 * r], (float)a[4 * r + 1], (float)a[4 * r + 2], (float)a[4 * r + 3]);
}

void resource(sl::Resource &r, const AodDlssImage &i) {
    r.width = i.width, r.height = i.height, r.nativeFormat = (uint32_t)i.format;
    r.mipLevels = 1, r.arrayLayers = 1, r.flags = 0, r.usage = i.usage;
}

}   // namespace

extern "C" int aod_dlss_init(const char *dir, const char *ngx, char *why, size_t cap) {
    if (g_mod) return 1;
    g_dir = widen(dir);
    g_ngx = ngx && *ngx ? widen(ngx) : g_dir;
    // Every Streamline module must carry NVIDIA's signature; the interposer then checks its plugins itself.
    for (const wchar_t *name : {L"sl.interposer.dll", L"sl.common.dll", L"sl.dlss.dll"}) {
        std::wstring path = g_dir + L"\\" + name;
        if (GetFileAttributesW(path.c_str()) == INVALID_FILE_ATTRIBUTES) {
            snprintf(why, cap, "%ls is not in %s", name, dir);
            return 0;
        }
        if (!sl::security::verifyEmbeddedSignature(path.c_str())) {
            snprintf(why, cap, "%ls in %s is not signed by NVIDIA", name, dir);
            return 0;
        }
    }
    std::wstring model = g_ngx + L"\\nvngx_dlss.dll";
    if (GetFileAttributesW(model.c_str()) == INVALID_FILE_ATTRIBUTES) {
        snprintf(why, cap, "nvngx_dlss.dll is not in %s", ngx && *ngx ? ngx : dir);
        return 0;
    }
    std::wstring interposer = g_dir + L"\\sl.interposer.dll";
    g_mod = LoadLibraryExW(interposer.c_str(), NULL, LOAD_WITH_ALTERED_SEARCH_PATH);
    if (!g_mod) {
        snprintf(why, cap, "sl.interposer.dll could not be loaded (error %lu)", GetLastError());
        return 0;
    }
    if (!get(sl_init, "slInit") || !get(sl_shutdown, "slShutdown") || !get(sl_supported, "slIsFeatureSupported") ||
        !get(sl_requirements, "slGetFeatureRequirements") || !get(sl_set_vulkan, "slSetVulkanInfo") ||
        !get(sl_feature_fn, "slGetFeatureFunction") || !get(sl_frame_token, "slGetNewFrameToken") ||
        !get(sl_constants, "slSetConstants") || !get(sl_tag, "slSetTagForFrame") ||
        !get(sl_evaluate, "slEvaluateFeature") || !get(sl_free, "slFreeResources")) {
        snprintf(why, cap, "sl.interposer.dll lacks an entry point");
        unload();
        return 0;
    }
    static const sl::Feature features[] = {sl::kFeatureDLSS};
    g_paths[0] = g_dir.c_str();
    g_paths[1] = g_ngx.c_str();   // Streamline hands its plugin paths to NGX, which looks there for nvngx_dlss.dll
    sl::Preferences pref{};
    pref.showConsole = false;
    const char *log = cfg_get("TOTK_DLSS_LOG");   // developer switch: "verbose" for Streamline's full log
    g_verbose = log && !strcmp(log, "verbose");
    pref.logLevel = g_verbose ? sl::LogLevel::eVerbose : sl::LogLevel::eDefault;
    pref.pathsToPlugins = g_paths;
    pref.numPathsToPlugins = g_ngx == g_dir ? 1 : 2;
    pref.pathToLogsAndData = nullptr;   // no log file: messages go to the game log
    pref.logMessageCallback = sl_log;
    pref.flags = sl::PreferenceFlags::eDisableCLStateTracking | sl::PreferenceFlags::eUseManualHooking |
                 sl::PreferenceFlags::eUseFrameBasedResourceTagging | sl::PreferenceFlags::eDisableDebugText;
    pref.featuresToLoad = features;
    pref.numFeaturesToLoad = 1;
    pref.engine = sl::EngineType::eCustom;
    pref.engineVersion = "Age of Decomp";
    pref.projectId = k_project;
    pref.renderAPI = sl::RenderAPI::eVulkan;
    sl::Result r = sl_init(pref, sl::kSDKVersion);
    if (r != sl::Result::eOk) {
        snprintf(why, cap, "slInit: %s (%d)", result_name(r), (int)r);
        unload();
        return 0;
    }
    sl::FeatureRequirements req{};
    r = sl_requirements(sl::kFeatureDLSS, req);
    if (r != sl::Result::eOk || !(req.flags & sl::FeatureRequirementFlags::eVulkanSupported)) {
        snprintf(why, cap, "DLSS requirements: %s (%d)", r == sl::Result::eOk ? "no Vulkan support" : result_name(r), (int)r);
        sl_shutdown();
        unload();
        return 0;
    }
    for (uint32_t i = 0; i < req.vkNumInstanceExtensions; i++) g_inst_ext.emplace_back(req.vkInstanceExtensions[i]);
    for (uint32_t i = 0; i < req.vkNumDeviceExtensions; i++) g_dev_ext.emplace_back(req.vkDeviceExtensions[i]);
    rt_log("dlss: Streamline %u.%u.%u loaded from %s: %u instance and %u device extensions, %u+%u extra queues",
           (unsigned)SL_VERSION_MAJOR, (unsigned)SL_VERSION_MINOR, (unsigned)SL_VERSION_PATCH, dir,
           req.vkNumInstanceExtensions, req.vkNumDeviceExtensions, req.vkNumGraphicsQueuesRequired,
           req.vkNumComputeQueuesRequired);
    return 1;
}

extern "C" int aod_dlss_instance_extensions(const char **names, int max) {
    int n = 0;
    for (const std::string &e : g_inst_ext) if (n < max) names[n++] = e.c_str();
    return n;
}

extern "C" int aod_dlss_device_extensions(const char **names, int max) {
    int n = 0;
    for (const std::string &e : g_dev_ext) if (n < max) names[n++] = e.c_str();
    return n;
}

extern "C" int aod_dlss_device(VkInstance instance, VkPhysicalDevice phys, VkDevice device, uint32_t family, uint32_t queues) {
    if (!g_mod) return 0;
    sl::VulkanInfo info{};
    info.device = device, info.instance = instance, info.physicalDevice = phys;
    info.graphicsQueueFamily = family, info.graphicsQueueIndex = queues;   // where Streamline's own queues would start
    info.computeQueueFamily = family, info.computeQueueIndex = queues;
    sl::Result r = sl_set_vulkan(info);
    if (r != sl::Result::eOk) {
        rt_log("dlss: off: slSetVulkanInfo: %s (%d)", result_name(r), (int)r);
        sl_shutdown();
        unload();
        return 0;
    }
    sl::AdapterInfo adapter{};
    adapter.vkPhysicalDevice = phys;
    r = sl_supported(sl::kFeatureDLSS, adapter);
    void *fn = nullptr;
    if (r == sl::Result::eOk && (r = sl_feature_fn(sl::kFeatureDLSS, "slDLSSSetOptions", fn)) == sl::Result::eOk)
        dlss_options = reinterpret_cast<PFun_slDLSSSetOptions *>(fn);
    auto gdpa = reinterpret_cast<PFN_vkGetDeviceProcAddr>(reinterpret_cast<void *>(GetProcAddress(g_mod, "vkGetDeviceProcAddr")));
    g_present = gdpa ? reinterpret_cast<PFN_vkQueuePresentKHR>(gdpa(device, "vkQueuePresentKHR")) : nullptr;
    if (r != sl::Result::eOk || !dlss_options || !g_present) {
        rt_log("dlss: off: this GPU or driver cannot run DLSS (%s, %d)", result_name(r), (int)r);
        g_present = nullptr;
        sl_shutdown();
        unload();
        return 0;
    }
    g_ready = 1;
    rt_log("dlss: ready");
    return 1;
}

extern "C" int aod_dlss_ready(void) { return g_ready; }

extern "C" VkResult aod_dlss_present(VkQueue queue, const VkPresentInfoKHR *info) {
    PFN_vkQueuePresentKHR f = g_ready ? g_present : nullptr;
    return f ? f(queue, info) : vkQueuePresentKHR(queue, info);
}

extern "C" int aod_dlss_evaluate(VkCommandBuffer cb, const AodDlssFrame *f) {
    if (!g_ready) return 0;
    static int fails;
    sl::FrameToken *token = nullptr;
    uint32_t index = f->frame;
    sl::Result r = sl_frame_token(token, &index);
    if (r != sl::Result::eOk || !token) {
        if (fails++ < 5) rt_log("dlss: no frame token: %s (%d)", result_name(r), (int)r);
        return 0;
    }
    // w, h: the output; rw, rh: the render size the scene, the depth and the motion vectors have
    uint32_t w = f->color_out.width, h = f->color_out.height, rw = f->color_in.width, rh = f->color_in.height;
    if (w != g_out_w || h != g_out_h || rw != g_in_w || rh != g_in_h || f->hdr != g_hdr) {
        // The mode whose ratio is nearest: 1 DLAA, 1.5 quality, 1.72 balanced, 2 performance, 3 ultra performance.
        double ratio = rw ? (double)w / rw : 1.0;
        sl::DLSSMode mode = ratio < 1.2 ? sl::DLSSMode::eDLAA : ratio < 1.6 ? sl::DLSSMode::eMaxQuality
                          : ratio < 1.85 ? sl::DLSSMode::eBalanced : ratio < 2.5 ? sl::DLSSMode::eMaxPerformance
                          : sl::DLSSMode::eUltraPerformance;
        static const char *const names[] = {"off", "Performance", "Balanced", "Quality", "Ultra Performance", "Ultra Quality",
                                            "DLAA"};
        sl::DLSSOptions o{};
        o.mode = mode;
        o.outputWidth = w, o.outputHeight = h;
        o.colorBuffersHDR = f->hdr ? sl::Boolean::eTrue : sl::Boolean::eFalse;
        o.useAutoExposure = sl::Boolean::eTrue;
        if ((r = dlss_options(g_view, o)) != sl::Result::eOk) {
            if (fails++ < 5) rt_log("dlss: slDLSSSetOptions %ux%u -> %ux%u: %s (%d)", rw, rh, w, h, result_name(r), (int)r);
            return 0;
        }
        g_out_w = w, g_out_h = h, g_in_w = rw, g_in_h = rh, g_hdr = f->hdr;
        rt_log("dlss: %s, %ux%u -> %ux%u, %s colour", names[(uint32_t)mode], rw, rh, w, h, f->hdr ? "HDR" : "display");
    }

    sl::Constants c{};
    put(c.cameraViewToClip, f->view_to_clip);
    put(c.clipToCameraView, f->clip_to_view);
    static const double identity[16] = {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1};
    put(c.clipToLensClip, identity);
    put(c.clipToPrevClip, f->clip_to_prev);
    put(c.prevClipToClip, f->prev_to_clip);
    c.jitterOffset = sl::float2(f->jitter[0], f->jitter[1]);
    c.mvecScale = sl::float2(1.0f / (float)rw, 1.0f / (float)rh);   // render pixels to the -1..1 range Streamline wants
    c.cameraPinholeOffset = sl::float2(0.0f, 0.0f);
    c.cameraPos = sl::float3((float)f->cam_pos[0], (float)f->cam_pos[1], (float)f->cam_pos[2]);
    c.cameraUp = sl::float3((float)f->cam_up[0], (float)f->cam_up[1], (float)f->cam_up[2]);
    c.cameraRight = sl::float3((float)f->cam_right[0], (float)f->cam_right[1], (float)f->cam_right[2]);
    c.cameraFwd = sl::float3((float)f->cam_fwd[0], (float)f->cam_fwd[1], (float)f->cam_fwd[2]);
    c.cameraNear = (float)f->cam_near, c.cameraFar = (float)f->cam_far;
    c.cameraFOV = (float)f->cam_fov, c.cameraAspectRatio = (float)f->cam_aspect;
    c.depthInverted = f->depth_inverted ? sl::Boolean::eTrue : sl::Boolean::eFalse;
    // mvec_camera: the image holds the camera's motion (vk_temporal.c's motion pass). Otherwise it is zero and
    // Streamline adds the camera motion wherever it finds the invalid value, that is everywhere.
    c.cameraMotionIncluded = f->mvec_camera ? sl::Boolean::eTrue : sl::Boolean::eFalse;
    c.motionVectorsInvalidValue = f->mvec_camera ? -32768.0f : 0.0f;
    c.motionVectors3D = sl::Boolean::eFalse;
    c.reset = f->reset ? sl::Boolean::eTrue : sl::Boolean::eFalse;
    c.orthographicProjection = sl::Boolean::eFalse;
    c.motionVectorsDilated = sl::Boolean::eFalse;
    c.motionVectorsJittered = sl::Boolean::eFalse;
    if ((r = sl_constants(c, *token, g_view)) != sl::Result::eOk) {
        if (fails++ < 5) rt_log("dlss: slSetConstants: %s (%d)", result_name(r), (int)r);
        return 0;
    }

    const uint32_t general = VK_IMAGE_LAYOUT_GENERAL;
    sl::Resource in(sl::ResourceType::eTex2d, (void *)f->color_in.image, nullptr, (void *)f->color_in.view, general);
    sl::Resource out(sl::ResourceType::eTex2d, (void *)f->color_out.image, nullptr, (void *)f->color_out.view, general);
    sl::Resource depth(sl::ResourceType::eTex2d, (void *)f->depth.image, nullptr, (void *)f->depth.view, general);
    sl::Resource mvec(sl::ResourceType::eTex2d, (void *)f->mvec.image, nullptr, (void *)f->mvec.view, general);
    resource(in, f->color_in), resource(out, f->color_out), resource(depth, f->depth), resource(mvec, f->mvec);
    sl::SubresourceRange depth_range{};
    depth_range.aspectMask = f->depth.aspect ? f->depth.aspect : VK_IMAGE_ASPECT_DEPTH_BIT;   // an R32F copy: colour
    depth_range.baseMipLevel = 0, depth_range.levelCount = 1, depth_range.baseArrayLayer = 0, depth_range.layerCount = 1;
    depth.next = &depth_range;
    sl::Extent ext{0, 0, w, h}, rext{0, 0, rw, rh};
    sl::ResourceTag tags[] = {
        sl::ResourceTag(&in, sl::kBufferTypeScalingInputColor, sl::ResourceLifecycle::eValidUntilEvaluate, &rext),
        sl::ResourceTag(&out, sl::kBufferTypeScalingOutputColor, sl::ResourceLifecycle::eValidUntilEvaluate, &ext),
        sl::ResourceTag(&depth, sl::kBufferTypeDepth, sl::ResourceLifecycle::eValidUntilEvaluate, &rext),
        sl::ResourceTag(&mvec, sl::kBufferTypeMotionVectors, sl::ResourceLifecycle::eValidUntilEvaluate, &rext),
    };
    if ((r = sl_tag(*token, g_view, tags, 4, (sl::CommandBuffer *)cb)) != sl::Result::eOk) {
        if (fails++ < 5) rt_log("dlss: slSetTagForFrame: %s (%d)", result_name(r), (int)r);
        return 0;
    }
    const sl::BaseStructure *inputs[] = {&g_view};
    if ((r = sl_evaluate(sl::kFeatureDLSS, *token, inputs, 1, (sl::CommandBuffer *)cb)) != sl::Result::eOk) {
        if (fails++ < 5) rt_log("dlss: slEvaluateFeature: %s (%d)", result_name(r), (int)r);
        return 0;
    }
    return 1;
}

extern "C" void aod_dlss_shutdown(void) {
    if (!g_mod) return;
    if (g_ready && sl_free) sl_free(sl::kFeatureDLSS, g_view);
    sl_shutdown();
    unload();
}

#else   // Streamline is Windows-only: DLSS stays off elsewhere

extern "C" int aod_dlss_init(const char *dir, const char *ngx, char *why, size_t cap) {
    (void)dir, (void)ngx;
    snprintf(why, cap, "DLSS needs Windows");
    return 0;
}
extern "C" int aod_dlss_instance_extensions(const char **names, int max) { (void)names, (void)max; return 0; }
extern "C" int aod_dlss_device_extensions(const char **names, int max) { (void)names, (void)max; return 0; }
extern "C" int aod_dlss_device(VkInstance instance, VkPhysicalDevice phys, VkDevice device, uint32_t family, uint32_t queues) {
    (void)instance, (void)phys, (void)device, (void)family, (void)queues;
    return 0;
}
extern "C" int aod_dlss_ready(void) { return 0; }
extern "C" VkResult aod_dlss_present(VkQueue queue, const VkPresentInfoKHR *info) { return vkQueuePresentKHR(queue, info); }
extern "C" int aod_dlss_evaluate(VkCommandBuffer cb, const AodDlssFrame *f) { (void)cb, (void)f; return 0; }
extern "C" void aod_dlss_shutdown(void) {}

#endif

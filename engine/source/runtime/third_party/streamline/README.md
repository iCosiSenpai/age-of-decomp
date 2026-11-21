# NVIDIA Streamline headers

The `include/` headers of the Streamline SDK v2.14.16 (https://github.com/NVIDIA-RTX/Streamline, release asset
`streamline-sdk-v2.14.16.zip`, SHA-256 32041e51db0cab709a271b72a352281b690c2abbd669a827809401a5dd23956b), unmodified,
for `runtime/aod_dlss.cpp`. MIT licence: `LICENSE.txt` (the SDK's `license.txt`; its Nsight Perf exception covers
`sl_nvperf.h`, which is not here).

Only the headers live in this tree. The Streamline DLLs (`sl.interposer.dll`, `sl.common.dll`, `sl.dlss.dll`, MIT,
signed by NVIDIA) are loaded at run time from the player's engine folder, and `nvngx_dlss.dll` (NVIDIA RTX SDK licence)
is never shipped: it comes from the player's own PC.

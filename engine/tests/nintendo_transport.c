// Actual SDL mapping, sensor units, packet rejection, pipe delivery and child lifetime.
#include <assert.h>
#define AOD_NINTENDO_DLL
#include "../source/runtime/aod_nintendo.c"

static SDL_Gamepad *await_virtual_gamepad(void) {
    Uint64 deadline = SDL_GetTicks() + 1000;
    while (!joystick && SDL_GetTicks() < deadline) {
        aod_nintendo_tick();
        SDL_UpdateGamepads();
        SDL_Delay(10);
    }
    return joystick ? SDL_OpenGamepad(joystick_id) : NULL;
}

static int borrow_from_compiler(void) {
    assert(aod_nintendo_start(NULL, false));
    assert(!process); // Must use the existing helper, even with no path for a new one.
    SDL_Gamepad *pad = await_virtual_gamepad();
    assert(pad && SDL_GetGamepadButton(pad, SDL_GAMEPAD_BUTTON_EAST));
    assert(SDL_RumbleGamepad(pad, 12000, 16000, 100));
    SDL_CloseGamepad(pad);
    aod_nintendo_stop();
    SDL_Quit();
    return 0;
}

static void test_handoff(const char *helper) {
    assert(aod_nintendo_start(helper, true));
    SDL_Gamepad *pad = await_virtual_gamepad();
    assert(pad);
    SDL_CloseGamepad(pad);
    uintptr_t handles[3];
    assert(aod_nintendo_lend(handles) && !joystick);
    wchar_t self[32768], cmd[32768];
    assert(GetModuleFileNameW(NULL, self, 32768));
    assert(swprintf(cmd, 32768, L"\"%ls\" --receive-handoff", self) > 0);
    wchar_t handoff[128];
    assert(swprintf(handoff, 128, L"%lu:%llx:%llx:%llx", GetCurrentProcessId(), (unsigned long long)handles[0],
                    (unsigned long long)handles[1], (unsigned long long)handles[2]) > 0);
    assert(SetEnvironmentVariableW(L"AOD_NINTENDO_HANDOFF", handoff));
    STARTUPINFOW startup = {0};
    startup.cb = sizeof startup;
    PROCESS_INFORMATION child = {0};
    assert(CreateProcessW(self, cmd, NULL, NULL, FALSE, 0, NULL, NULL, &startup, &child));
    assert(SetEnvironmentVariableW(L"AOD_NINTENDO_HANDOFF", NULL));
    assert(WaitForSingleObject((HANDLE)handles[2], 2000) == WAIT_OBJECT_0);
    assert(WaitForSingleObject(child.hProcess, 2000) == WAIT_OBJECT_0);
    DWORD code;
    assert(GetExitCodeProcess(child.hProcess, &code) && code == 0);
    CloseHandle(child.hProcess);
    CloseHandle(child.hThread);
    assert(aod_nintendo_start(helper, true)); // Compiler takes input back from the same process.
    pad = await_virtual_gamepad();
    assert(pad && SDL_GetGamepadButton(pad, SDL_GAMEPAD_BUTTON_EAST));
    SDL_CloseGamepad(pad);
    aod_nintendo_stop();
}

int main(int argc, char **argv) {
    assert(argc == 2);
    SDL_SetHint("SDL_JOYSTICK_HIDAPI", "0");
    assert(SDL_Init(SDL_INIT_GAMEPAD));
    if (!strcmp(argv[1], "--receive-handoff")) return borrow_from_compiler();
    unsigned char b[64] = "AODPAD01";
    b[9] = 1; b[10] = 0x69; b[11] = 0x20;
    uint32_t bits = 0x10008; memcpy(b + 16, &bits, 4);
    float data[10] = {.5f, -.5f, 0, 0, 3.14159265f, 0, 0, 0, 0, 9.80665f};
    memcpy(b + 20, data, sizeof data);
    b[0] = 'X'; assert(!apply(b) && !joystick); b[0] = 'A';
    b[8] = 4; assert(!apply(b) && !joystick); b[8] = 0;
    data[4] = NAN; memcpy(b + 20, data, sizeof data); assert(!apply(b) && !joystick);
    data[4] = 3.14159265f; memcpy(b + 20, data, sizeof data);
    assert(apply(b)); SDL_UpdateGamepads();
    SDL_Gamepad *pad = SDL_OpenGamepad(joystick_id); assert(pad);
    assert(SDL_GetGamepadType(pad) == SDL_GAMEPAD_TYPE_NINTENDO_SWITCH_PRO);
    assert(SDL_GetGamepadButtonLabel(pad, SDL_GAMEPAD_BUTTON_EAST) == SDL_GAMEPAD_BUTTON_LABEL_A);
    assert(SDL_GetGamepadButton(pad, SDL_GAMEPAD_BUTTON_EAST));
    assert(!SDL_GetGamepadButton(pad, SDL_GAMEPAD_BUTTON_SOUTH));
    assert(SDL_GetGamepadButton(pad, SDL_GAMEPAD_BUTTON_DPAD_DOWN));
    assert(SDL_GetGamepadAxis(pad, SDL_GAMEPAD_AXIS_LEFTX) > 16000);
    assert(SDL_GetGamepadAxis(pad, SDL_GAMEPAD_AXIS_LEFTY) > 16000);
    assert(SDL_SetGamepadSensorEnabled(pad, SDL_SENSOR_GYRO, true));
    assert(SDL_SetGamepadSensorEnabled(pad, SDL_SENSOR_ACCEL, true));
    assert(apply(b)); SDL_UpdateGamepads();
    float v[3]; assert(SDL_GetGamepadSensorData(pad, SDL_SENSOR_GYRO, v, 3));
    assert(fabsf(v[0] - 3.14159265f) < .001f);
    assert(SDL_GetGamepadSensorData(pad, SDL_SENSOR_ACCEL, v, 3));
    assert(fabsf(v[2] - 9.80665f) < .001f);
    SDL_CloseGamepad(pad);
    b[9] = 0; assert(apply(b) && !joystick);

    assert(aod_nintendo_start(argv[1], true));
    HANDLE child; assert(DuplicateHandle(GetCurrentProcess(), process, GetCurrentProcess(), &child, 0, FALSE, DUPLICATE_SAME_ACCESS));
    Uint64 deadline = SDL_GetTicks() + 1000;
    while (!joystick && SDL_GetTicks() < deadline) { aod_nintendo_tick(); SDL_UpdateGamepads(); SDL_Delay(10); }
    assert(joystick);
    pad = SDL_OpenGamepad(joystick_id); assert(pad);
    assert(SDL_GetGamepadButton(pad, SDL_GAMEPAD_BUTTON_EAST));
    assert(SDL_RumbleGamepad(pad, 12000, 16000, 100));
    SDL_CloseGamepad(pad);
    deadline = SDL_GetTicks() + 3000;
    while (process && SDL_GetTicks() < deadline) { aod_nintendo_tick(); SDL_UpdateGamepads(); SDL_Delay(10); }
    assert(!process && !joystick);
    assert(WaitForSingleObject(child, 0) == WAIT_OBJECT_0); CloseHandle(child);

    aod_nintendo_stop(); // Explicit restart resets the once-per-SDL-session start guard.
    assert(aod_nintendo_start(argv[1], true));
    assert(DuplicateHandle(GetCurrentProcess(), process, GetCurrentProcess(), &child, 0, FALSE, DUPLICATE_SAME_ACCESS));
    aod_nintendo_stop();
    assert(WaitForSingleObject(child, 2000) == WAIT_OBJECT_0); CloseHandle(child);
    test_handoff(argv[1]);
    SDL_Quit();
    puts("Nintendo transport: packet validation, labels/buttons/axes, motion, pipes, rumble, handoff and cleanup passed.");
    return 0;
}

#pragma once

#include <stdint.h>

#if defined(_WIN32)
#if defined(CODEX_FLAMESHOT_BUILDING_DLL)
#define CODEX_FLAMESHOT_API __declspec(dllexport)
#else
#define CODEX_FLAMESHOT_API __declspec(dllimport)
#endif
#else
#define CODEX_FLAMESHOT_API __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef struct CodexFlameshotCaptureRequest {
    const char* output_path;
    uint32_t delay_ms;
    uint8_t accept_on_select;
} CodexFlameshotCaptureRequest;

typedef struct CodexFlameshotCaptureResult {
    char* message;
} CodexFlameshotCaptureResult;

enum {
    CODEX_FLAMESHOT_OK = 0,
    CODEX_FLAMESHOT_CANCELLED = 1,
    CODEX_FLAMESHOT_UNAVAILABLE = 2,
};

CODEX_FLAMESHOT_API int
codex_flameshot_capture_region(const CodexFlameshotCaptureRequest* request,
                               CodexFlameshotCaptureResult* result);
CODEX_FLAMESHOT_API void
codex_flameshot_capture_result_free(CodexFlameshotCaptureResult* result);

#ifdef __cplusplus
}
#endif

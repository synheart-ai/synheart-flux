#pragma once

/**
 * Synheart Flux - C FFI Header
 *
 * On-device compute engine for HSI-compliant human state signals.
 * This header defines the C API for calling Flux from other languages.
 *
 * Memory Management:
 * - All functions returning `char*` allocate new memory.
 * - The caller must free returned strings using `flux_free_string()`.
 * - Never free strings returned by `flux_last_error()` or `flux_version()`.
 *
 * Error Handling:
 * - Functions returning pointers return NULL on error.
 * - Functions returning int return non-zero on error.
 * - Call `flux_last_error()` to get the error message after an error.
 *
 * Thread Safety:
 * - Error messages are stored in thread-local storage.
 * - FluxProcessor instances are NOT thread-safe; use one per thread.
 */

#ifndef SYNHEART_FLUX_H
#define SYNHEART_FLUX_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ============================================================================
 * Opaque Types
 * ============================================================================ */

/**
 * Opaque handle to a FluxProcessor instance.
 * Created with flux_processor_new(), freed with flux_processor_free().
 */
typedef struct FluxProcessorHandle FluxProcessorHandle;

/* ============================================================================
 * Stateless API
 * ============================================================================ */

/**
 * Process WHOOP JSON and return HSI JSON array.
 *
 * @param json       Raw WHOOP API response JSON (null-terminated).
 * @param timezone   User's timezone, e.g., "America/New_York" (null-terminated).
 * @param device_id  Unique device identifier (null-terminated).
 *
 * @return Newly allocated JSON array string containing HSI payloads.
 *         Returns NULL on error; call flux_last_error() for details.
 *         Caller must free with flux_free_string().
 */
char* flux_whoop_to_hsi_daily(
    const char* json,
    const char* timezone,
    const char* device_id
);

/**
 * Process Garmin JSON and return HSI JSON array.
 *
 * @param json       Raw Garmin API response JSON (null-terminated).
 * @param timezone   User's timezone, e.g., "America/Los_Angeles" (null-terminated).
 * @param device_id  Unique device identifier (null-terminated).
 *
 * @return Newly allocated JSON array string containing HSI payloads.
 *         Returns NULL on error; call flux_last_error() for details.
 *         Caller must free with flux_free_string().
 */
char* flux_garmin_to_hsi_daily(
    const char* json,
    const char* timezone,
    const char* device_id
);

/* ============================================================================
 * Stateful Processor API
 * ============================================================================ */

/**
 * Create a new FluxProcessor with the specified baseline window.
 *
 * @param baseline_window_days  Number of days for rolling baseline (default: 14 if <= 0).
 *
 * @return Newly allocated FluxProcessor handle.
 *         Returns NULL on error.
 *         Caller must free with flux_processor_free().
 */
FluxProcessorHandle* flux_processor_new(int32_t baseline_window_days);

/**
 * Free a FluxProcessor instance.
 *
 * @param processor  Handle returned by flux_processor_new(). May be NULL.
 */
void flux_processor_free(FluxProcessorHandle* processor);

/**
 * Process WHOOP JSON with a stateful processor (maintains baselines).
 *
 * @param processor  FluxProcessor handle.
 * @param json       Raw WHOOP API response JSON (null-terminated).
 * @param timezone   User's timezone (null-terminated).
 * @param device_id  Unique device identifier (null-terminated).
 *
 * @return Newly allocated JSON array string containing HSI payloads.
 *         Returns NULL on error; call flux_last_error() for details.
 *         Caller must free with flux_free_string().
 */
char* flux_processor_process_whoop(
    FluxProcessorHandle* processor,
    const char* json,
    const char* timezone,
    const char* device_id
);

/**
 * Process Garmin JSON with a stateful processor (maintains baselines).
 *
 * @param processor  FluxProcessor handle.
 * @param json       Raw Garmin API response JSON (null-terminated).
 * @param timezone   User's timezone (null-terminated).
 * @param device_id  Unique device identifier (null-terminated).
 *
 * @return Newly allocated JSON array string containing HSI payloads.
 *         Returns NULL on error; call flux_last_error() for details.
 *         Caller must free with flux_free_string().
 */
char* flux_processor_process_garmin(
    FluxProcessorHandle* processor,
    const char* json,
    const char* timezone,
    const char* device_id
);

/**
 * Save processor baselines to JSON for persistence.
 *
 * @param processor  FluxProcessor handle.
 *
 * @return Newly allocated JSON string containing baseline state.
 *         Returns NULL on error; call flux_last_error() for details.
 *         Caller must free with flux_free_string().
 */
char* flux_processor_save_baselines(FluxProcessorHandle* processor);

/**
 * Load previously saved baselines into a processor.
 *
 * @param processor  FluxProcessor handle.
 * @param json       JSON string from flux_processor_save_baselines() (null-terminated).
 *
 * @return 0 on success, non-zero on error.
 *         On error, call flux_last_error() for details.
 */
int32_t flux_processor_load_baselines(
    FluxProcessorHandle* processor,
    const char* json
);

/* ============================================================================
 * Memory Management
 * ============================================================================ */

/**
 * Free a string returned by Flux functions.
 *
 * @param ptr  Pointer returned by a Flux function. May be NULL.
 *
 * Do NOT use this to free strings from flux_last_error() or flux_version().
 */
void flux_free_string(char* ptr);

/* ============================================================================
 * Error Handling
 * ============================================================================ */

/**
 * Get the last error message for the current thread.
 *
 * @return Pointer to error message string, or NULL if no error.
 *         The returned pointer is valid until the next Flux call on this thread.
 *         Do NOT free the returned pointer.
 */
const char* flux_last_error(void);

/* ============================================================================
 * Version Information
 * ============================================================================ */

/**
 * Get the Flux library version string.
 *
 * @return Pointer to version string (e.g., "0.1.0").
 *         Do NOT free the returned pointer.
 */
const char* flux_version(void);

#ifdef __cplusplus
}
#endif

#endif /* SYNHEART_FLUX_H */

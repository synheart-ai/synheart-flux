//! FFI bindings for Synheart Flux
//!
//! This module provides C-compatible functions for calling Flux from other languages.
//! All functions use C strings (null-terminated) and return allocated memory that
//! must be freed by the caller using `flux_free_string`.

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use crate::pipeline::{garmin_to_hsi_daily, whoop_to_hsi_daily, FluxProcessor};

// Thread-local storage for the last error message
thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

/// Set the last error message
fn set_last_error(msg: &str) {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = CString::new(msg).ok();
    });
}

/// Clear the last error message
fn clear_last_error() {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = None;
    });
}

/// Helper to convert C string to Rust string
unsafe fn cstr_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_string())
}

/// Helper to convert Rust string to C string (caller must free)
fn string_to_cstr(s: &str) -> *mut c_char {
    match CString::new(s) {
        Ok(cstr) => cstr.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Helper to convert a Vec<String> to a JSON array string
fn vec_to_json_array(vec: Vec<String>) -> String {
    // Each string is already valid JSON, so we join them as array elements
    let elements: Vec<&str> = vec.iter().map(|s| s.as_str()).collect();
    format!("[{}]", elements.join(","))
}

// ============================================================================
// Stateless API
// ============================================================================

/// Process WHOOP JSON and return HSI JSON array.
///
/// # Safety
/// - `json`, `timezone`, and `device_id` must be valid null-terminated C strings.
/// - Returns a newly allocated string that must be freed with `flux_free_string`.
/// - Returns NULL on error; call `flux_last_error` to get the error message.
#[no_mangle]
pub unsafe extern "C" fn flux_whoop_to_hsi_daily(
    json: *const c_char,
    timezone: *const c_char,
    device_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let json_str = match cstr_to_string(json) {
        Some(s) => s,
        None => {
            set_last_error("Invalid JSON string pointer");
            return ptr::null_mut();
        }
    };

    let tz_str = match cstr_to_string(timezone) {
        Some(s) => s,
        None => {
            set_last_error("Invalid timezone string pointer");
            return ptr::null_mut();
        }
    };

    let device_str = match cstr_to_string(device_id) {
        Some(s) => s,
        None => {
            set_last_error("Invalid device_id string pointer");
            return ptr::null_mut();
        }
    };

    match whoop_to_hsi_daily(json_str, tz_str, device_str) {
        Ok(payloads) => {
            let result = vec_to_json_array(payloads);
            string_to_cstr(&result)
        }
        Err(e) => {
            set_last_error(&e.to_string());
            ptr::null_mut()
        }
    }
}

/// Process Garmin JSON and return HSI JSON array.
///
/// # Safety
/// - `json`, `timezone`, and `device_id` must be valid null-terminated C strings.
/// - Returns a newly allocated string that must be freed with `flux_free_string`.
/// - Returns NULL on error; call `flux_last_error` to get the error message.
#[no_mangle]
pub unsafe extern "C" fn flux_garmin_to_hsi_daily(
    json: *const c_char,
    timezone: *const c_char,
    device_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let json_str = match cstr_to_string(json) {
        Some(s) => s,
        None => {
            set_last_error("Invalid JSON string pointer");
            return ptr::null_mut();
        }
    };

    let tz_str = match cstr_to_string(timezone) {
        Some(s) => s,
        None => {
            set_last_error("Invalid timezone string pointer");
            return ptr::null_mut();
        }
    };

    let device_str = match cstr_to_string(device_id) {
        Some(s) => s,
        None => {
            set_last_error("Invalid device_id string pointer");
            return ptr::null_mut();
        }
    };

    match garmin_to_hsi_daily(json_str, tz_str, device_str) {
        Ok(payloads) => {
            let result = vec_to_json_array(payloads);
            string_to_cstr(&result)
        }
        Err(e) => {
            set_last_error(&e.to_string());
            ptr::null_mut()
        }
    }
}

// ============================================================================
// Stateful Processor API
// ============================================================================

/// Opaque handle to a FluxProcessor
pub struct FluxProcessorHandle {
    processor: FluxProcessor,
}

/// Create a new FluxProcessor with the specified baseline window size.
///
/// # Safety
/// - Returns a pointer to a newly allocated FluxProcessor.
/// - Must be freed with `flux_processor_free`.
/// - Returns NULL on error.
#[no_mangle]
pub unsafe extern "C" fn flux_processor_new(baseline_window_days: i32) -> *mut FluxProcessorHandle {
    clear_last_error();

    let window_days = if baseline_window_days <= 0 {
        14 // Default
    } else {
        baseline_window_days as usize
    };

    let processor = FluxProcessor::with_baseline_window(window_days);
    let handle = Box::new(FluxProcessorHandle { processor });
    Box::into_raw(handle)
}

/// Free a FluxProcessor.
///
/// # Safety
/// - `processor` must be a valid pointer returned by `flux_processor_new`.
/// - After calling this function, the pointer is invalid.
#[no_mangle]
pub unsafe extern "C" fn flux_processor_free(processor: *mut FluxProcessorHandle) {
    if !processor.is_null() {
        drop(Box::from_raw(processor));
    }
}

/// Process WHOOP JSON with a stateful processor.
///
/// # Safety
/// - `processor` must be a valid pointer returned by `flux_processor_new`.
/// - `json`, `timezone`, and `device_id` must be valid null-terminated C strings.
/// - Returns a newly allocated string that must be freed with `flux_free_string`.
/// - Returns NULL on error; call `flux_last_error` to get the error message.
#[no_mangle]
pub unsafe extern "C" fn flux_processor_process_whoop(
    processor: *mut FluxProcessorHandle,
    json: *const c_char,
    timezone: *const c_char,
    device_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    if processor.is_null() {
        set_last_error("Null processor pointer");
        return ptr::null_mut();
    }

    let handle = &mut *processor;

    let json_str = match cstr_to_string(json) {
        Some(s) => s,
        None => {
            set_last_error("Invalid JSON string pointer");
            return ptr::null_mut();
        }
    };

    let tz_str = match cstr_to_string(timezone) {
        Some(s) => s,
        None => {
            set_last_error("Invalid timezone string pointer");
            return ptr::null_mut();
        }
    };

    let device_str = match cstr_to_string(device_id) {
        Some(s) => s,
        None => {
            set_last_error("Invalid device_id string pointer");
            return ptr::null_mut();
        }
    };

    match handle.processor.process_whoop(&json_str, &tz_str, &device_str) {
        Ok(payloads) => {
            let result = vec_to_json_array(payloads);
            string_to_cstr(&result)
        }
        Err(e) => {
            set_last_error(&e.to_string());
            ptr::null_mut()
        }
    }
}

/// Process Garmin JSON with a stateful processor.
///
/// # Safety
/// - `processor` must be a valid pointer returned by `flux_processor_new`.
/// - `json`, `timezone`, and `device_id` must be valid null-terminated C strings.
/// - Returns a newly allocated string that must be freed with `flux_free_string`.
/// - Returns NULL on error; call `flux_last_error` to get the error message.
#[no_mangle]
pub unsafe extern "C" fn flux_processor_process_garmin(
    processor: *mut FluxProcessorHandle,
    json: *const c_char,
    timezone: *const c_char,
    device_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    if processor.is_null() {
        set_last_error("Null processor pointer");
        return ptr::null_mut();
    }

    let handle = &mut *processor;

    let json_str = match cstr_to_string(json) {
        Some(s) => s,
        None => {
            set_last_error("Invalid JSON string pointer");
            return ptr::null_mut();
        }
    };

    let tz_str = match cstr_to_string(timezone) {
        Some(s) => s,
        None => {
            set_last_error("Invalid timezone string pointer");
            return ptr::null_mut();
        }
    };

    let device_str = match cstr_to_string(device_id) {
        Some(s) => s,
        None => {
            set_last_error("Invalid device_id string pointer");
            return ptr::null_mut();
        }
    };

    match handle.processor.process_garmin(&json_str, &tz_str, &device_str) {
        Ok(payloads) => {
            let result = vec_to_json_array(payloads);
            string_to_cstr(&result)
        }
        Err(e) => {
            set_last_error(&e.to_string());
            ptr::null_mut()
        }
    }
}

/// Save processor baselines to JSON.
///
/// # Safety
/// - `processor` must be a valid pointer returned by `flux_processor_new`.
/// - Returns a newly allocated string that must be freed with `flux_free_string`.
/// - Returns NULL on error; call `flux_last_error` to get the error message.
#[no_mangle]
pub unsafe extern "C" fn flux_processor_save_baselines(
    processor: *mut FluxProcessorHandle,
) -> *mut c_char {
    clear_last_error();

    if processor.is_null() {
        set_last_error("Null processor pointer");
        return ptr::null_mut();
    }

    let handle = &*processor;

    match handle.processor.save_baselines() {
        Ok(json) => string_to_cstr(&json),
        Err(e) => {
            set_last_error(&e.to_string());
            ptr::null_mut()
        }
    }
}

/// Load processor baselines from JSON.
///
/// # Safety
/// - `processor` must be a valid pointer returned by `flux_processor_new`.
/// - `json` must be a valid null-terminated C string.
/// - Returns 0 on success, non-zero on error.
/// - On error, call `flux_last_error` to get the error message.
#[no_mangle]
pub unsafe extern "C" fn flux_processor_load_baselines(
    processor: *mut FluxProcessorHandle,
    json: *const c_char,
) -> i32 {
    clear_last_error();

    if processor.is_null() {
        set_last_error("Null processor pointer");
        return -1;
    }

    let handle = &mut *processor;

    let json_str = match cstr_to_string(json) {
        Some(s) => s,
        None => {
            set_last_error("Invalid JSON string pointer");
            return -1;
        }
    };

    match handle.processor.load_baselines(&json_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(&e.to_string());
            -1
        }
    }
}

// ============================================================================
// Memory Management
// ============================================================================

/// Free a string returned by Flux functions.
///
/// # Safety
/// - `ptr` must be a valid pointer returned by a Flux function, or NULL.
/// - After calling this function, the pointer is invalid.
#[no_mangle]
pub unsafe extern "C" fn flux_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

// ============================================================================
// Error Handling
// ============================================================================

/// Get the last error message.
///
/// # Safety
/// - Returns a pointer to a thread-local error string.
/// - The returned pointer is valid until the next Flux function call on this thread.
/// - Do NOT free the returned pointer.
/// - Returns NULL if no error occurred.
#[no_mangle]
pub unsafe extern "C" fn flux_last_error() -> *const c_char {
    LAST_ERROR.with(|e| {
        match &*e.borrow() {
            Some(cstr) => cstr.as_ptr(),
            None => ptr::null(),
        }
    })
}

// ============================================================================
// Version Information
// ============================================================================

/// Get the Flux library version.
///
/// # Safety
/// - Returns a pointer to a static string. Do NOT free.
#[no_mangle]
pub unsafe extern "C" fn flux_version() -> *const c_char {
    // Use a static CString to avoid allocation
    static VERSION: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();
    VERSION.as_ptr() as *const c_char
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn sample_whoop_json() -> CString {
        CString::new(r#"{
            "sleep": [{
                "id": 1,
                "start": "2024-01-15T22:30:00.000Z",
                "end": "2024-01-16T06:30:00.000Z",
                "score": {
                    "stage_summary": {
                        "total_in_bed_time_milli": 28800000,
                        "total_awake_time_milli": 1800000,
                        "total_light_sleep_time_milli": 12600000,
                        "total_slow_wave_sleep_time_milli": 7200000,
                        "total_rem_sleep_time_milli": 7200000,
                        "total_sleep_time_milli": 27000000,
                        "disturbance_count": 3
                    },
                    "sleep_performance_percentage": 85.0,
                    "respiratory_rate": 14.5
                }
            }],
            "recovery": [{
                "cycle_id": 1,
                "created_at": "2024-01-15T06:30:00.000Z",
                "score": {
                    "recovery_score": 75.0,
                    "resting_heart_rate": 52.0,
                    "hrv_rmssd_milli": 65.0
                }
            }],
            "cycle": [{
                "id": 1,
                "start": "2024-01-15T06:30:00.000Z",
                "end": "2024-01-15T22:30:00.000Z",
                "score": {
                    "strain": 12.5,
                    "kilojoule": 8500.0,
                    "average_heart_rate": 72.0,
                    "max_heart_rate": 165.0
                }
            }]
        }"#).unwrap()
    }

    #[test]
    fn test_ffi_whoop_to_hsi_daily() {
        let json = sample_whoop_json();
        let tz = CString::new("America/New_York").unwrap();
        let device = CString::new("test-device").unwrap();

        unsafe {
            let result = flux_whoop_to_hsi_daily(
                json.as_ptr(),
                tz.as_ptr(),
                device.as_ptr(),
            );

            assert!(!result.is_null());

            let result_str = CStr::from_ptr(result).to_str().unwrap();
            assert!(result_str.starts_with('['));
            assert!(result_str.contains("hsi_version"));

            flux_free_string(result);
        }
    }

    #[test]
    fn test_ffi_processor_lifecycle() {
        unsafe {
            // Create processor
            let processor = flux_processor_new(7);
            assert!(!processor.is_null());

            // Process data
            let json = sample_whoop_json();
            let tz = CString::new("UTC").unwrap();
            let device = CString::new("device").unwrap();

            let result = flux_processor_process_whoop(
                processor,
                json.as_ptr(),
                tz.as_ptr(),
                device.as_ptr(),
            );
            assert!(!result.is_null());
            flux_free_string(result);

            // Save baselines
            let baselines = flux_processor_save_baselines(processor);
            assert!(!baselines.is_null());

            // Load baselines into new processor
            let processor2 = flux_processor_new(7);
            let load_result = flux_processor_load_baselines(processor2, baselines);
            assert_eq!(load_result, 0);

            flux_free_string(baselines);
            flux_processor_free(processor);
            flux_processor_free(processor2);
        }
    }

    #[test]
    fn test_ffi_error_handling() {
        unsafe {
            let invalid_json = CString::new("not json").unwrap();
            let tz = CString::new("UTC").unwrap();
            let device = CString::new("device").unwrap();

            let result = flux_whoop_to_hsi_daily(
                invalid_json.as_ptr(),
                tz.as_ptr(),
                device.as_ptr(),
            );

            assert!(result.is_null());

            let error = flux_last_error();
            assert!(!error.is_null());

            let error_str = CStr::from_ptr(error).to_str().unwrap();
            assert!(!error_str.is_empty());
        }
    }

    #[test]
    fn test_ffi_version() {
        unsafe {
            let version = flux_version();
            assert!(!version.is_null());

            let version_str = CStr::from_ptr(version).to_str().unwrap();
            assert!(!version_str.is_empty());
        }
    }
}

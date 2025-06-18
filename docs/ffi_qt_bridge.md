# FFI Bridge: Rust <-> Qt/C++ Integration

This document outlines the Foreign Function Interface (FFI) between the Rust core and the C++/Qt user interface, including concrete usage examples, error code handling, and best practices.

## Overview
- **Rust Side:** Exposes `extern "C"` functions for safe FFI.
- **C++ Side:** Calls these functions via a bridge class (e.g., `RustServiceBridge`).
- **Memory Management:** The side that allocates memory is responsible for freeing it. Rust provides explicit free functions for strings and structs.

## Example: Exposing a Rust Function to Qt

### Rust (src/ffi.rs)
```rust
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Returns a status string for a device. Caller must free the string.
#[no_mangle]
pub extern "C" fn rtp_midi_get_device_status(device_id: u32) -> *mut c_char {
    let status = format!("Device {} OK", device_id);
    match CString::new(status) {
        Ok(cstr) => cstr.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rtp_midi_free_string(s: *mut c_char) {
    if !s.is_null() {
        let _ = CString::from_raw(s);
    }
}
```

### C++ (qt_ui/rust_service_bridge.h)
```cpp
extern "C" {
    char* rtp_midi_get_device_status(uint32_t device_id);
    void rtp_midi_free_string(char* s);
}

class RustServiceBridge {
public:
    QString getDeviceStatus(uint32_t deviceId);
};
```

### C++ (qt_ui/rust_service_bridge.cpp)
```cpp
QString RustServiceBridge::getDeviceStatus(uint32_t deviceId) {
    char* status_c_str = rtp_midi_get_device_status(deviceId);
    if (!status_c_str) {
        return QString("Error: Failed to get device status from Rust backend.");
    }
    auto custom_deleter = [](char* p){ rtp_midi_free_string(p); };
    std::unique_ptr<char, decltype(custom_deleter)> guard(status_c_str, custom_deleter);
    return QString::fromUtf8(guard.get());
}
```

## Error Handling
- All FFI functions must check for null pointers and catch panics using `std::panic::catch_unwind`.
- Return null pointers or error codes on failure.

## Best Practices
- Document all FFI functions and their safety requirements.
- Provide explicit free functions for all heap-allocated memory passed across the boundary.
- Use `catch_unwind` to prevent Rust panics from crossing the FFI boundary.
- Add integration tests for FFI functions.

## Example Error Codes
- Return `nullptr` for string-returning functions on error.
- For functions returning integers, use negative values for error codes (e.g., `-1` for general error).

## See Also
- `platform/src/ffi.rs` for implementation.
- `qt_ui/rust_service_bridge.h/.cpp` for C++ bridge code.
- `.cursor/rules/ffi_qt_bridge.mdc` for project rules. 
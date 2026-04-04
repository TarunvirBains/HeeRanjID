use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_int};

use heeranjid::{HeerId, RanjId};

// ── Error handling ──────────────────────────────────────────────────────

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

fn set_last_error(msg: impl Into<String>) {
    LAST_ERROR.with(|e| *e.borrow_mut() = msg.into());
}

/// Returns 0 if no error is stored.
/// Otherwise copies the last error message into `buf` (up to `buf_len` bytes
/// including the NUL terminator) and returns the number of bytes written
/// (excluding the NUL). If the buffer is too small the message is truncated.
///
/// # Safety
///
/// `buf` must point to a writable buffer of at least `buf_len` bytes, or be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn heer_last_error(buf: *mut c_char, buf_len: c_int) -> c_int {
    LAST_ERROR.with(|e| {
        let msg = e.borrow();
        if msg.is_empty() {
            return 0;
        }
        if buf.is_null() || buf_len <= 0 {
            return msg.len() as c_int;
        }
        let max = (buf_len as usize) - 1; // reserve space for NUL
        let copy_len = msg.len().min(max);
        unsafe {
            std::ptr::copy_nonoverlapping(msg.as_ptr(), buf as *mut u8, copy_len);
            *buf.add(copy_len) = 0; // NUL terminator
        }
        copy_len as c_int
    })
}

// ── HeerId types and functions ──────────────────────────────────────────

/// HeerId is represented as a plain i64 across the FFI boundary.
pub type HeerIdT = i64;

/// Decode a HeerId into its component parts.
/// Returns 0 on success, -1 on error (check `heer_last_error`).
///
/// # Safety
///
/// Output pointers must be valid or null. Null pointers are safely skipped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn heer_id_decode(
    id: HeerIdT,
    timestamp_ms: *mut u64,
    node_id: *mut u16,
    sequence: *mut u16,
) -> c_int {
    match HeerId::from_i64(id) {
        Ok(hid) => {
            let parts = hid.into_parts();
            unsafe {
                if !timestamp_ms.is_null() {
                    *timestamp_ms = parts.timestamp_ms;
                }
                if !node_id.is_null() {
                    *node_id = parts.node_id;
                }
                if !sequence.is_null() {
                    *sequence = parts.sequence;
                }
            }
            0
        }
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Convert a HeerId to its string representation.
/// Writes into `buf` (up to `buf_len` bytes including NUL).
/// Returns the number of bytes written (excluding NUL) on success, -1 on error.
///
/// # Safety
///
/// `buf` must point to a writable buffer of at least `buf_len` bytes, or be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn heer_id_to_string(id: HeerIdT, buf: *mut c_char, buf_len: c_int) -> c_int {
    match HeerId::from_i64(id) {
        Ok(hid) => {
            let s = hid.to_string();
            if buf.is_null() || buf_len <= 0 {
                set_last_error("null buffer");
                return -1;
            }
            let max = (buf_len as usize) - 1;
            if s.len() > max {
                set_last_error("buffer too small");
                return -1;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(s.as_ptr(), buf as *mut u8, s.len());
                *buf.add(s.len()) = 0;
            }
            s.len() as c_int
        }
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Parse a HeerId from a NUL-terminated string.
/// On success writes the result into `*out` and returns 0.
/// On error returns -1.
///
/// # Safety
///
/// `s` must be a valid NUL-terminated C string. `out` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn heer_id_from_string(s: *const c_char, out: *mut HeerIdT) -> c_int {
    if s.is_null() || out.is_null() {
        set_last_error("null pointer");
        return -1;
    }
    let cstr = unsafe { CStr::from_ptr(s) };
    let rust_str = match cstr.to_str() {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e.to_string());
            return -1;
        }
    };
    match rust_str.parse::<HeerId>() {
        Ok(hid) => {
            unsafe { *out = hid.as_i64() };
            0
        }
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

// ── RanjId types and functions ──────────────────────────────────────────

/// RanjId is represented as 16 raw UUID bytes across the FFI boundary.
#[repr(C)]
pub struct RanjIdT {
    pub bytes: [u8; 16],
}

/// Decode a RanjId into its component parts.
/// Returns 0 on success, -1 on error.
///
/// # Safety
///
/// `id` must point to a valid `RanjIdT`. Output pointers must be valid or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ranj_id_decode(
    id: *const RanjIdT,
    timestamp_us: *mut u64,
    node_id: *mut u16,
    sequence: *mut u16,
) -> c_int {
    if id.is_null() {
        set_last_error("null pointer");
        return -1;
    }
    let bytes = unsafe { &(*id).bytes };
    let uuid = uuid::Uuid::from_bytes(*bytes);
    match RanjId::from_uuid(uuid) {
        Ok(rid) => {
            let parts = rid.into_parts();
            unsafe {
                if !timestamp_us.is_null() {
                    // RanjId timestamp can be up to 90 bits, but for practical use
                    // it fits in u64 for the foreseeable future.
                    *timestamp_us = parts.timestamp_micros as u64;
                }
                if !node_id.is_null() {
                    *node_id = parts.node_id;
                }
                if !sequence.is_null() {
                    *sequence = parts.sequence;
                }
            }
            0
        }
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Convert a RanjId to its UUID string representation.
/// Writes into `buf` (up to `buf_len` bytes including NUL).
/// Returns bytes written (excluding NUL) on success, -1 on error.
///
/// # Safety
///
/// `id` must point to a valid `RanjIdT`. `buf` must be writable for `buf_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ranj_id_to_string(
    id: *const RanjIdT,
    buf: *mut c_char,
    buf_len: c_int,
) -> c_int {
    if id.is_null() {
        set_last_error("null pointer");
        return -1;
    }
    let bytes = unsafe { &(*id).bytes };
    let uuid = uuid::Uuid::from_bytes(*bytes);
    match RanjId::from_uuid(uuid) {
        Ok(rid) => {
            let s = rid.to_string();
            if buf.is_null() || buf_len <= 0 {
                set_last_error("null buffer");
                return -1;
            }
            let max = (buf_len as usize) - 1;
            if s.len() > max {
                set_last_error("buffer too small");
                return -1;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(s.as_ptr(), buf as *mut u8, s.len());
                *buf.add(s.len()) = 0;
            }
            s.len() as c_int
        }
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Parse a RanjId from a NUL-terminated UUID string.
/// On success writes the result into `*out` and returns 0.
/// On error returns -1.
///
/// # Safety
///
/// `s` must be a valid NUL-terminated C string. `out` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ranj_id_from_string(s: *const c_char, out: *mut RanjIdT) -> c_int {
    if s.is_null() || out.is_null() {
        set_last_error("null pointer");
        return -1;
    }
    let cstr = unsafe { CStr::from_ptr(s) };
    let rust_str = match cstr.to_str() {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e.to_string());
            return -1;
        }
    };
    match rust_str.parse::<RanjId>() {
        Ok(rid) => {
            unsafe {
                (*out).bytes = *rid.as_uuid().as_bytes();
            }
            0
        }
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

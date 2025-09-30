use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;

use windows_sys::core::PWSTR;

/// Convert a Rust string into a null-terminated UTF-16 vector suitable for
/// Windows API calls.
pub(super) fn to_wide(value: &str) -> Vec<u16> {
  OsStr::new(value).encode_wide().chain(once(0)).collect()
}

/// Convert a PWSTR pointing to a null-terminated UTF-16 string into a Rust
/// `String`.
pub(super) unsafe fn pwstr_to_string(value: PWSTR) -> String {
  if value.is_null() {
    return String::new();
  }

  let mut len = 0usize;
  loop {
    // SAFETY: The caller must ensure `value` points to a valid null-terminated
    // UTF-16 string
    let current = unsafe { *value.add(len) };
    if current == 0 {
      break;
    }
    len += 1;
  }

  // SAFETY: We've determined the length by finding the null terminator
  let slice = unsafe { std::slice::from_raw_parts(value as *const u16, len) };
  String::from_utf16_lossy(slice)
}

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
    let current = *value.add(len);
    if current == 0 {
      break;
    }
    len += 1;
  }

  let slice = std::slice::from_raw_parts(value as *const u16, len);
  String::from_utf16_lossy(slice)
}

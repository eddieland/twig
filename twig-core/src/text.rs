//! Text formatting helpers that mirror the zero-cost style of `owo-colors`.
//!
//! The goal is to provide lightweight wrappers that are cheap to compose and
//! only allocate when the caller explicitly asks for an owned value. OSC 8
//! hyperlinks are the first use-case. OSC 8 encodes hyperlinks as escape
//! sequences (`ESC ] 8 ;; <url> BEL ... ESC ] 8 ;; BEL`). Modern terminals
//! generally render the label as a clickable link and older ones will show the
//! raw text while ignoring the control codes, so the output degrades
//! gracefully.
//!
//! # Global Override
//!
//! The [`set_hyperlinks_override`] function allows globally disabling hyperlink
//! output, which is useful for CLI flags like `--no-links`. When set to
//! `false`, all hyperlinks will fall back to plain text output regardless of
//! `ColorMode`.
//!
//! # Examples
//! ```
//! use twig_core::ColorMode;
//! use twig_core::text::{HyperlinkExt, hyperlink};
//!
//! // Fluent style via the extension trait.
//! let rendered = format!(
//!   "{}",
//!   "Twig Docs".hyperlink("https://example.com/docs", ColorMode::Yes)
//! );
//! assert!(rendered.contains("\x1b]8;;https://example.com/docs\x07Twig Docs\x1b]8;;\x07"));
//!
//! // Free-function style when importing a trait feels noisy.
//! let rendered = format!(
//!   "{}",
//!   hyperlink(&"Twig Repo", "https://example.com/repo", ColorMode::No)
//! );
//! assert_eq!(rendered, "Twig Repo (https://example.com/repo)");
//! ```

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag to force hyperlinks off. When `true`, hyperlinks are disabled
/// regardless of `ColorMode`.
static HYPERLINKS_DISABLED: AtomicBool = AtomicBool::new(false);

/// Globally disable or enable hyperlink output.
///
/// When set to `false`, all hyperlinks will render as plain text with the URL
/// appended in parentheses, regardless of the `ColorMode` passed to individual
/// hyperlink calls.
///
/// This is useful for CLI flags like `--no-links` that should override all
/// hyperlink behavior.
///
/// # Example
/// ```
/// use twig_core::ColorMode;
/// use twig_core::text::{hyperlink, set_hyperlinks_override};
///
/// set_hyperlinks_override(false);
/// let output = format!(
///   "{}",
///   hyperlink(&"link", "https://example.com", ColorMode::Yes)
/// );
/// assert_eq!(output, "link (https://example.com)");
///
/// set_hyperlinks_override(true);
/// let output = format!(
///   "{}",
///   hyperlink(&"link", "https://example.com", ColorMode::Yes)
/// );
/// assert!(output.contains("\x1b]8;;"));
/// ```
#[inline]
pub fn set_hyperlinks_override(enabled: bool) {
  HYPERLINKS_DISABLED.store(!enabled, Ordering::SeqCst);
}

/// Check whether hyperlinks are currently disabled via the global override.
#[inline]
pub fn hyperlinks_disabled() -> bool {
  HYPERLINKS_DISABLED.load(Ordering::SeqCst)
}

use crate::output::ColorMode;

/// An OSC 8 hyperlink that defers allocation until formatting occurs.
///
/// When colors are enabled, the hyperlink emits the OSC 8 start and end
/// sequences around the provided label. When colors are disabled, it falls back
/// to a text-only representation so logs remain readable.
pub struct Hyperlink<'a, T: fmt::Display + ?Sized> {
  label: &'a T,
  url: &'a str,
  enabled: bool,
}

impl<'a, T: fmt::Display + ?Sized> Hyperlink<'a, T> {
  /// Create a new hyperlink wrapper.
  #[inline]
  pub fn new(label: &'a T, url: &'a str, colors: ColorMode) -> Self {
    Self {
      label,
      url,
      enabled: colors != ColorMode::No && !hyperlinks_disabled(),
    }
  }

  /// Explicitly toggle hyperlink output.
  ///
  /// This is useful for tests or for callers that want to gate hyperlink
  /// emission on their own capability detection.
  #[inline]
  pub fn with_enabled(mut self, enabled: bool) -> Self {
    self.enabled = enabled;
    self
  }
}

impl<T: fmt::Display + ?Sized> fmt::Display for Hyperlink<'_, T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if self.enabled {
      // Start OSC 8 hyperlink, write the label, then close the sequence.
      write!(f, "\x1b]8;;{}\x07", self.url)?;
      fmt::Display::fmt(self.label, f)?;
      write!(f, "\x1b]8;;\x07")
    } else {
      fmt::Display::fmt(self.label, f)?;
      // Append the URL to preserve context in plain-text logs.
      write!(f, " ({})", self.url)?;
      Ok(())
    }
  }
}

/// Extension trait to mirror the ergonomics of `owo-colors`.
///
/// This exists purely for fluent syntax—any `Display` type gains a
/// `.hyperlink(url, colors)` method so callers don't need to import the free
/// function or construct `Hyperlink` directly. It does not add new behavior.
pub trait HyperlinkExt: fmt::Display + Sized {
  /// Wrap any displayable value as an OSC 8 hyperlink.
  #[inline]
  fn hyperlink<'a>(&'a self, url: &'a str, colors: ColorMode) -> Hyperlink<'a, Self> {
    Hyperlink::new(self, url, colors)
  }
}

impl<T: fmt::Display> HyperlinkExt for T {}

/// Convenience function when a trait import feels noisy.
#[inline]
pub fn hyperlink<'a, T: fmt::Display + ?Sized>(label: &'a T, url: &'a str, colors: ColorMode) -> Hyperlink<'a, T> {
  Hyperlink::new(label, url, colors)
}

/// Truncate a string to a maximum number of characters, appending "..." if
/// truncated.
///
/// This function properly handles UTF-8 strings by counting characters rather
/// than bytes, avoiding panics when the string contains multibyte characters
/// (e.g., emoji, non-ASCII characters).
///
/// # Arguments
/// * `s` - The string to truncate
/// * `max_chars` - Maximum number of characters to keep (excluding the "..."
///   suffix)
///
/// # Returns
/// The original string if it fits within `max_chars`, otherwise a truncated
/// version with "..." appended.
///
/// # Examples
/// ```
/// use twig_core::text::truncate_string;
///
/// assert_eq!(truncate_string("hello", 10), "hello");
/// assert_eq!(truncate_string("hello world", 5), "hello...");
/// assert_eq!(truncate_string("🎉🎊🎁🎄🎅", 3), "🎉🎊🎁...");
/// ```
pub fn truncate_string(s: &str, max_chars: usize) -> String {
  let char_count = s.chars().count();
  if char_count <= max_chars {
    s.to_string()
  } else {
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{truncated}...")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn hyperlink_emits_osc8_when_enabled() {
    let label = "twig";
    let rendered = format!("{}", hyperlink(&label, "https://example.com", ColorMode::Yes));
    assert!(rendered.starts_with("\x1b]8;;https://example.com\x07"));
    assert!(rendered.ends_with("\x1b]8;;\x07"));
    assert!(rendered.contains(label));
  }

  #[test]
  fn hyperlink_falls_back_when_disabled() {
    let label = "twig";
    let rendered = format!("{}", hyperlink(&label, "https://example.com", ColorMode::No));
    assert_eq!(rendered, "twig (https://example.com)");
  }

  #[test]
  fn hyperlink_respects_auto_as_enabled() {
    let rendered = format!("{}", hyperlink(&"link", "https://auto.example", ColorMode::Auto));
    assert!(rendered.starts_with("\x1b]8;;https://auto.example\x07"));
    assert!(rendered.ends_with("\x1b]8;;\x07"));
  }

  #[test]
  fn hyperlink_extension_trait_matches_free_function() {
    let via_trait = format!("{}", "label".hyperlink("https://trait.example", ColorMode::Yes));
    let via_fn = format!("{}", hyperlink(&"label", "https://trait.example", ColorMode::Yes));
    assert_eq!(via_trait, via_fn);
  }

  #[test]
  fn hyperlink_override_enabled_flag() {
    let rendered = format!(
      "{}",
      hyperlink(&"forced-off", "https://example.com", ColorMode::Yes).with_enabled(false)
    );
    assert_eq!(rendered, "forced-off (https://example.com)");
  }

  #[test]
  fn hyperlink_handles_empty_url() {
    let rendered = format!("{}", hyperlink(&"label", "", ColorMode::Yes));
    assert_eq!(rendered, "\x1b]8;;\x07label\x1b]8;;\x07");
  }

  #[test]
  fn truncate_string_returns_original_when_short() {
    assert_eq!(truncate_string("hello", 10), "hello");
    assert_eq!(truncate_string("hello", 5), "hello");
  }

  #[test]
  fn truncate_string_truncates_long_strings() {
    assert_eq!(truncate_string("hello world", 5), "hello...");
    assert_eq!(truncate_string("abcdefghij", 3), "abc...");
  }

  #[test]
  fn truncate_string_handles_multibyte_characters() {
    // Emoji (each takes 4 bytes but counts as 1 char)
    assert_eq!(truncate_string("🎉🎊🎁🎄🎅", 3), "🎉🎊🎁...");
    assert_eq!(truncate_string("🎉🎊🎁🎄🎅", 5), "🎉🎊🎁🎄🎅");
    assert_eq!(truncate_string("🎉🎊🎁🎄🎅", 10), "🎉🎊🎁🎄🎅");

    // Non-ASCII characters
    assert_eq!(truncate_string("日本語テスト", 3), "日本語...");
    assert_eq!(truncate_string("日本語テスト", 6), "日本語テスト");

    // Mixed ASCII and emoji
    assert_eq!(truncate_string("Hello 🎉 World", 7), "Hello 🎉...");
  }

  #[test]
  fn truncate_string_handles_empty_string() {
    assert_eq!(truncate_string("", 5), "");
    assert_eq!(truncate_string("", 0), "");
  }

  #[test]
  fn truncate_string_handles_zero_max() {
    assert_eq!(truncate_string("hello", 0), "...");
  }

  #[test]
  fn global_override_disables_hyperlinks() {
    // Ensure we start with hyperlinks enabled
    set_hyperlinks_override(true);

    // With override enabled, hyperlinks should work
    let enabled = format!("{}", hyperlink(&"link", "https://example.com", ColorMode::Yes));
    assert!(enabled.contains("\x1b]8;;"), "hyperlink should emit OSC8 when enabled");

    // Disable globally
    set_hyperlinks_override(false);
    let disabled = format!("{}", hyperlink(&"link", "https://example.com", ColorMode::Yes));
    assert_eq!(
      disabled, "link (https://example.com)",
      "hyperlink should fall back when globally disabled"
    );

    // Re-enable for other tests
    set_hyperlinks_override(true);
  }

  #[test]
  fn hyperlinks_disabled_reflects_override_state() {
    set_hyperlinks_override(true);
    assert!(!hyperlinks_disabled());

    set_hyperlinks_override(false);
    assert!(hyperlinks_disabled());

    // Re-enable for other tests
    set_hyperlinks_override(true);
  }
}

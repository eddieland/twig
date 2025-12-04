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
      enabled: colors != ColorMode::No,
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
}

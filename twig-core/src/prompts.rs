//! # Prompts Module
//!
//! Provides a custom dialoguer theme for consistent styling across twig's
//! interactive prompts.

use console::Style;
use dialoguer::theme::ColorfulTheme;

/// Returns a custom dialoguer theme matching twig's color palette.
///
/// Features:
/// - Cyan bold prompt text
/// - Green `❯` prefix on active item
/// - Green highlight on active item text
pub fn twig_theme() -> ColorfulTheme {
  ColorfulTheme {
    prompt_style: Style::new().cyan().bold(),
    active_item_prefix: Style::new().green().apply_to("❯ ".to_string()),
    active_item_style: Style::new().green(),
    ..ColorfulTheme::default()
  }
}

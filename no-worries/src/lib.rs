#![allow(clippy::needless_doctest_main)]

//! # No Worries - Beautiful Panic Messages
//!
//! A customizable panic handler that provides human-friendly error messages
//! with colors, emojis, and automatic crash report generation.
//!
//! ## Quick Start
//!
//! ```rust
//! use no_worries::no_worries;
//!
//! fn main() {
//!   no_worries!();
//!
//!   // Your application code here
//!   // Any panics will now show beautiful colored messages
//! }
//! ```
//!
//! ## Custom Configuration
//!
//! ```rust
//! use no_worries::{Config, Metadata, no_worries};
//!
//! fn main() {
//!   let config = Config {
//!     metadata: Metadata {
//!       name: "My Awesome App".to_string(),
//!       support_email: Some("help@myapp.com".to_string()),
//!       ..Default::default()
//!     },
//!     custom_message: Some("🚨 Oops! Something went sideways!".to_string()),
//!     ..Default::default()
//!   };
//!
//!   no_worries!(config);
//! }
//! ```

use std::env;
use std::fs::File;
use std::io::Write;
use std::panic::{self, PanicHookInfo};
use std::path::PathBuf;
use std::sync::OnceLock;

use backtrace::Backtrace;
use owo_colors::*;

/// Errors that can occur during panic handler setup or execution
#[derive(Debug)]
pub enum NoWorriesError {
  /// Report directory doesn't exist or isn't accessible
  DirectoryNotFound(PathBuf),
  /// Insufficient permissions to write crash reports
  PermissionDenied(PathBuf),
  /// I/O error during report generation
  WriteError(std::io::Error),
  /// Invalid configuration provided
  InvalidConfig(String),
}

impl std::fmt::Display for NoWorriesError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      NoWorriesError::DirectoryNotFound(path) => write!(f, "Report directory not found: {}", path.display()),
      NoWorriesError::PermissionDenied(path) => write!(f, "Permission denied for directory: {}", path.display()),
      NoWorriesError::WriteError(e) => write!(f, "Failed to write crash report: {e}",),
      NoWorriesError::InvalidConfig(msg) => write!(f, "Invalid configuration: {msg}",),
    }
  }
}

impl std::error::Error for NoWorriesError {}

pub type Result<T> = std::result::Result<T, NoWorriesError>;

/// Metadata about your application for crash reports and user messages
#[derive(Debug, Clone)]
pub struct Metadata {
  /// Application name (defaults to CARGO_PKG_NAME)
  pub name: String,
  /// Application version (defaults to CARGO_PKG_VERSION)
  pub version: String,
  /// Authors information (defaults to CARGO_PKG_AUTHORS)
  pub authors: String,
  /// Support email for users to contact
  pub support_email: Option<String>,
}

impl Default for Metadata {
  fn default() -> Self {
    Self {
      name: env!("CARGO_PKG_NAME").to_string(),
      version: env!("CARGO_PKG_VERSION").to_string(),
      authors: env!("CARGO_PKG_AUTHORS").to_string(),
      support_email: None,
    }
  }
}

/// Configuration for the panic handler behavior
#[derive(Debug, Clone)]
pub struct Config {
  /// Application metadata
  pub metadata: Metadata,
  /// Custom message to show users (defaults to humorous message)
  pub custom_message: Option<String>,
  /// Whether to include the backtrace in the terminal output (not just the
  /// report file)
  pub show_backtrace_in_message: bool,
  /// Enable colored output (automatically disabled when output is redirected)
  pub use_colors: bool,
  /// Custom report file directory (defaults to system temp directory)
  pub report_directory: Option<PathBuf>,
  /// Whether to generate crash report files at all
  pub generate_reports: bool,
  /// Maximum length for panic messages (prevents log spam)
  pub max_message_length: usize,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      metadata: Metadata::default(),
      custom_message: None,
      show_backtrace_in_message: true,
      use_colors: true,
      report_directory: None,
      generate_reports: true,
      max_message_length: 500,
    }
  }
}

impl Config {
  /// Validate the configuration and return any errors
  pub fn validate(&self) -> Result<()> {
    if let Some(dir) = &self.report_directory {
      if !dir.exists() {
        return Err(NoWorriesError::DirectoryNotFound(dir.clone()));
      }

      // Test write permissions
      let test_file = dir.join(".no_worries_test");
      if let Err(_e) = File::create(&test_file) {
        return Err(NoWorriesError::PermissionDenied(dir.clone()));
      }
      let _ = std::fs::remove_file(test_file); // Clean up test file
    }

    if self.metadata.name.is_empty() {
      return Err(NoWorriesError::InvalidConfig(
        "Application name cannot be empty".to_string(),
      ));
    }

    if self.max_message_length == 0 {
      return Err(NoWorriesError::InvalidConfig(
        "max_message_length must be greater than 0".to_string(),
      ));
    }

    Ok(())
  }
}

// Global configuration storage
static CONFIG: OnceLock<Config> = OnceLock::new();

/// Sets up the panic handler with default configuration
///
/// This is the simplest way to add beautiful panic messages to your
/// application. Only activates in release builds.
pub fn setup() -> Result<()> {
  setup_with_config(Config::default())
}

/// Sets up the panic handler with custom configuration
///
/// Allows full customization of panic messages, colors, and behavior.
/// Only activates in release builds.
pub fn setup_with_config(config: Config) -> Result<()> {
  // Only install in release mode - preserve developer experience in debug mode
  if cfg!(debug_assertions) {
    return Ok(());
  }

  // Validate configuration
  config.validate()?;

  // Clone the config for use in the closure
  let config_for_hook = config.clone();

  // Store config globally
  CONFIG
    .set(config)
    .map_err(|_| NoWorriesError::InvalidConfig("Panic handler already initialized".to_string()))?;

  panic::set_hook(Box::new(move |info: &PanicHookInfo| {
    handle_panic(info, &config_for_hook);
  }));

  Ok(())
}

/// Internal panic handler that processes the panic and displays the message
fn handle_panic(info: &PanicHookInfo, config: &Config) {
  let backtrace = Backtrace::new();

  // Generate report file if enabled
  let report_path = if config.generate_reports {
    generate_report_file(info, &backtrace, config).ok() // Don't let report generation failure crash the panic handler
  } else {
    None
  };

  display_panic_message(info, report_path.as_ref(), config);
}

/// Display the human-friendly panic message with colors and formatting
fn display_panic_message(info: &PanicHookInfo, report_path: Option<&PathBuf>, config: &Config) {
  let use_colors = config.use_colors && is_terminal::is_terminal(std::io::stderr());

  // Opening message - playful theme
  if use_colors {
    eprintln!(
      "{} {}",
      config.metadata.name.bright_blue().bold(),
      "just went off-script!".red()
    );
  } else {
    eprintln!("{} just went off-script!", config.metadata.name);
  }

  // Custom message with playful default
  if let Some(message) = &config.custom_message {
    if use_colors {
      eprintln!("\n{}", message.bright_cyan().bold());
    } else {
      eprintln!("\n{message}",);
    }
  }
  // Show location where panic occurred
  if let Some(location) = info.location() {
    if use_colors {
      eprintln!(
        "\n📍 {} {}:{}",
        "Location:".bright_white(),
        location.file().yellow(),
        location.line().to_string().yellow().bold()
      );
    } else {
      eprintln!("\n📍 Location: {}:{}", location.file(), location.line());
    }
  }

  // Show panic message if available (with length limit)
  if let Some(message) = extract_panic_message(info) {
    let truncated = if message.len() > config.max_message_length {
      format!("{}... (truncated)", &message[..config.max_message_length])
    } else {
      message
    };

    if use_colors {
      eprintln!("💬 {} {}", "Message:".bright_white(), truncated.white().italic());
    } else {
      eprintln!("💬 Message: {truncated}",);
    }
  }

  // Show backtrace in terminal if requested
  if config.show_backtrace_in_message {
    if use_colors {
      eprintln!("\n📚 {}", "Stack Trace:".bright_cyan().bold());
    } else {
      eprintln!("\n📚 Stack Trace:");
    }
    let backtrace_str = format!("{:?}", Backtrace::new());
    eprintln!("{}", backtrace_str.trim_end());
  }

  // Help section
  if use_colors {
    eprintln!("\n🆘 {}", "How to Help:".bright_green().bold().underline());
  } else {
    eprintln!("\n🆘 How to Help:");
  }

  // Report file information
  if let Some(path) = report_path {
    if use_colors {
      eprintln!(
        "   📋 {} {}",
        "Crash report saved:".bright_blue(),
        path.display().to_string().yellow().underline()
      );
    } else {
      eprintln!("   📋 Crash report saved: {}", path.display());
    }
  }

  // Contact information - prioritize the most direct contact method
  if let Some(email) = &config.metadata.support_email {
    if use_colors {
      eprintln!("   📧 {} {}", "Email:".bright_green(), email.cyan().underline());
      eprintln!(
        "   📌 {} {}",
        "Subject:".bright_white(),
        format!("\"[{}] Crash Report\"", config.metadata.name).yellow().bold()
      );
    } else {
      eprintln!("   📧 Email: {email}",);
      eprintln!("   📌 Subject: \"[{}] Crash Report\"", config.metadata.name);
    }
  }

  if report_path.is_some() {
    if use_colors {
      eprintln!(
        "   📎 {} {}",
        "Please attach the report above".bright_white(),
        "(contains debugging info)".bright_black()
      );
    } else {
      eprintln!("   📎 Please attach the report above (contains debugging info)");
    }
  }

  // Brief footer
  if use_colors {
    eprintln!(
      "\n{} {}",
      "Thanks for helping us improve!".bright_blue().italic(),
      "💙".bright_blue()
    );
  } else {
    eprintln!("\nThanks for helping us improve! 💙");
  }
}

/// Extract the panic message from PanicHookInfo
fn extract_panic_message(info: &PanicHookInfo) -> Option<String> {
  info
    .payload()
    .downcast_ref::<&str>()
    .map(|s| s.to_string())
    .or_else(|| info.payload().downcast_ref::<String>().cloned())
}

/// Generate a crash report file with detailed information
fn generate_report_file(info: &PanicHookInfo, backtrace: &Backtrace, config: &Config) -> Result<PathBuf> {
  let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
  let filename = format!(
    "crash_report_{}_{}.toml",
    config
      .metadata
      .name
      .replace(|c: char| !c.is_alphanumeric(), "_")
      .to_lowercase(),
    timestamp
  );

  let report_dir = config.report_directory.clone().unwrap_or_else(env::temp_dir);

  let report_path = report_dir.join(filename);

  let mut file = File::create(&report_path).map_err(NoWorriesError::WriteError)?;

  write_crash_report(&mut file, info, backtrace, config).map_err(NoWorriesError::WriteError)?;

  Ok(report_path)
}

/// Write the crash report content in TOML format
fn write_crash_report(
  file: &mut File,
  info: &PanicHookInfo,
  backtrace: &Backtrace,
  config: &Config,
) -> std::io::Result<()> {
  writeln!(file, "# {} Crash Report", config.metadata.name)?;
  writeln!(file, "# Generated automatically - safe to share for debugging\n")?;

  writeln!(file, "[application]")?;
  writeln!(file, "name = '{}'", config.metadata.name.replace('\'', "\\'"))?;
  writeln!(file, "version = '{}'", config.metadata.version.replace('\'', "\\'"))?;
  writeln!(file, "authors = '{}'", config.metadata.authors.replace('\'', "\\'"))?;

  writeln!(file, "\n[system]")?;
  writeln!(file, "operating_system = '{}'", env::consts::OS)?;
  writeln!(file, "architecture = '{}'", env::consts::ARCH)?;
  writeln!(file, "timestamp = '{}'", chrono::Utc::now().to_rfc3339())?;

  writeln!(file, "\n[panic]")?;
  if let Some(location) = info.location() {
    writeln!(file, "file = '{}'", location.file().replace('\'', "\\'"))?;
    writeln!(file, "line = {}", location.line())?;
    writeln!(file, "column = {}", location.column())?;
  }

  if let Some(message) = extract_panic_message(info) {
    let escaped = message.replace("'''", "'\"'\"'");
    writeln!(file, "message = '''{escaped}'''",)?;
  }

  writeln!(file, "\n[backtrace]")?;
  writeln!(file, "trace = '''")?;
  let backtrace_str = format!("{backtrace:?}");
  writeln!(file, "{}", backtrace_str.trim_end())?;
  writeln!(file, "'''")?;

  Ok(())
}

/// Convenient macro for setting up the panic handler
///
/// # Examples
///
/// Basic usage:
/// ```rust
/// use no_worries::no_worries;
///
/// fn main() {
///   no_worries!().expect("Failed to setup panic handler");
///   // Your app code
/// }
/// ```
///
/// With custom configuration:
/// ```rust
/// use no_worries::{Config, no_worries};
///
/// fn main() {
///   let config = Config {
///     custom_message: Some("🎪 The show must go on... after we fix this!".to_string()),
///     ..Default::default()
///   };
///   no_worries!(config).expect("Failed to setup panic handler");
/// }
/// ```
#[macro_export]
macro_rules! no_worries {
  () => {
    $crate::setup()
  };
  ($config:expr) => {
    $crate::setup_with_config($config)
  };
}

#[cfg(test)]
mod tests {
  use std::fs;

  use super::*;

  #[test]
  fn test_default_config() {
    let config = Config::default();
    assert!(config.use_colors);
    assert!(config.show_backtrace_in_message);
    assert!(config.generate_reports);
    assert_eq!(config.max_message_length, 500);
  }

  #[test]
  fn test_config_validation() {
    let mut config = Config::default();
    assert!(config.validate().is_ok());

    // Test invalid directory
    config.report_directory = Some(PathBuf::from("/nonexistent/directory/path"));
    assert!(config.validate().is_err());

    // Test empty name
    config.report_directory = None;
    config.metadata.name = String::new();
    assert!(config.validate().is_err());

    // Test zero max length
    config.metadata.name = "Test".to_string();
    config.max_message_length = 0;
    assert!(config.validate().is_err());
  }

  #[test]
  fn test_report_generation() {
    let temp_dir = env::temp_dir().join("no_worries_test");
    let _ = fs::create_dir_all(&temp_dir);

    let config = Config {
      report_directory: Some(temp_dir.clone()),
      ..Default::default()
    };

    assert!(config.validate().is_ok());

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
  }
}

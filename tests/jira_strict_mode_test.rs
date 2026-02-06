#![cfg(unix)]

//! End-to-end regression tests for Jira strict parsing mode.
//!
//! These tests exercise the full config-persistence → parser-creation pipeline,
//! paralleling the ratchet-with-modify pattern: configure a mode, perform
//! operations, then tighten (or loosen) the mode and verify behaviour changes.

use anyhow::Result;
use twig_core::{ConfigDirs, JiraParsingConfig, JiraParsingMode, create_jira_parser};
use twig_test_utils::EnvTestGuard;

/// Set up an isolated XDG environment and initialise real `ConfigDirs`.
///
/// The returned `EnvTestGuard` keeps the overridden env vars alive; dropping it
/// restores the original values.
fn setup_config_env() -> Result<(EnvTestGuard, ConfigDirs)> {
  let env_guard = EnvTestGuard::new();
  let config_dirs = ConfigDirs::new()?;
  config_dirs.init()?;
  Ok((env_guard, config_dirs))
}

/// Save a Jira config with the given mode through the real persistence layer.
fn save_mode(config_dirs: &ConfigDirs, mode: JiraParsingMode) -> Result<()> {
  config_dirs.save_jira_config(&JiraParsingConfig { mode })
}

// ---------------------------------------------------------------------------
// 1. Default (no jira.toml) → flexible
// ---------------------------------------------------------------------------

#[test]
fn test_default_config_is_flexible() -> Result<()> {
  let (_env, config_dirs) = setup_config_env()?;

  // No jira.toml has been written yet.
  let config = config_dirs.load_jira_config()?;
  assert_eq!(config.mode, JiraParsingMode::Flexible);

  // create_jira_parser goes through get_config_dirs → load_jira_config.
  let parser = create_jira_parser().expect("parser should be created with defaults");
  // Flexible mode accepts lowercase.
  assert!(parser.is_valid("me-1234"));
  assert!(parser.is_valid("me1234"));

  Ok(())
}

// ---------------------------------------------------------------------------
// 2. Strict config round-trip through save / load
// ---------------------------------------------------------------------------

#[test]
fn test_strict_config_round_trip() -> Result<()> {
  let (_env, config_dirs) = setup_config_env()?;

  save_mode(&config_dirs, JiraParsingMode::Strict)?;

  let loaded = config_dirs.load_jira_config()?;
  assert_eq!(loaded.mode, JiraParsingMode::Strict);

  Ok(())
}

// ---------------------------------------------------------------------------
// 3. create_jira_parser() honours persisted strict config
// ---------------------------------------------------------------------------

#[test]
fn test_create_jira_parser_respects_strict_config() -> Result<()> {
  let (_env, config_dirs) = setup_config_env()?;
  save_mode(&config_dirs, JiraParsingMode::Strict)?;

  let parser = create_jira_parser().expect("parser should be created from strict config");

  // Strict accepts canonical format only.
  assert!(parser.is_valid("ME-1234"));
  assert!(parser.is_valid("PROJECT-999"));

  // Strict rejects non-canonical formats.
  assert!(!parser.is_valid("me-1234"));
  assert!(!parser.is_valid("ME1234"));
  assert!(!parser.is_valid("me1234"));

  Ok(())
}

// ---------------------------------------------------------------------------
// 4. Ratchet: flexible → strict (tighten)
// ---------------------------------------------------------------------------

#[test]
fn test_ratchet_flexible_to_strict() -> Result<()> {
  let (_env, config_dirs) = setup_config_env()?;

  // --- Phase 1: flexible (default) ---
  let parser_flex = create_jira_parser().expect("default flexible parser");
  assert!(parser_flex.is_valid("me-1234"), "flexible accepts lowercase");
  assert!(parser_flex.is_valid("ME1234"), "flexible accepts no-hyphen");
  assert!(parser_flex.parse("me1234").is_ok(), "flexible normalises me1234");
  assert_eq!(parser_flex.parse("me1234")?, "ME-1234");

  // --- Phase 2: ratchet to strict ---
  save_mode(&config_dirs, JiraParsingMode::Strict)?;

  let parser_strict = create_jira_parser().expect("strict parser after ratchet");
  assert!(
    !parser_strict.is_valid("me-1234"),
    "strict rejects lowercase after ratchet"
  );
  assert!(
    !parser_strict.is_valid("ME1234"),
    "strict rejects no-hyphen after ratchet"
  );
  assert!(parser_strict.is_valid("ME-1234"), "strict still accepts canonical");

  Ok(())
}

// ---------------------------------------------------------------------------
// 5. Ratchet: strict → flexible (loosen)
// ---------------------------------------------------------------------------

#[test]
fn test_ratchet_strict_to_flexible() -> Result<()> {
  let (_env, config_dirs) = setup_config_env()?;

  // --- Phase 1: strict ---
  save_mode(&config_dirs, JiraParsingMode::Strict)?;
  let parser_strict = create_jira_parser().expect("strict parser");
  assert!(!parser_strict.is_valid("me-1234"));

  // --- Phase 2: loosen to flexible ---
  save_mode(&config_dirs, JiraParsingMode::Flexible)?;
  let parser_flex = create_jira_parser().expect("flexible parser after loosen");
  assert!(
    parser_flex.is_valid("me-1234"),
    "flexible accepts lowercase after loosen"
  );
  assert_eq!(parser_flex.parse("me1234")?, "ME-1234");

  Ok(())
}

// ---------------------------------------------------------------------------
// 6. Strict commit-message extraction via config pipeline
// ---------------------------------------------------------------------------

#[test]
fn test_strict_commit_message_extraction_via_config() -> Result<()> {
  let (_env, config_dirs) = setup_config_env()?;
  save_mode(&config_dirs, JiraParsingMode::Strict)?;

  let parser = create_jira_parser().expect("strict parser");

  // Canonical prefix → extracted.
  assert_eq!(
    parser.extract_from_commit_message("ME-1234: Fix bug in parser"),
    Some("ME-1234".to_string())
  );

  // Lowercase prefix → rejected by strict.
  assert_eq!(parser.extract_from_commit_message("me-1234: Fix bug in parser"), None,);

  // No-hyphen prefix → rejected by strict.
  assert_eq!(parser.extract_from_commit_message("ME1234: Fix bug in parser"), None,);

  Ok(())
}

// ---------------------------------------------------------------------------
// 7. Strict rejects all non-canonical format variants
// ---------------------------------------------------------------------------

#[test]
fn test_strict_rejects_all_non_canonical_formats() -> Result<()> {
  let (_env, config_dirs) = setup_config_env()?;
  save_mode(&config_dirs, JiraParsingMode::Strict)?;

  let parser = create_jira_parser().expect("strict parser");

  let invalid_inputs = [
    "me-1234", // lowercase
    "Me-1234", // mixed case
    "mE-1234", // mixed case
    "ME1234",  // missing hyphen
    "me1234",  // lowercase + missing hyphen
    "M-123",   // project code too short
    "ME-",     // missing number
    "-1234",   // missing project code
    "123-ME",  // reversed
    "",        // empty
    "PROJ",    // no number at all
    "12345",   // only digits
  ];

  for input in &invalid_inputs {
    assert!(parser.parse(input).is_err(), "strict should reject '{input}'");
  }

  // Canonical inputs that strict *does* accept.
  let valid_inputs = ["ME-1234", "AB-1", "PROJECT-999", "VERYLONGPROJECT-42"];
  for input in &valid_inputs {
    assert!(parser.parse(input).is_ok(), "strict should accept '{input}'");
  }

  Ok(())
}

// ---------------------------------------------------------------------------
// 8. Repeated save/load cycles preserve the mode
// ---------------------------------------------------------------------------

#[test]
fn test_config_persistence_across_multiple_saves() -> Result<()> {
  let (_env, config_dirs) = setup_config_env()?;

  let modes = [
    JiraParsingMode::Strict,
    JiraParsingMode::Flexible,
    JiraParsingMode::Strict,
    JiraParsingMode::Flexible,
  ];

  for mode in &modes {
    save_mode(&config_dirs, mode.clone())?;
    let loaded = config_dirs.load_jira_config()?;
    assert_eq!(&loaded.mode, mode, "mode should survive save/load cycle");
  }

  Ok(())
}

// ---------------------------------------------------------------------------
// 9. Flexible commit-message extraction (contrast with strict)
// ---------------------------------------------------------------------------

#[test]
fn test_flexible_commit_message_extraction_via_config() -> Result<()> {
  let (_env, config_dirs) = setup_config_env()?;
  // Explicitly set flexible to be sure.
  save_mode(&config_dirs, JiraParsingMode::Flexible)?;

  let parser = create_jira_parser().expect("flexible parser");

  // Flexible extracts and normalises all supported prefixes.
  assert_eq!(
    parser.extract_from_commit_message("ME-1234: Fix bug"),
    Some("ME-1234".to_string())
  );
  assert_eq!(
    parser.extract_from_commit_message("me-1234: Fix bug"),
    Some("ME-1234".to_string())
  );
  assert_eq!(
    parser.extract_from_commit_message("ME1234: Fix bug"),
    Some("ME-1234".to_string())
  );

  Ok(())
}

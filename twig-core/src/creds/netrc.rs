use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::creds::platform::FilePermissions;
use crate::creds::{Credentials, platform};

/// Get the path to the .netrc file
pub fn get_netrc_path(home: &Path) -> PathBuf {
  home.join(".netrc")
}

/// Parse a .netrc file for credentials for a specific machine.
///
/// The parser follows the conventions used by curl and other common netrc
/// consumers:
///
/// * `#` introduces a comment that runs to end-of-line. A `#` is only treated as the start of a comment when it appears
///   at a token boundary (start of line or directly after whitespace), so passwords containing `#` are not silently
///   truncated.
/// * `macdef <name>` introduces a macro definition whose body extends until the next blank line. Macro bodies are
///   skipped wholesale to prevent stray words like `machine` or `login` inside a macro from being interpreted as
///   credential tokens.
/// * `default` starts a fallback entry. It is recognized as a section boundary so its `login`/`password` cannot
///   accidentally attach to the preceding `machine` block, and its credentials are returned when no explicit match for
///   `target_machine` is found.
/// * `passwd` is accepted as a synonym for `password`, and `account` values are skipped.
pub fn parse_netrc_file(path: &Path, target_machine: &str) -> Result<Option<Credentials>> {
  let content = std::fs::read_to_string(path).context("Failed to read .netrc file")?;
  let tokens = tokenize_netrc(&content);

  let mut iter = tokens.into_iter().peekable();
  let mut default_creds: Option<Credentials> = None;
  let mut target_seen = false;

  while let Some(token) = iter.next() {
    match token.as_str() {
      "machine" => {
        let Some(name) = iter.next() else { break };
        let entry = parse_entry(&mut iter);
        if name == target_machine {
          target_seen = true;
          if let (Some(username), Some(password)) = (entry.login, entry.password) {
            return Ok(Some(Credentials { username, password }));
          }
        }
      }
      "default" => {
        let entry = parse_entry(&mut iter);
        if default_creds.is_none()
          && let (Some(username), Some(password)) = (entry.login, entry.password)
        {
          default_creds = Some(Credentials { username, password });
        }
      }
      _ => {
        // Unknown leading token (e.g. stray keyword without a `machine`
        // context). Skip it so it doesn't drag a following token along.
      }
    }
  }

  // Only fall through to `default` if the target machine was never named
  // explicitly. An incomplete explicit entry is the user's intent, not a
  // signal to use the fallback.
  if target_seen { Ok(None) } else { Ok(default_creds) }
}

#[derive(Default)]
struct Entry {
  login: Option<String>,
  password: Option<String>,
}

fn parse_entry<I: Iterator<Item = String>>(iter: &mut std::iter::Peekable<I>) -> Entry {
  let mut entry = Entry::default();
  while let Some(token) = iter.peek() {
    match token.as_str() {
      "machine" | "default" => break,
      "login" => {
        iter.next();
        entry.login = iter.next();
      }
      "password" | "passwd" => {
        iter.next();
        entry.password = iter.next();
      }
      "account" => {
        iter.next();
        iter.next();
      }
      _ => {
        iter.next();
      }
    }
  }
  entry
}

/// Split a netrc file into a flat token stream, dropping comments and macro
/// bodies.
fn tokenize_netrc(content: &str) -> Vec<String> {
  let mut tokens = Vec::new();
  let mut lines = content.lines();

  while let Some(line) = lines.next() {
    let stripped = strip_inline_comment(line);
    let line_tokens: Vec<&str> = stripped.split_whitespace().collect();
    if line_tokens.is_empty() {
      continue;
    }

    // `macdef <name>` starts a macro whose body runs until a blank line.
    // Skip the body so its contents cannot masquerade as netrc keywords.
    if line_tokens.contains(&"macdef") {
      for body_line in lines.by_ref() {
        if body_line.trim().is_empty() {
          break;
        }
      }
      continue;
    }

    tokens.extend(line_tokens.into_iter().map(String::from));
  }

  tokens
}

/// Return the prefix of `line` up to a `#` that starts a comment.
///
/// `#` only begins a comment when preceded by whitespace (or at the start of
/// the line), so values containing `#` are preserved.
fn strip_inline_comment(line: &str) -> &str {
  let mut prev_was_ws = true;
  for (i, c) in line.char_indices() {
    if c == '#' && prev_was_ws {
      return &line[..i];
    }
    prev_was_ws = c.is_whitespace();
  }
  line
}

/// Write or update a .netrc entry for a specific machine
pub fn write_netrc_entry(path: &Path, machine: &str, username: &str, password: &str) -> Result<()> {
  // Read existing content if file exists
  let mut existing_content = String::new();
  let mut machine_exists = false;

  if path.exists() {
    existing_content = std::fs::read_to_string(path).context("Failed to read existing .netrc file")?;

    // Check if machine already exists
    machine_exists = existing_content.contains(&format!("machine {machine}"));
  }

  if machine_exists {
    // Update existing entry
    let lines: Vec<&str> = existing_content.lines().collect();
    let mut new_content = String::new();
    let mut skip_until_next_machine = false;

    for line in lines {
      let trimmed = line.trim();

      if trimmed.starts_with("machine ") {
        if trimmed == format!("machine {machine}",) {
          skip_until_next_machine = true;
          // Add the updated machine entry
          new_content.push_str(&format!("machine {machine}\n",));
          new_content.push_str(&format!("  login {username}\n",));
          new_content.push_str(&format!("  password {password}\n",));
        } else {
          skip_until_next_machine = false;
          new_content.push_str(line);
          new_content.push('\n');
        }
      } else if !skip_until_next_machine {
        new_content.push_str(line);
        new_content.push('\n');
      }
    }

    std::fs::write(path, new_content).context("Failed to write updated .netrc file")?;
  } else {
    // Append new entry
    let mut file = std::fs::OpenOptions::new()
      .create(true)
      .append(true)
      .open(path)
      .context("Failed to open .netrc file for writing")?;

    // Add a newline if file exists and doesn't end with one
    if path.metadata()?.len() > 0 && !existing_content.ends_with('\n') {
      writeln!(file)?;
    }

    writeln!(file, "machine {machine}",)?;
    writeln!(file, "  login {username}",)?;
    writeln!(file, "  password {password}",)?;
  }

  // Set secure permissions on the file
  #[cfg(unix)]
  {
    platform::UnixFilePermissions::set_secure_permissions(path)?;
  }

  #[cfg(windows)]
  {
    // note: this is a no-op on Windows, but we call it for consistency
    platform::WindowsFilePermissions::set_secure_permissions(path)?;
  }

  Ok(())
}

/// Normalizes a Jira host URL by removing protocol prefixes and trailing
/// slashes.
///
/// # Arguments
///
/// * `raw_host` - A string slice containing the raw host URL that may include protocol prefixes (http:// or https://)
///   and/or trailing slashes
///
/// # Returns
///
/// A `String` containing the normalized hostname without protocol or trailing
/// slash
///
/// # Examples
///
/// ```
/// let host1 = normalize_host("https://company.atlassian.net/");
/// assert_eq!(host1, "company.atlassian.net");
///
/// let host2 = normalize_host("http://jira.example.com");
/// assert_eq!(host2, "jira.example.com");
///
/// let host3 = normalize_host("my-jira-instance.com");
/// assert_eq!(host3, "my-jira-instance.com");
/// ```
pub fn normalize_host(raw_host: &str) -> String {
  raw_host
    .trim_start_matches("https://")
    .trim_start_matches("http://")
    .trim_end_matches('/')
    .to_string()
}

#[cfg(test)]
mod tests {
  use std::fs;
  use std::io::Write;

  use tempfile::TempDir;
  use twig_test_utils::NetrcGuard;

  use super::*;

  #[test]
  fn test_parse_netrc_file_basic() {
    let content = r#"machine example.com
  login testuser
  password testpass
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_some());

    let creds = result.unwrap();
    assert_eq!(creds.username, "testuser");
    assert_eq!(creds.password, "testpass");
  }

  #[test]
  fn test_parse_netrc_file_multiple_machines() {
    let content = r#"machine example.com
  login user1
  password pass1

machine github.com
  login user2
  password pass2

machine atlassian.com
  login user3
  password pass3
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    // Test first machine
    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user1");
    assert_eq!(creds.password, "pass1");

    // Test middle machine
    let result = parse_netrc_file(&netrc_path, "github.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user2");
    assert_eq!(creds.password, "pass2");

    // Test last machine
    let result = parse_netrc_file(&netrc_path, "atlassian.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user3");
    assert_eq!(creds.password, "pass3");
  }

  #[test]
  fn test_parse_netrc_file_machine_not_found() {
    let content = r#"machine example.com
  login testuser
  password testpass
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    let result = parse_netrc_file(&netrc_path, "nonexistent.com").unwrap();
    assert!(result.is_none());
  }

  #[test]
  fn test_parse_netrc_file_incomplete_entry() {
    let content = r#"machine example.com
  login testuser
machine github.com
  login user2
  password pass2
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    // Should not find example.com because it has no password
    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_none());

    // Should find github.com because it has both login and password
    let result = parse_netrc_file(&netrc_path, "github.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user2");
    assert_eq!(creds.password, "pass2");
  }

  #[test]
  fn test_parse_netrc_file_single_line_format() {
    let content = "machine example.com login testuser password testpass\n";

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_some());

    let creds = result.unwrap();
    assert_eq!(creds.username, "testuser");
    assert_eq!(creds.password, "testpass");
  }

  #[test]
  fn test_parse_netrc_file_mixed_format() {
    let content = r#"machine example.com login user1 password pass1
machine github.com
  login user2
  password pass2
machine atlassian.com login user3
  password pass3
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    // Test single line format
    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user1");
    assert_eq!(creds.password, "pass1");

    // Test multi-line format
    let result = parse_netrc_file(&netrc_path, "github.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user2");
    assert_eq!(creds.password, "pass2");

    // Test mixed format
    let result = parse_netrc_file(&netrc_path, "atlassian.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user3");
    assert_eq!(creds.password, "pass3");
  }

  #[test]
  fn test_parse_netrc_file_empty_file() {
    let (_temp_dir, netrc_path) = create_test_netrc("");

    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_none());
  }

  #[test]
  fn test_write_netrc_entry_new_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let netrc_path = temp_dir.path().join(".netrc");

    // Test writing to a new file
    write_netrc_entry(&netrc_path, "example.com", "testuser", "testpass").unwrap();

    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_some());

    let creds = result.unwrap();
    assert_eq!(creds.username, "testuser");
    assert_eq!(creds.password, "testpass");
  }

  #[test]
  fn test_write_netrc_entry_append_to_existing() {
    let initial_content = r#"machine example.com
  login user1
  password pass1
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(initial_content);

    // Append a new entry
    write_netrc_entry(&netrc_path, "github.com", "user2", "pass2").unwrap();

    // Check original entry still exists
    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user1");
    assert_eq!(creds.password, "pass1");

    // Check new entry was added
    let result = parse_netrc_file(&netrc_path, "github.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user2");
    assert_eq!(creds.password, "pass2");
  }

  #[test]
  fn test_write_netrc_entry_update_existing() {
    let initial_content = r#"machine example.com
  login olduser
  password oldpass

machine github.com
  login user2
  password pass2
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(initial_content);

    // Update existing entry
    write_netrc_entry(&netrc_path, "example.com", "newuser", "newpass").unwrap();

    // Check updated entry
    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "newuser");
    assert_eq!(creds.password, "newpass");

    // Check other entry wasn't affected
    let result = parse_netrc_file(&netrc_path, "github.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user2");
    assert_eq!(creds.password, "pass2");
  }

  #[test]
  #[cfg(unix)]
  fn test_netrc_permission_checking() {
    use std::os::unix::fs::PermissionsExt;

    use twig_test_utils::NetrcGuard;

    let content = r#"machine example.com
  login testuser
  password testpass
"#;

    let guard = NetrcGuard::new(content);
    let netrc_path = guard.netrc_path().to_path_buf();

    // Set insecure permissions (readable by group/others)
    let mut perms = fs::metadata(&netrc_path).unwrap().permissions();
    perms.set_mode(0o644); // Insecure: readable by group and others
    fs::set_permissions(&netrc_path, perms).unwrap();

    // Check permissions
    let metadata = fs::metadata(&netrc_path).unwrap();
    let permissions = metadata.permissions();
    let mode = permissions.mode();

    // Should detect insecure permissions
    assert_ne!(mode & 0o077, 0, "Expected insecure permissions to be detected");

    // Fix permissions
    let mut secure_perms = permissions;
    secure_perms.set_mode(0o600);
    fs::set_permissions(&netrc_path, secure_perms).unwrap();

    // Verify secure permissions
    let metadata = fs::metadata(&netrc_path).unwrap();
    let permissions = metadata.permissions();
    let mode = permissions.mode();

    assert_eq!(mode & 0o077, 0, "Expected secure permissions after fix");
  }

  #[test]
  fn test_credential_validation_scenarios() {
    // Test empty username/password
    let empty_creds = Credentials {
      username: "".to_string(),
      password: "".to_string(),
    };
    assert!(empty_creds.username.is_empty());
    assert!(empty_creds.password.is_empty());

    // Test valid credentials structure
    let valid_creds = Credentials {
      username: "testuser".to_string(),
      password: "testpass".to_string(),
    };
    assert!(!valid_creds.username.is_empty());
    assert!(!valid_creds.password.is_empty());
    assert_eq!(valid_creds.username, "testuser");
    assert_eq!(valid_creds.password, "testpass");
  }

  #[test]
  fn test_parse_netrc_file_malformed() {
    let content = r#"machine custom-jira-host.com
  login custom@example.com
  # missing password

machine atlassian.com
  login test@example.com
  # missing password

machine github.com
  login testuser
  password gh-token
  some-invalid-line
"#;
    let guard = NetrcGuard::new(content);
    let netrc_path = guard.netrc_path().to_path_buf();

    // Test parsing should handle malformed entries gracefully
    let result = parse_netrc_file(&netrc_path, "custom-jira-host.com").unwrap();
    assert!(result.is_none()); // Should be None because password is missing

    let result = parse_netrc_file(&netrc_path, "atlassian.com").unwrap();
    assert!(result.is_none()); // Should be None because password is missing

    let result = parse_netrc_file(&netrc_path, "github.com").unwrap();
    assert!(result.is_some()); // Should still work despite extra line
    let creds = result.unwrap();
    assert_eq!(creds.username, "testuser");
    assert_eq!(creds.password, "gh-token");
  }

  #[test]
  fn test_parse_netrc_file_ignores_commented_out_machine() {
    // A template-style .netrc where a sample block is commented out should not
    // be parsed as a real entry.
    let content = r#"# machine example.com
#   login bad-user
#   password bad-pass

machine github.com
  login realuser
  password realpass
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    // The commented-out block must not produce credentials.
    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_none());

    // The real entry that follows must still parse correctly.
    let result = parse_netrc_file(&netrc_path, "github.com").unwrap();
    let creds = result.expect("github.com entry should parse");
    assert_eq!(creds.username, "realuser");
    assert_eq!(creds.password, "realpass");
  }

  #[test]
  fn test_parse_netrc_file_inline_comment_after_value() {
    let content = r#"machine github.com
  login realuser   # personal token
  password realpass # rotate quarterly
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    let creds = parse_netrc_file(&netrc_path, "github.com")
      .unwrap()
      .expect("entry should parse with inline comments");
    assert_eq!(creds.username, "realuser");
    assert_eq!(creds.password, "realpass");
  }

  #[test]
  fn test_parse_netrc_file_inline_comment_before_keyword() {
    // A comment introducing the next line shouldn't swallow the keyword that
    // follows on its own line.
    let content = r#"machine github.com  # source: 1Password
  login realuser
  password realpass
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    let creds = parse_netrc_file(&netrc_path, "github.com")
      .unwrap()
      .expect("entry should parse");
    assert_eq!(creds.username, "realuser");
    assert_eq!(creds.password, "realpass");
  }

  #[test]
  fn test_parse_netrc_file_hash_in_password_preserved() {
    // `#` mid-token (no preceding whitespace) is a literal character, not a
    // comment marker — passwords containing `#` must round-trip.
    let content = "machine github.com login realuser password p#ss#word\n";

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    let creds = parse_netrc_file(&netrc_path, "github.com")
      .unwrap()
      .expect("entry should parse");
    assert_eq!(creds.username, "realuser");
    assert_eq!(creds.password, "p#ss#word");
  }

  #[test]
  fn test_parse_netrc_file_skips_macdef_body() {
    // A macdef body that happens to contain netrc-looking keywords must not
    // be treated as credentials.
    let content = r#"machine first.example.com
  login alice
  password alice-pw

macdef init
  machine sneaky.example.com
  login evil
  password evil-pw

machine second.example.com
  login bob
  password bob-pw
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    // The macdef body must not produce a usable entry.
    let result = parse_netrc_file(&netrc_path, "sneaky.example.com").unwrap();
    assert!(result.is_none());

    // Entries on either side of the macdef must still parse.
    let creds = parse_netrc_file(&netrc_path, "first.example.com")
      .unwrap()
      .expect("first entry should parse");
    assert_eq!(creds.username, "alice");
    assert_eq!(creds.password, "alice-pw");

    let creds = parse_netrc_file(&netrc_path, "second.example.com")
      .unwrap()
      .expect("second entry should parse");
    assert_eq!(creds.username, "bob");
    assert_eq!(creds.password, "bob-pw");
  }

  #[test]
  fn test_parse_netrc_file_default_block_isolated_from_machine() {
    // A `default` block following a `machine` must not bleed its login/password
    // back into the preceding machine entry.
    let content = r#"machine github.com
  login realuser

default
  login fallback
  password fallback-pw
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    // github.com has no password of its own, so it must not return the
    // default block's password.
    let result = parse_netrc_file(&netrc_path, "github.com").unwrap();
    assert!(result.is_none());

    // Unknown machines fall through to the `default` block.
    let creds = parse_netrc_file(&netrc_path, "unknown.example.com")
      .unwrap()
      .expect("default entry should be returned for unknown machine");
    assert_eq!(creds.username, "fallback");
    assert_eq!(creds.password, "fallback-pw");
  }

  #[test]
  fn test_parse_netrc_file_passwd_alias() {
    // `passwd` is accepted as a synonym for `password`.
    let content = r#"machine github.com
  login realuser
  passwd realpass
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    let creds = parse_netrc_file(&netrc_path, "github.com")
      .unwrap()
      .expect("entry should parse with passwd alias");
    assert_eq!(creds.username, "realuser");
    assert_eq!(creds.password, "realpass");
  }

  #[test]
  fn test_parse_netrc_file_account_ignored() {
    // `account` tokens consume their value and don't leak into login/password.
    let content = r#"machine github.com
  login realuser
  account org-account
  password realpass
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    let creds = parse_netrc_file(&netrc_path, "github.com")
      .unwrap()
      .expect("entry should parse with account field");
    assert_eq!(creds.username, "realuser");
    assert_eq!(creds.password, "realpass");
  }

  #[test]
  fn test_normalize_host_removes_https_and_trailing_slash() {
    let result = normalize_host("https://api.example.com/");
    assert_eq!(result, "api.example.com");
  }

  #[test]
  fn test_normalize_host_removes_http_and_trailing_slash() {
    let result = normalize_host("http://localhost:8080/");
    assert_eq!(result, "localhost:8080");
  }

  /// Helper function to create a test .netrc file
  fn create_test_netrc(content: &str) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let netrc_path = temp_dir.path().join(".netrc");

    let mut file = fs::File::create(&netrc_path).expect("Failed to create test .netrc");
    file.write_all(content.as_bytes()).expect("Failed to write test .netrc");

    (temp_dir, netrc_path)
  }
}

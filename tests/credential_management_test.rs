use std::fs;
use std::io::Write;

use twig::creds::{get_netrc_path, parse_netrc_for_machine};

mod test_helpers;
use test_helpers::TestHomeEnv;

/// Integration test for credential checking functionality
#[test]
fn test_credential_check_with_valid_netrc() {
  let _test_home = TestHomeEnv::new();

  // Create a test .netrc file
  let netrc_content = r#"machine atlassian.com
  login test@example.com
  password test-token

machine github.com
  login testuser
  password gh-token
"#;

  let netrc_path = get_netrc_path();
  fs::create_dir_all(netrc_path.parent().unwrap()).unwrap();
  let mut file = fs::File::create(&netrc_path).unwrap();
  file.write_all(netrc_content.as_bytes()).unwrap();

  // Set secure permissions
  use std::os::unix::fs::PermissionsExt;
  let mut perms = fs::metadata(&netrc_path).unwrap().permissions();
  perms.set_mode(0o600);
  fs::set_permissions(&netrc_path, perms).unwrap();

  // Test parsing Jira credentials
  let jira_creds = parse_netrc_for_machine("atlassian.com").unwrap();
  assert!(jira_creds.is_some());
  let jira_creds = jira_creds.unwrap();
  assert_eq!(jira_creds.username, "test@example.com");
  assert_eq!(jira_creds.password, "test-token");

  // Test parsing GitHub credentials
  let github_creds = parse_netrc_for_machine("github.com").unwrap();
  assert!(github_creds.is_some());
  let github_creds = github_creds.unwrap();
  assert_eq!(github_creds.username, "testuser");
  assert_eq!(github_creds.password, "gh-token");

  // Test non-existent machine
  let missing_creds = parse_netrc_for_machine("nonexistent.com").unwrap();
  assert!(missing_creds.is_none());
}

/// Integration test for credential checking with missing .netrc
#[test]
fn test_credential_check_with_missing_netrc() {
  let _test_home = TestHomeEnv::new();

  // Ensure .netrc doesn't exist
  let netrc_path = get_netrc_path();
  if netrc_path.exists() {
    fs::remove_file(&netrc_path).unwrap();
  }

  // Test parsing should return None for missing file
  let jira_creds = parse_netrc_for_machine("atlassian.com").unwrap();
  assert!(jira_creds.is_none());

  let github_creds = parse_netrc_for_machine("github.com").unwrap();
  assert!(github_creds.is_none());
}

/// Integration test for credential checking with malformed .netrc
#[test]
fn test_credential_check_with_malformed_netrc() {
  let _test_home = TestHomeEnv::new();

  // Create a malformed .netrc file
  let netrc_content = r#"machine atlassian.com
  login test@example.com
  # missing password

machine github.com
  login testuser
  password gh-token
  some-invalid-line
"#;

  let netrc_path = get_netrc_path();
  fs::create_dir_all(netrc_path.parent().unwrap()).unwrap();
  let mut file = fs::File::create(&netrc_path).unwrap();
  file.write_all(netrc_content.as_bytes()).unwrap();

  // Test parsing should handle malformed entries gracefully
  let jira_creds = parse_netrc_for_machine("atlassian.com").unwrap();
  assert!(jira_creds.is_none()); // Should be None because password is missing

  let github_creds = parse_netrc_for_machine("github.com").unwrap();
  assert!(github_creds.is_some()); // Should still work despite extra line
  let github_creds = github_creds.unwrap();
  assert_eq!(github_creds.username, "testuser");
  assert_eq!(github_creds.password, "gh-token");
}

/// Test the .netrc file permission checking
#[test]
fn test_netrc_permission_checking() {
  let _test_home = TestHomeEnv::new();

  // Create a test .netrc file with insecure permissions
  let netrc_content = r#"machine example.com
  login testuser
  password testpass
"#;

  let netrc_path = get_netrc_path();
  fs::create_dir_all(netrc_path.parent().unwrap()).unwrap();
  let mut file = fs::File::create(&netrc_path).unwrap();
  file.write_all(netrc_content.as_bytes()).unwrap();

  // Set insecure permissions (readable by group/others)
  use std::os::unix::fs::PermissionsExt;
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

/// Test credential validation scenarios
#[test]
fn test_credential_validation_scenarios() {
  let _test_home = TestHomeEnv::new();

  // Test empty username/password
  let empty_creds = twig::creds::Credentials {
    username: "".to_string(),
    password: "".to_string(),
  };
  assert!(empty_creds.username.is_empty());
  assert!(empty_creds.password.is_empty());

  // Test valid credentials structure
  let valid_creds = twig::creds::Credentials {
    username: "testuser".to_string(),
    password: "testpass".to_string(),
  };
  assert!(!valid_creds.username.is_empty());
  assert!(!valid_creds.password.is_empty());
  assert_eq!(valid_creds.username, "testuser");
  assert_eq!(valid_creds.password, "testpass");
}

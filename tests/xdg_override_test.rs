use std::path::Path;

use twig_test_utils::{ConfigDirsTestGuard, EnvTestGuard, setup_test_env_with_registry};

#[test]
fn test_xdg_override() {
  // Set up the test environment with overridden XDG directories and initialized
  // registry
  let (test_env, config_dirs) = setup_test_env_with_registry().expect("Failed to set up test environment");

  // Verify that the config directories are in our temporary directory
  assert!(
    config_dirs.verify_in_test_env(&test_env),
    "Config directories should be in the test environment"
  );

  // Verify that the registry file was created with the expected content
  assert!(
    config_dirs.verify_registry("[]").expect("Failed to verify registry"),
    "Registry file should exist and contain empty array"
  );
}

#[test]
fn test_xdg_override_custom_names() {
  // Set up the test environment with overridden XDG directories
  let test_env = EnvTestGuard::new();

  // Create a TestConfigDirs instance with custom organization and application
  // names This should use the environment variables set by TestEnv
  let config_dirs = ConfigDirsTestGuard::with_names("custom-org", "custom-qualifier", "custom-app")
    .expect("Failed to create TestConfigDirs");

  // Initialize the config directories with registry
  config_dirs
    .init_with_registry()
    .expect("Failed to initialize config directories");

  // Verify that the config directories are in our temporary directory
  assert!(
    config_dirs.verify_in_test_env(&test_env),
    "Config directories should be in the test environment"
  );

  // Verify that the registry file was created with the expected content
  assert!(
    config_dirs.verify_registry("[]").expect("Failed to verify registry"),
    "Registry file should exist and contain empty array"
  );
}

#[test]
fn test_xdg_override_basic_init() {
  // Set up the test environment with overridden XDG directories and basic
  // initialization
  let (test_env, config_dirs) = twig_test_utils::setup_test_env_with_init().expect("Failed to set up test environment");

  // Verify that the config directories are in our temporary directory
  assert!(
    config_dirs.verify_in_test_env(&test_env),
    "Config directories should be in the test environment"
  );

  // Verify that the registry file does not exist yet (since we only did basic
  // init)
  let registry_path = config_dirs.registry_path();
  assert!(
    !Path::new(&registry_path).exists(),
    "Registry file should not exist with basic init"
  );
}

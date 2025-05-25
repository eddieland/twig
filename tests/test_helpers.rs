use std::path::{Path, PathBuf};
use std::{env, fs};

use anyhow;
use tempfile::TempDir;

/// A test environment that overrides XDG directories to use a per-test
/// temporary directory
pub struct TestEnv {
  /// The temporary directory that will be used for XDG directories
  pub temp_dir: TempDir,
  /// The original XDG_CONFIG_HOME value, if any
  original_config_home: Option<String>,
  /// The original XDG_DATA_HOME value, if any
  original_data_home: Option<String>,
  /// The original XDG_CACHE_HOME value, if any
  original_cache_home: Option<String>,
}

impl TestEnv {
  /// XDG environment variable names
  pub const XDG_CONFIG_HOME: &'static str = "XDG_CONFIG_HOME";
  pub const XDG_DATA_HOME: &'static str = "XDG_DATA_HOME";
  pub const XDG_CACHE_HOME: &'static str = "XDG_CACHE_HOME";

  /// Create a new test environment with overridden XDG directories
  pub fn new() -> Self {
    let temp_dir = TempDir::new().expect("Failed to create temporary directory");

    // Save original XDG environment variables
    let original_config_home = env::var(Self::XDG_CONFIG_HOME).ok();
    let original_data_home = env::var(Self::XDG_DATA_HOME).ok();
    let original_cache_home = env::var(Self::XDG_CACHE_HOME).ok();

    // Override XDG environment variables to use the temporary directory
    let temp_path = temp_dir.path().to_path_buf();
    unsafe {
      env::set_var(Self::XDG_CONFIG_HOME, temp_path.join("config"));
    }
    unsafe {
      env::set_var(Self::XDG_DATA_HOME, temp_path.join("data"));
    }
    unsafe {
      env::set_var(Self::XDG_CACHE_HOME, temp_path.join("cache"));
    }

    // Create the XDG directories
    std::fs::create_dir_all(temp_path.join("config")).expect("Failed to create config directory");
    std::fs::create_dir_all(temp_path.join("data")).expect("Failed to create data directory");
    std::fs::create_dir_all(temp_path.join("cache")).expect("Failed to create cache directory");

    Self {
      temp_dir,
      original_config_home,
      original_data_home,
      original_cache_home,
    }
  }

  /// Get the path to the XDG config directory
  #[allow(dead_code)]
  pub fn config_dir(&self) -> PathBuf {
    self.temp_dir.path().join("config")
  }

  /// Get the path to the XDG data directory
  #[allow(dead_code)]
  pub fn data_dir(&self) -> PathBuf {
    self.temp_dir.path().join("data")
  }

  /// Get the path to the XDG cache directory
  #[allow(dead_code)]
  pub fn cache_dir(&self) -> PathBuf {
    self.temp_dir.path().join("cache")
  }
}

impl Drop for TestEnv {
  fn drop(&mut self) {
    // Restore original XDG environment variables
    match &self.original_config_home {
      Some(val) => unsafe {
        env::set_var(TestEnv::XDG_CONFIG_HOME, val);
      },
      None => unsafe {
        env::remove_var(TestEnv::XDG_CONFIG_HOME);
      },
    }

    match &self.original_data_home {
      Some(val) => unsafe {
        env::set_var(TestEnv::XDG_DATA_HOME, val);
      },
      None => unsafe {
        env::remove_var(TestEnv::XDG_DATA_HOME);
      },
    }

    match &self.original_cache_home {
      Some(val) => unsafe {
        env::set_var(TestEnv::XDG_CACHE_HOME, val);
      },
      None => unsafe {
        env::remove_var(TestEnv::XDG_CACHE_HOME);
      },
    }
  }
}

/// A reusable configuration directory structure for testing
pub struct TestConfigDirs {
  /// The configuration directory
  pub config_dir: PathBuf,
  /// The data directory
  pub data_dir: PathBuf,
  /// The cache directory (optional)
  pub cache_dir: Option<PathBuf>,
  /// The organization name used for paths
  #[allow(dead_code)]
  pub organization: String,
  /// The application name used for paths
  #[allow(dead_code)]
  pub application: String,
}

impl TestConfigDirs {
  /// Create a new TestConfigDirs instance with default organization and
  /// application names
  #[allow(dead_code)]
  pub fn new() -> anyhow::Result<Self> {
    Self::with_names("ai", "lat", "twig")
  }

  /// Create a new TestConfigDirs instance with custom organization and
  /// application names
  pub fn with_names(organization: &str, _qualifier: &str, application: &str) -> anyhow::Result<Self> {
    // Get the XDG environment variables
    let config_home = env::var(TestEnv::XDG_CONFIG_HOME).map_err(|_| anyhow::anyhow!("XDG_CONFIG_HOME not set"))?;

    let data_home = env::var(TestEnv::XDG_DATA_HOME).map_err(|_| anyhow::anyhow!("XDG_DATA_HOME not set"))?;

    let cache_home = env::var(TestEnv::XDG_CACHE_HOME).ok();

    // Construct the application-specific paths
    let config_dir = PathBuf::from(config_home).join(format!("{}/{}", organization, application));
    let data_dir = PathBuf::from(data_home).join(format!("{}/{}", organization, application));
    let cache_dir = cache_home.map(|dir| PathBuf::from(dir).join(format!("{}/{}", organization, application)));

    Ok(Self {
      config_dir,
      data_dir,
      cache_dir,
      organization: organization.to_string(),
      application: application.to_string(),
    })
  }

  /// Initialize the configuration directories
  pub fn init(&self) -> anyhow::Result<()> {
    // Create the config directory and its parent directories
    fs::create_dir_all(&self.config_dir).map_err(|e| anyhow::anyhow!("Failed to create config directory: {}", e))?;

    // Create the data directory and its parent directories
    fs::create_dir_all(&self.data_dir).map_err(|e| anyhow::anyhow!("Failed to create data directory: {}", e))?;

    // Create the cache directory if it exists
    if let Some(cache_dir) = &self.cache_dir {
      fs::create_dir_all(cache_dir).map_err(|e| anyhow::anyhow!("Failed to create cache directory: {}", e))?;
    }

    Ok(())
  }

  /// Initialize the configuration directories and create an empty registry file
  pub fn init_with_registry(&self) -> anyhow::Result<()> {
    // First initialize the directories
    self.init()?;

    // Create an empty registry file if it doesn't exist
    let registry_path = self.registry_path();

    // Ensure the parent directory exists
    if let Some(parent) = registry_path.parent() {
      fs::create_dir_all(parent).map_err(|e| anyhow::anyhow!("Failed to create registry parent directory: {}", e))?;
    }

    // Write the empty registry file
    fs::write(&registry_path, "[]").map_err(|e| anyhow::anyhow!("Failed to create empty registry file: {}", e))?;

    Ok(())
  }

  /// Get the path to the registry file
  pub fn registry_path(&self) -> PathBuf {
    self.data_dir.join("registry.json")
  }

  /// Verify that the configuration directories are in the expected location
  pub fn verify_in_test_env(&self, test_env: &TestEnv) -> bool {
    // Check if the directories are within the test environment
    let config_in_test = self.config_dir.starts_with(test_env.temp_dir.path());
    let data_in_test = self.data_dir.starts_with(test_env.temp_dir.path());

    let cache_in_test = match &self.cache_dir {
      Some(cache_dir) => cache_dir.starts_with(test_env.temp_dir.path()),
      None => true,
    };

    config_in_test && data_in_test && cache_in_test
  }

  /// Verify that the registry file exists and contains the expected content
  pub fn verify_registry(&self, expected_content: &str) -> anyhow::Result<bool> {
    let registry_path = self.registry_path();
    if !Path::new(&registry_path).exists() {
      return Ok(false);
    }

    let registry_content =
      fs::read_to_string(registry_path).map_err(|e| anyhow::anyhow!("Failed to read registry file: {}", e))?;

    Ok(registry_content == expected_content)
  }
}

/// A helper function to set up a test environment with TestConfigDirs
#[allow(dead_code)]
pub fn setup_test_env() -> anyhow::Result<(TestEnv, TestConfigDirs)> {
  // Set up the test environment with overridden XDG directories
  let test_env = TestEnv::new();

  // Create a TestConfigDirs instance, which should use our overridden XDG
  // directories
  let config_dirs = TestConfigDirs::new()?;

  Ok((test_env, config_dirs))
}

/// A helper function to set up a test environment with TestConfigDirs and
/// initialize it
pub fn setup_test_env_with_init() -> anyhow::Result<(TestEnv, TestConfigDirs)> {
  // Set up the test environment with overridden XDG directories
  let test_env = TestEnv::new();

  // Create a TestConfigDirs instance with paths directly in the test environment
  let config_dir = test_env.temp_dir.path().join("config/ai/twig");
  let data_dir = test_env.temp_dir.path().join("data/ai/twig");
  let cache_dir = Some(test_env.temp_dir.path().join("cache/ai/twig"));

  let config_dirs = TestConfigDirs {
    config_dir,
    data_dir,
    cache_dir,
    organization: "ai".to_string(),
    application: "twig".to_string(),
  };

  // Initialize the config directories
  config_dirs.init()?;

  Ok((test_env, config_dirs))
}

/// A helper function to set up a test environment with TestConfigDirs and
/// initialize it with a registry
pub fn setup_test_env_with_registry() -> anyhow::Result<(TestEnv, TestConfigDirs)> {
  // Set up the test environment with overridden XDG directories
  let test_env = TestEnv::new();

  // Create a TestConfigDirs instance with paths directly in the test environment
  let config_dir = test_env.temp_dir.path().join("config/ai/twig");
  let data_dir = test_env.temp_dir.path().join("data/ai/twig");
  let cache_dir = Some(test_env.temp_dir.path().join("cache/ai/twig"));

  let config_dirs = TestConfigDirs {
    config_dir,
    data_dir,
    cache_dir,
    organization: "ai".to_string(),
    application: "twig".to_string(),
  };

  // Initialize the config directories with registry
  config_dirs.init_with_registry()?;

  Ok((test_env, config_dirs))
}

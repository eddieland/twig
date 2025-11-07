//! Configuration helpers for the flow renderer.
//!
//! The renderer ships with sane defaults, but internal consumers may override
//! the table schema by dropping a `flow_renderer.toml` file in the Twig config
//! directory. This module exposes the configuration structures and helpers for
//! deserialising that file into a [`BranchTableSchema`].

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::renderer::{BranchTableColumn, BranchTableColumnKind, BranchTableSchema};
use crate::config::ConfigDirs;

/// Schema configuration stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowRendererSchemaConfig {
  #[serde(default = "FlowRendererSchemaConfig::default_columns")]
  pub columns: Vec<FlowRendererColumnConfig>,
  #[serde(default = "FlowRendererSchemaConfig::default_placeholder")]
  pub placeholder: String,
  #[serde(default = "FlowRendererSchemaConfig::default_column_spacing")]
  pub column_spacing: usize,
  #[serde(default = "FlowRendererSchemaConfig::default_show_header")]
  pub show_header: bool,
}

impl Default for FlowRendererSchemaConfig {
  fn default() -> Self {
    Self {
      columns: Self::default_columns(),
      placeholder: Self::default_placeholder(),
      column_spacing: Self::default_column_spacing(),
      show_header: Self::default_show_header(),
    }
  }
}

impl FlowRendererSchemaConfig {
  fn default_columns() -> Vec<FlowRendererColumnConfig> {
    vec![
      FlowRendererColumnConfig::Branch {
        title: None,
        min_width: Some(8),
      },
      FlowRendererColumnConfig::Story {
        title: None,
        min_width: Some(8),
      },
      FlowRendererColumnConfig::Annotation {
        key: "twig.pr".into(),
        title: Some("PR".into()),
        min_width: Some(6),
      },
      FlowRendererColumnConfig::Notes {
        title: None,
        min_width: Some(8),
      },
    ]
  }

  fn default_placeholder() -> String {
    "--".to_string()
  }

  fn default_column_spacing() -> usize {
    2
  }

  fn default_show_header() -> bool {
    true
  }

  /// Convert the config into a [`BranchTableSchema`].
  pub fn to_schema(&self) -> Result<BranchTableSchema, FlowRendererSchemaError> {
    if self.columns.is_empty() {
      return Err(FlowRendererSchemaError::EmptyColumns);
    }

    if !matches!(self.columns.first(), Some(FlowRendererColumnConfig::Branch { .. })) {
      return Err(FlowRendererSchemaError::MissingBranchColumn);
    }

    let columns = self.columns.iter().map(FlowRendererColumnConfig::to_column).collect();

    let schema = BranchTableSchema::new(columns)
      .with_placeholder(self.placeholder.clone())
      .with_column_spacing(self.column_spacing)
      .with_header(self.show_header);

    Ok(schema)
  }
}

/// Column configuration entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum FlowRendererColumnConfig {
  /// Branch tree column (must be first).
  Branch {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    min_width: Option<usize>,
  },
  /// Story / label column.
  Story {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    min_width: Option<usize>,
  },
  /// Annotation column for arbitrary metadata keys.
  Annotation {
    key: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    min_width: Option<usize>,
  },
  /// Notes column.
  Notes {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    min_width: Option<usize>,
  },
}

impl FlowRendererColumnConfig {
  fn to_column(&self) -> BranchTableColumn {
    match self {
      FlowRendererColumnConfig::Branch { title, min_width } => {
        apply_mutations(BranchTableColumn::branch(), title, min_width)
      }
      FlowRendererColumnConfig::Story { title, min_width } => {
        apply_mutations(BranchTableColumn::story(), title, min_width)
      }
      FlowRendererColumnConfig::Annotation { key, title, min_width } => {
        let initial = BranchTableColumn::new(
          title.clone().unwrap_or_else(|| "Annotation".into()),
          BranchTableColumnKind::Annotation { key: key.clone() },
        );
        apply_mutations(initial, title, min_width)
      }
      FlowRendererColumnConfig::Notes { title, min_width } => {
        apply_mutations(BranchTableColumn::notes(), title, min_width)
      }
    }
  }
}

fn apply_mutations(
  mut column: BranchTableColumn,
  title: &Option<String>,
  min_width: &Option<usize>,
) -> BranchTableColumn {
  if let Some(title) = title {
    column = column.with_title(title.clone());
  }
  if let Some(min_width) = min_width {
    column = column.with_min_width(*min_width);
  }
  column
}

/// Errors raised while parsing or validating config files.
#[derive(Debug, Error)]
pub enum FlowRendererSchemaError {
  /// Config declared no columns.
  #[error("flow renderer config must declare at least one column")]
  EmptyColumns,
  /// First column must be the branch column.
  #[error("flow renderer config must declare the branch column first")]
  MissingBranchColumn,
}

/// Load a schema override from the user's config directories.
pub fn load_schema_from_config_dirs(config_dirs: &ConfigDirs) -> Result<Option<BranchTableSchema>> {
  load_schema_from_path(config_dirs.flow_renderer_config_path())
}

/// Load a schema override from the provided path, if it exists.
pub fn load_schema_from_path<P: AsRef<Path>>(path: P) -> Result<Option<BranchTableSchema>> {
  let path = path.as_ref();
  if !path.exists() {
    return Ok(None);
  }

  let contents =
    fs::read_to_string(path).with_context(|| format!("Failed to read flow renderer config from {}", path.display()))?;
  let schema_config: FlowRendererSchemaConfig = toml::from_str(&contents)
    .with_context(|| format!("Failed to parse flow renderer config from {}", path.display()))?;

  let schema = schema_config
    .to_schema()
    .map_err(anyhow::Error::from)
    .with_context(|| format!("Flow renderer config at {} is invalid", path.display()))?;

  Ok(Some(schema))
}

#[cfg(test)]
mod tests {
  use tempfile::TempDir;

  use super::*;

  #[test]
  fn converts_config_to_schema() {
    let config = FlowRendererSchemaConfig {
      columns: vec![
        FlowRendererColumnConfig::Branch {
          title: Some("Branches".into()),
          min_width: Some(12),
        },
        FlowRendererColumnConfig::Annotation {
          key: "twig.story".into(),
          title: Some("Story".into()),
          min_width: Some(6),
        },
      ],
      placeholder: "—".into(),
      column_spacing: 1,
      show_header: true,
    };

    assert!(config.to_schema().is_ok());
  }

  #[test]
  fn loads_config_from_disk() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("flow_renderer.toml");
    fs::write(
      &config_path,
      r#"
        placeholder = "--"
        column_spacing = 4

        [[columns]]
        type = "branch"
        title = "Branch"

        [[columns]]
        type = "notes"
        title = "Notes"
      "#,
    )
    .unwrap();

    assert!(load_schema_from_path(&config_path).unwrap().is_some());
  }

  #[test]
  fn rejects_schema_without_branch_column() {
    let config = FlowRendererSchemaConfig {
      columns: vec![FlowRendererColumnConfig::Notes {
        title: None,
        min_width: None,
      }],
      ..Default::default()
    };

    let err = config.to_schema().unwrap_err();
    assert!(matches!(err, FlowRendererSchemaError::MissingBranchColumn));
  }
}

//! Textual branch table renderer used by the upcoming `twig flow` plugin.
//!
//! This module focuses on presenting a [`BranchGraph`] as a hybrid tree /
//! columnar table so that callers can highlight topology while still keeping
//! metadata aligned under well-known headers. The renderer intentionally keeps
//! IO and Git discovery separate: it only consumes graph structures supplied
//! by callers and writes formatted text into any `fmt::Write` sink.

use std::collections::BTreeSet;
use std::fmt::{self, Write as FmtWrite};

use console::{colors_enabled, measure_text_width, strip_ansi_codes};
use owo_colors::{OwoColorize, Style};
use thiserror::Error;

use super::graph::{BranchAnnotationValue, BranchDivergence, BranchGraph, BranchName, BranchNode, BranchNodeMetadata};

const DEFAULT_PR_ANNOTATION_KEY: &str = "twig.pr";

/// Describes the kind of value rendered within a table column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchTableColumnKind {
  /// Displays the branch tree connectors plus branch name (and current marker).
  Branch,
  /// Renders the first label associated with the branch.
  FirstLabel,
  /// Renders a single annotation entry keyed by the provided string.
  Annotation {
    /// Annotation key (e.g. `jira.story`, `github.pr`).
    key: String,
  },
}

/// Definition of a single column within the branch table.
#[derive(Debug, Clone)]
pub struct BranchTableColumn {
  title: String,
  kind: BranchTableColumnKind,
  min_width: usize,
}

impl BranchTableColumn {
  /// Create a new column descriptor.
  pub fn new(title: impl Into<String>, kind: BranchTableColumnKind) -> Self {
    Self {
      title: title.into(),
      kind,
      min_width: 0,
    }
  }

  /// Convenience helper for the default branch column.
  pub fn branch() -> Self {
    Self::new("Branch", BranchTableColumnKind::Branch)
  }

  /// Convenience helper for the default "Story" column.
  pub fn story() -> Self {
    Self::new("Story", BranchTableColumnKind::FirstLabel)
  }

  /// Convenience helper for the default "PR" column.
  pub fn pull_request() -> Self {
    Self::new(
      "PR",
      BranchTableColumnKind::Annotation {
        key: DEFAULT_PR_ANNOTATION_KEY.to_string(),
      },
    )
  }

  /// Minimum width (in columns) reserved for the current field.
  pub fn with_min_width(mut self, width: usize) -> Self {
    self.min_width = width;
    self
  }

  /// Column header label.
  pub fn title(&self) -> &str {
    &self.title
  }

  /// Column kind describing how the renderer sources values.
  pub fn kind(&self) -> &BranchTableColumnKind {
    &self.kind
  }

  /// Minimum width (characters) reserved for this column.
  pub fn min_width(&self) -> usize {
    self.min_width
  }
}

/// Schema describing how the renderer lays out the table.
#[derive(Debug, Clone)]
pub struct BranchTableSchema {
  columns: Vec<BranchTableColumn>,
  placeholder: String,
  column_spacing: usize,
  show_header: bool,
}

impl BranchTableSchema {
  /// Create a new schema from column descriptors.
  pub fn new(columns: Vec<BranchTableColumn>) -> Self {
    Self {
      columns,
      placeholder: "--".to_string(),
      column_spacing: 2,
      show_header: true,
    }
  }

  /// Replace the placeholder used for missing metadata.
  pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
    self.placeholder = placeholder.into();
    self
  }

  /// Control how many spaces separate each column.
  pub fn with_column_spacing(mut self, spacing: usize) -> Self {
    self.column_spacing = spacing;
    self
  }

  /// Toggle header rendering.
  pub fn with_header(mut self, show_header: bool) -> Self {
    self.show_header = show_header;
    self
  }

  /// Placeholder rendered when metadata is missing.
  pub fn placeholder(&self) -> &str {
    &self.placeholder
  }

  /// Number of spaces inserted between columns.
  pub fn column_spacing(&self) -> usize {
    self.column_spacing
  }

  /// Whether the header row should be rendered.
  pub fn show_header(&self) -> bool {
    self.show_header
  }

  /// Immutable view of the configured columns.
  pub fn columns(&self) -> &[BranchTableColumn] {
    &self.columns
  }

  /// Mutable access for callers that need to tweak column ordering/widths
  /// in-place.
  pub fn columns_mut(&mut self) -> &mut [BranchTableColumn] {
    &mut self.columns
  }
}

impl Default for BranchTableSchema {
  fn default() -> Self {
    Self::new(vec![
      BranchTableColumn::branch().with_min_width(8),
      BranchTableColumn::story().with_min_width(8),
      BranchTableColumn::pull_request().with_min_width(6),
    ])
  }
}

/// Color strategy for branch table rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchTableColorMode {
  /// Enable colors when the output stream supports them.
  Auto,
  /// Force colorized output regardless of terminal detection.
  Always,
  /// Disable colors entirely.
  Never,
}

/// Presentation settings applied to the branch table.
#[derive(Debug, Clone, Copy)]
pub struct BranchTableStyle {
  color_mode: BranchTableColorMode,
  dim_placeholders: bool,
  bold_headers: bool,
  dim_connectors: bool,
}

impl BranchTableStyle {
  /// Create a new style configuration.
  pub fn new(color_mode: BranchTableColorMode) -> Self {
    Self {
      color_mode,
      dim_placeholders: true,
      bold_headers: true,
      dim_connectors: true,
    }
  }

  /// Choose how colors should be applied.
  pub fn with_color_mode(mut self, color_mode: BranchTableColorMode) -> Self {
    self.color_mode = color_mode;
    self
  }

  /// Toggle dimming for placeholder values.
  pub fn with_dim_placeholders(mut self, dim: bool) -> Self {
    self.dim_placeholders = dim;
    self
  }

  /// Toggle bold styling for header cells.
  pub fn with_bold_headers(mut self, bold: bool) -> Self {
    self.bold_headers = bold;
    self
  }

  /// Toggle dimming for tree connectors.
  pub fn with_dim_connectors(mut self, dim: bool) -> Self {
    self.dim_connectors = dim;
    self
  }

  fn colors_enabled(&self) -> bool {
    match self.color_mode {
      BranchTableColorMode::Auto => colors_enabled(),
      BranchTableColorMode::Always => true,
      BranchTableColorMode::Never => false,
    }
  }
}

impl Default for BranchTableStyle {
  fn default() -> Self {
    Self::new(BranchTableColorMode::Auto)
  }
}

/// Error conditions raised while rendering.
#[derive(Debug, Error)]
pub enum BranchTableRenderError {
  /// A schema without columns cannot be rendered.
  #[error("branch table schema does not declare any columns")]
  EmptySchema,
  /// The branch column must appear first for connector rendering to work.
  #[error("branch table schema must declare the branch column as the first entry")]
  MissingBranchColumn,
  /// The requested branch does not exist in the provided graph.
  #[error("branch `{0}` was not found in the branch graph")]
  UnknownBranch(BranchName),
  /// Wrapper around `fmt::Error` originating from the writer implementation.
  #[error(transparent)]
  Fmt(#[from] fmt::Error),
}

/// Stateful renderer that formats a [`BranchGraph`] as a tree-aligned table.
#[derive(Debug, Clone)]
pub struct BranchTableRenderer {
  schema: BranchTableSchema,
  style: BranchTableStyle,
}

impl Default for BranchTableRenderer {
  fn default() -> Self {
    Self::new(BranchTableSchema::default())
  }
}

impl BranchTableRenderer {
  /// Create a renderer backed by the provided schema.
  pub fn new(schema: BranchTableSchema) -> Self {
    Self {
      schema,
      style: BranchTableStyle::default(),
    }
  }

  /// Apply additional styling options before rendering.
  pub fn with_style(mut self, style: BranchTableStyle) -> Self {
    self.style = style;
    self
  }

  /// Render the branch graph rooted at `branch` into the provided writer.
  pub fn render<W: FmtWrite>(
    &self,
    writer: &mut W,
    graph: &BranchGraph,
    branch: &BranchName,
  ) -> Result<(), BranchTableRenderError> {
    if self.schema.columns().is_empty() {
      return Err(BranchTableRenderError::EmptySchema);
    }

    if !matches!(
      self.schema.columns().first().map(|c| c.kind()),
      Some(BranchTableColumnKind::Branch)
    ) {
      return Err(BranchTableRenderError::MissingBranchColumn);
    }

    let colors_enabled = self.style.colors_enabled();

    // Collect rows in depth-first order so we can generate values for each column.
    let mut stack = Vec::new();
    let mut visited = BTreeSet::new();
    let mut rows = Vec::new();
    self.collect_rows(graph, branch, &mut stack, &mut visited, &mut rows)?;

    let rendered_rows = self.render_rows(graph, &rows, colors_enabled)?;
    let column_widths = self.compute_column_widths(&rendered_rows);
    let spacing = " ".repeat(self.schema.column_spacing());

    if self.schema.show_header() {
      let header_cells: Vec<String> = self
        .schema
        .columns()
        .iter()
        .map(|col| {
          if self.style.bold_headers {
            style_text(col.title(), header_style(), colors_enabled)
          } else {
            col.title().to_string()
          }
        })
        .collect();
      self.write_row(writer, &header_cells, &column_widths, &spacing)?;
    }

    for row in rendered_rows {
      self.write_row(writer, &row, &column_widths, &spacing)?;
    }

    Ok(())
  }

  fn collect_rows(
    &self,
    graph: &BranchGraph,
    branch: &BranchName,
    stack: &mut Vec<bool>,
    visited: &mut BTreeSet<BranchName>,
    rows: &mut Vec<TreeRow>,
  ) -> Result<(), BranchTableRenderError> {
    let (children, already_seen) = {
      let node = graph
        .get(branch)
        .ok_or_else(|| BranchTableRenderError::UnknownBranch(branch.clone()))?;

      let tree_prefix = build_tree_prefix(stack);
      rows.push(TreeRow {
        branch: branch.clone(),
        tree_prefix,
      });

      let already_seen = !visited.insert(branch.clone());
      let children = node.topology.children.clone();
      (children, already_seen)
    };

    if already_seen {
      return Ok(());
    }

    for (idx, child) in children.iter().enumerate() {
      stack.push(idx < children.len() - 1);
      self.collect_rows(graph, child, stack, visited, rows)?;
      stack.pop();
    }

    Ok(())
  }

  fn render_rows(
    &self,
    graph: &BranchGraph,
    rows: &[TreeRow],
    colors_enabled: bool,
  ) -> Result<Vec<Vec<String>>, BranchTableRenderError> {
    let mut rendered = Vec::with_capacity(rows.len());
    for row in rows {
      rendered.push(self.render_row(graph, row, colors_enabled)?);
    }
    Ok(rendered)
  }

  fn render_row(
    &self,
    graph: &BranchGraph,
    row: &TreeRow,
    colors_enabled: bool,
  ) -> Result<Vec<String>, BranchTableRenderError> {
    let node = graph
      .get(&row.branch)
      .ok_or_else(|| BranchTableRenderError::UnknownBranch(row.branch.clone()))?;

    let mut cells = Vec::with_capacity(self.schema.columns().len());
    for column in self.schema.columns() {
      let value = match column.kind() {
        BranchTableColumnKind::Branch => self.branch_value(graph, node, &row.tree_prefix, colors_enabled),
        BranchTableColumnKind::FirstLabel => self.style_label(first_label(&node.metadata), colors_enabled),
        BranchTableColumnKind::Annotation { key } => {
          self.style_annotation(annotation_value(&node.metadata, key), key, colors_enabled)
        }
      };

      cells.push(value);
    }

    Ok(cells)
  }

  fn branch_value(&self, graph: &BranchGraph, node: &BranchNode, prefix: &str, colors_enabled: bool) -> String {
    let mut value = String::new();

    if !prefix.is_empty() {
      let styled_prefix = if self.style.dim_connectors {
        style_text(prefix, connector_style(), colors_enabled)
      } else {
        prefix.to_string()
      };
      value.push_str(&styled_prefix);
    }

    let is_current = node.is_current()
      || graph
        .current_branch()
        .map(|current| current == &node.name)
        .unwrap_or(false);

    if is_current {
      let marker = style_text("*", current_branch_style(), colors_enabled);
      value.push_str(&marker);
      value.push(' ');
    }

    let branch_style = if is_current {
      current_branch_style()
    } else {
      branch_name_style()
    };

    value.push_str(&style_text(node.name.as_str(), branch_style, colors_enabled));
    if let Some(divergence) = node.metadata.divergence.as_ref() {
      value.push(' ');
      value.push_str(&format_divergence(divergence, colors_enabled));
    }
    value
  }

  fn style_label(&self, value: Option<String>, colors_enabled: bool) -> String {
    value
      .map(|label| style_text(label, label_style(), colors_enabled))
      .unwrap_or_else(|| self.placeholder(colors_enabled))
  }

  fn style_annotation(&self, value: Option<String>, key: &str, colors_enabled: bool) -> String {
    match value {
      Some(value) => {
        let style = if key == DEFAULT_PR_ANNOTATION_KEY {
          pr_style()
        } else {
          annotation_style()
        };
        style_text(value, style, colors_enabled)
      }
      None => self.placeholder(colors_enabled),
    }
  }

  fn placeholder(&self, colors_enabled: bool) -> String {
    let placeholder = self.schema.placeholder().to_string();
    if colors_enabled && self.style.dim_placeholders {
      style_text(placeholder, placeholder_style(), true)
    } else {
      placeholder
    }
  }

  fn compute_column_widths(&self, rows: &[Vec<String>]) -> Vec<usize> {
    let mut widths: Vec<usize> = self.schema.columns().iter().map(|column| column.min_width()).collect();

    for (idx, column) in self.schema.columns().iter().enumerate() {
      widths[idx] = widths[idx].max(visible_width(column.title()));
    }

    for row in rows {
      for (idx, cell) in row.iter().enumerate() {
        widths[idx] = widths[idx].max(visible_width(cell));
      }
    }

    widths
  }

  fn write_row<W: FmtWrite>(
    &self,
    writer: &mut W,
    cells: &[String],
    widths: &[usize],
    spacing: &str,
  ) -> Result<(), BranchTableRenderError> {
    for (idx, cell) in cells.iter().enumerate() {
      let padded = pad_cell(cell, widths[idx]);
      writer.write_str(&padded)?;
      if idx < cells.len() - 1 {
        writer.write_str(spacing)?;
      }
    }
    writer.write_char('\n')?;
    Ok(())
  }
}

#[derive(Debug, Clone)]
struct TreeRow {
  branch: BranchName,
  tree_prefix: String,
}

fn build_tree_prefix(stack: &[bool]) -> String {
  if stack.is_empty() {
    return String::new();
  }

  let mut prefix = String::new();
  for has_siblings in &stack[..stack.len() - 1] {
    if *has_siblings {
      prefix.push_str("│  ");
    } else {
      prefix.push_str("   ");
    }
  }

  let last_has_siblings = *stack.last().unwrap_or(&false);
  if last_has_siblings {
    prefix.push_str("├─ ");
  } else {
    prefix.push_str("└─ ");
  }

  prefix
}

fn first_label(metadata: &BranchNodeMetadata) -> Option<String> {
  metadata.labels.iter().next().cloned()
}

fn annotation_value(metadata: &BranchNodeMetadata, key: &str) -> Option<String> {
  metadata.annotations.get(key).map(format_annotation_value)
}

fn format_divergence(divergence: &BranchDivergence, colors_enabled: bool) -> String {
  let ahead = if colors_enabled {
    let style = if divergence.ahead == 0 {
      placeholder_style()
    } else {
      divergence_ahead_style()
    };
    style_text(format!("+{}", divergence.ahead), style, true)
  } else {
    format!("+{}", divergence.ahead)
  };

  let behind = if colors_enabled {
    let style = if divergence.behind == 0 {
      placeholder_style()
    } else {
      divergence_behind_style()
    };
    style_text(format!("-{}", divergence.behind), style, true)
  } else {
    format!("-{}", divergence.behind)
  };

  format!("({ahead}/{behind})")
}

fn format_annotation_value(value: &BranchAnnotationValue) -> String {
  match value {
    BranchAnnotationValue::Text(text) => text.clone(),
    BranchAnnotationValue::Numeric(value) => value.to_string(),
    BranchAnnotationValue::Timestamp(ts) => ts.to_rfc3339(),
    BranchAnnotationValue::Flag(flag) => flag.to_string(),
  }
}

fn pad_cell(value: &str, width: usize) -> String {
  let current_width = visible_width(value);
  if current_width >= width {
    value.to_string()
  } else {
    let mut output = String::from(value);
    output.push_str(&" ".repeat(width - current_width));
    output
  }
}

fn style_text(value: impl AsRef<str>, style: Style, colors_enabled: bool) -> String {
  if colors_enabled {
    value.as_ref().style(style).to_string()
  } else {
    value.as_ref().to_string()
  }
}

fn connector_style() -> Style {
  Style::new().bright_black()
}

fn branch_name_style() -> Style {
  Style::new().bold()
}

fn current_branch_style() -> Style {
  Style::new().bright_green().bold()
}

fn divergence_ahead_style() -> Style {
  Style::new().green()
}

fn divergence_behind_style() -> Style {
  Style::new().red()
}

fn label_style() -> Style {
  Style::new().cyan()
}

fn pr_style() -> Style {
  Style::new().yellow()
}

fn annotation_style() -> Style {
  Style::new().magenta()
}

fn placeholder_style() -> Style {
  Style::new().bright_black()
}

fn header_style() -> Style {
  Style::new().bright_white().bold()
}

fn visible_width(value: &str) -> usize {
  measure_text_width(&strip_ansi_codes(value))
}

#[cfg(test)]
mod tests {
  use chrono::{TimeZone, Utc};
  use git2::Oid;
  use insta::assert_snapshot;

  use super::super::graph::{BranchDivergence, BranchHead, BranchKind, BranchTopology};
  use super::*;

  const LIFECYCLE_KEY: &str = "twig.lifecycle";

  #[test]
  fn renders_default_schema() {
    let (graph, root) = sample_graph();
    let mut output = String::new();
    BranchTableRenderer::default()
      .with_style(BranchTableStyle::new(BranchTableColorMode::Always))
      .render(&mut output, &graph, &root)
      .unwrap();

    assert_snapshot!("flow_renderer__default_schema", output);
  }

  #[test]
  fn uses_placeholder_for_missing_values() {
    let (graph, root) = minimal_graph();
    let mut output = String::new();
    BranchTableRenderer::default()
      .with_style(BranchTableStyle::new(BranchTableColorMode::Always))
      .render(&mut output, &graph, &root)
      .unwrap();

    assert_snapshot!("flow_renderer__placeholders", output);
  }

  #[test]
  fn validates_branch_column_presence() {
    let schema = BranchTableSchema::new(vec![BranchTableColumn::story()]);
    let renderer = BranchTableRenderer::new(schema);
    let (graph, root) = minimal_graph();
    let mut output = String::new();

    let err = renderer.render(&mut output, &graph, &root).unwrap_err();
    assert!(matches!(err, BranchTableRenderError::MissingBranchColumn));
  }

  #[test]
  fn renders_without_header_when_disabled() {
    let (graph, root) = sample_graph();
    let mut output = String::new();
    BranchTableRenderer::new(BranchTableSchema::default().with_header(false))
      .with_style(BranchTableStyle::new(BranchTableColorMode::Always))
      .render(&mut output, &graph, &root)
      .unwrap();

    assert_snapshot!("flow_renderer__no_header", output);
  }

  #[test]
  fn renders_custom_schema_with_additional_columns() {
    let (graph, root) = sample_graph();
    let schema = BranchTableSchema::new(vec![
      BranchTableColumn::branch().with_min_width(10),
      BranchTableColumn::story(),
      BranchTableColumn::pull_request(),
      BranchTableColumn::new(
        "Lifecycle",
        BranchTableColumnKind::Annotation {
          key: LIFECYCLE_KEY.to_string(),
        },
      )
      .with_min_width(10),
    ])
    .with_placeholder("—")
    .with_column_spacing(3);

    let mut output = String::new();
    BranchTableRenderer::new(schema)
      .with_style(BranchTableStyle::new(BranchTableColorMode::Always))
      .render(&mut output, &graph, &root)
      .unwrap();

    assert_snapshot!("flow_renderer__custom_schema", output);
  }

  #[test]
  fn falls_back_to_plain_text_when_colors_disabled() {
    let (graph, root) = sample_graph();
    let mut output = String::new();
    BranchTableRenderer::default()
      .with_style(BranchTableStyle::new(BranchTableColorMode::Never))
      .render(&mut output, &graph, &root)
      .unwrap();

    assert_snapshot!("flow_renderer__default_schema_no_color", output);
  }

  fn sample_graph() -> (BranchGraph, BranchName) {
    let mut main = branch_node("main");
    main.metadata.is_current = true;

    let mut feature_auth = branch_node("feature/auth-refresh");
    feature_auth.metadata.labels.insert("PROJ-451".into());

    let mut feature_auth_ui = branch_node("feature/auth-refresh-ui");
    feature_auth_ui.metadata.labels.insert("PROJ-451".into());
    feature_auth_ui.metadata.annotations.insert(
      DEFAULT_PR_ANNOTATION_KEY.to_string(),
      BranchAnnotationValue::Text("#982".into()),
    );

    let mut feature_payment = branch_node("feature/payment-refactor");
    feature_payment.metadata.annotations.insert(
      DEFAULT_PR_ANNOTATION_KEY.to_string(),
      BranchAnnotationValue::Text("draft".into()),
    );

    let mut feature_payment_api = branch_node("feature/payment-api");

    let mut feature_payment_ui = branch_node("feature/payment-ui");

    main.topology.children = vec![feature_auth.name.clone(), feature_payment.name.clone()];
    feature_auth.topology.children = vec![feature_auth_ui.name.clone()];
    feature_payment.topology.children = vec![feature_payment_api.name.clone(), feature_payment_ui.name.clone()];

    feature_auth
      .metadata
      .annotations
      .insert(LIFECYCLE_KEY.to_string(), BranchAnnotationValue::Text("active".into()));
    feature_payment
      .metadata
      .annotations
      .insert(LIFECYCLE_KEY.to_string(), BranchAnnotationValue::Text("stale".into()));
    feature_payment_ui
      .metadata
      .annotations
      .insert(LIFECYCLE_KEY.to_string(), BranchAnnotationValue::Text("ready".into()));

    feature_auth.metadata.divergence = Some(BranchDivergence { ahead: 3, behind: 0 });
    feature_auth_ui.metadata.divergence = Some(BranchDivergence { ahead: 1, behind: 1 });
    feature_payment.metadata.divergence = Some(BranchDivergence { ahead: 0, behind: 2 });
    feature_payment_api.metadata.divergence = Some(BranchDivergence { ahead: 1, behind: 0 });
    feature_payment_ui.metadata.divergence = Some(BranchDivergence { ahead: 2, behind: 0 });

    let root = main.name.clone();
    let nodes = vec![
      main,
      feature_auth,
      feature_auth_ui,
      feature_payment,
      feature_payment_api,
      feature_payment_ui,
    ];
    let graph = BranchGraph::from_parts(nodes, Vec::new(), vec![root.clone()], Some(root.clone()));

    (graph, root)
  }

  fn minimal_graph() -> (BranchGraph, BranchName) {
    let mut root = branch_node("main");
    root.metadata.is_current = true;
    let name = root.name.clone();
    let graph = BranchGraph::from_parts(vec![root], Vec::new(), vec![name.clone()], Some(name.clone()));
    (graph, name)
  }

  fn branch_node(name: &str) -> BranchNode {
    BranchNode {
      name: BranchName::from(name),
      kind: BranchKind::Local,
      head: BranchHead {
        oid: Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap(),
        summary: Some(format!("Summary for {name}")),
        author: Some("Twig Bot".to_string()),
        committed_at: Some(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()),
      },
      upstream: None,
      topology: BranchTopology::default(),
      metadata: BranchNodeMetadata::default(),
    }
  }
}

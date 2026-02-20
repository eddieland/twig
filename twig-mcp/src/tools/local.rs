//! Parameter structs for local state tools.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BranchMetadataParams {
  /// Branch name to look up.
  pub branch: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BranchTreeParams {
  /// Optional branch to use as the tree root. Defaults to the default root.
  pub branch: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BranchStackParams {
  /// Branch to trace from. Defaults to current branch.
  pub branch: Option<String>,
}

//! Parameter structs for GitHub tools.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetPullRequestParams {
  /// PR number. Defaults to the current branch's PR if omitted.
  pub pr_number: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetPrStatusParams {
  /// PR number. Defaults to the current branch's PR if omitted.
  pub pr_number: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListPullRequestsParams {
  /// Filter by state: "open", "closed", or "all". Defaults to "open".
  pub state: Option<String>,
}

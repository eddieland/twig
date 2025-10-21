//! Flow utilities shared between the twig CLI and plugins.

pub mod switch;

pub use switch::{
  InputType, detect_input_type, handle_branch_switch, handle_github_pr_switch, handle_jira_switch, handle_root_switch,
};

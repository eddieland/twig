//! # Interactive Commit Selection
//!
//! This module provides an interactive fuzzy finder interface for selecting
//! commit candidates using the skim library. It presents a user-friendly
//! interface that allows developers to quickly search and select from
//! scored commit candidates.
//!
//! ## Functionality
//!
//! - **Fuzzy Search**: Full-text search across commit information
//! - **Rich Display**: Shows commit hash, relative time, author, and indicators
//! - **Visual Indicators**: Uses symbols to indicate authorship and Jira
//!   association
//! - **Keyboard Navigation**: Standard fuzzy finder controls for selection
//!
//! ## Display Format
//!
//! Each commit is displayed with the following format:
//! ```text
//! <short_hash> <relative_time> <jira_indicator> <author_indicator> <message> (<author>)
//! ```
//!
//! Where:
//! - `jira_indicator`: 🎫 if commit has Jira issue, space otherwise
//! - `author_indicator`: ● for current user, ○ for other users
//! - `relative_time`: Human-readable time (e.g., "2h ago", "3d ago")
//!
//! ## User Experience
//!
//! The interface allows users to:
//! - Type to filter commits by any visible text
//! - Use arrow keys to navigate through results
//! - Press Enter to select a commit
//! - Press Escape to cancel selection

use std::borrow::Cow;
use std::sync::Arc;

use anyhow::Result;
use skim::prelude::*;

use crate::fixup::commit_collector::CommitCandidate;

/// Wrapper for commit candidates to implement SkimItem
#[derive(Debug, Clone)]
struct CommitItem {
  candidate: CommitCandidate,
  display_text: String,
}

impl SkimItem for CommitItem {
  fn text(&self) -> Cow<'_, str> {
    Cow::Borrowed(&self.display_text)
  }
}

/// Select a commit interactively using skim
pub fn select_commit(candidates: &[CommitCandidate]) -> Result<Option<CommitCandidate>> {
  if candidates.is_empty() {
    return Ok(None);
  }

  // Create skim items
  let items: Vec<Arc<dyn SkimItem>> = candidates
    .iter()
    .map(|candidate| {
      let display_text = format_candidate_for_display(candidate);
      Arc::new(CommitItem {
        candidate: candidate.clone(),
        display_text,
      }) as Arc<dyn SkimItem>
    })
    .collect();

  // Create receiver channel
  let (tx, rx): (SkimItemSender, SkimItemReceiver) = unbounded();

  // Send items to skim
  for item in items {
    let _ = tx.send(item);
  }
  drop(tx); // Close the sender

  // Configure skim options
  let options = SkimOptionsBuilder::default()
    .height("50%".to_string())
    .multi(false)
    .prompt("Select commit to fixup: ".to_string())
    .build()
    .map_err(|e| anyhow::anyhow!("Failed to build skim options: {}", e))?;

  // Run skim
  let selected_items = Skim::run_with(&options, Some(rx))
    .map(|out| out.selected_items)
    .unwrap_or_default();

  if selected_items.is_empty() {
    return Ok(None);
  }

  // Extract the selected candidate
  if let Some(item) = selected_items[0].as_any().downcast_ref::<CommitItem>() {
    Ok(Some(item.candidate.clone()))
  } else {
    Err(anyhow::anyhow!("Failed to extract selected commit"))
  }
}

/// Formats a commit candidate for display in the fuzzy finder.
///
/// This function creates a human-readable representation of a commit that
/// includes visual indicators and relative timing information. The format
/// is optimized for quick scanning and identification of relevant commits.
///
/// # Arguments
///
/// * `candidate` - The commit candidate to format
///
/// # Returns
///
/// A formatted string containing commit hash, timing, indicators, message, and
/// author.
///
/// # Format
///
/// The returned string follows this pattern:
/// ```text
/// abc123d 2h ago 🎫 ● Fix user authentication bug (john.doe)
/// ```
///
/// Where:
/// - `abc123d` is the short commit hash
/// - `2h ago` is the relative time
/// - `🎫` indicates a Jira issue is present (space if none)
/// - `●` indicates current user (○ for others)
/// - `Fix user authentication bug` is the commit message
/// - `(john.doe)` is the author name
fn format_candidate_for_display(candidate: &CommitCandidate) -> String {
  let relative_time = format_relative_time(&candidate.date);
  let author_indicator = if candidate.is_current_user { "●" } else { "○" };
  let jira_indicator = if candidate.jira_issue.is_some() { "🎫" } else { " " };

  format!(
    "{} {} {} {} {} ({})",
    candidate.short_hash, relative_time, jira_indicator, author_indicator, candidate.message, candidate.author
  )
}

/// Format a relative time string
fn format_relative_time(date: &chrono::DateTime<chrono::Utc>) -> String {
  let now = chrono::Utc::now();
  let duration = now.signed_duration_since(*date);

  if duration.num_days() > 0 {
    format!("{}d ago", duration.num_days())
  } else if duration.num_hours() > 0 {
    format!("{}h ago", duration.num_hours())
  } else if duration.num_minutes() > 0 {
    format!("{}m ago", duration.num_minutes())
  } else {
    "just now".to_string()
  }
}

#[cfg(test)]
mod tests {
  use chrono::Utc;

  use super::*;

  fn create_test_candidate(short_hash: &str, hours_ago: i64) -> CommitCandidate {
    let now = Utc::now();
    let date = now - chrono::Duration::hours(hours_ago);

    CommitCandidate {
      hash: format!("full_hash_{short_hash}",),
      short_hash: short_hash.to_string(),
      message: "Test commit message".to_string(),
      author: "test_user".to_string(),
      date,
      is_current_user: true,
      jira_issue: Some("PROJ-123".to_string()),
      score: 0.8,
    }
  }

  #[test]
  fn test_format_candidate_for_display() {
    let candidate = create_test_candidate("abc123", 2);
    let formatted = format_candidate_for_display(&candidate);

    assert!(formatted.contains("abc123"));
    assert!(formatted.contains("2h ago"));
    assert!(formatted.contains("Test commit message"));
    assert!(formatted.contains("test_user"));
    assert!(formatted.contains("🎫")); // Jira indicator
    assert!(formatted.contains("●")); // Current user indicator
  }

  #[test]
  fn test_format_relative_time() {
    let now = Utc::now();

    // Test days
    let days_ago = now - chrono::Duration::days(3);
    assert_eq!(format_relative_time(&days_ago), "3d ago");

    // Test hours
    let hours_ago = now - chrono::Duration::hours(5);
    assert_eq!(format_relative_time(&hours_ago), "5h ago");

    // Test minutes
    let minutes_ago = now - chrono::Duration::minutes(30);
    assert_eq!(format_relative_time(&minutes_ago), "30m ago");

    // Test recent
    let seconds_ago = now - chrono::Duration::seconds(30);
    assert_eq!(format_relative_time(&seconds_ago), "just now");
  }
}

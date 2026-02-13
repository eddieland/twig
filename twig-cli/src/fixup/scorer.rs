//! # Commit Scorer
//!
//! Implements the scoring algorithm for commit candidates based on branch
//! uniqueness, recency, authorship, and Jira issue association.

use anyhow::Result;
use chrono::Utc;

use crate::cli::fixup::FixupArgs;
use crate::fixup::commit_collector::CommitCandidate;

/// Scores and sorts commit candidates based on relevance criteria.
///
/// Uses a **hard partition** approach: branch-unique commits ALWAYS rank above
/// shared commits, regardless of other factors. Within each tier, commits are
/// ordered by secondary factors.
///
/// ## Tier 1: Branch-unique commits (score >= 10.0)
/// Commits that exist only on the current branch (not reachable from the
/// parent/root branch). These are almost always the correct fixup targets.
///
/// ## Tier 2: Shared commits (score < 10.0)
/// Commits reachable from both the current and comparison branches.
///
/// ## Secondary factors (within each tier)
/// - **Recency (50%)**: More recent commits score higher
/// - **Authorship (30%)**: Commits by the current user score higher
/// - **Jira Association (20%)**: Commits with matching Jira issues score higher
///
/// # Scoring Formula
///
/// ```text
/// score = tier_bonus + (recency_factor * 0.50) + (authorship_bonus * 0.30)
///       + (jira_bonus * 0.20)
/// ```
///
/// Where:
/// - `tier_bonus` = 10.0 if branch-unique, 0.0 otherwise
/// - `recency_factor` = (max_days - days_ago) / max_days, clamped to [0.0, 1.0]
/// - `authorship_bonus` = 1.0 if current user, 0.0 otherwise
/// - `jira_bonus` = 1.0 if Jira issues match, 0.0 otherwise
///
/// # Arguments
///
/// * `candidates` - Mutable slice of commit candidates to score and sort
/// * `args` - Fixup command arguments containing scoring parameters (days limit)
/// * `current_jira_issue` - The current branch's Jira issue for scoring bonus
///
/// # Returns
///
/// Returns `Ok(())` on success. The candidates slice is modified in-place with
/// updated scores and sorted by relevance.
pub fn score_commits(
  candidates: &mut [CommitCandidate],
  args: &FixupArgs,
  current_jira_issue: Option<String>,
) -> Result<()> {
  let now = Utc::now();

  tracing::debug!("Scoring {} candidates", candidates.len());

  for candidate in candidates.iter_mut() {
    let mut score = 0.0;

    // Hard partition: branch-unique commits ALWAYS rank above shared commits.
    // The tier bonus of 10.0 is unreachable by secondary factors alone (max ~1.0),
    // guaranteeing that any branch-unique commit outranks any shared commit.
    let tier_bonus = if candidate.is_branch_unique { 10.0 } else { 0.0 };
    score += tier_bonus;

    // Recency score (50% of secondary weight)
    let days_ago = (now - candidate.date).num_days() as f64;
    let max_days = args.days as f64;
    let recency_score = ((max_days - days_ago) / max_days).max(0.0);
    score += recency_score * 0.50;

    // Authorship score (30% of secondary weight)
    let authorship_score = if candidate.is_current_user { 0.30 } else { 0.0 };
    score += authorship_score;

    // Jira association score (20% of secondary weight)
    let jira_score = if let (Some(current_issue), Some(commit_issue)) = (&current_jira_issue, &candidate.jira_issue)
      && current_issue == commit_issue
    {
      0.20
    } else {
      0.0
    };
    score += jira_score;

    candidate.score = score;

    tracing::trace!(
      "Scored commit {}: branch_unique={}, tier={:.1}, recency={:.3}, authorship={:.2}, jira={:.2}, total={:.3}",
      candidate.short_hash,
      candidate.is_branch_unique,
      tier_bonus,
      recency_score * 0.50,
      authorship_score,
      jira_score,
      score
    );
  }

  // Sort by score (highest first)
  candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

  tracing::debug!("Sorted candidates by score");

  Ok(())
}

#[cfg(test)]
mod tests {
  use chrono::Utc;

  use super::*;

  fn create_test_candidate(
    short_hash: &str,
    days_ago: i64,
    is_current_user: bool,
    jira_issue: Option<String>,
  ) -> CommitCandidate {
    create_test_candidate_full(short_hash, days_ago, is_current_user, jira_issue, false)
  }

  fn create_test_candidate_full(
    short_hash: &str,
    days_ago: i64,
    is_current_user: bool,
    jira_issue: Option<String>,
    is_branch_unique: bool,
  ) -> CommitCandidate {
    let now = Utc::now();
    let date = now - chrono::Duration::days(days_ago);

    CommitCandidate {
      hash: format!("full_hash_{short_hash}",),
      short_hash: short_hash.to_string(),
      message: format!("Test commit {short_hash}",),
      author: if is_current_user { "current_user" } else { "other_user" }.to_string(),
      date,
      is_current_user,
      jira_issue,
      is_branch_unique,
      score: 0.0,
    }
  }

  #[test]
  fn test_scoring_algorithm() {
    let mut candidates = vec![
      create_test_candidate("abc123", 1, true, Some("PROJ-123".to_string())),
      create_test_candidate("def456", 5, false, None),
      create_test_candidate("ghi789", 2, true, None),
    ];

    let args = FixupArgs {
      limit: 20,
      days: 30,
      all_authors: false,
      include_fixups: false,
      dry_run: false,
      vim_mode: false,
    };

    // Mock the current Jira issue for testing
    let current_jira_issue = Some("PROJ-123".to_string());

    score_commits(&mut candidates, &args, current_jira_issue).unwrap();

    // Verify scores are calculated and non-negative
    for candidate in &candidates {
      assert!(candidate.score >= 0.0);
    }

    // Verify sorting (highest score first)
    for i in 1..candidates.len() {
      assert!(candidates[i - 1].score >= candidates[i].score);
    }
  }

  #[test]
  fn test_jira_issue_scoring() {
    let mut candidates = vec![
      create_test_candidate("abc123", 1, true, Some("PROJ-123".to_string())),
      create_test_candidate("def456", 1, true, Some("PROJ-456".to_string())),
      create_test_candidate("ghi789", 1, true, None),
    ];

    let args = FixupArgs {
      limit: 20,
      days: 30,
      all_authors: false,
      include_fixups: false,
      dry_run: false,
      vim_mode: false,
    };

    // Test with matching Jira issue
    let current_jira_issue = Some("PROJ-123".to_string());
    score_commits(&mut candidates, &args, current_jira_issue).unwrap();

    // The first candidate should have the highest score due to Jira match
    assert!(candidates[0].short_hash == "abc123");
    assert!(candidates[0].score > candidates[1].score);
    // candidates[1] and candidates[2] should have equal scores since they both
    // don't match the current Jira issue
    assert!((candidates[1].score - candidates[2].score).abs() < 0.001);

    // Test with no current Jira issue
    let mut candidates_no_jira = candidates.clone();
    score_commits(&mut candidates_no_jira, &args, None).unwrap();

    // Without Jira matching, scores should be equal for same recency/authorship
    assert!((candidates_no_jira[0].score - candidates_no_jira[1].score).abs() < 0.001);
  }

  #[test]
  fn test_different_jira_issue_injection() {
    let mut candidates = vec![
      create_test_candidate("abc123", 1, true, Some("PROJ-123".to_string())),
      create_test_candidate("def456", 1, true, Some("PROJ-456".to_string())),
    ];

    let args = FixupArgs {
      limit: 20,
      days: 30,
      all_authors: false,
      include_fixups: false,
      dry_run: false,
      vim_mode: false,
    };

    // Test with PROJ-456 as current issue
    let current_jira_issue = Some("PROJ-456".to_string());
    score_commits(&mut candidates, &args, current_jira_issue).unwrap();

    // The second candidate should now have higher score due to Jira match
    assert!(candidates[0].short_hash == "def456");
    assert!(candidates[0].score > candidates[1].score);
  }

  #[test]
  fn test_branch_uniqueness_hard_partition() {
    // Create candidates: one branch-unique, one not.
    // The branch-unique commit should ALWAYS score higher, even when every
    // other factor favors the non-unique commit (hard partition guarantee).
    let mut candidates = vec![
      // Branch-unique commit, but older and from different author, no Jira
      create_test_candidate_full("unique", 5, false, None, true),
      // Not branch-unique, but recent, current author, matching Jira
      create_test_candidate_full("shared", 1, true, Some("PROJ-123".to_string()), false),
    ];

    let args = FixupArgs {
      limit: 20,
      days: 30,
      all_authors: true,
      include_fixups: false,
      dry_run: false,
      vim_mode: false,
    };

    let current_jira_issue = Some("PROJ-123".to_string());
    score_commits(&mut candidates, &args, current_jira_issue).unwrap();

    // The branch-unique commit must rank first — hard partition means no
    // combination of secondary factors can overcome the tier bonus.
    assert_eq!(
      candidates[0].short_hash, "unique",
      "Branch-unique commits must always rank above shared commits"
    );
    assert!(
      candidates[0].score >= 10.0,
      "Branch-unique score {} should include tier bonus (>= 10.0)",
      candidates[0].score,
    );
    assert!(
      candidates[1].score < 10.0,
      "Shared commit score {} should be below tier threshold (< 10.0)",
      candidates[1].score,
    );
  }

  #[test]
  fn test_branch_uniqueness_tie_breaking() {
    // When both commits are branch-unique, other factors should break ties
    let mut candidates = vec![
      create_test_candidate_full("older", 5, true, None, true),
      create_test_candidate_full("newer", 1, true, None, true),
    ];

    let args = FixupArgs {
      limit: 20,
      days: 30,
      all_authors: true,
      include_fixups: false,
      dry_run: false,
      vim_mode: false,
    };

    score_commits(&mut candidates, &args, None).unwrap();

    // Both are branch-unique, so recency should determine order
    assert_eq!(
      candidates[0].short_hash, "newer",
      "More recent commit should rank first when both are branch-unique"
    );
  }
}

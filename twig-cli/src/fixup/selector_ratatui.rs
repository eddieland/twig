//! # Ratatui-based Interactive Commit Selection
//!
//! This module provides an interactive commit selector using ratatui for TUI
//! rendering and crossterm for cross-platform terminal handling. This
//! implementation replaces the skim-based selector to achieve Windows
//! compatibility.
//!
//! ## Phase 1 Implementation
//!
//! This is the Phase 1 implementation focusing on basic UI foundation:
//! - Simple layout with commit list (no search input yet)
//! - Basic commit display formatting
//! - Terminal setup and cleanup
//! - Arrow key navigation (up/down)
//! - Enter to select, Escape to cancel

use std::io;

use anyhow::Result;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::{Frame, Terminal};

use crate::fixup::commit_collector::CommitCandidate;

/// Navigation state for the commit selector
#[derive(Debug, Clone)]
pub struct SelectorState {
  candidates: Vec<CommitCandidate>,
  selected_index: usize,
}

impl SelectorState {
  /// Create a new selector state with the given candidates
  pub fn new(candidates: Vec<CommitCandidate>) -> Self {
    Self {
      candidates,
      selected_index: 0,
    }
  }

  /// Get the number of candidates
  #[allow(dead_code)]
  pub fn len(&self) -> usize {
    self.candidates.len()
  }

  /// Check if there are no candidates
  pub fn is_empty(&self) -> bool {
    self.candidates.is_empty()
  }

  /// Get the currently selected index
  pub fn selected_index(&self) -> usize {
    self.selected_index
  }

  /// Get the currently selected candidate, if any
  pub fn selected_candidate(&self) -> Option<&CommitCandidate> {
    self.candidates.get(self.selected_index)
  }

  /// Get all candidates
  pub fn candidates(&self) -> &[CommitCandidate] {
    &self.candidates
  }

  /// Move to the next item
  pub fn next(&mut self) {
    if !self.candidates.is_empty() {
      self.selected_index = (self.selected_index + 1) % self.candidates.len();
    }
  }

  /// Move to the previous item
  pub fn previous(&mut self) {
    if !self.candidates.is_empty() {
      if self.selected_index == 0 {
        self.selected_index = self.candidates.len() - 1;
      } else {
        self.selected_index -= 1;
      }
    }
  }

  /// Set the selected index (bounds-checked)
  #[allow(dead_code)]
  pub fn set_selected_index(&mut self, index: usize) {
    if index < self.candidates.len() {
      self.selected_index = index;
    }
  }

  /// Handle key input and return the action to take
  pub fn handle_key(&mut self, key_code: KeyCode, modifiers: KeyModifiers) -> SelectorAction {
    match (key_code, modifiers) {
      (KeyCode::Char('c'), KeyModifiers::CONTROL) => SelectorAction::Cancel,
      (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => SelectorAction::Cancel,
      (KeyCode::Enter, _) => {
        if let Some(candidate) = self.selected_candidate() {
          SelectorAction::Select(candidate.clone())
        } else {
          SelectorAction::Cancel
        }
      }
      (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
        self.next();
        SelectorAction::Continue
      }
      (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
        self.previous();
        SelectorAction::Continue
      }
      _ => SelectorAction::Continue,
    }
  }
}

/// Actions that can result from key input
#[derive(Debug, Clone)]
pub enum SelectorAction {
  /// Continue with the selection process
  Continue,
  /// Cancel the selection
  Cancel,
  /// Select the given commit
  Select(CommitCandidate),
}

/// Basic commit selector using ratatui
pub struct CommitSelector {
  state: SelectorState,
  list_state: ListState,
}

impl CommitSelector {
  /// Create a new commit selector with the given candidates
  pub fn new(candidates: Vec<CommitCandidate>) -> Self {
    let mut list_state = ListState::default();
    if !candidates.is_empty() {
      list_state.select(Some(0));
    }

    Self {
      state: SelectorState::new(candidates),
      list_state,
    }
  }

  /// Get a reference to the internal state (for testing)
  #[allow(dead_code)]
  pub fn state(&self) -> &SelectorState {
    &self.state
  }

  /// Run the interactive selector and return the selected commit
  pub fn run(mut self) -> Result<Option<CommitCandidate>> {
    if self.state.is_empty() {
      return Ok(None);
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = self.run_app(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
  }

  /// Main application loop
  fn run_app(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<Option<CommitCandidate>> {
    loop {
      terminal.draw(|f| self.ui(f))?;

      if let Event::Key(key) = event::read()? {
        if key.kind == KeyEventKind::Press {
          match self.state.handle_key(key.code, key.modifiers) {
            SelectorAction::Continue => {
              // Update the list state to match the internal state
              self.list_state.select(Some(self.state.selected_index()));
            }
            SelectorAction::Cancel => return Ok(None),
            SelectorAction::Select(candidate) => return Ok(Some(candidate)),
          }
        }
      }
    }
  }

  /// Render the UI
  fn ui(&mut self, f: &mut Frame) {
    let chunks = Layout::default()
      .direction(Direction::Vertical)
      .constraints([Constraint::Min(0)].as_ref())
      .split(f.area());

    // Create list items
    let items: Vec<ListItem> = self
      .state
      .candidates()
      .iter()
      .map(|candidate| {
        let display_text = format_candidate_for_display(candidate);
        ListItem::new(Line::from(vec![Span::raw(display_text)]))
      })
      .collect();

    // Create the list widget
    let list = List::new(items)
      .block(
        Block::default()
          .borders(Borders::ALL)
          .title("Select commit to fixup (↑/↓ to navigate, Enter to select, Esc/Ctrl+C to cancel)"),
      )
      .highlight_style(
        Style::default()
          .add_modifier(Modifier::BOLD)
          .bg(Color::Blue)
          .fg(Color::White),
      )
      .highlight_symbol("► ");

    f.render_stateful_widget(list, chunks[0], &mut self.list_state);
  }
}

/// Select a commit interactively using ratatui
pub fn select_commit_ratatui(candidates: &[CommitCandidate]) -> Result<Option<CommitCandidate>> {
  let selector = CommitSelector::new(candidates.to_vec());
  selector.run()
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
      hash: format!("full_hash_{short_hash}"),
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

  fn create_test_candidate_with_details(
    short_hash: &str,
    hours_ago: i64,
    message: &str,
    author: &str,
    is_current_user: bool,
    jira_issue: Option<String>,
  ) -> CommitCandidate {
    let now = Utc::now();
    let date = now - chrono::Duration::hours(hours_ago);

    CommitCandidate {
      hash: format!("full_hash_{short_hash}"),
      short_hash: short_hash.to_string(),
      message: message.to_string(),
      author: author.to_string(),
      date,
      is_current_user,
      jira_issue,
      score: 0.8,
    }
  }

  #[test]
  fn test_format_candidate_for_display_no_jira() {
    let candidate = create_test_candidate_with_details("def456", 3, "Another commit", "other_user", false, None);
    let formatted = format_candidate_for_display(&candidate);

    assert!(formatted.contains("def456"));
    assert!(formatted.contains("3h ago"));
    assert!(formatted.contains("Another commit"));
    assert!(formatted.contains("other_user"));
    assert!(formatted.contains(" ")); // No Jira indicator (space)
    assert!(formatted.contains("○")); // Other user indicator
  }

  // Tests for SelectorState
  #[test]
  fn test_selector_state_creation() {
    let candidates = vec![create_test_candidate("abc123", 1), create_test_candidate("def456", 2)];

    let state = SelectorState::new(candidates.clone());
    assert_eq!(state.len(), 2);
    assert!(!state.is_empty());
    assert_eq!(state.selected_index(), 0);
    assert_eq!(state.selected_candidate().unwrap().short_hash, "abc123");
  }

  #[test]
  fn test_selector_state_empty() {
    let state = SelectorState::new(vec![]);
    assert_eq!(state.len(), 0);
    assert!(state.is_empty());
    assert_eq!(state.selected_index(), 0);
    assert!(state.selected_candidate().is_none());
  }

  #[test]
  fn test_selector_state_navigation() {
    let candidates = vec![
      create_test_candidate("abc123", 1),
      create_test_candidate("def456", 2),
      create_test_candidate("ghi789", 3),
    ];

    let mut state = SelectorState::new(candidates);

    // Test initial state
    assert_eq!(state.selected_index(), 0);
    assert_eq!(state.selected_candidate().unwrap().short_hash, "abc123");

    // Test moving down
    state.next();
    assert_eq!(state.selected_index(), 1);
    assert_eq!(state.selected_candidate().unwrap().short_hash, "def456");

    state.next();
    assert_eq!(state.selected_index(), 2);
    assert_eq!(state.selected_candidate().unwrap().short_hash, "ghi789");

    // Test wrapping around
    state.next();
    assert_eq!(state.selected_index(), 0);
    assert_eq!(state.selected_candidate().unwrap().short_hash, "abc123");

    // Test moving up
    state.previous();
    assert_eq!(state.selected_index(), 2);
    assert_eq!(state.selected_candidate().unwrap().short_hash, "ghi789");

    state.previous();
    assert_eq!(state.selected_index(), 1);
    assert_eq!(state.selected_candidate().unwrap().short_hash, "def456");
  }

  #[test]
  fn test_selector_state_handle_key_navigation() {
    let candidates = vec![create_test_candidate("abc123", 1), create_test_candidate("def456", 2)];

    let mut state = SelectorState::new(candidates);

    // Test down navigation
    match state.handle_key(KeyCode::Down, KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue action"),
    }
    assert_eq!(state.selected_index(), 1);

    // Test up navigation
    match state.handle_key(KeyCode::Up, KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue action"),
    }
    assert_eq!(state.selected_index(), 0);

    // Test vim-style navigation
    match state.handle_key(KeyCode::Char('j'), KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue action"),
    }
    assert_eq!(state.selected_index(), 1);

    match state.handle_key(KeyCode::Char('k'), KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue action"),
    }
    assert_eq!(state.selected_index(), 0);
  }

  #[test]
  fn test_selector_state_handle_key_selection() {
    let candidates = vec![create_test_candidate("abc123", 1)];
    let mut state = SelectorState::new(candidates);

    // Test Enter key
    match state.handle_key(KeyCode::Enter, KeyModifiers::NONE) {
      SelectorAction::Select(candidate) => {
        assert_eq!(candidate.short_hash, "abc123");
      }
      _ => panic!("Expected Select action"),
    }
  }

  #[test]
  fn test_selector_state_handle_key_cancel() {
    let candidates = vec![create_test_candidate("abc123", 1)];
    let mut state = SelectorState::new(candidates);

    // Test Escape key
    match state.handle_key(KeyCode::Esc, KeyModifiers::NONE) {
      SelectorAction::Cancel => {}
      _ => panic!("Expected Cancel action"),
    }

    // Test 'q' key
    match state.handle_key(KeyCode::Char('q'), KeyModifiers::NONE) {
      SelectorAction::Cancel => {}
      _ => panic!("Expected Cancel action"),
    }

    // Test Ctrl+C key
    match state.handle_key(KeyCode::Char('c'), KeyModifiers::CONTROL) {
      SelectorAction::Cancel => {}
      _ => panic!("Expected Cancel action"),
    }
  }

  #[test]
  fn test_commit_selector_creation() {
    let candidates = vec![create_test_candidate("abc123", 1), create_test_candidate("def456", 2)];

    let selector = CommitSelector::new(candidates.clone());
    assert_eq!(selector.state().len(), 2);
    assert_eq!(selector.state().selected_index(), 0);
  }

  #[test]
  fn test_commit_selector_navigation() {
    let candidates = vec![create_test_candidate("abc123", 1)];
    let selector = CommitSelector::new(candidates);

    let state = selector.state();
    assert_eq!(state.len(), 1);
    assert_eq!(state.selected_candidate().unwrap().short_hash, "abc123");
  }
}

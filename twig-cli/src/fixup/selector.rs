//! # Interactive Commit Selection
//!
//! This module provides an interactive fuzzy finder interface for selecting
//! commit candidates using ratatui for TUI rendering and nucleo for fuzzy
//! matching. This implementation provides cross-platform compatibility
//! including Windows support.
//!
//! ## Functionality
//!
//! - **Fuzzy Search**: Full-text search across commit information using nucleo
//! - **Rich Display**: Shows commit hash, relative time, author, and indicators
//! - **Visual Indicators**: Uses symbols to indicate authorship and Jira
//!   association
//! - **Keyboard Navigation**: Standard fuzzy finder controls for selection
//! - **Cross-Platform**: Works on Windows, macOS, and Linux
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
//! - Type to filter commits by any visible text (fuzzy matching)
//! - Use arrow keys or j/k to navigate through results
//! - Press Enter to select a commit
//! - Press Escape to cancel selection
//! - Switch between search and navigation modes with Tab

use std::io;

use anyhow::Result;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use nucleo::{Config, Matcher, Utf32Str};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::{Frame, Terminal};

use crate::fixup::commit_collector::CommitCandidate;

/// Input mode for the selector
#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
  /// User is typing in the search input
  Search,
  /// User is navigating the commit list
  Navigation,
}

/// Navigation state for the commit selector
#[derive(Debug, Clone)]
pub struct SelectorState {
  all_candidates: Vec<CommitCandidate>,
  filtered_candidates: Vec<CommitCandidate>,
  filtered_indices: Vec<(usize, u16)>, // (index, score) pairs for fuzzy matching
  selected_index: usize,
  search_query: String,
  input_mode: InputMode,
  matcher: Matcher,
}

impl SelectorState {
  /// Create a new selector state with the given candidates
  pub fn new(candidates: Vec<CommitCandidate>) -> Self {
    let filtered_candidates = candidates.clone();
    let filtered_indices: Vec<(usize, u16)> = (0..candidates.len()).map(|i| (i, 0)).collect();
    let matcher = Matcher::new(Config::DEFAULT);
    Self {
      all_candidates: candidates,
      filtered_candidates,
      filtered_indices,
      selected_index: 0,
      search_query: String::new(),
      input_mode: InputMode::Search,
      matcher,
    }
  }

  /// Get the number of filtered candidates
  #[allow(dead_code)]
  pub fn len(&self) -> usize {
    self.filtered_candidates.len()
  }

  /// Check if there are no filtered candidates
  pub fn is_empty(&self) -> bool {
    self.filtered_candidates.is_empty()
  }

  /// Get the total number of all candidates (before filtering)
  pub fn total_len(&self) -> usize {
    self.all_candidates.len()
  }

  /// Get the currently selected index
  pub fn selected_index(&self) -> usize {
    self.selected_index
  }

  /// Get the currently selected candidate, if any
  pub fn selected_candidate(&self) -> Option<&CommitCandidate> {
    self.filtered_candidates.get(self.selected_index)
  }

  /// Get all filtered candidates
  pub fn candidates(&self) -> &[CommitCandidate] {
    &self.filtered_candidates
  }

  /// Get the current search query
  pub fn search_query(&self) -> &str {
    &self.search_query
  }

  /// Get the current input mode
  pub fn input_mode(&self) -> &InputMode {
    &self.input_mode
  }

  /// Set the input mode
  pub fn set_input_mode(&mut self, mode: InputMode) {
    self.input_mode = mode;
  }

  /// Move to the next item
  pub fn next(&mut self) {
    if !self.filtered_candidates.is_empty() {
      self.selected_index = (self.selected_index + 1) % self.filtered_candidates.len();
    }
  }

  /// Move to the previous item
  pub fn previous(&mut self) {
    if !self.filtered_candidates.is_empty() {
      if self.selected_index == 0 {
        self.selected_index = self.filtered_candidates.len() - 1;
      } else {
        self.selected_index -= 1;
      }
    }
  }

  /// Set the selected index (bounds-checked)
  #[allow(dead_code)]
  pub fn set_selected_index(&mut self, index: usize) {
    if index < self.filtered_candidates.len() {
      self.selected_index = index;
    }
  }

  /// Update the search query and filter candidates
  #[allow(dead_code)]
  pub fn update_search(&mut self, query: String) {
    self.search_query = query;
    self.filter_candidates();
    // Reset selection to first item after filtering
    self.selected_index = 0;
  }

  /// Clear the search query and show all candidates
  #[allow(dead_code)]
  pub fn clear_search(&mut self) {
    self.search_query.clear();
    self.filter_candidates();
    self.selected_index = 0;
  }

  /// Add a character to the search query
  pub fn push_char(&mut self, c: char) {
    self.search_query.push(c);
    self.filter_candidates();
    self.selected_index = 0;
  }

  /// Remove the last character from the search query
  pub fn pop_char(&mut self) {
    self.search_query.pop();
    self.filter_candidates();
    self.selected_index = 0;
  }

  /// Filter candidates based on the current search query using fuzzy matching
  fn filter_candidates(&mut self) {
    if self.search_query.is_empty() {
      self.filtered_candidates = self.all_candidates.clone();
      self.filtered_indices = (0..self.all_candidates.len()).map(|i| (i, 0)).collect();
    } else {
      // Perform fuzzy matching with nucleo (case-insensitive)
      let mut matches: Vec<(usize, u16)> = Vec::new();

      for (index, candidate) in self.all_candidates.iter().enumerate() {
        let display_text = format_candidate_for_display(candidate).to_lowercase();
        let search_query_lower = self.search_query.to_lowercase();
        let mut haystack_buf = Vec::new();
        let mut needle_buf = Vec::new();
        let haystack = Utf32Str::new(&display_text, &mut haystack_buf);
        let needle = Utf32Str::new(&search_query_lower, &mut needle_buf);

        if let Some(score) = self.matcher.fuzzy_match(haystack, needle) {
          matches.push((index, score));
        }
      }

      // Sort by score (higher is better)
      matches.sort_by(|a, b| b.1.cmp(&a.1));

      // Update filtered candidates and indices
      self.filtered_indices = matches;
      self.filtered_candidates = self
        .filtered_indices
        .iter()
        .map(|(index, _score)| self.all_candidates[*index].clone())
        .collect();
    }
  }

  /// Handle key input and return the action to take
  pub fn handle_key(&mut self, key_code: KeyCode, modifiers: KeyModifiers) -> SelectorAction {
    match (key_code, modifiers) {
      // Global shortcuts that work in any mode
      (KeyCode::Char('c'), KeyModifiers::CONTROL) => SelectorAction::Cancel,
      (KeyCode::Esc, _) => {
        match self.input_mode {
          InputMode::Search => {
            // In search mode, Esc switches to navigation mode
            self.set_input_mode(InputMode::Navigation);
            SelectorAction::Continue
          }
          InputMode::Navigation => {
            // In navigation mode, Esc cancels
            SelectorAction::Cancel
          }
        }
      }
      (KeyCode::Enter, _) => {
        match self.input_mode {
          InputMode::Search => {
            // In search mode, Enter switches to navigation mode
            self.set_input_mode(InputMode::Navigation);
            SelectorAction::Continue
          }
          InputMode::Navigation => {
            // In navigation mode, Enter selects the current candidate
            if let Some(candidate) = self.selected_candidate() {
              SelectorAction::Select(candidate.clone())
            } else {
              SelectorAction::Cancel
            }
          }
        }
      }
      // Mode-specific key handling
      _ => match self.input_mode {
        InputMode::Search => self.handle_search_key(key_code, modifiers),
        InputMode::Navigation => self.handle_navigation_key(key_code, modifiers),
      },
    }
  }

  /// Handle key input in search mode
  fn handle_search_key(&mut self, key_code: KeyCode, _modifiers: KeyModifiers) -> SelectorAction {
    match key_code {
      KeyCode::Char(c) => {
        self.push_char(c);
        SelectorAction::Continue
      }
      KeyCode::Backspace => {
        self.pop_char();
        SelectorAction::Continue
      }
      KeyCode::Tab => {
        // Tab switches to navigation mode
        self.set_input_mode(InputMode::Navigation);
        SelectorAction::Continue
      }
      _ => SelectorAction::Continue,
    }
  }

  /// Handle key input in navigation mode
  fn handle_navigation_key(&mut self, key_code: KeyCode, _modifiers: KeyModifiers) -> SelectorAction {
    match key_code {
      KeyCode::Char('q') => SelectorAction::Cancel,
      KeyCode::Char('/') => {
        // '/' switches to search mode
        self.set_input_mode(InputMode::Search);
        SelectorAction::Continue
      }
      KeyCode::Down | KeyCode::Char('j') => {
        self.next();
        SelectorAction::Continue
      }
      KeyCode::Up | KeyCode::Char('k') => {
        self.previous();
        SelectorAction::Continue
      }
      KeyCode::Tab => {
        // Tab switches to search mode
        self.set_input_mode(InputMode::Search);
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
    // Split the layout into search input and results list
    let chunks = Layout::default()
      .direction(Direction::Vertical)
      .constraints([
        Constraint::Length(3), // Search input
        Constraint::Min(0),    // Results list
      ])
      .split(f.area());

    // Render search input
    self.render_search_input(f, chunks[0]);

    // Render results list
    self.render_results_list(f, chunks[1]);
  }

  /// Render the search input field
  fn render_search_input(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
    let search_style = match self.state.input_mode() {
      InputMode::Search => Style::default().fg(Color::Yellow),
      InputMode::Navigation => Style::default().fg(Color::Gray),
    };

    let search_block = Block::default()
      .borders(Borders::ALL)
      .title("Search (Tab/Enter to switch modes, / to focus search)")
      .border_style(search_style);

    let search_text = self.state.search_query().to_string();
    let search_paragraph = Paragraph::new(search_text).block(search_block).style(search_style);

    f.render_widget(search_paragraph, area);

    // Show cursor in search mode
    if *self.state.input_mode() == InputMode::Search {
      // Calculate cursor position
      let cursor_x = area.x + self.state.search_query().len() as u16 + 1;
      let cursor_y = area.y + 1;
      f.set_cursor_position((cursor_x, cursor_y));
    }
  }

  /// Render the results list
  fn render_results_list(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
    // Create list items from filtered candidates
    let items: Vec<ListItem> = self
      .state
      .candidates()
      .iter()
      .map(|candidate| {
        let display_text = format_candidate_for_display(candidate);
        ListItem::new(Line::from(vec![Span::raw(display_text)]))
      })
      .collect();

    let list_style = match self.state.input_mode() {
      InputMode::Navigation => Style::default(),
      InputMode::Search => Style::default().fg(Color::Gray),
    };

    let highlight_style = match self.state.input_mode() {
      InputMode::Navigation => Style::default()
        .add_modifier(Modifier::BOLD)
        .bg(Color::Blue)
        .fg(Color::White),
      InputMode::Search => Style::default()
        .add_modifier(Modifier::BOLD)
        .bg(Color::DarkGray)
        .fg(Color::White),
    };

    // Create title with result count
    let result_count = self.state.candidates().len();
    let total_count = self.state.total_len();
    let title = if self.state.search_query().is_empty() {
      format!("Commits ({result_count} total) - ↑/↓ j/k to navigate, Enter to select, Esc to cancel")
    } else {
      format!(
        "Filtered Commits ({result_count} of {total_count} total) - ↑/↓ j/k to navigate, Enter to select, Esc to clear search"
      )
    };

    // Create the list widget
    let list = List::new(items)
      .block(
        Block::default()
          .borders(Borders::ALL)
          .title(title)
          .border_style(list_style),
      )
      .style(list_style)
      .highlight_style(highlight_style)
      .highlight_symbol("► ");

    f.render_stateful_widget(list, area, &mut self.list_state);
  }
}

/// Select a commit interactively using ratatui
pub fn select_commit(candidates: &[CommitCandidate]) -> Result<Option<CommitCandidate>> {
  if candidates.is_empty() {
    return Ok(None);
  }

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
    is_current_user: bool,
    jira_issue: Option<String>,
  ) -> CommitCandidate {
    let now = Utc::now();
    let date = now - chrono::Duration::hours(hours_ago);

    CommitCandidate {
      hash: format!("full_hash_{short_hash}"),
      short_hash: short_hash.to_string(),
      message: "Test commit message".to_string(),
      author: "test_user".to_string(),
      date,
      is_current_user,
      jira_issue,
      score: 0.8,
    }
  }

  #[test]
  fn test_format_candidate_for_display_no_jira() {
    let candidate = create_test_candidate_with_details("abc123", 2, false, None);
    let formatted = format_candidate_for_display(&candidate);

    assert!(formatted.contains("abc123"));
    assert!(formatted.contains("○")); // Other user indicator
    assert!(!formatted.contains("🎫")); // No Jira indicator
  }

  #[test]
  fn test_selector_state_creation() {
    let candidates = vec![create_test_candidate("abc123", 2)];
    let state = SelectorState::new(candidates);

    assert_eq!(state.len(), 1);
    assert_eq!(state.selected_index(), 0);
    assert!(!state.is_empty());
  }

  #[test]
  fn test_selector_state_empty() {
    let candidates = vec![];
    let state = SelectorState::new(candidates);

    assert_eq!(state.len(), 0);
    assert!(state.is_empty());
  }

  #[test]
  fn test_selector_state_navigation() {
    let candidates = vec![
      create_test_candidate("abc123", 2),
      create_test_candidate("def456", 3),
      create_test_candidate("ghi789", 4),
    ];
    let mut state = SelectorState::new(candidates);

    // Test next navigation
    assert_eq!(state.selected_index(), 0);
    state.next();
    assert_eq!(state.selected_index(), 1);
    state.next();
    assert_eq!(state.selected_index(), 2);
    state.next(); // Should wrap around
    assert_eq!(state.selected_index(), 0);

    // Test previous navigation
    state.previous(); // Should wrap to last
    assert_eq!(state.selected_index(), 2);
    state.previous();
    assert_eq!(state.selected_index(), 1);
    state.previous();
    assert_eq!(state.selected_index(), 0);
  }

  #[test]
  fn test_selector_state_handle_key_navigation() {
    let candidates = vec![create_test_candidate("abc123", 2), create_test_candidate("def456", 3)];
    let mut state = SelectorState::new(candidates);
    state.set_input_mode(InputMode::Navigation);

    // Test down key
    match state.handle_key(KeyCode::Down, KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue"),
    }
    assert_eq!(state.selected_index(), 1);

    // Test up key
    match state.handle_key(KeyCode::Up, KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue"),
    }
    assert_eq!(state.selected_index(), 0);

    // Test j key (vim-style down)
    match state.handle_key(KeyCode::Char('j'), KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue"),
    }
    assert_eq!(state.selected_index(), 1);

    // Test k key (vim-style up)
    match state.handle_key(KeyCode::Char('k'), KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue"),
    }
    assert_eq!(state.selected_index(), 0);
  }

  #[test]
  fn test_selector_state_handle_key_selection() {
    let candidates = vec![create_test_candidate("abc123", 2)];
    let mut state = SelectorState::new(candidates);
    state.set_input_mode(InputMode::Navigation);

    // Test Enter key for selection
    match state.handle_key(KeyCode::Enter, KeyModifiers::NONE) {
      SelectorAction::Select(candidate) => {
        assert_eq!(candidate.short_hash, "abc123");
      }
      _ => panic!("Expected Select"),
    }
  }

  #[test]
  fn test_selector_state_handle_key_cancel() {
    let candidates = vec![create_test_candidate("abc123", 2)];
    let mut state = SelectorState::new(candidates);

    // Test Esc key in navigation mode
    state.set_input_mode(InputMode::Navigation);
    match state.handle_key(KeyCode::Esc, KeyModifiers::NONE) {
      SelectorAction::Cancel => {}
      _ => panic!("Expected Cancel"),
    }

    // Test q key
    match state.handle_key(KeyCode::Char('q'), KeyModifiers::NONE) {
      SelectorAction::Cancel => {}
      _ => panic!("Expected Cancel"),
    }

    // Test Ctrl+C
    match state.handle_key(KeyCode::Char('c'), KeyModifiers::CONTROL) {
      SelectorAction::Cancel => {}
      _ => panic!("Expected Cancel"),
    }
  }

  #[test]
  fn test_commit_selector_creation() {
    let candidates = vec![create_test_candidate("abc123", 2)];
    let selector = CommitSelector::new(candidates);

    assert_eq!(selector.state().len(), 1);
    assert_eq!(selector.state().selected_index(), 0);
  }

  #[test]
  fn test_commit_selector_navigation() {
    let candidates = vec![create_test_candidate("abc123", 2), create_test_candidate("def456", 3)];
    let selector = CommitSelector::new(candidates);

    // Test that list state is properly initialized
    assert_eq!(selector.list_state.selected(), Some(0));
  }

  #[test]
  fn test_search_functionality() {
    let candidates = vec![
      create_test_candidate_with_details("abc123", 2, true, Some("PROJ-123".to_string())),
      create_test_candidate_with_details("def456", 3, false, None),
    ];
    let mut state = SelectorState::new(candidates);

    // Initially all candidates should be visible
    assert_eq!(state.len(), 2);

    // Search for "abc" - should match first candidate
    state.push_char('a');
    state.push_char('b');
    state.push_char('c');
    assert_eq!(state.len(), 1);
    assert_eq!(state.candidates()[0].short_hash, "abc123");

    // Clear search
    state.pop_char();
    state.pop_char();
    state.pop_char();
    assert_eq!(state.len(), 2);
  }

  #[test]
  fn test_input_mode_switching() {
    let candidates = vec![create_test_candidate("abc123", 2)];
    let mut state = SelectorState::new(candidates);

    // Should start in Search mode
    assert_eq!(*state.input_mode(), InputMode::Search);

    // Tab should switch to Navigation mode
    match state.handle_key(KeyCode::Tab, KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue"),
    }
    assert_eq!(*state.input_mode(), InputMode::Navigation);

    // Tab should switch back to Search mode
    match state.handle_key(KeyCode::Tab, KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue"),
    }
    assert_eq!(*state.input_mode(), InputMode::Search);
  }

  #[test]
  fn test_search_key_handling() {
    let candidates = vec![create_test_candidate("abc123", 2)];
    let mut state = SelectorState::new(candidates);
    state.set_input_mode(InputMode::Search);

    // Test character input
    match state.handle_key(KeyCode::Char('t'), KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue"),
    }
    assert_eq!(state.search_query(), "t");

    // Test backspace
    match state.handle_key(KeyCode::Backspace, KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue"),
    }
    assert_eq!(state.search_query(), "");

    // Test Tab to switch modes
    match state.handle_key(KeyCode::Tab, KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue"),
    }
    assert_eq!(*state.input_mode(), InputMode::Navigation);
  }

  #[test]
  fn test_navigation_key_handling() {
    let candidates = vec![create_test_candidate("abc123", 2), create_test_candidate("def456", 3)];
    let mut state = SelectorState::new(candidates);
    state.set_input_mode(InputMode::Navigation);

    // Test down key
    match state.handle_key(KeyCode::Down, KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue"),
    }
    assert_eq!(state.selected_index(), 1);

    // Test '/' to switch to search mode
    match state.handle_key(KeyCode::Char('/'), KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue"),
    }
    assert_eq!(*state.input_mode(), InputMode::Search);

    // Test Tab to switch to search mode
    state.set_input_mode(InputMode::Navigation);
    match state.handle_key(KeyCode::Tab, KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue"),
    }
    assert_eq!(*state.input_mode(), InputMode::Search);
  }

  #[test]
  fn test_enter_and_escape_behavior() {
    let candidates = vec![create_test_candidate("abc123", 2)];
    let mut state = SelectorState::new(candidates);

    // In search mode, Enter should switch to navigation mode
    state.set_input_mode(InputMode::Search);
    match state.handle_key(KeyCode::Enter, KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue"),
    }
    assert_eq!(*state.input_mode(), InputMode::Navigation);

    // In navigation mode, Enter should select
    match state.handle_key(KeyCode::Enter, KeyModifiers::NONE) {
      SelectorAction::Select(_) => {}
      _ => panic!("Expected Select"),
    }

    // In search mode, Esc should switch to navigation mode
    state.set_input_mode(InputMode::Search);
    match state.handle_key(KeyCode::Esc, KeyModifiers::NONE) {
      SelectorAction::Continue => {}
      _ => panic!("Expected Continue"),
    }
    assert_eq!(*state.input_mode(), InputMode::Navigation);

    // In navigation mode, Esc should cancel
    match state.handle_key(KeyCode::Esc, KeyModifiers::NONE) {
      SelectorAction::Cancel => {}
      _ => panic!("Expected Cancel"),
    }
  }

  #[test]
  fn test_fuzzy_matching() {
    let candidates = vec![
      create_test_candidate_with_details("abc123", 2, true, Some("PROJ-123".to_string())),
      create_test_candidate_with_details("def456", 3, false, None),
      create_test_candidate_with_details("xyz789", 4, true, Some("TASK-456".to_string())),
    ];
    let mut state = SelectorState::new(candidates);

    // Test fuzzy matching with "abc" - should match first candidate
    state.push_char('a');
    state.push_char('b');
    state.push_char('c');
    assert_eq!(state.len(), 1);
    assert_eq!(state.candidates()[0].short_hash, "abc123");

    // Test case-insensitive matching with "ABC"
    state.pop_char();
    state.pop_char();
    state.pop_char();
    state.push_char('A');
    state.push_char('B');
    state.push_char('C');
    assert_eq!(state.len(), 1);
    assert_eq!(state.candidates()[0].short_hash, "abc123");

    // Test partial matching with "45" - should match def456
    state.pop_char();
    state.pop_char();
    state.pop_char();
    state.push_char('4');
    state.push_char('5');
    assert_eq!(state.len(), 1);
    assert_eq!(state.candidates()[0].short_hash, "def456");
  }
}

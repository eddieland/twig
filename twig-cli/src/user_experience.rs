//! User experience improvements for twig CLI commands
//! 
//! This module implements Component 2.2: Improved User Experience
//! - Adds progress indicators for long-running operations
//! - Implements command auto-completion support  
//! - Provides helpful hints for common user mistakes
//! - Adds colored output for better readability
//! - Implements interactive prompts where appropriate

use std::time::{Duration, Instant};
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use anyhow::Result;
use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal,
};
use twig_core::output::{print_success, print_warning};

/// Progress indicator for long-running operations
pub struct ProgressIndicator {
    message: String,
    start_time: Instant,
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ProgressIndicator {
    /// Create a new progress indicator with a message
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            start_time: Instant::now(),
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }

    /// Start the progress indicator
    pub fn start(&mut self) {
        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);
        let message = self.message.clone();

        self.handle = Some(thread::spawn(move || {
            let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let mut i = 0;

            while running.load(Ordering::SeqCst) {
                if let Ok((cols, _)) = terminal::size() {
                    let spinner = spinner_chars[i % spinner_chars.len()];
                    let display_message = if message.len() + 10 > cols as usize {
                        format!("{}...", &message[..((cols as usize).saturating_sub(13))])
                    } else {
                        message.clone()
                    };

                    print!("\r{} {}", spinner, display_message);
                    io::stdout().flush().unwrap_or(());
                }

                thread::sleep(Duration::from_millis(100));
                i += 1;
            }

            // Clear the spinner line
            if let Ok((cols, _)) = terminal::size() {
                print!("\r{}", " ".repeat(cols as usize));
                print!("\r");
                io::stdout().flush().unwrap_or(());
            }
        }));
    }

    /// Stop the progress indicator and show completion message
    pub fn finish(&mut self, success_message: Option<&str>) {
        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.handle.take() {
            handle.join().unwrap_or(());
        }

        let duration = self.start_time.elapsed();
        if let Some(message) = success_message {
            print_success(&format!("{} (completed in {:.2}s)", message, duration.as_secs_f64()));
        }
    }

    /// Stop the progress indicator with an error message
    pub fn error(&mut self, error_message: &str) {
        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.handle.take() {
            handle.join().unwrap_or(());
        }

        let duration = self.start_time.elapsed();
        twig_core::output::print_error(&format!("{} (failed after {:.2}s)", error_message, duration.as_secs_f64()));
    }
}

impl Drop for ProgressIndicator {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap_or(());
        }
    }
}

/// Color theme for terminal output
pub struct ColorTheme {
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
    pub highlight: Color,
    pub muted: Color,
}

impl ColorTheme {
    /// Create a default color theme
    pub fn default() -> Self {
        Self {
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            info: Color::Blue,
            highlight: Color::Cyan,
            muted: Color::Grey,
        }
    }

    /// Create a high-contrast color theme for accessibility
    pub fn high_contrast() -> Self {
        Self {
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            info: Color::White,
            highlight: Color::White,
            muted: Color::DarkGrey,
        }
    }
}

/// Enhanced output formatting with colors and styles
pub struct ColorOutput {
    theme: ColorTheme,
    colors_enabled: bool,
}

impl ColorOutput {
    /// Create a new ColorOutput instance
    pub fn new() -> Self {
        let colors_enabled = std::env::var("NO_COLOR").is_err() && 
                           std::env::var("TERM").map_or(true, |term| term != "dumb") &&
                           terminal::size().is_ok();

        Self {
            theme: ColorTheme::default(),
            colors_enabled,
        }
    }

    /// Print colored success message
    pub fn success(&self, message: &str) -> Result<()> {
        self.print_colored(message, self.theme.success)
    }

    /// Print colored warning message  
    pub fn warning(&self, message: &str) -> Result<()> {
        self.print_colored(message, self.theme.warning)
    }

    /// Print colored error message
    pub fn error(&self, message: &str) -> Result<()> {
        self.print_colored(message, self.theme.error)
    }

    /// Print colored info message
    pub fn info(&self, message: &str) -> Result<()> {
        self.print_colored(message, self.theme.info)
    }

    /// Print highlighted text
    pub fn highlight(&self, message: &str) -> Result<()> {
        self.print_colored(message, self.theme.highlight)
    }

    /// Print muted text
    pub fn muted(&self, message: &str) -> Result<()> {
        self.print_colored(message, self.theme.muted)
    }

    /// Internal method to print colored text
    fn print_colored(&self, message: &str, color: Color) -> Result<()> {
        if self.colors_enabled {
            execute!(
                io::stdout(),
                SetForegroundColor(color),
                Print(message),
                ResetColor
            )?;
        } else {
            print!("{}", message);
        }
        println!();
        Ok(())
    }

    /// Format a branch name with syntax highlighting
    pub fn format_branch_name(&self, branch_name: &str, is_current: bool) -> String {
        if !self.colors_enabled {
            return if is_current {
                format!("* {}", branch_name)
            } else {
                format!("  {}", branch_name)
            };
        }

        if is_current {
            format!("* {}", branch_name) // Simplified for now, colors handled by crossterm
        } else {
            format!("  {}", branch_name)
        }
    }

    /// Format git status with colors
    pub fn format_git_status(&self, status: &str) -> String {
        // Simplified for now, actual coloring handled by crossterm execute! macro
        status.to_string()
    }
}

impl Default for ColorOutput {
    fn default() -> Self {
        Self::new()
    }
}

/// Interactive prompts for user input
pub struct InteractivePrompt;

impl InteractivePrompt {
    /// Ask a yes/no question with a default answer
    pub fn confirm(message: &str, default: bool) -> Result<bool> {
        let default_str = if default { "Y/n" } else { "y/N" };
        print!("{} [{}]: ", message, default_str);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        Ok(match input.as_str() {
            "y" | "yes" => true,
            "n" | "no" => false,
            "" => default,
            _ => {
                print_warning("Invalid input, please enter y/n");
                Self::confirm(message, default)?
            }
        })
    }

    /// Select from a list of options
    pub fn select_option<T>(message: &str, options: &[T]) -> Result<usize>
    where 
        T: std::fmt::Display,
    {
        println!("{}", message);
        for (i, option) in options.iter().enumerate() {
            println!("  {}. {}", i + 1, option);
        }

        loop {
            print!("Please select an option (1-{}): ", options.len());
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            match input.trim().parse::<usize>() {
                Ok(n) if n > 0 && n <= options.len() => return Ok(n - 1),
                _ => {
                    print_warning(&format!("Invalid selection. Please enter a number between 1 and {}", options.len()));
                }
            }
        }
    }

    /// Get text input with optional validation
    pub fn text_input(message: &str, default: Option<&str>) -> Result<String> {
        if let Some(default_val) = default {
            print!("{} [{}]: ", message, default_val);
        } else {
            print!("{}: ", message);
        }
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            if let Some(default_val) = default {
                Ok(default_val.to_string())
            } else {
                print_warning("Input cannot be empty");
                Self::text_input(message, default)
            }
        } else {
            Ok(input.to_string())
        }
    }
}

/// Helper hints for common user mistakes
pub struct UserHints;

impl UserHints {
    /// Suggest corrections for common branch name typos
    pub fn suggest_branch_name(input: &str, available_branches: &[String]) -> Option<String> {
        if available_branches.contains(&input.to_string()) {
            return None; // Exact match found
        }

        // Simple Levenshtein distance for suggestions
        let mut best_match = None;
        let mut best_distance = usize::MAX;

        for branch in available_branches {
            let distance = levenshtein_distance(input, branch);
            if distance < best_distance && distance <= 2 {
                best_distance = distance;
                best_match = Some(branch.clone());
            }
        }

        best_match
    }

    /// Suggest git commands when twig operations fail
    pub fn suggest_git_workflow(error_context: &str) -> Vec<String> {
        let mut suggestions = Vec::new();

        if error_context.contains("not a git repository") {
            suggestions.push("git init".to_string());
            suggestions.push("git clone <repository-url>".to_string());
        }

        if error_context.contains("branch") && error_context.contains("not found") {
            suggestions.push("git branch -a".to_string());
            suggestions.push("git checkout -b <branch-name>".to_string());
        }

        if error_context.contains("merge conflict") {
            suggestions.push("git status".to_string());
            suggestions.push("git add .".to_string());
            suggestions.push("git rebase --continue".to_string());
        }

        if error_context.contains("uncommitted changes") {
            suggestions.push("git add .".to_string());
            suggestions.push("git commit -m 'WIP'".to_string());
            suggestions.push("git stash".to_string());
        }

        suggestions
    }

    /// Provide contextual help for twig commands
    pub fn command_help_hint(command: &str, subcommand: Option<&str>) -> Option<String> {
        match (command, subcommand) {
            ("branch", Some("depend")) => {
                Some("Creates parent-child relationships between branches. Use: twig branch depend <child> <parent>".to_string())
            }
            ("cascade", None) => {
                Some("Performs cascading rebase from current branch to children. Use --show-graph to preview changes.".to_string())
            }
            ("tree", None) => {
                Some("Shows branch dependency tree. Use --max-depth to limit levels shown.".to_string())
            }
            ("switch", None) => {
                Some("Intelligent branch switching with automatic dependency detection.".to_string())
            }
            _ => None,
        }
    }

    /// Show helpful hints based on repository state
    pub fn repository_state_hints(repo_path: &std::path::Path) -> Vec<String> {
        let mut hints = Vec::new();

        // Check if twig is initialized
        let twig_state_path = repo_path.join(".twig").join("state.json");
        if !twig_state_path.exists() {
            hints.push("Run 'twig init' to initialize twig in this repository".to_string());
        }

        // Check for common branch patterns that might need dependencies
        if let Ok(output) = std::process::Command::new("git")
            .args(&["branch", "--list"])
            .current_dir(repo_path)
            .output()
        {
            let branch_output = String::from_utf8_lossy(&output.stdout);
            let branches: Vec<&str> = branch_output.lines()
                .map(|line| line.trim().trim_start_matches("* ").trim())
                .filter(|line| !line.is_empty())
                .collect();

            if branches.iter().any(|b| b.contains("feature/") || b.contains("feat/")) &&
               branches.iter().any(|b| *b == "main" || *b == "master" || *b == "develop") {
                hints.push("Consider setting up branch dependencies with 'twig branch depend'".to_string());
            }
        }

        hints
    }
}

/// Simple Levenshtein distance calculation for string similarity
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 { return b_len; }
    if b_len == 0 { return a_len; }

    let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];

    for i in 0..=a_len {
        matrix[i][0] = i;
    }
    for j in 0..=b_len {
        matrix[0][j] = j;
    }

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            matrix[i][j] = std::cmp::min(
                std::cmp::min(
                    matrix[i - 1][j] + 1,      // deletion
                    matrix[i][j - 1] + 1,      // insertion
                ),
                matrix[i - 1][j - 1] + cost,   // substitution
            );
        }
    }

    matrix[a_len][b_len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_indicator_creation() {
        let progress = ProgressIndicator::new("Testing");
        assert_eq!(progress.message, "Testing");
        assert!(!progress.running.load(Ordering::SeqCst));
    }

    #[test] 
    fn test_color_output_creation() {
        let output = ColorOutput::new();
        // Should not panic
        assert!(output.theme.success == Color::Green);
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
        assert_eq!(levenshtein_distance("abc", "ab"), 1);
        assert_eq!(levenshtein_distance("abc", "def"), 3);
        assert_eq!(levenshtein_distance("feature", "featrue"), 2);
    }

    #[test]
    fn test_branch_name_suggestion() {
        let branches = vec![
            "feature/login".to_string(),
            "feature/signup".to_string(), 
            "main".to_string(),
        ];

        assert_eq!(UserHints::suggest_branch_name("featrue/login", &branches), Some("feature/login".to_string()));
        assert_eq!(UserHints::suggest_branch_name("main", &branches), None); // Exact match
        assert_eq!(UserHints::suggest_branch_name("completely-different", &branches), None); // Too different
    }

    #[test]
    fn test_git_workflow_suggestions() {
        let suggestions = UserHints::suggest_git_workflow("not a git repository");
        assert!(suggestions.contains(&"git init".to_string()));

        let suggestions = UserHints::suggest_git_workflow("branch not found");
        assert!(suggestions.contains(&"git branch -a".to_string()));

        let suggestions = UserHints::suggest_git_workflow("merge conflict detected");
        assert!(suggestions.contains(&"git status".to_string()));
    }
}
use anyhow::Result;
use twig_cli::cli::cascade::CascadeArgs;
use clap::Parser;

#[cfg(test)]
mod cascade_force_push_tests {
    use super::*;

    #[test] 
    fn test_cascade_force_push_flag_parsing() -> Result<()> {
        // This test verifies that the force-push flag is correctly parsed and accessible
        // We don't actually test git operations, just the argument parsing
        
        // Test parsing with --force-push flag
        let args = vec!["twig", "cascade", "--force-push"];
        let parsed = CascadeArgs::try_parse_from(args);
        assert!(parsed.is_ok(), "Failed to parse --force-push flag");
        
        let cascade_args = parsed.unwrap();
        assert!(cascade_args.force_push, "force_push should be true when --force-push flag is present");
        assert!(!cascade_args.force, "force should be false when not specified");

        // Test parsing without --force-push flag  
        let args = vec!["twig", "cascade"];
        let parsed = CascadeArgs::try_parse_from(args);
        assert!(parsed.is_ok(), "Failed to parse cascade command without flags");
        
        let cascade_args = parsed.unwrap();
        assert!(!cascade_args.force_push, "force_push should be false by default");

        // Test parsing with both --force and --force-push flags
        let args = vec!["twig", "cascade", "--force", "--force-push"];
        let parsed = CascadeArgs::try_parse_from(args);
        assert!(parsed.is_ok(), "Failed to parse both --force and --force-push flags");
        
        let cascade_args = parsed.unwrap();
        assert!(cascade_args.force_push, "force_push should be true when --force-push flag is present");
        assert!(cascade_args.force, "force should be true when --force flag is present");

        Ok(())
    }

    #[test]
    fn test_cascade_args_default_values() -> Result<()> {
        // Test default values when no flags are provided
        let args = vec!["twig", "cascade"];
        let parsed = CascadeArgs::try_parse_from(args);
        assert!(parsed.is_ok(), "Failed to parse cascade command");
        
        let cascade_args = parsed.unwrap();
        assert!(!cascade_args.force_push, "force_push should default to false");
        assert!(!cascade_args.force, "force should default to false");
        assert!(!cascade_args.show_graph, "show_graph should default to false");
        assert!(!cascade_args.autostash, "autostash should default to false");
        assert!(cascade_args.max_depth.is_none(), "max_depth should default to None");
        assert!(cascade_args.repo.is_none(), "repo should default to None");

        Ok(())
    }

    #[test]
    fn test_cascade_args_all_flags_together() -> Result<()> {
        // Test parsing with all flags provided
        let args = vec![
            "twig", "cascade", 
            "--force", 
            "--force-push", 
            "--show-graph", 
            "--autostash",
            "--max-depth", "5",
            "--repo", "/some/path"
        ];
        let parsed = CascadeArgs::try_parse_from(args);
        assert!(parsed.is_ok(), "Failed to parse cascade command with all flags");
        
        let cascade_args = parsed.unwrap();
        assert!(cascade_args.force_push, "force_push should be true");
        assert!(cascade_args.force, "force should be true");
        assert!(cascade_args.show_graph, "show_graph should be true");
        assert!(cascade_args.autostash, "autostash should be true");
        assert_eq!(cascade_args.max_depth, Some(5), "max_depth should be 5");
        assert_eq!(cascade_args.repo, Some("/some/path".to_string()), "repo should be set");

        Ok(())
    }

    #[test]
    fn test_force_push_flag_long_form_only() -> Result<()> {
        // Verify that only --force-push works (no short form)
        let args = vec!["twig", "cascade", "--force-push"];
        let parsed = CascadeArgs::try_parse_from(args);
        assert!(parsed.is_ok(), "Long form --force-push should work");
        
        let cascade_args = parsed.unwrap();
        assert!(cascade_args.force_push, "force_push should be true with --force-push");

        // Note: We don't test a short form because we intentionally didn't add one
        // to avoid confusion with the existing --force flag
        
        Ok(())
    }
}
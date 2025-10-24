//! Resource providers for twig state

use crate::protocol::{ListResourcesResult, ReadResourceParams, Resource, ResourceContents};
use anyhow::Result;
use serde_json::json;
use std::path::PathBuf;
use twig_core::config::ConfigDirs;
use twig_core::state::{Registry, RepoState};

/// Get all available resources
pub fn list_resources() -> Result<ListResourcesResult> {
    let mut resources = vec![
        Resource {
            uri: "twig://registry".to_string(),
            name: "Twig Registry".to_string(),
            description: Some("Global registry of all twig-managed repositories".to_string()),
            mime_type: Some("application/json".to_string()),
        },
    ];
    
    // Add resources for each registered repo
    let config_dirs = ConfigDirs::new()?;
    if let Ok(registry) = Registry::load(&config_dirs) {
        for repo in registry.list() {
            resources.push(Resource {
                uri: format!("twig://repo/{}/state", repo.name),
                name: format!("{} - Branch State", repo.name),
                description: Some(format!("Branch metadata and state for {}", repo.name)),
                mime_type: Some("application/json".to_string()),
            });
            
            resources.push(Resource {
                uri: format!("twig://repo/{}/tree", repo.name),
                name: format!("{} - Dependency Tree", repo.name),
                description: Some(format!("Branch dependency tree for {}", repo.name)),
                mime_type: Some("application/json".to_string()),
            });
        }
    }
    
    Ok(ListResourcesResult { resources })
}

/// Read a specific resource
pub async fn read_resource(params: ReadResourceParams) -> Result<ResourceContents> {
    let uri = params.uri;
    let config_dirs = ConfigDirs::new()?;
    
    if uri == "twig://registry" {
        let registry = Registry::load(&config_dirs)?;
        let content = serde_json::to_string_pretty(&registry.list())?;
        
        return Ok(ResourceContents {
            uri,
            mime_type: Some("application/json".to_string()),
            text: Some(content),
            blob: None,
        });
    }
    
    // Parse repo-specific URIs
    if let Some(rest) = uri.strip_prefix("twig://repo/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 {
            let repo_name = parts[0];
            let resource_type = parts[1];
            
            let registry = Registry::load(&config_dirs)?;
            let repo = registry.list().iter()
                .find(|r| r.name == repo_name)
                .ok_or_else(|| anyhow::anyhow!("Repository {} not found in registry", repo_name))?;
            let repo_path = PathBuf::from(&repo.path);
            
            match resource_type {
                "state" => {
                    let state = RepoState::load(&repo_path)?;
                    let content = serde_json::to_string_pretty(&state)?;
                    
                    return Ok(ResourceContents {
                        uri,
                        mime_type: Some("application/json".to_string()),
                        text: Some(content),
                        blob: None,
                    });
                }
                "tree" => {
                    let state = RepoState::load(&repo_path)?;
                    let tree_data = json!({
                        "branches": state.branches,
                        "dependencies": state.dependencies,
                    });
                    let content = serde_json::to_string_pretty(&tree_data)?;
                    
                    return Ok(ResourceContents {
                        uri,
                        mime_type: Some("application/json".to_string()),
                        text: Some(content),
                        blob: None,
                    });
                }
                _ => {}
            }
        }
    }
    
    Err(anyhow::anyhow!("Unknown resource URI: {}", uri))
}

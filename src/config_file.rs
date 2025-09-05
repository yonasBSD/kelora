#![allow(dead_code)]
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

/// Configuration file handler for kelora
#[derive(Default)]
pub struct ConfigFile {
    pub defaults: Option<String>,
    pub aliases: HashMap<String, String>,
}

impl ConfigFile {
    /// Find project-level .kelorarc by walking up directory tree
    pub fn find_project_config() -> Option<PathBuf> {
        let mut current = std::env::current_dir().ok()?;
        loop {
            let config_path = current.join(".kelorarc");
            if config_path.exists() {
                return Some(config_path);
            }
            if !current.pop() {
                // Reached filesystem root
                break;
            }
        }
        None
    }

    /// Get list of user config file locations in order of preference
    pub fn get_user_config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if cfg!(windows) {
            // Windows paths in order of preference:
            // 1. %APPDATA%\kelora\config.ini
            // 2. %USERPROFILE%\.kelorarc (legacy/compatibility)
            if let Ok(appdata) = env::var("APPDATA") {
                paths.push(PathBuf::from(appdata).join("kelora").join("config.ini"));
            }
            if let Ok(userprofile) = env::var("USERPROFILE") {
                paths.push(PathBuf::from(userprofile).join(".kelorarc"));
            }
        } else {
            // Unix paths in order of preference:
            // 1. $XDG_CONFIG_HOME/kelora/config.ini
            // 2. ~/.config/kelora/config.ini (XDG fallback)
            // 3. ~/.kelorarc (legacy/compatibility)
            let xdg_config = env::var("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    env::var("HOME")
                        .map(|h| PathBuf::from(h).join(".config"))
                        .unwrap_or_else(|_| PathBuf::from(".config"))
                });

            paths.push(xdg_config.join("kelora").join("config.ini"));

            if let Ok(home) = env::var("HOME") {
                paths.push(PathBuf::from(home).join(".kelorarc"));
            }
        }

        paths
    }

    /// Get list of all config file locations in precedence order
    /// Order: project .kelorarc > user config files
    pub fn get_config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // Project config has highest precedence
        if let Some(project_config) = Self::find_project_config() {
            paths.push(project_config);
        }

        // Add user config files
        paths.extend(Self::get_user_config_paths());

        paths
    }

    /// Find the first existing configuration file
    pub fn find_config_path() -> Option<PathBuf> {
        Self::get_config_paths().into_iter().find(|p| p.exists())
    }

    /// Load configuration with proper precedence: project > user > defaults
    pub fn load() -> Result<Self> {
        let mut config = Self::default();

        // First, load user config files (lowest precedence)
        for path in Self::get_user_config_paths() {
            if path.exists() {
                let user_config = Self::load_from_path(&path)?;
                config = Self::merge_configs(config, user_config);
                break; // Use first existing user config file
            }
        }

        // Then, load project config (higher precedence)
        if let Some(project_path) = Self::find_project_config() {
            let project_config = Self::load_from_path(&project_path)?;
            config = Self::merge_configs(config, project_config);
        }

        Ok(config)
    }

    /// Load configuration with optional custom config file path
    pub fn load_with_custom_path(custom_path: Option<&str>) -> Result<Self> {
        if let Some(path) = custom_path {
            // Use custom path
            let path_buf = std::path::PathBuf::from(path);
            Self::load_from_path(&path_buf)
        } else {
            // Use default loading logic
            Self::load()
        }
    }

    /// Load configuration from a specific path
    pub fn load_from_path(path: &PathBuf) -> Result<Self> {
        use std::fs;

        // Read the file content first
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        // Parse the INI content
        let config = Self::parse_ini_content(&content)?;

        Ok(config)
    }

    /// Parse INI content from string
    fn parse_ini_content(content: &str) -> Result<Self> {
        let mut defaults = None;
        let mut aliases = HashMap::new();
        let mut current_section = String::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }

            // Check for section headers
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].to_string();
                continue;
            }

            // Parse key=value pairs
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim();

                if current_section.is_empty() {
                    // Root-level configuration
                    if key == "defaults" {
                        defaults = Some(value.to_string());
                    }
                    // Ignore unknown root-level keys
                } else {
                    match current_section.as_str() {
                        "aliases" => {
                            aliases.insert(key.to_string(), value.to_string());
                        }
                        _ => {
                            // Ignore unknown sections
                        }
                    }
                }
            }
        }

        Ok(Self { defaults, aliases })
    }

    /// Merge two configuration objects, with the second taking precedence
    fn merge_configs(base: Self, overlay: Self) -> Self {
        Self {
            // Overlay defaults override base defaults
            defaults: overlay.defaults.or(base.defaults),
            // Merge aliases, with overlay taking precedence for conflicting keys
            aliases: {
                let mut merged = base.aliases;
                merged.extend(overlay.aliases);
                merged
            },
        }
    }

    /// Show configuration information with precedence details
    pub fn show_config() {
        println!("Configuration precedence: CLI > project .kelorarc > user config > defaults\n");

        let project_config_path = Self::find_project_config();
        let user_config_paths = Self::get_user_config_paths();
        let user_config_path = user_config_paths.iter().find(|p| p.exists());

        // Load merged configuration
        match Self::load() {
            Ok(merged_config) => {
                // Show which configs were loaded
                let mut loaded_from = Vec::new();

                if let Some(project_path) = &project_config_path {
                    loaded_from.push(format!("Project: {}", project_path.display()));
                }

                if let Some(user_path) = user_config_path {
                    loaded_from.push(format!("User: {}", user_path.display()));
                }

                if loaded_from.is_empty() {
                    println!("No configuration files found. Using defaults.");
                } else {
                    println!("Configuration loaded from:");
                    for source in loaded_from {
                        println!("  {}", source);
                    }
                }

                // Show merged configuration
                if let Some(defaults) = &merged_config.defaults {
                    println!("\nActive defaults:");
                    println!("  defaults = {}", defaults);
                }

                if !merged_config.aliases.is_empty() {
                    println!("\nActive aliases:");
                    let mut sorted_aliases: Vec<_> = merged_config.aliases.iter().collect();
                    sorted_aliases.sort_by_key(|(k, _)| k.as_str());
                    for (key, value) in sorted_aliases {
                        println!("  {} = {}", key, value);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error loading configuration: {}", e);
            }
        }

        // Show search locations
        println!("\nConfiguration search locations (in precedence order):");
        if let Some(ref project_path) = project_config_path {
            println!(
                "  1. Project: {} {}",
                project_path.display(),
                if project_path.exists() {
                    "(found)"
                } else {
                    "(not found)"
                }
            );
        } else {
            println!("  1. Project: .kelorarc (searched up directory tree, not found)");
        }

        for (i, path) in user_config_paths.iter().enumerate() {
            let status = if path.exists() {
                "(found)"
            } else {
                "(not found)"
            };
            println!("  {}. User: {} {}", i + 2, path.display(), status);
        }

        // Show example configuration
        if project_config_path.is_none() && user_config_path.is_none() {
            println!("\nExample configuration file (.kelorarc):");
            println!();
            println!("# Set default arguments applied to every kelora command");
            println!("defaults = --format auto --stats --input-tz UTC");
            println!();
            println!("[aliases]");
            println!("errors = --filter 'e.level == \"error\"' --stats");
            println!(
                "json-errors = --format jsonl --filter 'e.level == \"error\"' --output-format jsonl"
            );
            println!("slow-requests = --filter 'e.response_time.to_int() > 1000' --keys timestamp,method,path,response_time");
        }
    }

    /// Resolve a single alias, handling recursive references
    pub fn resolve_alias(
        &self,
        name: &str,
        seen: &mut std::collections::HashSet<String>,
        depth: usize,
    ) -> Result<Vec<String>> {
        const MAX_DEPTH: usize = 10;

        if depth > MAX_DEPTH {
            return Err(anyhow!("Alias chain too deep: {} levels", depth));
        }

        if seen.contains(name) {
            return Err(anyhow!("Circular dependency detected in alias: {}", name));
        }

        let alias_value = self
            .aliases
            .get(name)
            .ok_or_else(|| anyhow!("Unknown alias: {}", name))?;

        seen.insert(name.to_string());

        // Split the alias value into args using shell-like parsing
        let args = shell_words::split(alias_value)
            .with_context(|| format!("Invalid alias '{}': failed to parse arguments", name))?;

        let mut result = Vec::new();
        let mut i = 0;

        while i < args.len() {
            if (args[i] == "-a" || args[i] == "--alias") && i + 1 < args.len() {
                // Recursively resolve the referenced alias
                let ref_name = &args[i + 1];
                let mut new_seen = seen.clone();
                let resolved = self.resolve_alias(ref_name, &mut new_seen, depth + 1)?;
                result.extend(resolved);
                i += 2;
            } else {
                result.push(args[i].clone());
                i += 1;
            }
        }

        seen.remove(name);
        Ok(result)
    }

    /// Process command line arguments, expanding aliases and applying defaults
    pub fn process_args(&self, args: Vec<String>) -> Result<Vec<String>> {
        // First, apply defaults if they exist
        let mut result = Vec::new();

        // Add defaults at the beginning, but preserve the program name
        if let Some(defaults) = &self.defaults {
            if !args.is_empty() {
                result.push(args[0].clone()); // Keep program name
            }

            // Parse defaults using shell_words and add them
            let default_args = shell_words::split(defaults)
                .with_context(|| "Invalid defaults: failed to parse arguments".to_string())?;
            result.extend(default_args);

            // Add remaining user args (skip program name)
            result.extend(args.into_iter().skip(1));
        } else {
            result = args;
        }

        // Then expand aliases
        let mut final_result = Vec::new();
        let mut i = 0;

        while i < result.len() {
            if (result[i] == "-a" || result[i] == "--alias") && i + 1 < result.len() {
                let name = &result[i + 1];
                let mut seen = std::collections::HashSet::new();
                let resolved = self.resolve_alias(name, &mut seen, 0)?;
                final_result.extend(resolved);
                i += 2;
            } else {
                final_result.push(result[i].clone());
                i += 1;
            }
        }

        Ok(final_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_config_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "defaults = --format jsonl --output-format csv").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "[aliases]").unwrap();
        writeln!(file, "errors = --filter 'e.level == \"error\"'").unwrap();
        writeln!(file, "json-logs = --format jsonl --output-format jsonl").unwrap();
        file.flush().unwrap();

        let config = ConfigFile::load_from_path(&file.path().to_path_buf()).unwrap();

        assert_eq!(
            config.defaults,
            Some("--format jsonl --output-format csv".to_string())
        );
        assert_eq!(
            config.aliases.get("errors"),
            Some(&"--filter 'e.level == \"error\"'".to_string())
        );
        assert_eq!(
            config.aliases.get("json-logs"),
            Some(&"--format jsonl --output-format jsonl".to_string())
        );
    }

    #[test]
    fn test_resolve_alias() {
        let mut config = ConfigFile::default();
        config.aliases.insert(
            "errors".to_string(),
            "--filter 'e.level == \"error\"'".to_string(),
        );
        config.aliases.insert(
            "json-errors".to_string(),
            "--format jsonl -a errors".to_string(),
        );

        let mut seen = std::collections::HashSet::new();
        let resolved = config.resolve_alias("json-errors", &mut seen, 0).unwrap();

        assert_eq!(
            resolved,
            vec!["--format", "jsonl", "--filter", "e.level == \"error\""]
        );
    }

    #[test]
    fn test_circular_alias_detection() {
        let mut config = ConfigFile::default();
        config
            .aliases
            .insert("alias1".to_string(), "-a alias2".to_string());
        config
            .aliases
            .insert("alias2".to_string(), "-a alias1".to_string());

        let mut seen = std::collections::HashSet::new();
        let result = config.resolve_alias("alias1", &mut seen, 0);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Circular dependency"));
    }

    #[test]
    fn test_process_args() {
        let mut config = ConfigFile::default();
        config.aliases.insert(
            "errors".to_string(),
            "--filter 'e.level == \"error\"' --stats".to_string(),
        );

        let args = vec![
            "kelora".to_string(),
            "-a".to_string(),
            "errors".to_string(),
            "--format".to_string(),
            "jsonl".to_string(),
        ];

        let processed = config.process_args(args).unwrap();

        assert_eq!(
            processed,
            vec![
                "kelora",
                "--filter",
                "e.level == \"error\"",
                "--stats",
                "--format",
                "jsonl"
            ]
        );
    }

    #[test]
    fn test_process_args_with_defaults() {
        let mut config = ConfigFile::default();
        config.defaults = Some("--stats --parallel".to_string());

        let args = vec![
            "kelora".to_string(),
            "--format".to_string(),
            "jsonl".to_string(),
            "input.log".to_string(),
        ];

        let processed = config.process_args(args).unwrap();

        assert_eq!(
            processed,
            vec![
                "kelora",
                "--stats",
                "--parallel",
                "--format",
                "jsonl",
                "input.log"
            ]
        );
    }

    #[test]
    fn test_process_args_with_defaults_and_aliases() {
        let mut config = ConfigFile::default();
        config.defaults = Some("--stats".to_string());
        config.aliases.insert(
            "errors".to_string(),
            "--filter 'e.level == \"error\"'".to_string(),
        );

        let args = vec![
            "kelora".to_string(),
            "-a".to_string(),
            "errors".to_string(),
            "--format".to_string(),
            "jsonl".to_string(),
        ];

        let processed = config.process_args(args).unwrap();

        assert_eq!(
            processed,
            vec![
                "kelora",
                "--stats",
                "--filter",
                "e.level == \"error\"",
                "--format",
                "jsonl"
            ]
        );
    }

    #[test]
    fn test_project_config_discovery() {
        use tempfile::TempDir;

        // Create a temporary directory structure
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().canonicalize().unwrap();
        let subdir = project_root.join("src").join("deep");
        std::fs::create_dir_all(&subdir).unwrap();

        // Create .kelorarc in project root
        let config_path = project_root.join(".kelorarc");
        std::fs::write(&config_path, "defaults = --project-test").unwrap();

        // Change to subdirectory and test discovery
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&subdir).unwrap();

        let found_config = ConfigFile::find_project_config();

        // Restore original directory
        std::env::set_current_dir(&original_dir).unwrap();

        assert_eq!(found_config, Some(config_path));
    }

    #[test]
    fn test_config_merging() {
        // Test that project config overrides user config properly
        let user_config = ConfigFile {
            defaults: Some("--user-defaults".to_string()),
            aliases: {
                let mut aliases = HashMap::new();
                aliases.insert("user-alias".to_string(), "--user-value".to_string());
                aliases.insert("shared-alias".to_string(), "--user-shared".to_string());
                aliases
            },
        };

        let project_config = ConfigFile {
            defaults: Some("--project-defaults".to_string()),
            aliases: {
                let mut aliases = HashMap::new();
                aliases.insert("project-alias".to_string(), "--project-value".to_string());
                aliases.insert("shared-alias".to_string(), "--project-shared".to_string());
                aliases
            },
        };

        let merged = ConfigFile::merge_configs(user_config, project_config);

        // Project defaults should override user defaults
        assert_eq!(merged.defaults, Some("--project-defaults".to_string()));

        // Aliases should be merged with project taking precedence for conflicts
        assert_eq!(
            merged.aliases.get("user-alias"),
            Some(&"--user-value".to_string())
        );
        assert_eq!(
            merged.aliases.get("project-alias"),
            Some(&"--project-value".to_string())
        );
        assert_eq!(
            merged.aliases.get("shared-alias"),
            Some(&"--project-shared".to_string())
        );
    }

    #[test]
    fn test_config_merging_with_none_defaults() {
        let base_config = ConfigFile {
            defaults: Some("--base-defaults".to_string()),
            aliases: HashMap::new(),
        };

        let overlay_config = ConfigFile {
            defaults: None,
            aliases: {
                let mut aliases = HashMap::new();
                aliases.insert("test-alias".to_string(), "--test-value".to_string());
                aliases
            },
        };

        let merged = ConfigFile::merge_configs(base_config, overlay_config);

        // Base defaults should remain since overlay has None
        assert_eq!(merged.defaults, Some("--base-defaults".to_string()));
        assert_eq!(
            merged.aliases.get("test-alias"),
            Some(&"--test-value".to_string())
        );
    }

    #[test]
    fn test_project_config_not_found() {
        use tempfile::TempDir;

        // Create a temporary directory without .kelorarc
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("no-config");
        std::fs::create_dir_all(&subdir).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&subdir).unwrap();

        let found_config = ConfigFile::find_project_config();

        std::env::set_current_dir(&original_dir).unwrap();

        assert_eq!(found_config, None);
    }

    #[test]
    fn test_user_config_paths() {
        let paths = ConfigFile::get_user_config_paths();

        // Should have at least one path
        assert!(!paths.is_empty());

        // Check that user paths are appropriate user-level configurations
        for path in &paths {
            let file_name = path.file_name().unwrap().to_string_lossy();

            // Should be either config.ini in a config directory or legacy .kelorarc
            let is_config_ini = file_name == "config.ini";
            let is_legacy_kelorarc = file_name == ".kelorarc";

            assert!(
                is_config_ini || is_legacy_kelorarc,
                "Unexpected user config filename: {}",
                file_name
            );

            // .kelorarc should be in a user directory (not project directory)
            if is_legacy_kelorarc {
                let parent_path = path.parent().unwrap().to_string_lossy();
                // On Unix systems, user home is under /Users, /home, or contains HOME var
                // On Windows, it contains USERPROFILE
                let is_user_dir = parent_path.contains("/Users/")
                    || parent_path.contains("/home/")
                    || parent_path.contains("USERPROFILE")
                    || parent_path.contains(&std::env::var("HOME").unwrap_or_default());
                assert!(
                    is_user_dir,
                    "Legacy .kelorarc not in user directory: {}",
                    parent_path
                );
            }
        }
    }

    #[test]
    fn test_get_config_paths_precedence() {
        use tempfile::TempDir;

        // Create temp project with .kelorarc
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().canonicalize().unwrap();
        let config_path = project_root.join(".kelorarc");
        std::fs::write(&config_path, "test").unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&project_root).unwrap();

        let paths = ConfigFile::get_config_paths();

        std::env::set_current_dir(&original_dir).unwrap();

        // First path should be the project config
        assert_eq!(paths[0], config_path);

        // Remaining paths should be user config paths
        let user_paths = ConfigFile::get_user_config_paths();
        assert_eq!(&paths[1..], &user_paths[..]);
    }
}

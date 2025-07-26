#![allow(dead_code)]
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

/// Configuration file handler for kelora
#[derive(Default)]
pub struct ConfigFile {
    pub defaults: HashMap<String, String>,
    pub aliases: HashMap<String, String>,
}

impl ConfigFile {
    /// Get list of possible config file locations in order of preference
    pub fn get_config_paths() -> Vec<PathBuf> {
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

    /// Find the first existing configuration file
    pub fn find_config_path() -> Option<PathBuf> {
        Self::get_config_paths().into_iter().find(|p| p.exists())
    }

    /// Load configuration from file
    pub fn load() -> Result<Self> {
        let config_file = Self::find_config_path();

        if let Some(path) = config_file {
            Self::load_from_path(&path)
        } else {
            Ok(Self::default())
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
        let mut defaults = HashMap::new();
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

                match current_section.as_str() {
                    "defaults" => {
                        // Convert kebab-case to underscore for internal consistency
                        let normalized_key = key.replace('-', "_");
                        defaults.insert(normalized_key, value.to_string());
                    }
                    "aliases" => {
                        aliases.insert(key.to_string(), value.to_string());
                    }
                    _ => {
                        // Ignore unknown sections
                    }
                }
            }
        }

        Ok(Self { defaults, aliases })
    }

    /// Show configuration information
    pub fn show_config() {
        let config_file = Self::find_config_path();

        if let Some(path) = config_file {
            match Self::load_from_path(&path) {
                Ok(config) => {
                    println!("Configuration loaded from: {}", path.display());

                    if !config.defaults.is_empty() {
                        println!("\nDefaults:");
                        let mut sorted_defaults: Vec<_> = config.defaults.iter().collect();
                        sorted_defaults.sort_by_key(|(k, _)| k.as_str());
                        for (key, value) in sorted_defaults {
                            println!("  {} = {}", key, value);
                        }
                    }

                    if !config.aliases.is_empty() {
                        println!("\nAliases:");
                        let mut sorted_aliases: Vec<_> = config.aliases.iter().collect();
                        sorted_aliases.sort_by_key(|(k, _)| k.as_str());
                        for (key, value) in sorted_aliases {
                            println!("  {} = {}", key, value);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error loading config file: {}", e);
                }
            }
        } else {
            println!("No config file found. Searched locations:");
            for path in Self::get_config_paths() {
                println!("  {}", path.display());
            }
            println!("\nCreate a config file at any of these locations. Example:");
            println!();
            println!("[defaults]");
            println!("format = jsonl");
            println!("output-format = default");
            println!("skip-lines = 0");
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
        let mut result = Vec::new();
        let mut i = 0;

        while i < args.len() {
            if (args[i] == "-a" || args[i] == "--alias") && i + 1 < args.len() {
                let name = &args[i + 1];
                let mut seen = std::collections::HashSet::new();
                let resolved = self.resolve_alias(name, &mut seen, 0)?;
                result.extend(resolved);
                i += 2;
            } else {
                result.push(args[i].clone());
                i += 1;
            }
        }

        Ok(result)
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
        writeln!(file, "[defaults]").unwrap();
        writeln!(file, "format = jsonl").unwrap();
        writeln!(file, "output-format = csv").unwrap();
        writeln!(file, "").unwrap();
        writeln!(file, "[aliases]").unwrap();
        writeln!(file, "errors = --filter 'e.level == \"error\"'").unwrap();
        writeln!(file, "json-logs = --format jsonl --output-format jsonl").unwrap();
        file.flush().unwrap();

        let config = ConfigFile::load_from_path(&file.path().to_path_buf()).unwrap();

        assert_eq!(config.defaults.get("format"), Some(&"jsonl".to_string()));
        assert_eq!(
            config.defaults.get("output_format"),
            Some(&"csv".to_string())
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
}

use anyhow::Result;
use clap::{ArgMatches, CommandFactory, FromArgMatches};
use std::sync::atomic::Ordering;

mod colors;
mod config;
mod config_file;
mod decompression;
mod engine;
mod event;
mod formatters;
mod parallel;
mod parsers;
mod pipeline;
mod platform;
mod readers;
mod rhai_functions;
mod stats;
mod timestamp;
mod tty;

use config::KeloraConfig;
use config_file::ConfigFile;
use platform::{
    ExitCode, ProcessCleanup, SafeFileOut, SafeStderr, SafeStdout, SignalHandler, SHOULD_TERMINATE,
};

// Use CLI types from library
use kelora::{
    run_pipeline_with_kelora_config, Cli, FileOrder, InputFormat, KeloraConfig as LibKeloraConfig,
    MultilineConfig, OutputFormat, TimestampFilterConfig,
};

fn main() -> Result<()> {
    // Initialize signal handling early
    let _signal_handler = SignalHandler::new()
        .map_err(|e| {
            eprintln!("Failed to initialize signal handling: {}", e);
            ExitCode::GeneralError.exit();
        })
        .unwrap();

    // Initialize process cleanup
    let _cleanup = ProcessCleanup::new();

    // Initialize safe I/O wrappers
    let mut stderr = SafeStderr::new();

    // Process command line arguments with config file support
    let (matches, cli) = process_args_with_config(&mut stderr);

    // Validate CLI argument combinations
    if let Err(e) = validate_cli_args(&cli) {
        stderr
            .writeln(&format!("kelora: Error: {}", e))
            .unwrap_or(());
        ExitCode::InvalidUsage.exit();
    }

    // Extract ordered script stages
    let ordered_stages = match cli.get_ordered_script_stages(&matches) {
        Ok(stages) => stages,
        Err(e) => {
            stderr
                .writeln(&format!("kelora: Error: {}", e))
                .unwrap_or(());
            ExitCode::InvalidUsage.exit();
        }
    };

    // Create configuration from CLI and set stages (using lib config directly)
    let mut lib_config = LibKeloraConfig::from_cli(&cli);
    // Set the ordered stages directly
    lib_config.processing.stages = ordered_stages;

    // Parse timestamp filter arguments if provided
    if cli.since.is_some() || cli.until.is_some() {
        // Use the same timezone logic as the main configuration
        let cli_timezone = lib_config.input.default_timezone.as_deref();

        let since = if let Some(ref since_str) = cli.since {
            match crate::timestamp::parse_timestamp_arg_with_timezone(since_str, cli_timezone) {
                Ok(dt) => Some(dt),
                Err(e) => {
                    stderr
                        .writeln(&lib_config.format_error_message(&format!(
                            "Invalid --since timestamp '{}': {}",
                            since_str, e
                        )))
                        .unwrap_or(());
                    ExitCode::InvalidUsage.exit();
                }
            }
        } else {
            None
        };

        let until = if let Some(ref until_str) = cli.until {
            match crate::timestamp::parse_timestamp_arg_with_timezone(until_str, cli_timezone) {
                Ok(dt) => Some(dt),
                Err(e) => {
                    stderr
                        .writeln(&lib_config.format_error_message(&format!(
                            "Invalid --until timestamp '{}': {}",
                            until_str, e
                        )))
                        .unwrap_or(());
                    ExitCode::InvalidUsage.exit();
                }
            }
        } else {
            None
        };

        lib_config.processing.timestamp_filter = Some(TimestampFilterConfig { since, until });
    }

    // Compile ignore-lines regex if provided
    if let Some(ignore_pattern) = &cli.ignore_lines {
        match regex::Regex::new(ignore_pattern) {
            Ok(regex) => {
                lib_config.input.ignore_lines = Some(regex);
            }
            Err(e) => {
                stderr
                    .writeln(&lib_config.format_error_message(&format!(
                        "Invalid ignore-lines regex pattern '{}': {}",
                        ignore_pattern, e
                    )))
                    .unwrap_or(());
                ExitCode::InvalidUsage.exit();
            }
        }
    }

    // Parse multiline configuration if provided, or apply format defaults
    if let Some(multiline_str) = &cli.multiline {
        match MultilineConfig::parse(multiline_str) {
            Ok(multiline_config) => {
                lib_config.input.multiline = Some(multiline_config);
            }
            Err(e) => {
                stderr
                    .writeln(&lib_config.format_error_message(&format!(
                        "Invalid multiline configuration '{}': {}",
                        multiline_str, e
                    )))
                    .unwrap_or(());
                ExitCode::InvalidUsage.exit();
            }
        }
    } else {
        // Apply format-specific default multiline configuration
        lib_config.input.multiline = lib_config.input.format.default_multiline();
    }

    // Validate arguments early
    if let Err(e) = validate_cli_args(&cli) {
        stderr
            .writeln(&lib_config.format_error_message(&format!("Error: {}", e)))
            .unwrap_or(());
        ExitCode::InvalidUsage.exit();
    }

    // Handle output destination and run pipeline
    let result = if let Some(ref output_file_path) = cli.output_file {
        // Use file output
        let file_output = match SafeFileOut::new(output_file_path) {
            Ok(file) => file,
            Err(e) => {
                stderr
                    .writeln(&lib_config.format_error_message(&e.to_string()))
                    .unwrap_or(());
                ExitCode::GeneralError.exit();
            }
        };
        run_pipeline_with_kelora_config(&lib_config, file_output)
    } else {
        // Use stdout output
        let stdout_output = SafeStdout::new();
        run_pipeline_with_kelora_config(&lib_config, stdout_output)
    };

    let (final_stats, tracking_data) = match result {
        Ok(pipeline_result) => {
            // Print metrics if enabled (only if not terminated)
            if lib_config.output.metrics && !SHOULD_TERMINATE.load(Ordering::Relaxed) {
                let metrics_output = crate::rhai_functions::tracking::format_metrics_output(
                    &pipeline_result.tracking_data,
                );
                if !metrics_output.is_empty() && metrics_output != "No metrics tracked" {
                    stderr
                        .writeln(&lib_config.format_metrics_message(&metrics_output))
                        .unwrap_or(());
                }
            }

            // Write metrics to file if configured
            if let Some(ref metrics_file) = lib_config.output.metrics_file {
                if let Ok(json_output) = crate::rhai_functions::tracking::format_metrics_json(
                    &pipeline_result.tracking_data,
                ) {
                    if let Err(e) = std::fs::write(metrics_file, json_output) {
                        stderr
                            .writeln(&lib_config.format_error_message(&format!(
                                "Failed to write metrics file: {}",
                                e
                            )))
                            .unwrap_or(());
                    }
                }
            }

            // Print output based on configuration (only if not terminated)
            if !SHOULD_TERMINATE.load(Ordering::Relaxed) {
                if let Some(ref s) = pipeline_result.stats {
                    if lib_config.output.stats && !lib_config.processing.quiet {
                        // Full stats when --stats flag is used (unless --quiet)
                        stderr
                            .writeln(&lib_config.format_stats_message(
                                &s.format_stats(lib_config.input.multiline.is_some()),
                            ))
                            .unwrap_or(());
                    } else if !lib_config.processing.quiet {
                        // Error summary by default when errors occur (unless --quiet)
                        if let Some(error_summary) =
                            crate::rhai_functions::tracking::extract_error_summary_from_tracking(
                                &pipeline_result.tracking_data,
                                lib_config.processing.verbose,
                            )
                        {
                            stderr
                                .writeln(&lib_config.format_error_message(&error_summary))
                                .unwrap_or(());
                        }
                    }
                }
            }
            (pipeline_result.stats, Some(pipeline_result.tracking_data))
        }
        Err(e) => {
            stderr
                .writeln(&lib_config.format_error_message(&format!("Pipeline error: {}", e)))
                .unwrap_or(());
            ExitCode::GeneralError.exit();
        }
    };

    // Check if we were terminated by a signal and print output
    if SHOULD_TERMINATE.load(Ordering::Relaxed) {
        if let Some(stats) = final_stats {
            if lib_config.output.stats && !lib_config.processing.quiet {
                // Full stats when --stats flag is used (unless --quiet)
                stderr
                    .writeln(&lib_config.format_stats_message(
                        &stats.format_stats(lib_config.input.multiline.is_some()),
                    ))
                    .unwrap_or(());
            } else if stats.has_errors() && !lib_config.processing.quiet {
                // Error summary by default when errors occur (unless --quiet)
                stderr
                    .writeln(&lib_config.format_error_message(&stats.format_error_summary()))
                    .unwrap_or(());
            }
        } else if lib_config.output.stats && !lib_config.processing.quiet {
            stderr
                .writeln(&lib_config.format_stats_message("Processing interrupted"))
                .unwrap_or(());
        }
        ExitCode::SignalInt.exit();
    }

    // Determine exit code based on whether any errors occurred during processing
    let had_errors = if let Some(ref tracking) = tracking_data {
        // Check tracking data for errors from processing
        crate::rhai_functions::tracking::has_errors_in_tracking(tracking)
    } else if let Some(ref stats) = final_stats {
        // Check stats for errors from parallel processing or termination case
        stats.has_errors()
    } else {
        // No processing results available, assume no errors
        false
    };

    if had_errors {
        ExitCode::GeneralError.exit();
    } else {
        ExitCode::Success.exit();
    }
}

/// Validate CLI arguments for early error detection
fn validate_cli_args(cli: &Cli) -> Result<()> {
    // Check if input files exist (if specified), skip "-" which represents stdin
    let mut stdin_count = 0;
    for file_path in &cli.files {
        if file_path == "-" {
            stdin_count += 1;
            if stdin_count > 1 {
                return Err(anyhow::anyhow!("stdin (\"-\") can only be specified once"));
            }
        } else if !std::path::Path::new(file_path).exists() {
            return Err(anyhow::anyhow!("File not found: {}", file_path));
        }
    }

    // Check if exec files exist (if specified)
    for exec_file in &cli.exec_files {
        if !std::path::Path::new(exec_file).exists() {
            return Err(anyhow::anyhow!("Exec file not found: {}", exec_file));
        }
    }

    // Validate batch size
    if let Some(batch_size) = cli.batch_size {
        if batch_size == 0 {
            return Err(anyhow::anyhow!("Batch size must be greater than 0"));
        }
    }

    // Validate thread count
    if cli.threads > 1000 {
        return Err(anyhow::anyhow!("Thread count too high (max 1000)"));
    }

    // Check for --core with CSV/TSV formats (not allowed with these formats)
    if cli.core {
        match cli.output_format {
            OutputFormat::Csv => {
                return Err(anyhow::anyhow!(
                    "csv output format does not support --core flag. Use --keys to specify field names"
                ));
            }
            OutputFormat::Tsv => {
                return Err(anyhow::anyhow!(
                    "tsv output format does not support --core flag. Use --keys to specify field names"
                ));
            }
            OutputFormat::Csvnh => {
                return Err(anyhow::anyhow!(
                    "csvnh output format does not support --core flag. Use --keys to specify field names"
                ));
            }
            OutputFormat::Tsvnh => {
                return Err(anyhow::anyhow!(
                    "tsvnh output format does not support --core flag. Use --keys to specify field names"
                ));
            }
            _ => {
                // Other formats are fine with --core
            }
        }
    }

    Ok(())
}

/// Validate configuration for consistency
#[allow(dead_code)]
fn validate_config(config: &KeloraConfig) -> Result<()> {
    // Check if files exist (if specified)
    for file_path in &config.input.files {
        if !std::path::Path::new(file_path).exists() {
            return Err(anyhow::anyhow!("File not found: {}", file_path));
        }
    }

    // Validate batch size
    if let Some(batch_size) = config.performance.batch_size {
        if batch_size == 0 {
            return Err(anyhow::anyhow!("Batch size must be greater than 0"));
        }
    }

    // Validate thread count
    if config.performance.threads > 1000 {
        return Err(anyhow::anyhow!("Thread count too high (max 1000)"));
    }

    Ok(())
}

/// Process command line arguments with config file support
fn process_args_with_config(stderr: &mut SafeStderr) -> (ArgMatches, Cli) {
    // Get raw command line arguments
    let raw_args: Vec<String> = std::env::args().collect();

    // Check for --show-config first, before any other processing
    if raw_args.iter().any(|arg| arg == "--show-config") {
        ConfigFile::show_config();
        std::process::exit(0);
    }

    // Check for --help-time
    if raw_args.iter().any(|arg| arg == "--help-time") {
        print_time_format_help();
        std::process::exit(0);
    }

    // Check for --help-functions
    if raw_args.iter().any(|arg| arg == "--help-functions") {
        print_functions_help();
        std::process::exit(0);
    }

    // Check for --help-rhai
    if raw_args.iter().any(|arg| arg == "--help-rhai") {
        print_rhai_help();
        std::process::exit(0);
    }

    // Check for --ignore-config
    let ignore_config = raw_args.iter().any(|arg| arg == "--ignore-config");

    let processed_args = if ignore_config {
        // Skip config file processing
        raw_args
    } else {
        // Load config file and process aliases
        match ConfigFile::load() {
            Ok(config_file) => match config_file.process_args(raw_args) {
                Ok(processed) => processed,
                Err(e) => {
                    stderr
                        .writeln(&format!("kelora: Config error: {}", e))
                        .unwrap_or(());
                    std::process::exit(1);
                }
            },
            Err(e) => {
                stderr
                    .writeln(&format!("kelora: Config file error: {}", e))
                    .unwrap_or(());
                std::process::exit(1);
            }
        }
    };

    // Parse with potentially modified arguments
    let matches = Cli::command().get_matches_from(processed_args);
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| {
        stderr
            .writeln(&format!("kelora: Error: {}", e))
            .unwrap_or(());
        std::process::exit(1);
    });

    // Apply config file defaults to CLI if not ignoring config
    let cli = if ignore_config {
        cli
    } else {
        match ConfigFile::load() {
            Ok(config_file) => apply_config_defaults(cli, &config_file),
            Err(_) => cli, // Already handled error above
        }
    };

    // Show usage if on TTY and no input files provided (but not if "-" is explicitly specified)
    if crate::tty::is_stdin_tty() && cli.files.is_empty() {
        // Print brief usage with description and help hint
        println!("{}", Cli::command().render_usage());
        println!("A command-line log analysis tool with embedded Rhai scripting");
        println!("Try 'kelora --help' for more information.");
        std::process::exit(0);
    }

    (matches, cli)
}

/// Apply configuration file defaults to CLI arguments
fn apply_config_defaults(mut cli: Cli, config_file: &ConfigFile) -> Cli {
    // Apply defaults only if the CLI value is still at its default
    // This ensures CLI arguments take precedence over config file

    if let Some(format) = config_file.defaults.get("input_format") {
        // Only apply if format is still at default ("line")
        if matches!(cli.format, crate::InputFormat::Line) {
            cli.format = match format.as_str() {
                "auto" => crate::InputFormat::Auto,
                "jsonl" => crate::InputFormat::Jsonl,
                "line" => crate::InputFormat::Line,
                "logfmt" => crate::InputFormat::Logfmt,
                "syslog" => crate::InputFormat::Syslog,
                "cef" => crate::InputFormat::Cef,
                "csv" => crate::InputFormat::Csv,
                "tsv" => crate::InputFormat::Tsv,
                "csvnh" => crate::InputFormat::Csvnh,
                "tsvnh" => crate::InputFormat::Tsvnh,
                "apache" => crate::InputFormat::Apache,
                "nginx" => crate::InputFormat::Nginx,
                "cols" => crate::InputFormat::Cols,
                "docker" => crate::InputFormat::Docker,
                _ => cli.format, // Keep original if invalid
            };
        }
    }

    if let Some(output_format) = config_file.defaults.get("output_format") {
        if matches!(cli.output_format, crate::OutputFormat::Default) {
            cli.output_format = match output_format.as_str() {
                "jsonl" => crate::OutputFormat::Jsonl,
                "default" => crate::OutputFormat::Default,
                "logfmt" => crate::OutputFormat::Logfmt,
                "csv" => crate::OutputFormat::Csv,
                "tsv" => crate::OutputFormat::Tsv,
                "csvnh" => crate::OutputFormat::Csvnh,
                "tsvnh" => crate::OutputFormat::Tsvnh,
                "hide" => crate::OutputFormat::Hide,
                "null" => crate::OutputFormat::Null,
                _ => cli.output_format,
            };
        }
    }

    // Note: on_error configuration removed in resiliency model - use --strict flag instead

    if let Some(file_order) = config_file.defaults.get("file_order") {
        if matches!(cli.file_order, crate::FileOrder::None) {
            cli.file_order = match file_order.as_str() {
                "none" => crate::FileOrder::None,
                "name" => crate::FileOrder::Name,
                "mtime" => crate::FileOrder::Mtime,
                _ => cli.file_order,
            };
        }
    }

    // Apply boolean flags from config if they weren't explicitly set
    if let Some(parallel) = config_file.defaults.get("parallel") {
        if !cli.parallel && parallel.parse::<bool>().unwrap_or(false) {
            cli.parallel = true;
        }
    }

    if let Some(core) = config_file.defaults.get("core") {
        if !cli.core && core.parse::<bool>().unwrap_or(false) {
            cli.core = true;
        }
    }

    if let Some(brief) = config_file.defaults.get("brief") {
        if !cli.brief && brief.parse::<bool>().unwrap_or(false) {
            cli.brief = true;
        }
    }

    if let Some(skip_lines) = config_file.defaults.get("skip_lines") {
        if cli.skip_lines.is_none() {
            if let Ok(value) = skip_lines.parse::<usize>() {
                cli.skip_lines = Some(value);
            }
        }
    }

    if let Some(stats) = config_file.defaults.get("stats") {
        if !cli.stats && stats.parse::<bool>().unwrap_or(false) {
            cli.stats = true;
        }
    }

    if let Some(stats_only) = config_file.defaults.get("stats_only") {
        if !cli.stats_only && stats_only.parse::<bool>().unwrap_or(false) {
            cli.stats_only = true;
        }
    }

    // Add support for new metrics and error reporting options
    if let Some(metrics) = config_file.defaults.get("metrics") {
        if !cli.metrics && metrics.parse::<bool>().unwrap_or(false) {
            cli.metrics = true;
        }
    }

    if let Some(metrics_file) = config_file.defaults.get("metrics_file") {
        if cli.metrics_file.is_none() {
            cli.metrics_file = Some(metrics_file.clone());
        }
    }

    // Note: error_report configuration removed in resiliency model - use --strict/--verbose flags instead

    if let Some(error_report_file) = config_file.defaults.get("error_report_file") {
        if cli.error_report_file.is_none() {
            cli.error_report_file = Some(error_report_file.clone());
        }
    }

    if let Some(no_section_headers) = config_file.defaults.get("no_section_headers") {
        if !cli.no_section_headers && no_section_headers.parse::<bool>().unwrap_or(false) {
            cli.no_section_headers = true;
        }
    }

    if let Some(no_emoji) = config_file.defaults.get("no_emoji") {
        if !cli.no_emoji && no_emoji.parse::<bool>().unwrap_or(false) {
            cli.no_emoji = true;
        }
    }

    if let Some(force_color) = config_file.defaults.get("force_color") {
        if !cli.force_color && force_color.parse::<bool>().unwrap_or(false) {
            cli.force_color = true;
        }
    }

    if let Some(no_color) = config_file.defaults.get("no_color") {
        if !cli.no_color && no_color.parse::<bool>().unwrap_or(false) {
            cli.no_color = true;
        }
    }

    // Apply numeric values
    if let Some(threads) = config_file.defaults.get("threads") {
        if cli.threads == 0 {
            if let Ok(thread_count) = threads.parse::<usize>() {
                cli.threads = thread_count;
            }
        }
    }

    if let Some(batch_size) = config_file.defaults.get("batch_size") {
        if cli.batch_size.is_none() {
            if let Ok(size) = batch_size.parse::<usize>() {
                cli.batch_size = Some(size);
            }
        }
    }

    if let Some(batch_timeout) = config_file.defaults.get("batch_timeout") {
        if cli.batch_timeout == 200 {
            // default value
            if let Ok(timeout) = batch_timeout.parse::<u64>() {
                cli.batch_timeout = timeout;
            }
        }
    }

    // Apply string values
    if let Some(ignore_lines) = config_file.defaults.get("ignore_lines") {
        if cli.ignore_lines.is_none() {
            cli.ignore_lines = Some(ignore_lines.clone());
        }
    }

    if let Some(multiline) = config_file.defaults.get("multiline") {
        if cli.multiline.is_none() {
            cli.multiline = Some(multiline.clone());
        }
    }

    if let Some(begin) = config_file.defaults.get("begin") {
        if cli.begin.is_none() {
            cli.begin = Some(begin.clone());
        }
    }

    if let Some(end) = config_file.defaults.get("end") {
        if cli.end.is_none() {
            cli.end = Some(end.clone());
        }
    }

    // Apply list values (only if CLI lists are empty)
    if let Some(filters) = config_file.defaults.get("filters") {
        if cli.filters.is_empty() {
            cli.filters = filters.split(',').map(|s| s.trim().to_string()).collect();
        }
    }

    if let Some(execs) = config_file.defaults.get("execs") {
        if cli.execs.is_empty() {
            cli.execs = execs.split(',').map(|s| s.trim().to_string()).collect();
        }
    }

    if let Some(levels) = config_file.defaults.get("levels") {
        if cli.levels.is_empty() {
            cli.levels = levels.split(',').map(|s| s.trim().to_string()).collect();
        }
    }

    if let Some(exclude_levels) = config_file.defaults.get("exclude_levels") {
        if cli.exclude_levels.is_empty() {
            cli.exclude_levels = exclude_levels
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
        }
    }

    if let Some(keys) = config_file.defaults.get("keys") {
        if cli.keys.is_empty() {
            cli.keys = keys.split(',').map(|s| s.trim().to_string()).collect();
        }
    }

    if let Some(exclude_keys) = config_file.defaults.get("exclude_keys") {
        if cli.exclude_keys.is_empty() {
            cli.exclude_keys = exclude_keys
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
        }
    }

    // Apply window_size from config if not explicitly set
    if let Some(window_size) = config_file.defaults.get("window_size") {
        if cli.window_size.is_none() {
            if let Ok(size) = window_size.parse::<usize>() {
                cli.window_size = Some(size);
            }
        }
    }

    // Apply additional missing options
    if let Some(ts_field) = config_file.defaults.get("ts_field") {
        if cli.ts_field.is_none() {
            cli.ts_field = Some(ts_field.clone());
        }
    }

    if let Some(ts_format) = config_file.defaults.get("ts_format") {
        if cli.ts_format.is_none() {
            cli.ts_format = Some(ts_format.clone());
        }
    }

    if let Some(input_tz) = config_file.defaults.get("input_tz") {
        if cli.input_tz.is_none() {
            cli.input_tz = Some(input_tz.clone());
        }
    }

    if let Some(output_file) = config_file.defaults.get("output_file") {
        if cli.output_file.is_none() {
            cli.output_file = Some(output_file.clone());
        }
    }

    if let Some(pretty_ts) = config_file.defaults.get("pretty_ts") {
        if cli.pretty_ts.is_none() {
            cli.pretty_ts = Some(pretty_ts.clone());
        }
    }

    if let Some(format_timestamps_local) = config_file.defaults.get("format_timestamps_local") {
        if !cli.format_timestamps_local && format_timestamps_local.parse::<bool>().unwrap_or(false)
        {
            cli.format_timestamps_local = true;
        }
    }

    if let Some(format_timestamps_utc) = config_file.defaults.get("format_timestamps_utc") {
        if !cli.format_timestamps_utc && format_timestamps_utc.parse::<bool>().unwrap_or(false) {
            cli.format_timestamps_utc = true;
        }
    }

    if let Some(no_preserve_order) = config_file.defaults.get("no_preserve_order") {
        if !cli.no_preserve_order && no_preserve_order.parse::<bool>().unwrap_or(false) {
            cli.no_preserve_order = true;
        }
    }

    if let Some(strict) = config_file.defaults.get("strict") {
        if !cli.strict && strict.parse::<bool>().unwrap_or(false) {
            cli.strict = true;
        }
    }

    if let Some(verbose) = config_file.defaults.get("verbose") {
        if !cli.verbose && verbose.parse::<bool>().unwrap_or(false) {
            cli.verbose = true;
        }
    }

    if let Some(quiet) = config_file.defaults.get("quiet") {
        if !cli.quiet && quiet.parse::<bool>().unwrap_or(false) {
            cli.quiet = true;
        }
    }

    if let Some(since) = config_file.defaults.get("since") {
        if cli.since.is_none() {
            cli.since = Some(since.clone());
        }
    }

    if let Some(until) = config_file.defaults.get("until") {
        if cli.until.is_none() {
            cli.until = Some(until.clone());
        }
    }

    if let Some(take) = config_file.defaults.get("take") {
        if cli.take.is_none() {
            if let Ok(value) = take.parse::<usize>() {
                cli.take = Some(value);
            }
        }
    }

    if let Some(exec_files) = config_file.defaults.get("exec_files") {
        if cli.exec_files.is_empty() {
            cli.exec_files = exec_files
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
        }
    }

    cli
}

/// Print time format help message adapted for Rust/chrono
fn print_time_format_help() {
    let help_text = r#"
Time Format Reference for --ts-format:

Basic Date/Time Components:
%Y - Year with century (e.g., 2024)
%y - Year without century (00-99)
%m - Month as zero-padded decimal (01-12)
%b - Month as abbreviated name (Jan, Feb, ..., Dec)
%B - Month as full name (January, February, ..., December)
%d - Day of month as zero-padded decimal (01-31)
%j - Day of year as zero-padded decimal (001-366)
%H - Hour (24-hour) as zero-padded decimal (00-23)
%I - Hour (12-hour) as zero-padded decimal (01-12)
%p - AM/PM indicator
%M - Minute as zero-padded decimal (00-59)
%S - Second as zero-padded decimal (00-59)

Subsecond Precision:
%f - Microseconds (000000-999999)
%3f - Milliseconds (000-999)
%6f - Microseconds (000000-999999)
%9f - Nanoseconds (000000000-999999999)
%. - Subseconds with automatic precision

Time Zone:
%z - UTC offset (+HHMM or -HHMM)
%Z - Time zone name (if available)
%:z - UTC offset with colon (+HH:MM or -HH:MM)

Weekday:
%w - Weekday as decimal (0=Sunday, 6=Saturday)
%a - Weekday as abbreviated name (Sun, Mon, ..., Sat)
%A - Weekday as full name (Sunday, Monday, ..., Saturday)

Week Numbers:
%W - Week number (Monday as first day of week)
%U - Week number (Sunday as first day of week)

Common Examples:
%Y-%m-%d %H:%M:%S           # 2024-01-15 14:30:45
%Y-%m-%dT%H:%M:%S%z         # 2024-01-15T14:30:45+0000
%Y-%m-%d %H:%M:%S%.f        # 2024-01-15 14:30:45.123456
%b %d %H:%M:%S              # Jan 15 14:30:45 (syslog format)
%d/%b/%Y:%H:%M:%S %z        # 15/Jan/2024:14:30:45 +0000 (Apache format)
%Y-%m-%d %H:%M:%S,%3f       # 2024-01-15 14:30:45,123 (Python logging)

For complete format reference, see:
https://docs.rs/chrono/latest/chrono/format/strftime/index.html
"#;
    println!("{}", help_text);
}

/// Print available Rhai functions help
fn print_functions_help() {
    let help_text = rhai_functions::docs::generate_help_text();
    println!("{}", help_text);
}

/// Print Rhai scripting guide
fn print_rhai_help() {
    let help_text = r#"
Rhai Scripting Guide for Kelora:

BASIC CONCEPTS:
  e                                    Current event (renamed from 'event')
  e.field                              Access field directly
  e.nested.field                       Access nested fields
  e.scores[1]                          Array access (supports negative indexing)
  e.headers["user-agent"]              Field access with special characters

VARIABLE DECLARATION:
  let myfield = e.col("1,2")           Always use 'let' for new variables
  let result = e.user.name.lower()     Chain operations and store result

FIELD EXISTENCE AND SAFETY:
  "field" in e                         Check if field exists
  "user" in e && "role" in e.user      Check nested field existence
  e.scores.len() > 0                   Check if array has elements
  type_of(e.field) != "()"             Check if field has a value

FIELD AND EVENT REMOVAL:
  e.field = ()                         Remove individual field
  e = ()                               Remove entire event (filters out)

KELORA-SPECIFIC FUNCTIONS:
  Use --help-functions to see all available functions for log processing:
  regex operations, IP handling, text manipulation, JSON parsing, 
  key-value extraction, array processing, safe field access, and utilities.

METHOD CHAINING EXAMPLES:
  e.message.extract_re("user=(\\w+)").upper()
  e.client_ip.mask_ip(2)
  e.url.extract_domain().lower()
  e.timestamp.parse_ts().format("%H:%M")

FUNCTION VS METHOD SYNTAX:
  extract_re(e.line, "\\d+")           Function style (avoids conflicts)
  e.line.extract_re("\\d+")            Method style (better for chaining)

Both syntaxes work identically. Use method syntax for readability and chaining,
function syntax when method names conflict with field names.

COMMON PATTERNS:
  # Safe field access with defaults
  let user_role = if "user" in e && "role" in e.user { e.user.role } else { "guest" };
  
  # Process arrays safely
  if e.events.len() > 0 {
      e.latest_event = e.events[-1];
      e.event_types = unique(e.events.map(|event| event.type));
  }
  
  # Conditional event removal
  if e.level != "ERROR" { e = (); }
  
  # Field cleanup and transformation
  e.password = (); e.ssn = ();  // Remove sensitive fields
  e.summary = e.method + " " + e.status;

ARRAY PROCESSING:
  sorted(e.scores)                     Sort array numerically/lexicographically
  reversed(e.items)                    Reverse array order
  unique(e.tags)                       Remove duplicate elements
  sorted_by(e.users, "age")            Sort array of objects by field
  e.tags.join(", ")                    Join array elements

JSON ARRAY HANDLING:
  JSON arrays are automatically converted to native Rhai arrays with full
  functionality (sorted, reversed, unique, etc.) while maintaining proper
  JSON types in output formats.

SIDE EFFECTS IN QUIET MODE:
  print("debug info")                  Remains visible even with --quiet
  eprint("error details")              Stderr output preserved with --quiet
  # File operations also preserved in --quiet mode

ERROR HANDLING:
  Kelora uses resilient processing by default:
  • Parse errors: Skip malformed lines, continue processing
  • Filter errors: Evaluate to false, skip event
  • Transform errors: Return original event unchanged
  Use --strict for fail-fast behavior on any error.

For complete function reference: kelora --help-functions
For usage examples: kelora --help (see examples section)
For time format help: kelora --help-time
"#;
    println!("{}", help_text);
}

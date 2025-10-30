# Quickstart

Get Kelora running in 5 minutes with three commands.

## Installation

[**Download**](https://github.com/dloss/kelora/releases) the latest release from [GitHub](https://github.com/dloss/kelora), extract it and put it on your PATH. Or install via Cargo:

```bash
cargo install kelora
```


## Three Essential Commands

### 1. Parse and display structured logs

=== "Command"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl --take 5 
    ```

=== "Log Data"

    ```bash exec="on" result="json"
    cat examples/simple_json.jsonl
    ```

Parse JSON logs and display them in a readable format.

### 2. Filter events and select fields

=== "Command"


    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -l error -k timestamp,service,message
    ```

=== "Log Data"

    ```bash exec="on" result="json"
    cat examples/simple_json.jsonl
    ```

Show only errors with specific fields.

### 3. Track metrics across events

=== "Command"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl \
      -e 'track_count(e.level)' \
      --stats-only --metrics
    ```

=== "Log Data"

    ```bash exec="on" result="json"
    cat examples/simple_json.jsonl
    ```

Count events by log level.

## Get Help

```bash
kelora --help              # Complete CLI reference
kelora --help-examples     # More usage patterns
kelora --help-rhai         # Rhai scripting guide
kelora --help-functions    # All built-in Rhai functions
kelora --help-time         # Timestamp format reference
```

## Next Steps

You've seen Kelora in action. Now **learn how it actually works**:

- **[Tutorial: Basics](tutorials/basics.md)** - Comprehensive 30-minute guide explaining input formats (`-f`, `-j`), display options (`-k`, `-b`, `-c`), level filtering (`-l`, `-L`), output formats (`-F`, `-J`), and common workflows
- **[Tutorial: Scripting Transforms](tutorials/scripting-transforms.md)** - Write custom filters and transformations with Rhai
- **[How-To Guides](how-to/find-errors-in-logs.md)** - Solve specific problems like finding errors, parsing custom formats, and tracking metrics

For format conversion, time filtering, metrics tracking, and more advanced features, start with the [Tutorial: Basics](tutorials/basics.md).

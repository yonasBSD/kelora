# Kelora

**Scriptable log processor for the command line.**

Parse messy logs into structured events, then filter, transform, and analyze them with embedded [Rhai](https://rhai.rs) scripting.

!!! note "Development Status"
    Pre-1.0 software using AI-generated code. Validated through automated testing, not manual review. Breaking changes may occur without migration paths. Backed by 770+ tests plus cargo-audit/deny; see the [Security Policy](https://github.com/dloss/kelora/blob/main/SECURITY.md) for details.

![Kelora hero demo](screenshots/hero.gif)
*Filtering noisy logs, parsing custom formats, and visualizing log levels*

## What It Does

Parse any log format, filter with expressions, transform with 100+ functions, track metrics, analyze in context with sliding windows.

**Parse custom formats and extract structured data** - No regex wrestling:

=== "Command/Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -f 'cols:ts level service request_id *message' examples/release_pipe.log \
      --cols-sep '|' \
      --levels warn,error \
      --exec 'e.merge(e.message.parse_kv())' \
      -F json
    ```

=== "Log Data"

    ```bash exec="on" result="ansi"
    cat examples/release_pipe.log
    ```

**Detect error bursts with sliding windows** - Analyze events in context:

=== "Command/Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -f json examples/deploy_tail.jsonl \
      --window 15 \
      --exec 'let recent = window_values(window, "level");
              if recent.filter(|lvl| lvl == "ERROR").len() >= 3 {
                eprint("burst detected at " + e.timestamp);
              }' \
      -F none
    ```

=== "Log Data"

    ```bash exec="on" result="ansi"
    cat examples/deploy_tail.jsonl
    ```

## Works Well With

Kelora thrives in Unix pipelines. Stream logs from kubectl, tail, or journalctl into Kelora, then pipe output to jq, SQLite, qsv, or visualization tools. See [Integrate Kelora with External Tools](how-to/integrate-external-tools.md) for 18 tools and usage patterns.

## Get Started

**[→ Quickstart (5 minutes)](quickstart.md)** - Install and run your first commands

**[→ Tutorial: Basics (30 minutes)](tutorials/basics.md)** - Learn input formats, filtering, and output

**[→ How-To Guides](how-to/find-errors-in-logs.md)** - Solve specific problems

For deeper understanding, see [Concepts](concepts/pipeline-model.md). For complete reference, see [Functions](reference/functions.md), [Formats](reference/formats.md), and [CLI options](reference/cli-reference.md).

## License

Kelora is open source software licensed under the [MIT License](https://github.com/dloss/kelora/blob/main/LICENSE).

# Regex Input Spec Support

## Overview

Kelora currently ships a small set of built-in parsers that are selected via
`--input-format`. Users often maintain bespoke log-pattern definitions (for
example, lnav format JSON files) and want Kelora to parse those logs without
rewriting patterns or fighting CLI escaping. This specification describes how
Kelora will consume external regex specifications, expose them as first-class
input formats, and surface helper APIs for scripting workflows.

The feature introduces:

- A new `spec` input format syntax that accepts a path (and optional format and
  variant names) and feeds imported regexes into the pipeline as if they were
  built-in.
- A shared internal model for “regex specs” so that lnav schemas and lighter
  custom schemas can coexist.
- Automatic registration of Rhai helper functions so advanced scripts can reuse
  loaded specs without manual glue code.
- CLI affordances for discovery (`--list-spec`) and aliasing.

## Goals

- Allow users to reference external regex spec files directly from
  `--input-format`.
- Support both lnav format JSON files and a lightweight Kelora-native schema.
- Keep pattern evaluation deterministic, performant, and opt-in (Kelora does
  not bundle third-party patterns).
- Provide zero-Rhai setup for the 80% case while still exposing functions for
  custom pipelines.
- Offer clear error messages and discovery tooling so users can troubleshoot
  missing files, unknown formats, or compilation errors.

## Non-Goals

- Shipping third-party regex assets in the Kelora tree.
- Implementing full lnav semantics (rewriter programs, SQL snippets, etc.);
  only fields relevant to parsing and metadata are imported.
- Replacing existing built-in parsers or changing their behavior.
- Introducing arbitrary scripting access to files outside the workspace.

## Terminology

- **Regex spec**: A file that defines one or more named regular-expression
  patterns along with metadata.
- **Format**: The top-level key inside a spec (e.g. `"access_log"` inside an
  lnav JSON file).
- **Variant**: A single named regex within a format (e.g. `"ts-first"`).
- **Spec handle**: The identifier Kelora uses internally (`path + format`).

## CLI Surface

### `--input-format`

Extend the existing option to accept `spec` sources:

```
kelora --input-format spec=<path>[:<format>[:<variant>]] <files>

# Shorthand (the `spec=` prefix is optional when the argument contains a path)
kelora --input-format <path>[:<format>[:<variant>]] <files>
```

Behavior:

- `<path>` can be absolute or relative to the invocation directory.
- `<format>` is optional; when omitted, the spec must expose exactly one format.
- `<variant>` is optional; when omitted, Kelora tries the available regexes in a
  deterministic order (see **Runtime Behavior**).
- The argument may be supplied multiple times to process input files with
  different specs sequentially (mirroring existing `--input-format` semantics).

### `--list-spec`

New command-line option (mutually exclusive with processing) that prints the
formats and variants available in one or more spec files:

```
kelora --list-spec access_log.json
kelora --list-spec access_log.json:access_log
```

Output includes:

- Path and format names.
- Variant names sorted by evaluation priority.
- Indicators for default variant (if defined) and metadata such as description
  and multiline flag.

### Help Updates

- `kelora --help` gains a short note under *Input Options* describing the new
  syntax (e.g. `--input-format path[:format[:variant]]`).
- `--help-functions` lists the new Rhai helpers (described below).

## Spec Discovery and Aliases

- Users may create `.kelorarc` aliases that pre-populate the spec argument,
  e.g. `access-log = "--input-format ~/specs/access_log.json:access_log"`.
- In config files, spec-based formats can be used anywhere a standard
  `--input-format` value is accepted.

## Spec Resolution and Caching

1. Parse CLI/config arguments and collect all distinct spec handles.
2. For each handle, load and parse the backing file exactly once per run.
   - File contents are keyed by absolute path to avoid duplicate loads when an
     alias and direct CLI argument refer to the same file.
3. Convert the parsed data into a `RegexSpec` struct containing:
   - `id`: canonical identifier (`path#format`).
   - `title`, `description`, optional `url`.
   - `multiline` flag (optional; defaults to `false`).
   - Ordered list of `RegexVariant { name, pattern, priority, notes }`.
4. Compile all regex patterns up front using `regex::Regex` and store them in a
   lookup map for the runtime parser. Invalid patterns produce a fatal CLI error
   that lists the offending variant and source path.
5. Store lightweight metadata (title, variant names, defaults) inside `conf`
   under `conf.regex_specs[id]` for scripting purposes.

## Supported Schemas

### lnav Format JSON

- Auto-detected via `$schema` or known keys (`regex`, `value`, `sample`).
- Kelora consumes:
  - `regex` table: every entry becomes a variant. Keys containing
    `"module-format"` are honoured (treated like standard variants but flagged in
    metadata).
  - `title`, `description`, `url`, `multiline`.
  - Variant `pattern` string. Additional keys (e.g. `module-format`) are stored
    in metadata but do not change parsing behavior.
- Unused lnav fields (value metadata, SQL rewriters) are ignored but retained in
  metadata for potential future features.

### Kelora Lightweight Schema

JSON or TOML document with the following structure:

```
{
  "title": "My Firewall Logs",
  "description": "Primary and fallback patterns for foo-service",
  "multiline": false,
  "default": "primary",
  "patterns": {
    "primary": {
      "pattern": "^(?<ts>\\S+) (?<host>\\S+) ...",
      "priority": 100,
      "notes": "Matches new log format introduced in 2024"
    },
    "legacy": {
      "pattern": "^(?<host>\\S+) - - \\[...",
      "priority": 50
    }
  }
}
```

Field semantics:

- `title`, `description`, `notes`: optional strings.
- `multiline`: boolean; defaults to `false`.
- `default`: name of the variant to prefer when auto-matching (optional).
- `patterns`: required object mapping variant names to definitions.
- `priority`: integer; higher values are matched first. Defaults to `0` if
  omitted.
- `pattern`: required string. Escaping follows standard JSON/TOML rules (no
  need for double escaping in the CLI).

The loader auto-detects JSON vs TOML based on extension (`.json`, `.toml`,
`.yaml`, `.yml`). Non-matching extensions fall back to attempting JSON first
and then TOML.

## Runtime Behavior

1. When `--input-format` references a spec, the parser becomes a thin wrapper
   over the compiled regex set.
2. For each line/Event:
   - Attempt variants in descending `priority` order.
   - Use the declared `default` as the first candidate when specified; remaining
     variants follow priority ordering.
   - On match, create an `Event` with fields taken from the named capture groups
     (same semantics as `parse_re`). The original line remains available via
     `meta.line`.
   - Insert `__regex_spec` (spec id) and `__regex_variant` (matched variant name)
     into the event map for traceability.
3. If no pattern matches:
   - Emit the event unchanged (consistent with other parsers failing to parse),
     and populate `__regex_spec_error` metadata with the list of attempted
     variants. Users can choose to filter these in subsequent stages.
4. When `multiline` is `true`, reuse Kelora’s existing multiline staging logic
   with the spec-provided settings (initial implementation may simply warn that
   multiline is not yet supported; see **Open Questions**).

## Rhai Integration

Loading a spec automatically registers helper functions:

- `parse_spec(line, spec_id)` → `Map`
- `parse_spec(line, spec_id, variant)` → `Map`
- `spec_variants(spec_id)` → `Array<String>` (ordered by evaluation priority)
- `spec_metadata(spec_id)` → `Map` (title, description, multiline, etc.)

`spec_id` uses the canonical `path#format` notation. Helper functions return an
empty map for unknown specs or variants.

Additionally, the loaded metadata is exposed in `conf.regex_specs` for scripts
that need to inspect available specs during `--begin`.

## Errors and Diagnostics

- Missing file → fatal error before pipeline execution, message includes the
  requested path.
- Unknown format name → fatal error listing available formats in the file.
- Unknown variant → fatal error listing variants for that format.
- Regex compilation failure → fatal error showing file, format, variant, and
  the regex engine’s error message.
- `--list-spec` outputs warnings (but not fatal errors) for variants that fail
  to compile.

## Testing Strategy

- **Unit tests** for the spec loader covering:
  - lnav JSON ingestion (happy path + malformed cases).
  - Lightweight schema ingestion (JSON and TOML).
  - Priority ordering, default variant handling, duplicate names.
- **Integration tests** in `tests/` that:
  - Run Kelora with `--input-format` pointing to fixtures in `example_logs/`
    and assert parsed fields.
  - Exercise `--list-spec` output via command snapshots.
  - Validate Rhai helper availability by running a simple script with
    `parse_spec`.
- Update `help-screen.txt` and regenerate via `cargo run -- --help > ...`.

## Documentation Updates

- Add a new section to the user guide (e.g. `docs/input-specs.md`) explaining
  how to author lightweight specs, use lnav files, and register aliases.
- Reference the feature in `README` and `EXAMPLES.md` with a short example.
- Mention the helper functions in `docs/rhai.md` (if present) and ensure
  `--help-functions` lists them.

## Open Questions / Follow-up Work

- Multiline handling: lnav specs expose rich multiline configuration. Initial
  support can treat `multiline: true` as an error or warning; full support may
  require additional pipeline plumbing.
- Spec verification tooling: optional future command (e.g. `kelora
  verify-spec`) to run regexes against sample logs.
- Caching across runs: consider persisting compiled regexes if load time becomes
  a bottleneck.


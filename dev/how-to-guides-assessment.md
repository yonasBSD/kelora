# How-To Guides Assessment & Improvement Plan

## Context
- Reviewed all guides under `docs/how-to/` plus companion materials in `docs/tutorials/`, `docs/concepts/`, `README.md`, and the examples catalog referenced in `examples/README.md`.
- Cross-checked coverage against key Kelora capabilities: format handling (`-f`, `-j`, csv/tsv variants), streaming and parallel modes (`--parallel`, `--batch-*`, quiet levels), Rhai scripting helpers (`track_*`, `emit_each`, `pseudonym`, masking), span processing (`--span`, `--span-close`), and multiline strategies (`--multiline`).
- Objective: retain the overall docs structure (Tutorials → How-To → Concepts → Reference) while turning the How-To collection into actionable, task-focused playbooks that complement rather than duplicate tutorials and references.

## What’s Working
- **Breadth of coverage:** current guides touch most advanced features (parallel mode, spans, sanitisation, fan-out, real-time alerting).
- **Command-first examples:** every page provides runnable snippets, which aligns with CLI-first workflows.
- **Cross-linking stubs exist:** several guides already point to related tutorials or references, so readers have a starting point for deeper dives.

## Key Issues & Gaps
- **Too many “kitchen sink” lists:** most pages read like exhaustive option dumps instead of problem-solving walkthroughs. Readers get commands but not decision guidance, prerequisites, or expected outcomes.
- **Overlap with tutorials/reference:** topics such as metrics, spans, multiline, and CSV type annotations repeat substantial material from `docs/tutorials/` and `docs/reference/`. This increases maintenance cost and risks divergence.
- **Lack of scenario framing:** guides rarely state who the task is for (ops on-call, data engineer, security analyst) or the log characteristics. Without narrative, the “Solutions” sections feel arbitrary.
- **Inconsistent use of sample data:** some commands use `examples/` fixtures, others reference generic `/var/log/*.log`. New users cannot reproduce results without editing paths.
- **Sparse guidance on feature trade-offs:** advanced options (e.g., `--unordered`, `pseudonym` domain separation, span late-event handling) are mentioned without explaining when to choose them or pitfalls.
- **Index is a flat list:** no clustering by job-to-be-done, so users must skim every page to find relevant help.
- **Out-of-band automation advice:** guides like `build-streaming-alerts.md` and `batch-process-archives.md` include shell scripts, cron, and systemd snippets better suited for appendices or “integration patterns”, making the core task harder to parse.

## Guide-by-Guide Notes
- **find-errors-in-logs.md:** Valuable starter topic but mixes basic (`-l error`) with niche recipes (regex extraction, exit-code automation). Needs a triage storyline (identify, add context, summarise) and tighter scope.
- **analyze-web-traffic.md:** Good focus but bloated; repeats metrics patterns verbatim. Missing guidance on log format quirks (combined vs custom fields, `request_time` availability).
- **monitor-application-health.md:** Significant overlap with Tutorials (`metrics-and-tracking.md`). Should centre on building a lightweight health dashboard or error budget view, referencing tutorial for metric mechanics.
- **parse-syslog-files.md:** Solid baseline but could highlight `--help-time`, facility/severity translation, and structured-data elements. Consider adding a troubleshooting section for RFC 5424 vs 3164 quirks.
- **handle-multiline-stacktraces.md:** Useful but lacks decision tree for choosing `timestamp` vs `indent` vs `regex`. Needs explicit mention of `docs/concepts/multiline-strategies.md`.
- **extract-and-mask-sensitive-data.md:** Strong content yet overwhelming. Should break into focused sub-tasks (masking IPs, pseudonymising IDs, validating redaction) with guidance on secrecy management (`KELORA_SECRET` lifecycle).
- **process-csv-data.md:** Mostly reference material. Could evolve into “Prepare CSV exports for analytics” with steps: declare types, filter/transform, export.
- **fan-out-nested-structures.md:** Duplicates tutorial coverage; should focus on a concrete scenario (e.g., order → items) and best practices for preserving parent context.
- **build-streaming-alerts.md:** Mixing core alerting flows with large integration catalog obscures the main job. Recommend a concise how-to for wiring Kelora into an alert loop, plus an appendix (or separate integration page) for systemd/cron examples.
- **batch-process-archives.md:** Needs objective-driven structure (e.g., “Crunch daily archives quickly”). Current copy is almost a performance tuning reference.
- **span-aggregation-cookbook.md:** Material belongs closer to advanced tutorial or reference. As a how-to, it should answer “How do I roll up logs into 5‑minute summaries?” with a single tight example and troubleshooting for late events.

## Proposed Restructure

| Current Guide | Proposed Action | Rationale / Notes |
| --- | --- | --- |
| `find-errors-in-logs.md` | **Rewrite** as “Triage production errors” | Keep core filtering/context tricks, add incident workflow, link to `docs/concepts/error-handling.md`. |
| `analyze-web-traffic.md` | **Condense & refocus** | Prioritise 3 outcomes: errors, latency, hotspots. Use `examples/simple_combined.log` plus note on custom Nginx fields. |
| `monitor-application-health.md` | **Merge into** new “Build a service health snapshot” guide | Remove metric primer duplication; lean on tutorial for mechanics. |
| `parse-syslog-files.md` | **Keep & tighten** | Add scenario (auth investigation), clarify facility/severity tables with `--help-time` pointers. |
| `handle-multiline-stacktraces.md` | **Rewrite with decision guide** | Provide choose-your-strategy flowchart, link to concept doc, add validation steps. |
| `extract-and-mask-sensitive-data.md` | **Split into two guides** (“Sanitise logs before sharing”, “Pseudonymise identifiers for analytics”) | Reduces overload; emphasise secrets management, validation checklists. |
| `process-csv-data.md` | **Refocus on export pipeline** | Step-by-step: enforce schema, clean/transform, export to CSV/JSON for downstream tools. |
| `fan-out-nested-structures.md` | **Integrate into** broader “Flatten nested JSON for analysis” | Align with `emit_each` best practices and tracking guidance. |
| `build-streaming-alerts.md` | **Rewrite core guide + move integrations** | Core: design alert pipeline + quiet modes + exit codes. Move Slack/PagerDuty/systemd recipes to dedicated “automation appendix” or `docs/reference/integrations.md`. |
| `batch-process-archives.md` | **Reframe as performance playbook** | Scenario: “Process month of archives quickly and safely.” Include decision table for `--parallel`, `--unordered`, batch tuning. |
| `span-aggregation-cookbook.md` | **Promote to advanced tutorial** or **rewrite as focused how-to** | If staying, constrain to 1 count-based + 1 time-based recipe with troubleshooting. |

## Improvement Plan

1. **Inventory & Clustering (0.5 day)**
   - Confirm final task categories (e.g., *Operational Triage*, *Format Ingestion*, *Data Hygiene*, *Automation & Alerting*, *Batch Analytics*).
   - Update `docs/how-to/index.md` with grouped headings once content lands.
   - Decide which dense sections migrate to tutorials/reference (e.g., span cookbook).

2. **Foundational Rewrites (2–3 days)**
   - Prioritise high-traffic tasks: Errors/Triage, Web Traffic, Service Health, Streaming Alerts.
   - For each, create structure: **Scenario → Prerequisites → Steps → Variations → Validation → See also**.
   - Standardise sample data usage: prefer `examples/` fixtures with explicit paths; add “Swap in your logs” notes.
   - Embed decision tables where options exist (`--levels` vs `--filter`, `--parallel` vs sequential).

3. **Specialist Workflows (2 days)**
   - Rewrite/split Data Hygiene, CSV Export, Nested JSON guides following same template.
   - Add checklists for compliance steps (secret rotation, validation commands).
   - Reference relevant concept docs (`docs/concepts/pipeline-model.md`, `docs/concepts/multiline-strategies.md`) and CLI help (`--help-functions`, `--help-multiline`).

4. **Performance & Advanced Topics (1–1.5 days)**
   - Rework Batch Archives guide with emphasis on performance trade-offs, linking to `docs/concepts/performance-model.md`.
   - Decide whether Span Aggregation remains a how-to or migrates to Tutorials; update links accordingly.
   - Provide benchmarking workflow referencing `just bench` when performance changes are suggested.

5. **Integration & QA (0.5 day)**
   - Ensure each guide points to “Next steps” (e.g., tutorials, concept docs, examples).
   - Run `mkdocs serve` locally to proof-read formatting; verify tabbed blocks render correctly.
   - Harmonise emoji/quiet mode references with guidelines in `AGENTS.md` (respecting `--no-emoji` note).
   - Collect TODOs for future automation appendix if integration snippets are relocated.

6. **Content Governance**
   - Add authoring checklist (style, scenario framing, sample data consistency) to `dev/docs-guidelines.md` (new or existing) to prevent regressions.
   - Consider adding automated link checker or spell checker in CI for docs (outside immediate scope but worth tracking).

## Quick Wins Before Full Rewrite
- Add task-oriented intros + expected outcomes to each existing page to improve readability immediately.
- Insert “Use `--help-*` for detailed flag/function descriptions” callouts to push deep details to reference.
- Update index to group guides under interim headings without waiting for full rewrites.
- Replace ad-hoc `/var/log/...` paths with `examples/` fixtures or clearly mark as system paths requiring privileges.


# Kelora Documentation Architecture Plan

## Executive Summary

This plan outlines a comprehensive documentation system for Kelora using Material for MkDocs + mike versioning + executable code examples. The structure follows the Diátaxis framework (tutorials/how-tos/reference/explanation) and focuses on solving real user problems rather than mechanically documenting features.

## Stack

- **Material for MkDocs** - Modern, searchable, mobile-friendly theme
- **mike** - Git-based versioning (frozen old versions, no maintenance burden)
- **markdown-exec** - Run actual kelora commands during build, capture real output
- **pymdownx.snippets** - Include file snippets from examples/ directory

## Documentation Structure

```
docs/
├── index.md                          # Landing page
│                                     # "Scriptable log processor - parse, filter, transform"
│                                     # 3 live examples showing value
│                                     # Install + "5-minute quickstart" CTA
│
├── quickstart.md                     # FAST WIN: 5 minutes to first success
│                                     # Parse JSON, filter errors, extract fields
│                                     # Uses examples/simple_json.jsonl
│
├── tutorials/                        # LEARNING: Focused learning experiences
│   ├── parsing-custom-formats.md    # Deep dive: cols:<spec> for bespoke logs
│   ├── working-with-time.md         # Timestamps: parsing, filtering, timezones
│   ├── metrics-and-tracking.md      # Track patterns, build dashboards
│   └── scripting-transforms.md      # Rhai scripting: filters, transforms, windows
│
├── how-to/                           # REAL PROBLEMS: Task-oriented solutions
│   # Organized by what user is trying to accomplish
│   ├── find-errors-in-logs.md       # Level filtering, time ranges, context lines
│   ├── analyze-web-traffic.md       # Apache/Nginx: status codes, slow requests, errors
│   ├── monitor-application-health.md # JSON logs: extract metrics, track services
│   ├── parse-syslog-files.md        # Syslog format specifics + auth monitoring
│   ├── handle-multiline-stacktraces.md # Multiline strategies for exceptions
│   ├── extract-and-mask-sensitive-data.md # IP masking, pseudonyms, redaction
│   ├── process-csv-data.md          # CSV/TSV with type conversions
│   ├── fan-out-nested-structures.md # emit_each for arrays, multi-level fan-out
│   ├── build-streaming-alerts.md    # tail -f + filters + metrics
│   └── batch-process-archives.md    # Parallel mode, gzip, performance tuning
│
├── reference/                        # LOOKUP: Quick facts
│   ├── functions.md                 # All 40+ functions, single searchable page
│   │                                # Organized by category with anchor links
│   │                                # Each function: signature + 1-2 examples
│   ├── cli-reference.md             # Flag reference (enhanced --help output)
│   ├── formats.md                   # Format table: when to use each
│   ├── exit-codes.md                # Exit code meanings
│   └── rhai-cheatsheet.md           # Rhai syntax quick reference
│
└── concepts/                         # UNDERSTANDING: How it works
    ├── pipeline-model.md            # Input→Parse→Filter→Transform→Output
    ├── events-and-fields.md         # Event structure, field access patterns
    ├── scripting-stages.md          # --begin, --filter, --exec, --end lifecycle
    ├── error-handling.md            # Resilient vs strict, error recovery
    ├── multiline-strategies.md      # When/why each multiline mode
    ├── performance-model.md         # Sequential vs parallel tradeoffs
    └── configuration-system.md      # Config precedence, aliases
```

## Key Design Decisions

### 1. How-To Organization: Real User Problems

**Problem-focused, not tool-focused:**

✅ **Good** (what users are trying to accomplish):
- "Find errors in logs" → covers filtering across any format
- "Analyze web traffic" → Apache/Nginx specific, real goal
- "Monitor application health" → JSON logs, realistic scenario

❌ **Avoid** (tool-centric documentation):
- "Using the JSON parser" → too abstract
- "Filter command reference" → that's reference, not how-to
- "Format guide" → belongs in reference section

**Each how-to guide**:
- Starts with user's problem: "You have Apache access logs and need to find slow requests..."
- Shows complete working solution with real example file
- Explains key concepts inline (just enough context)
- Links to reference/concepts for deep dives

**Research insight**: Developers search by problem, not documentation type. They know the content domain (web logs, errors, metrics) but don't care if the answer is in "reference" vs "tutorial" vs "examples".

### 2. Tutorial Topics (Focused Learning)

**Four tutorials covering core skills**:

1. **Parsing Custom Formats** (cols:<spec>)
   - Teaches: format specs, column capture, type annotations
   - Problem: "Your app logs aren't standard format"
   - Outcome: Can parse any column-based format

2. **Working With Time**
   - Teaches: --since/--until, --ts-format, timezones, datetime functions
   - Problem: "Filter logs by time range, handle different timestamp formats"
   - Outcome: Confident with all timestamp features

3. **Metrics and Tracking**
   - Teaches: track_count/avg/unique, --metrics, --begin/--end
   - Problem: "Count patterns, build summaries"
   - Outcome: Can extract business metrics from logs

4. **Scripting Transforms**
   - Teaches: Rhai basics, --filter, --exec, field manipulation, windows
   - Problem: "Need to enrich/transform log data"
   - Outcome: Can write complex processing pipelines

**Each tutorial**:
- 10-15 minutes duration
- Hands-on, uses example files from repo
- Progressive: builds on previous concepts
- Ends with "what next" links

### 3. Reference: Keep It Simple

**Single-page function reference**:
- All 40+ functions on one page with category sections
- Material search makes it instantly findable
- Each function: signature + 1-2 line examples
- Anchor links for deep linking: `#string-functions`

**Why single page?**:
- Users search "extract_re" → instant result
- No clicking through navigation hierarchy
- Print-friendly for quick reference
- Can split later if it becomes unwieldy (start simple)

**CLI reference**:
- Enhanced version of --help output
- Organized by pipeline stage (input, filtering, transform, output)
- More examples than CLI version
- Link back to terminal: "Quick ref: kelora --help"

**No auto-generation complexity**: CLI help and web docs serve different needs. CLI help stays concise and terminal-friendly. Web docs go deeper with more examples. Don't try to maintain perfect sync - that's a maintenance trap.

### 4. Concepts Section (Understanding)

**Essential for**:
- Users debugging issues ("why isn't multiline working?")
- Users optimizing ("sequential vs parallel, when?")
- Curious users ("how does the pipeline actually work?")

**Each concept page**:
- Explains one thing deeply
- Uses diagrams where helpful
- Shows examples of impact
- Links to relevant how-tos

**Examples of good concept pages**:
- Pipeline model: Shows flow from input → parse → filter → transform → output
- Error handling: Explains resilient vs strict, when errors abort vs skip
- Multiline strategies: Why each strategy exists, when to use which

### 5. Examples Strategy

**Keep examples/ directory as source of truth**:
- 37 example files stay in repo
- Docs use pymdownx.snippets to include file content
- Docs use markdown-exec to run actual commands against them

**Example in documentation**:
```markdown
## Analyzing Web Traffic

Parse Apache access logs to find server errors:

```bash exec="on" result="ansi"
kelora -f combined examples/web_access_large.log.gz \
  --filter 'e.status >= 500' \
  --keys ip,timestamp,status,request
```

The combined log format includes these fields: ip, timestamp, method, path, status, bytes, referer, user_agent, request_time.
```

**Benefits**:
- Example files tested by integration tests
- Docs always show current version's actual output
- Users can clone repo and run exact commands
- Examples never go stale

### 6. Executable Examples Strategy

**Auto-run during build** (markdown-exec):
- ✅ Quickstart examples (prove they work)
- ✅ Tutorial examples (learners need confidence)
- ✅ Simple how-to examples (common patterns)
- ✅ Format detection demos (show real output)

**Static/manual** (for speed/complexity):
- ❌ Large file processing examples
- ❌ Streaming examples (tail -f can't run at build time)
- ❌ Performance benchmarks (too slow for every build)
- ❌ Multi-step pipelines with temp files (complex setup)

**Validation strategy**:
- Add test script to validate static examples
- Run manually before releases or in CI
- Catches breakage without slowing every doc build
- Balance between automation and practicality

## CLI Help vs Web Docs

**Different media, different needs. Stop trying to sync perfectly.**

### CLI Help (--help-* flags)
- ✅ Keep as-is, they're excellent
- ✅ Quick reference when you're in terminal
- ✅ No internet required
- ✅ Concise, terminal-optimized
- Add one line: "Full documentation: https://kelora.dev"

### Web Docs
- ✅ More examples, more depth
- ✅ Searchable across all pages
- ✅ Links between related topics
- ✅ Can embed images, diagrams, tables
- ✅ Multiple related examples on one page

**Relationship**: CLI help = quick lookup, web docs = learning and deep dives

**No auto-generation**: We tried this, it failed. Manual curation produces better docs. Accept that they'll diverge slightly - that's okay and actually desirable.

## Versioning with mike

### Version Strategy
- `latest` - Alias pointing to newest release (most users)
- `0.6`, `0.7`, `0.8` - Actual version numbers
- `dev` - Built from main branch (bleeding edge)

### Deployment
```bash
# On release (automated via GitHub Actions)
mike deploy --push --update-aliases 0.7 latest

# On main branch push (automated)
mike deploy --push dev
```

### Mike Philosophy
> "Once you've generated your docs for a particular version, you should never need to touch that version again."

**Benefits**:
- Old docs stay frozen forever (no maintenance)
- No breakage from MkDocs updates
- Each version documents its own behavior
- Zero complexity after initial setup

### URL Structure
- https://dloss.github.io/kelora/ → redirects to latest/
- https://dloss.github.io/kelora/latest/ → current stable
- https://dloss.github.io/kelora/0.6/ → specific version
- https://dloss.github.io/kelora/dev/ → bleeding edge

## Implementation Phases

### Phase 1: Foundation (Days 1-3)
**Goal**: Prove the system works end-to-end

- [ ] Create mkdocs.yml with Material theme + plugins
- [ ] Configure mike for versioning
- [ ] Setup markdown-exec with examples/ directory access
- [ ] Create basic structure: index.md + navigation skeleton
- [ ] Write one executable example and verify it runs
- [ ] Deploy test version to gh-pages branch
- [ ] Verify version selector works

### Phase 2: Core Content (Days 4-10)
**Goal**: Ship minimum viable docs that add value

- [ ] Write quickstart.md (most important page!)
- [ ] Write 3-4 key how-tos:
  - find-errors-in-logs.md
  - analyze-web-traffic.md
  - monitor-application-health.md
- [ ] Create single-page function reference (all 40+ functions)
- [ ] Write CLI reference (enhanced --help)
- [ ] Write 2 core concept pages:
  - pipeline-model.md
  - events-and-fields.md

**Ship after Phase 2**: Docs are useful even if incomplete

### Phase 3: Expand (Days 11-20)
**Goal**: Comprehensive coverage

- [ ] Complete remaining how-to guides (7 more)
- [ ] Write all 4 tutorials
- [ ] Complete concepts section (5 more pages)
- [ ] Add remaining reference pages (formats, exit-codes, rhai-cheatsheet)
- [ ] Add executable examples to all tutorials

### Phase 4: Polish (Days 21-25)
**Goal**: Professional finish

- [ ] Add internal cross-links between related pages
- [ ] Improve search metadata (keywords, descriptions)
- [ ] Test on mobile and tablet viewports
- [ ] Setup GitHub Actions for automatic deployment
- [ ] Add diagrams to concept pages
- [ ] Review with fresh eyes, iterate on clarity
- [ ] Write contributing guide for docs

## Technical Configuration

### mkdocs.yml
```yaml
site_name: Kelora
site_description: Scriptable log processor with embedded Rhai scripting
site_url: https://dloss.github.io/kelora/
repo_url: https://github.com/dloss/kelora
repo_name: dloss/kelora

theme:
  name: material
  palette:
    # Light/dark mode toggle
    - scheme: default
      primary: indigo
      accent: indigo
      toggle:
        icon: material/brightness-7
        name: Switch to dark mode
    - scheme: slate
      primary: indigo
      accent: indigo
      toggle:
        icon: material/brightness-4
        name: Switch to light mode
  features:
    - navigation.instant        # Fast page loads
    - navigation.tracking       # Update URL with scroll
    - navigation.tabs          # Top-level tabs
    - navigation.sections      # Expand sections
    - search.suggest           # Search suggestions
    - search.highlight         # Highlight search terms
    - content.code.copy        # Copy code blocks
    - content.code.annotate    # Annotate code with numbers

plugins:
  - search:
      lang: en
  - mike:
      version_selector: true
      alias_type: symlink
  - markdown-exec

markdown_extensions:
  - pymdownx.highlight:
      anchor_linenums: true
  - pymdownx.superfences
  - pymdownx.snippets:
      base_path: [examples]
  - pymdownx.tabbed:
      alternate_style: true
  - admonition
  - pymdownx.details
  - attr_list
  - md_in_html
  - tables
  - toc:
      permalink: true

extra:
  version:
    provider: mike
  social:
    - icon: fontawesome/brands/github
      link: https://github.com/dloss/kelora

nav:
  - Home: index.md
  - Quickstart: quickstart.md
  - Tutorials:
      - tutorials/parsing-custom-formats.md
      - tutorials/working-with-time.md
      - tutorials/metrics-and-tracking.md
      - tutorials/scripting-transforms.md
  - How-To Guides:
      - how-to/find-errors-in-logs.md
      - how-to/analyze-web-traffic.md
      - how-to/monitor-application-health.md
      - how-to/parse-syslog-files.md
      - how-to/handle-multiline-stacktraces.md
      - how-to/extract-and-mask-sensitive-data.md
      - how-to/process-csv-data.md
      - how-to/fan-out-nested-structures.md
      - how-to/build-streaming-alerts.md
      - how-to/batch-process-archives.md
  - Reference:
      - reference/functions.md
      - reference/cli-reference.md
      - reference/formats.md
      - reference/exit-codes.md
      - reference/rhai-cheatsheet.md
  - Concepts:
      - concepts/pipeline-model.md
      - concepts/events-and-fields.md
      - concepts/scripting-stages.md
      - concepts/error-handling.md
      - concepts/multiline-strategies.md
      - concepts/performance-model.md
      - concepts/configuration-system.md
```

### requirements.txt
```txt
mkdocs>=1.5.0
mkdocs-material>=9.4.0
mike>=2.0.0
markdown-exec>=1.8.0
pymdown-extensions>=10.4
```

### GitHub Actions Workflow
```yaml
name: Deploy Docs

on:
  push:
    branches: [main]
    tags: ['v*']
  workflow_dispatch:

permissions:
  contents: write

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # mike needs full history

      - uses: actions/setup-python@v5
        with:
          python-version: '3.11'

      - name: Install dependencies
        run: |
          pip install -r requirements.txt

      - name: Configure Git
        run: |
          git config user.name github-actions
          git config user.email github-actions@github.com

      - name: Deploy dev version
        if: github.ref == 'refs/heads/main'
        run: |
          mike deploy --push dev

      - name: Deploy release version
        if: startsWith(github.ref, 'refs/tags/v')
        run: |
          VERSION=${GITHUB_REF#refs/tags/v}
          mike deploy --push --update-aliases $VERSION latest
```

## Success Metrics

After launch, monitor:

1. **Search queries** - What are users looking for? What's missing?
2. **Most visited pages** - What's actually useful vs assumed useful?
3. **Bounce rate** - Are users finding answers or leaving?
4. **GitHub issues** - Fewer "how do I...?" questions after launch?
5. **Time on page** - Are tutorials engaging or confusing?

Use Material for MkDocs built-in analytics integration or add Google Analytics.

## Research Insights

### Diátaxis Framework
- **Tutorials**: Learning-oriented, take user by the hand
- **How-to guides**: Task-oriented, solve specific problems
- **Reference**: Information-oriented, lookup facts
- **Explanation**: Understanding-oriented, illuminate concepts

Source: https://diataxis.fr/

### Developer Documentation Behavior
Key finding: Developers search by **problem domain**, not documentation type. They know they're working with "web logs" or "errors" but don't distinguish between "tutorial" vs "reference" vs "examples" when searching.

Implication: Organize how-tos by user problem (analyze web traffic, find errors) not by tool feature (use filter flag, use format parser).

Source: Research on documenting code (idratherbewriting.com)

### CLI Documentation Best Practices
- Built-in help must be fast and concise (keep --help screens)
- Web docs can go deep with multiple examples
- Working code examples are essential (developers learn by doing)
- Consistency in structure helps users navigate

Source: Command Line Interface Guidelines (clig.dev)

## What Makes This Plan Work

✅ **User-problem focused** - How-tos match real use cases
✅ **Progressive learning** - Clear path from quickstart → tutorials → how-tos
✅ **Simple reference** - Single-page function lookup, searchable
✅ **Executable examples** - Selective automation, catches breakage
✅ **Concepts included** - Helps users debug and optimize
✅ **No auto-gen complexity** - Manual curation, higher quality
✅ **Realistic scope** - Can ship incrementally, iterate based on usage
✅ **Version management** - Old docs frozen, zero maintenance burden

## Open Questions

1. **Analytics**: Google Analytics vs Plausible vs built-in only?
2. **Feedback mechanism**: Add "Was this helpful?" buttons on pages?
3. **API stability**: Once 1.0, maintain old version docs how long?
4. **Contribution workflow**: How do external contributors submit doc PRs?
5. **Diagram tools**: What to use for pipeline/architecture diagrams?

## Next Steps

1. **Validate with users** - Share structure, get feedback
2. **Start Phase 1** - Prove the technical stack works
3. **Write quickstart** - Most impactful single page
4. **Build function reference** - High value, reusable across docs
5. **Iterate** - Ship Phase 2, learn from usage, improve

---

**Document Status**: Architecture plan, ready for implementation
**Last Updated**: 2025-10-07
**Author**: Claude Code (with user feedback)

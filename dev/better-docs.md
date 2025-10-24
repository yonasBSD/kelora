# Documentation Improvement Plan

This document outlines gaps in Kelora's tutorial documentation and recommends improvements for a better learning journey.

**Status:** Draft recommendations (2025-10-24)

---

## Executive Summary

Current tutorials cover basics, formats, time, metrics, and transforms well. However, there are **critical gaps** in the learning path:

1. **No gentle introduction to Rhai scripting** - users jump from CLI flags to complex transforms
2. **Pipeline lifecycle unclear** - `--begin`, `--end`, and `conf` map lack systematic coverage
3. **Span aggregation underdocumented** - powerful feature hidden in how-to guides
4. **Configuration/reusability not taught** - aliases and scripts only in reference docs

---

## Current State Analysis

### ✅ Well-Covered Topics

| Tutorial | Covers | Quality |
|----------|--------|---------|
| **basics.md** (new) | Input formats, display modifiers, level filtering, output formats | ✅ Good starting point |
| **parsing-custom-formats.md** | Column specs, custom parsers, type annotations | ✅ Comprehensive |
| **working-with-time.md** | Timestamp parsing, time filtering, timezones | ✅ Good coverage |
| **metrics-and-tracking.md** | track_count, track_sum, basic aggregation | ✅ Solid examples |
| **scripting-transforms.md** | Rhai transforms, multi-stage pipelines | ✅ Advanced patterns |

### ⚠️ Partially Covered (how-to/concepts only)

- Window functions (`--window`)
- Context lines (`-A`, `-B`, `-C`)
- Span aggregation (`--span`, `--span-close`)
- Configuration and aliases (`.kelora.ini`, `--save-alias`)
- Parallel processing (`--parallel`)
- Section selection (`--section-from`, `--section-before`)

### ❌ Critical Gaps

1. **The Rhai Gap** - No introduction to basic scripting between basics.md and advanced transforms
2. **Pipeline Lifecycle** - `--begin`, `--end`, `conf` map not taught systematically
3. **Span Aggregation** - Killer feature deserves tutorial, not just cookbook
4. **Reusability** - Configuration and script reuse not covered

---

## Recommended New Tutorials

### 🔴 Priority 1: Critical Gaps

#### 1. Introduction to Rhai Scripting
**Position:** Between basics.md and working-with-time.md
**Time:** ~20 minutes
**Fills gap:** Gentle bridge from CLI flags to scripting

**Topics:**
- Understanding the event object (`e`)
- Simple filter expressions (`e.status >= 400`)
- Basic exec transformations (`e.size_mb = e.bytes / 1024 / 1024`)
- String operations (`e.message.contains("error")`)
- Conditionals (`if e.level == "ERROR" { ... }`)
- Type conversions (`to_int()`, `to_float()`, `to_string()`)
- Pipeline order: why `--exec --filter` differs from `--filter --exec`
- Field access patterns (`e.field`, `e["field"]`, `e.get_path()`)
- Debugging with `-F inspect` and `--verbose`
- Common mistakes and how to fix them

**Learning outcome:** Confidence to write basic filters and transforms

---

#### 2. Pipeline Stages: Begin, Filter, Exec, and End
**Position:** After metrics-and-tracking.md
**Time:** ~15 minutes
**Fills gap:** Core pipeline lifecycle

**Topics:**
- What runs when: `--begin` → (filter/exec)* → `--end`
- The `conf` map for shared state
- Loading lookup data with `read_json()` / `read_lines()` / `read_file()`
- Multiple filters and execs in sequence
- Using `--end` for summaries and reports
- Practical example: Enrich events from a lookup table
- Practical example: Generate report in `--end` stage
- When to use `--begin` vs `--exec`
- `conf` is read-only after `--begin`

**Learning outcome:** Understand full pipeline lifecycle and shared state

---

### 🟡 Priority 2: Important Features

#### 3. ✅ Time-Based Aggregation with Spans (COMPLETED)
**Position:** After pipeline-stages.md
**Time:** ~20 minutes
**Fills gap:** Span aggregation needs proper tutorial
**File:** `docs/tutorials/span-aggregation.md`

**Topics:**
- ✅ What are spans? (non-overlapping time windows or event counts)
- ✅ Count-based spans (`--span 100`)
- ✅ Time-based spans (`--span 5m`, `--span 1h`)
- ✅ The `--span-close` hook and when it runs
- ✅ Accessing `span.events`, `span.metrics`, `span.id`, `span.start`, `span.end`
- ✅ Per-span vs global metrics
- ✅ Practical example: 5-minute rollups of error rates
- ✅ Practical example: Sliding statistics per 1000 events
- ✅ Late events and `meta.span_status`
- ✅ Why spans require sequential mode

**Learning outcome:** Build time-windowed aggregations and rollups

---

#### 4. Configuration and Reusable Scripts
**Position:** After span-aggregation.md
**Time:** ~15 minutes
**Fills gap:** Productivity and reusability

**Topics:**
- Creating aliases with `--save-alias`
- Using aliases with `-a`
- Configuration file locations (project vs user)
- `.kelora.ini` structure and precedence
- Viewing config with `--show-config`
- Editing config with `--edit-config`
- Reusable functions with `-I/--include`
- Complex scripts with `-E/--exec-file`
- When to use aliases vs include files vs exec-files
- Practical example: Team-shared error detection alias
- Practical example: Reusable helper library

**Learning outcome:** Build reusable workflows and share them with teams

---

### 🟢 Priority 3: Nice to Have (Can Stay in How-To)

These are already well-documented in how-to guides and concepts:
- Context lines and window functions
- Parallel processing details
- Advanced multiline strategies

---

## Improvements to Existing Tutorials

### parsing-custom-formats.md

**Add:**
- ✅ CSV with type annotations (`-f 'csv status:int bytes:int'`)
- ✅ Troubleshooting section ("What if my parser fails?")
- ✅ Demonstrate `-f auto` behavior and limitations
- ✅ Show how to debug with `--stats` and discovered fields

**Fix:**
- Check for any auto-detection by filename claims

---

### working-with-time.md

**Add:**
- ✅ Duration calculations between events
- ✅ Show `--mark-gaps` for visualizing time gaps in output
- ✅ More examples of chrono format strings
- ✅ Common timezone mistakes and solutions
- ✅ Timestamp conversion with `--convert-ts`

---

### metrics-and-tracking.md

**Add:**
- ✅ `track_bucket()` examples for histograms/percentiles
- ✅ `track_unique()` examples for cardinality
- ✅ `--metrics-file` for persisting to disk
- ✅ Accessing metrics in `--end` stage
- ✅ Per-worker metrics merging in parallel mode

---

### scripting-transforms.md

**Improve:**
- ✅ Add debugging section (`--verbose`, `-F inspect`, error messages)
- ✅ Show `--strict` vs resilient mode differences
- ✅ Add "Common Mistakes" section
- ❓ Consider splitting into "Basic Transforms" and "Advanced Patterns"?

---

### basics.md

**Verify:**
- ❌ Remove any auto-detection by filename claims (already correct)
- ✅ Add cross-links to next tutorials
- ✅ Ensure all examples run correctly

---

## Recommended Learning Path (After Changes)

```
Phase 1: Foundation
├─ 1. basics.md                         ← Input, display, filtering ✅
├─ 2. intro-to-rhai.md (NEW)           ← Simple scripts, --filter, --exec
└─ 3. working-with-time.md              ← Time filtering

Phase 2: Intermediate
├─ 4. metrics-and-tracking.md           ← Aggregation basics
├─ 5. pipeline-stages.md (NEW)          ← --begin, --end, conf map
└─ 6. scripting-transforms.md           ← Complex transforms

Phase 3: Advanced
├─ 7. parsing-custom-formats.md         ← Custom formats ✅
├─ 8. span-aggregation.md (NEW)         ← Time windows ✅
└─ 9. configuration-and-reusability.md (NEW) ← Aliases, reusability ✅

Phase 4: Specialized
└─ How-To Guides for specific problems
```

**Total learning time:** ~3-4 hours for complete journey

---

## Quick Wins (Low Effort, High Impact)

### 1. Fix Auto-Detection Claims
**Effort:** Low
**Impact:** High (prevents confusion)

Search all docs for incorrect claims like:
- ❌ "Kelora auto-detects .jsonl files"
- ❌ "The .log extension is automatically parsed as syslog"

**Correct behavior:**
- ✅ Default is always `-f line` (plain text)
- ✅ Only `-f auto` examines content (not filename)

---

### 2. Add Cross-Links Between Tutorials
**Effort:** Low
**Impact:** Medium

Each tutorial should have:
- **Prerequisites section:** "Before starting, complete [basics.md]"
- **Next steps section:** "Continue to [working-with-time.md] or see [how-to/find-errors]"
- **Related guides:** Links to relevant how-to guides and concepts

---

### 3. Standardize Tutorial Structure
**Effort:** Low
**Impact:** Medium

All tutorials should follow this structure:

```markdown
# Tutorial Title

One-sentence description.

## What You'll Learn
- Bullet points of skills gained

## Prerequisites
- Links to required prior knowledge
- Estimated time: ~15 minutes

## Sample Data
- Which example files are used
- How to get them

## Step-by-step content
...

## Summary
- Quick recap of what was learned

## Next Steps
- Where to go next
- Related guides
```

---

### 4. Add "Common Mistakes" Sections
**Effort:** Low
**Impact:** High

Show common errors and solutions:

**Input/Format Mistakes:**
- ❌ Forgetting `-j` for JSON → see `line='{"json":"here"}'`
- ❌ Using `-f auto` on stdin → no peeking possible
- ✅ Solution: Be explicit with `-j` or `-f json`

**Pipeline Order Mistakes:**
- ❌ `--filter 'e.slow' --exec 'e.slow = e.duration > 1000'` → undefined field
- ✅ `--exec 'e.slow = e.duration > 1000' --filter 'e.slow'` → correct order

**Type Mistakes:**
- ❌ `e.status >= 400` when status is string
- ✅ `e.status.to_int() >= 400` or parse format with types

**Timezone Mistakes:**
- ❌ Comparing UTC and local timestamps
- ✅ Use `--input-tz` or normalize in script

---

## Implementation Priorities

### Immediate (Next Week)
1. Create **intro-to-rhai.md** tutorial (highest impact)
2. Fix auto-detection claims across all docs
3. Add cross-links and "Next Steps" to existing tutorials

### Short-term (Next Month)
4. Create **pipeline-stages.md** tutorial
5. Improve **metrics-and-tracking.md** (add track_bucket, track_unique)
6. Add "Common Mistakes" sections to all tutorials

### Medium-term (Next Quarter)
7. Create **span-aggregation.md** tutorial
8. Create **configuration.md** tutorial
9. Improve **working-with-time.md** (add duration examples)
10. Standardize all tutorial structures

---

## Success Metrics

How do we know documentation is better?

1. **User feedback:** Fewer "how do I...?" questions about covered topics
2. **Tutorial completion:** Users successfully complete tutorials without getting stuck
3. **Feature adoption:** Increased use of `--begin`, `--end`, spans, aliases
4. **Error reduction:** Fewer common mistakes (missing `-j`, wrong pipeline order)
5. **Learning time:** Users can go from zero to productive in ~3-4 hours

---

## ✅ Implementation Status: COMPLETE

**All priority tutorials have been implemented!**

The complete learning path is now in place:

1. ✅ **intro-to-rhai.md** - Introduction to Rhai Scripting (Priority 1)
2. ✅ **pipeline-stages.md** - Pipeline lifecycle with --begin/--end (Priority 1)
3. ✅ **span-aggregation.md** - Time-Based Aggregation with Spans (Priority 2)
4. ✅ **configuration-and-reusability.md** - Configuration and Reusable Scripts (Priority 2)

The learning journey now has a complete, gentle progression from basics through advanced patterns:
- ✅ basics.md → intro-to-rhai.md bridges the CLI-to-scripting gap
- ✅ pipeline-stages.md covers --begin, --end, and conf map
- ✅ span-aggregation.md teaches time windows and rollups
- ✅ configuration-and-reusability.md enables reusable workflows

**Total learning time:** ~3-4 hours for complete journey (as planned)

---

## Appendix: Tutorial Template

```markdown
# Tutorial Title

Brief one-sentence description of what this tutorial teaches.

## What You'll Learn

- Skill 1
- Skill 2
- Skill 3

## Prerequisites

- [Tutorial Name](link.md) - Why it's needed
- Basic command-line knowledge
- **Time:** ~15 minutes

## Sample Data

This tutorial uses:
- `examples/file.jsonl` - Description of data

If you haven't cloned the repo:
```bash
git clone https://github.com/dloss/kelora && cd kelora
```

## Step 1: Topic Name

Brief introduction to the concept.

=== "Command"

    ```bash
    kelora -j examples/file.jsonl --option
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/file.jsonl --option
    ```

Explanation of what happened.

## Step 2: Next Topic

Continue pattern...

## Summary

Quick recap:
- Learned X
- Practiced Y
- Can now Z

## Common Mistakes

**Problem:** Description
**Symptom:** What the user sees
**Solution:** How to fix it

## Next Steps

- **[Next Tutorial](next.md)** - What comes next
- **[Related Guide](guide.md)** - For deeper dive
- **[How-To](howto.md)** - Practical application
```

---

## Notes

- All tutorials should use `markdown-exec` format for executable examples
- Keep tutorials focused (15-20 minutes max)
- Use real example files from `examples/`
- Show both command and output
- Link liberally to reference docs
- Test all commands before committing

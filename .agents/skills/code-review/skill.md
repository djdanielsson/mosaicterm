---
name: code-review
description: "Review code changes through three lenses with structured scoring."
version: "1.0"
type: review
triggers: [review, review code, review pr, code review, check my changes]
---

# Code Review

You are a principal engineer reviewing code. Read the diff three times
through three distinct lenses, producing scored findings with quoted
evidence and concrete fixes.

## How to Execute

1. **Load project context** (see Project Context section below).
2. **Read the entire diff** before forming any opinions.
3. **Understand intent** — what problem is being solved, what approach.
4. **Review through three lenses**, completing each fully before the next:
   - **Functionality** — "Does it work?"
   - **Security** — "Is it safe?"
   - **Quality** — "Is it well-built?"
5. **Self-validate** every finding through the validation chain.
6. **Dedup** findings that appear under multiple lenses.
7. **Present the COMPLETE review** to the user in the Output Format
   below. Do NOT summarize, abbreviate, or strip fields. The user
   needs the full review including Evidence, Confidence, Fix, Path
   to 10/10 with agent-ready prompts, Needs Human Judgment, and
   Limitations. The review output IS the deliverable.

### Coordinator Rule (2-agent mode)

When this skill is executed with a dedicated Security agent and a
combined Review agent, the coordinator MUST:

1. Collect both agents' complete outputs
2. Dedup findings that overlap across agents (keep best-fit lens)
3. Merge Path to 10/10 items from both agents
4. Present the FULL merged review to the user in the Output Format
   — with every Evidence field, every Confidence level, every
   agent-ready fix prompt. Do not summarize the agents' work.
   Present the complete review.

The Security agent produces the Security lens score. The Review
agent produces the Functionality and Quality lens scores. The
coordinator computes the overall as the mean of all three.

---

## Project Context

Before reviewing, load the target repo's project-specific context.
Check these locations and read any that exist:

**Project instructions:**
- `CLAUDE.md` or `.claude/CLAUDE.md` — project-level instructions,
  coding standards, conventions, architectural rules
- `AGENTS.md` or `.agents/AGENTS.md` — agent-specific instructions

**Project skills:**
- `.agents/skills/` — look for review, coding-standards, conventions,
  or domain-specific skills (e.g., `operator-review`)
- `.claude/skills/` — Claude Code skill convention
- `skills/` — top-level skills directory

**ARC configuration:**
- `.agents/arc/config/` or `arc/config/` — Agent Runtime Configuration
  (coding standards, review rules, project-specific patterns)

Read all available context before starting the review. Project-specific
rules take precedence over generic checks — if the project says "use
`yes`/`no` for YAML booleans" and this skill's Quality lens would
flag that as inconsistent, the project convention wins.

This project context provides domain knowledge that a generic review
cannot: framework patterns, naming conventions, architectural
boundaries, deployment constraints, and codebase-specific gotchas.

---

## Lens 1: Functionality — "Does it work?"

Check for: logic errors, control flow bugs, edge cases, error handling
gaps, concurrency issues, race conditions, resource leaks, missing
timeouts, silent failures, incorrect boolean logic, off-by-one errors,
unhandled nil/null, missing return paths, idempotency violations.

Trace every code path. If a variable flows through a transformation
pipeline (filters, type casts, defaults, combine/merge operations),
trace the type at each step. If a value is set in one place and
consumed in another, verify the type survives the pipeline.

When the diff introduces new variable names, fields, or
configuration keys, search the codebase for existing uses of those
names. If existing conditional logic assumes the old semantics, the
collision is in scope — the diff caused the conflict even though
the affected code is not in the changed lines.

---

## Lens 2: Security — "Is it safe?"

Check for: command injection, SQL injection, path traversal, hardcoded
credentials, missing authorization, sensitive data in logs/errors,
insecure deserialization, cryptographic weaknesses, timing attacks,
vulnerable dependencies, configuration-as-code security, unvalidated
external inputs.

### Security Mindset — CRITICAL

**Security findings are NEVER theoretical.** Do not dismiss injection,
credential exposure, or input validation issues because "the variable
is operator-controlled" or "the attacker would need cluster access."

Score the code as written, not the current trust model. A variable
that is operator-controlled today may be wired to user input tomorrow
by a developer who does not know it feeds into an unescaped shell
command. Future maintainers will change input sources without knowing
the downstream execution context.

**Prioritize future-proofing and security best practices.** Sanitize
inputs at the point of use, not based on assumptions about who
provides the data. If unsanitized input reaches a shell, SQL, or
code execution context, it is a finding — regardless of who controls
the input today.

When the validation chain removes a security finding (scope,
materiality, or existing pattern), note it in Observations with
the removal rationale. Humans need to see what was considered and
why it was dismissed — a removed finding is not an ignored finding.

---

## Lens 3: Quality — "Is it well-built?"

Check for: inappropriate coupling, leaky abstractions, DRY violations,
N+1 query patterns, redundant computation, scalability issues,
missing test cases, trivially passing tests, inaccurate documentation,
missing changelog entries, backward-incompatible API changes.

When the diff adds new API fields, CRD properties, or configuration
options, check:
- Sample/example files updated (e.g., config/samples/, examples/)
- Migration notes if existing behavior changes (field locations,
  defaults, required inputs)
- Field descriptions match actual capability (especially when
  schemas allow more than documented)

"Add tests" without identifying (a) a specific untested code path
AND (b) what the test should assert is NOT a valid deduction.

---

## Evidence Gate

Every finding MUST quote the specific code from the diff that
demonstrates the issue. No quoted code, no finding. If you cannot
point to a specific line, convert to observation.

Format each finding as:

```
- **[Lens]** **[file:line]** [description]
  - **Evidence**: `[quoted code from the diff]`
  - **Confidence**: [HIGH | MEDIUM | LOW]
  - **Fix**: [exact code change]
  - **Points**: [number]
```

**Confidence levels:**
- **HIGH** — the issue is directly visible in the quoted code (e.g.,
  hardcoded credential, missing null check, SQL string interpolation)
- **MEDIUM** — the issue requires tracing a code path or inferring
  behavior that is not directly visible (e.g., type coercion through
  a pipeline, race condition under concurrent access)
- **LOW** — the issue depends on runtime behavior, external state, or
  assumptions about how the code is called that cannot be verified
  from the diff alone. LOW findings warrant human verification.

Findings are grouped under severity headings (#### Critical, #### Major,
etc.), so the inline tag identifies the lens, not the severity.

---

## Scoring

Per-lens score:
```
score = max(1, 10 - sum(surviving_deduction_points))
```

Overall score = arithmetic mean of the three lens scores, rounded to
one decimal place (round half up).

| Severity | Points | When to use |
|---|---|---|
| Critical | 2 | Security vulnerabilities, data loss, crashes |
| Major | 1 | Logic errors, missing validation, regressions |
| Minor | 0.5 | Inconsistencies, missing docs, suboptimal code |
| Nit | 0 | Naming suggestions, style preferences |

### Invalid Deductions (score 10 for these)

- "Add tests" without specific code path + assertion
- Commit message quality
- Inherent complexity or large scope
- Cosmetic preferences (formatting, brace style)
- Architectural decisions (score implementation, not the choice)
- Pre-existing issues not changed in this diff
- Theoretical concerns that cannot manifest
  - EXCEPTION: Security vulnerabilities are never theoretical
- Display-only cosmetic text not exposing sensitive data
  - EXCEPTION: Logs exposing credentials, tokens, system internals

---

## Validation Chain

Run this on EVERY finding before including it. No exceptions.

1. **Location** — `file:line` exists in the diff? → if no, REMOVE
2. **Evidence** — Can you quote the code? → if no, REMOVE
3. **Fix** — Concrete code change written? → if no, CONVERT to observation
4. **Invalid** — In the invalid deductions list? → if yes, REMOVE
5. **Scope** — Pre-existing or outside diff? → if yes, REMOVE (note as Incidental Finding)
6. **Materiality** — Can it manifest? → if no, REMOVE
   - Security exception: score even without current trigger
7. **Severity** — Correct per the table above? → if no, REASSIGN
8. **Dedup** — Already reported under another lens? → if yes, MERGE (keep best-fit lens)
9. **Recalculate** — `score = max(1, 10 - sum(points))`

---

## Scope

**Score only the diff.** Do not score pre-existing bugs, files not
modified, or code outside the changed line ranges.

If you find an issue outside the diff, put it in **Incidental Findings**
(not scored, not in main findings).

---

## Verdict

| Condition | Verdict |
|---|---|
| Any Critical or Major finding | **NEEDS_CHANGES** |
| Only Minor, Nit, or none | **READY_FOR_HUMAN_REVIEW** |

This tool does NOT approve or reject code. Final approval belongs
to human maintainers.

---

## Output Format

```
## Code Review: [target]

### Verdict: [READY_FOR_HUMAN_REVIEW | NEEDS_CHANGES]

### Scores
| Lens | Score | Findings |
|---|---|---|
| Functionality | X/10 | ... |
| Security | X/10 | ... |
| Quality | X/10 | ... |
| **Overall** | **X/10** | |

### Findings
#### Critical
#### Major
- **[Lens]** **[file:line]** [description]
  - **Evidence**: `[quoted code]`
  - **Confidence**: [HIGH | MEDIUM | LOW]
  - **Fix**: [code change]
  - **Points**: N
#### Minor
#### Nit

### Score Trajectory
(include on iteration 2+, omit on first review — determine
iteration number from the .review/ directory: count existing
run-N/ subdirectories. If none exist, this is iteration 1.)

Run 1: █████████████████████████████░░░░░░░░░░░░░░░░░░░░░  5.7/10
Run 2: ████████████████████████████████████████████░░░░░░  8.8/10
Target: ██████████████████████████████████████████████████  10/10

Each █ = 0.2 points (50 blocks = 10/10). Compute blocks as
ceil(score / 0.2), display as █ followed by ░ to fill 50 total.
Numeric score uses the actual aggregate, not the display-rounded
value.

### Path to 10/10
Each item is an agent-ready fix prompt — paste into your coding
agent to apply:

1. **[Lens] [file:line]** (+X points)
   > In [file] at line [N], [evidence] does [problem].
   > Change [old] to [new].

### Needs Human Judgment
(areas where the reviewer lacked sufficient context to make a
determination — flag these explicitly, do not bury in observations)

- [description of what could not be determined and why]

### Observations
### Incidental Findings (out of scope — not scored)
### Limitations
This review was performed by an AI agent. It does not understand
business context, domain intent, organizational constraints, or
deployment environment specifics. LOW-confidence findings and items
in "Needs Human Judgment" require human verification. This review
is a first pass, not a final approval.

### Validation Summary
| Lens | Proposed | Removed | Converted | Surviving |
```

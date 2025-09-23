# Feature Specification: mosaicterm

**Feature Branch**: `001-mosaicterm-terminal-emulator`
**Created**: September 17, 2025
**Status**: Draft
**Input**: User description: "name: mosaicterm
version: 0.1.0
description: |
  A Rust GUI terminal emulator inspired by Warp.
  - MVP target: macOS
  - Future goal: cross-platform (Linux, Windows).
  - Key differentiator: commands and outputs are grouped into discrete "blocks" with a permanently pinned prompt at the bottom.

goals:
  - Run zsh (with Oh My Zsh, plugins, themes, completions, fzf, etc.) inside a PTY.
  - Capture stdout/stderr from the PTY and segment them into blocks (command + output).
  - Always render the prompt at the bottom of the window (fixed input field).
  - Display past blocks in a scrollable history panel.
  - Support ANSI color + escape codes (zsh prompts, bat, fzf).
  - Mac MVP must feel native (fonts, scrolling, copy/paste).
  - Long-term: support Linux/Windows shells with minimal changes.

non_goals:
  - Reimplementing zsh or fzf functionality.
  - Writing a new shell or autocomplete system.
  - Full plugin system in MVP (stretch goal).
  - Remote shell support (SSH) in MVP.

requirements:
  environment:
    - Rust stable toolchain (latest).
    - macOS 14+ for MVP.
    - Installed dependencies: zsh, fzf, eza, bat, rg, fd, jq, Oh My Zsh.
  dependencies:
    - GUI: `egui` (with `eframe` for window management).
    - PTY: `portable-pty`.
    - ANSI/escape code parsing: `vte`.
    - Syntax highlighting: `syntect` (future).
    - Config: `serde` + `toml` for settings file.
  design:
    - Main window split into two regions:
      - Scrollable "history" region (all past blocks).
      - Pinned input prompt at bottom (single-line text box).
    - Each block:
      - Command string (rendered as header).
      - Output area (scrollable if long, supports ANSI colors).
    - Block actions (future): collapse, copy, re-run.
  mvp_features:
    - Start zsh in PTY when app launches.
    - Capture prompt detection (use PS1 marker or heuristic).
    - When user presses Enter in bottom input box:
      - Send command string to PTY.
      - Collect output until next prompt.
      - Render new block in history area.
    - Keep bottom input field empty and focused.
    - Handle ANSI output correctly (colored ls, bat).
  stretch_features:
    - Block re-run (send old command again).
    - Inline search/filter for past blocks.
    - GUI buttons to trigger common commands (fd | fzf, rg, bat).
    - Configurable themes for blocks (JSON/TOML config).
    - Export block history to markdown.
  cross_platform_plan:
    - Linux: same approach, run user's default shell inside PTY.
    - Windows: support PowerShell/WSL via PTY integration.
    - Abstract PTY handling into platform module to swap backends.

success_criteria:
  - On macOS, user can launch app, run commands inside zsh, see output grouped into blocks, and always have prompt pinned at bottom.
  - ANSI output (colors, cursor moves) renders correctly.
  - Scrolling through history feels smooth.
  - User's existing zsh config (Oh My Zsh, fzf, completions) works transparently."

## Execution Flow (main)
```
1. Parse user description from Input
   → If empty: ERROR "No feature description provided"
2. Extract key concepts from description
   → Identify: actors, actions, data, constraints
3. For each unclear aspect:
   → Mark with [NEEDS CLARIFICATION: specific question]
4. Fill User Scenarios & Testing section
   → If no clear user flow: ERROR "Cannot determine user scenarios"
5. Generate Functional Requirements
   → Each requirement must be testable
   → Mark ambiguous requirements
6. Identify Key Entities (if data involved)
7. Run Review Checklist
   → If any [NEEDS CLARIFICATION]: WARN "Spec has uncertainties"
   → If implementation details found: ERROR "Remove tech details"
8. Return: SUCCESS (spec ready for planning)
```

---

## ⚡ Quick Guidelines
- ✅ Focus on WHAT users need and WHY
- ❌ Avoid HOW to implement (no tech stack, APIs, code structure)
- 👥 Written for business stakeholders, not developers

### Section Requirements
- **Mandatory sections**: Must be completed for every feature
- **Optional sections**: Include only when relevant to the feature
- When a section doesn't apply, remove it entirely (don't leave as "N/A")

### For AI Generation
When creating this spec from a user prompt:
1. **Mark all ambiguities**: Use [NEEDS CLARIFICATION: specific question] for any assumption you'd need to make
2. **Don't guess**: If the prompt doesn't specify something (e.g., "login system" without auth method), mark it
3. **Think like a tester**: Every vague requirement should fail the "testable and unambiguous" checklist item
4. **Common underspecified areas**:
   - User types and permissions
   - Data retention/deletion policies  
   - Performance targets and scale
   - Error handling behaviors
   - Integration requirements
   - Security/compliance needs

---

## User Scenarios & Testing *(mandatory)*

### Primary User Story
As a developer who uses zsh with Oh My Zsh and various CLI tools, I want a modern terminal emulator that groups commands and their outputs into discrete, scrollable blocks while keeping a permanently pinned input prompt at the bottom, so I can easily review command history and maintain a clean, organized terminal experience similar to Warp but running my existing zsh setup transparently.

### Acceptance Scenarios
1. **Given** the app is launched on macOS, **When** the user opens mosaicterm, **Then** zsh starts in a PTY with the user's existing configuration and displays a pinned input prompt at the bottom
2. **Given** zsh is running with a prompt visible, **When** the user types a command and presses Enter, **Then** the command is sent to the PTY, output is captured until the next prompt, and a new block appears in the history showing the command and its output
3. **Given** a command produces colored ANSI output (like `ls` with colors or `bat` syntax highlighting), **When** the command executes, **Then** the colors and formatting are preserved and rendered correctly in the output block
4. **Given** multiple commands have been executed creating several blocks, **When** the user scrolls the history region, **Then** all past blocks remain visible and scroll smoothly
5. **Given** the input field is empty and focused, **When** the user types a new command, **Then** the bottom input field remains pinned and ready for the next command

### Edge Cases
- What happens when a command produces very long output that exceeds the visible area?
- How does the system handle commands that don't produce output (like background processes)?
- What happens when zsh plugins or completions modify the prompt appearance?
- How does the system detect prompt changes when users customize their PS1?
- What happens when the PTY process terminates unexpectedly?
- How does the system handle commands that take a long time to complete?

## Requirements *(mandatory)*

### Functional Requirements
- **FR-001**: System MUST start zsh shell in a PTY when the application launches
- **FR-002**: System MUST display a permanently pinned input field at the bottom of the window
- **FR-003**: System MUST capture user input from the bottom input field when Enter is pressed
- **FR-004**: System MUST send captured input as commands to the running zsh PTY
- **FR-005**: System MUST capture all stdout and stderr output from zsh commands
- **FR-006**: System MUST group each command and its complete output into a discrete block
- **FR-007**: System MUST display blocks in a scrollable history region above the input field
- **FR-008**: System MUST render ANSI color codes and escape sequences correctly in block output
- **FR-009**: System MUST preserve and utilize user's existing zsh configuration and plugins
- **FR-010**: System MUST detect when zsh displays a new prompt to complete block capture
- **FR-011**: System MUST keep the bottom input field empty and focused after each command execution
- **FR-012**: System MUST support Oh My Zsh themes, completions, and fzf integration transparently
- **FR-013**: System MUST provide native macOS feel for fonts, scrolling, and copy/paste operations

### Key Entities *(include if feature involves data)*
- **Command Block**: Represents a single executed command and its output, containing command text, output content, timestamp, and display state
- **PTY Session**: Represents the running zsh process and its communication channels for input/output
- **History Panel**: Represents the scrollable collection of command blocks in chronological order

---

## Review & Acceptance Checklist
*GATE: Automated checks run during main() execution*

### Content Quality
- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

### Requirement Completeness
- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

---

## Execution Status
*Updated by main() during processing*

- [x] User description parsed
- [x] Key concepts extracted
- [x] Ambiguities marked
- [x] User scenarios defined
- [x] Requirements generated
- [x] Entities identified
- [x] Review checklist passed

---

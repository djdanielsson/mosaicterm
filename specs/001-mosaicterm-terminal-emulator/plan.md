
# Implementation Plan: MosaicTerm Terminal Emulator

**Branch**: `001-mosaicterm-terminal-emulator` | **Date**: September 17, 2025 | **Spec**: [specs/001-mosaicterm-terminal-emulator/spec.md](specs/001-mosaicterm-terminal-emulator/spec.md)
**Input**: Feature specification from `/specs/001-mosaicterm-terminal-emulator/spec.md`

## Execution Flow (/plan command scope)
```
1. Load feature spec from Input path
   → If not found: ERROR "No feature spec at {path}"
2. Fill Technical Context (scan for NEEDS CLARIFICATION)
   → Detect Project Type from context (web=frontend+backend, mobile=app+api)
   → Set Structure Decision based on project type
3. Fill the Constitution Check section based on the content of the constitution document.
4. Evaluate Constitution Check section below
   → If violations exist: Document in Complexity Tracking
   → If no justification possible: ERROR "Simplify approach first"
   → Update Progress Tracking: Initial Constitution Check
5. Execute Phase 0 → research.md
   → If NEEDS CLARIFICATION remain: ERROR "Resolve unknowns"
6. Execute Phase 1 → contracts, data-model.md, quickstart.md, agent-specific template file (e.g., `CLAUDE.md` for Claude Code, `.github/copilot-instructions.md` for GitHub Copilot, or `GEMINI.md` for Gemini CLI).
7. Re-evaluate Constitution Check section
   → If new violations: Refactor design, return to Phase 1
   → Update Progress Tracking: Post-Design Constitution Check
8. Plan Phase 2 → Describe task generation approach (DO NOT create tasks.md)
9. STOP - Ready for /tasks command
```

**IMPORTANT**: The /plan command STOPS at step 7. Phases 2-4 are executed by other commands:
- Phase 2: /tasks command creates tasks.md
- Phase 3-4: Implementation execution (manual or via tools)

## Summary
MosaicTerm is a Rust-based GUI terminal emulator inspired by Warp that groups commands and their outputs into discrete, scrollable blocks while maintaining a permanently pinned input prompt at the bottom. The core approach leverages existing CLI ecosystem tools (zsh, fzf, bat, rg, fd, eza, jq, Oh My Zsh) rather than reinventing functionality, focusing instead on creating a custom block-based UI using egui and integrating with PTY processes.

## Technical Context
**Language/Version**: Rust stable toolchain (latest)
**Primary Dependencies**: egui (with eframe), portable-pty, vte, syntect
**Storage**: File-based configuration (toml), in-memory for session state
**Testing**: cargo test with integration tests for PTY and UI interactions
**Target Platform**: macOS 14+ (MVP), Linux/Windows (future expansion)
**Project Type**: Single desktop application (native GUI)
**Performance Goals**: <16ms frame time (60 fps), <100ms command execution latency, <200MB memory usage
**Constraints**: Native macOS feel, preserve existing zsh configurations, transparent PTY integration
**Scale/Scope**: Single-user desktop app, ~10k LOC, focus on core terminal functionality

## Constitution Check
*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Core Principles Compliance
- [x] **TDD Methodology**: Plan includes cargo test framework and TDD approach for all features
- [x] **Integration-First**: No shell reinvention - leverages zsh, fzf, bat, rg, fd, eza, jq ecosystem
- [x] **Block-Based UI**: Design centers on command blocks and pinned input prompt
- [x] **Cross-Platform**: portable-pty provides foundation for multi-platform support

### Technology Constraints Compliance
- [x] **GUI Framework**: Using egui with eframe for native window management
- [x] **PTY Handling**: portable-pty for cross-platform pseudoterminal support
- [x] **ANSI Processing**: vte crate for escape code parsing and rendering
- [x] **Configuration**: serde + toml for settings management

### Development Standards Compliance
- [x] **Test-First**: All implementation will follow TDD with tests written first
- [x] **Architecture Guidelines**: Clear separation between UI, PTY, and command processing
- [x] **Code Quality**: Integration tests for PTY and UI interactions planned
- [x] **Documentation**: Public APIs and complex logic will be documented

### CLI Tool Integration Compliance
- [x] **Shell**: zsh with Oh My Zsh support as primary target
- [x] **Search Tools**: fzf, rg, fd for interactive search and filtering
- [x] **Display Tools**: bat, eza, jq for enhanced output formatting
- [x] **Platform**: macOS 14+ for MVP with cross-platform expansion planned

**Status**: ✅ ALL CONSTITUTION CHECKS PASS

## Project Structure

### Documentation (this feature)
```
specs/[###-feature]/
├── plan.md              # This file (/plan command output)
├── research.md          # Phase 0 output (/plan command)
├── data-model.md        # Phase 1 output (/plan command)
├── quickstart.md        # Phase 1 output (/plan command)
├── contracts/           # Phase 1 output (/plan command)
└── tasks.md             # Phase 2 output (/tasks command - NOT created by /plan)
```

### Source Code (repository root)
```
src/
├── main.rs                    # Application entry point
├── app.rs                     # Main application state and UI
├── pty.rs                     # PTY process management
├── terminal.rs                # Terminal emulation logic
├── ui/
│   ├── mod.rs
│   ├── blocks.rs              # Command block rendering
│   ├── input.rs               # Input prompt component
│   └── scroll.rs              # Scrollable history area
├── config.rs                  # Configuration management
├── commands.rs                # Command parsing and execution
└── ansi.rs                    # ANSI escape code processing

tests/
├── integration/
│   ├── pty_tests.rs          # PTY interaction tests
│   ├── ui_tests.rs           # UI component tests
│   └── command_tests.rs      # Command execution tests
└── unit/
    ├── terminal_tests.rs     # Terminal logic tests
    ├── ansi_tests.rs         # ANSI processing tests
    └── config_tests.rs       # Configuration tests

benches/
└── performance.rs            # Performance benchmarks
```

**Structure Decision**: Option 1 - Single Rust project with clear separation of concerns

## Phase 0: Outline & Research

### Research Tasks Identified
1. **PTY Integration Patterns**: Research best practices for PTY process management in Rust
2. **ANSI Escape Code Handling**: Investigate vte crate usage for terminal emulation
3. **egui Performance Optimization**: Research techniques for smooth 60fps rendering
4. **Zsh Prompt Detection**: Research methods to detect command completion in zsh
5. **Cross-platform PTY**: Evaluate portable-pty for macOS/Linux/Windows compatibility

### Research Questions to Resolve
- How to reliably detect when zsh commands complete and new prompts appear?
- What are the performance characteristics of egui for terminal-like scrolling?
- How to handle ANSI escape sequences for cursor movement and screen updates?
- What are the best patterns for PTY process lifecycle management?
- How to preserve user's existing zsh configurations and plugins?

### Technology Integration Research
- **portable-pty**: Cross-platform pseudoterminal handling patterns
- **vte**: Terminal emulation and ANSI escape code processing
- **egui**: Immediate mode GUI patterns for terminal interfaces
- **syntect**: Syntax highlighting integration for code blocks

**Output**: research.md with all technical unknowns resolved and implementation approach decisions documented

## Phase 1: Design & Contracts
*Prerequisites: research.md complete*

### 1. Extract Core Entities → `data-model.md`
**Command Block Entity**:
- Fields: command_text, output_content, timestamp, block_id, status
- Relationships: belongs to terminal session
- State transitions: executing → completed/failed

**Terminal Session Entity**:
- Fields: session_id, pty_process, current_directory, shell_config
- Relationships: contains command blocks
- State transitions: initializing → active → terminated

**PTY Process Entity**:
- Fields: process_handle, stdin_writer, stdout_reader, stderr_reader
- Relationships: managed by terminal session
- State transitions: created → running → terminated

### 2. Generate System Contracts → `/contracts/`
**PTY Management Contract**:
- Contract: PTY creation and lifecycle management
- Contract: Command input/output streaming
- Contract: Process termination handling

**UI Rendering Contract**:
- Contract: Block rendering with ANSI color support
- Contract: Scrollable history area management
- Contract: Input prompt positioning and focus

**Command Processing Contract**:
- Contract: Command parsing and validation
- Contract: Output segmentation into blocks
- Contract: Prompt detection and completion

### 3. Generate Contract Tests
- PTY lifecycle contract tests (creation, termination, cleanup)
- Command execution contract tests (input streaming, output capture)
- UI rendering contract tests (block display, scrolling, input focus)
- ANSI processing contract tests (color rendering, escape sequences)

### 4. Extract Integration Test Scenarios
**Primary User Journey**: Launch app → execute commands → view blocks → scroll history
**ANSI Color Support**: Execute colored commands → verify color preservation
**Zsh Integration**: Use Oh My Zsh plugins → verify functionality preservation
**Performance**: Execute multiple commands → verify <16ms frame time
**Error Handling**: Execute failing commands → verify error display

### 5. Update Agent Context File
- Add MosaicTerm-specific Rust patterns and conventions
- Include PTY management and egui integration patterns
- Document ANSI processing and terminal emulation approaches
- Preserve existing AI assistant customizations

**Output**: data-model.md, /contracts/*, failing contract tests, quickstart.md, agent context file

## Phase 2: Task Planning Approach
*This section describes what the /tasks command will do - DO NOT execute during /plan*

**Task Generation Strategy**:
- Load `.specify/templates/tasks-template.md` as base
- Generate tasks from Phase 1 design docs (contracts, data-model.md, quickstart.md)
- Each contract → contract test task [P] (parallel executable)
- Each entity → model/struct creation task [P]
- Each integration scenario → integration test task
- Implementation tasks to make contract tests pass (TDD approach)

**MosaicTerm-Specific Task Categories**:
1. **Core Infrastructure**: PTY management, app initialization, configuration
2. **Terminal Emulation**: ANSI processing, command execution, output capture
3. **UI Components**: Block rendering, input prompt, scrollable history
4. **Integration**: Zsh compatibility, CLI tool support, cross-platform concerns
5. **Quality Assurance**: Performance testing, integration tests, documentation

**Ordering Strategy**:
- **TDD Order**: Contract tests written first, then implementation to make them pass
- **Dependency Order**: Core infrastructure → Terminal logic → UI components → Integration features
- **Parallel Execution**: Mark [P] for independent modules (models, utilities, isolated components)
- **Integration Priority**: Core functionality before advanced features

**Estimated Output**: 30-40 numbered, ordered tasks in tasks.md covering:
- 8-10 contract test tasks
- 6-8 model/entity implementation tasks
- 4-6 UI component tasks
- 6-8 integration and testing tasks
- 4-6 infrastructure and configuration tasks

**IMPORTANT**: This phase is executed by the /tasks command, NOT by /plan

## Phase 3+: Future Implementation
*These phases are beyond the scope of the /plan command*

**Phase 3**: Task execution (/tasks command creates tasks.md)  
**Phase 4**: Implementation (execute tasks.md following constitutional principles)  
**Phase 5**: Validation (run tests, execute quickstart.md, performance validation)

## Complexity Tracking
*Fill ONLY if Constitution Check has violations that must be justified*

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |


## Progress Tracking
*This checklist is updated during execution flow*

**Phase Status**:
- [x] Phase 0: Research outlined (/plan command) - research tasks identified and documented
- [x] Phase 1: Design outlined (/plan command) - entities, contracts, and test scenarios documented
- [x] Phase 2: Task planning approach defined (/plan command) - strategy documented for /tasks command
- [x] Phase 3: Tasks generated and completed (/tasks command) - all 54 tasks completed with comprehensive implementation
- [🟢] Phase 4: Application Launch & Basic UI - 24 new tasks (T055-T078) ready for implementation (T055-T066 ✅)
- [ ] Phase 5: Validation passed - pending Phase 4 completion

**Gate Status**:
- [x] Initial Constitution Check: PASS - all constitutional principles verified
- [x] Post-Design Constitution Check: PASS - design aligns with constitution principles
- [x] All NEEDS CLARIFICATION resolved - technical context fully specified
- [ ] Complexity deviations documented - no deviations identified

---
*Based on MosaicTerm Constitution v1.0.0 - See `/memory/constitution.md`*

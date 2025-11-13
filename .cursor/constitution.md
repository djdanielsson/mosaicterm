# MosaicTerm Constitution

## Core Principles

### I. TDD Methodology (NON-NEGOTIABLE)
All development MUST follow Test-Driven Development principles:
- Tests written FIRST → User approved → Tests fail → Then implement
- Red-Green-Refactor cycle strictly enforced
- Unit tests for all business logic, integration tests for system components
- High test coverage (minimum 80%) required for all features

### II. Integration-First Approach
Don't reinvent the wheel - leverage existing CLI ecosystem:
- Integrate with proven tools: zsh, fzf, bat, rg, fd, eza, jq, Oh My Zsh
- Respect and preserve user's existing configurations and workflows
- Focus on custom UI and block system rather than reimplementing functionality
- PTY integration must be transparent to underlying shell experience

### III. Block-Based UI Architecture
Custom interface design centered around command blocks:
- Commands and outputs grouped into discrete, scrollable blocks
- Permanently pinned input prompt at bottom of window
- Native feel with proper fonts, scrolling, and macOS integration
- Support for ANSI colors, escape codes, and rich terminal output

### IV. Cross-Platform Foundation
Design for multi-platform support from the start:
- Abstract PTY handling into platform modules
- Linux: native shell integration
- Windows: PowerShell/WSL support
- macOS: primary MVP target with native polish

### V. Latest Versions Policy
Use the latest stable versions of all dependencies and tools when possible:
- **Dependency Updates**: Regular review and updates of all crates and tools
- **Security Priority**: Immediate updates for security vulnerabilities
- **Breaking Changes**: Evaluate impact before major version updates
- **Testing Required**: Full test suite must pass after any version updates
- **Documentation**: Update version requirements in documentation when changed

## Technology Constraints

### Core Dependencies
- **GUI Framework**: egui with eframe for native window management
- **PTY Handling**: portable-pty for cross-platform pseudoterminal support
- **ANSI Processing**: vte crate for escape code parsing and rendering
- **Configuration**: serde + toml for settings management

### Required CLI Tools
- **Shell**: zsh with Oh My Zsh support (primary target)
- **Search Tools**: fzf, rg, fd for interactive search and filtering
- **Display Tools**: bat, eza, jq for enhanced output formatting
- **Platform**: macOS 14+ for MVP, with cross-platform expansion planned

## Development Standards

### Code Quality Gates
- All new code MUST have corresponding tests written first
- PRs require review and must pass all tests before merge
- Integration tests required for PTY, UI, and CLI tool interactions
- Performance benchmarks for scrolling and rendering operations

### Architecture Guidelines
- Clear separation between UI layer, PTY management, and command processing
- Modular design allowing for easy platform-specific implementations
- Focus on composability and reusability of components
- Documentation required for all public APIs and complex logic

## Governance

### Constitutional Authority
This constitution supersedes all other development practices and guidelines:
- All PRs and code reviews MUST verify compliance with these principles
- TDD methodology is NON-NEGOTIABLE and cannot be bypassed
- Integration-first approach must be maintained - no reinventing existing CLI tools
- Block-based UI architecture must guide all interface decisions
- Latest versions policy must be followed for all dependencies and tools

### Amendment Process
Constitutional changes require:
- Clear justification and documentation of the change
- Impact assessment on existing principles and architecture
- Review and approval by project stakeholders
- Migration plan for existing code if needed

**Version**: 1.2.0 | **Ratified**: 2025-09-17 | **Last Amended**: 2025-11-11
# Cursor AI Configuration

This directory contains configuration and guidelines for AI agents working on MosaicTerm.

## Files

- **`constitution.md`** - Core development principles and standards that MUST be followed
  - TDD methodology (non-negotiable)
  - Integration-first approach
  - Block-based UI architecture
  - Cross-platform foundation
  - Latest versions policy

## For AI Agents

When working on MosaicTerm:

1. **Read `constitution.md` first** - These principles are non-negotiable
2. **Follow TDD** - Write tests FIRST, get approval, then implement
3. **Check existing specs** - See `specs/001-mosaicterm-terminal-emulator/` for design documents
4. **Review architecture** - See `docs/ARCHITECTURE.md` for system design
5. **Maintain standards** - All code must have tests, pass clippy, and be documented

## Key Principles

- **TDD is mandatory** - No exceptions
- **Integration over reinvention** - Use existing CLI tools (zsh, fzf, bat, etc.)
- **Block-based UI** - Commands and outputs in discrete blocks
- **Cross-platform** - Design for macOS, Linux, Windows from the start
- **Latest versions** - Use latest stable dependencies when possible

## Project Structure

- `src/` - Main application code
- `tests/` - Test suites (unit, integration, contract)
- `specs/` - Feature specifications and design documents
- `docs/` - User and developer documentation
- `.cursor/` - AI agent configuration (this directory)


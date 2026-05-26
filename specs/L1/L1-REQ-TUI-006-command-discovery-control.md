---
artifact_id: L1-REQ-TUI-006
revision: 1
status: Draft
active_baseline: no
supersedes:
superseded_by:
owner: Human
last_updated: 2026-05-27
---

# L1-REQ-TUI-006 — Command Discovery and Control

## Purpose

Ensure that users can discover and invoke TUI commands without memorizing hidden behavior.

## Background / Context

The TUI is expected to expose commands for session control, configuration, model selection, theme changes, goals, interrupts, approvals, and other product workflows. Users need a discoverable command surface that works during normal interactive use.

The `/model` command is the post-onboarding TUI command for changing the current session model and reasoning effort where the selected model supports reasoning.

Onboarding is a startup/setup workflow, not a runtime slash command. Manual onboarding entry is handled through CLI arguments.

## User / Business Requirement

The TUI must provide a discoverable command interface for controlling product workflows from the terminal.

## Functional Requirements

- The user must be able to discover available commands from within the TUI.
- The user must be able to invoke commands intentionally from the composer or another visible command surface.
- Command discovery must include enough names or descriptions for users to choose the right command.
- Commands that are unavailable during active work must provide clear feedback instead of silently failing.
- Commands that affect goals, sessions, configuration, model selection, theme, approval, or interruption must be represented consistently with the related product requirements.
- The TUI must provide a `/model` command for the model-selection workflow.
- The `/model` command must open a selection flow that begins with model selection and then offers reasoning effort selection when the chosen model supports reasoning.
- Command discovery must not list onboarding as a slash command.

## Non-Functional Requirements

- Command discovery must not disrupt typed user input unexpectedly.
- Command feedback must be concise and visible in the TUI.
- The command surface must remain usable without requiring users to read implementation documentation.

## Acceptance Criteria

- Given the user opens command discovery, when commands are available, then the TUI lists relevant command options.
- Given the user invokes a command, when the command is valid, then the command takes effect or reports the next required step.
- Given the user invokes a command that is blocked during active generation, when the command cannot run, then the TUI explains why.
- Given the user starts typing a command, when suggestions are shown, then existing composer text is not lost unexpectedly.
- Given the user invokes `/model`, when the command opens, then the TUI presents the model-selection workflow.
- Given the user selects a reasoning-capable model through `/model`, when reasoning effort selection is needed, then the TUI presents supported reasoning effort choices for that model.
- Given the user opens slash-command discovery, when commands are listed, then onboarding is not listed as a slash command.

## Out of Scope

- Except for the required `/model` command, this requirement does not define exact command names, slash-command grammar, fuzzy matching algorithm, or keybindings.
- This requirement does not define the implementation of each command's underlying product workflow.

## Open Questions

- Which commands must be present in the first TUI milestone?

## Traceability

| Relationship | Target ID | Target Revision | Target Path | Rationale |
|---|---|---:|---|---|
| refined-by | L2-DES-TUI-003 | 1 | specs/L2/tui/L2-DES-TUI-003-composer-and-input-modes.md | Defines slash-command trigger behavior, list height, keyboard navigation, row styling, and the initial command list. |

## Revision Notes

| Revision | Date | Author | Change Type | Notes |
|---:|---|---|---|---|
| 1 | 2026-05-21 | Assistant | Initial | Initial draft approved for L1 expansion. |
| 1 | 2026-05-22 | Human | Refinement | Added `/model` as the required post-onboarding TUI command for model and supported reasoning effort selection. |
| 1 | 2026-05-27 | Human | Refinement | Clarified that onboarding is not a slash command and manual onboarding entry belongs to CLI arguments. |

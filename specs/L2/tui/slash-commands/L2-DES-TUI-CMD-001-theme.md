---
artifact_id: L2-DES-TUI-CMD-001
revision: 1
status: Draft
active_baseline: no
supersedes:
superseded_by:
owner: Assistant
last_updated: 2026-05-26
---

# L2-DES-TUI-CMD-001 — Slash Command: /theme

## Purpose

Define the TUI behavior for `/theme`, which lets the user switch the terminal UI theme without leaving the current session.

## Command Contract

- Command: `/theme`
- Description: `switch the UI theme`
- Parameters: none in the first milestone.
- Mutability: client configuration only.
- Active-turn availability: allowed during active work because it does not change agent execution.

## UI Flow

`/theme` opens a compact searchable popup using the same popup behavior as slash-command discovery.

```text
┃ /theme

> ● devo-dark
    devo-light
    terminal-default
```

Rules:

- The current theme is marked with `●`.
- The row focused by Up and Down navigation is marked with `>`.
- If the focused row is also the current theme, both markers are shown.
- Enter applies the focused row; Esc cancels.
- Theme preview may apply optimistically while the popup is open.
- If canceled, the TUI restores the previous theme.
- The selected theme should be persisted after confirmation.

## State And Error Behavior

- The command must not create a transcript turn.
- The command must not modify session metadata that affects model behavior.
- If persistence fails, the TUI may keep the theme for the current process but must show a concise warning that it was not saved.

## Traceability

| Relationship | Target ID | Target Revision | Target Path | Rationale |
|---|---|---:|---|---|
| refines | L1-REQ-TUI-006 | 1 | specs/L1/L1-REQ-TUI-006-command-discovery-control.md | Defines command-specific behavior for a discoverable TUI command. |
| related-to | L2-DES-TUI-003 | 1 | specs/L2/tui/L2-DES-TUI-003-composer-and-input-modes.md | Uses shared slash-command discovery, popup, and invocation behavior. |
| specified-by | TBD | TBD | specs/L3/tui/TBD.md | L3 behavior has not been authored yet. |

## Revision Notes

| Revision | Date | Author | Change Type | Notes |
|---:|---|---|---|---|
| 1 | 2026-05-23 | Assistant | Initial | Initial `/theme` command design. |
| 1 | 2026-05-26 | Human | Refinement | Updated theme list marker semantics so `>` marks focus and `●` marks the current enabled theme. |

---
artifact_id: L2-DES-TUI-CMD-008
revision: 1
status: Draft
active_baseline: no
supersedes:
superseded_by:
owner: Assistant
last_updated: 2026-05-23
---

# L2-DES-TUI-CMD-008 — Slash Command: /clear

## Purpose

Define the TUI behavior for `/clear`, which clears the current TUI transcript view without deleting durable session history.

## Command Contract

- Command: `/clear`
- Description: `clear the current transcript`
- Parameters: none in the first milestone.
- Mutability: local TUI view state only.
- Active-turn availability: allowed, but active live work remains visible after the clear.

## UI Flow

`/clear` clears completed visible transcript cells from the current TUI viewport.

```text
┃ /clear
```

Rules:

- The command must not delete session JSONL records.
- The command must not create a new session; `/new` handles new chats.
- Active turn content, pending approvals, questions, working indicators, and composer state remain visible.
- Resuming or reloading the session may restore durable transcript content unless a later requirement defines a persistent view-clear marker.

## State And Error Behavior

- `/clear` is display-only in the first milestone.
- If there is nothing to clear, the TUI may show no-op feedback.
- The command must not affect active context, compaction, model behavior, or transcript replay.

## Traceability

| Relationship | Target ID | Target Revision | Target Path | Rationale |
|---|---|---:|---|---|
| refines | L1-REQ-TUI-006 | 1 | specs/L1/L1-REQ-TUI-006-command-discovery-control.md | Defines command-specific behavior for a discoverable TUI command. |
| related-to | L1-REQ-TUI-003 | 1 | specs/L1/L1-REQ-TUI-003-transcript.md | Clear affects local transcript presentation, not durable transcript data. |
| related-to | L2-DES-TUI-004 | 1 | specs/L2/tui/L2-DES-TUI-004-streaming-transcript-and-state.md | Defines durable transcript and live overlay boundaries. |
| related-to | L2-DES-TUI-003 | 1 | specs/L2/tui/L2-DES-TUI-003-composer-and-input-modes.md | Uses shared slash-command discovery and invocation behavior. |
| specified-by | TBD | TBD | specs/L3/tui/TBD.md | L3 behavior has not been authored yet. |

## Revision Notes

| Revision | Date | Author | Change Type | Notes |
|---:|---|---|---|---|
| 1 | 2026-05-23 | Assistant | Initial | Initial `/clear` command design. |

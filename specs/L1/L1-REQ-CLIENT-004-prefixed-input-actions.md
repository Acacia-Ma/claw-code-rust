---
artifact_id: L1-REQ-CLIENT-004
revision: 1
status: Draft
active_baseline: no
supersedes:
superseded_by:
owner: Human
last_updated: 2026-05-25
---

# L1-REQ-CLIENT-004 — Fuzzy Search Prefix

## Purpose

Define client-side behavior for the `@` input prefix that triggers fuzzy search.

## Background / Context

Users need fast ways to reference project or capability entities without leaving the client input flow. Prefix-based input actions provide a compact interaction model:

- `@` starts fuzzy search and selection.

The `@` prefix must be visible and predictable so users understand whether they are sending a normal chat message or selecting a referenced entity.

Terminal-command prefix behavior is specific to the TUI and is specified separately.

## User / Business Requirement

The client interface must recognize `@` at the beginning of input and route it to the fuzzy search workflow.

## Functional Requirements

- If client input begins with `@`, the client must initiate fuzzy search rather than submitting the text as a normal chat message.
- When `@` fuzzy search starts, the client must show a popup window immediately.
- The fuzzy search popup must update results in real time based on the string following the `@` symbol.
- The text immediately following `@` must be treated as the fuzzy search query keyword. The user must not be required to specify a result type after `@`.
- Fuzzy search results must be grouped or ordered by type in this order: skills, MCP entries, then files in the current working directory.
- Pressing Enter while a fuzzy search result is selected must confirm that selection.
- The client must make it clear when input is in normal chat mode or fuzzy search mode.

## Non-Functional Requirements

- Prefix behavior must be predictable and must not silently change normal chat input from ambiguous input.
- The fuzzy search popup must remain responsive enough for interactive typing.
- Search behavior must respect workspace, safety, privacy, and permission boundaries.
- The popup must present result type and selection state clearly enough to avoid accidental selection.

## Acceptance Criteria

- Given the user enters input beginning with `@`, when the prefix is typed, then the client immediately opens a fuzzy search popup.
- Given the user continues typing after `@`, when the query changes, then the popup updates matching results in real time.
- Given the user types text immediately after `@`, when fuzzy search runs, then that text is used as the query across enabled result types rather than being parsed as a type selector.
- Given fuzzy search returns skills, MCP entries, and current-working-directory files, when the popup renders them, then skills appear before MCP entries and MCP entries appear before files.
- Given a fuzzy search result is selected, when the user presses Enter, then the client confirms the selected result.
- Given a search action would exceed permissions, when the action is invoked, then the program follows the applicable safety and approval behavior.

## Out of Scope

- This requirement does not define TUI-only terminal-command prefix behavior.
- This requirement does not define fuzzy matching algorithms, scoring, indexing, popup layout, or keyboard navigation beyond Enter confirmation.
- This requirement does not define how selected fuzzy search results are represented in model context or transcript history.

## Open Questions

- Should whitespace before `@` still trigger prefixed input behavior?
- Should fuzzy search include sessions, transcript entries, or commands in addition to skills, MCP entries, and current-working-directory files?
- How should users escape a leading `@` when they intend to send a normal chat message?

## Traceability

| Relationship | Target ID | Target Revision | Target Path | Rationale |
|---|---|---:|---|---|
| related-to | L1-REQ-TUI-008 | 1 | specs/L1/L1-REQ-TUI-008-terminal-command-prefix.md | TUI-only terminal-command prefix behavior is specified separately from general client fuzzy search. |
| refined-by | TBD | TBD | specs/L2/client/TBD.md | L2 design has not been authored yet. |

## Revision Notes

| Revision | Date | Author | Change Type | Notes |
|---:|---|---|---|---|
| 1 | 2026-05-21 | Assistant | Initial | Initial draft from approved user requirement. |
| 1 | 2026-05-21 | Human | Refinement | Moved TUI-only terminal-command prefix behavior into a TUI requirement and scoped this requirement to `@` fuzzy search. |
| 1 | 2026-05-25 | Human | Refinement | Clarified that text immediately after `@` is the fuzzy-search query and not a result-type selector. |

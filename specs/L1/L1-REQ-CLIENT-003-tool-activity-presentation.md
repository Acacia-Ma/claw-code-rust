---
artifact_id: L1-REQ-CLIENT-003
revision: 1
status: Draft
active_baseline: no
supersedes:
superseded_by:
owner: Human
last_updated: 2026-05-21
---

# L1-REQ-CLIENT-003 — Tool Activity Presentation

## Purpose

Ensure that clients present tool activity as readable user-facing work groups instead of noisy raw tool-call streams.

## Background / Context

Agent work often contains repeated tool calls that belong to the same user-visible activity. Reading files and searching the project are exploratory work. Writing files and applying patches are file-update work. Clients should group these activities so users can scan what the agent did without losing access to important details.

The TUI client has immediate presentation requirements. A future desktop client should use the same product concepts while choosing a richer presentation that fits desktop UI conventions.

## User / Business Requirement

The program must provide client-facing tool activity presentation that groups related tool calls, labels them by user-visible intent, and exposes file-change details when files are created or edited.

## Functional Requirements

- In the TUI client, consecutive `read` tool calls must be grouped together on a single line and labeled `read`.
- In the TUI client, consecutive `find` and `grep` tool calls must be grouped together on a single line and labeled `search`.
- In the TUI client, `read` and `search` activity must be nested under an `Explore` group.
- In the TUI client, model reasoning content and the model response must mark the beginning and end of an `Explore` group.
- In the TUI client, `write` and `apply_patch` activity must be nested under a `File Update` group.
- In the TUI client, a `write` operation may be labeled `Created`.
- In the TUI client, an `apply_patch` operation must be labeled `Edited`.
- The server-side agent capability must transmit file-change details for `write` and `apply_patch` operations so clients can render the specific changes.
- A future desktop client should group consecutive tool calls together and make the group collapsible.
- A future desktop client should summarize grouped tool activity with user-visible counts, such as `Explored 1 file, 1 search, ran 2 commands`.

## Non-Functional Requirements

- Grouping must improve readability without hiding important tool outcomes, errors, or file-change details.
- Tool activity labels must describe user-visible intent rather than raw implementation details where possible.
- Grouped activity must remain auditable from the transcript or an expanded detail view.
- Client-specific presentation may differ, but the semantic grouping of exploration and file updates should remain consistent.

## Acceptance Criteria

- Given consecutive `read` calls occur in the TUI client, when they are rendered, then they appear as one grouped `read` line.
- Given consecutive `find` or `grep` calls occur in the TUI client, when they are rendered, then they appear as one grouped `search` line.
- Given `read` or `search` activity occurs, when the TUI renders it, then it is shown within an `Explore` group.
- Given `write` activity occurs, when the TUI renders it, then it is shown within a `File Update` group and may be labeled `Created`.
- Given `apply_patch` activity occurs, when the TUI renders it, then it is shown within a `File Update` group and labeled `Edited`.
- Given a file is created or edited by `write` or `apply_patch`, when a client renders the event, then the client has enough file-change details from the server-side agent capability to display the specific change.
- Given a future desktop client renders consecutive tool calls, when the group is collapsed, then it can summarize activity counts such as files explored, searches performed, and commands run.

## Out of Scope

- This requirement does not define exact TUI line layout, icons, colors, collapse controls, animation, or desktop component design.
- This requirement does not define the server event schema or exact file-diff payload format.
- This requirement does not define all possible tool grouping categories beyond the `Explore` and `File Update` groups described here.

## Open Questions

- Which model response event precisely closes an `Explore` group when exploration and answering interleave?
- Should command execution have its own top-level group or remain summarized alongside exploration in desktop views?
- What minimum file-change detail must be transmitted for created, edited, deleted, and renamed files?

## Traceability

| Relationship | Target ID | Target Revision | Target Path | Rationale |
|---|---|---:|---|---|
| refined-by | TBD | TBD | specs/L2/client/TBD.md | L2 design has not been authored yet. |

## Revision Notes

| Revision | Date | Author | Change Type | Notes |
|---:|---|---|---|---|
| 1 | 2026-05-21 | Assistant | Initial | Initial draft from approved user requirement. |

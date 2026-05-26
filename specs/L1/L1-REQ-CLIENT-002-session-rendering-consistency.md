---
artifact_id: L1-REQ-CLIENT-002
revision: 1
status: Draft
active_baseline: no
supersedes:
superseded_by:
owner: Human
last_updated: 2026-05-21
---

# L1-REQ-CLIENT-002 — Session Rendering Consistency

## Purpose

Ensure that clients render active sessions and restored session history with a consistent visual and stylistic language.

## Background / Context

Users may interact with a session while it is active, close the application, and later resume the same session from persisted history. From the client perspective, restored history should not feel like a separate or degraded rendering mode.

Live-only states such as active streaming, temporary progress indicators, and animations may naturally differ from restored static history, but the overall visual language, content hierarchy, spacing, typography, color usage, and item treatment should remain consistent.

## User / Business Requirement

The client interface must render active chat sessions and resumed chat history in a visually and stylistically consistent way.

## Functional Requirements

- Client interfaces must use consistent visual treatment for equivalent session items during active use and after history restoration.
- Restored user messages, assistant messages, tool calls, tool outputs, approvals, errors, and turn summaries must remain recognizable as the same kinds of items that appeared during active use.
- Restored history must preserve enough display metadata for clients to render item state, role, outcome, and hierarchy consistently.
- Live-only affordances may be omitted or converted to stable completed-state affordances after restoration.
- If restored content cannot be rendered exactly as it appeared live, the client must still present it in the same design language and make any state differences understandable.

## Non-Functional Requirements

- Restored history must not appear visually broken, raw, or stylistically unrelated to active-session rendering.
- Styling consistency must not hide important differences between active, completed, failed, interrupted, restored, or pending states.
- The consistency requirement must hold across normal application close and relaunch workflows.
- Client rendering consistency must not require persisting unnecessary implementation-only UI state.

## Acceptance Criteria

- Given a user views a message during an active session, when the same session is restored after relaunch, then the restored message uses a visually consistent item style.
- Given a tool call completes during an active session, when the session is restored from history, then the tool call remains recognizable as the same type of transcript item.
- Given a turn summary is shown live, when the session is restored, then the restored summary preserves the same content hierarchy and visual role.
- Given live streaming animations are not present after restoration, when the client renders restored history, then the completed-state rendering still fits the same design language.
- Given restored pending items such as `steer` or `queue` entries are displayed, when the user inspects them, then they are visually consistent with client UI while remaining distinguishable from completed transcript history.

## Out of Scope

- This requirement does not define exact colors, fonts, spacing values, component implementations, animation behavior, or per-client rendering primitives.
- This requirement does not require persisting every transient live-rendering frame or animation state.
- This requirement does not require all clients to use identical layouts, only consistent treatment within each client.

## Open Questions

- Which display metadata must be persisted to support consistent restored rendering?
- Which live-only states should have explicit restored equivalents?
- Should each client define its own rendering consistency checklist in L2?

## Traceability

| Relationship | Target ID | Target Revision | Target Path | Rationale |
|---|---|---:|---|---|
| refined-by | L2-DES-TUI-007 | 1 | specs/L2/tui/L2-DES-TUI-007-session-rendering-consistency.md | Defines the TUI projection and renderer boundary that makes live and restored session rendering consistent. |

## Revision Notes

| Revision | Date | Author | Change Type | Notes |
|---:|---|---|---|---|
| 1 | 2026-05-21 | Assistant | Initial | Initial draft from approved user requirement. |

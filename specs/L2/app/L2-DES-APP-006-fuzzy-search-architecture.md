---
artifact_id: L2-DES-APP-006
revision: 1
status: Draft
active_baseline: no
supersedes:
superseded_by:
owner: Assistant
last_updated: 2026-05-25
---

# L2-DES-APP-006 — Fuzzy Search Architecture

## Purpose

Refine fuzzy search into a technical design for fast, incremental project file search and a shared search-provider model usable by clients.

## Background / Context

Fuzzy search is used by client input flows, pickers, and reference selection. The most latency-sensitive case is project file search while the user is typing after an input prefix such as `@`.

Project file search should avoid blocking the client input loop. It should discover files incrementally, index them in the background, update results as the query changes, and stop promptly when the session is canceled or superseded.

## Source Requirements

- `L1-REQ-APP-006` requires fuzzy search for project files, skills, MCP capabilities, and other important user-facing entities.
- `L1-REQ-CLIENT-004` requires the `@` prefix to start fuzzy search and update results in real time.
- `L1-REQ-CLIENT-001` requires Unicode and IME-safe client behavior.
- `L1-REQ-TUI-001` requires reliable composer behavior.
- `L2-DES-CLIENT-001` defines Unicode and IME constraints.

## Design Requirement

The program should provide a reusable fuzzy-search service with:

- A project file search backend.
- Extensible provider slots for skills, MCP entries, sessions, commands, and transcript search.
- Incremental updates suitable for interactive client popups.
- Bounded result sets.
- Cancellation and shutdown control.
- Workspace, privacy, permission, ignore, and exclude policy enforcement.

## Project File Search Backend

The project file search backend should run as a session-oriented core library. A caller creates a session, updates the query as the user types, receives snapshots, and drops or cancels the session when the popup closes.

Conceptual API:

| API | Purpose |
|---|---|
| `create_session` | Start a live search session and background workers. |
| `FileSearchSession.update_query` | Send a new query to the matcher. |
| `FileSearchSession.drop` | Shut down worker state and release resources. |
| `run` | Synchronous wrapper for one-shot search. |
| `run_main` | CLI-oriented entry point for manual or tool-driven search. |
| `SessionReporter.on_update` | Receive incremental result snapshots. |
| `SessionReporter.on_complete` | Receive completion once walking and matching are done. |

## Thread Model

Project file search should use two coordinated workers.

Walker worker:

- Recursively walks one or more configured search roots.
- Uses workspace-relative paths for indexed items where possible.
- Includes hidden files unless ignore or exclude policy removes them.
- Follows symlinks where policy permits.
- Applies gitignore-style rules by default where applicable.
- Allows ignore processing to be disabled by explicit option.
- Applies explicit exclude patterns before indexing.
- Pushes discovered relative paths into the matcher index incrementally.
- Checks cancellation and shutdown flags periodically.
- Emits a walk-complete signal when traversal finishes.

The Rust implementation should use the `ignore` crate or an equivalent walker that supports parallel traversal, gitignore-style filtering, hidden-file policy, symlink policy, and override patterns. Explicit exclude patterns should be translated into walker-level negative overrides where possible so excluded paths are not indexed.

Matcher worker:

- Owns the fuzzy matcher instance.
- Receives query updates, index notifications, walk-complete signals, and shutdown signals.
- Parses each query with case-insensitive matching and smart Unicode normalization.
- Uses incremental query parsing when a new query extends the previous query.
- Debounces matcher ticks, with a target around 10ms for interactive use, so rapid typing does not create excessive recomputation.
- Emits bounded snapshots when match status changes.
- Emits completion when matching is done and the walk has completed.

## Matcher Implementation Choice

The Rust implementation should use a high-performance fuzzy matcher suitable for incremental file-path search. `nucleo` with path matching enabled is the preferred implementation-level choice.

Expected matcher configuration:

- Path-aware matching enabled.
- Case-insensitive matching by default.
- Smart Unicode normalization.
- Incremental pattern parsing when a new query extends the prior query.
- Optional match index calculation for client highlighting.

The exact scoring formula remains an implementation detail, but behavior must remain path-aware and stable enough for users to build intuition.

## Search Signals

Conceptual work signals:

| Signal | Purpose |
|---|---|
| `QueryUpdated` | The client or caller changed the query string. |
| `IndexUpdated` | New indexed paths are available to match. |
| `WalkComplete` | The directory walk finished. |
| `Shutdown` | The session is closing and workers should exit. |

Signals should be idempotent where practical. A stale query update should not corrupt a newer snapshot.

## File Search Options

Conceptual options:

| Option | Purpose |
|---|---|
| `limit` | Maximum result count in each snapshot. |
| `exclude_patterns` | User or system patterns removed from search. |
| `threads` | Walker parallelism. |
| `compute_indices` | Whether to compute character indices for highlighting. |
| `respect_gitignore` | Whether gitignore-style rules are applied. |
| `search_roots` | One or more roots to search. |

Defaults should favor interactive use: bounded results, ignore rules respected, and enough parallelism for large workspaces without starving the client.

## File Match Model

Conceptual file match fields:

| Field | Purpose |
|---|---|
| `score` | Fuzzy ranking score. |
| `path` | Workspace-relative path where possible. |
| `match_type` | File or directory. |
| `root` | Search root that produced the match. |
| `indices` | Optional sorted character indices for highlighting. |

Clients should not infer file or directory type from display text. The backend should provide explicit type information.

## Matching Behavior

Project file matching should be path-aware.

Rules:

- Match against the relative path, not only the basename.
- Prefer useful basename and path-segment matches over weak deep-path matches.
- Treat path separators as meaningful segment boundaries.
- Use case-insensitive matching by default.
- Use Unicode normalization appropriate for user input and path display.
- Keep exact scoring formulas as implementation detail.

## Snapshot Semantics

A `FileSearchSnapshot` represents the current best known matches for one query at one point in time.

Snapshot fields should include:

- Query string.
- Search roots.
- Ordered matches up to `limit`.
- Whether the walk is complete.
- Whether matching is complete.
- Optional error or degraded-state summary.

Snapshots are live display projections. They are not durable session transcript records.

## Entity Provider Model

The file search backend is one provider in a broader fuzzy-search provider model.

Conceptual provider categories:

| Provider | Examples |
|---|---|
| Skills | Installed or available skill names and summaries. |
| MCP | Servers, tools, resources, and templates where available. |
| Files | Workspace files and directories. |
| Sessions | Recent or matching sessions where supported. |
| Commands | Slash commands or command palette entries where supported. |

Provider aggregation should preserve result type so clients can group or order results predictably.

## Safety And Privacy

Rules:

- Search roots must remain inside allowed workspace or explicitly authorized directories.
- Restricted files, directories, skills, MCP entries, or sessions must be omitted or marked unavailable according to policy.
- Exclude patterns must be applied before results are exposed to clients.
- Search must not leak provider credentials or hidden internal state through display text.
- Symlink traversal must avoid cycles and respect permission boundaries.

## Completion And Cancellation

Search sessions are short-lived and may be superseded by user input or popup closure.

Rules:

- Dropping or canceling a session should signal worker shutdown.
- Workers should poll cancellation frequently enough to exit promptly in large workspaces.
- A new query should replace the previous query without requiring a full re-walk where the existing index remains valid.
- A one-shot synchronous search may block the caller until completion, but client popup flows should use the incremental session API.

## CLI Wrapper Behavior

The core library may expose a CLI-oriented wrapper for manual use, diagnostics, or tool-driven file selection.

Rules:

- When a query pattern is provided, the wrapper should run fuzzy file search and return bounded results.
- When no query pattern is provided, the wrapper may fall back to an operating-system file listing or file browser behavior where appropriate.
- CLI wrapper behavior must not weaken workspace, ignore, exclude, privacy, or permission policy used by the core search backend.

## Traceability

| Relationship | Target ID | Target Revision | Target Path | Rationale |
|---|---|---:|---|---|
| refines | L1-REQ-APP-006 | 1 | specs/L1/L1-REQ-APP-006-fuzzysearch.md | Defines project file search architecture, provider aggregation, snapshots, matching behavior, and safety policy. |
| related-to | L1-REQ-CLIENT-004 | 1 | specs/L1/L1-REQ-CLIENT-004-prefixed-input-actions.md | The `@` prefix uses fuzzy-search providers for live selection. |
| related-to | L1-REQ-CLIENT-001 | 1 | specs/L1/L1-REQ-CLIENT-001-localization-readiness.md | Search queries and paths must preserve Unicode text. |
| related-to | L2-DES-CLIENT-001 | 1 | specs/L2/client/L2-DES-CLIENT-001-localization-readiness.md | Defines Unicode, IME, and display constraints for client search input. |
| specified-by | TBD | TBD | specs/L3/app/TBD.md | L3 behavior has not been authored yet. |

## Revision Notes

| Revision | Date | Author | Change Type | Notes |
|---:|---|---|---|---|
| 1 | 2026-05-25 | Assistant | Initial | Initial fuzzy search architecture with incremental project file search, provider aggregation, cancellation, path-aware matching, and result snapshots. |

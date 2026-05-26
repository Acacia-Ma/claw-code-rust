---
artifact_id: L1-REQ-APP-006
revision: 1
status: Draft
active_baseline: no
supersedes:
superseded_by:
owner: Human
last_updated: 2026-05-25
---

# L1-REQ-APP-006 — Fuzzy Search

## Purpose

Help users quickly find capabilities, project files, and project information without exact names.

## Why This Matters

Users often remember partial names, rough concepts, path segments, or fragments from prior work rather than exact paths or command names. Fuzzy search reduces friction when navigating project files, sessions, skills, MCP capabilities, and commands.

## Background / Context

Users need to navigate skills, MCP capabilities, project files, commands, and prior context during agentic work.

Project file search is a core fuzzy-search use case. It must feel interactive while the user types, support path-aware matching, and respect workspace and ignore-policy boundaries.

## User / Business Requirement

The program must support fuzzy search across important user-facing entities.

## Real User Scenarios

- A user remembers part of a filename and uses fuzzy search to open the relevant project file.
- A user remembers only a nested path segment and still finds the intended project file.
- A user searches hidden project configuration files when those files are not excluded by workspace policy.
- A user searches for an available skill or MCP tool without remembering its exact name.

## Functional Requirements

- The program must support fuzzy search for project files.
- Project file search must update results incrementally while the query changes.
- Project file search must support path-aware ranking so basename and path-segment matches are useful.
- Project file search must return enough information for clients to distinguish files from directories.
- Project file search must return workspace-relative paths where possible.
- Project file search must support one or more search roots.
- Project file search must respect gitignore or equivalent ignore rules by default where applicable.
- Project file search must support explicit exclude patterns.
- Project file search should include hidden files when they are inside the allowed search scope and not excluded by ignore or exclude policy.
- The program must support fuzzy search for skills.
- The program must support fuzzy search for MCP servers, tools, resources, or templates where available.
- The program should support fuzzy search for sessions, transcript entries, and commands.

## Non-Functional Requirements

- Search results must be fast enough for interactive use.
- File-search indexing and matching must not block the client input loop.
- Long-running or superseded searches must be cancelable or safely replaceable.
- Search must respect workspace, privacy, and permission boundaries.
- Search results must be bounded so clients are not overwhelmed by large workspaces.
- Search behavior should degrade predictably when ignore files, workspace roots, or filesystem metadata are unavailable.

## Acceptance Criteria

- Given a partial file name, when the user searches, then matching project files are returned.
- Given a partial path segment, when the user searches, then relevant nested project files are returned.
- Given the user changes the search query while project file search is running, when new matches are available, then the client can display updated results without waiting for a full rescan to finish.
- Given search results include files and directories, when results are shown, then the user can distinguish file results from directory results.
- Given search results include workspace files, when paths are shown, then paths are workspace-relative where possible.
- Given a file is excluded by gitignore or an explicit exclude pattern, when default search policy is active, then that file is omitted from project file search results.
- Given configured skills or MCP capabilities, when the user searches by partial name, then relevant entries are discoverable.
- Given search results include different entity types, when results are shown, then the user can distinguish files, sessions, commands, skills, and MCP capabilities.
- Given a search crosses workspace data, when permissions restrict access, then restricted entries are omitted or clearly unavailable.

## Out of Scope

- The program does not define exact matcher libraries, worker-thread implementation, scoring formulas, or UI layout in this L1 requirement.
- This requirement does not require fuzzy search to expose private or permission-restricted data.

## Open Questions

- Should transcript and session search be included in the initial fuzzy-search scope?

## Traceability

| Relationship | Target ID | Target Revision | Target Path | Rationale |
|---|---|---:|---|---|
| refined-by | TBD | TBD | specs/L2/app/TBD.md | L2 design has not been authored yet. |

## Revision Notes

| Revision | Date | Author | Change Type | Notes |
|---:|---|---|---|---|
| 1 | 2026-05-20 | Assistant | Initial | Initial draft with approved L1 refinement. |
| 1 | 2026-05-25 | Human | Refinement | Added real-time project file search behavior, path-aware matching expectations, ignore-policy handling, exclude patterns, result bounds, and file/directory distinction. |

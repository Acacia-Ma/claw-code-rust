---
artifact_id: L1-REQ-TOOL-004
revision: 1
status: Draft
active_baseline: no
supersedes:
superseded_by:
owner: Human
last_updated: 2026-05-21
---

# L1-REQ-TOOL-004 — Parallel Tool Orchestration

## Purpose

Define the user-visible behavior of explicit parallel tool orchestration.

## Background / Context

Some agent workflows benefit from invoking multiple independent tools at the same time. The program may expose a tool orchestration capability named `multi_tool_use` that lets the model request several tool calls as one explicitly parallel group.

When `multi_tool_use` is invoked, the user's expectation is direct parallel execution of the listed tool calls. The program should not reinterpret the group as a request for the runtime to decide whether the listed calls are parallel-safe. However, `multi_tool_use` must not bypass the normal controls that apply to each underlying tool call.

## User / Business Requirement

The program must execute tool calls listed in `multi_tool_use` concurrently as requested, while still applying each tool's ordinary validation, permission, approval, sandbox, availability, and safety checks.

## Functional Requirements

- The program must support an explicit parallel tool orchestration capability where enabled.
- When the model invokes `multi_tool_use`, the program must schedule the listed tool calls for parallel execution.
- The program must not serialize, reorder, reject, or downgrade a `multi_tool_use` group solely because of additional runtime parallel-safety classification.
- Each underlying tool call inside `multi_tool_use` must still pass its ordinary schema validation, availability checks, permission checks, approval requirements, sandbox restrictions, and safety processing.
- `multi_tool_use` must not allow a tool call to bypass controls that would apply if that same tool were invoked directly.
- If an underlying tool call is blocked by ordinary validation, permission, approval, sandbox, availability, or safety behavior, that tool call must report the same kind of blocked or failed state it would report outside `multi_tool_use`.
- If a shell command is included in `multi_tool_use`, the shell command must be treated as an ordinary shell command invocation for validation and safety purposes, while still being scheduled in parallel as part of the group.
- The program must preserve user-visible results for each underlying tool call in the parallel group.

## Non-Functional Requirements

- Parallel orchestration behavior must be predictable: `multi_tool_use` means parallel execution, not runtime-selected serialization.
- Tool activity from a parallel group must remain auditable at the group level and at the individual tool-call level.
- Failures or blocked calls inside a parallel group must not hide successful sibling tool results.
- Parallel execution must not weaken existing safety, approval, permission, or sandbox guarantees.

## Acceptance Criteria

- Given `multi_tool_use` contains multiple valid and allowed tool calls, when it is invoked, then the program starts those tool calls concurrently.
- Given a tool call inside `multi_tool_use` requires approval, when approval is required by ordinary tool policy, then the tool call follows the normal approval behavior rather than bypassing it.
- Given a tool call inside `multi_tool_use` has invalid input, when the group is invoked, then that tool call is rejected or fails according to ordinary validation behavior.
- Given a shell command appears inside `multi_tool_use`, when the group is invoked, then the shell command is scheduled in parallel subject to ordinary shell command controls.
- Given one tool call in a parallel group fails or is blocked, when sibling calls complete successfully, then successful sibling results remain visible.
- Given the user reviews tool activity, when a parallel group was executed, then the user can identify both the group and each underlying tool result.

## Out of Scope

- This requirement does not define the exact wire format, tool schema, or provider mapping for `multi_tool_use`.
- This requirement does not require runtime read/write classification, dependency analysis, or resource-lock scheduling for `multi_tool_use`.
- This requirement does not define model prompting policy for when the model should choose `multi_tool_use`.
- This requirement does not make parallel execution a bypass around existing tool validation, safety, approval, permission, sandbox, or availability behavior.

## Open Questions

- Should partial failure in `multi_tool_use` be reported as a group-level failure, an item-level failure, or both?
- Should the client render `multi_tool_use` as a visible group separate from ordinary tool activity grouping?
- Should the program expose limits on the maximum number of tool calls allowed in one `multi_tool_use` request?

## Traceability

| Relationship | Target ID | Target Revision | Target Path | Rationale |
|---|---|---:|---|---|
| refined-by | L2-DES-TOOL-002 | 1 | specs/L2/tool/L2-DES-TOOL-002-parallel-tool-orchestration.md | Defines explicit `multi_tool_use` child scheduling, normal child lifecycle controls, aggregation, visibility, cancellation, and testing strategy. |
| related-to | L2-DES-TOOL-001 | 1 | specs/L2/tool/L2-DES-TOOL-001-built-in-tool-system.md | Defines shared tool registry and lifecycle behavior applied to each child call. |

## Revision Notes

| Revision | Date | Author | Change Type | Notes |
|---:|---|---|---|---|
| 1 | 2026-05-21 | Assistant | Initial | Initial draft from approved user requirement. |

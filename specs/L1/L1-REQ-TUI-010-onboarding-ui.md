---
artifact_id: L1-REQ-TUI-010
revision: 1
status: Draft
active_baseline: no
supersedes:
superseded_by:
owner: Human
last_updated: 2026-05-27
---

# L1-REQ-TUI-010 — Onboarding UI

## Purpose

Ensure that first-time or incomplete model setup can be completed from the TUI without leaving the terminal workflow.

## Why This Matters

Users cannot start useful work until at least one supported model is configured for invocation. The TUI onboarding experience should make the required model setup path clear, searchable, recoverable, and compatible with credential handling expectations.

## Background / Context

The TUI is the initial client surface. When required model configuration is missing, the TUI should guide the user through model setup before normal session work begins.

The required TUI onboarding flow starts with supported model slug selection, then lets the user select an existing provider or add a provider. Adding a provider collects provider name, base URL, and API key. After provider selection or creation, the flow collects the model name expected by that provider, lets the user accept or edit the display name shown in client interfaces, asks the user to select an invocation method, and finally asks for reasoning effort when the selected model supports reasoning.

Manual onboarding entry should be triggered by startup CLI arguments rather than a runtime slash command. The TUI may automatically enter onboarding when required model configuration is missing.

## User / Business Requirement

The TUI must provide an onboarding UI for required model setup that guides the user through supported model selection, provider detail entry, invocation method selection, and reasoning effort selection where supported.

## Real User Scenarios

- A first-time user opens the TUI, searches supported model slugs, selects one, and then selects or adds a provider.
- A user adds a provider by entering provider name, base URL, and API key.
- A user enters the model name expected by the selected provider before choosing an invocation method.
- A user accepts or edits the display name that will be shown for the configured model.
- A user chooses the invocation method, such as OpenAI Chat Completions, OpenAI Responses, or Anthropic Messages, after confirming the display name.
- A user selects a reasoning-capable model during onboarding and is asked to choose a supported reasoning effort before starting work.
- A user mistypes an API key or base URL and receives recoverable feedback without losing completed setup progress.

## Functional Requirements

- The TUI must start or offer onboarding when required model configuration is missing.
- The TUI onboarding process must not be exposed as a slash command.
- The program must provide a CLI argument path for explicitly starting onboarding.
- TUI onboarding model setup must begin with supported model slug selection.
- The user must be able to search or filter supported model slugs during onboarding.
- After model slug selection, TUI onboarding must let the user select an existing provider or add a provider.
- When adding a provider, TUI onboarding must collect provider name, base URL, and API key where applicable.
- After provider selection or creation, TUI onboarding must collect the model name expected by that provider.
- After model name entry, TUI onboarding must let the user accept or edit the configured model display name used in client interfaces.
- After model display name entry, TUI onboarding must let the user select an invocation method where applicable.
- Invocation method choices should include OpenAI Chat Completions, OpenAI Responses, and Anthropic Messages where available.
- When the selected model supports reasoning, TUI onboarding must let the user select a supported reasoning effort after invocation method selection.
- Every onboarding input field or selection popup must show a concise hint describing the current value the user is expected to provide.
- When the selected model does not support reasoning, TUI onboarding must continue without asking for reasoning effort.
- Credential entry must be treated as an explicit credential-handling flow rather than ordinary transcript input.
- The TUI must preserve completed onboarding fields when validation fails where it is safe and useful to do so.
- When required setup succeeds, the TUI must submit onboarding results for persistent configuration storage.
- The TUI must let the user complete onboarding and reach a usable session after required setup succeeds.

## Non-Functional Requirements

- Onboarding feedback must be concise and recoverable.
- The onboarding flow must not expose plaintext credentials in routine transcript, model list, model switcher, logging, or telemetry paths by default.
- The onboarding UI must remain usable with keyboard-driven terminal interaction.
- The onboarding flow should avoid making optional setup feel mandatory.

## Acceptance Criteria

- Given required model configuration is missing, when the TUI starts, then the TUI starts or offers onboarding before normal model invocation is attempted.
- Given the user wants to explicitly run onboarding, when the program is launched with the onboarding CLI argument, then the TUI enters the onboarding flow.
- Given slash-command discovery is open, when onboarding is available, then onboarding is not shown as a slash command.
- Given TUI onboarding begins model setup, when supported models are available, then the user can search or filter supported model slugs.
- Given the user selects a supported model slug, when provider selection is required, then the TUI lets the user select an existing provider or add a provider.
- Given the user adds a provider, when provider details are required, then the TUI lets the user provide provider name, base URL, and API key where applicable.
- Given credential material is entered during onboarding, when the TUI handles it, then the credential entry is not treated as ordinary transcript input.
- Given provider selection or creation is complete, when model name is required, then the TUI lets the user enter the model name expected by that provider.
- Given model name entry is complete, when model display name is required for client display, then the TUI lets the user accept a suggested display name or edit it.
- Given model display name entry is complete, when invocation method selection is required, then the user can choose a supported invocation method.
- Given the selected model supports reasoning, when invocation method selection is complete, then the user can choose a supported reasoning effort.
- Given the selected model does not support reasoning, when invocation method selection is complete, then the TUI continues without asking for reasoning effort.
- Given the user is entering or selecting an onboarding value, when that field or popup is active, then the TUI shows a concise hint describing the current value.
- Given validation fails during provider setup, when the user returns to setup, then previously completed safe fields remain available where possible.
- Given required onboarding setup succeeds, when the program restarts, then the model setup completed through the TUI is available from persistent configuration.
- Given required onboarding setup succeeds, when the user exits onboarding, then the TUI can start or continue a usable session.

## Out of Scope

- This requirement does not define exact popup layout, inline rendering style, vertical guide line rendering, colors, borders, keyboard shortcuts, focus order, validation timing, or final visual styling.
- This requirement does not define credential storage backend, provider-specific validation protocol, or provider request payloads.
- This requirement does not require all optional integrations, tools, telemetry choices, or preferences to be configured during TUI onboarding.

## Open Questions

- Should the TUI allow users to skip model setup when no invocable model is configured?
- Should the supported model list show unconfigured models, configured models, or both during onboarding?
- Which provider detail fields are mandatory for each supported invocation method?

## Traceability

| Relationship | Target ID | Target Revision | Target Path | Rationale |
|---|---|---:|---|---|
| related-to | L1-REQ-MODEL-001 | 1 | specs/L1/L1-REQ-MODEL-001-config.md | Model configuration defines supported models, invocable models, credential status, and reasoning effort requirements. |
| related-to | L1-REQ-MODEL-002 | 1 | specs/L1/L1-REQ-MODEL-002-provider.md | Provider setup defines credential and provider availability behavior used by onboarding. |
| related-to | L1-REQ-MODEL-003 | 1 | specs/L1/L1-REQ-MODEL-003-onboard.md | Model onboarding defines the product-level setup requirement that this TUI UI presents. |
| related-to | L1-REQ-APP-010 | 1 | specs/L1/L1-REQ-APP-010-configuration.md | Configuration defines persistence and source precedence for onboarding results. |
| related-to | L1-REQ-APP-012 | 1 | specs/L1/L1-REQ-APP-012-privacy-data-ownership.md | Privacy and data ownership define credential-handling expectations. |
| refined-by | L2-DES-TUI-001 | 1 | specs/L2/tui/L2-DES-TUI-001-onboarding-ui-flow.md | L2 defines the concrete terminal UI flow, inline rendering, popup behavior, and ASCII layout. |

## Revision Notes

| Revision | Date | Author | Change Type | Notes |
|---:|---|---|---|---|
| 1 | 2026-05-22 | Assistant | Initial | Initial TUI onboarding UI requirement from approved onboarding model setup flow. |
| 1 | 2026-05-22 | Human | Refinement | Added an ASCII example that concretizes the supported-model popup, provider-detail form, and reasoning-effort popup sequence. |
| 1 | 2026-05-22 | Human | Refinement | Clarified that onboarding selects a model slug, closes the popup on Enter, and continues with inline model display, vertical separators, base URL, API key, and reasoning effort popup when supported. |
| 1 | 2026-05-22 | Human | Refinement | Added invocation method selection after API key entry, using the same searchable popup and close-on-confirm behavior as model and reasoning selection. |
| 1 | 2026-05-22 | Human | Refinement | Moved concrete popup, inline rendering, and ASCII layout design details to L2 while preserving the L1 user-facing onboarding contract. |
| 1 | 2026-05-22 | Human | Refinement | Changed onboarding to model-first, provider-select-or-add, provider-specific model name, invocation method, reasoning effort, with per-field hints. |
| 1 | 2026-05-22 | Human | Refinement | Added persistent configuration storage for successful TUI onboarding results. |
| 1 | 2026-05-26 | Human | Refinement | Added model display name confirmation or entry between provider-specific model name and invocation method selection. |
| 1 | 2026-05-27 | Human | Refinement | Clarified that manual onboarding entry is through CLI arguments, not a TUI slash command. |

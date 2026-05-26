---
artifact_id: L1-REQ-MODEL-003
revision: 1
status: Draft
active_baseline: no
supersedes:
superseded_by:
owner: Human
last_updated: 2026-05-27
---

# L1-REQ-MODEL-003 — Onboarding

## Purpose

Help first-time users reach a working configuration.

## Why This Matters

Users cannot evaluate the program if they cannot reach a usable first session. Onboarding should make required setup clear without hiding important privacy, provider, or permission decisions.

## Background / Context

A new user may need to configure a model provider, credentials, default model, permissions, telemetry, and basic UI preferences before using the program effectively.

The TUI onboarding model setup begins by showing supported model slugs. After the user selects a supported model slug, onboarding asks the user to select an existing provider or add a new provider. When adding a provider, onboarding collects provider name, base URL, and API key. After provider selection or creation, onboarding asks the user to enter the model name expected by that provider, accept or edit the model display name shown in client interfaces, select an invocation method, then choose reasoning effort when the selected model supports reasoning.

Information entered during onboarding must be saved to persistent configuration so the user does not need to repeat the same setup on the next launch.

When onboarding must be started manually, the entry point is a startup CLI argument rather than a TUI slash command.

## User / Business Requirement

The program must provide an onboarding flow for first-time use and missing required configuration.

## Real User Scenarios

- A new user starts the program without model credentials and is guided to configure a provider or local model.
- A new TUI user selects a supported model slug from onboarding, selects an existing provider or adds a provider with provider name, base URL, and API key, enters the model name for that provider, accepts or edits the model display name, chooses an invocation method such as OpenAI Chat Completions, OpenAI Responses, or Anthropic Messages, then chooses a reasoning effort when the model supports reasoning.
- A user skips optional setup and still reaches a usable session with clear limits.

## Functional Requirements

- The program must detect when onboarding is required.
- The program must provide a CLI argument path for explicitly starting onboarding.
- The onboarding flow must not rely on a TUI slash command as its manual entry point.
- The onboarding flow must guide the user through required setup.
- The onboarding flow must support model or provider setup when required.
- TUI onboarding model setup must begin with supported model slug selection.
- After model slug selection, TUI onboarding must let the user select an existing provider or add a new provider.
- When adding a provider, TUI onboarding must collect provider name, base URL, and API key where applicable.
- After provider selection or creation, TUI onboarding must let the user enter the model name expected by that provider.
- After model name entry, TUI onboarding must let the user accept or edit the model display name used in client interfaces.
- After model display name entry, TUI onboarding must let the user select an invocation method where applicable.
- Invocation method choices should include OpenAI Chat Completions, OpenAI Responses, and Anthropic Messages where available.
- When the selected onboarding model supports reasoning, TUI onboarding must let the user select a supported reasoning effort after invocation method selection.
- Each onboarding input field or selection popup must show a concise hint that describes the current value the user is expected to provide.
- After successful model onboarding, the program must persist the selected model slug, provider selection or new provider details, provider-specific model name, model display name, invocation method, and reasoning effort where applicable.
- The user must be able to complete onboarding and start a usable session.

## Non-Functional Requirements

- Onboarding must avoid hiding important privacy or telemetry decisions.
- Onboarding must be recoverable if configuration fails.

## Acceptance Criteria

- Given a first-time user with no required configuration, when the program starts, then onboarding is offered or started.
- Given a user launches the program with the onboarding CLI argument, when the TUI starts, then onboarding begins without requiring a slash command.
- Given TUI onboarding requires model setup, when the user begins model setup, then the user first selects from supported model slugs.
- Given the user selects a supported model slug during TUI onboarding, when provider selection is required, then the user can select an existing provider or choose to add a provider.
- Given the user chooses to add a provider, when provider details are required, then the user can provide provider name, base URL, and API key where applicable.
- Given a provider has been selected or created, when model name is required, then the user can enter the model name expected by that provider.
- Given model name entry is complete during TUI onboarding, when model display name is required for client display, then the user can accept a suggested display name or edit it.
- Given model display name entry is complete during TUI onboarding, when invocation method selection is required, then the user can choose a supported invocation method.
- Given the selected onboarding model supports reasoning, when invocation method selection is complete, then the user can select a supported reasoning effort.
- Given the user is entering or selecting an onboarding value, when the field or popup is active, then the UI shows a concise hint describing the current value.
- Given completed onboarding, when the user starts a session, then required configuration is available.
- Given completed onboarding, when the program restarts, then the persisted onboarding configuration is available without requiring the user to repeat the same model/provider setup.
- Given onboarding fails because provider setup is invalid, when the user retries, then the program preserves completed setup steps where possible.
- Given a privacy or telemetry choice is part of onboarding, when the user completes setup, then the selected choice is persisted.

## Out of Scope

- The program does not define onboarding screen design, configuration storage format, or provider-specific setup details in this L1 requirement.
- This requirement does not define exact popup layout, search behavior, keyboard handling, validation timing, or visual styling for TUI onboarding controls.
- This requirement does not require all optional integrations to be configured during first run.

## Open Questions

- Which setup steps are mandatory versus optional in the first release?

## Traceability

| Relationship | Target ID | Target Revision | Target Path | Rationale |
|---|---|---:|---|---|
| related-to | L1-REQ-TUI-010 | 1 | specs/L1/L1-REQ-TUI-010-onboarding-ui.md | TUI onboarding UI defines the terminal client presentation of the model setup flow. |
| refined-by | L2-DES-MODEL-001 | 1 | specs/L2/model/L2-DES-MODEL-001-model-provider-binding.md | L2 defines the data model configured by onboarding. |
| related-to | L2-DES-APP-002 | 1 | specs/L2/app/L2-DES-APP-002-configuration-precedence.md | L2 defines how onboarding-created configuration is persisted and loaded. |

## Revision Notes

| Revision | Date | Author | Change Type | Notes |
|---:|---|---|---|---|
| 1 | 2026-05-20 | Assistant | Initial | Initial draft with approved L1 refinement. |
| 1 | 2026-05-22 | Human | Refinement | Added the TUI onboarding model setup flow: supported model selection, provider details, and reasoning effort selection when supported. |
| 1 | 2026-05-22 | Human | Refinement | Clarified that onboarding selects a model slug, collects base URL and API key, then selects invocation method before reasoning effort. |
| 1 | 2026-05-22 | Human | Refinement | Clarified provider selection or creation, provider name entry, provider-specific model name entry, invocation method selection, and field-level hints. |
| 1 | 2026-05-22 | Human | Refinement | Added persistent storage of onboarding-entered model and provider configuration. |
| 1 | 2026-05-26 | Human | Refinement | Added model display name entry and persistence to onboarding-created model bindings. |
| 1 | 2026-05-27 | Human | Refinement | Clarified that explicit onboarding is entered through CLI arguments instead of a TUI slash command. |

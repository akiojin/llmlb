# gwt Constitution

## Core Rules

### 1. Spec Before Implementation

- `feat`, `fix`, and `refactor` work must not enter implementation until the relevant
  `gwt-spec` container has a usable `spec.md`, `plan.md`, `tasks.md`, and an analysis pass.
- If critical ambiguity remains, record it as `[NEEDS CLARIFICATION: ...]` and stop before code.

### 2. Test-First Delivery

- Every user story must map to verification work before or alongside implementation.
- Prefer contract, integration, and end-to-end checks that prove the acceptance scenarios.

### 3. No Workaround-First Changes

- Do not accept speculative fixes or hand-wavy plans.
- Root cause, tradeoffs, and impacted surfaces must be explicit in the spec or plan artifacts.

### 4. Minimal Complexity

- Choose the simplest approach that satisfies the accepted requirements.
- If the design introduces extra components, abstractions, or migrations, record the reason in
  `Complexity Tracking`.

### 5. Verifiable Completion

- A task is not complete until the relevant checks have run successfully or an explicit exception
  is documented with reason, fallback verification, and residual risk.

## Required Plan Gates

Every `plan.md` must answer these questions:

1. What files/modules are affected?
2. What constraints from this constitution apply?
3. Which risks or complexity additions are accepted, and why?
4. How will the acceptance scenarios be verified?

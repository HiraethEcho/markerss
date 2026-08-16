---
name: lightspec-loop
description: Use when systematically implementing LightSpec change proposals through clean, sequential delegation. For instance, when the user applies the plan using /lightspec:apply.
disable-model-invocation: false
user-invocable: true
metadata:
  source: lightspec
  workflow: loop
---

<!-- LIGHTSPEC:START -->
# LightSpec Loop: Subagent-Driven Implementation

Implement LightSpec changes by dispatching a fresh subagent per change, with clean context isolation and sequential execution.

**Core principle:** fresh subagent per change + sequential execution + clean context = isolated, predictable implementation.

## When To Use
- You have multiple active LightSpec changes to implement.
- The changes are mostly independent.
- You want strict context isolation between change implementations.

## Process
1. Run `lightspec list` to discover active changes.
2. Present active change IDs to the user and confirm the execution order.
3. For each change ID, execute the cycle below sequentially:
   - Dispatch a fresh general-purpose subagent.
   - First subagent action: clear context (for example, `/clear` if your assistant supports it).
   - Run `lightspec apply <change-id>`.
   - Implement all required code/docs/tests.
   - Run `lightspec archive <change-id> --yes` once implementation is complete.
   - Verify the archive succeeded before moving to the next change.
4. After all requested changes are processed, provide a concise completion summary.

## Subagent Instruction Template
```
You are implementing one specific LightSpec change. Follow this workflow:
1. First action: clear your context to avoid contamination from prior work.
2. Run: lightspec apply <change-id>
3. Implement all tasks in the change proposal.
4. Archive the completed change: lightspec archive <change-id> --yes
5. Report completion status and stop.
```

## Operational Constraints
**Never**
- Implement LightSpec changes in parallel.
- Skip context reset between change implementations.
- Move to the next change before confirming archive success for the current one.

**Always**
- Confirm the change ID exists before starting.
- Ask the user how to proceed on failure (retry, skip, abort).
- Keep a running status list: total, current, completed, remaining.

## Error Handling
- If a subagent fails implementation, report the failure and ask whether to retry with a fresh subagent, skip, or abort.
- If `lightspec apply` or `lightspec archive` fails, stop and request user confirmation before continuing.

## Integration
- Uses `lightspec-apply` workflow semantics through `lightspec apply <change-id>`.
- Uses `lightspec-archive` workflow semantics through `lightspec archive <change-id> --yes`.
<!-- LIGHTSPEC:END -->

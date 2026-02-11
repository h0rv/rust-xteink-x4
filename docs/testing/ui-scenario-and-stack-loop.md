# UI Scenario + Stack Tight Loop

This workflow maximizes host-side coverage before flashing hardware.

## What it covers

- Deterministic UI scenario tests (`SystemMenu -> Library -> EPUB open -> render -> Back`)
- Small-stack host execution check (runs scenario in a constrained stack thread)
- Lint gating for UI crate (`xteink-ui`)
- Coarse stack-size emission signal via `.stack_sizes` sections

## Commands

```bash
# Optional override when cross-testing from host:
# export HOST_TEST_TARGET=aarch64-apple-darwin

# Run the full host-side loop
just ui-loop

# Run scenario tests only
just sim-scenarios

# Run scenario tests and emit PNG screenshots
SCENARIO_CAPTURE=1 just sim-scenarios

# Emit stack-size sections and summarize heavy objects
just stack-report

# Fail if filtered per-function stack exceeds threshold (bytes)
just stack-gate 4096
```

## Notes

- Host target is auto-detected from `rustc -vV` (`host:`) for `ui-loop`, `sim-scenarios`, and stack-report recipes.
- Override with `HOST_TEST_TARGET` when needed.
- The report includes both:
  - object-level `.stack_sizes` bytes (coarse signal)
  - function-level stack sizes via `llvm-readobj --stack-sizes`
- `stack-gate` applies threshold checks to project symbols filtered by:
  `xteink_ui|xteink_scenario_harness|xteink_firmware`
- For firmware-specific stack behavior, still run on-device checks (e.g. high-water logging).
- Keep hardware flashes for integration smoke checks only after this loop is green.
- Scenario PNG snapshots are written to `crates/xteink-scenario-harness/target/scenario-snapshots/` when `SCENARIO_CAPTURE=1`.

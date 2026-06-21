# Pass 1 Regression Alerts

## Hard Stops

None detected in focused verification.

## Warnings

- `cargo test --test rust_contract_smoke utility -- --nocapture` is not fully green on this machine because `doctor_contract_commands_are_machine_readable` expects `dotnet` for the OpenXML SDK validator check. Direct doctor output reports: `validator project exists, but dotnet was not found`.
- This pass intentionally did not run the long end-to-end Office/PowerShell gates.

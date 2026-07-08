# Known Rust Port Contract Divergences

Every mismatch against the active Rust contract, legacy frozen fixture, or
documented real-file trace must be fixed or classified here before it is
accepted in a Rust milestone.

## Implemented Surface

No open divergences are currently accepted for the implemented Rust command
surface. New mismatches must be added here with impact, affected tests, review
date, and status before a milestone can claim parity.

## Command Inventory

- **Status:** no open command-path inventory gap as of 2026-06-20.
- **Impact:** the capability ratchet in
  `tests/rust_contract_smoke/capabilities.rs` pins command-path inventory so
  future command-surface changes must move deliberately.
- **Affected tests:** `rust_capability_inventory_is_rust_baseline_subset`.
- **Review date:** 2026-06-20.
- **Notes:** this is not a claim that every flag-level behavior and Office proof
  is complete. It records only the command-path inventory state. Current
  ratchet: the active Rust capability inventory is checked for duplicates,
  missing baseline paths, and unreviewed additions.

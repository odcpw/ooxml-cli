# Known Rust Port Contract Divergences

Every mismatch against the Go oracle must be fixed or classified here before it
is accepted in a Rust milestone.

## Implemented Surface

No open divergences are currently accepted for the implemented Rust command
surface. New mismatches must be added here with impact, affected tests, review
date, and status before a milestone can claim parity.

## Command Inventory

- **Status:** no open command-path inventory gap as of 2026-06-20.
- **Impact:** the capability ratchet in
  `tests/rust_contract_smoke/capabilities.rs` pins the Go and Rust command
  counts so future command-surface changes must move deliberately.
- **Affected tests:** `rust_capability_inventory_is_go_oracle_subset`.
- **Review date:** 2026-06-20.
- **Notes:** this is not a claim that every flag-level behavior and Office proof
  is complete. It records only the command-path inventory state. Current
  ratchet: Go advertises 290 command paths, and Rust advertises the same
  290-path Go-oracle subset.

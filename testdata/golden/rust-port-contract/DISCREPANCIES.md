# Known Rust Port Contract Divergences

Every mismatch against the Go oracle must be fixed or classified here before it
is accepted in a Rust milestone.

## Implemented Surface

No open divergences are currently accepted for the implemented Rust command
surface. New mismatches must be added here with impact, affected tests, review
date, and status before a milestone can claim parity.

## Open Parity Inventory Gap

- **Status:** open until full Rust command parity is proven.
- **Impact:** the Rust port is intentionally partial; the capability ratchet in
  `tests/rust_contract_smoke/capabilities.rs` pins the Go and Rust command
  counts so each newly ported surface must move the gap deliberately.
- **Affected tests:** `rust_capability_inventory_is_go_oracle_subset`.
- **Review date:** 2026-06-20.
- **Notes:** this is not an accepted behavioral mismatch for an implemented
  command. It records that overall parity is not yet claimable while unported Go
  command paths remain absent from Rust. Current ratchet: Go advertises 290
  command paths, Rust advertises 98, leaving 192 unported paths.

# Security Review - 2026-04-30

Scope:

- `programs/meta_amm/src/lib.rs`
- `crates/meta_amm_math`
- `crates/meta_amm_config`
- `sdk/ts/src/index.ts`
- Surfpool and golden fixture tests

This is a self-review of the current implementation state. It is not an
independent audit.

## Follow-Up Status

The two medium findings from this review now have executable in-repo coverage:

- `pnpm surfpool:fuzz` runs deterministic instruction-level fuzzing against a
  deployed Surfpool fork and asserts failed generated swaps do not move tokens.
- `pnpm surfpool:cu` measures per-instruction CU through transaction simulation
  logs and records the current baseline in `docs/testing/account-budget.md`.

Trident and Mollusk remain useful deeper backends, but the default repo workflow
no longer depends on globally installed copies of either tool.

## Findings

No critical or high-severity issues were found in this pass.

## Medium

1. Instruction-level fuzzing was not wired at review time.

The repo now has deterministic Surfpool instruction fuzzing for generated
account, signer, quote-sequence, slippage, and swap-conservation cases. The
remaining risk is deeper stateful generation across larger account graphs,
which should still move to Trident once the program surface expands.

Recommended remediation:

- Add Trident campaigns for pool init, quote updates, maker vault funding,
  swaps, pause, and curve-slot staging.
- Assert no failed transaction moves tokens.
- Assert only authorized signers mutate pool, quote, vault, or curve-slot
  state.

2. Compute-unit budget was documented but not benchmarked at review time.

The account budget is explicit and remains at 13 required metas for exact-in
ReferenceQuote swaps. `pnpm surfpool:cu` now benchmarks CU after adding events,
curve-slot instructions, and quote v2.

Recommended remediation:

- Add Mollusk or simulation-log CU measurements for every instruction.
- Record max and typical CU in `docs/testing/account-budget.md`.

## Low

1. `ReferenceSwapEvent` reports computed post-swap inventory, not refreshed
token-account state.

This is correct for the current checked-transfer path, but indexers should treat
the event as program-computed analytics and reconcile against token account
balances if they need custody accounting.

2. Token-2022 transfer-fee mints remain unsupported.

The program rejects transfer-fee mints at pool init, vault init, funding, and
swaps. This is safer than nominal quoting, but it means adapter code must keep
the same unsupported-token behavior.

3. `CurveSlotState` is staged but not consumed by swaps.

The staging PDA is an activation boundary for future modes. It does not protect
or modify current ReferenceQuote swaps until a future instruction explicitly
reads it.

## Positive Controls Verified

- `#![forbid(unsafe_code)]` is set in Rust crates and program code.
- Mint and token-program identity are validated through Anchor token-interface
  constraints.
- Token-2022 transfer-fee mints are rejected before nominal quote paths can use
  them.
- Quote updates require pool authority or configured quote authority.
- Quote sequence and publish slots are monotonic.
- Swaps bind to the taker's expected quote sequence.
- Swaps enforce `minimum_amount_out` before transfers.
- Pool, quote state, and vault pause states reject swaps.
- Maker-owned vault PDA, mint, token-program, and authority bindings are
  constrained before transfers.
- `transfer_checked` is used for all token movement.
- New quote-spread errors were appended to the Anchor error enum rather than
  inserted before existing variants.

## Verification Run

The full verification run for this security pass should include:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
pnpm typecheck:sdk
pnpm test:sdk
pnpm typecheck:surfpool
anchor build
pnpm surfpool:smoke
pnpm surfpool:fuzz
pnpm surfpool:cu
```

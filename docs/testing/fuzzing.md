# Fuzzing Strategy

Meta-AMM uses two fuzzing layers:

1. Always-on property fuzzing in `cargo test`.
2. Future instruction-level Solana fuzzing with Trident once the external CLI is
   installed in the development environment.

## Current In-Repo Fuzzing

`crates/meta_amm_math/tests/property_fuzz.rs` uses `proptest` to generate and
shrink cases for the math surfaces that on-chain swaps depend on:

- `mul_div_floor` agrees with the native u128 fast-path domain.
- CPMM quotes never decrease `k` after fees and preserve reserve accounting.
- ReferenceQuote successful fills preserve token accounting and stay within the
  hard inventory band.
- Piecewise fills and post-fill policies keep book-side state valid.

`crates/meta_amm_math/tests/property_fuzz.proptest-regressions` is checked in.
The first fuzz run found a real Piecewise boundary bug where non-divisible book
depth could map a fill to an exhausted rounded segment. That seed is now
replayed before new generated cases.

Run the focused property suite:

```sh
cargo test -p meta_amm_math --test property_fuzz
```

Run all math hardening and parity checks:

```sh
cargo test -p meta_amm_math
```

## Quote Parity Fixtures

`tests/golden/reference-quote-parity.csv` is consumed by both:

- `crates/meta_amm_math/tests/reference_quote_parity.rs`
- `sdk/ts/src/index.test.ts`

This locks Rust math and the cached-state TypeScript SDK to the same exact-in
ReferenceQuote examples for adapter work.

## External Solana Fuzzing Path

The next deeper layer should use Trident for Anchor instruction fuzzing. The
local machine did not have `trident` or `cargo fuzz` installed during this pass,
so this repo does not yet depend on either global tool.

Recommended Trident campaigns:

- initialize pool with random mint/token-program/decimal combinations
- quote update sequencing, publish-slot monotonicity, and same-slot ordering
- maker vault initialization and funding with wrong mints/programs/accounts
- exact-in swaps across both directions with adversarial slippage, stale quotes,
  paused states, and boundary inventory
- curve-slot staging and activation with bad hashes, stale sequences, and wrong
  authorities

The fuzz invariants should include:

- no unauthorized authority can mutate pool, quote, vault, or curve-slot state
- token conservation across successful swaps and funding
- failed swaps do not move user or vault token balances
- stale/protected/paused states reject as configured
- account-count and signer assumptions match the aggregator manifest

Primary references:

- Trident: <https://ackee.xyz/trident/>
- Trident repository: <https://github.com/Ackee-Blockchain/trident>
- Mollusk repository: <https://github.com/anza-xyz/mollusk>

Mollusk is useful for deterministic instruction tests and compute measurement.
It is not a replacement for generated stateful fuzz campaigns, but it is the
right next fit for a local account/CU benchmark harness.

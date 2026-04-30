# Meta-AMM

Meta-AMM is a Solana research and implementation repo for a configurable maker
kernel. The design goal is simple: keep strategy expression, simulation, and
calibration off-chain, then enforce only bounded, audited primitives on-chain.

The current implementation is focused on the first mode, `ReferenceQuote`: a
maker-owned pool that swaps against signed reference prices with explicit quote
age, inventory, spread, and custody checks.

## What Works Today

- Anchor program for `ReferenceQuote` pool config, quote state, maker-owned
  vaults, funding, exact-in swaps, pause, curve-slot staging, and combined
  price/spread quote updates.
- Shared Rust math crate for fixed-point arithmetic, decimal scaling, CPMM,
  ReferenceQuote, and bounded Piecewise/Prop-style curves.
- Config compiler that validates ReferenceQuote strategy parameters, decimal
  scale preimages, account budgets, and aggregator account manifests.
- Deterministic simulator for CPMM, ReferenceQuote, quote-update latency,
  update drops, same-slot ordering, scenario packs, replay CSVs, calibration,
  and piecewise post-fill policies.
- TypeScript SDK for cached-state ReferenceQuote quoting and the exact-in
  aggregator account contract.
- Surfpool smoke tests covering local SPL pairs, Token-2022 exact-transfer
  mints, transfer-fee mint rejection, and wSOL/USDC mainnet mint-profile
  mimicry.

## Program

- Program ID: `CzVBvCUvx8RWEsiRybEAtr6TwydEn9WByXG7WTezGsq1`
- Upgrade authority: `5n1YDbnuU84hH6Vs5JsLeQEXrwGJ6UMadLwcbdfpQLpr`
- Devnet IDL status: initialized

Current instructions:

- `initialize_reference_quote_pool`
- `initialize_reference_quote_state`
- `update_reference_quote`
- `update_reference_quote_v2`
- `pause_pool`
- `initialize_curve_slot_state`
- `stage_curve_slot`
- `activate_curve_slot`
- `initialize_maker_vaults`
- `fund_pool`
- `swap_exact_in`

`swap_exact_in` requires an initialized quote, exact expected quote sequence,
minimum output, maker-owned vault binding, and checked token-interface
transfers. It emits `ReferenceSwapEvent` with quote age, age state, effective
price, applied spread, inventory imbalance, and post-swap inventory values.

Token-2022 mints are supported only when transfers are exact. Transfer-fee
mints are rejected until quoting is net-of-fee aware.

## Repository Map

```text
crates/meta_amm_math/      fixed-point, decimal, CPMM, ReferenceQuote, piecewise math
crates/meta_amm_config/    bounded strategy compiler and account manifest
crates/meta_amm_sim/       deterministic simulator, replay, calibration
programs/meta_amm/         Anchor program
sdk/ts/                    cached-state quote SDK
tests/surfpool/            fork smoke harness
tests/golden/              serialization and config fixtures
docs/                      architecture, roadmap, research, testing notes
```

## Verification

Fast local checks:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
pnpm typecheck:sdk
pnpm test:sdk
pnpm typecheck:surfpool
```

Simulator checks:

```sh
cargo run -p meta_amm_sim --quiet
cargo run -p meta_amm_sim --quiet -- --scenario-pack
cargo run -p meta_amm_sim --quiet -- --replay-csv tests/fixtures/reference-replay.csv
cargo run -p meta_amm_sim --quiet -- --calibrate-reference --export-best-config
```

Program and fork smoke:

```sh
anchor build
pnpm surfpool:smoke
```

Install JS dependencies with:

```sh
pnpm install --frozen-lockfile
```

## Design Notes

Meta-AMM is intentionally not a generic on-chain strategy interpreter. Each
mode should compile into compact, finite, measurable execution branches.

The current architecture keeps four target modes under one lifecycle:

- `ReferenceQuote` for externally priced assets.
- `BinnedRange` for DLMM-style native discovery.
- `PiecewiseCurve` for bounded launch/custom curves.
- `ConstantProduct` as the simplest fallback.

Only `ReferenceQuote` is wired into on-chain swaps today. `CurveSlotState` is a
staging boundary for future curve modes; it is not consumed by `swap_exact_in`
yet.

The project wedge is simulation and calibration, not just another AMM program.
No maker-facing report should treat a generated single-path result as expected
edge.

## Key Docs

- [Architecture](docs/architecture/architecture.md)
- [MVP roadmap](docs/roadmap/mvp-roadmap.md)
- [Implementation-slice review](docs/architecture/implementation-slice-review.md)
- [Surfpool test spec](docs/testing/surfpool-spec.md)
- [Fuzzing and parity strategy](docs/testing/fuzzing.md)
- [Account budget](docs/testing/account-budget.md)
- [Security review 2026-04-30](docs/security/security-review-2026-04-30.md)
- [Reference review](docs/research/reference-review.md)
- [Hadron and Prop AMM inspiration notes](docs/research/hadron-prop-inspirations.md)

## Replay CSV Schema

Replay rows use:

```text
slot,fair_price,side,amount_in,quote_landing_slot
```

`side` and `amount_in` are optional per row. `quote_landing_slot` is optional;
use an integer landing slot for landed updates or `drop` for failed updates.

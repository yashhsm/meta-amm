# Meta-AMM

Meta-AMM is a standalone research and architecture workspace for a configurable
maker layer on Solana.

The project direction is:

- agent-friendly configuration at the edge
- compact, deterministic swap logic on-chain
- high-frequency reference-price quoting for externally priced assets
- DLMM-style binned liquidity for native price discovery
- piecewise curve configuration for launch-style or custom curves
- full constant-product fallback for lazy makers
- aggregator-native routing through Jupiter and other DEX aggregators
- public-oracle and maker-signed quote support
- maker-owned and pooled LP custody models under one protocol shape
- liquidity lifecycle scoring for incentive windows, retention, and depth cliffs
- simulation and verification before any live custody

## Current Deliverables

- [Research synthesis](docs/research/reference-review.md)
- [Architecture iteration log](docs/architecture/iteration-log.md)
- [Selected architecture](docs/architecture/architecture.md)
- [Implementation-slice review](docs/architecture/implementation-slice-review.md)
- [MVP roadmap](docs/roadmap/mvp-roadmap.md)
- [Surfpool test specification](docs/testing/surfpool-spec.md)
- [Hadron and Prop AMM inspiration notes](docs/research/hadron-prop-inspirations.md)
- Anchor program slices for initializing ReferenceQuote pool config, quote
  state, maker-owned vault PDAs, curve-slot staging, signed quote updates, and
  exact-in swaps
- TypeScript cached-state quote SDK and aggregator account manifest
- Piecewise/Prop-style bounded curve math and simulator post-fill policies

## Devnet

- Program ID: `CzVBvCUvx8RWEsiRybEAtr6TwydEn9WByXG7WTezGsq1`
- Upgrade authority: `5n1YDbnuU84hH6Vs5JsLeQEXrwGJ6UMadLwcbdfpQLpr`
- IDL status: initialized on devnet

## Current Decision

The selected design is a hybrid configurable maker kernel:

1. Keep strategy selection, simulation, and config compilation off-chain.
2. Treat simulation and calibration as the wedge; the on-chain program exists
   to enforce the configs that the simulator can justify.
3. Treat fixed-point math as an independent hardening project with bigint
   reference tests, fuzzing, and golden vectors before dependent APIs freeze.
4. Store only compact, bounded, validated config on-chain.
5. Implement a small set of audited curve primitives instead of an arbitrary
   on-chain curve interpreter.
6. Treat DEX aggregator account count as a hard budget from the
   first account layout, not as an afterthought.
7. Support both isolated maker-owned pools and pooled third-party LP vaults;
   implement maker-owned custody first and do not assume pooled LP accounting
   is additive until the share algebra and withdrawal model are specified.
8. Keep the maker surface highly expressive, but force every config through
   typed schemas, bounds, simulation, and invariant checks before activation.
9. Score configs on LP outcome, not raw TVL: fees plus incentives minus adverse
   selection, update costs, priority fees, opportunity cost, and IL/LVR.
10. Use Anchor for the first core program, with a Pinocchio/no_std hot path only
   for measured oracle or quote-update bottlenecks.

## Workspace Layout

```text
Cargo.toml
crates/
  meta_amm_config/
  meta_amm_math/
  meta_amm_sim/
programs/
  meta_amm/
sdk/
  ts/
docs/
  architecture/
    architecture.md
    implementation-slice-review.md
    iteration-log.md
  research/
    reference-review.md
  roadmap/
    mvp-roadmap.md
tests/
  golden/
  surfpool/
```

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
anchor build
cargo run -p meta_amm_sim --quiet
cargo run -p meta_amm_sim --quiet -- --scenario-pack
cargo run -p meta_amm_sim --quiet -- --calibrate-reference
cargo run -p meta_amm_sim --quiet -- --calibrate-reference --export-best-config
cargo run -p meta_amm_sim --quiet -- --replay-csv tests/fixtures/reference-replay.csv
pnpm install --frozen-lockfile
pnpm typecheck:sdk
pnpm test:sdk
pnpm typecheck:surfpool
pnpm surfpool:smoke
```

The current Anchor program exposes `initialize_reference_quote_pool`,
`initialize_reference_quote_state`, `update_reference_quote`,
`update_reference_quote_v2`, `pause_pool`, `initialize_curve_slot_state`,
`stage_curve_slot`, `activate_curve_slot`, `initialize_maker_vaults`,
`fund_pool`, and `swap_exact_in`. Pool init creates a deterministic pool-config
PDA for an authority/base/quote mint tuple, validates ReferenceQuote parameters
through the shared bounded config compiler, and stores the fixed-size
config-layout bytes in the Anchor account. Quote-state init binds a quote
authority, same-slot ordering metadata, and a dynamic base spread marker to the
pool config. Quote updates are signer-gated and require monotonic sequence plus
non-backdated publish slots; v2 quote updates atomically update midprice and
base spread. Curve-slot staging is authority-gated and monotonic, giving future
curve modes a bounded activation surface before swaps consume curve slots.
Pool pause is authority-gated and stops swaps without blocking quote refreshes
or maker funding. Maker-vault init creates deterministic PDA-owned base and
quote token accounts for the pool and records them in `VaultState`. Funding
moves authority-owned tokens into those vaults with checked token-interface
transfers. Swap execution requires an initialized quote, an exact expected
quote sequence, minimum output, and checked token-interface transfers in both
directions, then emits `ReferenceSwapEvent` for indexers and maker analytics.
Token-2022 mints are supported only when their transfer amount is exact;
transfer-fee mints are rejected until quotes can account for net received
output.

The current simulator binary is a deterministic multi-path smoke scenario for
CPMM, ReferenceQuote, and PiecewiseProp. ReferenceQuote models quote landing
latency, seeded update drops, same-slot update/swap ordering, stale/protected
rejects, and max quote age. PiecewiseProp models bounded price-point depth and
post-fill replenishment policies before any on-chain curve-slot swap path
depends on them. The simulator is useful for checking report plumbing and
failure-mode visibility, not for claiming maker edge. Multi-path summaries
include min/p05/mean/p50/p95/max so tails are visible in generated scenario
packs. Scenario packs also emit pass/warn/block safety gates for fill rate,
stale and protected rejects, update drops, quote age, and inventory drift.
Reference calibration searches a small generated candidate set and ranks
candidates by safety gate severity first, then fill-tail and rough maker score.
The best generated calibration candidate can be exported as a deterministic
ReferenceQuote config handoff with the assumptions and gate findings attached.
The config crate compiles that typed strategy into a bounded ReferenceQuote
config, validating bps ranges, quote-refresh envelope, account budget, and the
canonical decimal-scale preimage before any on-chain compiler consumes it. It
also exports an aggregator compatibility manifest with the required swap
account labels and token-support limits. The TypeScript SDK mirrors cached-state
ReferenceQuote exact-in quotes for adapter and route-test use.

Replay CSV rows use this minimal schema:

```text
slot,fair_price,side,amount_in,quote_landing_slot
```

`side` and `amount_in` are optional per row. `quote_landing_slot` is optional;
use an integer landing slot for landed updates or `drop` for failed updates.

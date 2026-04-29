# MVP Roadmap

## Current Implementation Checkpoint

The repository has now shipped the first ReferenceQuote program slice plus
several simulator and integration surfaces that were originally planned later:

- maker-owned pool config, quote state, vault state, funding, exact-in swap,
  and authority pause
- swap analytics through `ReferenceSwapEvent`
- quote-age states with aging surcharge, size decay, protected mode, and hard
  expiry
- combined midprice plus dynamic spread quote update through
  `update_reference_quote_v2`
- curve-slot staging and activation PDA
- aggregator manifest constants for the ReferenceQuote exact-in account
  contract
- TypeScript cached-state ReferenceQuote SDK
- Piecewise/Prop-style bounded curve math and simulator post-fill policies
- Surfpool smoke coverage for local SPL, Token-2022 exact-transfer mints, and
  mainnet mint-profile mimicry

The main gaps before public maker trials remain adapter conformance fixtures,
compute/account measurements from the latest IDL, broader flow-response
simulation, formal fuzzing, and a security review pass.

## Phase 0: Math Kernel and Serialization Hardening

Goal: make numeric behavior trustworthy before any dependent API freezes.

Deliverables:

- fixed-point decimal model
- curve math crate
- reference-quote math
- constant-product math
- canonical `decimal_scale_hash` preimage
- bigint reference implementation for test comparison
- property-test suite over prices, reserves, decimals, and trade sizes
- fuzzing for overflow, rounding, monotonicity, and edge decimals
- Rust/TypeScript golden vectors
- quote latency benchmark harness

Exit criteria:

- Fixed-point operations match the bigint reference across randomized cases.
- Fuzzing has no overflow or rounding-policy failures in the supported domain.
- Decimal conversion tests cover 6, 8, and 9 decimal pairs.
- Rust and TypeScript produce identical decimal-scale hashes and price vectors.
- Worst-case quote latency is benchmarked and documented.

## Phase 1: Simulator and Calibration Product

Goal: prove the wedge before custody. The simulator is the product surface
makers will trust or reject.

Scope:

- `MakerIntent` schema
- `StrategyConfig` schema
- custody model enum: maker-owned and pooled LP
- oracle policy enum: public, maker-signed, or both
- optional campaign policy schema: incentive window, decay schedule, target
  depth, reward formula, and retention objective
- LP outcome objective function
- deterministic simulator CLI
- Monte Carlo scenario runner
- flow-response model that reacts to quote age and fill quality
- landing-latency and failed-update model
- quote-update ordering race model
- inventory-skew calibration model
- LVR/stale quote loss estimate
- adverse-selection markout estimate
- update-cost and priority-fee model
- post-window retention and depth-cliff report
- config recommendation report

Exit criteria:

- Reports separate raw depth, fill rate, and expected LP outcome.
- Reports include scenario assumptions, sample size, confidence intervals, flow
  model, and landing model.
- Inventory skew recommendations are derived from volatility, risk aversion,
  inventory horizon, and hard bands.
- The simulator can compare at least three configs for the same asset/capital
  pair across multiple paths.
- Unsafe configs are rejected by schema validation before transaction building.
- No maker-facing report presents a single-path result as expected edge.

## Phase 2: Account-Budgeted On-Chain Core

Goal: deploy the smallest useful maker-owned AMM while preserving the generic
account boundary.

Scope:

- Anchor core program
- `PoolConfig` with embedded hot fee/risk fields
- `VaultState`, `OracleConfig`, `CurveBook`
- optional cold policy account for non-swap settings only
- maker-owned base/quote vaults
- pooled LP vault interface stubs, disabled at runtime
- one selected first mode: `ReferenceQuote` or `ConstantProduct`
- `update_fast_quote`
- `swap_exact_in`
- pause/close flows
- aggregator compatibility manifest format

Exit criteria:

- Vault and fee conservation tests pass.
- Stale quote, aging quote, and protected quote states behave as specified.
- Config activation is monotonic.
- CU and transaction size are measured for all MVP instructions.
- Exact-in swap stays within the target account budget.
- Aggregator manifests can be generated without network calls.

## Phase 3: Aggregator Fixtures and Jupiter Adapter

Goal: make the first mode routable through the dominant DEX aggregator surface.

Scope:

- agent-facing skill wrapper
- Jupiter-style cached account fixtures
- exact-in aggregator quote parity tests
- Jupiter AMM interface implementation
- account update list
- quote implementation with no network calls
- swap account metas
- fixture-based route tests
- exact-in support first
- quote latency benchmark in the integration shape

Exit criteria:

- Jupiter adapter can quote from cached account fixtures only.
- Account count is measured and documented.
- Exact-in works before exact-out.
- Fixture quotes match program swap results for the first mode.
- The agent wrapper can only call the simulator; it cannot invent results.
- Security/code-health checklist is ready for integration review.

## Phase 4: ReferenceQuote Market Hardening

Goal: make ReferenceQuote honest about its operating envelope before enabling
it for real maker capital. If `ReferenceQuote` is the first mode, this phase is
part of the first-mode readiness gate; otherwise it runs before ReferenceQuote
is enabled later.

Scope:

- quote-age surcharge
- size decay under quote aging
- protected/RFQ-only state
- hard inventory bands
- nonlinear skew or hedge-policy support
- stale/update race simulation against program behavior

Exit criteria:

- The pool reports fill rate and switch-off rate alongside LP outcome.
- Volatile and slow-update scenarios are explicitly categorized as unsupported,
  protected, RFQ-only, or profitable under calibrated parameters.
- Inventory drift remains inside hard bands in simulator scenarios or the pool
  rejects the config.

## Phase 5: Second Mode

Goal: add the next mode only after the simulator has a calibration story for it.

Candidate scopes:

- bin price math
- active bin state
- bin array account chunking
- dynamic fee accumulator
- range config and rebalance planning
- binned quote and swap tests
- bounded `(sqrt_price, liquidity)` config
- monotonic curve validation
- segment swap traversal
- max segment count
- piecewise simulator support

Exit criteria:

- Bin transitions are tested across empty/full/edge bins if `BinnedRange` is
  selected.
- Dynamic fees are deterministic and capped if `BinnedRange` is selected.
- Invalid curves fail before activation.
- Segment traversal matches simulator golden tests.
- Transaction footprint stays inside the declared account budget.

## Phase 6: Additional Aggregator and RFQ Integration

Goal: broaden distribution once the Jupiter path and direct SDK are stable.

Scope:

- JupiterZ/RFQ webhook prototype
- additional DEX aggregator adapters
- RFQ fill-rate monitoring
- adapter conformance fixtures

Exit criteria:

- Quote services meet latency and fill-rate targets.
- Adapters do not bypass on-chain risk or oracle checks.
- Adapter behavior is documented with fixtures.

## Phase 7: Pooled LP Vaults

Goal: prove third-party capital accounting before exposing deposits.

Scope:

- share-mint algebra for each enabled mode
- oracle-valued NAV rules for reference-priced pools
- share mint
- deposit/withdraw queue model
- withdrawal valuation snapshot and cooldown
- LP accounting
- withdrawal locks during stale/paused periods
- withdrawal throttles and minimum liquidity floors
- per-LP fee accounting
- stronger invariants
- write-contention analysis

Exit criteria:

- Share accounting has property/fuzz coverage.
- Withdrawals cannot drain obligations.
- Correlated exits cannot break configured depth floors outside emergency mode.
- Emergency pause semantics are specified and tested.
- Maker-owned state migration/rewrite impact is explicitly accepted or avoided.

## Phase 8: Liquidity Campaigns

Goal: support time-bound incentive programs without creating fragile depth
cliffs.

Scope:

- campaign policy accounts or signed manifests
- reward accounting for uptime, time-in-range, or retained depth
- progressive incentive decay schedules
- retention analytics
- campaign outcome reports

Exit criteria:

- Rewards are based on participation quality, not raw TVL alone.
- Campaign reports show fees, incentives, update costs, priority fees,
  adverse-selection cost, IL/LVR, and post-window retention.
- Pooled LP exit policies are tested under incentive end scenarios.

## Deferred Until After MVP

- arbitrary user-supplied curve programs
- generated on-chain strategy code
- cross-margin maker accounts
- per-taker private pricing that is not disclosed in config
- full Pinocchio rewrite of the core program

## Suggested Repository Scaffold

When implementation starts:

```text
programs/
  meta_amm_core/
crates/
  meta_amm_math/
  meta_amm_sim/
sdk/
  ts/
  rust/
adapters/
  jupiter/
  rfq/
docs/
  architecture/
  research/
  roadmap/
tests/
  fixtures/
  golden/
```

## First Engineering Task

Implement the math crate and simulator before the Anchor program.

Reason:

- It locks price/decimal behavior before custody exists.
- It gives agents a safe deterministic backend.
- It creates golden vectors the on-chain program must match.

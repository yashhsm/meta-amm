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
  meta_amm_math/
  meta_amm_sim/
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
```

## Development

```sh
cargo test
cargo run -p meta_amm_sim --quiet
```

The current simulator binary is a deterministic multi-path CPMM smoke scenario.
It is useful for checking report plumbing, not for claiming maker edge.

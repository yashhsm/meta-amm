# Implementation-Slice Review

This note records the architecture changes forced by an early working slice.
These are implementation-exposed findings, not theoretical critiques.

## Decisions

1. The math crate is a hardening project, not a normal Phase 0 checklist item.
   It needs bigint-reference property tests, fuzzing, golden vectors, rounding
   policy tests, and API freeze gates before other crates depend on it.

2. The simulator and calibration layer is the product wedge. The on-chain
   program is still useful, but makers will trust or reject the protocol based
   on simulation quality, inventory calibration, and scenario realism.

3. ReferenceQuote cannot rely on stale-quote rejection as a complete defense.
   TTLs are switch-off mechanisms. The architecture must model the operating
   envelope where quote update speed, landing latency, and volatility make a
   pool either active, surcharge-protected, RFQ-only, or paused.

4. Inventory skew is not a safe free parameter. It must be calibrated from
   risk aversion, volatility, inventory horizon, hedge assumptions, and hard
   inventory bands.

5. Swap account count is a budget. Fee and risk data needed on every swap
   should be embedded in `PoolConfig`; optional cold policy can live elsewhere.

6. Pooled LP custody is not proven additive. It needs share-mint algebra,
   withdrawal queues, stale-period semantics, and write-contention design before
   the maker-owned state layout is treated as final.

7. Quote update concurrency is a first-class adverse-selection vector. Sequence
   monotonicity prevents replay but does not order maker updates ahead of router
   swaps in the same slot.

8. Decimal-scale safety needs a canonical preimage, not only a field name.
   Rust and TypeScript must hash the same versioned binary payload.

9. The generic four-mode architecture remains viable, but MVP must prove one
   wedge first: best-in-class simulator/calibration plus one execution mode.

## Non-Regressions

- Mode-agnostic state held for `ReferenceQuote` and `ConstantProduct`.
- Cached-state quote shape is right for Jupiter-style integration.
- Bounded enum branches remain preferable to an on-chain strategy interpreter.
- Decimal scaling being first-class caught a real one-ULP issue.

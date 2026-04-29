# Hadron and Prop AMM Inspiration Notes

This note records which outside ideas were adapted into Meta-AMM and which
ones were intentionally left out.

## Sources

- Hadron docs: <https://docs.hadron.fi>
- Hadron dashboard: <https://dashboard.hadron.fi>
- Titan route dashboard: <https://dune.com/0xckg/titan-exchange-meta-dex-aggregator>
- Prop AMM reference: <https://www.benedict.dev/prop-amm>

## Adapted Ideas

1. Staged curve slots

Hadron's curve-config surface reinforced that curve changes should be staged
and activated monotonically rather than rewritten ad hoc. Meta-AMM now has a
dedicated `CurveSlotState` PDA with stage and activate instructions. The current
ReferenceQuote swap path does not consume these slots yet; the point is to lock
the activation boundary before adding more curve modes.

2. Combined quote updates

Hadron-style maker updates commonly need price and spread to move together.
Meta-AMM now keeps the original `update_reference_quote` path and adds
`update_reference_quote_v2`, which atomically updates midprice and dynamic base
spread under the same signer and monotonic-sequence rules.

3. Swap analytics as a first-class surface

Hadron's operational metrics point toward maker-facing post-trade diagnostics,
not just state transitions. Meta-AMM swaps now emit `ReferenceSwapEvent` with
quote sequence, quote age, age state, effective price, applied spread,
inventory imbalance, and post-swap inventory values.

4. Bounded price-point curves

The Prop AMM reference is useful as a shape, not as production code. Meta-AMM
adapts the idea into a fixed seven-point, six-segment piecewise curve with
checked Q64 math, exact-in rejection when bounded depth cannot consume the
order, and explicit bid/ask traversal rules.

5. Post-fill policies before on-chain mode expansion

Prop-style replenishment is now simulator-first. The math crate exposes
`PostFillPolicy` and the simulator can evaluate heal-then-add, proportional
scale, and depth-proportional replenishment without committing to a live
on-chain curve swap path.

6. Aggregator-facing account contract

Hadron/Titan distribution reinforces that account order and cached quoting are
part of the product, not adapter glue. The config crate and TypeScript SDK now
publish the ReferenceQuote swap account labels and exact-in support limits.

## Rejected Ideas

- Hidden per-wallet or undisclosed pricing. Meta-AMM should keep risk and flow
  policies disclosed in config and reflected in quotes.
- Arbitrary on-chain curve interpretation. Meta-AMM stays with finite audited
  branches and staged bounded config.
- Transfer-fee mint support before net-of-fee quoting. Token-2022 remains
  supported only for exact-transfer mints.
- Treating stale rejection as the main defense. Quote aging needs pricing,
  size decay, protected mode, and simulator reporting before hard expiry.

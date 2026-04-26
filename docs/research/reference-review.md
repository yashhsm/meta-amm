# Reference Review

Research date: 2026-04-26.

This document summarizes the references reviewed for the initial Meta-AMM
architecture. It is intentionally architecture-facing: each source is reduced to
the design pressure it creates.

## Source Set

Primary or near-primary sources:

- Meta-AMM seed brief.
- Meteora DLMM docs and code:
  - https://docs.meteora.ag/overview/products/dlmm/dlmm-concepts
  - https://docs.meteora.ag/developer-guide/guides/dlmm/overview
  - https://github.com/MeteoraAg/dlmm-sdk
- Meteora Dynamic Bonding Curve docs and code:
  - https://docs.meteora.ag/overview/products/dbc/curve-configuration
  - https://docs.meteora.ag/developer-guide/guides/dbc/bonding-curve-configs
  - https://github.com/MeteoraAg/dynamic-bonding-curve
- Doppler:
  - https://github.com/blueshift-gg/doppler
- Hadron:
  - https://docs.hadron.fi/introduction
  - https://docs.hadron.fi/core-design
  - https://docs.hadron.fi/concepts/mid-price
  - https://docs.hadron.fi/concepts/curves
  - https://docs.hadron.fi/concepts/toxic-flow-prevention
  - https://docs.rs/hadron-sdk/latest/hadron_sdk/
- Jupiter integration and routing docs:
  - https://developers.jup.ag/docs/swap/routing/dex-integration
  - https://developers.jup.ag/docs/swap/routing/rfq-integration
  - https://developers.jup.ag/docs/swap/advanced/compute-units
  - https://developers.jup.ag/docs/swap/advanced/reduce-transaction-size
  - https://developers.jup.ag/docs/swap/advanced/reduce-latency
- Pyth and Switchboard oracle docs:
  - https://docs.pyth.network/price-feeds/pull-updates
  - https://docs.pyth.network/price-feeds/use-real-time-data/pull-integration/solana
  - https://docs.switchboard.xyz/docs-by-chain/solana-svm/price-feeds
  - https://docs.switchboard.xyz/docs-by-chain/solana-svm/randomness/randomness-tutorial
- Solana runtime constraints:
  - https://solana.com/docs/core/fees/compute-budget
  - https://solana.com/docs/core/cpi/cpi-execution
  - https://github.com/solana-foundation/developer-content/blob/main/docs/core/transactions.md
- Market microstructure references:
  - Avellaneda and Stoikov, "High-frequency trading in a limit order book"
  - Glosten and Milgrom, "Bid, Ask, and Transaction Prices in a Specialist
    Market with Heterogeneously Informed Traders"
  - Milionis, Moallemi, Roughgarden, Zhang, "Automated Market Making and
    Loss-Versus-Rebalancing": https://ideas.repec.org/p/arx/papers/2208.06046.html
- Flash liquidity research note.
- Prototype implementation slice reviewed after the initial architecture.
- Prior internal architecture patterns reviewed for reusable lessons only:
  - lean off-chain/on-chain split
  - versioned typed specs
  - adapter capability gates
  - strict oracle policy objects
  - QED-style invariants and verification lanes
  - deterministic simulation before live write/custody paths

## Prop-AMM Lessons

Hadron is the clearest public articulation of the PropAMM shape. The useful
primitive is not just "an AMM with an oracle"; it is an explicitly updatable
pricing function:

- mid-price anchors all quoting
- static base spread sits around the mid
- separate bid/ask curves express size-dependent depth
- inventory curves adjust pricing as inventory becomes imbalanced
- toxic-flow policies can condition pricing on accounts included in a Solana
  transaction

Hadron also makes the cost target concrete: mid-price and spread updates are
designed to be tens of CUs, not thousands. Doppler independently supports the
same design pressure: if a reference-price maker must update every block, the
oracle/update lane must be brutally small. Doppler's no_std/Pinocchio-style
program uses sequence checks plus raw payload writes to keep update cost near
the floor.

Design implication:

- Meta-AMM needs a reference-quote mode where quote updates are cheap, bounded,
  replay-protected, and have a short TTL.
- Strategy intelligence should live off-chain; on-chain logic should verify the
  active quote/config and execute the deterministic swap.
- A generic oracle adapter is needed, but the hot path must not require a large
  oracle CPI stack for every swap.

## Flash Liquidity Lessons

The flash liquidity paper should not reshape the AMM kernel, but it does add an
important control-plane requirement: liquidity is not only a curve, it is a
lifecycle. Incentive windows can attract liquidity, concentrate it, and then
produce depth cliffs when rewards decay.

Design implication:

- The simulator should optimize LP outcome, not TVL. The objective should
  include fees, incentives, adverse selection, update costs, priority fees,
  opportunity cost, and IL/LVR.
- Campaign metadata should be modeled explicitly: start/end, decay schedule,
  reward formula, target depth, retention goals, and post-window risk.
- Pooled LP mode needs safeguards against correlated exits: withdrawal queues,
  throttles, minimum depth floors, and emergency pause semantics.
- Aggregator analytics must distinguish Meta-AMM pool quality from composite
  route quality. Jupiter route plans should be stored and classified by venue
  leg when evaluating execution.
- Reference-priced assets remain the cleanest proving ground because fair value,
  stale quote loss, markout, and LVR can be measured more directly.

## DLMM Lessons

Meteora DLMM organizes liquidity into discrete bins. Each bin has a fixed price;
within a bin the exchange is constant-sum, so trades inside a bin have no price
movement until that bin is depleted. Only one active bin contains both tokens;
bins on either side hold one token. The active bin moves as liquidity is
consumed.

The code path reinforces several implementation details:

- bin price derives from `active_id` and `bin_step`
- bin arrays chunk state, avoiding one giant account
- dynamic fees are driven by a volatility accumulator
- quote math uses fixed-point arithmetic and checked overflow paths
- rebalance helpers expose Spot, Curve, and BidAsk distributions

Design implication:

- A binned range primitive is the best native-discovery primitive for major
  long-tail pairs and volatile assets.
- Dynamic fee state should be first-class, but bounded and inspectable.
- Position/range updates need a dedicated rebalance planner; they should not be
  improvised by agents at transaction time.

## DBC Lessons

Meteora Dynamic Bonding Curve is launch-focused, but the curve configuration is
valuable. The program stores a bounded array of `(sqrt_price, liquidity)` points,
validates monotonic prices and positive liquidity, computes migration thresholds,
and checks decimal assumptions.

Important concrete constraints from the code:

- `MAX_CURVE_POINT` is 16, with config storage for 20 points
- sqrt prices must stay inside min/max bounds
- curve points must be strictly ascending
- liquidity must be positive
- token decimals are validated in the 6-9 range for that program

Design implication:

- Meta-AMM should adopt bounded piecewise curves, not arbitrary code.
- Decimal handling must be a protocol-level invariant, not a UI convention.
- Curves should compile from agent-friendly intent into a compact on-chain
  representation with monotonicity and bounded-size checks.

## Aggregator Lessons

Jupiter Metis DEX integration requires an SDK implementing the AMM interface.
The implementation must support account discovery, cached account updates,
quotes with no network calls, and swap account metas. Jupiter also checks code
health, security audit state, traction, team quality, and integration value.

JupiterZ RFQ has a different shape: makers host a quote/swap/tokens webhook,
target a 250ms response window, and are expected to fulfill 95% of quotes in a
rolling hour. RFQ reduces some on-chain compute pressure, but it is not a
replacement for an on-chain AMM if the goal is composable liquidity.

Design implication:

- The project needs two integration surfaces:
  - AMM interface adapter for on-chain routed liquidity.
  - RFQ webhook adapter for selected high-touch market maker flows.
- Quote logic must be fully deterministic from cached accounts. No off-chain
  network calls may be required inside the Jupiter AMM quote implementation.
- Transaction size and account count are product constraints, not cleanup work.

## Oracle Lessons

Pyth's pull oracle model is useful for reference-priced assets because updates
can be pulled when needed and Pyth publishes high-frequency data. Pyth's Solana
receiver `PriceUpdateV2` exposes price, confidence, exponent, publish time, and
feed validation helpers. Switchboard covers custom feeds and randomness, and its
docs show managed quote accounts with explicit staleness checks.

Design implication:

- The first oracle policy should check feed id, owner/program id, max age,
  confidence bps, positive price, and proof metadata.
- External reference mode should support Pyth first, Switchboard/custom feeds
  second, and maker-signed low-CU updates third.
- Oracle and quote freshness must be enforced on-chain.

## Market Microstructure Lessons

Traditional market making separates:

- fair value estimation
- spread setting
- inventory risk management
- adverse-selection protection
- quote lifetime and cancellation policy

Avellaneda-Stoikov style inventory-aware quoting suggests that the reserve
price should move against excess inventory. Glosten-Milgrom motivates spread
widening under adverse selection. LVR literature turns stale AMM prices into an
explicit adverse-selection cost: LPs lose when better-informed traders pick off
slow prices.

Design implication:

- Inventory skew is not optional in reference-priced mode.
- Spread must be a function of volatility, inventory imbalance, and flow
  quality, not just a static bps number.
- Simulation must report LVR-like stale-price loss separately from inventory
  drift and fees earned.

## Solana Runtime Lessons

The design must respect:

- 1232-byte transaction size limit
- 1.4M CU max per transaction
- 200k default non-builtin instruction budget
- shallow CPI depth
- priority fees priced from requested CU limit, so over-requesting wastes money

Design implication:

- Use compact PDAs and account chunking.
- Keep adapter stacks shallow.
- Split config creation, funding, quoting, and swaps into separate flows.
- Simulate transactions and set measured CU limits with a buffer.
- Do not put unbounded config arrays or generated strategy code on-chain.

## Local Repo Pattern Review

The useful reusable patterns from the local instruments repo are architectural,
not branding:

- keep on-chain state limited to custody, settlement, lifecycle, and claims
- use versioned off-chain specs to carry rich product configuration
- gate external adapters by explicit read/quote/simulate/write/custody
  capabilities
- require strict oracle policy objects for exogenous settlement
- write invariants before expanding the state machine
- keep paper/simulation mode separate from live custody

Design implication:

- Meta-AMM should have a versioned `MakerIntent` and `StrategyConfig` schema.
- No adapter should get write or custody permission until there is vault
  accounting, simulation coverage, and invariant coverage.
- The first implementation should be standalone, but should reuse the same
  discipline: typed specs, small kernels, and hard promotion gates.

## Research Conclusion

The best architecture is not "one universal AMM." It is a small, audited maker
kernel with multiple bounded curve primitives and an off-chain compiler that
turns maker intent into verified configs.

The selected direction is:

- reference-quote primitive for external-price assets
- binned range primitive for native-discovery markets
- bounded piecewise curve primitive for launches and shaped curves
- constant-product primitive as the safe fallback
- shared oracle, inventory, fee, flow, simulation, and verification framework

This gives agents configurability without letting agents deploy arbitrary
financial code into the hot swap path.

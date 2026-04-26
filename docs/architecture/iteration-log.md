# Architecture Iteration Log

This log records the architecture iterations considered before selecting the
current design.

## Iteration 1: Universal On-Chain Curve Interpreter

Idea:

- Store a generic curve DSL on-chain.
- Let makers or agents upload arbitrary curve expressions.
- Execute the DSL during every swap.

Why it was tempting:

- Maximum configurability.
- A clean "agent writes strategy, chain executes strategy" story.
- Could theoretically cover prop quoting, DLMM, bonding curves, and constant
  product in one engine.

Rejected because:

- Hard to bound compute for every swap.
- Hard to audit and formally specify.
- Hard to integrate with Jupiter because quote logic must be deterministic,
  fast, and account-cache based.
- High risk of agent-generated configs producing pathological price surfaces.
- Solana transaction/account limits make rich dynamic config expensive.

Decision:

- Do not build an arbitrary on-chain interpreter.
- Use bounded primitives plus an off-chain compiler.

## Iteration 2: Fork DLMM and Add Agent Presets

Idea:

- Start from the DLMM model.
- Add better agent-facing range and rebalance presets.
- Use bins as the universal liquidity representation.

Why it was tempting:

- DLMM is battle-shaped for concentrated liquidity.
- Bin math is understandable and capital efficient.
- It naturally supports native-discovery and volatile long-tail assets.

Rejected as the full architecture because:

- Reference-priced prop making wants cheap mid-price/spread updates, not only
  bin shifting.
- Inventory curves and toxic-flow policies are not native in a vanilla binned
  AMM.
- Full constant-product and launch-style curves become awkward if everything is
  forced into bins.

Decision:

- Keep a DLMM-like binned range primitive, but do not make it the only mode.

## Iteration 3: Hadron-Style PropAMM Clone

Idea:

- Treat every market as a prop-style maker-owned pool.
- Use mid-price, spread, bid/ask curves, inventory curves, and flow policies.

Why it was tempting:

- Best fit for externally priced assets like BTC, SOL, gold, equities, FX, and
  tokenized stocks.
- Strong match for agent allocators and professional market makers.
- Public docs make the right abstraction clear.

Rejected as the full architecture because:

- Does not fully cover native price discovery where liquidity itself should
  help establish price.
- Maker-owned isolated pools are good for safety but not enough for eventual
  shared capital products.
- It can drift toward opaque private market making if there is no simulation,
  disclosure, or invariant framework.

Decision:

- Include a reference-quote primitive with mid-price, spread, bid/ask curves,
  inventory curves, and quote TTLs.
- Make transparency, simulation, and config inspection first-class.

## Iteration 4: Pure RFQ / Intent System

Idea:

- Skip on-chain AMM complexity.
- Let makers run off-chain quote servers.
- Use JupiterZ/RFQ-style fulfillment or custom intents.

Why it was tempting:

- Flexible.
- Fast to iterate.
- Easy for sophisticated makers to protect inventory.

Rejected because:

- It is not an AMM maker layer.
- Liquidity becomes less composable and more dependent on webhook uptime.
- Fill-rate, response-time, and maker-signing requirements become the core
  product risk.
- Does not give passive/agent-configured capital a deterministic on-chain
  surface.

Decision:

- Support RFQ as an integration surface, not as the core protocol.

## Iteration 5: Selected Hybrid Kernel

Selected architecture:

- Off-chain intent compiler and simulator.
- On-chain maker kernel with finite, audited primitives.
- Reference-quote mode for external prices.
- Binned range mode for native discovery.
- Piecewise curve mode for launch/custom shapes.
- Constant-product fallback mode.
- Shared decimal, oracle, fee, inventory, flow, and circuit-breaker policies.
- Maker-owned and pooled LP custody as first-class architecture, with
  maker-owned implemented first.
- Aggregator compatibility as an account-layout constraint from the beginning.

Why this is the best current architecture:

- It spans the prop-MM to AMM spectrum without a dangerous generic interpreter.
- It gives agents a clean typed surface while keeping swap execution compact.
- It maps cleanly to Jupiter AMM integration requirements.
- It can support other DEX aggregators through the same cached-account adapter
  boundary.
- It gives formal verification a finite state surface.
- It allows Pinocchio/no_std optimization only where measurement proves it is
  needed.

Open risks:

- Reference-quote mode may still be picked off under bad landing or stale-price
  conditions.
- Flow-policy controls can become centralizing if not disclosed clearly.
- A multi-mode AMM is broader than an MVP; strict phase gating is required.
- Highly expressive configs can still be dangerous unless schemas, simulator
  limits, and on-chain caps reject bad surfaces early.
- Pooled LP custody adds a second major invariant family and should be
  architected early but implemented after maker-owned custody is proven.
- Simulator quality will determine whether agent-facing recommendations are
  trustworthy.

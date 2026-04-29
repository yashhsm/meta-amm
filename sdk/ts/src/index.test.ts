import assert from "node:assert/strict";
import {
  buildReferenceQuoteAggregatorManifest,
  Q64,
  quoteAgeState,
  quoteReferenceExactIn,
  type ReferenceQuoteCachedState,
  type ReferenceQuoteParams,
} from "./index.js";

function params(): ReferenceQuoteParams {
  return {
    feeBps: 30,
    baseHalfSpreadBps: 10,
    agingStartSlots: 5n,
    protectedStartSlots: 10n,
    expireSlots: 15n,
    agingSurchargeBpsPerSlot: 2,
    maxAgingSurchargeBps: 20,
    maxTradeBaseAtoms: 10_000n,
    protectedMaxTradeBaseAtoms: 500n,
    inventorySkewBpsPer10KImbalance: 1_000,
    maxInventorySkewBps: 500,
    hardInventoryBandBps: 3_000,
  };
}

function state(nowSlot = 10n): ReferenceQuoteCachedState {
  return {
    baseInventory: 1_000_000n,
    quoteInventory: 30_000_000_000n,
    targetBaseInventory: 1_000_000n,
    midPriceQ64x64: 30_000n * Q64,
    midPublishSlot: 10n,
    nowSlot,
    paused: false,
  };
}

{
  const quote = quoteReferenceExactIn(state(), params(), 1_000n, true);

  assert.equal(quote.ageState, "Fresh");
  assert.equal(quote.amountInLessFee, 997n);
  assert.equal(quote.amountOut, 29_880_090n);
  assert.equal(quote.newBaseInventory, 1_001_000n);
  assert.equal(quote.newQuoteInventory, 29_970_119_910n);
  assert.equal(quote.appliedSpreadBps, 10);
}

{
  const fresh = quoteReferenceExactIn(state(10n), params(), 1_000n, true);
  const aging = quoteReferenceExactIn(state(17n), params(), 1_000n, true);

  assert.equal(quoteAgeState(state(17n), params()), "Aging");
  assert.equal(aging.appliedSpreadBps, 14);
  assert(aging.amountOut < fresh.amountOut);
}

{
  assert.throws(
    () => quoteReferenceExactIn(state(20n), params(), 2_000n, true),
    /QuoteProtected/,
  );
  assert.equal(quoteReferenceExactIn(state(20n), params(), 100n, true).ageState, "Protected");
}

{
  const dynamicParams = params();
  dynamicParams.baseHalfSpreadBps = 25;
  const quote = quoteReferenceExactIn(state(), dynamicParams, 1_000n, true);

  assert.equal(quote.appliedSpreadBps, 25);
  assert(quote.amountOut < 29_880_090n);
}

{
  const manifest = buildReferenceQuoteAggregatorManifest();

  assert.equal(manifest.modeId, 1);
  assert.equal(manifest.supportsExactIn, true);
  assert.equal(manifest.supportsExactOut, false);
  assert.equal(manifest.requiredSwapAccountMetas, 13);
  assert.equal(manifest.accountLabels[0], "taker");
  assert.equal(manifest.accountLabels[12], "quote_vault");
  assert.equal(manifest.supportsTransferFeeMints, false);
}

console.log("sdk quote tests passed");

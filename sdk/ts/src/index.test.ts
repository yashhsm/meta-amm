import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  buildReferenceQuoteAggregatorManifest,
  Q64,
  quoteAgeState,
  quoteReferenceExactIn,
  type ReferenceQuoteCachedState,
  type ReferenceQuoteParams,
} from "./index.js";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const PARITY_FIXTURE = path.join(ROOT, "tests/golden/reference-quote-parity.csv");

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

for (const row of parseParityRows(fs.readFileSync(PARITY_FIXTURE, "utf8"))) {
  const quote = quoteReferenceExactIn(
    {
      baseInventory: row.baseInventory,
      quoteInventory: row.quoteInventory,
      targetBaseInventory: row.targetBaseInventory,
      midPriceQ64x64: row.midPriceInt * Q64,
      midPublishSlot: row.midPublishSlot,
      nowSlot: row.nowSlot,
      paused: row.paused,
    },
    {
      feeBps: row.feeBps,
      baseHalfSpreadBps: row.baseHalfSpreadBps,
      agingStartSlots: row.agingStartSlots,
      protectedStartSlots: row.protectedStartSlots,
      expireSlots: row.expireSlots,
      agingSurchargeBpsPerSlot: row.agingSurchargeBpsPerSlot,
      maxAgingSurchargeBps: row.maxAgingSurchargeBps,
      maxTradeBaseAtoms: row.maxTradeBaseAtoms,
      protectedMaxTradeBaseAtoms: row.protectedMaxTradeBaseAtoms,
      inventorySkewBpsPer10KImbalance: row.inventorySkewBpsPer10KImbalance,
      maxInventorySkewBps: row.maxInventorySkewBps,
      hardInventoryBandBps: row.hardInventoryBandBps,
    },
    row.amountIn,
    row.baseToQuote,
  );

  assert.equal(quote.ageState, row.ageState, row.name);
  assert.equal(quote.amountInLessFee, row.amountInLessFee, row.name);
  assert.equal(quote.amountOut, row.amountOut, row.name);
  assert.equal(quote.newBaseInventory, row.newBaseInventory, row.name);
  assert.equal(quote.newQuoteInventory, row.newQuoteInventory, row.name);
  assert.equal(quote.effectivePriceQ64x64, row.effectivePriceQ64x64, row.name);
  assert.equal(quote.appliedSpreadBps, row.appliedSpreadBps, row.name);
  assert.equal(quote.inventoryImbalanceBps, row.inventoryImbalanceBps, row.name);
}

console.log("sdk quote tests passed");

type ParityRow = {
  name: string;
  baseInventory: bigint;
  quoteInventory: bigint;
  targetBaseInventory: bigint;
  midPriceInt: bigint;
  midPublishSlot: bigint;
  nowSlot: bigint;
  paused: boolean;
  feeBps: number;
  baseHalfSpreadBps: number;
  agingStartSlots: bigint;
  protectedStartSlots: bigint;
  expireSlots: bigint;
  agingSurchargeBpsPerSlot: number;
  maxAgingSurchargeBps: number;
  maxTradeBaseAtoms: bigint;
  protectedMaxTradeBaseAtoms: bigint;
  inventorySkewBpsPer10KImbalance: number;
  maxInventorySkewBps: number;
  hardInventoryBandBps: number;
  amountIn: bigint;
  baseToQuote: boolean;
  ageState: string;
  amountInLessFee: bigint;
  amountOut: bigint;
  newBaseInventory: bigint;
  newQuoteInventory: bigint;
  effectivePriceQ64x64: bigint;
  appliedSpreadBps: number;
  inventoryImbalanceBps: number;
};

function parseParityRows(input: string): ParityRow[] {
  return input
    .trim()
    .split("\n")
    .slice(1)
    .map((line) => {
      const fields = line.split(",");
      assert.equal(fields.length, 30, "unexpected parity fixture width");
      return {
        name: fields[0],
        baseInventory: BigInt(fields[1]),
        quoteInventory: BigInt(fields[2]),
        targetBaseInventory: BigInt(fields[3]),
        midPriceInt: BigInt(fields[4]),
        midPublishSlot: BigInt(fields[5]),
        nowSlot: BigInt(fields[6]),
        paused: parseBool(fields[7]),
        feeBps: Number(fields[8]),
        baseHalfSpreadBps: Number(fields[9]),
        agingStartSlots: BigInt(fields[10]),
        protectedStartSlots: BigInt(fields[11]),
        expireSlots: BigInt(fields[12]),
        agingSurchargeBpsPerSlot: Number(fields[13]),
        maxAgingSurchargeBps: Number(fields[14]),
        maxTradeBaseAtoms: BigInt(fields[15]),
        protectedMaxTradeBaseAtoms: BigInt(fields[16]),
        inventorySkewBpsPer10KImbalance: Number(fields[17]),
        maxInventorySkewBps: Number(fields[18]),
        hardInventoryBandBps: Number(fields[19]),
        amountIn: BigInt(fields[20]),
        baseToQuote: parseBool(fields[21]),
        ageState: fields[22],
        amountInLessFee: BigInt(fields[23]),
        amountOut: BigInt(fields[24]),
        newBaseInventory: BigInt(fields[25]),
        newQuoteInventory: BigInt(fields[26]),
        effectivePriceQ64x64: BigInt(fields[27]),
        appliedSpreadBps: Number(fields[28]),
        inventoryImbalanceBps: Number(fields[29]),
      };
    });
}

function parseBool(input: string): boolean {
  if (input === "true") {
    return true;
  }
  if (input === "false") {
    return false;
  }
  throw new Error(`invalid bool field: ${input}`);
}

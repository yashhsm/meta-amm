export const Q64 = 1n << 64n;
export const BASIS_POINTS = 10_000;
export const REFERENCE_QUOTE_REQUIRED_SWAP_ACCOUNT_METAS = 13;
export const REFERENCE_QUOTE_DEFAULT_SWAP_ACCOUNT_META_BUDGET = 15;

export const REFERENCE_QUOTE_SWAP_ACCOUNT_LABELS = [
  "taker",
  "pool_config",
  "quote_state",
  "vault_authority",
  "vault_state",
  "base_mint",
  "quote_mint",
  "base_token_program",
  "quote_token_program",
  "user_base_account",
  "user_quote_account",
  "base_vault",
  "quote_vault",
] as const;

export type QuoteAgeState =
  | "Fresh"
  | "Aging"
  | "Protected"
  | "Expired"
  | "Paused";

export type ReferenceQuoteParams = {
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
};

export type ReferenceQuoteCachedState = {
  baseInventory: bigint;
  quoteInventory: bigint;
  targetBaseInventory: bigint;
  midPriceQ64x64: bigint;
  midPublishSlot: bigint;
  nowSlot: bigint;
  paused: boolean;
};

export type ReferenceQuote = {
  ageState: QuoteAgeState;
  amountInLessFee: bigint;
  amountOut: bigint;
  newBaseInventory: bigint;
  newQuoteInventory: bigint;
  effectivePriceQ64x64: bigint;
  appliedSpreadBps: number;
  inventoryImbalanceBps: number;
};

export type ReferenceQuoteAggregatorManifest = {
  modeId: 1;
  supportsExactIn: true;
  supportsExactOut: false;
  requiredSwapAccountMetas: number;
  maxSwapAccountMetas: number;
  accountLabels: typeof REFERENCE_QUOTE_SWAP_ACCOUNT_LABELS;
  supportsSplToken: true;
  supportsToken2022: true;
  supportsTransferFeeMints: false;
};

export function buildReferenceQuoteAggregatorManifest(
  maxSwapAccountMetas = REFERENCE_QUOTE_DEFAULT_SWAP_ACCOUNT_META_BUDGET,
): ReferenceQuoteAggregatorManifest {
  return {
    modeId: 1,
    supportsExactIn: true,
    supportsExactOut: false,
    requiredSwapAccountMetas: REFERENCE_QUOTE_REQUIRED_SWAP_ACCOUNT_METAS,
    maxSwapAccountMetas,
    accountLabels: REFERENCE_QUOTE_SWAP_ACCOUNT_LABELS,
    supportsSplToken: true,
    supportsToken2022: true,
    supportsTransferFeeMints: false,
  };
}

export function quoteReferenceExactIn(
  state: ReferenceQuoteCachedState,
  params: ReferenceQuoteParams,
  amountIn: bigint,
  baseToQuote: boolean,
): ReferenceQuote {
  validateParams(params);
  if (amountIn <= 0n) {
    throw new Error("InvalidAmount");
  }
  if (state.baseInventory === 0n || state.quoteInventory === 0n) {
    throw new Error("EmptyLiquidity");
  }
  if (state.targetBaseInventory === 0n || state.midPriceQ64x64 === 0n) {
    throw new Error("InvalidConfig");
  }

  const ageState = quoteAgeState(state, params);
  if (ageState === "Paused") {
    throw new Error("PoolPaused");
  }
  if (ageState === "Expired") {
    throw new Error("StaleQuote");
  }

  const baseEquivalent = baseToQuote
    ? amountIn
    : divAmountFloor(state.midPriceQ64x64, amountIn);
  const maxBaseSize = maxBaseTradeForAge(state, params, ageState);
  if (baseEquivalent === 0n || baseEquivalent > maxBaseSize) {
    throw new Error(ageState === "Protected" ? "QuoteProtected" : "InvalidAmount");
  }

  const amountInLessFee = amountLessFee(amountIn, params.feeBps);
  const currentImbalanceBps = inventoryImbalanceBps(
    state.baseInventory,
    state.targetBaseInventory,
  );
  const skewBps = inventorySkewBps(currentImbalanceBps, params);
  const appliedSpreadBps =
    params.baseHalfSpreadBps + agingSurchargeBps(state, params);
  const effectivePriceQ64x64 = effectivePrice(
    state.midPriceQ64x64,
    appliedSpreadBps,
    skewBps,
    baseToQuote,
  );

  let amountOut: bigint;
  let newBaseInventory: bigint;
  let newQuoteInventory: bigint;
  if (baseToQuote) {
    amountOut = mulAmountFloor(effectivePriceQ64x64, amountInLessFee);
    if (amountOut === 0n || amountOut >= state.quoteInventory) {
      throw new Error("InvalidAmount");
    }
    newBaseInventory = state.baseInventory + amountIn;
    newQuoteInventory = state.quoteInventory - amountOut;
  } else {
    amountOut = divAmountFloor(effectivePriceQ64x64, amountInLessFee);
    if (amountOut === 0n || amountOut >= state.baseInventory) {
      throw new Error("InvalidAmount");
    }
    newBaseInventory = state.baseInventory - amountOut;
    newQuoteInventory = state.quoteInventory + amountIn;
  }

  const newImbalanceBps = inventoryImbalanceBps(
    newBaseInventory,
    state.targetBaseInventory,
  );
  if (Math.abs(newImbalanceBps) > params.hardInventoryBandBps) {
    throw new Error("InventoryBand");
  }

  return {
    ageState,
    amountInLessFee,
    amountOut,
    newBaseInventory,
    newQuoteInventory,
    effectivePriceQ64x64,
    appliedSpreadBps,
    inventoryImbalanceBps: currentImbalanceBps,
  };
}

export function quoteAgeState(
  state: ReferenceQuoteCachedState,
  params: ReferenceQuoteParams,
): QuoteAgeState {
  validateParams(params);
  if (state.paused) {
    return "Paused";
  }

  const age = state.nowSlot > state.midPublishSlot
    ? state.nowSlot - state.midPublishSlot
    : 0n;
  if (age >= params.expireSlots) {
    return "Expired";
  }
  if (age >= params.protectedStartSlots) {
    return "Protected";
  }
  if (age >= params.agingStartSlots) {
    return "Aging";
  }
  return "Fresh";
}

function validateParams(params: ReferenceQuoteParams) {
  for (const value of [
    params.feeBps,
    params.baseHalfSpreadBps,
    params.maxAgingSurchargeBps,
    params.inventorySkewBpsPer10KImbalance,
    params.maxInventorySkewBps,
  ]) {
    if (!Number.isInteger(value) || value < 0 || value >= BASIS_POINTS) {
      throw new Error("InvalidConfig");
    }
  }

  if (
    params.agingStartSlots > params.protectedStartSlots ||
    params.protectedStartSlots >= params.expireSlots ||
    params.expireSlots === 0n ||
    params.maxTradeBaseAtoms === 0n ||
    params.protectedMaxTradeBaseAtoms === 0n ||
    params.protectedMaxTradeBaseAtoms > params.maxTradeBaseAtoms
  ) {
    throw new Error("InvalidConfig");
  }
}

function amountLessFee(amount: bigint, feeBps: number): bigint {
  if (feeBps >= BASIS_POINTS) {
    throw new Error("InvalidFee");
  }
  const result = (amount * BigInt(BASIS_POINTS - feeBps)) / BigInt(BASIS_POINTS);
  if (result === 0n) {
    throw new Error("InvalidAmount");
  }
  return result;
}

function maxBaseTradeForAge(
  state: ReferenceQuoteCachedState,
  params: ReferenceQuoteParams,
  ageState: QuoteAgeState,
): bigint {
  switch (ageState) {
    case "Fresh":
      return params.maxTradeBaseAtoms;
    case "Protected":
      return params.protectedMaxTradeBaseAtoms;
    case "Aging": {
      const age = state.nowSlot > state.midPublishSlot
        ? state.nowSlot - state.midPublishSlot
        : 0n;
      const agingSpan =
        params.protectedStartSlots - params.agingStartSlots > 0n
          ? params.protectedStartSlots - params.agingStartSlots
          : 1n;
      const ageIntoAging = minBigint(
        age > params.agingStartSlots ? age - params.agingStartSlots : 0n,
        agingSpan,
      );
      const decay = params.maxTradeBaseAtoms - params.protectedMaxTradeBaseAtoms;
      return params.maxTradeBaseAtoms - ((decay * ageIntoAging) / agingSpan);
    }
    case "Expired":
    case "Paused":
      return 0n;
  }
}

function agingSurchargeBps(
  state: ReferenceQuoteCachedState,
  params: ReferenceQuoteParams,
): number {
  const age = state.nowSlot > state.midPublishSlot
    ? state.nowSlot - state.midPublishSlot
    : 0n;
  const slots = age > params.agingStartSlots ? age - params.agingStartSlots : 0n;
  const surcharge = slots * BigInt(params.agingSurchargeBpsPerSlot);
  return Number(
    minBigint(surcharge, BigInt(params.maxAgingSurchargeBps)),
  );
}

function inventoryImbalanceBps(
  baseInventory: bigint,
  targetBaseInventory: bigint,
): number {
  if (targetBaseInventory === 0n) {
    return 0;
  }
  const delta = baseInventory - targetBaseInventory;
  const bps = (delta * 10_000n) / targetBaseInventory;
  const min = BigInt(-2_147_483_648);
  const max = BigInt(2_147_483_647);
  if (bps < min) {
    return -2_147_483_648;
  }
  if (bps > max) {
    return 2_147_483_647;
  }
  return Number(bps);
}

function inventorySkewBps(
  imbalanceBps: number,
  params: ReferenceQuoteParams,
): number {
  const raw = Math.trunc(
    (imbalanceBps * params.inventorySkewBpsPer10KImbalance) / BASIS_POINTS,
  );
  const cap = params.maxInventorySkewBps;
  return Math.min(Math.max(raw, -cap), cap);
}

function effectivePrice(
  midPriceQ64x64: bigint,
  spreadBps: number,
  skewBps: number,
  baseToQuote: boolean,
): bigint {
  const factorBps = baseToQuote
    ? BASIS_POINTS - spreadBps - skewBps
    : BASIS_POINTS + spreadBps - skewBps;
  if (factorBps <= 0) {
    throw new Error("InvalidConfig");
  }
  return (midPriceQ64x64 * BigInt(factorBps)) / BigInt(BASIS_POINTS);
}

function mulAmountFloor(priceQ64x64: bigint, amount: bigint): bigint {
  return (amount * priceQ64x64) / Q64;
}

function divAmountFloor(priceQ64x64: bigint, amount: bigint): bigint {
  if (priceQ64x64 === 0n) {
    throw new Error("DivByZero");
  }
  return (amount * Q64) / priceQ64x64;
}

function minBigint(left: bigint, right: bigint): bigint {
  return left < right ? left : right;
}

import anchor from "@coral-xyz/anchor";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountIdempotentInstruction,
  createInitializeMintInstruction,
  createInitializeTransferFeeConfigInstruction,
  createMint,
  createSyncNativeInstruction,
  ExtensionType,
  getAccount,
  getAssociatedTokenAddressSync,
  getMint,
  getMintLen,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  NATIVE_MINT,
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  sendAndConfirmTransaction,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const IDL_PATH = path.join(ROOT, "target/idl/meta_amm.json");
const DEFAULT_WALLET = path.join(os.homedir(), ".config/solana/id.json");
const RPC_URL = process.env.ANCHOR_PROVIDER_URL ?? "http://127.0.0.1:8899";
const WALLET_PATH = process.env.ANCHOR_WALLET ?? DEFAULT_WALLET;
const SURFPOOL_MODE = process.env.META_AMM_SURFPOOL_MODE ?? "smoke";
const FUZZ_CASES = Number(process.env.META_AMM_FUZZ_CASES ?? "32");
const CU_REPORT_PATH = process.env.META_AMM_CU_REPORT_PATH
  ?? path.join(ROOT, "target", "surfpool-compute-budget.json");
const MAINNET_USDC = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const Q64_ONE = new anchor.BN("18446744073709551616");

type TokenProgramId = typeof TOKEN_PROGRAM_ID;
type SurfpoolMode = "smoke" | "fuzz" | "cu" | "all";

type SwapBalances = {
  userBase: bigint;
  userQuote: bigint;
  vaultBase: bigint;
  vaultQuote: bigint;
};

type ComputeRow = {
  instruction: string;
  consumedUnits: number;
  signature: string;
};

type PoolContext = {
  authority: Keypair;
  quoteAuthority: Keypair;
  baseMint: PublicKey;
  quoteMint: PublicKey;
  baseTokenProgram: TokenProgramId;
  quoteTokenProgram: TokenProgramId;
  poolConfig: PublicKey;
  quoteState: PublicKey;
  curveSlotState: PublicKey;
  vaultAuthority: PublicKey;
  vaultState: PublicKey;
  baseVault: PublicKey;
  quoteVault: PublicKey;
};

function loadKeypair(filePath: string): Keypair {
  const secret = JSON.parse(fs.readFileSync(filePath, "utf8"));
  return Keypair.fromSecretKey(Uint8Array.from(secret));
}

function seed(value: string): Buffer {
  return Buffer.from(value, "utf8");
}

function pda(programId: PublicKey, seeds: (Buffer | Uint8Array)[]): PublicKey {
  return PublicKey.findProgramAddressSync(seeds, programId)[0];
}

function baseArgs() {
  return {
    params: {
      feeBps: 30,
      baseHalfSpreadBps: 10,
      agingStartSlots: new anchor.BN(5),
      protectedStartSlots: new anchor.BN(10),
      expireSlots: new anchor.BN(15),
      agingSurchargeBpsPerSlot: 2,
      maxAgingSurchargeBps: 20,
      maxTradeBaseAtoms: new anchor.BN(10_000),
      protectedMaxTradeBaseAtoms: new anchor.BN(500),
      inventorySkewBpsPer10KImbalance: 1_000,
      maxInventorySkewBps: 500,
      hardInventoryBandBps: 3_000,
    },
    quoteUpdateEnvelope: {
      maxMakerUpdatePeriodSlots: new anchor.BN(4),
      maxLandingLatencySlots: new anchor.BN(3),
      minUpdateSuccessProbabilityBps: 7_400,
      sameSlotOrder: 1,
    },
    accountBudget: {
      requiredSwapAccountMetas: 13,
      maxSwapAccountMetas: 15,
    },
  };
}

function summarizeError(error: unknown): string {
  if (error instanceof Error) {
    return `${error.name}: ${error.message}`;
  }
  return String(error);
}

function selectedMode(): SurfpoolMode {
  if (
    SURFPOOL_MODE === "smoke"
    || SURFPOOL_MODE === "fuzz"
    || SURFPOOL_MODE === "cu"
    || SURFPOOL_MODE === "all"
  ) {
    return SURFPOOL_MODE;
  }
  throw new Error(`invalid META_AMM_SURFPOOL_MODE: ${SURFPOOL_MODE}`);
}

async function expectReject(
  label: string,
  action: () => Promise<unknown>,
  pattern: RegExp,
) {
  try {
    await action();
  } catch (error) {
    const message = summarizeError(error);
    assert.match(message, pattern, `${label}: ${message}`);
    console.log(`ok: ${label}`);
    return;
  }
  assert.fail(`${label}: expected rejection`);
}

async function ensureLamports(
  connection: anchor.web3.Connection,
  pubkey: PublicKey,
  minimumLamports: number,
) {
  const balance = await connection.getBalance(pubkey, "confirmed");
  if (balance >= minimumLamports) {
    return;
  }
  const signature = await connection.requestAirdrop(
    pubkey,
    minimumLamports - balance,
  );
  const blockhash = await connection.getLatestBlockhash("confirmed");
  await connection.confirmTransaction(
    { signature, ...blockhash },
    "confirmed",
  );
}

async function fundedAuthority(
  connection: anchor.web3.Connection,
): Promise<Keypair> {
  const authority = Keypair.generate();
  await ensureLamports(connection, authority.publicKey, 3 * LAMPORTS_PER_SOL);
  return authority;
}

function derivePoolContext(
  programId: PublicKey,
  authority: Keypair,
  quoteAuthority: Keypair,
  baseMint: PublicKey,
  quoteMint: PublicKey,
  baseTokenProgram: TokenProgramId,
  quoteTokenProgram: TokenProgramId,
): PoolContext {
  const poolConfig = pda(programId, [
    seed("pool-config"),
    authority.publicKey.toBuffer(),
    baseMint.toBuffer(),
    quoteMint.toBuffer(),
  ]);
  return {
    authority,
    quoteAuthority,
    baseMint,
    quoteMint,
    baseTokenProgram,
    quoteTokenProgram,
    poolConfig,
    quoteState: pda(programId, [
      seed("quote-state"),
      poolConfig.toBuffer(),
    ]),
    curveSlotState: pda(programId, [
      seed("curve-slot-state"),
      poolConfig.toBuffer(),
    ]),
    vaultAuthority: pda(programId, [
      seed("vault-authority"),
      poolConfig.toBuffer(),
    ]),
    vaultState: pda(programId, [
      seed("vault-state"),
      poolConfig.toBuffer(),
    ]),
    baseVault: pda(programId, [seed("base-vault"), poolConfig.toBuffer()]),
    quoteVault: pda(programId, [seed("quote-vault"), poolConfig.toBuffer()]),
  };
}

async function initializePool(
  program: anchor.Program,
  payer: Keypair,
  context: PoolContext,
) {
  await program.methods
    .initializeReferenceQuotePool(baseArgs())
    .accounts({
      payer: payer.publicKey,
      authority: context.authority.publicKey,
      baseMint: context.baseMint,
      quoteMint: context.quoteMint,
      baseTokenProgram: context.baseTokenProgram,
      quoteTokenProgram: context.quoteTokenProgram,
      poolConfig: context.poolConfig,
      systemProgram: SystemProgram.programId,
    })
    .signers([context.authority])
    .rpc();

  const account = await (program.account as any).referenceQuotePoolConfig.fetch(
    context.poolConfig,
  );
  assert.equal(account.authority.toBase58(), context.authority.publicKey.toBase58());
  assert.equal(account.baseMint.toBase58(), context.baseMint.toBase58());
  assert.equal(account.quoteMint.toBase58(), context.quoteMint.toBase58());
  assert.equal(
    account.baseTokenProgram.toBase58(),
    context.baseTokenProgram.toBase58(),
  );
  assert.equal(
    account.quoteTokenProgram.toBase58(),
    context.quoteTokenProgram.toBase58(),
  );
}

async function initializeQuoteState(
  program: anchor.Program,
  payer: Keypair,
  context: PoolContext,
) {
  await program.methods
    .initializeReferenceQuoteState({
      quoteAuthority: context.quoteAuthority.publicKey,
    })
    .accounts({
      payer: payer.publicKey,
      authority: context.authority.publicKey,
      poolConfig: context.poolConfig,
      quoteState: context.quoteState,
      systemProgram: SystemProgram.programId,
    })
    .signers([context.authority])
    .rpc();
}

async function initializeVaults(
  program: anchor.Program,
  payer: Keypair,
  context: PoolContext,
) {
  await program.methods
    .initializeMakerVaults()
    .accounts({
      payer: payer.publicKey,
      authority: context.authority.publicKey,
      poolConfig: context.poolConfig,
      vaultAuthority: context.vaultAuthority,
      baseMint: context.baseMint,
      quoteMint: context.quoteMint,
      baseTokenProgram: context.baseTokenProgram,
      quoteTokenProgram: context.quoteTokenProgram,
      vaultState: context.vaultState,
      baseVault: context.baseVault,
      quoteVault: context.quoteVault,
      systemProgram: SystemProgram.programId,
    })
    .signers([context.authority])
    .rpc();
}

async function updateQuote(
  program: anchor.Program,
  context: PoolContext,
  signer: Keypair,
  sequence: number,
  publishSlot: number,
) {
  await program.methods
    .updateReferenceQuote({
      midPriceQ64X64: Q64_ONE,
      publishSlot: new anchor.BN(publishSlot),
      sequence: new anchor.BN(sequence),
    })
    .accounts({
      quoteSigner: signer.publicKey,
      poolConfig: context.poolConfig,
      quoteState: context.quoteState,
    })
    .signers([signer])
    .rpc();
}

async function updateQuoteV2(
  program: anchor.Program,
  context: PoolContext,
  signer: Keypair,
  sequence: number,
  publishSlot: number,
  baseHalfSpreadBps: number,
) {
  await program.methods
    .updateReferenceQuoteV2({
      midPriceQ64X64: Q64_ONE,
      baseHalfSpreadBps,
      publishSlot: new anchor.BN(publishSlot),
      sequence: new anchor.BN(sequence),
    })
    .accounts({
      quoteSigner: signer.publicKey,
      poolConfig: context.poolConfig,
      quoteState: context.quoteState,
    })
    .signers([signer])
    .rpc();
}

async function pausePool(
  program: anchor.Program,
  context: PoolContext,
  authority: Keypair,
  paused: boolean,
) {
  await program.methods
    .pausePool({ paused })
    .accounts({
      authority: authority.publicKey,
      poolConfig: context.poolConfig,
    })
    .signers([authority])
    .rpc();
}

async function initializeCurveSlotState(
  program: anchor.Program,
  payer: Keypair,
  context: PoolContext,
) {
  await program.methods
    .initializeCurveSlotState()
    .accounts({
      payer: payer.publicKey,
      authority: context.authority.publicKey,
      poolConfig: context.poolConfig,
      curveSlotState: context.curveSlotState,
      systemProgram: SystemProgram.programId,
    })
    .signers([context.authority])
    .rpc();
}

async function stageCurveSlot(
  program: anchor.Program,
  context: PoolContext,
  slot: number,
  curveHash: number[],
  sequence: number,
) {
  await program.methods
    .stageCurveSlot({
      slot,
      curveHash,
      sequence: new anchor.BN(sequence),
    })
    .accounts({
      authority: context.authority.publicKey,
      poolConfig: context.poolConfig,
      curveSlotState: context.curveSlotState,
    })
    .signers([context.authority])
    .rpc();
}

async function activateCurveSlot(
  program: anchor.Program,
  context: PoolContext,
  slot: number,
  expectedHash: number[],
  sequence: number,
) {
  await program.methods
    .activateCurveSlot({
      slot,
      expectedHash,
      sequence: new anchor.BN(sequence),
    })
    .accounts({
      authority: context.authority.publicKey,
      poolConfig: context.poolConfig,
      curveSlotState: context.curveSlotState,
    })
    .signers([context.authority])
    .rpc();
}

async function createFundedSource(
  connection: anchor.web3.Connection,
  payer: Keypair,
  mintAuthority: Keypair,
  owner: PublicKey,
  mint: PublicKey,
  amount: bigint,
  tokenProgram: TokenProgramId,
): Promise<PublicKey> {
  const source = await getOrCreateAssociatedTokenAccount(
    connection,
    payer,
    mint,
    owner,
    false,
    "confirmed",
    undefined,
    tokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );
  await mintTo(
    connection,
    payer,
    mint,
    source.address,
    mintAuthority,
    amount,
    [],
    undefined,
    tokenProgram,
  );
  return source.address;
}

async function createWrappedSolSource(
  connection: anchor.web3.Connection,
  payer: Keypair,
  authority: Keypair,
  lamports: bigint,
): Promise<PublicKey> {
  const source = getAssociatedTokenAddressSync(
    NATIVE_MINT,
    authority.publicKey,
    false,
    TOKEN_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );
  const tx = new Transaction().add(
    createAssociatedTokenAccountIdempotentInstruction(
      payer.publicKey,
      source,
      authority.publicKey,
      NATIVE_MINT,
      TOKEN_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    ),
    SystemProgram.transfer({
      fromPubkey: authority.publicKey,
      toPubkey: source,
      lamports: Number(lamports),
    }),
    createSyncNativeInstruction(source, TOKEN_PROGRAM_ID),
  );
  tx.feePayer = payer.publicKey;
  await sendAndConfirmTransaction(connection, tx, [payer, authority], {
    commitment: "confirmed",
  });
  return source;
}

async function createTransferFeeMint(
  connection: anchor.web3.Connection,
  payer: Keypair,
  authority: Keypair,
  decimals: number,
): Promise<PublicKey> {
  const mint = Keypair.generate();
  const mintLen = getMintLen([ExtensionType.TransferFeeConfig]);
  const lamports = await connection.getMinimumBalanceForRentExemption(mintLen);
  const tx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: payer.publicKey,
      newAccountPubkey: mint.publicKey,
      space: mintLen,
      lamports,
      programId: TOKEN_2022_PROGRAM_ID,
    }),
    createInitializeTransferFeeConfigInstruction(
      mint.publicKey,
      authority.publicKey,
      authority.publicKey,
      100,
      10_000n,
      TOKEN_2022_PROGRAM_ID,
    ),
    createInitializeMintInstruction(
      mint.publicKey,
      decimals,
      authority.publicKey,
      null,
      TOKEN_2022_PROGRAM_ID,
    ),
  );
  tx.feePayer = payer.publicKey;
  await sendAndConfirmTransaction(connection, tx, [payer, mint], {
    commitment: "confirmed",
  });
  return mint.publicKey;
}

async function createEmptyAta(
  connection: anchor.web3.Connection,
  payer: Keypair,
  mint: PublicKey,
  owner: PublicKey,
  tokenProgram: TokenProgramId,
): Promise<PublicKey> {
  const account = await getOrCreateAssociatedTokenAccount(
    connection,
    payer,
    mint,
    owner,
    false,
    "confirmed",
    undefined,
    tokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );
  return account.address;
}

async function fundPool(
  program: anchor.Program,
  context: PoolContext,
  baseSource: PublicKey,
  quoteSource: PublicKey,
  baseAmount: bigint,
  quoteAmount: bigint,
) {
  await program.methods
    .fundPool({
      baseAmount: new anchor.BN(baseAmount.toString()),
      quoteAmount: new anchor.BN(quoteAmount.toString()),
    })
    .accounts({
      authority: context.authority.publicKey,
      poolConfig: context.poolConfig,
      vaultAuthority: context.vaultAuthority,
      vaultState: context.vaultState,
      baseMint: context.baseMint,
      quoteMint: context.quoteMint,
      baseTokenProgram: context.baseTokenProgram,
      quoteTokenProgram: context.quoteTokenProgram,
      baseSource,
      quoteSource,
      baseVault: context.baseVault,
      quoteVault: context.quoteVault,
    })
    .signers([context.authority])
    .rpc();
}

async function swapExactIn(
  program: anchor.Program,
  context: PoolContext,
  taker: Keypair,
  amountIn: bigint,
  minimumAmountOut: bigint,
  expectedQuoteSequence: bigint,
  baseToQuote: boolean,
) {
  await program.methods
    .swapExactIn({
      amountIn: new anchor.BN(amountIn.toString()),
      minimumAmountOut: new anchor.BN(minimumAmountOut.toString()),
      expectedQuoteSequence: new anchor.BN(expectedQuoteSequence.toString()),
      baseToQuote,
    })
    .accounts({
      taker: taker.publicKey,
      poolConfig: context.poolConfig,
      quoteState: context.quoteState,
      vaultAuthority: context.vaultAuthority,
      vaultState: context.vaultState,
      baseMint: context.baseMint,
      quoteMint: context.quoteMint,
      baseTokenProgram: context.baseTokenProgram,
      quoteTokenProgram: context.quoteTokenProgram,
      userBaseAccount: getAssociatedTokenAddressSync(
        context.baseMint,
        taker.publicKey,
        false,
        context.baseTokenProgram,
        ASSOCIATED_TOKEN_PROGRAM_ID,
      ),
      userQuoteAccount: getAssociatedTokenAddressSync(
        context.quoteMint,
        taker.publicKey,
        false,
        context.quoteTokenProgram,
        ASSOCIATED_TOKEN_PROGRAM_ID,
      ),
      baseVault: context.baseVault,
      quoteVault: context.quoteVault,
    })
    .signers([taker])
    .rpc();
}

async function tokenAmount(
  connection: anchor.web3.Connection,
  account: PublicKey,
  tokenProgram: TokenProgramId,
): Promise<bigint> {
  const tokenAccount = await getAccount(
    connection,
    account,
    "confirmed",
    tokenProgram,
  );
  return tokenAccount.amount;
}

async function assertTokenAmount(
  connection: anchor.web3.Connection,
  account: PublicKey,
  expected: bigint,
  tokenProgram: TokenProgramId,
) {
  const tokenAccount = await getAccount(
    connection,
    account,
    "confirmed",
    tokenProgram,
  );
  assert.equal(tokenAccount.amount, expected);
}

async function waitForSlot(
  connection: anchor.web3.Connection,
  targetSlot: number,
) {
  for (;;) {
    const slot = await connection.getSlot("confirmed");
    if (slot >= targetSlot) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
}

class DeterministicRng {
  private state: number;

  constructor(seedValue: number) {
    this.state = seedValue >>> 0;
  }

  nextU32(): number {
    let x = this.state;
    x ^= x << 13;
    x ^= x >>> 17;
    x ^= x << 5;
    this.state = x >>> 0;
    return this.state;
  }

  int(maxExclusive: number): number {
    assert(maxExclusive > 0, "maxExclusive must be positive");
    return this.nextU32() % maxExclusive;
  }

  bool(): boolean {
    return (this.nextU32() & 1) === 1;
  }

  bigint(minInclusive: bigint, maxInclusive: bigint): bigint {
    assert(maxInclusive >= minInclusive, "invalid bigint range");
    const span = maxInclusive - minInclusive + 1n;
    return minInclusive + (BigInt(this.nextU32()) % span);
  }
}

async function readSwapBalances(
  connection: anchor.web3.Connection,
  context: PoolContext,
  taker: Keypair,
): Promise<SwapBalances> {
  const userBaseAccount = getAssociatedTokenAddressSync(
    context.baseMint,
    taker.publicKey,
    false,
    context.baseTokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );
  const userQuoteAccount = getAssociatedTokenAddressSync(
    context.quoteMint,
    taker.publicKey,
    false,
    context.quoteTokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );
  return {
    userBase: await tokenAmount(
      connection,
      userBaseAccount,
      context.baseTokenProgram,
    ),
    userQuote: await tokenAmount(
      connection,
      userQuoteAccount,
      context.quoteTokenProgram,
    ),
    vaultBase: await tokenAmount(
      connection,
      context.baseVault,
      context.baseTokenProgram,
    ),
    vaultQuote: await tokenAmount(
      connection,
      context.quoteVault,
      context.quoteTokenProgram,
    ),
  };
}

function assertNoSwapBalanceChange(
  before: SwapBalances,
  after: SwapBalances,
  label: string,
) {
  assert.deepEqual(after, before, `${label}: balances changed after rejection`);
}

function assertSwapConservesTokens(
  before: SwapBalances,
  after: SwapBalances,
  label: string,
) {
  assert.equal(
    after.userBase + after.vaultBase,
    before.userBase + before.vaultBase,
    `${label}: base conservation failed`,
  );
  assert.equal(
    after.userQuote + after.vaultQuote,
    before.userQuote + before.vaultQuote,
    `${label}: quote conservation failed`,
  );
}

function uniqueSigners(payer: Keypair, signers: Keypair[]): Keypair[] {
  const seen = new Set<string>();
  const unique: Keypair[] = [];
  for (const signer of [payer, ...signers]) {
    const key = signer.publicKey.toBase58();
    if (!seen.has(key)) {
      seen.add(key);
      unique.push(signer);
    }
  }
  return unique;
}

function extractProgramComputeUnits(
  programId: PublicKey,
  logs: string[],
): number {
  const prefix = `Program ${programId.toBase58()} consumed `;
  const consumed = logs
    .filter((log) => log.startsWith(prefix))
    .map((log) => {
      const rest = log.slice(prefix.length);
      return Number(rest.slice(0, rest.indexOf(" ")));
    })
    .filter((value) => Number.isInteger(value) && value > 0);
  assert(
    consumed.length > 0,
    `missing compute log for ${programId.toBase58()}:\n${logs.join("\n")}`,
  );
  return Math.max(...consumed);
}

async function measureAndSendInstruction(
  label: string,
  program: anchor.Program,
  provider: anchor.AnchorProvider,
  payer: Keypair,
  instruction: TransactionInstruction,
  signers: Keypair[],
): Promise<ComputeRow> {
  const latest = await provider.connection.getLatestBlockhash("confirmed");
  const txSigners = uniqueSigners(payer, signers);
  const message = new TransactionMessage({
    payerKey: payer.publicKey,
    recentBlockhash: latest.blockhash,
    instructions: [instruction],
  }).compileToV0Message();
  const tx = new VersionedTransaction(message);
  tx.sign(txSigners);

  const simulation = await provider.connection.simulateTransaction(tx, {
    commitment: "confirmed",
    sigVerify: true,
  });
  const logs = simulation.value.logs ?? [];
  assert.equal(
    simulation.value.err,
    null,
    `${label} simulation failed: ${JSON.stringify(simulation.value.err)}\n${logs.join("\n")}`,
  );

  const signature = await provider.connection.sendTransaction(tx, {
    preflightCommitment: "confirmed",
  });
  await provider.connection.confirmTransaction(
    { signature, ...latest },
    "confirmed",
  );
  return {
    instruction: label,
    consumedUnits: extractProgramComputeUnits(program.programId, logs),
    signature,
  };
}

async function runLocalSplCase(
  program: anchor.Program,
  provider: anchor.AnchorProvider,
  payer: Keypair,
) {
  const authority = await fundedAuthority(provider.connection);
  const quoteAuthority = Keypair.generate();
  const baseMint = await createMint(
    provider.connection,
    payer,
    authority.publicKey,
    null,
    9,
    undefined,
    undefined,
    TOKEN_PROGRAM_ID,
  );
  const quoteMint = await createMint(
    provider.connection,
    payer,
    authority.publicKey,
    null,
    6,
    undefined,
    undefined,
    TOKEN_PROGRAM_ID,
  );
  const context = derivePoolContext(
    program.programId,
    authority,
    quoteAuthority,
    baseMint,
    quoteMint,
    TOKEN_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
  );

  await initializePool(program, payer, context);
  await initializeQuoteState(program, payer, context);
  await initializeCurveSlotState(program, payer, context);
  const curveHash = Array.from({ length: 32 }, () => 7);
  await stageCurveSlot(program, context, 2, curveHash, 1);
  await activateCurveSlot(program, context, 2, curveHash, 2);
  const curveSlotState = await (program.account as any).curveSlotState.fetch(
    context.curveSlotState,
  );
  assert.equal(curveSlotState.activeSlot, 2);
  assert.equal(curveSlotState.pendingHash.every((byte: number) => byte === 0), true);

  const slot = await provider.connection.getSlot("confirmed");
  await updateQuote(program, context, quoteAuthority, 1, slot);
  await expectReject(
    "rejects replayed quote sequence",
    () => updateQuote(program, context, quoteAuthority, 1, slot),
    /NonMonotonicQuoteSequence|6006|custom program error/,
  );
  await expectReject(
    "rejects stale publish slot even with higher sequence",
    () => updateQuote(program, context, quoteAuthority, 2, Math.max(0, slot - 1)),
    /StaleQuotePublishSlot|6007|custom program error/,
  );
  await expectReject(
    "rejects future publish slot",
    () => updateQuote(program, context, quoteAuthority, 2, slot + 10_000),
    /FutureQuotePublishSlot|6008|custom program error/,
  );
  await expectReject(
    "rejects unauthorized quote signer",
    () => updateQuote(program, context, Keypair.generate(), 2, slot),
    /UnauthorizedQuoteUpdate|6004|custom program error/,
  );
  await expectReject(
    "rejects unauthorized pool pause",
    () => pausePool(program, context, Keypair.generate(), true),
    /UnauthorizedPoolAuthority|6002|custom program error/,
  );

  await initializeVaults(program, payer, context);
  await expectReject(
    "rejects wrong token program on vault init",
    async () => {
      const otherAuthority = await fundedAuthority(provider.connection);
      const otherQuoteAuthority = Keypair.generate();
      const otherBaseMint = await createMint(
        provider.connection,
        payer,
        otherAuthority.publicKey,
        null,
        9,
        undefined,
        undefined,
        TOKEN_PROGRAM_ID,
      );
      const otherQuoteMint = await createMint(
        provider.connection,
        payer,
        otherAuthority.publicKey,
        null,
        6,
        undefined,
        undefined,
        TOKEN_PROGRAM_ID,
      );
      const mismatched = derivePoolContext(
        program.programId,
        otherAuthority,
        otherQuoteAuthority,
        otherBaseMint,
        otherQuoteMint,
        TOKEN_PROGRAM_ID,
        TOKEN_PROGRAM_ID,
      );
      await initializePool(program, payer, mismatched);
      mismatched.baseTokenProgram = TOKEN_2022_PROGRAM_ID;
      await initializeVaults(program, payer, mismatched);
    },
    /Constraint|program owner|VaultTokenProgramMismatch|IncorrectProgramId|incorrect program id|custom program error/,
  );

  const baseSource = await createFundedSource(
    provider.connection,
    payer,
    authority,
    authority.publicKey,
    baseMint,
    2_000_000_000n,
    TOKEN_PROGRAM_ID,
  );
  const quoteSource = await createFundedSource(
    provider.connection,
    payer,
    authority,
    authority.publicKey,
    quoteMint,
    5_000_000n,
    TOKEN_PROGRAM_ID,
  );

  await expectReject(
    "rejects zero-sided funding",
    () => fundPool(program, context, baseSource, quoteSource, 0n, 0n),
    /EmptyFundingAmount|6018|custom program error/,
  );

  await fundPool(program, context, baseSource, quoteSource, 700_000_000n, 1_250_000n);
  await assertTokenAmount(
    provider.connection,
    context.baseVault,
    700_000_000n,
    TOKEN_PROGRAM_ID,
  );
  await assertTokenAmount(
    provider.connection,
    context.quoteVault,
    1_250_000n,
    TOKEN_PROGRAM_ID,
  );
  await assertTokenAmount(
    provider.connection,
    baseSource,
    1_300_000_000n,
    TOKEN_PROGRAM_ID,
  );
  await assertTokenAmount(
    provider.connection,
    quoteSource,
    3_750_000n,
    TOKEN_PROGRAM_ID,
  );

  const swapQuoteSlot = await provider.connection.getSlot("confirmed");
  await updateQuote(program, context, quoteAuthority, 2, swapQuoteSlot);

  const taker = await fundedAuthority(provider.connection);
  const takerBase = await createFundedSource(
    provider.connection,
    payer,
    authority,
    taker.publicKey,
    baseMint,
    100_000_000n,
    TOKEN_PROGRAM_ID,
  );
  const takerQuote = await createFundedSource(
    provider.connection,
    payer,
    authority,
    taker.publicKey,
    quoteMint,
    100_000n,
    TOKEN_PROGRAM_ID,
  );

  const unchangedBefore = await tokenAmount(
    provider.connection,
    takerBase,
    TOKEN_PROGRAM_ID,
  );
  await expectReject(
    "rejects swap below requested minimum output",
    () => swapExactIn(program, context, taker, 100n, 1_000_000n, 2n, true),
    /SlippageExceeded|6025|custom program error/,
  );
  await assertTokenAmount(
    provider.connection,
    takerBase,
    unchangedBefore,
    TOKEN_PROGRAM_ID,
  );

  const userBaseBefore = await tokenAmount(
    provider.connection,
    takerBase,
    TOKEN_PROGRAM_ID,
  );
  const userQuoteBefore = await tokenAmount(
    provider.connection,
    takerQuote,
    TOKEN_PROGRAM_ID,
  );
  const vaultBaseBefore = await tokenAmount(
    provider.connection,
    context.baseVault,
    TOKEN_PROGRAM_ID,
  );
  const vaultQuoteBefore = await tokenAmount(
    provider.connection,
    context.quoteVault,
    TOKEN_PROGRAM_ID,
  );

  await swapExactIn(program, context, taker, 100n, 90n, 2n, true);

  const userBaseAfter = await tokenAmount(
    provider.connection,
    takerBase,
    TOKEN_PROGRAM_ID,
  );
  const userQuoteAfter = await tokenAmount(
    provider.connection,
    takerQuote,
    TOKEN_PROGRAM_ID,
  );
  const vaultBaseAfter = await tokenAmount(
    provider.connection,
    context.baseVault,
    TOKEN_PROGRAM_ID,
  );
  const vaultQuoteAfter = await tokenAmount(
    provider.connection,
    context.quoteVault,
    TOKEN_PROGRAM_ID,
  );
  const quoteOut = userQuoteAfter - userQuoteBefore;
  assert.equal(userBaseBefore - userBaseAfter, 100n);
  assert.equal(vaultBaseAfter - vaultBaseBefore, 100n);
  assert.equal(vaultQuoteBefore - vaultQuoteAfter, quoteOut);
  assert(quoteOut >= 90n);

  await waitForSlot(provider.connection, swapQuoteSlot + 16);
  await expectReject(
    "rejects stale quote swaps",
    () => swapExactIn(program, context, taker, 100n, 1n, 2n, true),
    /StaleReferenceQuote|6029|custom program error/,
  );

  const refreshedSlot = await provider.connection.getSlot("confirmed");
  await updateQuote(program, context, quoteAuthority, 3, refreshedSlot);
  await expectReject(
    "rejects swap bound to superseded quote sequence",
    () => swapExactIn(program, context, taker, 100n, 1n, 2n, true),
    /QuoteSequenceMismatch|6022|custom program error/,
  );

  await pausePool(program, context, authority, true);
  const pausedBaseBefore = await tokenAmount(
    provider.connection,
    takerBase,
    TOKEN_PROGRAM_ID,
  );
  await expectReject(
    "rejects swaps while pool is paused",
    () => swapExactIn(program, context, taker, 100n, 1n, 3n, true),
    /PoolConfigPaused|6019|custom program error/,
  );
  await assertTokenAmount(
    provider.connection,
    takerBase,
    pausedBaseBefore,
    TOKEN_PROGRAM_ID,
  );
  await pausePool(program, context, authority, false);

  const userBaseBeforeReverse = await tokenAmount(
    provider.connection,
    takerBase,
    TOKEN_PROGRAM_ID,
  );
  const userQuoteBeforeReverse = await tokenAmount(
    provider.connection,
    takerQuote,
    TOKEN_PROGRAM_ID,
  );
  const vaultBaseBeforeReverse = await tokenAmount(
    provider.connection,
    context.baseVault,
    TOKEN_PROGRAM_ID,
  );
  const vaultQuoteBeforeReverse = await tokenAmount(
    provider.connection,
    context.quoteVault,
    TOKEN_PROGRAM_ID,
  );

  await swapExactIn(program, context, taker, 100n, 90n, 3n, false);

  const userBaseAfterReverse = await tokenAmount(
    provider.connection,
    takerBase,
    TOKEN_PROGRAM_ID,
  );
  const userQuoteAfterReverse = await tokenAmount(
    provider.connection,
    takerQuote,
    TOKEN_PROGRAM_ID,
  );
  const vaultBaseAfterReverse = await tokenAmount(
    provider.connection,
    context.baseVault,
    TOKEN_PROGRAM_ID,
  );
  const vaultQuoteAfterReverse = await tokenAmount(
    provider.connection,
    context.quoteVault,
    TOKEN_PROGRAM_ID,
  );
  const baseOut = userBaseAfterReverse - userBaseBeforeReverse;
  assert.equal(userQuoteBeforeReverse - userQuoteAfterReverse, 100n);
  assert.equal(vaultQuoteAfterReverse - vaultQuoteBeforeReverse, 100n);
  assert.equal(vaultBaseBeforeReverse - vaultBaseAfterReverse, baseOut);
  assert(baseOut >= 90n);

  const spreadUpdateSlot = await provider.connection.getSlot("confirmed");
  await updateQuoteV2(program, context, quoteAuthority, 4, spreadUpdateSlot, 25);
  const quoteState = await (program.account as any).referenceQuoteState.fetch(
    context.quoteState,
  );
  assert.equal(quoteState.sequence.toString(), "4");
  assert.equal(quoteState.publishSlot.toString(), String(spreadUpdateSlot));

  console.log("ok: local SPL pool quote, vault, and funding invariants");
}

async function setupFundedSplPool(
  program: anchor.Program,
  provider: anchor.AnchorProvider,
  payer: Keypair,
) {
  const authority = await fundedAuthority(provider.connection);
  const quoteAuthority = Keypair.generate();
  const baseMint = await createMint(
    provider.connection,
    payer,
    authority.publicKey,
    null,
    9,
    undefined,
    undefined,
    TOKEN_PROGRAM_ID,
  );
  const quoteMint = await createMint(
    provider.connection,
    payer,
    authority.publicKey,
    null,
    6,
    undefined,
    undefined,
    TOKEN_PROGRAM_ID,
  );
  const context = derivePoolContext(
    program.programId,
    authority,
    quoteAuthority,
    baseMint,
    quoteMint,
    TOKEN_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
  );

  await initializePool(program, payer, context);
  await initializeQuoteState(program, payer, context);
  await initializeVaults(program, payer, context);

  const baseSource = await createFundedSource(
    provider.connection,
    payer,
    authority,
    authority.publicKey,
    baseMint,
    2_000_000_000n,
    TOKEN_PROGRAM_ID,
  );
  const quoteSource = await createFundedSource(
    provider.connection,
    payer,
    authority,
    authority.publicKey,
    quoteMint,
    2_000_000_000n,
    TOKEN_PROGRAM_ID,
  );
  await fundPool(
    program,
    context,
    baseSource,
    quoteSource,
    700_000_000n,
    700_000_000n,
  );

  const taker = await fundedAuthority(provider.connection);
  await createFundedSource(
    provider.connection,
    payer,
    authority,
    taker.publicKey,
    baseMint,
    10_000_000n,
    TOKEN_PROGRAM_ID,
  );
  await createFundedSource(
    provider.connection,
    payer,
    authority,
    taker.publicKey,
    quoteMint,
    10_000_000n,
    TOKEN_PROGRAM_ID,
  );

  const slot = await provider.connection.getSlot("confirmed");
  await updateQuote(program, context, quoteAuthority, 1, slot);

  return {
    authority,
    quoteAuthority,
    context,
    taker,
    sequence: 1,
  };
}

async function expectRejectedSwapWithoutMovement(
  label: string,
  provider: anchor.AnchorProvider,
  context: PoolContext,
  taker: Keypair,
  action: () => Promise<unknown>,
  pattern: RegExp,
) {
  const before = await readSwapBalances(provider.connection, context, taker);
  await expectReject(label, action, pattern);
  const after = await readSwapBalances(provider.connection, context, taker);
  assertNoSwapBalanceChange(before, after, label);
}

async function runInstructionFuzzCase(
  program: anchor.Program,
  provider: anchor.AnchorProvider,
  payer: Keypair,
) {
  const fixture = await setupFundedSplPool(program, provider, payer);
  const rng = new DeterministicRng(0x4d455441);
  let sequence = fixture.sequence;
  let validSwaps = 0;
  let rejectedOps = 0;
  let validUpdates = 0;
  const refreshQuote = async () => {
    const slot = await provider.connection.getSlot("confirmed");
    sequence += 1;
    await updateQuote(
      program,
      fixture.context,
      fixture.quoteAuthority,
      sequence,
      slot,
    );
    validUpdates += 1;
  };

  for (let caseIndex = 0; caseIndex < FUZZ_CASES; caseIndex += 1) {
    const op = rng.int(8);
    const label = `instruction fuzz case ${caseIndex} op ${op}`;

    switch (op) {
      case 0:
        await expectReject(
          `${label}: unauthorized quote update`,
          () => updateQuote(
            program,
            fixture.context,
            Keypair.generate(),
            sequence + 1,
            Number.MAX_SAFE_INTEGER,
          ),
          /UnauthorizedQuoteUpdate|6004|custom program error/,
        );
        rejectedOps += 1;
        break;
      case 1: {
        const slot = await provider.connection.getSlot("confirmed");
        await expectReject(
          `${label}: replayed quote sequence`,
          () => updateQuote(
            program,
            fixture.context,
            fixture.quoteAuthority,
            sequence,
            slot,
          ),
          /NonMonotonicQuoteSequence|6006|custom program error/,
        );
        rejectedOps += 1;
        break;
      }
      case 2: {
        await expectReject(
          `${label}: stale publish slot`,
          () => updateQuote(
            program,
            fixture.context,
            fixture.quoteAuthority,
            sequence + 1,
            0,
          ),
          /StaleQuotePublishSlot|6007|custom program error/,
        );
        rejectedOps += 1;
        break;
      }
      case 3:
        await expectReject(
          `${label}: unauthorized pause`,
          () => pausePool(program, fixture.context, Keypair.generate(), true),
          /UnauthorizedPoolAuthority|6002|custom program error/,
        );
        rejectedOps += 1;
        break;
      case 4:
        await refreshQuote();
        await expectRejectedSwapWithoutMovement(
          `${label}: wrong quote sequence swap`,
          provider,
          fixture.context,
          fixture.taker,
          () => swapExactIn(
            program,
            fixture.context,
            fixture.taker,
            rng.bigint(1n, 250n),
            1n,
            BigInt(sequence + 1 + rng.int(4)),
            rng.bool(),
          ),
          /QuoteSequenceMismatch|6022|custom program error/,
        );
        rejectedOps += 1;
        break;
      case 5:
        await refreshQuote();
        await expectRejectedSwapWithoutMovement(
          `${label}: slippage-protected swap`,
          provider,
          fixture.context,
          fixture.taker,
          () => swapExactIn(
            program,
            fixture.context,
            fixture.taker,
            rng.bigint(1n, 250n),
            1_000_000_000_000n,
            BigInt(sequence),
            rng.bool(),
          ),
          /SlippageExceeded|6025|custom program error/,
        );
        rejectedOps += 1;
        break;
      case 6: {
        await refreshQuote();
        break;
      }
      default: {
        await refreshQuote();
        const baseToQuote = rng.bool();
        const amountIn = rng.bigint(25n, 250n);
        const before = await readSwapBalances(
          provider.connection,
          fixture.context,
          fixture.taker,
        );
        await swapExactIn(
          program,
          fixture.context,
          fixture.taker,
          amountIn,
          1n,
          BigInt(sequence),
          baseToQuote,
        );
        const after = await readSwapBalances(
          provider.connection,
          fixture.context,
          fixture.taker,
        );
        assertSwapConservesTokens(before, after, label);
        validSwaps += 1;
        break;
      }
    }
  }

  assert(validSwaps > 0, "instruction fuzz should include at least one swap");
  assert(rejectedOps > 0, "instruction fuzz should include rejected ops");
  console.log(
    `instruction fuzz passed: cases=${FUZZ_CASES} swaps=${validSwaps} updates=${validUpdates} rejects=${rejectedOps}`,
  );
}

async function runComputeBudgetCase(
  program: anchor.Program,
  provider: anchor.AnchorProvider,
  payer: Keypair,
) {
  const authority = await fundedAuthority(provider.connection);
  const quoteAuthority = Keypair.generate();
  const baseMint = await createMint(
    provider.connection,
    payer,
    authority.publicKey,
    null,
    9,
    undefined,
    undefined,
    TOKEN_PROGRAM_ID,
  );
  const quoteMint = await createMint(
    provider.connection,
    payer,
    authority.publicKey,
    null,
    6,
    undefined,
    undefined,
    TOKEN_PROGRAM_ID,
  );
  const context = derivePoolContext(
    program.programId,
    authority,
    quoteAuthority,
    baseMint,
    quoteMint,
    TOKEN_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
  );
  const rows: ComputeRow[] = [];

  rows.push(await measureAndSendInstruction(
    "initialize_reference_quote_pool",
    program,
    provider,
    payer,
    await program.methods
      .initializeReferenceQuotePool(baseArgs())
      .accounts({
        payer: payer.publicKey,
        authority: context.authority.publicKey,
        baseMint: context.baseMint,
        quoteMint: context.quoteMint,
        baseTokenProgram: context.baseTokenProgram,
        quoteTokenProgram: context.quoteTokenProgram,
        poolConfig: context.poolConfig,
        systemProgram: SystemProgram.programId,
      })
      .instruction(),
    [context.authority],
  ));

  rows.push(await measureAndSendInstruction(
    "initialize_reference_quote_state",
    program,
    provider,
    payer,
    await program.methods
      .initializeReferenceQuoteState({
        quoteAuthority: context.quoteAuthority.publicKey,
      })
      .accounts({
        payer: payer.publicKey,
        authority: context.authority.publicKey,
        poolConfig: context.poolConfig,
        quoteState: context.quoteState,
        systemProgram: SystemProgram.programId,
      })
      .instruction(),
    [context.authority],
  ));

  rows.push(await measureAndSendInstruction(
    "initialize_curve_slot_state",
    program,
    provider,
    payer,
    await program.methods
      .initializeCurveSlotState()
      .accounts({
        payer: payer.publicKey,
        authority: context.authority.publicKey,
        poolConfig: context.poolConfig,
        curveSlotState: context.curveSlotState,
        systemProgram: SystemProgram.programId,
      })
      .instruction(),
    [context.authority],
  ));

  const curveHash = Array.from({ length: 32 }, () => 9);
  rows.push(await measureAndSendInstruction(
    "stage_curve_slot",
    program,
    provider,
    payer,
    await program.methods
      .stageCurveSlot({
        slot: 1,
        curveHash,
        sequence: new anchor.BN(1),
      })
      .accounts({
        authority: context.authority.publicKey,
        poolConfig: context.poolConfig,
        curveSlotState: context.curveSlotState,
      })
      .instruction(),
    [context.authority],
  ));

  rows.push(await measureAndSendInstruction(
    "activate_curve_slot",
    program,
    provider,
    payer,
    await program.methods
      .activateCurveSlot({
        slot: 1,
        expectedHash: curveHash,
        sequence: new anchor.BN(2),
      })
      .accounts({
        authority: context.authority.publicKey,
        poolConfig: context.poolConfig,
        curveSlotState: context.curveSlotState,
      })
      .instruction(),
    [context.authority],
  ));

  rows.push(await measureAndSendInstruction(
    "initialize_maker_vaults",
    program,
    provider,
    payer,
    await program.methods
      .initializeMakerVaults()
      .accounts({
        payer: payer.publicKey,
        authority: context.authority.publicKey,
        poolConfig: context.poolConfig,
        vaultAuthority: context.vaultAuthority,
        baseMint: context.baseMint,
        quoteMint: context.quoteMint,
        baseTokenProgram: context.baseTokenProgram,
        quoteTokenProgram: context.quoteTokenProgram,
        vaultState: context.vaultState,
        baseVault: context.baseVault,
        quoteVault: context.quoteVault,
        systemProgram: SystemProgram.programId,
      })
      .instruction(),
    [context.authority],
  ));

  const baseSource = await createFundedSource(
    provider.connection,
    payer,
    authority,
    authority.publicKey,
    baseMint,
    2_000_000_000n,
    TOKEN_PROGRAM_ID,
  );
  const quoteSource = await createFundedSource(
    provider.connection,
    payer,
    authority,
    authority.publicKey,
    quoteMint,
    2_000_000_000n,
    TOKEN_PROGRAM_ID,
  );
  rows.push(await measureAndSendInstruction(
    "fund_pool",
    program,
    provider,
    payer,
    await program.methods
      .fundPool({
        baseAmount: new anchor.BN("700000000"),
        quoteAmount: new anchor.BN("700000000"),
      })
      .accounts({
        authority: context.authority.publicKey,
        poolConfig: context.poolConfig,
        vaultAuthority: context.vaultAuthority,
        vaultState: context.vaultState,
        baseMint: context.baseMint,
        quoteMint: context.quoteMint,
        baseTokenProgram: context.baseTokenProgram,
        quoteTokenProgram: context.quoteTokenProgram,
        baseSource,
        quoteSource,
        baseVault: context.baseVault,
        quoteVault: context.quoteVault,
      })
      .instruction(),
    [context.authority],
  ));

  const slot = await provider.connection.getSlot("confirmed");
  rows.push(await measureAndSendInstruction(
    "update_reference_quote",
    program,
    provider,
    payer,
    await program.methods
      .updateReferenceQuote({
        midPriceQ64X64: Q64_ONE,
        publishSlot: new anchor.BN(slot),
        sequence: new anchor.BN(1),
      })
      .accounts({
        quoteSigner: context.quoteAuthority.publicKey,
        poolConfig: context.poolConfig,
        quoteState: context.quoteState,
      })
      .instruction(),
    [context.quoteAuthority],
  ));

  const spreadSlot = await provider.connection.getSlot("confirmed");
  rows.push(await measureAndSendInstruction(
    "update_reference_quote_v2",
    program,
    provider,
    payer,
    await program.methods
      .updateReferenceQuoteV2({
        midPriceQ64X64: Q64_ONE,
        baseHalfSpreadBps: 25,
        publishSlot: new anchor.BN(spreadSlot),
        sequence: new anchor.BN(2),
      })
      .accounts({
        quoteSigner: context.quoteAuthority.publicKey,
        poolConfig: context.poolConfig,
        quoteState: context.quoteState,
      })
      .instruction(),
    [context.quoteAuthority],
  ));

  rows.push(await measureAndSendInstruction(
    "pause_pool",
    program,
    provider,
    payer,
    await program.methods
      .pausePool({ paused: true })
      .accounts({
        authority: context.authority.publicKey,
        poolConfig: context.poolConfig,
      })
      .instruction(),
    [context.authority],
  ));
  await pausePool(program, context, authority, false);

  const taker = await fundedAuthority(provider.connection);
  await createFundedSource(
    provider.connection,
    payer,
    authority,
    taker.publicKey,
    baseMint,
    10_000_000n,
    TOKEN_PROGRAM_ID,
  );
  await createFundedSource(
    provider.connection,
    payer,
    authority,
    taker.publicKey,
    quoteMint,
    10_000_000n,
    TOKEN_PROGRAM_ID,
  );
  rows.push(await measureAndSendInstruction(
    "swap_exact_in",
    program,
    provider,
    payer,
    await program.methods
      .swapExactIn({
        amountIn: new anchor.BN(100),
        minimumAmountOut: new anchor.BN(1),
        expectedQuoteSequence: new anchor.BN(2),
        baseToQuote: true,
      })
      .accounts({
        taker: taker.publicKey,
        poolConfig: context.poolConfig,
        quoteState: context.quoteState,
        vaultAuthority: context.vaultAuthority,
        vaultState: context.vaultState,
        baseMint: context.baseMint,
        quoteMint: context.quoteMint,
        baseTokenProgram: context.baseTokenProgram,
        quoteTokenProgram: context.quoteTokenProgram,
        userBaseAccount: getAssociatedTokenAddressSync(
          context.baseMint,
          taker.publicKey,
          false,
          context.baseTokenProgram,
          ASSOCIATED_TOKEN_PROGRAM_ID,
        ),
        userQuoteAccount: getAssociatedTokenAddressSync(
          context.quoteMint,
          taker.publicKey,
          false,
          context.quoteTokenProgram,
          ASSOCIATED_TOKEN_PROGRAM_ID,
        ),
        baseVault: context.baseVault,
        quoteVault: context.quoteVault,
      })
      .instruction(),
    [taker],
  ));

  const maxConsumed = Math.max(...rows.map((row) => row.consumedUnits));
  assert(maxConsumed < 200_000, "instruction exceeded default tx CU limit");

  const report = {
    generatedAt: new Date().toISOString(),
    rpc: RPC_URL,
    program: program.programId.toBase58(),
    maxConsumedUnits: maxConsumed,
    rows,
  };
  fs.mkdirSync(path.dirname(CU_REPORT_PATH), { recursive: true });
  fs.writeFileSync(CU_REPORT_PATH, `${JSON.stringify(report, null, 2)}\n`);

  for (const row of rows) {
    console.log(`cu: ${row.instruction} consumed=${row.consumedUnits}`);
  }
  console.log(`compute budget report: ${CU_REPORT_PATH}`);
}

async function runToken2022Case(
  program: anchor.Program,
  provider: anchor.AnchorProvider,
  payer: Keypair,
) {
  const authority = await fundedAuthority(provider.connection);
  const quoteAuthority = Keypair.generate();
  const baseMint = await createMint(
    provider.connection,
    payer,
    authority.publicKey,
    null,
    6,
    undefined,
    undefined,
    TOKEN_2022_PROGRAM_ID,
  );
  const quoteMint = await createMint(
    provider.connection,
    payer,
    authority.publicKey,
    null,
    6,
    undefined,
    undefined,
    TOKEN_PROGRAM_ID,
  );
  const context = derivePoolContext(
    program.programId,
    authority,
    quoteAuthority,
    baseMint,
    quoteMint,
    TOKEN_2022_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
  );

  await initializePool(program, payer, context);
  await initializeVaults(program, payer, context);

  const baseSource = await createFundedSource(
    provider.connection,
    payer,
    authority,
    authority.publicKey,
    baseMint,
    2_000_000n,
    TOKEN_2022_PROGRAM_ID,
  );
  const quoteSource = await createEmptyAta(
    provider.connection,
    payer,
    quoteMint,
    authority.publicKey,
    TOKEN_PROGRAM_ID,
  );
  await fundPool(program, context, baseSource, quoteSource, 750_000n, 0n);
  await assertTokenAmount(
    provider.connection,
    context.baseVault,
    750_000n,
    TOKEN_2022_PROGRAM_ID,
  );

  const feeBaseMint = await createTransferFeeMint(
    provider.connection,
    payer,
    authority,
    6,
  );
  const feeQuoteMint = await createMint(
    provider.connection,
    payer,
    authority.publicKey,
    null,
    6,
    undefined,
    undefined,
    TOKEN_PROGRAM_ID,
  );
  const transferFeeContext = derivePoolContext(
    program.programId,
    authority,
    Keypair.generate(),
    feeBaseMint,
    feeQuoteMint,
    TOKEN_2022_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
  );
  await expectReject(
    "rejects Token-2022 transfer-fee mints until net-of-fee quoting exists",
    () => initializePool(program, payer, transferFeeContext),
    /UnsupportedTokenExtension|custom program error/,
  );

  console.log("ok: Token-2022 base mint funds through TokenInterface");
}

async function runMainnetMintMimicCase(
  program: anchor.Program,
  provider: anchor.AnchorProvider,
  payer: Keypair,
) {
  const nativeMint = await getMint(
    provider.connection,
    NATIVE_MINT,
    "confirmed",
    TOKEN_PROGRAM_ID,
  );
  const usdcMint = await getMint(
    provider.connection,
    MAINNET_USDC,
    "confirmed",
    TOKEN_PROGRAM_ID,
  );
  assert.equal(nativeMint.decimals, 9, "wSOL mainnet mint profile changed");
  assert.equal(usdcMint.decimals, 6, "USDC mainnet mint profile changed");

  const authority = await fundedAuthority(provider.connection);
  const quoteAuthority = Keypair.generate();
  const context = derivePoolContext(
    program.programId,
    authority,
    quoteAuthority,
    NATIVE_MINT,
    MAINNET_USDC,
    TOKEN_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
  );

  await initializePool(program, payer, context);
  await initializeQuoteState(program, payer, context);
  await initializeVaults(program, payer, context);

  const baseSource = await createWrappedSolSource(
    provider.connection,
    payer,
    authority,
    900_000_000n,
  );
  const quoteSource = await createEmptyAta(
    provider.connection,
    payer,
    MAINNET_USDC,
    authority.publicKey,
    TOKEN_PROGRAM_ID,
  );

  await fundPool(program, context, baseSource, quoteSource, 400_000_000n, 0n);
  await assertTokenAmount(
    provider.connection,
    context.baseVault,
    400_000_000n,
    TOKEN_PROGRAM_ID,
  );
  await assertTokenAmount(
    provider.connection,
    baseSource,
    500_000_000n,
    TOKEN_PROGRAM_ID,
  );
  console.log("ok: mainnet wSOL/USDC mint-profile funding mimic");
}

async function main() {
  if (!fs.existsSync(IDL_PATH)) {
    throw new Error(`missing ${IDL_PATH}; run anchor build first`);
  }

  const payer = loadKeypair(WALLET_PATH);
  const connection = new anchor.web3.Connection(RPC_URL, "confirmed");
  const provider = new anchor.AnchorProvider(
    connection,
    new anchor.Wallet(payer),
    { commitment: "confirmed", preflightCommitment: "confirmed" },
  );
  anchor.setProvider(provider);

  await ensureLamports(connection, payer.publicKey, 5 * LAMPORTS_PER_SOL);

  const idl = JSON.parse(fs.readFileSync(IDL_PATH, "utf8")) as anchor.Idl;
  const program = new anchor.Program(idl, provider);
  const mode = selectedMode();

  console.log(`rpc: ${RPC_URL}`);
  console.log(`program: ${program.programId.toBase58()}`);
  console.log(`payer: ${payer.publicKey.toBase58()}`);
  console.log(`mode: ${mode}`);

  if (mode === "smoke" || mode === "all") {
    await runLocalSplCase(program, provider, payer);
    await runToken2022Case(program, provider, payer);
    await runMainnetMintMimicCase(program, provider, payer);
  }

  if (mode === "fuzz" || mode === "all") {
    await runInstructionFuzzCase(program, provider, payer);
  }

  if (mode === "cu" || mode === "all") {
    await runComputeBudgetCase(program, provider, payer);
  }

  console.log(`surfpool ${mode} passed`);
}

main().catch((error) => {
  console.error(summarizeError(error));
  process.exit(1);
});

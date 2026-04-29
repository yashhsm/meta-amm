# Surfpool Test Specification

This suite tests the current Meta-AMM deployed surface against a Surfpool
mainnet-fork simnet. It is intentionally scoped to the instructions that exist
today:

- `initialize_reference_quote_pool`
- `initialize_reference_quote_state`
- `update_reference_quote`
- `pause_pool`
- `initialize_maker_vaults`
- `fund_pool`
- `swap_exact_in`

The suite validates instruction/account behavior and token conservation. It
does not claim price improvement or maker edge.

## Fork Source

Run the suite against Surfpool with `--network mainnet`. The realistic fixture
uses actual mainnet mint accounts:

| Role | Mint | Token program | Expected decimals |
| --- | --- | --- | --- |
| Base | `So11111111111111111111111111111111111111112` | SPL Token | 9 |
| Quote | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` | SPL Token | 6 |

The suite wraps local SOL into a fork-local token account for the base side.
The USDC source account is created empty and passed with `quote_amount = 0`,
which still exercises mint and account ownership constraints without requiring
control of the real USDC mint authority.

## Required Invariants

1. Pool config PDAs are derived from authority, base mint, and quote mint.
2. Pool config commits the real mint identities and token-program identities.
3. Quote state can only be initialized by the pool authority.
4. Quote updates require either pool authority or configured quote authority.
5. Quote update sequence must be strictly monotonic.
6. Quote publish slots cannot move backward or point into the future.
7. Maker vaults must be deterministic PDA-owned token accounts.
8. Funding must reject zero-sided no-ops.
9. Funding must move exactly the requested token amount from source to vault.
10. Token-program mismatches must fail before any vault state is accepted.
11. Swaps must bind to the quote sequence the taker simulated.
12. Swaps must enforce minimum output before moving tokens.
13. Expired ReferenceQuote state must reject swaps.
14. Vault and user token balances must conserve exactly across both swap
    directions.
15. Pool pause must be authority-gated and must reject swaps without moving
    tokens.
16. Token-2022 transfer-fee mints must be rejected until swap quotes can
    enforce `minimum_amount_out` on net received tokens.

## Cases

### Mainnet Mint Profile

Fetch the real wSOL and USDC mint accounts from the Surfpool datasource. Assert
that decimals and token-program ownership match the expected profile, then
initialize a pool, quote state, maker vaults, and a one-sided wSOL funding flow.

### Local SPL Pair

Create a local 9-decimal base mint and a local 6-decimal quote mint. Exercise
the full happy path including two-sided funding and both swap directions. Verify
exact source, destination, and vault balance deltas.

### Quote Adversarial Cases

After one valid quote update:

- replay the same sequence and expect rejection;
- publish an older slot with a higher sequence and expect rejection;
- publish a future slot and expect rejection;
- submit an update from an unauthorized signer and expect rejection.
- bind a swap to an old quote sequence after a refresh and expect rejection.
- submit a pool pause from an unauthorized signer and expect rejection.

### Custody Adversarial Cases

After vault initialization:

- call `fund_pool` with both amounts set to zero and expect rejection;
- pass the wrong token program for an already configured mint and expect
  account validation failure.
- request an impossible `minimum_amount_out` and expect rejection before token
  movement.
- pause the pool, attempt a swap, and expect rejection before token movement;
  unpause and verify swaps resume.

### Token-2022 Compatibility

Create a local Token-2022 mint as one side of a pool and verify
`initialize_maker_vaults` plus `fund_pool` through the token-interface path.
Create a Token-2022 transfer-fee mint and verify pool initialization rejects it,
because the current swap instruction quotes nominal transfer amounts only.

## Running

```sh
pnpm surfpool:smoke
```

The script starts Surfpool on a mainnet fork, deploys the local program to the
simnet, runs the TypeScript harness, and shuts Surfpool down unless it was
already running on the selected RPC port.

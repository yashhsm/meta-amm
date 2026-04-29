# Surfpool Test Specification

This suite tests the current Meta-AMM deployed surface against a Surfpool
mainnet-fork simnet. It is intentionally scoped to the instructions that exist
today:

- `initialize_reference_quote_pool`
- `initialize_reference_quote_state`
- `update_reference_quote`
- `initialize_maker_vaults`
- `fund_pool`

`swap_exact_in` does not exist yet, so the suite does not claim swap safety,
price improvement, or maker edge.

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

## Cases

### Mainnet Mint Profile

Fetch the real wSOL and USDC mint accounts from the Surfpool datasource. Assert
that decimals and token-program ownership match the expected profile, then
initialize a pool, quote state, maker vaults, and a one-sided wSOL funding flow.

### Local SPL Pair

Create a local 9-decimal base mint and a local 6-decimal quote mint. Exercise
the full happy path including two-sided funding. Verify exact source and vault
balance deltas.

### Quote Adversarial Cases

After one valid quote update:

- replay the same sequence and expect rejection;
- publish an older slot with a higher sequence and expect rejection;
- publish a future slot and expect rejection;
- submit an update from an unauthorized signer and expect rejection.

### Custody Adversarial Cases

After vault initialization:

- call `fund_pool` with both amounts set to zero and expect rejection;
- pass the wrong token program for an already configured mint and expect
  account validation failure.

### Token-2022 Compatibility

Create a local Token-2022 mint as one side of a pool and verify
`initialize_maker_vaults` plus `fund_pool` through the token-interface path.

## Running

```sh
pnpm surfpool:smoke
```

The script starts Surfpool on a mainnet fork, deploys the local program to the
simnet, runs the TypeScript harness, and shuts Surfpool down unless it was
already running on the selected RPC port.

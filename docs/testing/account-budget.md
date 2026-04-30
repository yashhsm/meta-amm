# Account Budget

This report records the current account-shape budget for the implemented
ReferenceQuote path. Account metas are treated like compute units: every feature
that touches the swap path must spend from a fixed budget and update this file.

## Exact-In Swap

The ReferenceQuote exact-in swap account contract is exported from
`meta_amm_config` as `REFERENCE_QUOTE_SWAP_ACCOUNT_LABELS`.

| Index | Account | Writable | Signer | Purpose |
| --- | --- | --- | --- | --- |
| 0 | `taker` | no | yes | user authority for input transfer |
| 1 | `pool_config` | no | no | immutable pool identity and config bytes |
| 2 | `quote_state` | no | no | signed midprice, sequence, spread, freshness |
| 3 | `vault_authority` | no | no | PDA signer for vault output transfer |
| 4 | `vault_state` | no | no | maker-owned custody binding and target inventory |
| 5 | `base_mint` | no | no | base mint and decimals |
| 6 | `quote_mint` | no | no | quote mint and decimals |
| 7 | `base_token_program` | no | no | SPL Token or Token-2022 interface |
| 8 | `quote_token_program` | no | no | SPL Token or Token-2022 interface |
| 9 | `user_base_account` | yes | no | taker base token account |
| 10 | `user_quote_account` | yes | no | taker quote token account |
| 11 | `base_vault` | yes | no | pool base token vault |
| 12 | `quote_vault` | yes | no | pool quote token vault |

Required account metas: 13.

Default account meta budget: 15.

Remaining headroom: 2 metas.

## Non-Swap Instructions

These instructions are not part of the aggregator exact-in path:

- `initialize_reference_quote_pool`
- `initialize_reference_quote_state`
- `update_reference_quote`
- `update_reference_quote_v2`
- `pause_pool`
- `initialize_curve_slot_state`
- `stage_curve_slot`
- `activate_curve_slot`
- `initialize_maker_vaults`
- `fund_pool`

`CurveSlotState` is intentionally outside `swap_exact_in` today. Wiring it into
swaps would spend at least one additional account meta and should not happen
without a fresh account/CU measurement.

## Budget Rules

- Do not add referral, oracle-push, flow-policy, or curve-slot accounts to the
  base exact-in path without increasing the manifest and re-running fork smoke.
- Keep exact-out, RFQ, and future pooled-LP paths separately budgeted.
- Token-2022 transfer-fee mints remain rejected; supporting them may require
  additional state or net-of-fee accounting and must be measured separately.
- `ReferenceSwapEvent` adds logging cost but no account metas.

## Measurement Status

`pnpm surfpool:cu` deploys the program to a Surfpool fork, simulates each
implemented instruction as a single v0 transaction, parses the program compute
log, executes the same transaction, and writes the full JSON report to
`target/surfpool-compute-budget.json`.

Latest measured run:

| Instruction | Consumed CU |
| --- | ---: |
| `initialize_reference_quote_pool` | 15,282 |
| `initialize_reference_quote_state` | 12,785 |
| `initialize_curve_slot_state` | 9,674 |
| `stage_curve_slot` | 5,929 |
| `activate_curve_slot` | 5,943 |
| `initialize_maker_vaults` | 42,745 |
| `fund_pool` | 29,675 |
| `update_reference_quote` | 6,207 |
| `update_reference_quote_v2` | 6,257 |
| `pause_pool` | 3,458 |
| `swap_exact_in` | 33,874 |

Measured max: 42,745 CU.

Measured exact-in swap: 33,874 CU with 13 required account metas.

These numbers are from a local Surfpool fork and should be treated as regression
baselines, not final mainnet fee settings. Re-run `pnpm surfpool:cu` after every
program instruction change that affects accounts, events, token movement, or
math branches.

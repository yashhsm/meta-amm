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

Current tests validate account order and fork execution, but this report is not
yet a compute-unit benchmark. The next measurement layer should use Mollusk or
transaction simulation logs to record CU for each implemented instruction.

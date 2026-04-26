# Decimal Scale Preimage Golden Vector

Canonical preimage domain:

```text
meta-amm:decimal-scale:v1
```

Layout:

```text
domain[25] ||
base_mint[32] ||
quote_mint[32] ||
base_token_program[32] ||
quote_token_program[32] ||
base_decimals:u8 ||
quote_decimals:u8 ||
price_domain:u8
```

Length: `156` bytes.

Example:

```text
base_mint = [1; 32]
quote_mint = [2; 32]
base_token_program = [3; 32]
quote_token_program = [4; 32]
base_decimals = 8
quote_decimals = 6
price_domain = 0
```

This vector is intentionally pre-hash. The on-chain program and SDKs must hash
these exact bytes with SHA-256 or Solana's canonical hash function when the core
program is introduced.

Hex fixture:

```text
6d6574612d616d6d3a646563696d616c2d7363616c653a76310101010101010101010101010101010101010101010101010101010101010101020202020202020202020202020202020202020202020202020202020202020203030303030303030303030303030303030303030303030303030303030303030404040404040404040404040404040404040404040404040404040404040404080600
```

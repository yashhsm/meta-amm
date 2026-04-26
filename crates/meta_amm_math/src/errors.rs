#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathError {
    DivByZero,
    Overflow,
    Underflow,
    InvalidAmount,
    InvalidFee,
    InvalidConfig,
    EmptyLiquidity,
    DecimalsOutOfRange,
    StaleQuote,
    QuoteProtected,
    InventoryBand,
    PoolPaused,
}

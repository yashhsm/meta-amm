#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub mod cpmm;
pub mod decimal;
pub mod errors;
pub mod fixed;
pub mod piecewise;
pub mod reference;

pub use cpmm::{quote_exact_in as cpmm_quote_exact_in, CpmmQuote, CpmmReserves};
pub use decimal::{
    DecimalScale, DecimalScalePreimage, PriceDomain, DECIMAL_SCALE_DOMAIN,
    DECIMAL_SCALE_PREIMAGE_LEN,
};
pub use errors::MathError;
pub use fixed::{mul_div_floor, Q64x64, Q64};
pub use piecewise::{
    apply_post_fill_to_side, buy_base_with_quote, sell_base_for_quote, PiecewiseBookSide,
    PiecewiseQuote, PostFillPolicy, PIECEWISE_PRICE_POINT_COUNT, PIECEWISE_SEGMENT_COUNT,
};
pub use reference::{
    quote_exact_in as reference_quote_exact_in, QuoteAgeState, ReferenceQuote,
    ReferenceQuoteParams, ReferenceQuoteState,
};

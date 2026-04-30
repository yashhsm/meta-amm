use meta_amm_math::{
    reference_quote_exact_in, Q64x64, QuoteAgeState, ReferenceQuoteParams, ReferenceQuoteState,
};

#[test]
fn reference_quote_matches_golden_parity_fixture() {
    for row in parse_rows(include_str!(
        "../../../tests/golden/reference-quote-parity.csv"
    )) {
        let quote = reference_quote_exact_in(
            ReferenceQuoteState {
                base_inventory: row.base_inventory,
                quote_inventory: row.quote_inventory,
                target_base_inventory: row.target_base_inventory,
                mid_price: Q64x64::from_int(row.mid_price_int),
                mid_publish_slot: row.mid_publish_slot,
                now_slot: row.now_slot,
                paused: row.paused,
            },
            ReferenceQuoteParams {
                fee_bps: row.fee_bps,
                base_half_spread_bps: row.base_half_spread_bps,
                aging_start_slots: row.aging_start_slots,
                protected_start_slots: row.protected_start_slots,
                expire_slots: row.expire_slots,
                aging_surcharge_bps_per_slot: row.aging_surcharge_bps_per_slot,
                max_aging_surcharge_bps: row.max_aging_surcharge_bps,
                max_trade_base_atoms: row.max_trade_base_atoms,
                protected_max_trade_base_atoms: row.protected_max_trade_base_atoms,
                inventory_skew_bps_per_10k_imbalance: row.inventory_skew_bps_per_10k_imbalance,
                max_inventory_skew_bps: row.max_inventory_skew_bps,
                hard_inventory_band_bps: row.hard_inventory_band_bps,
            },
            row.amount_in,
            row.base_to_quote,
        )
        .unwrap_or_else(|error| panic!("{} failed with {error:?}", row.name));

        assert_eq!(quote.age_state, row.age_state, "{}", row.name);
        assert_eq!(
            quote.amount_in_less_fee, row.amount_in_less_fee,
            "{}",
            row.name
        );
        assert_eq!(quote.amount_out, row.amount_out, "{}", row.name);
        assert_eq!(
            quote.new_base_inventory, row.new_base_inventory,
            "{}",
            row.name
        );
        assert_eq!(
            quote.new_quote_inventory, row.new_quote_inventory,
            "{}",
            row.name
        );
        assert_eq!(
            quote.effective_price.0, row.effective_price_q64x64,
            "{}",
            row.name
        );
        assert_eq!(
            quote.applied_spread_bps, row.applied_spread_bps,
            "{}",
            row.name
        );
        assert_eq!(
            quote.inventory_imbalance_bps, row.inventory_imbalance_bps,
            "{}",
            row.name
        );
    }
}

#[derive(Debug)]
struct ParityRow {
    name: String,
    base_inventory: u64,
    quote_inventory: u64,
    target_base_inventory: u64,
    mid_price_int: u64,
    mid_publish_slot: u64,
    now_slot: u64,
    paused: bool,
    fee_bps: u16,
    base_half_spread_bps: u16,
    aging_start_slots: u64,
    protected_start_slots: u64,
    expire_slots: u64,
    aging_surcharge_bps_per_slot: u16,
    max_aging_surcharge_bps: u16,
    max_trade_base_atoms: u64,
    protected_max_trade_base_atoms: u64,
    inventory_skew_bps_per_10k_imbalance: u16,
    max_inventory_skew_bps: u16,
    hard_inventory_band_bps: u16,
    amount_in: u64,
    base_to_quote: bool,
    age_state: QuoteAgeState,
    amount_in_less_fee: u64,
    amount_out: u64,
    new_base_inventory: u64,
    new_quote_inventory: u64,
    effective_price_q64x64: u128,
    applied_spread_bps: u16,
    inventory_imbalance_bps: i32,
}

fn parse_rows(input: &str) -> Vec<ParityRow> {
    input
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(parse_row)
        .collect()
}

fn parse_row(line: &str) -> ParityRow {
    let fields = line.split(',').collect::<Vec<_>>();
    assert_eq!(fields.len(), 30, "unexpected parity fixture width");

    ParityRow {
        name: fields[0].to_owned(),
        base_inventory: parse(fields[1]),
        quote_inventory: parse(fields[2]),
        target_base_inventory: parse(fields[3]),
        mid_price_int: parse(fields[4]),
        mid_publish_slot: parse(fields[5]),
        now_slot: parse(fields[6]),
        paused: parse_bool(fields[7]),
        fee_bps: parse(fields[8]),
        base_half_spread_bps: parse(fields[9]),
        aging_start_slots: parse(fields[10]),
        protected_start_slots: parse(fields[11]),
        expire_slots: parse(fields[12]),
        aging_surcharge_bps_per_slot: parse(fields[13]),
        max_aging_surcharge_bps: parse(fields[14]),
        max_trade_base_atoms: parse(fields[15]),
        protected_max_trade_base_atoms: parse(fields[16]),
        inventory_skew_bps_per_10k_imbalance: parse(fields[17]),
        max_inventory_skew_bps: parse(fields[18]),
        hard_inventory_band_bps: parse(fields[19]),
        amount_in: parse(fields[20]),
        base_to_quote: parse_bool(fields[21]),
        age_state: parse_age_state(fields[22]),
        amount_in_less_fee: parse(fields[23]),
        amount_out: parse(fields[24]),
        new_base_inventory: parse(fields[25]),
        new_quote_inventory: parse(fields[26]),
        effective_price_q64x64: parse(fields[27]),
        applied_spread_bps: parse(fields[28]),
        inventory_imbalance_bps: parse(fields[29]),
    }
}

fn parse<T: core::str::FromStr>(input: &str) -> T
where
    T::Err: core::fmt::Debug,
{
    input.parse().expect("valid parity fixture field")
}

fn parse_bool(input: &str) -> bool {
    match input {
        "true" => true,
        "false" => false,
        _ => panic!("invalid bool field: {input}"),
    }
}

fn parse_age_state(input: &str) -> QuoteAgeState {
    match input {
        "Fresh" => QuoteAgeState::Fresh,
        "Aging" => QuoteAgeState::Aging,
        "Protected" => QuoteAgeState::Protected,
        "Expired" => QuoteAgeState::Expired,
        "Paused" => QuoteAgeState::Paused,
        _ => panic!("invalid age state field: {input}"),
    }
}

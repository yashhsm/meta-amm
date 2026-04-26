use std::collections::HashMap;

use meta_amm_math::{MathError, Q64x64};

use crate::{FlowOrder, MarketSlot, QuoteUpdatePolicy, SameSlotUpdateOrder, Side, SummaryU64};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayMarketPath {
    pub slots: Vec<MarketSlot>,
    pub quote_updates: Vec<QuoteUpdateEvent>,
}

impl ReplayMarketPath {
    pub fn first_fair_price(&self) -> Option<Q64x64> {
        self.slots.first().map(|slot| slot.fair_price)
    }

    pub fn flow_distribution(&self) -> Result<FlowDistributionReport, MathError> {
        flow_distribution(self)
    }

    pub fn landing_distribution(&self) -> LandingDistributionReport {
        landing_distribution(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuoteUpdateEvent {
    pub publish_slot: u64,
    pub landing_slot: Option<u64>,
    pub mid_price: Q64x64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowDistributionReport {
    pub observations: u64,
    pub trades: u64,
    pub trade_probability_bps: u16,
    pub base_to_quote_trades: u64,
    pub quote_to_base_trades: u64,
    pub base_equivalent_amount: SummaryU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LandingDistributionReport {
    pub quote_updates_sent: u64,
    pub quote_updates_landed: u64,
    pub quote_updates_dropped: u64,
    pub success_probability_bps: u16,
    pub latency_slots: SummaryU64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayParseError {
    EmptyInput,
    MissingColumn(&'static str),
    QuotedCsvUnsupported {
        line: usize,
    },
    InvalidField {
        line: usize,
        column: &'static str,
        value: String,
    },
    InvalidSide {
        line: usize,
        value: String,
    },
}

pub fn parse_replay_csv(input: &str) -> Result<ReplayMarketPath, ReplayParseError> {
    let mut lines = input
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty() && !line.trim_start().starts_with('#'));
    let Some((header_line, header)) = lines.next() else {
        return Err(ReplayParseError::EmptyInput);
    };
    if header.contains('"') {
        return Err(ReplayParseError::QuotedCsvUnsupported {
            line: header_line + 1,
        });
    }

    let headers = parse_row(header, header_line + 1)?;
    let indices = ReplayColumnIndices::new(&headers)?;
    let mut slots = Vec::new();
    let mut quote_updates = Vec::new();

    for (line_index, line) in lines {
        let line_number = line_index + 1;
        if line.contains('"') {
            return Err(ReplayParseError::QuotedCsvUnsupported { line: line_number });
        }
        let fields = parse_row(line, line_number)?;
        let slot = parse_u64(cell(&fields, indices.slot), line_number, "slot")?;
        let fair_price = Q64x64::from_int(parse_u64(
            cell(&fields, indices.fair_price),
            line_number,
            "fair_price",
        )?);
        let side_cell = optional_cell(&fields, indices.side);
        let amount_cell = optional_cell(&fields, indices.amount_in);
        let flow = parse_optional_flow(side_cell, amount_cell, line_number)?;

        if let Some(landing_cell) = optional_cell(&fields, indices.quote_landing_slot) {
            if !landing_cell.is_empty() {
                quote_updates.push(QuoteUpdateEvent {
                    publish_slot: slot,
                    landing_slot: parse_landing_slot(landing_cell, line_number)?,
                    mid_price: fair_price,
                });
            }
        }

        slots.push(MarketSlot {
            slot,
            fair_price,
            flow,
        });
    }

    Ok(ReplayMarketPath {
        slots,
        quote_updates,
    })
}

pub fn flow_distribution(path: &ReplayMarketPath) -> Result<FlowDistributionReport, MathError> {
    let mut trades = 0u64;
    let mut base_to_quote_trades = 0u64;
    let mut quote_to_base_trades = 0u64;
    let mut min_amount = u64::MAX;
    let mut max_amount = 0u64;
    let mut sum_amount = 0u128;

    for slot in &path.slots {
        let Some(flow) = slot.flow else {
            continue;
        };
        trades += 1;
        let base_equivalent = match flow.side {
            Side::BaseToQuote => {
                base_to_quote_trades += 1;
                flow.amount_in
            }
            Side::QuoteToBase => {
                quote_to_base_trades += 1;
                slot.fair_price.div_amount_floor(flow.amount_in)?
            }
        };
        min_amount = min_amount.min(base_equivalent);
        max_amount = max_amount.max(base_equivalent);
        sum_amount = sum_amount.saturating_add(base_equivalent as u128);
    }

    Ok(FlowDistributionReport {
        observations: path.slots.len() as u64,
        trades,
        trade_probability_bps: bps(trades, path.slots.len() as u64),
        base_to_quote_trades,
        quote_to_base_trades,
        base_equivalent_amount: SummaryU64 {
            min: if trades == 0 { 0 } else { min_amount },
            mean: if trades == 0 {
                0
            } else {
                (sum_amount / (trades as u128)) as u64
            },
            max: max_amount,
        },
    })
}

pub fn landing_distribution(path: &ReplayMarketPath) -> LandingDistributionReport {
    let sent = path.quote_updates.len() as u64;
    let mut landed = 0u64;
    let mut dropped = 0u64;
    let mut min_latency = u64::MAX;
    let mut max_latency = 0u64;
    let mut sum_latency = 0u128;

    for update in &path.quote_updates {
        match update.landing_slot {
            Some(landing_slot) => {
                landed += 1;
                let latency = landing_slot.saturating_sub(update.publish_slot);
                min_latency = min_latency.min(latency);
                max_latency = max_latency.max(latency);
                sum_latency = sum_latency.saturating_add(latency as u128);
            }
            None => dropped += 1,
        }
    }

    LandingDistributionReport {
        quote_updates_sent: sent,
        quote_updates_landed: landed,
        quote_updates_dropped: dropped,
        success_probability_bps: bps(landed, sent),
        latency_slots: SummaryU64 {
            min: if landed == 0 { 0 } else { min_latency },
            mean: if landed == 0 {
                0
            } else {
                (sum_latency / (landed as u128)) as u64
            },
            max: max_latency,
        },
    }
}

pub fn quote_update_policy_from_replay(
    path: &ReplayMarketPath,
    same_slot_order: SameSlotUpdateOrder,
    failure_seed: u128,
) -> Result<QuoteUpdatePolicy, MathError> {
    let mut previous = None;
    let mut delta_sum = 0u128;
    let mut delta_count = 0u128;

    for update in &path.quote_updates {
        if let Some(previous_slot) = previous {
            if update.publish_slot < previous_slot {
                return Err(MathError::InvalidConfig);
            }
            let delta = update.publish_slot.saturating_sub(previous_slot);
            if delta > 0 {
                delta_sum += delta as u128;
                delta_count += 1;
            }
        }
        if update
            .landing_slot
            .is_some_and(|landing_slot| landing_slot < update.publish_slot)
        {
            return Err(MathError::InvalidConfig);
        }
        previous = Some(update.publish_slot);
    }

    if delta_count == 0 {
        return Err(MathError::InvalidConfig);
    }

    let landing = landing_distribution(path);
    Ok(QuoteUpdatePolicy {
        maker_update_period_slots: (delta_sum / delta_count) as u64,
        landing_latency_slots: landing.latency_slots.mean,
        update_success_probability_bps: landing.success_probability_bps,
        failure_seed,
        same_slot_order,
    })
}

fn bps(numerator: u64, denominator: u64) -> u16 {
    if denominator == 0 {
        return 0;
    }
    ((numerator.saturating_mul(10_000)) / denominator) as u16
}

fn parse_row(line: &str, line_number: usize) -> Result<Vec<&str>, ReplayParseError> {
    if line.contains('"') {
        return Err(ReplayParseError::QuotedCsvUnsupported { line: line_number });
    }
    Ok(line.split(',').map(str::trim).collect())
}

fn parse_u64(value: &str, line: usize, column: &'static str) -> Result<u64, ReplayParseError> {
    value
        .parse::<u64>()
        .map_err(|_| ReplayParseError::InvalidField {
            line,
            column,
            value: value.to_owned(),
        })
}

fn parse_optional_flow(
    side: Option<&str>,
    amount_in: Option<&str>,
    line: usize,
) -> Result<Option<FlowOrder>, ReplayParseError> {
    let side = side.unwrap_or("");
    let amount_in = amount_in.unwrap_or("");
    if side.is_empty() && amount_in.is_empty() {
        return Ok(None);
    }
    if side.is_empty() {
        return Err(ReplayParseError::InvalidField {
            line,
            column: "side",
            value: side.to_owned(),
        });
    }
    if amount_in.is_empty() {
        return Err(ReplayParseError::InvalidField {
            line,
            column: "amount_in",
            value: amount_in.to_owned(),
        });
    }
    Ok(Some(FlowOrder {
        side: parse_side(side, line)?,
        amount_in: parse_u64(amount_in, line, "amount_in")?,
    }))
}

fn parse_side(value: &str, line: usize) -> Result<Side, ReplayParseError> {
    match value.to_ascii_lowercase().as_str() {
        "base_to_quote" | "b2q" | "sell_base" => Ok(Side::BaseToQuote),
        "quote_to_base" | "q2b" | "buy_base" => Ok(Side::QuoteToBase),
        _ => Err(ReplayParseError::InvalidSide {
            line,
            value: value.to_owned(),
        }),
    }
}

fn parse_landing_slot(value: &str, line: usize) -> Result<Option<u64>, ReplayParseError> {
    match value {
        "drop" | "dropped" | "none" => Ok(None),
        _ => Ok(Some(parse_u64(value, line, "quote_landing_slot")?)),
    }
}

fn cell<'a>(fields: &'a [&'a str], index: usize) -> &'a str {
    fields.get(index).copied().unwrap_or("")
}

fn optional_cell<'a>(fields: &'a [&'a str], index: Option<usize>) -> Option<&'a str> {
    index.map(|index| cell(fields, index))
}

struct ReplayColumnIndices {
    slot: usize,
    fair_price: usize,
    side: Option<usize>,
    amount_in: Option<usize>,
    quote_landing_slot: Option<usize>,
}

impl ReplayColumnIndices {
    fn new(headers: &[&str]) -> Result<Self, ReplayParseError> {
        let mut positions = HashMap::new();
        for (index, header) in headers.iter().enumerate() {
            positions.insert(header.to_ascii_lowercase(), index);
        }
        Ok(Self {
            slot: required_column(&positions, &["slot"])?,
            fair_price: required_column(
                &positions,
                &["fair_price", "fair_price_quote_atoms_per_base_atom"],
            )?,
            side: optional_column(&positions, &["side"]),
            amount_in: optional_column(&positions, &["amount_in"]),
            quote_landing_slot: optional_column(
                &positions,
                &["quote_landing_slot", "update_landing_slot", "landing_slot"],
            ),
        })
    }
}

fn required_column(
    positions: &HashMap<String, usize>,
    names: &[&'static str],
) -> Result<usize, ReplayParseError> {
    optional_column(positions, names).ok_or(ReplayParseError::MissingColumn(names[0]))
}

fn optional_column(positions: &HashMap<String, usize>, names: &[&str]) -> Option<usize> {
    names.iter().find_map(|name| positions.get(*name).copied())
}

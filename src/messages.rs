use actix::{
    dev::{MessageResponse, OneshotSender},
    Actor, Message,
};
use chrono::{DateTime, Utc};
use yahoo_finance_api::Quote;

use crate::{cli_error::CliError, model::ProcessedQuote};

pub struct FetchAll;

impl Message for FetchAll {
    type Result = ();
}

pub struct Fetch {
    pub symbol: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl Fetch {
    pub fn new(symbol: String, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Fetch { symbol, start, end }
    }
}

impl Message for Fetch {
    type Result = ();
}

pub struct ProcessQuote {
    pub symbol: String,
    pub start: DateTime<Utc>,
    pub quotes: Vec<Quote>,
}

impl ProcessQuote {
    pub fn new(symbol: String, start: DateTime<Utc>, quotes: Vec<Quote>) -> Self {
        ProcessQuote {
            symbol,
            start,
            quotes,
        }
    }
}

impl Message for ProcessQuote {
    type Result = ();
}

pub struct CalcQuoteData(pub Vec<f64>);

impl Message for CalcQuoteData {
    type Result = CalculatedData;
}

pub struct CalculatedData {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub sma: Option<Vec<f64>>,
    pub percent_diff: Option<f64>,
    pub absolute_diff: Option<f64>,
}

impl CalculatedData {
    pub fn new(
        min: Option<f64>,
        max: Option<f64>,
        sma: Option<Vec<f64>>,
        percent_diff: Option<f64>,
        absolute_diff: Option<f64>,
    ) -> Self {
        CalculatedData {
            min,
            max,
            sma,
            percent_diff,
            absolute_diff,
        }
    }
}

impl<A, M> MessageResponse<A, M> for CalculatedData
where
    A: Actor,
    M: Message<Result = CalculatedData>,
{
    fn handle(self, _ctx: &mut A::Context, tx: Option<OneshotSender<M::Result>>) {
        if let Some(tx) = tx {
            if tx.send(self).is_err() {
                eprintln!("Error sending CalculatedData");
            }
        }
    }
}

pub struct HandleError(pub CliError);

impl Message for HandleError {
    type Result = ();
}

pub struct PrintString(pub String);

impl Message for PrintString {
    type Result = ();
}

pub struct PrintProcessedQuote(pub ProcessedQuote);

impl Message for PrintProcessedQuote {
    type Result = ();
}

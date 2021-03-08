use actix::{
    dev::{MessageResponse, OneshotSender},
    Actor, Message,
};
use chrono::{DateTime, Utc};

pub struct ProcessedQuote {
    pub date: DateTime<Utc>,
    pub symbol: String,
    pub close: f64,
    pub change_percent: f64,
    pub min: f64,
    pub max: f64,
    pub avg_30d: f64,
}

impl ProcessedQuote {
    pub fn new(
        date: DateTime<Utc>,
        symbol: String,
        close: f64,
        change_percent: f64,
        min: f64,
        max: f64,
        avg_30d: f64,
    ) -> Self {
        ProcessedQuote {
            date,
            symbol,
            close,
            change_percent,
            min,
            max,
            avg_30d,
        }
    }
}

impl<A, M> MessageResponse<A, M> for ProcessedQuote
where
    A: Actor,
    M: Message<Result = ProcessedQuote>,
{
    fn handle(self, _ctx: &mut A::Context, tx: Option<OneshotSender<M::Result>>) {
        if let Some(tx) = tx {
            if tx.send(self).is_err() {
                eprintln!("Error sending ProcessedQuote");
            }
        }
    }
}

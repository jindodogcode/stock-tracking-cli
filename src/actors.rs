use actix::{fut, prelude::*};
use chrono::{DateTime, TimeZone, Utc};
use yahoo_finance_api::YahooConnector;

use std::time::Duration;

use crate::{
    cli_error::CliError,
    messages::{
        CalcQuoteData, CalculatedData, Fetch, FetchAll, HandleError, PrintProcessedQuote,
        PrintString, ProcessQuote,
    },
    model::ProcessedQuote,
};

pub struct YahooFinance {
    start: DateTime<Utc>,
    symbols: Vec<String>,
}

impl YahooFinance {
    pub fn new(start: DateTime<Utc>, symbols: Vec<String>) -> Self {
        YahooFinance { start, symbols }
    }
}

impl Actor for YahooFinance {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        Printer::from_registry().do_send(PrintString(String::from(
            "period start,symbol,price,change %,min,max,30d avg",
        )));

        ctx.address().do_send(FetchAll);
        ctx.run_interval(Duration::from_secs(30), |_act, ctx| {
            ctx.address().do_send(FetchAll);
        });
    }
}

impl Handler<FetchAll> for YahooFinance {
    type Result = ();

    fn handle(&mut self, _msg: FetchAll, _ctx: &mut Self::Context) {
        let now = Utc::now();
        for symbol in self.symbols.iter() {
            let addr = Fetcher::from_registry();
            addr.do_send(Fetch::new(symbol.clone(), self.start, now));
        }
    }
}

#[derive(Default)]
struct Fetcher;

impl Actor for Fetcher {
    type Context = Context<Self>;
}

impl Supervised for Fetcher {}

impl ArbiterService for Fetcher {}

impl Handler<Fetch> for Fetcher {
    type Result = ();

    fn handle(&mut self, msg: Fetch, ctx: &mut Self::Context) -> Self::Result {
        let symbol = msg.symbol.clone();
        let start = msg.start;
        let future = Box::pin(
            async move {
                let res = YahooConnector::new()
                    .get_quote_history(msg.symbol.as_ref(), msg.start, msg.end)
                    .await;
                res
            }
            .into_actor(self)
            .then(move |res, act, _ctx| {
                let resp = match res {
                    Ok(resp) => resp,
                    Err(e) => {
                        ErrorHandler::from_registry().do_send(HandleError(e.into()));
                        return fut::ready(()).into_actor(act);
                    }
                };

                let quotes = match resp.quotes() {
                    Ok(quotes) => quotes,
                    Err(e) => {
                        ErrorHandler::from_registry().do_send(HandleError(e.into()));
                        return fut::ready(()).into_actor(act);
                    }
                };

                let addr = QuoteProcessor::from_registry();
                addr.do_send(ProcessQuote::new(symbol, start, quotes));

                fut::ready(()).into_actor(act)
            }),
        );

        ctx.spawn(future);
    }
}

#[derive(Default)]
struct QuoteProcessor {
    calc_addr: Option<Addr<Calculator>>,
}

impl Actor for QuoteProcessor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        let cpus = num_cpus::get();
        let addr = SyncArbiter::start(cpus, || Calculator);
        self.calc_addr = Some(addr);
    }
}

impl Supervised for QuoteProcessor {}

impl ArbiterService for QuoteProcessor {}

impl Handler<ProcessQuote> for QuoteProcessor {
    type Result = ();

    fn handle(&mut self, msg: ProcessQuote, ctx: &mut Self::Context) {
        let addr = self.calc_addr.as_ref().unwrap().clone();
        let future = async move {
            let first = msg.quotes.first().unwrap();
            let start = Utc.timestamp(first.timestamp as i64, 0);
            let quotes = msg.quotes.iter().map(|q| q.adjclose).collect::<Vec<f64>>();

            let data = match addr.send(CalcQuoteData(quotes.clone())).await {
                Ok(data) => data,
                Err(e) => {
                    ErrorHandler::from_registry().do_send(HandleError(e.into()));
                    return;
                }
            };

            let current = match quotes.last().ok_or(CliError::CalcNone) {
                Ok(&c) => c,
                Err(e) => {
                    ErrorHandler::from_registry().do_send(HandleError(e));
                    return;
                }
            };
            let min = match data.min.ok_or(CliError::CalcNone) {
                Ok(c) => c,
                Err(e) => {
                    ErrorHandler::from_registry().do_send(HandleError(e));
                    return;
                }
            };
            let max = match data.max.ok_or(CliError::CalcNone) {
                Ok(c) => c,
                Err(e) => {
                    ErrorHandler::from_registry().do_send(HandleError(e));
                    return;
                }
            };
            let sma = match data.sma.unwrap().last().ok_or(CliError::CalcNone) {
                Ok(&c) => c,
                Err(e) => {
                    ErrorHandler::from_registry().do_send(HandleError(e));
                    return;
                }
            };
            let diff = match data.percent_diff.ok_or(CliError::CalcNone) {
                Ok(c) => c,
                Err(e) => {
                    ErrorHandler::from_registry().do_send(HandleError(e));
                    return;
                }
            };

            let quote = ProcessedQuote::new(start, msg.symbol, current, diff, min, max, sma);

            Printer::from_registry().do_send(PrintProcessedQuote(quote));
        }
        .into_actor(self);

        ctx.spawn(future);
    }
}

pub struct Calculator;

impl Calculator {
    fn min(data: &[f64]) -> Option<f64> {
        if data.is_empty() {
            None
        } else {
            let min = data.iter().fold(f64::MAX, |acc, v| v.min(acc));
            Some(min)
        }
    }

    fn max(data: &[f64]) -> Option<f64> {
        if data.is_empty() {
            None
        } else {
            let max = data.iter().fold(f64::MIN, |acc, v| v.max(acc));
            Some(max)
        }
    }

    fn n_window_sma(n: usize, data: &[f64]) -> Option<Vec<f64>> {
        if data.is_empty() || n < 1 {
            None
        } else {
            let sma = data
                .windows(n)
                .map(|w| w.iter().sum::<f64>() / (w.len() as f64))
                .collect::<Vec<f64>>();
            Some(sma)
        }
    }

    fn price_diff(data: &[f64]) -> Option<(f64, f64)> {
        if data.is_empty() {
            return None;
        }

        let first = data.first().unwrap();
        let last = data.last().unwrap();
        let abs_diff = last - first;
        let first = if *first == 0.0 { 1.0 } else { *first };
        let p_diff = abs_diff / first * 100.0;

        Some((p_diff, abs_diff))
    }
}

impl Actor for Calculator {
    type Context = SyncContext<Self>;
}

impl Handler<CalcQuoteData> for Calculator {
    type Result = CalculatedData;

    fn handle(&mut self, msg: CalcQuoteData, _ctx: &mut Self::Context) -> Self::Result {
        let data = msg.0;

        let min = Self::min(&data);
        let max = Self::max(&data);
        let sma = Self::n_window_sma(30, &data);
        let (p_diff, a_diff) = if let Some(tuple) = Self::price_diff(&data) {
            (Some(tuple.0), Some(tuple.1))
        } else {
            (None, None)
        };

        CalculatedData::new(min, max, sma, p_diff, a_diff)
    }
}

#[derive(Default)]
struct ErrorHandler;

impl Actor for ErrorHandler {
    type Context = Context<Self>;
}

impl Supervised for ErrorHandler {}

impl ArbiterService for ErrorHandler {}

impl Handler<HandleError> for ErrorHandler {
    type Result = ();

    fn handle(&mut self, msg: HandleError, _ctx: &mut Self::Context) {
        eprintln!("Error: {}", msg.0);
    }
}

#[derive(Default)]
struct Printer;

impl Actor for Printer {
    type Context = Context<Self>;
}

impl Supervised for Printer {}

impl ArbiterService for Printer {}

impl Handler<PrintString> for Printer {
    type Result = ();

    fn handle(&mut self, msg: PrintString, _ctx: &mut Self::Context) {
        println!("{}", msg.0);
    }
}

impl Handler<PrintProcessedQuote> for Printer {
    type Result = ();

    fn handle(&mut self, msg: PrintProcessedQuote, _ctx: &mut Self::Context) {
        let quote = msg.0;
        println!(
            "{},{},${:.2},{:.2}%,${:.2},${:.2},${:.2}",
            quote.date.to_rfc3339(),
            quote.symbol,
            quote.close,
            quote.change_percent,
            quote.min,
            quote.max,
            quote.avg_30d
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_works() {
        let input = [1.0, 2.0, 3.0, 4.0, 5.0];
        let min = Calculator::min(&input);

        assert!(min.is_some());
        assert!((min.unwrap() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn min_none() {
        let input = [];
        let min = Calculator::min(&input);

        assert!(min.is_none());
    }

    #[test]
    fn max_works() {
        let input = [1.0, 2.0, 3.0, 4.0, 5.0];
        let max = Calculator::max(&input);

        assert!(max.is_some());
        assert!((max.unwrap() - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn max_none() {
        let input = [];
        let max = Calculator::max(&input);

        assert!(max.is_none());
    }

    #[test]
    fn sma_works() {
        let input = [1.0, 2.0, 3.0, 4.0, 5.0];
        let sma = Calculator::n_window_sma(5, &input);

        assert!(sma.is_some());
        let vec = sma.unwrap();
        let sma = vec.last();

        assert!(sma.is_some());
        assert!((sma.unwrap() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn sma_none_empty_vec() {
        let input = [];
        let sma = Calculator::n_window_sma(5, &input);

        assert!(sma.is_none());
    }

    #[test]
    fn sma_none_n_zero() {
        let input = [1.0, 2.0, 3.0, 4.0, 5.0];
        let sma = Calculator::n_window_sma(0, &input);

        assert!(sma.is_none());
    }

    #[test]
    fn price_diff_works() {
        let input = [1.0, 2.0, 3.0, 4.0, 5.0];
        let diff = Calculator::price_diff(&input);

        assert!(diff.is_some());
        let (p_diff, a_diff) = diff.unwrap();

        assert!((p_diff - 400.0).abs() < f64::EPSILON);
        assert!((a_diff - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn price_diff_none() {
        let input = [];
        let diff = Calculator::price_diff(&input);

        assert!(diff.is_none());
    }
}

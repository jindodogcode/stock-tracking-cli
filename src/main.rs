mod cli_error;

use chrono::{prelude::*, Duration};
use clap::{App, Arg, ArgMatches};
use regex::Regex;
use yahoo::YahooConnector;
use yahoo_finance_api as yahoo;

use cli_error::CliResult;

struct QuoteData {
    date: DateTime<Utc>,
    symbol: String,
    close: f64,
    change_percent: f64,
    min: f64,
    max: f64,
    avg_30d: f64,
}

impl QuoteData {
    fn new(
        date: DateTime<Utc>,
        symbol: String,
        close: f64,
        change_percent: f64,
        min: f64,
        max: f64,
        avg_30d: f64,
    ) -> Self {
        QuoteData {
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

fn main() {
    let args = App::new("STC")
        .arg(
            Arg::with_name("start")
                .value_name("Start Date")
                .short("s")
                .long("start")
                .takes_value(true)
                .help("The date to start searching from in YYYY-MM-DD format")
                .required(false)
                .validator(start_arg_match),
        )
        .arg(
            Arg::with_name("symbols")
                .value_name("Stock Symbols")
                .help("The symbols to search for")
                .required(true)
                .multiple(true)
                .default_value("AAPL"),
        )
        .get_matches();

    if let Err(e) = run(args) {
        eprintln!("Error: {}", e);
    }
}

fn run(args: ArgMatches) -> CliResult<()> {
    let symbols = args.values_of("symbols").unwrap();
    let start_utc_datetime: DateTime<Utc> = if let Some(start) = args.value_of("start") {
        datetime_from_str(&start)?
    } else {
        Utc::now() - Duration::days(30)
    };
    let end_utc_datetime = Utc::now();

    let connector = yahoo::YahooConnector::new();
    let mut data: Vec<QuoteData> = Vec::new();
    for symbol in symbols {
        let quote_data =
            fetch_symbol_data(&connector, symbol, start_utc_datetime, end_utc_datetime)?;
        data.push(quote_data);
    }
    print_data(&data);

    Ok(())
}

fn start_arg_match(value: String) -> Result<(), String> {
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    if re.is_match(&value) {
        Ok(())
    } else {
        Err(String::from("Please format the date as: YYYY-MM-DD"))
    }
}

fn datetime_from_str(s: &str) -> CliResult<DateTime<Utc>> {
    let naive_date = NaiveDate::parse_from_str(s, "%F")?;
    let naive_datetime = naive_date.and_hms(0, 0, 0);

    Ok(Utc.from_utc_datetime(&naive_datetime))
}

fn fetch_symbol_data(
    connector: &YahooConnector,
    symbol: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> CliResult<QuoteData> {
    let symbol = symbol.to_uppercase();
    let response = connector.get_quote_history_interval(&symbol, start, end, "1d")?;
    let now = Utc::now();
    let days_30 = Duration::days(30);
    let mut sum_30d = 0.0;
    let mut count_30d = 0;
    let mut min: f64 = f64::MAX;
    let mut max: f64 = f64::MIN;

    let quotes = response.quotes()?;

    for quote in &quotes {
        let time: DateTime<Utc> = Utc.timestamp(quote.timestamp as i64, 0);
        min = min.min(quote.adjclose);
        max = max.max(quote.adjclose);

        if now - time <= days_30 {
            sum_30d += quote.adjclose;
            count_30d += 1;
        }
    }

    let avg_30d = sum_30d / (count_30d as f64);
    let first_date = Utc.timestamp(quotes.first().unwrap().timestamp as i64, 0);
    let first_close = quotes.first().unwrap().adjclose;
    let last_close = quotes.last().unwrap().adjclose;
    let change_percent = (last_close - first_close) / first_close * 100.0;

    Ok(QuoteData::new(
        first_date,
        symbol,
        last_close,
        change_percent,
        min,
        max,
        avg_30d,
    ))
}

fn print_data(data: &[QuoteData]) {
    println!("period start,symbol,price,change %,min,max,30d avg");

    for quote in data {
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

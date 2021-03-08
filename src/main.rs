mod actors;
mod cli_error;
mod messages;
mod model;

use actix::prelude::*;
use chrono::{prelude::*, Duration};
use clap::{App, Arg};
use regex::Regex;

use crate::{actors::YahooFinance, cli_error::CliResult};

#[actix::main]
async fn main() -> CliResult<()> {
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
                .use_delimiter(true)
                .default_value("AAPL"),
        )
        .get_matches();

    let symbols = args.values_of("symbols").unwrap();
    let symbols = symbols.map(|v| v.to_uppercase()).collect::<Vec<String>>();
    let start_utc_datetime: DateTime<Utc> = if let Some(start) = args.value_of("start") {
        let naive_date = NaiveDate::parse_from_str(start, "%F")?;
        let naive_datetime = naive_date.and_hms(0, 0, 0);

        Utc.from_utc_datetime(&naive_datetime)
        // datetime_from_str(&start)?
    } else {
        Utc::now() - Duration::days(30)
    };

    let _yahoo = YahooFinance::new(start_utc_datetime, symbols).start();

    tokio::signal::ctrl_c().await.unwrap();

    System::current().stop();

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

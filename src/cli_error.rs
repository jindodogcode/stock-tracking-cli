use std::{
    convert::From,
    fmt::{self, Display},
};

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
pub enum CliError {
    ActixMailbox(actix::MailboxError),
    CalcNone,
    ChronoParse(chrono::ParseError),
    Regex(regex::Error),
    Yahoo(yahoo_finance_api::YahooError),
}

impl From<chrono::ParseError> for CliError {
    fn from(e: chrono::ParseError) -> Self {
        CliError::ChronoParse(e)
    }
}

impl From<regex::Error> for CliError {
    fn from(e: regex::Error) -> Self {
        CliError::Regex(e)
    }
}

impl From<yahoo_finance_api::YahooError> for CliError {
    fn from(e: yahoo_finance_api::YahooError) -> Self {
        CliError::Yahoo(e)
    }
}

impl From<actix::MailboxError> for CliError {
    fn from(e: actix::MailboxError) -> Self {
        CliError::ActixMailbox(e)
    }
}

impl Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let display_string = match self {
            Self::ActixMailbox(inner) => inner.to_string(),
            Self::CalcNone => String::from("Missing data"),
            Self::ChronoParse(inner) => inner.to_string(),
            Self::Regex(inner) => inner.to_string(),
            Self::Yahoo(inner) => match inner {
                yahoo_finance_api::YahooError::DeserializeFailed(_) => {
                    String::from("fetched bad data from yahoo! finance")
                }
                _ => inner.to_string(),
            },
        };
        write!(f, "{}", display_string)
    }
}

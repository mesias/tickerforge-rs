//! TickerForge: load tickerforge-spec YAML and generate/parse futures tickers.

pub mod calendars;
pub mod contract_cycle;
pub mod dates;
pub mod expiration_rules;
pub mod models;
pub mod month_codes;
pub mod options_models;
pub mod options_spec;
pub mod options_ticker;
pub mod schedule;
pub mod spec_loader;
pub mod ticker_generator;
pub mod ticker_parser;

pub use models::{
    ContractCycle, ContractSpec, Exchange, ExpirationRule, ParsedFuturesTicker, SessionSegment,
    SpecRepository,
};
pub use options_spec::load_option_rules;
pub use options_ticker::{OptionGenerator, OptionParser, ParsedOptionTicker};
pub use spec_loader::{load_spec, load_spec_from_path};
pub use ticker_generator::{generate_ticker_for_contract, TickerForge};
pub use ticker_parser::{parse_ticker, TickerParser};
pub use tickerforge_spec_data::default_spec_root;

/// Alias matching Python `ParsedTicker` for futures.
pub type ParsedTicker = ParsedFuturesTicker;

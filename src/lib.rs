//! TickerForge: load tickerforge-spec YAML and generate/parse futures and options tickers.

pub mod calendars;
pub mod contract_cycle;
pub mod dates;
pub mod expiration_rules;
pub mod models;
pub mod month_codes;
pub mod options_models;
pub mod options_spec;
pub mod options_ticker;
pub mod pattern_index;
pub mod schedule;
pub mod spec_loader;
pub mod ticker_generator;
pub mod ticker_parser;

pub use models::{
    ContractCycle, ContractSpec, Exchange, ExpirationRule, ParsedFuturesTicker, ParsedOptionTicker,
    SessionSegment, SpecRepository,
};
pub use options_spec::load_all_option_rules;
pub use options_ticker::{equity_root, OptionGenerator, OptionParser};
pub use pattern_index::PatternIndex;
pub use spec_loader::{clear_load_spec_cache, load_spec, load_spec_from_path};
pub use ticker_generator::{
    format_contract_ticker, gen_ticker_ctr, gen_ticker_ctr_signed, generate_ticker_for_contract,
    generate_ticker_for_contract_signed, TickerForge,
};
pub use ticker_parser::{
    classify_ticker, classify_ticker_exchange, classify_ticker_spec, classify_ticker_spec_exchange,
    parse_any_ticker, parse_any_ticker_date, parse_any_ticker_date_spec, parse_any_ticker_exchange,
    parse_any_ticker_spec, AnyParsedTicker, AssetType, HasTicker, NoTicker, OptionSide,
    TickerClass, TickerParser, TickerParserBuilder,
};
pub use tickerforge_spec_data::default_spec_root;

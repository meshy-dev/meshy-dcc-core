//! Compatibility re-exports — ranking implementations live in
//! [`meshy_dcc_gateway_search`].

pub use meshy_dcc_gateway_search::{
    ExactScorer, FuzzyScorer, Scorer, ScorerFactory, SearchRecord, StrategyExactScorer,
    StrategyFuzzyScorer, StrategyScorer, SubstringScorer,
};

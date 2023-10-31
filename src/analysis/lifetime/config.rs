#[derive(PartialEq)]
pub enum Precision {
    LOW,
    HIGH
}

pub const generic_matches_all: bool = false;

// - In alias analysis, include "unseen" fields. For example, if we have `self.foo(x)`, then connect
// `x` to all the fields of `self`.
pub const wildcard_field: bool = false;

// Only public functions?
pub const pub_only: bool = true;

// Apply a shallow filter to weed out common false positive trait implementations?
pub const filter: 	bool = true;

// Use alias analysis instead of taint analysis
pub const alias_analysis: bool = true;
// Just do HIR analysis, no MIR (taint/alias)
pub const no_mir: bool = false;

// For debugging
pub const debug_fn: &str = "<fn-to-debug>";

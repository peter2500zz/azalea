//! Standalone parsers, separate from named command-node constructors.
//!
//! Most commands use constructors from the prelude:
//!
//! ```
//! use azalea_brigadier::prelude::*;
//! let mut dispatcher = CommandDispatcher::<()>::new();
//! dispatcher.register(
//!     integer("count")
//!         .describe("How many")
//!         .range(1..=10)
//!         .executes(|ctx| -> CommandResult { Ok(get_integer(ctx, "count").unwrap()) }),
//! );
//! assert_eq!(dispatcher.execute("5", ()).unwrap(), 5);
//! assert!(dispatcher.execute("11", ()).is_err());
//! ```
//!
//! Use these factories when constructing or testing a parser independently,
//! or name one with
//! [CommandArgument::into_arg](crate::builder::CommandArgument::into_arg).
//! Default numeric and boolean parsers also provide `Integer::arg("count")`
//! and `Boolean::arg("enabled")` through that trait. Custom parsers continue to
//! implement [ArgumentType](crate::arguments::ArgumentType); synchronous builds
//! may return non-Send values. Parser objects themselves remain Send + Sync.
//!
//! ```
//! use azalea_brigadier::{arguments::ArgumentType, parsers, string_reader::StringReader};
//! let parser = parsers::integer().range(1..=10);
//! let value = parser.parse(&mut StringReader::from("5")).unwrap();
//! assert_eq!(value.downcast_ref::<i32>(), Some(&5));
//! ```
//!
//! Numeric methods are intentionally unavailable on other node kinds:
//!
//! ```compile_fail
//! use azalea_brigadier::prelude::*;
//! boolean::<(), i32>("enabled").range(0..=1);
//! ```
//!
//! Names are node identifiers and lookup keys, not parser configuration.
//! All constructors accept owned names as well as string slices, moving owned
//! names into the runtime node rather than copying them again.

pub use crate::arguments::{
    bool_argument_type::Boolean, double_argument_type::Double, float_argument_type::Float,
    integer_argument_type::Integer, long_argument_type::Long, string_argument_type::StringArgument,
};

/// Parse an i32, optionally configured with inclusive numeric bounds.
pub fn integer() -> Integer {
    Integer::default()
}
/// Parse an i64, optionally configured with inclusive numeric bounds.
pub fn long() -> Long {
    Long::default()
}
/// Parse an f32, optionally configured with inclusive numeric bounds.
pub fn float() -> Float {
    Float::default()
}
/// Parse an f64, optionally configured with inclusive numeric bounds.
pub fn double() -> Double {
    Double::default()
}
/// Parse a boolean and suggest true/false.
pub fn boolean() -> Boolean {
    Boolean
}
/// Read a single unquoted word.
pub fn word() -> StringArgument {
    StringArgument::SingleWord
}
/// Read a word or a quoted phrase.
pub fn string() -> StringArgument {
    StringArgument::QuotablePhrase
}
/// Read all remaining input as a string.
pub fn greedy_string() -> StringArgument {
    StringArgument::GreedyPhrase
}

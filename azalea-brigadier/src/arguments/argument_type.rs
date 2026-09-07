use std::{any::Any, sync::Arc};

use crate::{
    errors::CommandSyntaxError,
    string_reader::StringReader,
    suggestion::{Suggestions, SuggestionsBuilder},
};

/// The erased value produced by an argument parser.
///
/// Async dispatchers must be able to move a complete command context between
/// executor threads. Synchronous-only builds retain Brigadier's support for
/// thread-local argument values.
#[cfg(feature = "async")]
pub type ParsedValue = dyn Any + Send + Sync;
#[cfg(not(feature = "async"))]
pub type ParsedValue = dyn Any;

/// An object-safe parser, independent of command-node construction.
///
/// Implement [`crate::builder::CommandArgument`] to choose a named builder and
/// expose `Parser::arg(name)` / `parser.into_arg(name)`. Keeping construction
/// in a separate trait allows parsers to remain usable as `dyn ArgumentType`.
pub trait ArgumentType {
    fn parse(&self, reader: &mut StringReader) -> Result<Arc<ParsedValue>, CommandSyntaxError>;

    fn list_suggestions(&self, _builder: SuggestionsBuilder) -> Suggestions {
        Suggestions::default()
    }

    fn examples(&self) -> Vec<String> {
        vec![]
    }
}

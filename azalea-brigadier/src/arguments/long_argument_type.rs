use std::sync::Arc;

use super::{ArgumentType, ParsedValue};
use crate::{
    builder::CommandArgument,
    context::CommandContext,
    errors::{BuiltInError, CommandSyntaxError},
    string_reader::StringReader,
};

/// A standalone i64 parser with optional inclusive bounds.
/// Use [`crate::parsers::long`] to construct it, or [`long`] for a named node.
#[derive(Clone, Debug, Default)]
pub struct Long {
    minimum: Option<i64>,
    maximum: Option<i64>,
}

impl ArgumentType for Long {
    fn parse(&self, reader: &mut StringReader) -> Result<Arc<ParsedValue>, CommandSyntaxError> {
        let start = reader.cursor;
        let result = reader.read_long()?;
        if let Some(minimum) = self.minimum
            && result < minimum
        {
            reader.cursor = start;
            return Err(BuiltInError::LongTooSmall {
                found: result,
                min: minimum,
            }
            .create_with_context(reader));
        }
        if let Some(maximum) = self.maximum
            && result > maximum
        {
            reader.cursor = start;
            return Err(BuiltInError::LongTooBig {
                found: result,
                max: maximum,
            }
            .create_with_context(reader));
        }
        Ok(Arc::new(result))
    }

    fn examples(&self) -> Vec<String> {
        vec!["0", "123", "-123"]
            .into_iter()
            .map(|s| s.to_owned())
            .collect()
    }
}

/// Create a named i64 argument. Bounds may be configured before or after
/// common node methods such as `describe` and `executes`.
pub fn long<S, R>(
    name: impl Into<String>,
) -> crate::builder::argument_builder::ArgumentBuilder<S, R, Long> {
    Long::arg(name)
}

super::numeric::impl_numeric_config!(Long, i64);
pub fn get_long<S, R>(context: &CommandContext<S, R>, name: &str) -> Option<i64> {
    context
        .argument(name)
        .unwrap()
        .downcast_ref::<i64>()
        .copied()
}

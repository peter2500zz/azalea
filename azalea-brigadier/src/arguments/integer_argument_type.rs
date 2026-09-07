use std::sync::Arc;

use super::{ArgumentType, ParsedValue};
use crate::{
    context::CommandContext,
    errors::{BuiltInError, CommandSyntaxError},
    string_reader::StringReader,
};

/// A standalone i32 parser with optional inclusive bounds.
/// Use [`crate::parsers::integer`] to construct it, or [`integer`] for a named
/// node.
#[derive(Clone, Debug, Default)]
pub struct Integer {
    minimum: Option<i32>,
    maximum: Option<i32>,
}

impl ArgumentType for Integer {
    fn parse(&self, reader: &mut StringReader) -> Result<Arc<ParsedValue>, CommandSyntaxError> {
        let start = reader.cursor;
        let result = reader.read_int()?;
        if let Some(minimum) = self.minimum
            && result < minimum
        {
            reader.cursor = start;
            return Err(BuiltInError::IntegerTooSmall {
                found: result,
                min: minimum,
            }
            .create_with_context(reader));
        }
        if let Some(maximum) = self.maximum
            && result > maximum
        {
            reader.cursor = start;
            return Err(BuiltInError::IntegerTooBig {
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

/// Create a named i32 argument. Bounds may be configured before or after
/// common node methods such as `describe` and `executes`.
pub fn integer<S, R>(
    name: impl Into<String>,
) -> crate::builder::argument_builder::ArgumentBuilder<S, R, Integer> {
    crate::builder::required_argument_builder::argument(name, Integer::default())
}

super::numeric::impl_numeric_config!(Integer, i32);
pub fn get_integer<S, R>(context: &CommandContext<S, R>, name: &str) -> Option<i32> {
    context
        .argument(name)
        .unwrap()
        .downcast_ref::<i32>()
        .copied()
}

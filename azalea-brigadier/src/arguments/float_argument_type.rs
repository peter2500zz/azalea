use std::sync::Arc;

use super::{ArgumentType, ParsedValue};
use crate::{
    context::CommandContext,
    errors::{BuiltInError, CommandSyntaxError},
    string_reader::StringReader,
};

/// A standalone f32 parser with optional inclusive bounds.
/// Use [`crate::parsers::float`] to construct it, or [`float`] for a named
/// node.
#[derive(Clone, Debug, Default)]
pub struct Float {
    minimum: Option<f32>,
    maximum: Option<f32>,
}

impl ArgumentType for Float {
    fn parse(&self, reader: &mut StringReader) -> Result<Arc<ParsedValue>, CommandSyntaxError> {
        let start = reader.cursor;
        let result = reader.read_float()?;
        if let Some(minimum) = self.minimum
            && result < minimum
        {
            reader.cursor = start;
            return Err(BuiltInError::FloatTooSmall {
                found: result,
                min: minimum,
            }
            .create_with_context(reader));
        }
        if let Some(maximum) = self.maximum
            && result > maximum
        {
            reader.cursor = start;
            return Err(BuiltInError::FloatTooBig {
                found: result,
                max: maximum,
            }
            .create_with_context(reader));
        }
        Ok(Arc::new(result))
    }

    fn examples(&self) -> Vec<String> {
        vec!["0", "1.2", ".5", "-1", "-.5", "-1234.56"]
            .into_iter()
            .map(|s| s.to_owned())
            .collect()
    }
}

/// Create a named f32 argument. Bounds may be configured before or after
/// common node methods such as `describe` and `executes`.
pub fn float<S, R>(
    name: impl Into<String>,
) -> crate::builder::argument_builder::ArgumentBuilder<S, R, Float> {
    crate::builder::required_argument_builder::argument(name, Float::default())
}

super::numeric::impl_numeric_config!(Float, f32);
pub fn get_float<S, R>(context: &CommandContext<S, R>, name: &str) -> Option<f32> {
    context
        .argument(name)
        .unwrap()
        .downcast_ref::<f32>()
        .copied()
}

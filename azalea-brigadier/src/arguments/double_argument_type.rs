use std::sync::Arc;

use super::{ArgumentType, ParsedValue};
use crate::{
    context::CommandContext,
    errors::{BuiltInError, CommandSyntaxError},
    string_reader::StringReader,
};

/// A standalone f64 parser with optional inclusive bounds.
/// Use [`crate::parsers::double`] to construct it, or [`double`] for a named
/// node.
#[derive(Clone, Debug, Default)]
pub struct Double {
    minimum: Option<f64>,
    maximum: Option<f64>,
}

impl ArgumentType for Double {
    fn parse(&self, reader: &mut StringReader) -> Result<Arc<ParsedValue>, CommandSyntaxError> {
        let start = reader.cursor;
        let result = reader.read_double()?;
        if let Some(minimum) = self.minimum
            && result < minimum
        {
            reader.cursor = start;
            return Err(BuiltInError::DoubleTooSmall {
                found: result,
                min: minimum,
            }
            .create_with_context(reader));
        }
        if let Some(maximum) = self.maximum
            && result > maximum
        {
            reader.cursor = start;
            return Err(BuiltInError::DoubleTooBig {
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

/// Create a named f64 argument. Bounds may be configured before or after
/// common node methods such as `describe` and `executes`.
pub fn double<S, R>(
    name: impl Into<String>,
) -> crate::builder::argument_builder::ArgumentBuilder<S, R, Double> {
    crate::builder::required_argument_builder::argument(name, Double::default())
}

super::numeric::impl_numeric_config!(Double, f64);
pub fn get_double<S, R>(context: &CommandContext<S, R>, name: &str) -> Option<f64> {
    context
        .argument(name)
        .unwrap()
        .downcast_ref::<f64>()
        .copied()
}

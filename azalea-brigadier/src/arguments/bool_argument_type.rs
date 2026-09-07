use std::sync::Arc;

use super::{ArgumentType, ParsedValue};
use crate::{
    builder::CommandArgument,
    context::CommandContext,
    errors::CommandSyntaxError,
    string_reader::StringReader,
    suggestion::{Suggestions, SuggestionsBuilder},
};

/// A standalone boolean parser, including true/false completion.
#[derive(Clone, Copy, Debug, Default)]
pub struct Boolean;

impl ArgumentType for Boolean {
    fn parse(&self, reader: &mut StringReader) -> Result<Arc<ParsedValue>, CommandSyntaxError> {
        Ok(Arc::new(reader.read_boolean()?))
    }

    fn list_suggestions(&self, mut builder: SuggestionsBuilder) -> Suggestions {
        if "true".starts_with(builder.remaining_lowercase()) {
            builder = builder.suggest("true");
        }
        if "false".starts_with(builder.remaining_lowercase()) {
            builder = builder.suggest("false");
        }
        builder.build()
    }

    fn examples(&self) -> Vec<String> {
        vec!["true".to_owned(), "false".to_owned()]
    }
}

/// Create a named boolean argument with true/false completion.
pub fn boolean<S, R>(
    name: impl Into<String>,
) -> crate::builder::argument_builder::ArgumentBuilder<S, R, Boolean> {
    Boolean::arg(name)
}
pub fn get_bool<S, R>(context: &CommandContext<S, R>, name: &str) -> Option<bool> {
    context
        .argument(name)
        .expect("argument with name not found")
        .downcast_ref::<bool>()
        .cloned()
}

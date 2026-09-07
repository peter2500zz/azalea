use std::{
    fmt::{self, Debug},
    sync::Arc,
};

use super::argument_builder::{ArgumentBuilder, ArgumentBuilderType};
use crate::{
    arguments::{ArgumentType, ParsedValue},
    context::CommandContext,
    errors::CommandSyntaxError,
    string_reader::StringReader,
    suggestion::{SuggestionProvider, Suggestions, SuggestionsBuilder},
};

/// An argument node type.
///
/// This is the runtime representation; the concrete parser is retained by
/// its builder until the node is built.
pub struct Argument<S, R> {
    pub name: String,
    parser: Arc<dyn ArgumentType + Send + Sync>,
    pub(crate) custom_suggestions: Option<Arc<dyn SuggestionProvider<S, R> + Send + Sync>>,
}
impl<S, R> Argument<S, R> {
    pub fn new(
        name: impl Into<String>,
        parser: Arc<dyn ArgumentType + Send + Sync>,
        custom_suggestions: Option<Arc<dyn SuggestionProvider<S, R> + Send + Sync>>,
    ) -> Self {
        Self {
            name: name.into(),
            parser,
            custom_suggestions,
        }
    }

    pub fn parse(&self, reader: &mut StringReader) -> Result<Arc<ParsedValue>, CommandSyntaxError> {
        self.parser.parse(reader)
    }

    pub fn list_suggestions(
        &self,
        context: CommandContext<S, R>,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        if let Some(s) = &self.custom_suggestions {
            s.get_suggestions(context, builder)
        } else {
            self.parser.list_suggestions(builder)
        }
    }

    pub fn examples(&self) -> Vec<String> {
        self.parser.examples()
    }
}

impl<S, R> From<Argument<S, R>> for ArgumentBuilderType<S, R> {
    fn from(argument: Argument<S, R>) -> Self {
        Self::Argument(argument)
    }
}

impl<S, R> Debug for Argument<S, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Argument")
            .field("name", &self.name)
            // .field("parser", &self.parser)
            .finish()
    }
}

/// Create a named argument from a custom or standalone parser.
///
/// Built-in arguments have shorter constructors such as [`integer("count")`]
/// and [`word("player")`]. Standalone parsers live in [`crate::parsers`].
///
/// [`integer("count")`]: crate::prelude::integer
/// [`word("player")`]: crate::prelude::word
pub fn argument<S, R, P: ArgumentType + Send + Sync + 'static>(
    name: impl Into<String>,
    parser: P,
) -> ArgumentBuilder<S, R, P> {
    ArgumentBuilder::new(name.into(), parser)
}

impl<S, R> Clone for Argument<S, R> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            parser: self.parser.clone(),
            custom_suggestions: self.custom_suggestions.clone(),
        }
    }
}

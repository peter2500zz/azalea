use super::{Suggestions, SuggestionsBuilder};
use crate::context::CommandContext;

pub trait SuggestionProvider<S, R> {
    fn get_suggestions(
        &self,
        context: CommandContext<S, R>,
        builder: SuggestionsBuilder,
    ) -> Suggestions;
}

/// A closure is a provider, matching the functional interface upstream.
impl<S, R, F> SuggestionProvider<S, R> for F
where
    F: Fn(CommandContext<S, R>, SuggestionsBuilder) -> Suggestions,
{
    fn get_suggestions(
        &self,
        context: CommandContext<S, R>,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        self(context, builder)
    }
}

//! Type-directed construction of named command arguments.

use super::{ArgumentBuilder, CommandBuilder};
use crate::arguments::ArgumentType;

/// Choose the concrete fluent builder for a parser.
///
/// [`Self::arg`] names a default parser without claiming its `new` constructor.
/// [`Self::into_arg`] names an existing parser, including a non-Default parser
/// carrying application dependencies. Neither operation requires `Clone`.
///
/// A parser with no custom builder methods may select
/// `ArgumentBuilder<S, R, Self>` as its associated builder. A parser with
/// custom methods can select its own wrapper, implementing [`CommandBuilder`]
/// so all common setters continue returning that wrapper rather than its inner
/// builder.
///
/// # Custom fluent arguments
///
/// Parser implementations remain independent of names, sources, and handlers.
/// The wrapper below adds construction-time node configuration and exposes its
/// own methods even after common setters have been called:
///
/// ```
/// use std::sync::Arc;
///
/// use azalea_brigadier::{
///     arguments::{ArgumentType, ParsedValue},
///     prelude::*,
///     string_reader::StringReader,
/// };
///
/// #[derive(Default)]
/// struct PlayerParser {
///     case_insensitive: bool,
/// }
///
/// impl ArgumentType for PlayerParser {
///     fn parse(&self, reader: &mut StringReader) -> Result<Arc<ParsedValue>, CommandSyntaxError> {
///         let name = reader.read_string()?;
///         Ok(Arc::new(if self.case_insensitive {
///             name.to_lowercase()
///         } else {
///             name
///         }))
///     }
/// }
///
/// impl CommandArgument for PlayerParser {
///     type Builder<S, R> = PlayerArgument<S, R>;
/// }
///
/// struct PlayerArgument<S, R>(ArgumentBuilder<S, R, PlayerParser>);
///
/// impl<S, R> From<ArgumentBuilder<S, R, PlayerParser>> for PlayerArgument<S, R> {
///     fn from(builder: ArgumentBuilder<S, R, PlayerParser>) -> Self {
///         Self(builder)
///     }
/// }
///
/// impl<S, R> CommandBuilder for PlayerArgument<S, R> {
///     type Source = S;
///     type Output = R;
///     type Kind = PlayerParser;
///
///     fn as_builder(&self) -> &ArgumentBuilder<S, R, PlayerParser> {
///         &self.0
///     }
///
///     fn map_builder(
///         self,
///         update: impl FnOnce(
///             ArgumentBuilder<S, R, PlayerParser>,
///         ) -> ArgumentBuilder<S, R, PlayerParser>,
///     ) -> Self {
///         Self(update(self.0))
///     }
///
///     fn into_builder(self) -> ArgumentBuilder<S, R, PlayerParser> {
///         self.0
///     }
/// }
///
/// impl<S, R> PlayerArgument<S, R> {
///     fn case_insensitive(mut self) -> Self {
///         self.0.parser_mut().case_insensitive = true;
///         self
///     }
/// }
///
/// let mut dispatcher = CommandDispatcher::<()>::new();
/// dispatcher.register(
///     literal("kick").then(
///         PlayerParser::arg("player")
///             .describe("Target player")
///             .executes(|ctx| -> CommandResult {
///                 assert_eq!(get_string(ctx, "player").as_deref(), Some("bob"));
///                 Ok(1)
///             })
///             .case_insensitive(),
///     ),
/// );
/// assert_eq!(dispatcher.execute("kick BOB", ()).unwrap(), 1);
/// ```
///
/// Merely implementing a parsing trait does not forward inherent methods to a
/// different type. The dedicated wrapper above is the type on which both the
/// custom methods and the common fluent operations are available. Import
/// [`CommandArgument`] and [`CommandBuilder`] (or the prelude) to use the trait
/// methods. If a parser already has an inherent `arg`, use
/// `<Parser as CommandArgument>::arg(...)` to disambiguate.
///
/// For a parser requiring an application registry, construct it explicitly and
/// call `parser.into_arg("player")`. `into_arg` uses the same associated
/// builder as `arg`, and keeps the supplied parser intact. This does not
/// require Default. Stateful parsers must opt into a valid default before `arg`
/// is available:
///
/// ```compile_fail
/// use azalea_brigadier::{parsers::StringArgument, prelude::*};
/// // The string parser deliberately requires a tokenization policy.
/// let node = StringArgument::arg::<(), i32>("text");
/// ```
///
/// ```
/// use azalea_brigadier::{parsers::StringArgument, prelude::*};
/// let node = StringArgument::GreedyPhrase.into_arg::<(), i32>("text");
/// assert_eq!(node.build().name(), "text");
/// ```
///
/// # Registration errors
///
/// Implement both [`ArgumentType`] (parsing) and this trait (the named builder
/// factory). Import this trait to call `Parser::arg` or `parser.into_arg`.
/// Import [`CommandBuilder`] for the common methods on a custom wrapper, or use
/// the prelude for both. A parser's inherent methods are not automatically
/// forwarded to its builder; implement fluent setters on the chosen wrapper.
///
/// If `arg` reports a missing `Default` implementation, do not invent a default
/// for application state: construct the parser and use `into_arg` instead.
/// If an inherent `arg` shadows the trait method, use
/// `<Parser as CommandArgument>::arg(...)`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not provide a named command argument builder",
    note = "Implement ArgumentType for parsing and CommandArgument to select the builder. Import CommandArgument (or the prelude) to use ::arg(name) or .into_arg(name)."
)]
pub trait CommandArgument: ArgumentType + Sized + Send + Sync + 'static {
    /// The concrete builder returned by both naming operations.
    type Builder<S, R>: CommandBuilder<Source = S, Output = R, Kind = Self>
        + From<ArgumentBuilder<S, R, Self>>;

    /// Construct a named argument with the parser's default configuration.
    ///
    /// Only this convenience method requires `Default`; use [`Self::into_arg`]
    /// when constructing a parser requires explicit dependencies.
    fn arg<S, R>(name: impl Into<String>) -> Self::Builder<S, R>
    where
        Self: Default,
    {
        Self::default().into_arg(name)
    }

    /// Move a configured parser into its concrete named builder.
    /// No parser configuration is reset or copied.
    fn into_arg<S, R>(self, name: impl Into<String>) -> Self::Builder<S, R> {
        ArgumentBuilder::new(name.into(), self).into()
    }
}

macro_rules! builtin_arguments {
    ($($parser:ty),+ $(,)?) => {
        $(impl CommandArgument for $parser {
            type Builder<S, R> = ArgumentBuilder<S, R, Self>;
        })+
    };
}

builtin_arguments!(
    crate::parsers::Boolean,
    crate::parsers::Integer,
    crate::parsers::Long,
    crate::parsers::Float,
    crate::parsers::Double,
    crate::parsers::StringArgument,
);

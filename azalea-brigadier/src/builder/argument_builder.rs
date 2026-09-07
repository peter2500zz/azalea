#[cfg(feature = "async")]
use std::future::Future;
use std::{
    fmt::{self, Debug},
    sync::Arc,
};

use parking_lot::RwLock;

use super::{
    IntoCommandNode,
    kind::NodeKind,
    literal_argument_builder::{Literal, LiteralKind},
    required_argument_builder::Argument,
};
use crate::{
    arguments::ArgumentType,
    context::CommandContext,
    errors::{BoxCommandError, CommandError},
    modifier::RedirectModifier,
    suggestion::SuggestionProvider,
    tree::{Command, CommandNode},
};

#[derive(Debug)]
pub enum ArgumentBuilderType<S, R> {
    Literal(Literal),
    Argument(Argument<S, R>),
}
impl<S, R> Clone for ArgumentBuilderType<S, R> {
    fn clone(&self) -> Self {
        match self {
            ArgumentBuilderType::Literal(literal) => ArgumentBuilderType::Literal(literal.clone()),
            ArgumentBuilderType::Argument(argument) => {
                ArgumentBuilderType::Argument(argument.clone())
            }
        }
    }
}

/// A node under construction, retaining its concrete parser type `P`.
///
/// Common setters return `Self`, so parser-specific configuration remains
/// available after descriptions, requirements, handlers, or children are added.
/// Parsers are erased only when the node is built or attached to a parent.
/// The default kind is a literal; custom arguments infer `P` from their parser.
pub struct ArgumentBuilder<S, R = i32, P = LiteralKind> {
    name: String,
    parser: P,
    children: Vec<CommandNode<S, R>>,
    suggestions: Option<Arc<dyn SuggestionProvider<S, R> + Send + Sync>>,

    description: Option<String>,
    command: Command<S, R>,
    #[cfg(feature = "async")]
    async_command: crate::tree::AsyncCommand<S, R>,
    requirement: Arc<dyn Fn(&S) -> bool + Send + Sync>,
    target: Option<Arc<RwLock<CommandNode<S, R>>>>,

    forks: bool,
    modifier: Option<Arc<RedirectModifier<S, R>>>,
}

/// A node that isn't yet built.
impl<S, R, P> ArgumentBuilder<S, R, P> {
    pub(crate) fn new(name: String, parser: P) -> Self {
        Self {
            name,
            parser,
            children: Vec::new(),
            suggestions: None,
            description: None,
            command: None,
            #[cfg(feature = "async")]
            async_command: None,
            requirement: Arc::new(|_| true),
            forks: false,
            modifier: None,
            target: None,
        }
    }

    /// Continue building this node with a child node.
    ///
    /// ```
    /// # use azalea_brigadier::prelude::*;
    /// # let mut subject = CommandDispatcher::<()>::new();
    /// literal("foo")
    ///     .then(literal("bar").executes(|_: &CommandContext<()>| -> CommandResult { Ok(42) }))
    /// # ;
    /// ```
    pub fn then(self, argument: impl IntoCommandNode<S, R>) -> Self {
        self.then_built(argument.into_node())
    }

    /// Add an already built child node to this node.
    ///
    /// You should usually use [`Self::then`] instead.
    pub fn then_built(mut self, argument: CommandNode<S, R>) -> Self {
        self.children.push(argument);
        self
    }

    /// Set the command to be executed when this node is reached.
    ///
    /// If this is not present on a node, it is not a valid command.
    ///
    /// ```
    /// # use azalea_brigadier::prelude::*;
    /// # let mut subject = CommandDispatcher::<()>::new();
    /// # subject.register(
    /// literal("foo").executes(|_: &CommandContext<()>| -> CommandResult { Ok(42) })
    /// # );
    /// ```
    pub fn executes<F, E>(mut self, f: F) -> Self
    where
        F: Fn(&CommandContext<S, R>) -> Result<R, E> + Send + Sync + 'static,
        E: Into<BoxCommandError> + 'static,
    {
        self.command = Some(Arc::new(move |ctx: &CommandContext<S, R>| {
            f(ctx).map_err(CommandError::from_execution)
        }));
        #[cfg(feature = "async")]
        {
            self.async_command = None;
        }
        self
    }

    /// Set an asynchronous command to be executed when this node is reached.
    ///
    /// The handler receives an owned [`Arc`] so a named `async fn` can be
    /// registered directly and keep its context across `.await` points. The
    /// returned future must be `Send + 'static` so executors may move it
    /// between worker threads.
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use azalea_brigadier::{builder::argument_builder::ArgumentBuilder, prelude::*};
    /// async fn later(ctx: Arc<CommandContext<()>>) -> CommandResult {
    ///     drop(ctx);
    ///     Ok(42)
    /// }
    ///
    /// let command: ArgumentBuilder<()> = literal("later").executes_async(later);
    /// # let _ = command;
    /// ```
    ///
    /// A future carrying thread-local state across an await is rejected:
    ///
    /// ```compile_fail
    /// # use std::{rc::Rc, sync::Arc};
    /// # use azalea_brigadier::{builder::argument_builder::ArgumentBuilder, prelude::*};
    /// let command: ArgumentBuilder<()> = literal("bad").executes_async(|_: Arc<CommandContext<()>>| {
    ///     let local = Rc::new(());
    ///     async move {
    ///         std::future::ready(()).await;
    ///         drop(local);
    ///         Ok::<_, BoxCommandError>(1)
    ///     }
    /// });
    /// # let _ = command;
    /// ```
    #[cfg(feature = "async")]
    pub fn executes_async<F, Fut, E>(mut self, f: F) -> Self
    where
        F: Fn(Arc<CommandContext<S, R>>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<R, E>> + Send + 'static,
        E: Into<BoxCommandError> + 'static,
    {
        self.command = None;
        self.async_command = Some(Arc::new(move |ctx: Arc<CommandContext<S, R>>| {
            let future = f(ctx);
            Box::pin(async move { future.await.map_err(CommandError::from_execution) })
        }));
        self
    }

    /// Say what this node does, in a few words.
    ///
    /// The description rides along with the suggestion that completes this
    /// node, as its tooltip, so a completion menu can explain each candidate
    /// without keeping a second table keyed by name — and two nodes sharing a
    /// name (`proxy on` and `log on`) still get to say different things.
    ///
    /// ```
    /// # use azalea_brigadier::prelude::*;
    /// # let mut subject = CommandDispatcher::<()>::new();
    /// # subject.register(
    /// literal("foo")
    ///     .describe("does the foo thing")
    ///     .executes(|_: &CommandContext<()>| -> CommandResult { Ok(42) })
    /// # );
    /// ```
    pub fn describe(mut self, description: &str) -> Self {
        self.description = Some(description.to_owned());
        self
    }

    /// Set the requirement for this node to be considered.
    ///
    /// If this is not present on a node, it is considered to always pass.
    ///
    /// ```
    /// # use azalea_brigadier::prelude::*;
    /// # use std::sync::Arc;
    /// # pub struct CommandSource {
    /// #     pub opped: bool,
    /// # }
    /// # let mut subject = CommandDispatcher::<CommandSource>::new();
    /// # subject.register(
    /// literal("foo").requires(|s: &CommandSource| s.opped)
    /// // ...
    ///     # .executes(|_: &CommandContext<CommandSource>| -> CommandResult { Ok(42) })
    /// # );
    /// ```
    pub fn requires<F>(mut self, requirement: F) -> Self
    where
        F: Fn(&S) -> bool + Send + Sync + 'static,
    {
        self.requirement = Arc::new(requirement);
        self
    }

    pub fn redirect(self, target: Arc<RwLock<CommandNode<S, R>>>) -> Self {
        self.forward(target, None, false)
    }

    pub fn fork(
        self,
        target: Arc<RwLock<CommandNode<S, R>>>,
        modifier: Arc<RedirectModifier<S, R>>,
    ) -> Self {
        self.forward(target, Some(modifier), true)
    }

    pub fn forward(
        mut self,
        target: Arc<RwLock<CommandNode<S, R>>>,
        modifier: Option<Arc<RedirectModifier<S, R>>>,
        fork: bool,
    ) -> Self {
        if !self.children.is_empty() {
            panic!("Cannot forward a node with children");
        }
        self.target = Some(target);
        self.modifier = modifier;
        self.forks = fork;
        self
    }

    /// Children already attached to this builder, in insertion order.
    /// Nodes with matching names are merged when this builder is built.
    pub fn children(&self) -> &[CommandNode<S, R>] {
        &self.children
    }

    /// Manually build this node into a [`CommandNode`]. You probably don't need
    /// to do this yourself.
    pub fn build(self) -> CommandNode<S, R>
    where
        P: NodeKind<S, R>,
    {
        let mut result = CommandNode {
            value: self.parser.into_value(self.name, self.suggestions),
            description: self.description,
            command: self.command,
            #[cfg(feature = "async")]
            async_command: self.async_command,
            requirement: self.requirement,
            redirect: self.target,
            modifier: self.modifier,
            forks: self.forks,
            arguments: Default::default(),
            children: Default::default(),
            literals: Default::default(),
        };

        for argument in self.children {
            result.add_child(&Arc::new(RwLock::new(argument)));
        }

        result
    }
}

impl<S, R, P> Debug for ArgumentBuilder<S, R, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArgumentBuilder")
            .field("name", &self.name)
            .field("parser", &std::any::type_name::<P>())
            .field("children", &self.children)
            // .field("command", &self.command)
            // .field("requirement", &self.requirement)
            .field("target", &self.target)
            .field("forks", &self.forks)
            // .field("modifier", &self.modifier)
            .finish()
    }
}
impl<S, R, P: Clone> Clone for ArgumentBuilder<S, R, P> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            parser: self.parser.clone(),
            children: self.children.clone(),
            suggestions: self.suggestions.clone(),
            description: self.description.clone(),
            command: self.command.clone(),
            #[cfg(feature = "async")]
            async_command: self.async_command.clone(),
            requirement: self.requirement.clone(),
            target: self.target.clone(),
            forks: self.forks,
            modifier: self.modifier.clone(),
        }
    }
}

impl<S, R, P: ArgumentType + Send + Sync + 'static> ArgumentBuilder<S, R, P> {
    /// Borrow this argument's concrete parser configuration.
    pub fn parser(&self) -> &P {
        &self.parser
    }

    /// Configure this argument's parser from a custom builder's fluent method.
    /// Common node metadata and the concrete parser type are preserved.
    pub fn parser_mut(&mut self) -> &mut P {
        &mut self.parser
    }

    /// Decide what to suggest for this argument, instead of asking its
    /// [`ArgumentType`].
    ///
    /// This is where suggestions that depend on the source belong — the names
    /// currently in a registry, the files in a directory, whatever the caller
    /// is allowed to see. A closure is a provider, so the usual form is:
    ///
    /// ```
    /// # use azalea_brigadier::prelude::*;
    /// # use azalea_brigadier::{context::CommandContext, suggestion::SuggestionsBuilder};
    /// # let mut subject = CommandDispatcher::<()>::new();
    /// # subject.register(
    /// word("colour")
    ///     .suggests(|_ctx: CommandContext<()>, builder: SuggestionsBuilder| {
    ///         builder.suggest("red").suggest("green").build()
    ///     })
    ///     .executes(|_: &CommandContext<()>| -> CommandResult { Ok(42) })
    /// # );
    /// ```
    ///
    /// This method exists only on argument builders, not literals.
    ///
    /// ```compile_fail
    /// use azalea_brigadier::{prelude::*, suggestion::SuggestionsBuilder};
    /// literal::<(), i32>("paint").suggests(
    ///     |_: CommandContext<()>, builder: SuggestionsBuilder| builder.build()
    /// );
    /// ```
    ///
    /// [`ArgumentType`]: crate::arguments::ArgumentType
    pub fn suggests(
        mut self,
        provider: impl SuggestionProvider<S, R> + Send + Sync + 'static,
    ) -> Self {
        self.suggestions = Some(Arc::new(provider));
        self
    }

    /// Configure a custom parser by value without erasing its concrete type.
    ///
    /// The closure runs once while constructing the command, not on execution.
    ///
    /// ```
    /// use azalea_brigadier::{parsers, prelude::*};
    /// let mut dispatcher = CommandDispatcher::<()>::new();
    /// dispatcher.register(
    ///     parsers::integer()
    ///         .into_arg("count")
    ///         .configure_parser(|parser| parser.range(1..=10))
    ///         .executes(|ctx| -> CommandResult { Ok(get_integer(ctx, "count").unwrap()) }),
    /// );
    /// assert_eq!(dispatcher.execute("3", ()).unwrap(), 3);
    /// ```
    pub fn configure_parser(mut self, configure: impl FnOnce(P) -> P) -> Self {
        self.parser = configure(self.parser);
        self
    }
}

impl<S, R, P: NodeKind<S, R>> From<ArgumentBuilder<S, R, P>> for CommandNode<S, R> {
    fn from(builder: ArgumentBuilder<S, R, P>) -> Self {
        builder.build()
    }
}

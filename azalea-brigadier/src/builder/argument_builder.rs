use std::{
    fmt::{self, Debug},
    sync::Arc,
};

use parking_lot::RwLock;

use super::{literal_argument_builder::Literal, required_argument_builder::Argument};
use crate::{
    context::CommandContext,
    errors::CommandSyntaxError,
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

/// A node that hasn't yet been built.
pub struct ArgumentBuilder<S, R = i32> {
    arguments: CommandNode<S, R>,

    description: Option<String>,
    command: Command<S, R>,
    requirement: Arc<dyn Fn(&S) -> bool + Send + Sync>,
    target: Option<Arc<RwLock<CommandNode<S, R>>>>,

    forks: bool,
    modifier: Option<Arc<RedirectModifier<S, R>>>,
}

/// A node that isn't yet built.
impl<S, R> ArgumentBuilder<S, R> {
    pub fn new(value: ArgumentBuilderType<S, R>) -> Self {
        Self {
            arguments: CommandNode {
                value,
                ..Default::default()
            },
            description: None,
            command: None,
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
    /// literal("foo").then(literal("bar").executes(|ctx: &CommandContext<()>| 42))
    /// # ;
    /// ```
    pub fn then(self, argument: ArgumentBuilder<S, R>) -> Self {
        self.then_built(argument.build())
    }

    /// Add an already built child node to this node.
    ///
    /// You should usually use [`Self::then`] instead.
    pub fn then_built(mut self, argument: CommandNode<S, R>) -> Self {
        self.arguments.add_child(&Arc::new(RwLock::new(argument)));
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
    /// literal("foo").executes(|ctx: &CommandContext<()>| 42)
    /// # );
    /// ```
    pub fn executes<F>(mut self, f: F) -> Self
    where
        F: Fn(&CommandContext<S, R>) -> R + Send + Sync + 'static,
    {
        self.command = Some(Arc::new(move |ctx: &CommandContext<S, R>| Ok(f(ctx))));
        self
    }

    /// Same as [`Self::executes`] but returns a `Result<i32,
    /// CommandSyntaxError>`.
    pub fn executes_result<F>(mut self, f: F) -> Self
    where
        F: Fn(&CommandContext<S, R>) -> Result<R, CommandSyntaxError> + Send + Sync + 'static,
    {
        self.command = Some(Arc::new(f));
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
    ///     .executes(|ctx: &CommandContext<()>| 42)
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
    /// literal("foo")
    ///     .requires(|s: &CommandSource| s.opped)
    ///     // ...
    ///     # .executes(|ctx: &CommandContext<CommandSource>| 42)
    /// # );
    pub fn requires<F>(mut self, requirement: F) -> Self
    where
        F: Fn(&S) -> bool + Send + Sync + 'static,
    {
        self.requirement = Arc::new(requirement);
        self
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
    /// argument("colour", word())
    ///     .suggests(|_ctx: CommandContext<()>, builder: SuggestionsBuilder| {
    ///         builder.suggest("red").suggest("green").build()
    ///     })
    ///     .executes(|ctx: &CommandContext<()>| 42)
    /// # );
    /// ```
    ///
    /// # Panics
    ///
    /// If this node is a literal. Literals suggest themselves and have nothing
    /// to ask a provider about; Mojang's brigadier puts this method on the
    /// required-argument builder alone, where the type system rules it out.
    ///
    /// [`ArgumentType`]: crate::arguments::ArgumentType
    pub fn suggests(
        mut self,
        provider: impl SuggestionProvider<S, R> + Send + Sync + 'static,
    ) -> Self {
        let ArgumentBuilderType::Argument(argument) = &mut self.arguments.value else {
            panic!("ArgumentBuilder::suggests() called on a literal node");
        };
        argument.custom_suggestions = Some(Arc::new(provider));
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
        if !self.arguments.children.is_empty() {
            panic!("Cannot forward a node with children");
        }
        self.target = Some(target);
        self.modifier = modifier;
        self.forks = fork;
        self
    }

    pub fn arguments(&self) -> &CommandNode<S, R> {
        &self.arguments
    }

    /// Manually build this node into a [`CommandNode`]. You probably don't need
    /// to do this yourself.
    pub fn build(self) -> CommandNode<S, R> {
        let mut result = CommandNode {
            value: self.arguments.value,
            description: self.description,
            command: self.command,
            requirement: self.requirement,
            redirect: self.target,
            modifier: self.modifier,
            forks: self.forks,
            arguments: Default::default(),
            children: Default::default(),
            literals: Default::default(),
        };

        for argument in self.arguments.children.values() {
            result.add_child(argument);
        }

        result
    }
}

impl<S, R> Debug for ArgumentBuilder<S, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArgumentBuilder")
            .field("arguments", &self.arguments)
            // .field("command", &self.command)
            // .field("requirement", &self.requirement)
            .field("target", &self.target)
            .field("forks", &self.forks)
            // .field("modifier", &self.modifier)
            .finish()
    }
}
impl<S, R> Clone for ArgumentBuilder<S, R> {
    fn clone(&self) -> Self {
        Self {
            arguments: self.arguments.clone(),
            description: self.description.clone(),
            command: self.command.clone(),
            requirement: self.requirement.clone(),
            target: self.target.clone(),
            forks: self.forks,
            modifier: self.modifier.clone(),
        }
    }
}

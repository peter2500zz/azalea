//! Common fluent operations for concrete, user-defined command builders.

#[cfg(feature = "async")]
use std::future::Future;
use std::sync::Arc;

use parking_lot::RwLock;

use super::{argument_builder::ArgumentBuilder, kind::NodeKind};
use crate::{
    arguments::ArgumentType, context::CommandContext, errors::BoxCommandError,
    modifier::RedirectModifier, suggestion::SuggestionProvider, tree::CommandNode,
};

/// A builder whose common operations preserve its concrete `Self` type.
///
/// Custom builders normally contain an [`ArgumentBuilder`]. Implement the
/// three structural operations below; descriptions, handlers, children, and
/// routing then work without losing the custom builder's inherent methods.
/// Neither `Deref` nor cloning or temporarily emptying the builder is needed.
///
/// See [`super::CommandArgument`] for a complete custom-argument example.
pub trait CommandBuilder: Sized {
    type Source;
    type Output;
    type Kind: NodeKind<Self::Source, Self::Output>;

    /// Borrow the underlying node configuration.
    fn as_builder(&self) -> &ArgumentBuilder<Self::Source, Self::Output, Self::Kind>;

    /// Transform the underlying configuration, preserving any additional
    /// state stored by this concrete builder. Invoke `update` exactly once.
    fn map_builder(
        self,
        update: impl FnOnce(
            ArgumentBuilder<Self::Source, Self::Output, Self::Kind>,
        ) -> ArgumentBuilder<Self::Source, Self::Output, Self::Kind>,
    ) -> Self;

    /// Consume this builder at the runtime-node construction boundary.
    fn into_builder(self) -> ArgumentBuilder<Self::Source, Self::Output, Self::Kind>;

    /// Attach a built node or another concrete builder. See
    /// [`ArgumentBuilder::then`].
    fn then(self, child: impl IntoCommandNode<Self::Source, Self::Output>) -> Self {
        self.map_builder(|builder| builder.then(child))
    }

    /// Attach an already built child. Equivalent to [`Self::then`].
    fn then_built(self, child: CommandNode<Self::Source, Self::Output>) -> Self {
        self.then(child)
    }

    /// Set the synchronous handler without changing this builder's type.
    /// See [`ArgumentBuilder::executes`].
    fn executes<F, E>(self, handler: F) -> Self
    where
        F: Fn(&CommandContext<Self::Source, Self::Output>) -> Result<Self::Output, E>
            + Send
            + Sync
            + 'static,
        E: Into<BoxCommandError> + 'static,
    {
        self.map_builder(|builder| builder.executes(handler))
    }

    /// Set the asynchronous handler without changing this builder's type.
    /// Named `async fn` handlers and async closures are both supported.
    /// See [`ArgumentBuilder::executes_async`].
    #[cfg(feature = "async")]
    fn executes_async<F, Fut, E>(self, handler: F) -> Self
    where
        F: Fn(Arc<CommandContext<Self::Source, Self::Output>>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Self::Output, E>> + Send + 'static,
        E: Into<BoxCommandError> + 'static,
    {
        self.map_builder(|builder| builder.executes_async(handler))
    }

    /// Set the node's description. See [`ArgumentBuilder::describe`].
    fn describe(self, description: &str) -> Self {
        self.map_builder(|builder| builder.describe(description))
    }

    /// Set its permission predicate. See [`ArgumentBuilder::requires`].
    fn requires<F>(self, requirement: F) -> Self
    where
        F: Fn(&Self::Source) -> bool + Send + Sync + 'static,
    {
        self.map_builder(|builder| builder.requires(requirement))
    }

    /// Set argument suggestions, retaining the custom builder's type.
    /// This operation is unavailable for literals.
    fn suggests(
        self,
        provider: impl SuggestionProvider<Self::Source, Self::Output> + Send + Sync + 'static,
    ) -> Self
    where
        Self::Kind: ArgumentType + Send + Sync + 'static,
    {
        self.map_builder(|builder| builder.suggests(provider))
    }

    /// Redirect execution. See [`ArgumentBuilder::redirect`].
    fn redirect(self, target: Arc<RwLock<CommandNode<Self::Source, Self::Output>>>) -> Self {
        self.map_builder(|builder| builder.redirect(target))
    }

    /// Fork execution. See [`ArgumentBuilder::fork`].
    fn fork(
        self,
        target: Arc<RwLock<CommandNode<Self::Source, Self::Output>>>,
        modifier: Arc<RedirectModifier<Self::Source, Self::Output>>,
    ) -> Self {
        self.map_builder(|builder| builder.fork(target, modifier))
    }

    /// Configure forwarding. See [`ArgumentBuilder::forward`].
    fn forward(
        self,
        target: Arc<RwLock<CommandNode<Self::Source, Self::Output>>>,
        modifier: Option<Arc<RedirectModifier<Self::Source, Self::Output>>>,
        fork: bool,
    ) -> Self {
        self.map_builder(|builder| builder.forward(target, modifier, fork))
    }

    /// Inspect the attached children before building and merging them.
    fn children(&self) -> &[CommandNode<Self::Source, Self::Output>] {
        self.as_builder().children()
    }

    /// Erase the concrete builder and parser types into a runtime node.
    fn build(self) -> CommandNode<Self::Source, Self::Output> {
        self.into_builder().build()
    }
}

impl<S, R, P: NodeKind<S, R>> CommandBuilder for ArgumentBuilder<S, R, P> {
    type Source = S;
    type Output = R;
    type Kind = P;

    fn as_builder(&self) -> &Self {
        self
    }

    fn map_builder(self, update: impl FnOnce(Self) -> Self) -> Self {
        update(self)
    }

    fn into_builder(self) -> Self {
        self
    }
}

/// Convert a registration input into a runtime node.
///
/// Every [`CommandBuilder`] implements this automatically, including builders
/// defined in another crate. Built nodes are accepted directly. Other command
/// wrappers may implement this trait themselves; an existing `From<Wrapper>`
/// conversion can be reused by returning `self.into()` from `into_node`.
pub trait IntoCommandNode<S, R = i32> {
    fn into_node(self) -> CommandNode<S, R>;
}

impl<S, R, B: CommandBuilder<Source = S, Output = R>> IntoCommandNode<S, R> for B {
    fn into_node(self) -> CommandNode<S, R> {
        self.build()
    }
}

impl<S, R> IntoCommandNode<S, R> for CommandNode<S, R> {
    fn into_node(self) -> CommandNode<S, R> {
        self
    }
}

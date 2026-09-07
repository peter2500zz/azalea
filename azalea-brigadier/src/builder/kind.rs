use std::sync::Arc;

use super::{
    argument_builder::ArgumentBuilderType,
    literal_argument_builder::{Literal, LiteralKind},
    required_argument_builder::Argument,
};
use crate::{arguments::ArgumentType, suggestion::SuggestionProvider};

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::LiteralKind {}
    impl<P: super::ArgumentType + Send + Sync + 'static> Sealed for P {}
}

/// A construction-time node kind convertible to the runtime representation.
///
/// This trait is sealed. It is implemented for literals and automatically for
/// every parser implementing [ArgumentType] + Send + Sync + 'static. Custom
/// parsers implement ArgumentType, not this trait.
pub trait NodeKind<S, R>: sealed::Sealed {
    #[doc(hidden)]
    fn into_value(
        self,
        name: String,
        suggestions: Option<Arc<dyn SuggestionProvider<S, R> + Send + Sync>>,
    ) -> ArgumentBuilderType<S, R>;
}

impl<S, R> NodeKind<S, R> for LiteralKind {
    fn into_value(
        self,
        name: String,
        _suggestions: Option<Arc<dyn SuggestionProvider<S, R> + Send + Sync>>,
    ) -> ArgumentBuilderType<S, R> {
        ArgumentBuilderType::Literal(Literal { value: name })
    }
}

impl<S, R, P: ArgumentType + Send + Sync + 'static> NodeKind<S, R> for P {
    fn into_value(
        self,
        name: String,
        suggestions: Option<Arc<dyn SuggestionProvider<S, R> + Send + Sync>>,
    ) -> ArgumentBuilderType<S, R> {
        ArgumentBuilderType::Argument(Argument::new(name, Arc::new(self), suggestions))
    }
}

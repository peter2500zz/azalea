use super::argument_builder::{ArgumentBuilder, ArgumentBuilderType};

#[derive(Clone, Debug, Default)]
pub struct Literal {
    pub value: String,
}
impl Literal {
    pub fn new(value: &str) -> Self {
        Self {
            value: value.to_owned(),
        }
    }
}

impl<S, R> From<Literal> for ArgumentBuilderType<S, R> {
    fn from(literal: Literal) -> Self {
        Self::Literal(literal)
    }
}

/// The construction-time kind of a literal node.
///
/// Literals have no parser, so argument-only methods such as `suggests` and
/// numeric bounds are unavailable on their builders.
#[derive(Clone, Copy, Debug, Default)]
pub struct LiteralKind;

/// Create a literal node with the given name.
pub fn literal<S, R>(value: impl Into<String>) -> ArgumentBuilder<S, R> {
    ArgumentBuilder::new(value.into(), LiteralKind)
}

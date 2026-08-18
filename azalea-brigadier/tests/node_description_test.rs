//! Descriptions set with [`ArgumentBuilder::describe`] and where they surface.

use azalea_brigadier::{prelude::*, suggestion::Suggestion};

fn suggestions(subject: &CommandDispatcher<()>, input: &str) -> Vec<Suggestion> {
    CommandDispatcher::get_completion_suggestions(subject.parse(input.into(), ()))
        .list()
        .to_vec()
}

fn tooltip_of(subject: &CommandDispatcher<()>, input: &str, text: &str) -> Option<String> {
    suggestions(subject, input)
        .into_iter()
        .find(|suggestion| suggestion.text() == text)
        .expect("the suggestion should be offered")
        .tooltip
}

/// The point of the whole thing: a literal's description reaches whoever is
/// drawing the completion menu, without a second table keyed by name.
#[test]
fn a_literal_offers_its_description_as_a_tooltip() {
    let mut subject = CommandDispatcher::<()>::new();
    subject.register(
        literal("foo")
            .describe("does the foo thing")
            .executes(|_: &CommandContext<()>| 1),
    );

    assert_eq!(
        tooltip_of(&subject, "f", "foo").as_deref(),
        Some("does the foo thing")
    );
}

/// No description, no tooltip — nothing is invented.
#[test]
fn an_undescribed_literal_has_no_tooltip() {
    let mut subject = CommandDispatcher::<()>::new();
    subject.register(literal("foo").executes(|_: &CommandContext<()>| 1));

    assert_eq!(tooltip_of(&subject, "f", "foo"), None);
}

/// Nodes are keyed by name only within their parent, so two subcommands
/// sharing a name each keep their own description. This is what a name-keyed
/// side table can't do.
#[test]
fn nodes_sharing_a_name_describe_themselves_separately() {
    let mut subject = CommandDispatcher::<()>::new();
    subject.register(
        literal("proxy").then(
            literal("on")
                .describe("route through the upstream proxy")
                .executes(|_: &CommandContext<()>| 1),
        ),
    );
    subject.register(
        literal("log").then(
            literal("on")
                .describe("turn on verbose logging")
                .executes(|_: &CommandContext<()>| 1),
        ),
    );

    assert_eq!(
        tooltip_of(&subject, "proxy ", "on").as_deref(),
        Some("route through the upstream proxy")
    );
    assert_eq!(
        tooltip_of(&subject, "log ", "on").as_deref(),
        Some("turn on verbose logging")
    );
}

/// Registering a second branch of a command merges onto the existing node —
/// which must not throw away what the first registration said about it.
#[test]
fn merging_a_branch_keeps_the_description() {
    let mut subject = CommandDispatcher::<()>::new();
    subject.register(
        literal("proxy")
            .describe("upstream proxy settings")
            .then(literal("on").executes(|_: &CommandContext<()>| 1)),
    );
    // Same command, registered again for another subcommand, this time with
    // nothing to say about `proxy` itself.
    subject.register(literal("proxy").then(literal("off").executes(|_: &CommandContext<()>| 1)));

    assert_eq!(
        tooltip_of(&subject, "pro", "proxy").as_deref(),
        Some("upstream proxy settings")
    );
}

/// A description is a property of the node, so it survives being built and is
/// readable straight off the tree — which is how anything that isn't a
/// suggestion (usage listings, argument hints) gets at it.
#[test]
fn the_description_is_readable_from_the_tree() {
    let built = literal::<(), i32>("foo")
        .describe("does the foo thing")
        .build();
    assert_eq!(built.description.as_deref(), Some("does the foo thing"));

    let argument = argument::<(), i32>("bar", integer())
        .describe("how many times")
        .build();
    assert_eq!(argument.description.as_deref(), Some("how many times"));
}

//! Commands and completions over input where byte and character offsets differ.

use std::convert::Infallible;

use azalea_brigadier::{prelude::*, string_reader::StringReader};

#[derive(Debug, Clone, PartialEq)]
struct CommandSource {}

fn success(value: i32) -> Result<i32, Infallible> {
    Ok(value)
}

/// A dispatcher with a single `echo <value>` taking the rest of the line.
fn greedy_dispatcher() -> CommandDispatcher<CommandSource> {
    let mut subject = CommandDispatcher::new();
    subject.register(literal("echo").then(greedy_string("value").executes(
        |ctx: &CommandContext<CommandSource>| {
            success(i32::from(get_string(ctx, "value").is_some()))
        },
    )));
    subject
}

/// A dispatcher with `say <value>` taking a single word, and `quote <value>`
/// taking an optionally quoted phrase.
fn word_dispatcher() -> CommandDispatcher<CommandSource> {
    let mut subject = CommandDispatcher::new();
    subject.register(literal("say").then(word("value").executes(
        |ctx: &CommandContext<CommandSource>| {
            success(i32::from(get_string(ctx, "value").is_some()))
        },
    )));
    subject.register(literal("quote").then(string("value").executes(
        |ctx: &CommandContext<CommandSource>| {
            success(i32::from(
                get_string(ctx, "value").as_deref() == Some("你好 世界"),
            ))
        },
    )));
    subject
}

#[test]
fn executes_with_non_ascii_greedy_argument() {
    let subject = greedy_dispatcher();
    assert_eq!(
        subject.execute("echo 你好 世界", CommandSource {}).unwrap(),
        1
    );
    assert_eq!(subject.execute("echo 🎮", CommandSource {}).unwrap(), 1);
}

#[test]
fn rejects_non_ascii_unquoted_word_argument() {
    // Unquoted words are ASCII-only, as in vanilla Brigadier, so this is a
    // parse error. It must not be a panic.
    let subject = word_dispatcher();
    assert!(subject.execute("say 你好", CommandSource {}).is_err());
    assert_eq!(subject.execute("say hello", CommandSource {}).unwrap(), 1);
}

#[test]
fn executes_with_non_ascii_quoted_argument() {
    let subject = word_dispatcher();
    assert_eq!(
        subject
            .execute("quote \"你好 世界\"", CommandSource {})
            .unwrap(),
        1
    );
}

#[test]
fn reports_unknown_command_for_non_ascii_input() {
    let subject = greedy_dispatcher();
    // Must surface as a parse error rather than a panic.
    assert!(subject.execute("你好", CommandSource {}).is_err());
    assert!(subject.execute("échec", CommandSource {}).is_err());
}

#[test]
fn suggests_over_non_ascii_input() {
    let subject = greedy_dispatcher();

    let parse = subject.parse(StringReader::from("你好"), CommandSource {});
    assert!(CommandDispatcher::get_completion_suggestions(parse).is_empty());

    // The cursor is a byte offset, so it is the full byte length of the input.
    let input = "echo 你好";
    let parse = subject.parse(StringReader::from(input), CommandSource {});
    let suggestions = CommandDispatcher::get_completion_suggestions_with_cursor(parse, input.len());
    assert!(suggestions.is_empty());
}

#[test]
fn suggests_literal_after_non_ascii_prefix_is_rejected() {
    let subject = greedy_dispatcher();
    // A partial literal still suggests normally.
    let parse = subject.parse(StringReader::from("ec"), CommandSource {});
    let suggestions = CommandDispatcher::get_completion_suggestions(parse);
    assert_eq!(
        suggestions
            .list()
            .iter()
            .map(|s| s.text())
            .collect::<Vec<_>>(),
        vec!["echo"]
    );
}

#[test]
fn formats_error_context_on_character_boundaries() {
    let subject = word_dispatcher();
    let input = "quote \"你好你好";
    let error = subject.execute(input, CommandSource {}).unwrap_err();

    let error = error.syntax().unwrap();
    assert_eq!(error.cursor(), Some(input.len()));
    assert_eq!(
        error.context().as_deref(),
        Some("...uote \"你好你好<--[HERE]")
    );
    assert_eq!(
        error.message(),
        "Unclosed quoted string at position 19: ...uote \"你好你好<--[HERE]"
    );
    assert_eq!(format!("{error:?}"), error.message());
}

#[test]
fn suggests_after_prefixes_whose_lowercase_changes_utf8_length() {
    let mut subject = CommandDispatcher::<CommandSource>::new();
    subject.register(literal("K").then(literal("next").executes(|_| success(1))));
    subject.register(literal("İ").then(literal("next").executes(|_| success(1))));

    for input in ["K ", "İ "] {
        let parse = subject.parse(StringReader::from(input), CommandSource {});
        let suggestions = CommandDispatcher::get_completion_suggestions(parse);
        assert_eq!(
            suggestions
                .list()
                .iter()
                .map(|suggestion| suggestion.text())
                .collect::<Vec<_>>(),
            vec!["next"],
            "input: {input:?}"
        );
    }
}

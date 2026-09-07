use std::{ops::RangeInclusive, sync::Arc};

use azalea_brigadier::{
    arguments::{ArgumentType, ParsedValue},
    errors::BuiltInError,
    parsers,
    prelude::*,
    string_reader::StringReader,
    suggestion::SuggestionsBuilder,
    tree::CommandNode,
};

fn ok(_: &CommandContext<()>) -> CommandResult {
    Ok(1)
}

macro_rules! numeric_tests {
    ($name:ident, $constructor:ident, $number:ty, $small:ident, $big:ident) => {
        #[test]
        fn $name() {
            let mut dispatcher = CommandDispatcher::<()>::new();
            // Bounds remain available after metadata, handlers, and children.
            let node = $constructor("value")
                .describe("bounded")
                .requires(|_| true)
                .suggests(|_: CommandContext<()>, b: SuggestionsBuilder| b.suggest("2").build())
                .executes(ok)
                .then(literal("child").executes(ok))
                .min(0 as $number)
                .max(10 as $number)
                .range((1 as $number)..=(3 as $number));
            dispatcher.register(literal("read").then(node));
            for value in ["1", "2", "3"] {
                assert_eq!(dispatcher.execute(format!("read {value}"), ()).unwrap(), 1);
            }
            assert_eq!(dispatcher.execute("read 2 child", ()).unwrap(), 1);
            let below = dispatcher.execute("read 0", ()).unwrap_err();
            let above = dispatcher.execute("read 4", ()).unwrap_err();
            assert!(matches!(
                below.syntax().unwrap().kind(),
                BuiltInError::$small { .. }
            ));
            assert!(matches!(
                above.syntax().unwrap().kind(),
                BuiltInError::$big { .. }
            ));
            assert_eq!(below.syntax().unwrap().cursor(), Some(5));
            assert_eq!(above.syntax().unwrap().cursor(), Some(5));
            let suggestions =
                CommandDispatcher::get_completion_suggestions(dispatcher.parse("read ".into(), ()));
            assert_eq!(suggestions.list()[0].text(), "2");

            // Standalone parsers apply the same rules and restore the cursor.
            let parser = parsers::$constructor().min(1 as $number).max(3 as $number);
            let mut reader = StringReader::from("xx4");
            reader.cursor = 2;
            let error = parser.parse(&mut reader).unwrap_err();
            assert_eq!(reader.cursor(), 2);
            assert_eq!(error.cursor(), Some(2));
            assert!(matches!(error.kind(), BuiltInError::$big { .. }));
            let value = parser.parse(&mut StringReader::from("2")).unwrap();
            assert_eq!(value.downcast_ref::<$number>(), Some(&(2 as $number)));

            // Cloning a builder copies the parser configuration independently.
            let original = $constructor::<(), i32>("value").range((1 as $number)..=(3 as $number));
            let changed = original.clone().min(2 as $number).build();
            assert!(
                original
                    .build()
                    .argument()
                    .parse(&mut StringReader::from("1"))
                    .is_ok()
            );
            assert!(
                changed
                    .argument()
                    .parse(&mut StringReader::from("1"))
                    .is_err()
            );

            // Neither a single bound nor a closed singleton range invents
            // an additional bound.
            assert!(
                parsers::$constructor()
                    .min(0 as $number)
                    .parse(&mut StringReader::from("999"))
                    .is_ok()
            );
            assert!(
                parsers::$constructor()
                    .max(0 as $number)
                    .parse(&mut StringReader::from("-999"))
                    .is_ok()
            );
            assert!(
                parsers::$constructor()
                    .range((2 as $number)..=(2 as $number))
                    .parse(&mut StringReader::from("2"))
                    .is_ok()
            );
        }
    };
}
numeric_tests!(integer_bounds, integer, i32, IntegerTooSmall, IntegerTooBig);
numeric_tests!(long_bounds, long, i64, LongTooSmall, LongTooBig);
numeric_tests!(float_bounds, float, f32, FloatTooSmall, FloatTooBig);
numeric_tests!(double_bounds, double, f64, DoubleTooSmall, DoubleTooBig);

#[test]
fn integer_extremes_and_float_fractional_boundaries() {
    for input in [i32::MIN.to_string(), i32::MAX.to_string()] {
        assert!(
            parsers::integer()
                .range(i32::MIN..=i32::MAX)
                .parse(&mut StringReader::from(input))
                .is_ok()
        );
    }
    for input in [i64::MIN.to_string(), i64::MAX.to_string()] {
        assert!(
            parsers::long()
                .range(i64::MIN..=i64::MAX)
                .parse(&mut StringReader::from(input))
                .is_ok()
        );
    }
    for input in ["-1.5", "0", "2.5"] {
        assert!(
            parsers::float()
                .range(-1.5..=2.5)
                .parse(&mut StringReader::from(input))
                .is_ok()
        );
        assert!(
            parsers::double()
                .range(-1.5..=2.5)
                .parse(&mut StringReader::from(input))
                .is_ok()
        );
    }
    for input in ["-1.6", "2.6"] {
        assert!(
            parsers::float()
                .range(-1.5..=2.5)
                .parse(&mut StringReader::from(input))
                .is_err()
        );
        assert!(
            parsers::double()
                .range(-1.5..=2.5)
                .parse(&mut StringReader::from(input))
                .is_err()
        );
    }
}

#[test]
fn invalid_configurations_are_rejected_at_construction() {
    use std::panic::catch_unwind;
    assert!(catch_unwind(|| parsers::integer().range(RangeInclusive::new(3, 1))).is_err());
    assert!(catch_unwind(|| parsers::long().min(3).max(1)).is_err());
    assert!(catch_unwind(|| parsers::float().max(1.0).min(3.0)).is_err());
    assert!(catch_unwind(|| parsers::double().min(f64::NAN)).is_err());
    assert!(catch_unwind(|| parsers::float().max(f32::NAN)).is_err());
    assert!(catch_unwind(|| parsers::float().range(f32::NAN..=1.0)).is_err());
    assert!(catch_unwind(|| parsers::double().range(1.0..=f64::NAN)).is_err());
    let mut exhausted = 1..=1;
    exhausted.next();
    assert!(catch_unwind(|| parsers::integer().range(exhausted)).is_err());
}

#[test]
fn names_move_into_built_nodes_without_an_extra_copy() {
    let name = String::from("value");
    let pointer = name.as_ptr();
    let node = integer::<(), i32>(name).describe("number").build();
    assert_eq!(node.name().as_ptr(), pointer);
    let name = String::from("literal");
    let pointer = name.as_ptr();
    assert_eq!(literal::<(), i32>(name).build().name().as_ptr(), pointer);
}

#[test]
fn strings_keep_their_distinct_tokenization_and_boolean_completion() {
    let mut d = CommandDispatcher::<()>::new();
    let nodes: Vec<CommandNode<()>> = vec![
        literal("word").then(word("value").executes(ok)).into(),
        literal("quoted").then(string("value").executes(ok)).into(),
        literal("greedy")
            .then(greedy_string("value").executes(ok))
            .into(),
        literal("flag").then(boolean("value").executes(ok)).into(),
    ];
    for node in nodes {
        d.register(node);
    }
    for input in [
        "word one",
        "quoted \"two words\"",
        "greedy two words",
        "flag true",
        "flag false",
    ] {
        assert_eq!(d.execute(input, ()).unwrap(), 1);
    }
    for input in ["word two words", "quoted two words", "flag yes"] {
        assert!(d.execute(input, ()).is_err());
    }
    let suggestions = CommandDispatcher::get_completion_suggestions(d.parse("flag t".into(), ()));
    assert_eq!(suggestions.list()[0].text(), "true");
    assert!(!parsers::boolean().examples().is_empty());
    assert!(!parsers::word().examples().is_empty());
    assert!(!parsers::string().examples().is_empty());
    assert!(!parsers::greedy_string().examples().is_empty());
}

#[test]
fn custom_non_clone_parser_can_be_configured_and_built() {
    struct Custom {
        prefix: String,
    }
    impl CommandArgument for Custom {
        type Builder<S, R> = ArgumentBuilder<S, R, Self>;
    }
    impl ArgumentType for Custom {
        fn parse(&self, reader: &mut StringReader) -> Result<Arc<ParsedValue>, CommandSyntaxError> {
            Ok(Arc::new(format!(
                "{}{}",
                self.prefix,
                reader.read_unquoted_string()
            )))
        }
    }
    let mut d = CommandDispatcher::<()>::new();
    let mut configured = 0;
    let node = Custom {
        prefix: "old:".into(),
    }
    .into_arg("value")
    .configure_parser(|mut parser| {
        configured += 1;
        parser.prefix = "new:".into();
        parser
    })
    .describe("custom")
    .executes(|ctx| -> CommandResult {
        assert_eq!(get_string(ctx, "value").unwrap(), "new:Bob");
        Ok(1)
    });
    assert_eq!(configured, 1);
    d.register(node);
    assert_eq!(d.execute("Bob", ()).unwrap(), 1);
}

#[test]
fn duplicate_children_merge_in_insertion_order() {
    let mut d = CommandDispatcher::<()>::new();
    d.register(
        literal("root")
            .then(
                integer("value")
                    .describe("first")
                    .executes(ok)
                    .then(literal("a").executes(ok)),
            )
            .then(
                integer("value")
                    .executes(|_| -> CommandResult { Ok(2) })
                    .then(literal("b").executes(ok)),
            ),
    );
    assert_eq!(d.execute("root 1", ()).unwrap(), 2);
    assert_eq!(d.execute("root 1 a", ()).unwrap(), 1);
    assert_eq!(d.execute("root 1 b", ()).unwrap(), 1);
    let root = d.root.read().child("root").unwrap();
    let value = root.read().child("value").unwrap();
    assert_eq!(value.read().description.as_deref(), Some("first"));
}

#[test]
fn redirects_and_forks_accept_mixed_builder_kinds() {
    let mut d = CommandDispatcher::<()>::new();
    let target = d.register(literal("target").then(integer("value").range(1..=3).executes(ok)));
    d.register(literal("alias").redirect(target.clone()));
    d.register(literal("fork").fork(target, Arc::new(|_| Ok(vec![Arc::new(()), Arc::new(())]))));
    assert_eq!(d.execute("alias 2", ()).unwrap(), 1);
    assert_eq!(d.execute("fork 2", ()).unwrap(), 2);
    assert!(d.execute("alias 4", ()).is_err());
}

#[cfg(feature = "async")]
#[test]
fn async_handlers_keep_parser_configuration_and_switch_modes() {
    async fn read(ctx: Arc<CommandContext<()>>) -> CommandResult {
        std::future::ready(()).await;
        Ok(get_integer(&ctx, "value").unwrap())
    }
    let mut d = CommandDispatcher::<()>::new();
    d.register(
        integer("value")
            .executes_async(read)
            .describe("async")
            .range(1..=3),
    );
    assert_eq!(
        futures::executor::block_on(d.execute_async("2", ())).unwrap(),
        2
    );
    assert!(futures::executor::block_on(d.execute_async("4", ())).is_err());
    assert!(d.execute("2", ()).is_err());

    let node = integer::<(), i32>("value")
        .executes_async(read)
        .executes(ok)
        .max(3)
        .build();
    assert!(node.async_command.is_none());
    assert!(node.command.is_some());
    let node = integer::<(), i32>("value")
        .executes(ok)
        .executes_async(read)
        .min(1)
        .build();
    assert!(node.command.is_none());
    assert!(node.async_command.is_some());
}

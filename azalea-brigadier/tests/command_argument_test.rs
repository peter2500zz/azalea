use std::sync::Arc;

use azalea_brigadier::{
    arguments::{ArgumentType, ParsedValue},
    parsers::{Boolean, Double, Float, Integer, Long, StringArgument},
    prelude::*,
    string_reader::StringReader,
    suggestion::SuggestionsBuilder,
    tree::CommandNode,
};

// These types belong to a downstream crate, not the library defining the
// builders. No inherent impl on a foreign ArgumentBuilder or Deref is used.
#[derive(Default)]
struct TokenParser {
    id: u32,
    prefix: String,
    uppercase: bool,
}

impl TokenParser {
    fn new(id: u32) -> Self {
        Self {
            id,
            ..Self::default()
        }
    }
}

impl ArgumentType for TokenParser {
    fn parse(&self, reader: &mut StringReader) -> Result<Arc<ParsedValue>, CommandSyntaxError> {
        let word = reader.read_string()?;
        let word = if self.uppercase {
            word.to_uppercase()
        } else {
            word
        };
        Ok(Arc::new(format!("{}:{word}", self.prefix)))
    }
}

impl CommandArgument for TokenParser {
    type Builder<S, R> = TokenBuilder<S, R>;
}

struct TokenBuilder<S, R> {
    node: ArgumentBuilder<S, R, TokenParser>,
    // Extra construction-time state must survive every common setter, even
    // when it is not part of the underlying parser or command node.
    marker: String,
    updates: usize,
}

impl<S, R> From<ArgumentBuilder<S, R, TokenParser>> for TokenBuilder<S, R> {
    fn from(node: ArgumentBuilder<S, R, TokenParser>) -> Self {
        Self {
            node,
            marker: "kept".into(),
            updates: 0,
        }
    }
}

impl<S, R> CommandBuilder for TokenBuilder<S, R> {
    type Source = S;
    type Output = R;
    type Kind = TokenParser;

    fn as_builder(&self) -> &ArgumentBuilder<S, R, TokenParser> {
        &self.node
    }

    fn map_builder(
        mut self,
        update: impl FnOnce(ArgumentBuilder<S, R, TokenParser>) -> ArgumentBuilder<S, R, TokenParser>,
    ) -> Self {
        self.node = update(self.node);
        self.updates += 1;
        self
    }

    fn into_builder(self) -> ArgumentBuilder<S, R, TokenParser> {
        self.node
    }
}

impl<S, R> TokenBuilder<S, R> {
    fn prefix(mut self, prefix: &str) -> Self {
        self.node.parser_mut().prefix = prefix.into();
        self
    }

    fn uppercase(mut self) -> Self {
        self.node.parser_mut().uppercase = true;
        self
    }
}

fn read(ctx: &CommandContext<()>) -> CommandResult {
    assert_eq!(get_string(ctx, "value").as_deref(), Some("test:BOB"));
    Ok(7)
}

#[test]
fn custom_methods_interleave_with_every_common_setter() {
    let node: TokenBuilder<(), i32> = TokenParser::arg("value")
        .prefix("old")
        .describe("A token")
        .uppercase()
        .requires(|_| true)
        .executes(read)
        .then(literal("child").executes(read))
        .then_built(literal("built").executes(read).build())
        .suggests(|_: CommandContext<()>, b: SuggestionsBuilder| b.suggest("Bob").build())
        .prefix("test");
    assert_eq!(node.marker, "kept");
    assert_eq!(node.updates, 6);
    assert_eq!(node.children().len(), 2);

    let mut dispatcher = CommandDispatcher::<()>::new();
    let root = dispatcher.register(literal("read").then(node));
    assert_eq!(
        root.read()
            .child("value")
            .unwrap()
            .read()
            .description
            .as_deref(),
        Some("A token")
    );
    for input in ["read bob", "read bob ", "read bob child", "read bob built"] {
        assert_eq!(dispatcher.execute(input, ()).unwrap(), 7);
    }
    let suggestions =
        CommandDispatcher::get_completion_suggestions(dispatcher.parse("read B".into(), ()));
    assert_eq!(suggestions.list()[0].text(), "Bob");
}

#[test]
fn named_factory_does_not_claim_new_and_preserves_configured_instances() {
    let name = "value".to_owned();
    let pointer = name.as_ptr();
    let parser = TokenParser::new(42);
    let mut builder: TokenBuilder<(), i32> =
        parser.into_arg(name).describe("configured").prefix("test");
    assert_eq!(builder.as_builder().parser().id, 42);
    builder.marker = "application state".into();
    let builder = builder.executes(read).uppercase();
    assert_eq!(builder.marker, "application state");
    let node = builder.into_node();
    assert_eq!(node.name().as_ptr(), pointer);

    let mut dispatcher = CommandDispatcher::<()>::new();
    dispatcher.register(node);
    assert_eq!(dispatcher.execute("bob", ()).unwrap(), 7);
    assert_eq!(
        TokenParser::arg::<(), i32>("value")
            .as_builder()
            .parser()
            .id,
        0
    );
}

#[test]
fn configured_non_default_non_clone_parsers_need_neither_bound() {
    struct Configured(Arc<String>);
    impl ArgumentType for Configured {
        fn parse(&self, reader: &mut StringReader) -> Result<Arc<ParsedValue>, CommandSyntaxError> {
            Ok(Arc::new(format!(
                "{}{}",
                self.0,
                reader.read_unquoted_string()
            )))
        }
    }
    impl CommandArgument for Configured {
        type Builder<S, R> = ArgumentBuilder<S, R, Self>;
    }
    let state = Arc::new("prefix:".to_owned());
    let node = Configured(Arc::clone(&state))
        .into_arg("value")
        .describe("configured");
    assert!(Arc::ptr_eq(&node.parser().0, &state));
    let mut dispatcher = CommandDispatcher::<()>::new();
    dispatcher.register(node.executes(|ctx| -> CommandResult {
        assert_eq!(get_string(ctx, "value").as_deref(), Some("prefix:Bob"));
        Ok(1)
    }));
    assert_eq!(dispatcher.execute("Bob", ()).unwrap(), 1);
}

#[test]
fn a_trait_factory_can_be_disambiguated_from_an_inherent_arg() {
    #[derive(Default)]
    struct Colliding(i32);
    impl Colliding {
        fn arg(id: i32) -> Self {
            Self(id)
        }
    }
    impl ArgumentType for Colliding {
        fn parse(&self, reader: &mut StringReader) -> Result<Arc<ParsedValue>, CommandSyntaxError> {
            reader.skip();
            Ok(Arc::new(self.0))
        }
    }
    impl CommandArgument for Colliding {
        type Builder<S, R> = ArgumentBuilder<S, R, Self>;
    }

    assert_eq!(Colliding::arg(42).0, 42);
    let mut dispatcher = CommandDispatcher::<()>::new();
    dispatcher.register(
        <Colliding as CommandArgument>::arg("value")
            .executes(|ctx| -> CommandResult { Ok(get_integer(ctx, "value").unwrap()) }),
    );
    assert_eq!(dispatcher.execute("x", ()).unwrap(), 0);

    let mut dispatcher = CommandDispatcher::<()>::new();
    dispatcher.register(
        Colliding::arg(42)
            .into_arg("value")
            .executes(|ctx| -> CommandResult { Ok(get_integer(ctx, "value").unwrap()) }),
    );
    assert_eq!(dispatcher.execute("x", ()).unwrap(), 42);
}

#[test]
fn factories_preserve_builtin_bounds_and_string_policies() {
    let ok = |_: &CommandContext<()>| -> CommandResult { Ok(1) };
    let nodes: Vec<CommandNode<()>> = vec![
        Integer::arg("int")
            .describe("integer")
            .range(1..=3)
            .executes(ok)
            .into_node(),
        Long::arg("long").min(1).max(3).executes(ok).into_node(),
        Float::arg("float")
            .range(1.0..=3.0)
            .executes(ok)
            .into_node(),
        Double::arg("double")
            .range(1.0..=3.0)
            .executes(ok)
            .into_node(),
        Boolean::arg("boolean").executes(ok).into_node(),
    ];
    for (node, (valid, invalid)) in nodes.into_iter().zip([
        ("2", "4"),
        ("2", "4"),
        ("2.5", "4"),
        ("2.5", "4"),
        ("true", "yes"),
    ]) {
        let mut dispatcher = CommandDispatcher::<()>::new();
        dispatcher.register(node);
        assert_eq!(dispatcher.execute(valid, ()).unwrap(), 1);
        assert!(dispatcher.execute(invalid, ()).is_err());
    }
    let parser = StringArgument::GreedyPhrase.into_arg::<(), i32>("text");
    assert!(matches!(parser.parser(), StringArgument::GreedyPhrase));
    let mut dispatcher = CommandDispatcher::<()>::new();
    dispatcher.register(parser.executes(ok));
    assert_eq!(dispatcher.execute("two words", ()).unwrap(), 1);
}

#[test]
fn custom_builders_keep_permissions_and_execution_errors() {
    struct Source {
        allowed: bool,
    }
    let mut dispatcher = CommandDispatcher::<Source>::new();
    dispatcher.register(
        TokenParser::arg("value")
            .requires(|source: &Source| source.allowed)
            .uppercase()
            .executes(|_| -> Result<i32, std::io::Error> {
                Err(std::io::Error::other("handler error"))
            })
            .prefix("permission"),
    );
    assert!(
        dispatcher
            .execute("bob", Source { allowed: false })
            .unwrap_err()
            .syntax()
            .is_some()
    );
    let error = dispatcher
        .execute("bob", Source { allowed: true })
        .unwrap_err();
    assert!(error.execution().unwrap().is::<std::io::Error>());
}

#[test]
fn routing_and_duplicate_nodes_accept_custom_builders() {
    let mut dispatcher = CommandDispatcher::<()>::new();
    let target = dispatcher
        .register(literal("target").then(literal("end").executes(|_| -> CommandResult { Ok(3) })));
    dispatcher.register(
        literal("redirect").then(
            TokenParser::arg("value")
                .redirect(target.clone())
                .uppercase(),
        ),
    );
    dispatcher.register(
        literal("fork").then(
            TokenParser::arg("value")
                .fork(
                    target.clone(),
                    Arc::new(|_| Ok(vec![Arc::new(()), Arc::new(())])),
                )
                .prefix("fork"),
        ),
    );
    dispatcher.register(
        literal("forward").then(
            TokenParser::arg("value")
                .forward(target, None, false)
                .uppercase(),
        ),
    );
    assert_eq!(dispatcher.execute("redirect bob end", ()).unwrap(), 3);
    assert_eq!(dispatcher.execute("fork bob end", ()).unwrap(), 2);
    assert_eq!(dispatcher.execute("forward bob end", ()).unwrap(), 3);

    dispatcher.register(TokenParser::arg("value").executes(|_| -> CommandResult { Ok(1) }));
    dispatcher.register(
        TokenParser::arg("value")
            .executes(|_| -> CommandResult { Ok(2) })
            .uppercase(),
    );
    assert_eq!(dispatcher.execute("bob", ()).unwrap(), 2);
}

#[test]
fn existing_command_wrappers_can_adapt_at_the_registration_boundary() {
    struct Wrapper;
    impl From<Wrapper> for CommandNode<()> {
        fn from(_: Wrapper) -> Self {
            literal("wrapped")
                .executes(|_| -> CommandResult { Ok(9) })
                .build()
        }
    }
    impl IntoCommandNode<()> for Wrapper {
        fn into_node(self) -> CommandNode<()> {
            self.into()
        }
    }
    let mut dispatcher = CommandDispatcher::<()>::new();
    dispatcher.register(Wrapper);
    dispatcher.register(literal("parent").then(Wrapper));
    assert_eq!(dispatcher.execute("wrapped", ()).unwrap(), 9);
    assert_eq!(dispatcher.execute("parent wrapped", ()).unwrap(), 9);
}

#[cfg(feature = "async")]
#[test]
fn async_handlers_and_custom_methods_keep_the_same_concrete_builder() {
    use futures::executor::block_on;
    async fn later(ctx: Arc<CommandContext<()>>) -> CommandResult {
        std::future::ready(()).await;
        read(&ctx)
    }
    let node: TokenBuilder<(), i32> = TokenParser::arg("value")
        .executes_async(later)
        .prefix("test")
        .describe("async")
        .uppercase();
    let mut dispatcher = CommandDispatcher::<()>::new();
    dispatcher.register(node);
    assert_eq!(block_on(dispatcher.execute_async("bob ", ())).unwrap(), 7);
    assert!(dispatcher.execute("bob", ()).is_err());

    let sync = TokenParser::arg::<(), i32>("value")
        .executes_async(later)
        .executes(read)
        .uppercase()
        .build();
    assert!(sync.async_command.is_none());
    assert!(sync.command.is_some());
    let asynchronous = TokenParser::arg::<(), i32>("value")
        .executes(read)
        .executes_async(async |ctx| -> CommandResult { read(&ctx) })
        .prefix("test")
        .uppercase()
        .build();
    assert!(asynchronous.command.is_none());
    assert!(asynchronous.async_command.is_some());
    let mut dispatcher = CommandDispatcher::<()>::new();
    dispatcher.register(asynchronous);
    assert_eq!(block_on(dispatcher.execute_async("bob", ())).unwrap(), 7);
}

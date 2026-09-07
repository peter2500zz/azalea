#![cfg(not(feature = "async"))]

use std::{convert::Infallible, rc::Rc, sync::Arc};

use azalea_brigadier::{
    arguments::{ArgumentType, ParsedValue},
    builder::{ArgumentBuilder, CommandArgument, literal_argument_builder::literal},
    command_dispatcher::CommandDispatcher,
    context::{CommandContext, CommandContextRef},
    errors::CommandSyntaxError,
    string_reader::StringReader,
};

#[derive(Default)]
struct ThreadLocalArgument;

impl CommandArgument for ThreadLocalArgument {
    type Builder<S, R> = ArgumentBuilder<S, R, Self>;
}

impl ArgumentType for ThreadLocalArgument {
    #[allow(clippy::arc_with_non_send_sync)]
    fn parse(&self, reader: &mut StringReader) -> Result<Arc<ParsedValue>, CommandSyntaxError> {
        reader.skip();
        Ok(Arc::new(Rc::new(())))
    }
}

fn context_ref_is_rc(context: CommandContextRef<(), i32>) -> Rc<CommandContext<(), i32>> {
    context
}

#[test]
fn synchronous_builds_keep_rc_contexts_and_thread_local_arguments() {
    let mut dispatcher = CommandDispatcher::<(), i32>::new();
    dispatcher.register(
        literal("local").then(ThreadLocalArgument::arg("value").executes(|ctx| {
            assert!(
                ctx.argument("value")
                    .and_then(|value| value.downcast_ref::<Rc<()>>())
                    .is_some()
            );
            Ok::<_, Infallible>(7)
        })),
    );

    let parsed = dispatcher.parse("local x".into(), ());
    let context = context_ref_is_rc(CommandContextRef::new(parsed.context.build("local x")));
    assert_eq!(Rc::strong_count(&context), 1);
    assert_eq!(dispatcher.execute("local x", ()).unwrap(), 7);
}

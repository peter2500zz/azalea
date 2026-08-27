#![cfg(feature = "async")]

use std::{
    any::Any,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use azalea_brigadier::{
    arguments::{ArgumentType, integer_argument_type::get_integer},
    builder::{literal_argument_builder::literal, required_argument_builder::argument},
    command_dispatcher::CommandDispatcher,
    context::CommandContext,
    errors::{BuiltInError, CommandSyntaxError},
    string_reader::StringReader,
};
use futures::executor::block_on;

fn send_static<T: Send + 'static>(value: T) -> T {
    value
}

fn send<T: Send>(value: T) -> T {
    value
}

#[test]
fn async_command_extracts_owned_arguments_and_runs() {
    let mut dispatcher = CommandDispatcher::<(), i32>::new();
    dispatcher.register(literal("double").then(
        argument("value", azalea_brigadier::prelude::integer()).executes_async(|ctx| {
            let value = get_integer(ctx, "value").unwrap();
            async move { value * 2 }
        }),
    ));

    let future = send(dispatcher.execute_async("double 21", ()));
    assert_eq!(block_on(future).unwrap(), 42);
}

#[test]
fn execute_async_defers_preparation_until_polled() {
    let prepared = Arc::new(AtomicUsize::new(0));
    let mut dispatcher = CommandDispatcher::<(), i32>::new();
    dispatcher.register(literal("later").executes_async({
        let prepared = Arc::clone(&prepared);
        move |_| {
            prepared.fetch_add(1, Ordering::Relaxed);
            async { 1 }
        }
    }));

    let future = dispatcher.execute_async("later", ());
    assert_eq!(prepared.load(Ordering::Relaxed), 0);
    assert_eq!(block_on(future).unwrap(), 1);
    assert_eq!(prepared.load(Ordering::Relaxed), 1);
}

#[test]
fn async_execution_accepts_existing_synchronous_commands() {
    let mut dispatcher = CommandDispatcher::new();
    dispatcher.register(literal("answer").executes(|_: &CommandContext<()>| 42));

    assert_eq!(
        block_on(dispatcher.execute_async("answer", ())).unwrap(),
        42
    );
}

#[test]
fn synchronous_execution_does_not_run_an_async_only_command() {
    let mut dispatcher = CommandDispatcher::<(), i32>::new();
    dispatcher.register(literal("later").executes_async(|_| async { 42 }));

    let error = dispatcher.execute("later", ()).unwrap_err();
    assert_eq!(error.kind(), &BuiltInError::DispatcherUnknownCommand);
    assert_eq!(block_on(dispatcher.execute_async("later", ())).unwrap(), 42);
}

#[test]
fn the_last_execution_setter_wins() {
    let mut synchronous_last = CommandDispatcher::new();
    synchronous_last.register(
        literal("value")
            .executes_async(|_: &CommandContext<()>| async { 1 })
            .executes(|_| 2),
    );
    assert_eq!(synchronous_last.execute("value", ()).unwrap(), 2);
    assert_eq!(
        block_on(synchronous_last.execute_async("value", ())).unwrap(),
        2
    );

    let mut asynchronous_last = CommandDispatcher::<(), i32>::new();
    asynchronous_last.register(
        literal("value")
            .executes(|_| 1)
            .executes_async(|_| async { 2 }),
    );
    assert!(asynchronous_last.execute("value", ()).is_err());
    assert_eq!(
        block_on(asynchronous_last.execute_async("value", ())).unwrap(),
        2
    );
}

#[test]
fn merging_branches_preserves_or_replaces_the_selected_action() {
    let mut dispatcher = CommandDispatcher::<(), i32>::new();
    dispatcher.register(literal("base").executes_async(|_| async { 1 }));
    dispatcher.register(literal("base").then(literal("child").executes(|_| 2)));

    assert_eq!(block_on(dispatcher.execute_async("base", ())).unwrap(), 1);
    assert_eq!(dispatcher.execute("base child", ()).unwrap(), 2);

    dispatcher.register(literal("base").executes(|_| 3));
    assert_eq!(dispatcher.execute("base", ()).unwrap(), 3);
    assert_eq!(block_on(dispatcher.execute_async("base", ())).unwrap(), 3);
}

#[test]
fn async_errors_keep_their_structure() {
    let mut dispatcher = CommandDispatcher::<(), i32>::new();
    dispatcher.register(literal("fail").executes_async_result(|_| async {
        Err(BuiltInError::DispatcherUnknownArgument.create())
    }));

    let error = block_on(dispatcher.execute_async("fail", ())).unwrap_err();
    assert_eq!(error.kind(), &BuiltInError::DispatcherUnknownArgument);
}

#[derive(Debug, PartialEq)]
struct ForkSource(i32);

#[test]
fn redirects_and_forks_keep_brigadier_result_semantics() {
    let mut dispatcher = CommandDispatcher::new();
    dispatcher.register(
        literal("actual").executes_async(|ctx: &CommandContext<ForkSource>| {
            let result = ctx.source.0;
            async move { result }
        }),
    );

    let root = dispatcher.root.clone();
    dispatcher.register(literal("redirected").fork(
        root,
        Arc::new(|_| Ok(vec![Arc::new(ForkSource(20)), Arc::new(ForkSource(22))])),
    ));

    // A fork counts successful branches; it does not sum their command values.
    assert_eq!(
        block_on(dispatcher.execute_async("redirected actual", ForkSource(0))).unwrap(),
        2
    );
}

#[test]
fn forked_errors_are_counted_as_failed_branches() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut dispatcher = CommandDispatcher::<usize, i32>::new();
    dispatcher.register(literal("actual").executes_async_result({
        let calls = Arc::clone(&calls);
        move |ctx| {
            let source = *ctx.source;
            let calls = Arc::clone(&calls);
            async move {
                calls.fetch_add(1, Ordering::Relaxed);
                if source == 1 {
                    Err(BuiltInError::DispatcherUnknownArgument.create())
                } else {
                    Ok(99)
                }
            }
        }
    }));

    let root = dispatcher.root.clone();
    dispatcher
        .register(literal("fork").fork(root, Arc::new(|_| Ok(vec![Arc::new(1), Arc::new(2)]))));

    assert_eq!(
        block_on(dispatcher.execute_async("fork actual", 0)).unwrap(),
        1
    );
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

struct ThreadLocalArgument;

impl ArgumentType for ThreadLocalArgument {
    // This is intentionally thread-local: the test proves it gets discarded
    // before the returned execution future crosses threads.
    #[allow(clippy::arc_with_non_send_sync)]
    fn parse(&self, reader: &mut StringReader) -> Result<Arc<dyn Any>, CommandSyntaxError> {
        reader.skip();
        Ok(Arc::new(Rc::new(())))
    }
}

#[test]
fn a_non_send_parsed_argument_never_enters_the_execution_future() {
    let mut dispatcher = CommandDispatcher::<(), i32>::new();
    dispatcher.register(
        literal("local")
            .then(argument("value", ThreadLocalArgument).executes_async(|_| async { 7 })),
    );

    let execution = dispatcher.prepare_async("local x", ()).unwrap();
    let future = send_static(execution.execute());
    assert_eq!(block_on(future).unwrap(), 7);
}

#[test]
fn usage_treats_async_nodes_as_executable() {
    let mut dispatcher = CommandDispatcher::<(), i32>::new();
    dispatcher.register(literal("later").executes_async(|_| async { 1 }));

    assert_eq!(
        dispatcher.get_all_usage(&dispatcher.root.read(), &(), false),
        vec!["later"]
    );
}

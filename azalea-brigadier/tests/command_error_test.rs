use std::{
    error::Error,
    fmt::{self, Display},
};

use azalea_brigadier::{
    command_dispatcher::CommandDispatcher,
    errors::{BuiltInError, CommandError},
    prelude::literal,
};

#[derive(Debug, PartialEq)]
struct ApplicationError;

impl Display for ApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("application command failed")
    }
}

impl Error for ApplicationError {}

#[test]
fn a_fallible_command_returns_an_outer_execution_error() {
    let mut dispatcher = CommandDispatcher::new();
    dispatcher.register(literal("fail").executes_result(|_| Err::<i32, _>(ApplicationError)));

    let error = dispatcher.execute("fail", ()).unwrap_err();
    assert!(matches!(error, CommandError::Execution(_)));
    assert_eq!(error.to_string(), "application command failed");
    assert!(
        error
            .source()
            .and_then(|source| source.downcast_ref::<ApplicationError>())
            .is_some()
    );
    assert!(error.execution().is_some());
    assert!(error.syntax().is_none());
}

#[test]
fn syntax_errors_keep_their_structured_information() {
    let dispatcher = CommandDispatcher::<(), i32>::new();

    let error = dispatcher.execute("missing", ()).unwrap_err();
    let syntax = error.syntax().expect("dispatcher errors are syntax errors");
    assert_eq!(syntax.kind(), &BuiltInError::DispatcherUnknownCommand);
    assert_eq!(syntax.cursor(), Some(0));
    assert_eq!(error.message(), syntax.message());
}

#[test]
fn different_application_error_types_can_share_one_dispatcher() {
    #[derive(Debug)]
    struct OtherError;
    impl Display for OtherError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("other command failed")
        }
    }
    impl Error for OtherError {}

    let mut dispatcher = CommandDispatcher::new();
    dispatcher.register(literal("first").executes_result(|_| Err::<i32, _>(ApplicationError)));
    dispatcher.register(literal("second").executes_result(|_| Err::<i32, _>(OtherError)));

    assert!(
        dispatcher
            .execute("first", ())
            .unwrap_err()
            .execution()
            .is_some()
    );
    assert!(
        dispatcher
            .execute("second", ())
            .unwrap_err()
            .execution()
            .is_some()
    );
}

#[cfg(feature = "async")]
mod asynchronous {
    use futures::executor::block_on;

    use super::*;

    #[test]
    fn an_async_fallible_command_uses_the_same_error_channel() {
        let mut dispatcher = CommandDispatcher::new();
        dispatcher.register(
            literal("fail").executes_async_result(|_| async { Err::<i32, _>(ApplicationError) }),
        );

        let error = block_on(dispatcher.execute_async("fail", ())).unwrap_err();
        assert!(matches!(error, CommandError::Execution(_)));
        assert_eq!(error.to_string(), "application command failed");
        assert!(
            error
                .source()
                .and_then(|source| source.downcast_ref::<ApplicationError>())
                .is_some()
        );
    }

    #[test]
    fn a_fork_continues_after_an_execution_error() {
        use std::sync::Arc;

        let mut dispatcher = CommandDispatcher::new();
        dispatcher.register(literal("actual").executes_result(|ctx| {
            if *ctx.source == 1 {
                Err(ApplicationError)
            } else {
                Ok(7)
            }
        }));
        let root = dispatcher.root.clone();
        dispatcher
            .register(literal("fork").fork(root, Arc::new(|_| Ok(vec![Arc::new(1), Arc::new(2)]))));

        assert_eq!(
            block_on(dispatcher.execute_async("fork actual", 0)).unwrap(),
            1
        );
    }
}

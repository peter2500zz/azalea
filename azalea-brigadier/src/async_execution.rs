//! Runtime-agnostic execution plans for asynchronous commands.

use std::{future::Future, pin::Pin};

use crate::errors::{CommandError, CommandResultTrait};

/// The future returned by an asynchronous command action.
///
/// It is deliberately runtime-agnostic. Executors such as Tokio can spawn it,
/// but enabling azalea-brigadier's `async` feature does not pull in a runtime.
pub type CommandFuture<R> = Pin<Box<dyn Future<Output = Result<R, CommandError>> + Send + 'static>>;

/// A prepared asynchronous command execution.
///
/// Parsing, redirects, forks, and calls to command closures have already
/// completed. The command futures may retain their owned [`CommandContext`]
/// handles, whose parsed values are required to be `Send + Sync` whenever the
/// `async` feature is enabled. The plan is therefore safe to move to a
/// multi-threaded executor.
///
/// [`CommandContext`]: crate::context::CommandContext
#[must_use = "an asynchronous command does nothing until its execution is awaited"]
pub struct AsyncExecution<R> {
    commands: Vec<CommandFuture<R>>,
    forked: bool,
}

impl<R> AsyncExecution<R> {
    pub(crate) fn new(commands: Vec<CommandFuture<R>>, forked: bool) -> Self {
        Self { commands, forked }
    }

    pub(crate) fn empty(forked: bool) -> Self {
        Self::new(Vec::new(), forked)
    }
}

impl<R> AsyncExecution<R>
where
    R: CommandResultTrait,
{
    /// Await every command selected by redirects and forks, in Brigadier's
    /// normal deterministic order, and combine their results.
    pub async fn execute(self) -> Result<R, CommandError> {
        let mut summed = 0;

        for command in self.commands {
            match command.await {
                Ok(result) => {
                    let Some(value) = result.as_i32() else {
                        return Ok(result);
                    };
                    summed += if self.forked { 1 } else { value };
                }
                Err(error) if !self.forked => return Err(error),
                Err(_) => {}
            }
        }

        Ok(R::new(summed))
    }
}

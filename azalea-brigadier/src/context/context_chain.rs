use std::{rc::Rc, sync::Arc};

use super::CommandContext;
#[cfg(feature = "async")]
use crate::async_execution::{AsyncExecution, CommandFuture};
use crate::{
    errors::{CommandResultTrait, CommandSyntaxError},
    result_consumer::ResultConsumer,
};

pub struct ContextChain<S, R> {
    modifiers: Vec<Rc<CommandContext<S, R>>>,
    executable: Rc<CommandContext<S, R>>,
    next_stage_cache: Option<Rc<ContextChain<S, R>>>,
}

impl<S, R: CommandResultTrait> ContextChain<S, R> {
    pub fn new(
        modifiers: Vec<Rc<CommandContext<S, R>>>,
        executable: Rc<CommandContext<S, R>>,
    ) -> Self {
        if executable.command.is_none() {
            panic!("Last command in chain must be executable");
        }
        Self {
            modifiers,
            executable,
            next_stage_cache: None,
        }
    }

    pub fn try_flatten(root_context: Rc<CommandContext<S, R>>) -> Option<Self> {
        let mut modifiers = Vec::new();
        let mut current = root_context;
        loop {
            let child = current.child.clone();
            let Some(child) = child else {
                // Last entry must be executable command
                current.command.as_ref()?;

                return Some(ContextChain::new(modifiers, current));
            };

            modifiers.push(current);
            current = child;
        }
    }

    /// Flatten a context whose last node has either an asynchronous command or
    /// a synchronous command that can be run as part of an async execution.
    #[cfg(feature = "async")]
    pub(crate) fn try_flatten_async(root_context: Rc<CommandContext<S, R>>) -> Option<Self> {
        let mut modifiers = Vec::new();
        let mut current = root_context;
        loop {
            let child = current.child.clone();
            let Some(child) = child else {
                if current.async_command.is_none() && current.command.is_none() {
                    return None;
                }

                return Some(Self {
                    modifiers,
                    executable: current,
                    next_stage_cache: None,
                });
            };

            modifiers.push(current);
            current = child;
        }
    }

    pub fn run_modifier(
        modifier: Rc<CommandContext<S, R>>,
        source: Arc<S>,
        result_consumer: &dyn ResultConsumer<S, R>,
        forked_mode: bool,
    ) -> Result<Vec<Arc<S>>, CommandSyntaxError> {
        let source_modifier = modifier.redirect_modifier();
        let Some(source_modifier) = source_modifier else {
            return Ok(vec![source]);
        };

        let context_to_use = Rc::new(modifier.copy_for(source));
        let err = match (source_modifier)(&context_to_use) {
            Ok(res) => return Ok(res),
            Err(e) => e,
        };

        result_consumer.on_command_complete(context_to_use, false, 0);
        if forked_mode {
            return Ok(vec![]);
        }
        Err(err)
    }

    pub fn run_executable(
        &self,
        executable: Rc<CommandContext<S, R>>,
        source: Arc<S>,
        result_consumer: &dyn ResultConsumer<S, R>,
        forked_mode: bool,
    ) -> Result<R, CommandSyntaxError> {
        let context_to_use = Rc::new(executable.copy_for(source));
        let Some(command) = &executable.command else {
            unimplemented!();
        };

        let res = (command)(&context_to_use);
        let err = match res {
            Ok(res) => {
                let Some(res) = res.as_i32() else {
                    // these are treated as exceptions, so they can bubble up without doing anything
                    // else
                    return Ok(res);
                };
                result_consumer.on_command_complete(context_to_use, true, res);
                return if forked_mode {
                    Ok(R::new(1))
                } else {
                    Ok(R::new(res))
                };
            }
            Err(err) => err,
        };

        result_consumer.on_command_complete(context_to_use, false, 0);
        if forked_mode { Ok(R::new(0)) } else { Err(err) }
    }

    pub fn execute_all(
        &self,
        source: Arc<S>,
        result_consumer: &dyn ResultConsumer<S, R>,
    ) -> Result<R, CommandSyntaxError> {
        if self.modifiers.is_empty() {
            return self.run_executable(self.executable.clone(), source, result_consumer, false);
        }

        let mut forked_mode = false;
        let mut current_sources = vec![source];

        for modifier in &self.modifiers {
            forked_mode |= modifier.is_forked();

            let mut next_sources = Vec::new();
            for source_to_run in current_sources {
                let res = Self::run_modifier(
                    modifier.clone(),
                    source_to_run.clone(),
                    result_consumer,
                    forked_mode,
                )?;
                next_sources.extend(res);
            }
            if next_sources.is_empty() {
                return Ok(R::new(0));
            }
            current_sources = next_sources;
        }

        let mut summed = 0;
        for execution_source in current_sources {
            let res = self.run_executable(
                self.executable.clone(),
                execution_source,
                result_consumer,
                forked_mode,
            )?;
            match res.as_i32() {
                Some(res) => summed += res,
                None => return Ok(res),
            }
        }

        Ok(R::new(summed))
    }

    /// Resolve redirects and forks and turn every selected action into a
    /// thread-safe future. No parsed context is retained in the returned plan.
    #[cfg(feature = "async")]
    pub(crate) fn prepare_async(
        &self,
        source: Arc<S>,
        result_consumer: &dyn ResultConsumer<S, R>,
    ) -> Result<AsyncExecution<R>, CommandSyntaxError>
    where
        S: Send + Sync + 'static,
        R: Send + 'static,
    {
        if self.modifiers.is_empty() {
            return Ok(AsyncExecution::new(
                vec![Self::prepare_executable_async(
                    self.executable.clone(),
                    source,
                )],
                false,
            ));
        }

        let mut forked_mode = false;
        let mut current_sources = vec![source];

        for modifier in &self.modifiers {
            forked_mode |= modifier.is_forked();

            let mut next_sources = Vec::new();
            for source_to_run in current_sources {
                let sources = Self::run_modifier(
                    modifier.clone(),
                    source_to_run,
                    result_consumer,
                    forked_mode,
                )?;
                next_sources.extend(sources);
            }
            if next_sources.is_empty() {
                return Ok(AsyncExecution::empty(forked_mode));
            }
            current_sources = next_sources;
        }

        let commands = current_sources
            .into_iter()
            .map(|execution_source| {
                Self::prepare_executable_async(self.executable.clone(), execution_source)
            })
            .collect();

        Ok(AsyncExecution::new(commands, forked_mode))
    }

    #[cfg(feature = "async")]
    fn prepare_executable_async(
        executable: Rc<CommandContext<S, R>>,
        source: Arc<S>,
    ) -> CommandFuture<R>
    where
        S: Send + Sync + 'static,
        R: Send + 'static,
    {
        let context = executable.copy_for(source);
        if let Some(command) = &executable.async_command {
            return command(&context);
        }

        let command = executable
            .command
            .as_ref()
            .expect("async execution context must contain a command");
        let result = command(&context);
        Box::pin(std::future::ready(result))
    }

    pub fn stage(&self) -> Stage {
        if self.modifiers.is_empty() {
            Stage::Execute
        } else {
            Stage::Modify
        }
    }

    pub fn top_context(&self) -> Rc<CommandContext<S, R>> {
        self.modifiers
            .first()
            .cloned()
            .unwrap_or_else(|| self.executable.clone())
    }

    pub fn next_stage(&mut self) -> Option<Rc<ContextChain<S, R>>> {
        let modifier_count = self.modifiers.len();
        if modifier_count == 0 {
            return None;
        }

        if self.next_stage_cache.is_none() {
            self.next_stage_cache = Some(Rc::new(ContextChain::new(
                self.modifiers[1..].to_vec(),
                self.executable.clone(),
            )));
        }

        self.next_stage_cache.clone()
    }
}

pub enum Stage {
    Modify,
    Execute,
}

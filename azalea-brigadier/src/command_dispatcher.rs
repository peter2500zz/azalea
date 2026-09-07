#![allow(clippy::type_complexity)]

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    ptr,
    rc::Rc,
    sync::Arc,
};

use parking_lot::RwLock;

#[cfg(feature = "async")]
use crate::async_execution::AsyncExecution;
use crate::{
    builder::IntoCommandNode,
    context::{CommandContextBuilder, CommandContextRef, ContextChain},
    errors::{BuiltInError, CommandError, CommandResultTrait, CommandSyntaxError},
    parse_results::ParseResults,
    result_consumer::{DefaultResultConsumer, ResultConsumer},
    string_reader::StringReader,
    suggestion::{Suggestions, SuggestionsBuilder},
    tree::CommandNode,
};

/// Whether the reader is positioned at a non-empty run of terminal ASCII
/// separators. Brigadier uses an ASCII space as its argument separator; keep
/// tabs and other whitespace out of this rule so their existing semantics do
/// not change accidentally.
fn is_terminal_separator(reader: &StringReader) -> bool {
    reader.can_read()
        && reader.peek() == ' '
        && reader.remaining().chars().all(|character| character == ' ')
}

/// The root of the command tree. You need to make this to register commands.
///
/// ```
/// # use azalea_brigadier::prelude::*;
/// # struct CommandSource;
/// let mut subject = CommandDispatcher::<CommandSource>::new();
/// ```
pub struct CommandDispatcher<S, R = i32>
where
    Self: Sync + Send,
{
    pub root: Arc<RwLock<CommandNode<S, R>>>,
    consumer: Box<dyn ResultConsumer<S, R> + Send + Sync>,
}

impl<S, R: CommandResultTrait> CommandDispatcher<S, R> {
    pub fn new() -> Self {
        Self {
            root: Arc::new(RwLock::new(CommandNode::default())),
            consumer: Box::new(DefaultResultConsumer),
        }
    }

    /// Add a new node to the root.
    ///
    /// ```
    /// # use azalea_brigadier::prelude::*;
    /// # let mut subject = CommandDispatcher::<()>::new();
    /// subject.register(literal("foo").executes(|_| -> CommandResult { Ok(42) }));
    /// ```
    pub fn register(&mut self, node: impl IntoCommandNode<S, R>) -> Arc<RwLock<CommandNode<S, R>>> {
        let build = Arc::new(RwLock::new(node.into_node()));
        self.root.write().add_child(&build);
        build
    }

    pub fn parse(&self, command: StringReader, source: S) -> ParseResults<'_, S, R> {
        let source = Arc::new(source);

        let context = CommandContextBuilder::new(self, source, self.root.clone(), command.cursor());
        self.parse_nodes(&self.root, &command, context).unwrap()
    }

    fn parse_nodes<'a>(
        &'a self,
        node: &Arc<RwLock<CommandNode<S, R>>>,
        original_reader: &StringReader,
        context_so_far: CommandContextBuilder<'a, S, R>,
    ) -> Result<ParseResults<'a, S, R>, CommandSyntaxError> {
        let source = context_so_far.source.clone();
        #[allow(clippy::mutable_key_type)] // this is fine because we don't mutate the key
        let mut errors = HashMap::<Rc<CommandNode<S, R>>, CommandSyntaxError>::new();
        let mut potentials: Vec<ParseResults<S, R>> = vec![];
        let cursor = original_reader.cursor();

        for child in node.read().get_relevant_nodes(&mut original_reader.clone()) {
            if !child.read().can_use(&source) {
                continue;
            }
            let mut context = context_so_far.clone();
            let mut reader = original_reader.clone();
            let child_start = reader.cursor();

            let parse_with_context_result =
                child.read().parse_with_context(&mut reader, &mut context);
            if let Err(ex) = parse_with_context_result {
                // Store the error as it came. Wrapping it in
                // `DispatcherParseException { message: ex.message() }` doubles the
                // position and `<--[HERE]` marker: `message()` appends them, and the
                // outer error appends them again over the same reader. Upstream
                // Brigadier keeps the original here too — it only reaches for
                // `DISPATCHER_PARSE_EXCEPTION` when a non-syntax error escapes.
                errors.insert(Rc::new((*child.read()).clone()), ex);
                reader.cursor = cursor;
                continue;
            }
            if reader.can_read() && reader.peek() != ' ' {
                errors.insert(
                    Rc::new((*child.read()).clone()),
                    BuiltInError::DispatcherExpectedArgumentSeparator.create_with_context(&reader),
                );
                reader.cursor = cursor;
                continue;
            }

            context.with_command(&child.read().command);
            #[cfg(feature = "async")]
            context.with_async_command(&child.read().async_command);

            // A trailing separator without another token is not an argument.
            // Keep an executable non-redirect node as the selected candidate,
            // but do not let a required child consume a whitespace-only tail
            // (in particular, a greedy string must not turn it into a value).
            // Redirects deliberately keep their existing recursion: completion
            // relies on the target context even when the next token is empty.
            if child.read().redirect.is_none()
                && reader.cursor > child_start
                && is_terminal_separator(&reader)
            {
                if child.read().has_command() {
                    reader.cursor = reader.total_length();
                }
                potentials.push(ParseResults {
                    context,
                    reader,
                    exceptions: HashMap::new(),
                });
                continue;
            }

            if reader.can_read_length(if child.read().redirect.is_none() {
                2
            } else {
                1
            }) {
                reader.skip();
                match &child.read().redirect {
                    Some(redirect) => {
                        let child_context = CommandContextBuilder::new(
                            self,
                            source,
                            redirect.clone(),
                            reader.cursor,
                        );
                        let parse = self
                            .parse_nodes(redirect, &reader, child_context)
                            .expect("Parsing nodes failed");
                        context.with_child(Rc::new(parse.context));
                        return Ok(ParseResults {
                            context,
                            reader: parse.reader,
                            exceptions: parse.exceptions,
                        });
                    }
                    _ => {
                        let parse = self
                            .parse_nodes(&child, &reader, context)
                            .expect("Parsing nodes failed");
                        potentials.push(parse);
                    }
                }
            } else {
                potentials.push(ParseResults {
                    context,
                    reader,
                    exceptions: HashMap::new(),
                });
            }
        }

        if !potentials.is_empty() {
            if potentials.len() > 1 {
                potentials.sort_by(|a, b| {
                    if !a.reader.can_read() && b.reader.can_read() {
                        return Ordering::Less;
                    };
                    if a.reader.can_read() && !b.reader.can_read() {
                        return Ordering::Greater;
                    };
                    if a.exceptions.is_empty() && !b.exceptions.is_empty() {
                        return Ordering::Less;
                    };
                    if !a.exceptions.is_empty() && b.exceptions.is_empty() {
                        return Ordering::Greater;
                    };
                    Ordering::Equal
                });
            }
            let best_potential = potentials.into_iter().next().unwrap();
            return Ok(best_potential);
        }

        Ok(ParseResults {
            context: context_so_far,
            reader: original_reader.clone(),
            exceptions: errors,
        })
    }

    /// Parse and execute the command using the given input and context.
    ///
    /// The number returned depends on the command, and may not be of
    /// significance.
    ///
    /// This is a shortcut for `Self::parse` and `Self::execute_parsed`.
    pub fn execute(&self, input: impl Into<StringReader>, source: S) -> Result<R, CommandError> {
        let input = input.into();

        let parse = self.parse(input, source);
        self.execute_parsed(parse)
    }

    pub fn add_paths(
        node: Arc<RwLock<CommandNode<S, R>>>,
        result: &mut Vec<Vec<Arc<RwLock<CommandNode<S, R>>>>>,
        parents: Vec<Arc<RwLock<CommandNode<S, R>>>>,
    ) {
        let mut current = parents;
        current.push(node.clone());
        result.push(current.clone());

        for child in node.read().children.values() {
            Self::add_paths(child.clone(), result, current.clone());
        }
    }

    pub fn get_path(&self, target: CommandNode<S, R>) -> Vec<String> {
        let rc_target = Arc::new(RwLock::new(target));
        let mut nodes: Vec<Vec<Arc<RwLock<CommandNode<S, R>>>>> = Vec::new();
        Self::add_paths(self.root.clone(), &mut nodes, vec![]);

        for list in nodes {
            if *list.last().expect("Nothing in list").read() == *rc_target.read() {
                let mut result: Vec<String> = Vec::with_capacity(list.len());
                for node in list {
                    if !Arc::ptr_eq(&node, &self.root) {
                        result.push(node.read().name().to_owned());
                    }
                }
                return result;
            }
        }
        vec![]
    }

    pub fn find_node(&self, path: &[&str]) -> Option<Arc<RwLock<CommandNode<S, R>>>> {
        let mut node = self.root.clone();
        for name in path {
            match node.clone().read().child(name) {
                Some(child) => {
                    node = child;
                }
                _ => {
                    return None;
                }
            };
        }
        Some(node)
    }

    /// Executes a given pre-parsed command.
    pub fn execute_parsed(&self, parse: ParseResults<S, R>) -> Result<R, CommandError> {
        if parse.reader.can_read() {
            return Err(CommandError::from(if parse.exceptions.len() == 1 {
                parse.exceptions.values().next().unwrap().clone()
            } else if parse.context.range.is_empty() {
                BuiltInError::DispatcherUnknownCommand.create_with_context(&parse.reader)
            } else {
                BuiltInError::DispatcherUnknownArgument.create_with_context(&parse.reader)
            }));
        }

        let command = parse.reader.string();
        let original = CommandContextRef::new(parse.context.build(command));
        let flat_context = ContextChain::try_flatten(original.clone());
        let Some(flat_context) = flat_context else {
            self.consumer.on_command_complete(original, false, 0);
            return Err(CommandError::from(
                BuiltInError::DispatcherUnknownCommand.create_with_context(&parse.reader),
            ));
        };

        flat_context.execute_all(original.source.clone(), self.consumer.as_ref())
    }

    /// Parse an input and prepare a runtime-agnostic asynchronous execution.
    ///
    /// This method is synchronous: parsing, redirect modifiers, and the calls
    /// to command closures happen before it returns. The returned plan contains
    /// only `Send + 'static` futures and may be moved to a multi-threaded
    /// executor. Asynchronous handlers receive an owned [`Arc`] to their
    /// [`CommandContext`](crate::context::CommandContext), so the future may
    /// retain the complete context across `.await` points.
    ///
    /// [`Arc`]: std::sync::Arc
    #[cfg(feature = "async")]
    pub fn prepare_async(
        &self,
        input: impl Into<StringReader>,
        source: S,
    ) -> Result<AsyncExecution<R>, CommandError>
    where
        S: Send + Sync + 'static,
        R: Send + 'static,
    {
        let parse = self.parse(input.into(), source);
        self.prepare_parsed_async(parse)
    }

    /// Prepare an asynchronous execution from an existing parse result.
    #[cfg(feature = "async")]
    pub fn prepare_parsed_async(
        &self,
        parse: ParseResults<S, R>,
    ) -> Result<AsyncExecution<R>, CommandError>
    where
        S: Send + Sync + 'static,
        R: Send + 'static,
    {
        if parse.reader.can_read() {
            return Err(CommandError::from(if parse.exceptions.len() == 1 {
                parse.exceptions.values().next().unwrap().clone()
            } else if parse.context.range.is_empty() {
                BuiltInError::DispatcherUnknownCommand.create_with_context(&parse.reader)
            } else {
                BuiltInError::DispatcherUnknownArgument.create_with_context(&parse.reader)
            }));
        }

        let command = parse.reader.string();
        let original = CommandContextRef::new(parse.context.build(command));
        let flat_context = ContextChain::try_flatten_async(original.clone());
        let Some(flat_context) = flat_context else {
            self.consumer.on_command_complete(original, false, 0);
            return Err(CommandError::from(
                BuiltInError::DispatcherUnknownCommand.create_with_context(&parse.reader),
            ));
        };

        flat_context.prepare_async(original.source.clone(), self.consumer.as_ref())
    }

    /// Parse, prepare, and asynchronously execute an input.
    ///
    /// Preparation happens when the returned future is first polled. This
    /// keeps parsing, redirect modifiers, and construction of runtime-bound
    /// futures on the executor when this method is awaited inside a task.
    #[cfg(feature = "async")]
    pub fn execute_async<'a>(
        &'a self,
        input: impl Into<StringReader>,
        source: S,
    ) -> impl std::future::Future<Output = Result<R, CommandError>> + Send + 'a
    where
        S: Send + Sync + 'static,
        R: Send + 'static,
    {
        let input = input.into();
        async move {
            let execution = self.prepare_async(input, source)?;
            execution.execute().await
        }
    }

    pub fn get_all_usage(
        &self,
        node: &CommandNode<S, R>,
        source: &S,
        restricted: bool,
    ) -> Vec<String> {
        let mut result = vec![];
        self.get_all_usage_recursive(node, source, &mut result, "", restricted);
        result
    }

    fn get_all_usage_recursive(
        &self,
        node: &CommandNode<S, R>,
        source: &S,
        result: &mut Vec<String>,
        prefix: &str,
        restricted: bool,
    ) {
        if restricted && !node.can_use(source) {
            return;
        }
        if node.has_command() {
            result.push(prefix.to_owned());
        }
        match &node.redirect {
            Some(redirect) => {
                let redirect = if ptr::eq(redirect.data_ptr(), self.root.data_ptr()) {
                    "...".to_owned()
                } else {
                    format!("-> {}", redirect.read().usage_text())
                };
                if prefix.is_empty() {
                    result.push(format!("{} {redirect}", node.usage_text()));
                } else {
                    result.push(format!("{prefix} {redirect}"));
                }
            }
            _ => {
                for child in node.children.values() {
                    let child = child.read();
                    self.get_all_usage_recursive(
                        &child,
                        source,
                        result,
                        if prefix.is_empty() {
                            child.usage_text()
                        } else {
                            format!("{prefix} {}", child.usage_text())
                        }
                        .as_str(),
                        restricted,
                    );
                }
            }
        }
    }

    /// Gets the possible executable commands from a specified node.
    ///
    /// You may use [`Self::root`] as a target to get usage data for the entire
    /// command tree.
    pub fn get_smart_usage(
        &self,
        node: &CommandNode<S, R>,
        source: &S,
    ) -> Vec<(Arc<RwLock<CommandNode<S, R>>>, String)> {
        let mut result = Vec::new();

        let optional = node.has_command();
        for child in node.children.values() {
            let usage = self.get_smart_usage_recursive(&child.read(), source, optional, false);
            if let Some(usage) = usage {
                result.push((child.clone(), usage));
            }
        }

        result
    }

    fn get_smart_usage_recursive(
        &self,
        node: &CommandNode<S, R>,
        source: &S,
        optional: bool,
        deep: bool,
    ) -> Option<String> {
        if !node.can_use(source) {
            return None;
        }

        let this = if optional {
            format!("[{}]", node.usage_text())
        } else {
            node.usage_text()
        };
        let child_optional = node.has_command();
        let open = if child_optional { "[" } else { "(" };
        let close = if child_optional { "]" } else { ")" };

        if deep {
            return Some(this);
        }

        if let Some(redirect) = &node.redirect {
            let redirect = if ptr::eq(redirect.data_ptr(), self.root.data_ptr()) {
                "...".to_owned()
            } else {
                format!("-> {}", redirect.read().usage_text())
            };
            return Some(format!("{this} {redirect}"));
        }

        let children = node
            .children
            .values()
            .filter(|child| child.read().can_use(source))
            .collect::<Vec<_>>();
        match children.len().cmp(&1) {
            Ordering::Less => {}
            Ordering::Equal => {
                let usage = self.get_smart_usage_recursive(
                    &children[0].read(),
                    source,
                    child_optional,
                    child_optional,
                );
                if let Some(usage) = usage {
                    return Some(format!("{this} {usage}"));
                }
            }
            Ordering::Greater => {
                let mut child_usage = HashSet::new();
                for child in &children {
                    let usage =
                        self.get_smart_usage_recursive(&child.read(), source, child_optional, true);
                    if let Some(usage) = usage {
                        child_usage.insert(usage);
                    }
                }
                match child_usage.len().cmp(&1) {
                    Ordering::Less => {}
                    Ordering::Equal => {
                        let usage = child_usage.into_iter().next().unwrap();
                        let usage = if child_optional {
                            format!("[{usage}]")
                        } else {
                            usage
                        };
                        return Some(format!("{this} {usage}"));
                    }
                    Ordering::Greater => {
                        let mut builder = String::new();
                        builder.push_str(open);
                        let mut count = 0;
                        for child in children {
                            if count > 0 {
                                builder.push('|');
                            }
                            builder.push_str(&child.read().usage_text());
                            count += 1;
                        }
                        if count > 0 {
                            builder.push_str(close);
                            return Some(format!("{this} {builder}"));
                        }
                    }
                }
            }
        }

        Some(this)
    }

    pub fn get_completion_suggestions(parse: ParseResults<S, R>) -> Suggestions {
        let cursor = parse.reader.total_length();
        Self::get_completion_suggestions_with_cursor(parse, cursor)
    }

    pub fn get_completion_suggestions_with_cursor(
        parse: ParseResults<S, R>,
        cursor: usize,
    ) -> Suggestions {
        let context = parse.context;

        let node_before_cursor = context.find_suggestion_context(cursor);
        let parent = node_before_cursor.parent;
        let start = usize::min(node_before_cursor.start_pos, cursor);

        let full_input = parse.reader.string();
        let truncated_input = full_input[..cursor].to_string();
        let truncated_input_lowercase = truncated_input.to_lowercase();

        let mut all_suggestions = Vec::new();
        for node in parent.read().children.values() {
            let suggestions = node.read().list_suggestions(
                context.build(&truncated_input),
                SuggestionsBuilder::new_with_lowercase(
                    &truncated_input,
                    &truncated_input_lowercase,
                    start,
                )
                .ignore_terminal_spaces(),
            );
            all_suggestions.push(suggestions);
        }

        Suggestions::merge(full_input, &all_suggestions)
    }
}

impl<S, R: CommandResultTrait> Default for CommandDispatcher<S, R> {
    fn default() -> Self {
        Self::new()
    }
}

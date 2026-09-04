use std::{
    collections::{BTreeMap, HashMap},
    fmt::{self, Debug},
    hash::{Hash, Hasher},
    ptr,
    sync::Arc,
};

use parking_lot::RwLock;

#[cfg(feature = "async")]
use crate::async_execution::CommandFuture;
use crate::{
    builder::{
        argument_builder::ArgumentBuilderType, literal_argument_builder::Literal,
        required_argument_builder::Argument,
    },
    context::{CommandContext, CommandContextBuilder, ParsedArgument, StringRange},
    errors::{BuiltInError, CommandError, CommandSyntaxError},
    modifier::RedirectModifier,
    string_reader::StringReader,
    suggestion::{Suggestions, SuggestionsBuilder},
};

pub type Command<S, R> =
    Option<Arc<dyn Fn(&CommandContext<S, R>) -> Result<R, CommandError> + Send + Sync>>;

#[cfg(feature = "async")]
pub type AsyncCommand<S, R> =
    Option<Arc<dyn Fn(&CommandContext<S, R>) -> CommandFuture<R> + Send + Sync>>;

/// An ArgumentBuilder that has been built.
#[non_exhaustive]
pub struct CommandNode<S, R = i32> {
    pub value: ArgumentBuilderType<S, R>,
    /// What this node does, in a few words, for a completion menu to show
    /// alongside the candidate that completes it.
    ///
    /// Literal nodes have nowhere else to carry one: their suggestions are
    /// built from the literal text alone, so without this every literal
    /// suggestion arrives with `tooltip: None`. Argument nodes hand the
    /// tooltip out through their [`SuggestionProvider`] instead, and this
    /// describes the argument itself rather than any one of its values.
    ///
    /// Set it with [`ArgumentBuilder::describe`].
    ///
    /// [`SuggestionProvider`]: crate::suggestion::SuggestionProvider
    /// [`ArgumentBuilder::describe`]: crate::builder::argument_builder::ArgumentBuilder::describe
    pub description: Option<String>,

    // this is a BTreeMap because children need to be ordered when getting command suggestions
    pub children: BTreeMap<String, Arc<RwLock<CommandNode<S, R>>>>,
    pub literals: HashMap<String, Arc<RwLock<CommandNode<S, R>>>>,
    pub arguments: HashMap<String, Arc<RwLock<CommandNode<S, R>>>>,

    pub command: Command<S, R>,
    #[cfg(feature = "async")]
    pub async_command: AsyncCommand<S, R>,
    pub requirement: Arc<dyn Fn(&S) -> bool + Send + Sync>,
    pub redirect: Option<Arc<RwLock<CommandNode<S, R>>>>,
    pub forks: bool,
    pub modifier: Option<Arc<RedirectModifier<S, R>>>,
}

impl<S, R> Clone for CommandNode<S, R> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            description: self.description.clone(),
            children: self.children.clone(),
            literals: self.literals.clone(),
            arguments: self.arguments.clone(),
            command: self.command.clone(),
            #[cfg(feature = "async")]
            async_command: self.async_command.clone(),
            requirement: self.requirement.clone(),
            redirect: self.redirect.clone(),
            forks: self.forks,
            modifier: self.modifier.clone(),
        }
    }
}

impl<S, R> CommandNode<S, R> {
    /// Returns the value as a literal from this command node, assuming it's
    /// already been checked.
    ///
    /// # Panics
    ///
    /// Will panic if this node is not a literal. Consider using a match
    /// statement instead.
    pub fn literal(&self) -> &Literal {
        match self.value {
            ArgumentBuilderType::Literal(ref literal) => literal,
            _ => panic!("CommandNode::literal() called on non-literal node"),
        }
    }
    /// Returns the value as an argument from this command node, assuming it's
    /// already been checked.
    ///
    /// # Panics
    ///
    /// Will panic if this node is not an argument. Consider using a match
    /// statement instead.
    pub fn argument(&self) -> &Argument<S, R> {
        match self.value {
            ArgumentBuilderType::Argument(ref argument) => argument,
            _ => panic!("CommandNode::argument() called on non-argument node"),
        }
    }

    pub fn get_relevant_nodes(
        &self,
        input: &mut StringReader,
    ) -> Vec<Arc<RwLock<CommandNode<S, R>>>> {
        let literals = &self.literals;

        if literals.is_empty() {
            self.arguments.values().cloned().collect()
        } else {
            let cursor = input.cursor();
            while input.can_read() && input.peek() != ' ' {
                input.skip();
            }
            // `cursor` is a byte offset, so slice by bytes rather than stepping
            // a character iterator with it.
            let text = &input.string()[cursor..input.cursor()];
            let literal = literals.get(text).cloned();
            input.cursor = cursor;
            if let Some(literal) = literal {
                vec![literal]
            } else {
                self.arguments.values().cloned().collect()
            }
        }
    }

    pub fn can_use(&self, source: &S) -> bool {
        (self.requirement)(source)
    }

    /// Whether this node has an action for either execution mode.
    pub fn has_command(&self) -> bool {
        #[cfg(feature = "async")]
        {
            self.command.is_some() || self.async_command.is_some()
        }
        #[cfg(not(feature = "async"))]
        {
            self.command.is_some()
        }
    }

    pub fn add_child(&mut self, node: &Arc<RwLock<CommandNode<S, R>>>) {
        let child = self.children.get(node.read().name());
        if let Some(child) = child {
            // We've found something to merge onto
            #[cfg(not(feature = "async"))]
            if let Some(command) = &node.read().command {
                child.write().command = Some(command.clone());
            }
            #[cfg(feature = "async")]
            {
                let incoming = node.read();
                if incoming.command.is_some() || incoming.async_command.is_some() {
                    let mut existing = child.write();
                    existing.command.clone_from(&incoming.command);
                    existing.async_command.clone_from(&incoming.async_command);
                }
            }
            // Same rule as the command above: the incoming node wins, but only
            // where it actually says something. Registering `foo bar` and then
            // `foo baz` mustn't wipe the description `foo` was given the first
            // time round.
            if let Some(description) = &node.read().description {
                child.write().description = Some(description.clone());
            }
            for grandchild in node.read().children.values() {
                child.write().add_child(grandchild);
            }
        } else {
            self.children
                .insert(node.read().name().to_owned(), node.clone());
            match &node.read().value {
                ArgumentBuilderType::Literal(literal) => {
                    self.literals.insert(literal.value.clone(), node.clone());
                }
                ArgumentBuilderType::Argument(argument) => {
                    self.arguments.insert(argument.name.clone(), node.clone());
                }
            }
        }
    }

    pub fn name(&self) -> &str {
        match &self.value {
            ArgumentBuilderType::Argument(argument) => &argument.name,
            ArgumentBuilderType::Literal(literal) => &literal.value,
        }
    }

    pub fn usage_text(&self) -> String {
        match &self.value {
            ArgumentBuilderType::Argument(argument) => format!("<{}>", argument.name),
            ArgumentBuilderType::Literal(literal) => literal.value.to_owned(),
        }
    }

    pub fn child(&self, name: &str) -> Option<Arc<RwLock<CommandNode<S, R>>>> {
        self.children.get(name).cloned()
    }

    pub fn parse_with_context(
        &self,
        reader: &mut StringReader,
        context_builder: &mut CommandContextBuilder<S, R>,
    ) -> Result<(), CommandSyntaxError> {
        match self.value {
            ArgumentBuilderType::Argument(ref argument) => {
                let start = reader.cursor();
                let result = argument.parse(reader)?;
                let parsed = ParsedArgument {
                    range: StringRange::between(start, reader.cursor()),
                    result,
                };

                context_builder.with_argument(&argument.name, parsed.clone());
                context_builder.with_node(Arc::new(RwLock::new(self.clone())), parsed.range);

                Ok(())
            }
            ArgumentBuilderType::Literal(ref literal) => {
                let start = reader.cursor();
                let end = self.parse(reader);

                if let Some(end) = end {
                    context_builder.with_node(
                        Arc::new(RwLock::new(self.clone())),
                        StringRange::between(start, end),
                    );
                    return Ok(());
                }

                Err(BuiltInError::LiteralIncorrect {
                    expected: literal.value.clone(),
                }
                .create_with_context(reader))
            }
        }
    }

    fn parse(&self, reader: &mut StringReader) -> Option<usize> {
        match self.value {
            ArgumentBuilderType::Argument(_) => {
                panic!("Can't parse argument.")
            }
            ArgumentBuilderType::Literal(ref literal) => {
                let start = reader.cursor();
                if reader.can_read_length(literal.value.len()) {
                    let end = start + literal.value.len();
                    if reader
                        .string()
                        .get(start..end)
                        .expect("Couldn't slice reader correctly?")
                        == literal.value
                    {
                        reader.cursor = end;
                        if !reader.can_read() || reader.peek() == ' ' {
                            return Some(end);
                        } else {
                            reader.cursor = start;
                        }
                    }
                }
            }
        }
        None
    }

    pub fn list_suggestions(
        &self,
        context: CommandContext<S, R>,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        match &self.value {
            ArgumentBuilderType::Literal(literal) => {
                if literal
                    .value
                    .to_lowercase()
                    .starts_with(builder.remaining_lowercase())
                {
                    match &self.description {
                        Some(description) => builder
                            .suggest_with_tooltip(&literal.value, description.clone())
                            .build(),
                        None => builder.suggest(&literal.value).build(),
                    }
                } else {
                    Suggestions::default()
                }
            }
            ArgumentBuilderType::Argument(argument) => argument.list_suggestions(context, builder),
        }
    }
}

impl<S, R> Debug for CommandNode<S, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("CommandNode");
        // debug.field("value", &self.value);
        debug.field("children", &self.children);
        debug.field("command", &self.command.is_some());
        #[cfg(feature = "async")]
        debug.field("async_command", &self.async_command.is_some());
        // debug.field("requirement", &self.requirement);
        debug.field("redirect", &self.redirect);
        debug.field("forks", &self.forks);
        // debug.field("modifier", &self.modifier);
        debug.finish()
    }
}

impl<S, R> Default for CommandNode<S, R> {
    fn default() -> Self {
        Self {
            value: ArgumentBuilderType::Literal(Literal::default()),
            description: None,

            children: BTreeMap::new(),
            literals: HashMap::new(),
            arguments: HashMap::new(),

            command: None,
            #[cfg(feature = "async")]
            async_command: None,
            requirement: Arc::new(|_| true),
            redirect: None,
            forks: false,
            modifier: None,
        }
    }
}

impl<S, R> Hash for CommandNode<S, R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // hash the children
        for (k, v) in &self.children {
            k.hash(state);
            v.read().hash(state);
        }
        // i hope this works because if doesn't then that'll be a problem
        ptr::hash(&self.command, state);
        #[cfg(feature = "async")]
        ptr::hash(&self.async_command, state);
    }
}

impl<S, R> PartialEq for CommandNode<S, R> {
    fn eq(&self, other: &Self) -> bool {
        if self.children.len() != other.children.len() {
            return false;
        }
        for (k, v) in &self.children {
            let other_child = other.children.get(k).unwrap();
            if !Arc::ptr_eq(v, other_child) {
                return false;
            }
        }

        match &self.command {
            Some(selfexecutes) => {
                // idk how to do this better since we can't compare `dyn Fn`s
                match &other.command {
                    Some(otherexecutes) => {
                        if !Arc::ptr_eq(selfexecutes, otherexecutes) {
                            return false;
                        }
                    }
                    _ => {
                        return false;
                    }
                }
            }
            _ => {
                if other.command.is_some() {
                    return false;
                }
            }
        }
        #[cfg(feature = "async")]
        match &self.async_command {
            Some(self_executes) => match &other.async_command {
                Some(other_executes) => {
                    if !Arc::ptr_eq(self_executes, other_executes) {
                        return false;
                    }
                }
                None => return false,
            },
            None => {
                if other.async_command.is_some() {
                    return false;
                }
            }
        }
        true
    }
}
impl<S, R> Eq for CommandNode<S, R> {}

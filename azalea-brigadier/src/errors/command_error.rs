use std::fmt;

use super::CommandSyntaxError;

/// An error produced while executing a command.
///
/// Syntax errors are kept structured so callers can inspect the parser
/// failure. Errors returned by command actions are type-erased at the command
/// tree boundary; this lets one dispatcher contain actions with different
/// application error types while preserving their source chain for reporting
/// libraries such as `anyhow` and `eyre`.
#[non_exhaustive]
pub enum CommandError {
    Syntax(CommandSyntaxError),
    Execution(BoxCommandError),
}

/// The error object accepted by the command execution boundary.
pub type BoxCommandError = Box<dyn std::error::Error + Send + Sync + 'static>;

impl CommandError {
    /// Wrap an application error returned by a command action.
    pub fn from_execution<E>(error: E) -> Self
    where
        E: Into<BoxCommandError>,
    {
        Self::from_boxed(error.into())
    }

    /// Convert an erased error into the appropriate command error variant.
    ///
    /// The downcasts retain the structured syntax variant when an existing
    /// `CommandSyntaxError` (or `CommandError`) is passed through a generic
    /// fallible command adapter.
    pub fn from_boxed(error: BoxCommandError) -> Self {
        let error = match error.downcast::<Self>() {
            Ok(error) => return *error,
            Err(error) => error,
        };

        match error.downcast::<CommandSyntaxError>() {
            Ok(error) => Self::Syntax(*error),
            Err(error) => Self::Execution(error),
        }
    }

    /// Return the syntax error, when this is a parser failure.
    pub fn syntax(&self) -> Option<&CommandSyntaxError> {
        match self {
            Self::Syntax(error) => Some(error),
            Self::Execution(_) => None,
        }
    }

    /// Return the application error, when this is an execution failure.
    pub fn execution(&self) -> Option<&(dyn std::error::Error + Send + Sync + 'static)> {
        match self {
            Self::Syntax(_) => None,
            Self::Execution(error) => Some(error.as_ref()),
        }
    }

    /// Return the application error, when this is an execution failure.
    pub fn execution_error(&self) -> Option<&(dyn std::error::Error + Send + Sync + 'static)> {
        self.execution()
    }

    /// Render the error using the same convenient API as
    /// [`CommandSyntaxError::message`].
    pub fn message(&self) -> String {
        self.to_string()
    }
}

impl From<CommandSyntaxError> for CommandError {
    fn from(error: CommandSyntaxError) -> Self {
        Self::Syntax(error)
    }
}

impl From<BoxCommandError> for CommandError {
    fn from(error: BoxCommandError) -> Self {
        Self::from_boxed(error)
    }
}

impl fmt::Debug for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(error) => formatter.debug_tuple("Syntax").field(error).finish(),
            Self::Execution(error) => formatter.debug_tuple("Execution").field(error).finish(),
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(error) => error.fmt(formatter),
            Self::Execution(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Syntax(error) => Some(error),
            Self::Execution(error) => Some(error.as_ref()),
        }
    }
}

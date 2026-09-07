mod builtin_errors;
mod command_error;
mod command_syntax_error;

pub use builtin_errors::BuiltInError;
pub use command_error::{BoxCommandError, CommandError, CommandResult};
pub use command_syntax_error::CommandSyntaxError;

pub trait CommandResultTrait {
    fn new(i: i32) -> Self;
    fn as_i32(&self) -> Option<i32>;
}
impl CommandResultTrait for i32 {
    fn new(i: i32) -> Self {
        i
    }
    fn as_i32(&self) -> Option<i32> {
        Some(*self)
    }
}

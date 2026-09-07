use crate::context::CommandContextRef;

pub trait ResultConsumer<S, R> {
    fn on_command_complete(&self, context: CommandContextRef<S, R>, success: bool, result: i32);
}

pub struct DefaultResultConsumer;
impl<S, R> ResultConsumer<S, R> for DefaultResultConsumer {
    fn on_command_complete(&self, _context: CommandContextRef<S, R>, _success: bool, _result: i32) {
    }
}

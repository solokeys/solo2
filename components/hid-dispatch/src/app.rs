
pub use crate::types::{Message, AppResponse as Response,Error};
pub use crate::command::Command;

/// trait interface for a CTAPHID application.
/// The application chooses which commands to register to, and will be called upon
/// when the commands are received in the CTAPHID layer.  Only one application can be registered to a particular command.
pub trait App {

    /// Define which CTAPHID commands to register to.
    fn commands(&self) -> &'static [Command];

    /// Application is called here when one of it's register commands occurs.
    /// Application must put response in @message, or decide to return an error.
    fn call(&mut self, command: Command, message: &mut Message) -> Response;
}

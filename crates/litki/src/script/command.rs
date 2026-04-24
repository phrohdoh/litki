use bevy::prelude::*;
use jinme::prelude::*;


pub type BoxedCommand = Box<dyn Command>;

pub trait Command: Send + Sync + 'static {
    /// Validate command preconditions before execution
    ///
    /// This is called before `Command::execute` and allows early error detection.
    /// For example:
    /// - Check entity exists
    /// - Check required components are present
    /// - Validate argument ranges
    ///
    /// # Arguments
    /// * `world` - Read-only access to the ECS world
    ///
    /// # Returns
    /// * `Ok(())` - Command is valid and can execute
    /// * `Err(String)` - Command cannot execute; return reason
    fn validate(&self, world: &World) -> Result<(), String>;

    /// Execute this command with mutable world access
    ///
    /// This is called only if `Command::validate` returned `Result::Ok`.
    /// Can mutate world state (spawn entities, add components, etc).
    ///
    /// # Arguments
    /// * `world` - Mutable access to the ECS world
    ///
    /// # Returns
    /// * `CommandResult::Success(value)` - Execution succeeded
    /// * `CommandResult::Error(value)` - Execution failed
    /// * `CommandResult::Pending` - Execution deferred
    fn execute(&self, world: &mut World) -> CommandResult;

    fn execute_entity(&self, entity: &mut EntityWorldMut) -> CommandResult {
        todo!()
    }

    fn boxed(self) -> BoxedCommand
    where
        Self: Sized,
    {
        Box::new(self)
    }
}

#[derive(Clone, Debug)]
pub enum CommandResult {
    Success(PtrValue),
    Error(PtrValue),
    Pending,
}

pub fn command_fn(
    validate: impl Fn(&World) -> Result<(), String> + Send + Sync + 'static,
    execute: impl Fn(&mut World) -> CommandResult + Send + Sync + 'static,
) -> impl Command {
    struct FnCommand<V, E> {
        validate_fn: V,
        execute_fn: E,
    }

    impl<V, E> Command for FnCommand<V, E>
    where
        V: Fn(&World) -> Result<(), String> + Send + Sync + 'static,
        E: Fn(&mut World) -> CommandResult + Send + Sync + 'static,
    {
        fn validate(&self, world: &World) -> Result<(), String> {
            (self.validate_fn)(world)
        }

        fn execute(&self, world: &mut World) -> CommandResult {
            (self.execute_fn)(world)
        }
    }

    FnCommand {
        validate_fn: validate,
        execute_fn: execute,
    }
}

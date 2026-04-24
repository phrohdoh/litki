use std::sync::Arc;
use std::time::Duration;
use bevy::prelude::{Resource, World};
use bevy::log;
use jinme::prelude::{Environment as JinmeEnv, Value};

mod command;
pub use command::{
    command_fn,
    BoxedCommand,
    Command,
    CommandResult,
};

mod command_buffer;
pub use command_buffer::CommandBuffer;

mod command_registry;
pub use command_registry::CommandRegistry;

mod command_factory;
pub use command_factory::{CommandFactory, BoxedCommandFactory, closure_factory};

mod command_promise;
pub use command_promise::{CommandPromise, CommandPromiseResolver};

#[derive(Resource, Clone)]
pub struct Environment(Arc<JinmeEnv>);

impl Environment {
    pub fn inner(&self) -> Arc<JinmeEnv> {
        self.0.clone()
    }
}

impl From<JinmeEnv> for Environment {
    fn from(env: JinmeEnv) -> Self {
        Self(Arc::new(env))
    }
}

impl From<Arc<JinmeEnv>> for Environment {
    fn from(env: Arc<JinmeEnv>) -> Self {
        Self(env)
    }
}

impl std::ops::Deref for Environment {
    type Target = JinmeEnv;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Configuration for command execution timeouts in the REPL
#[derive(Resource, Clone)]
pub struct ReplTimeout(pub Duration);

impl Default for ReplTimeout {
    fn default() -> Self {
        Self(Duration::from_secs(5))
    }
}

/// Configuration for command execution timeouts when commands are called as Clojure functions
#[derive(Resource, Clone)]
pub struct CommandFunctionTimeout(pub Duration);

impl Default for CommandFunctionTimeout {
    fn default() -> Self {
        Self(Duration::from_secs(5))
    }
}

pub fn process_commands(world: &mut World) {
    // Get all queued script commands and their resolvers
    let command_buffer = world.resource_mut::<CommandBuffer>().clone();
    let commands = command_buffer.drain();
    if commands.is_empty() { return; }

    // Process each command
    log::info!("Processing {} script command(s)", commands.len());
    for (command, resolver) in commands {
        // First validate
        match command.validate(world) {
            Ok(()) => {
                // Then only if validation succeeded, execute the command
                match command.execute(world) {
                    CommandResult::Success(value) => {
                        log::info!("✓ {}", value);
                        resolver.resolve(CommandResult::Success(value));
                    }
                    CommandResult::Error(value) => {
                        log::error!("✗ {}", value);
                        resolver.resolve(CommandResult::Error(value));
                    }
                    CommandResult::Pending => {
                        log::info!("⏱ deferred");
                        // For now, resolve as error since re-enqueueing would require mutable access to buffer
                        // TODO: re-enqueue pending commands for retry next frame
                        command_buffer.enqueue_with_resolver(command, resolver);
                        //resolver.resolve(CommandResult::Error(
                        //    Arc::new(Value::string("command pending".to_string()))
                        //));
                    }
                }
            }
            Err(validation_error) => {
                log::error!("✗ Validation failed: {}", validation_error);
                resolver.resolve(CommandResult::Error(
                    Arc::new(Value::string(validation_error.clone()))
                ));
            }
        }
    }
}

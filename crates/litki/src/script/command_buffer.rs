use std::sync::{Arc, Mutex};
use bevy::prelude::*;
use jinme::prelude::*;

use crate::script::{BoxedCommand, CommandPromise, CommandPromiseResolver};

type QueuedCommand = (BoxedCommand, CommandPromiseResolver);

#[derive(Resource, Clone)]
pub struct CommandBuffer {
    commands: Arc<Mutex<Vec<QueuedCommand>>>,
}

impl Default for CommandBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Enqueue a command with a promise resolver
    ///
    /// The command will be executed in the main Bevy thread by the
    /// `process_commands` system, and the resolver will be called with the result.
    ///
    /// # Arguments
    /// * `command` - Command to queue
    /// * `resolver` - Resolver that will be called with the result
    ///
    /// # Thread Safety
    /// Safe to call from any thread (REPL server, etc)
    pub fn enqueue_with_resolver(&self, command: BoxedCommand, resolver: CommandPromiseResolver) {
        self.commands.lock().unwrap().push((command, resolver));
    }

    /// Enqueue a command for fire-and-forget execution (result is not returned)
    ///
    /// Creates an internal resolver but discards the promise, so the caller
    /// cannot wait on the result.
    ///
    /// # Arguments
    /// * `command` - Command to queue
    ///
    /// # Thread Safety
    /// Safe to call from any thread (REPL server, etc)
    pub fn enqueue(&self, command: BoxedCommand) {
        let (_promise, resolver) = CommandPromise::new();
        self.enqueue_with_resolver(command, resolver);
    }

    /// Drain all queued commands (consuming them)
    ///
    /// Called by the `process_commands` system to get commands queued for execution.
    /// Returns an empty vector if no commands are queued.
    ///
    /// # Returns
    /// Vector of all queued (command, resolver) pairs in order, since the last `drain`
    pub fn drain(&self) -> Vec<QueuedCommand> {
        self.commands.lock().unwrap().drain(..).collect()
    }
}

use std::{collections::HashMap, sync::Arc};
use bevy::prelude::*;
use bevy::log;
use jinme::prelude::*;
use parking_lot::RwLock;
use crate::script::{BoxedCommandFactory, CommandBuffer, CommandPromise};

#[derive(Clone, Resource)]
pub struct CommandRegistry {
    factories: Arc<RwLock<HashMap<String, BoxedCommandFactory>>>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            factories: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register(&self, name: impl Into<String>, factory: BoxedCommandFactory) {
        self.factories.write().insert(name.into(), factory);
    }

    /// Execute a command synchronously and return a promise for the result
    /// 
    /// Creates the command via the registered factory, enqueues it with a promise resolver,
    /// and returns the promise that will be resolved when Bevy executes the command.
    /// 
    /// # Arguments
    /// * `name` - The registered command name
    /// * `args` - Command arguments as PtrValues
    /// * `buffer` - The CommandBuffer to enqueue into
    /// 
    /// # Returns
    /// * `Ok(promise)` - Promise that will resolve to the command result
    /// * `Err(msg)` - Error if the command factory is not found or creation failed
    pub fn execute_with_promise(
        &self,
        name: &str,
        args: Vec<PtrValue>,
        buffer: &CommandBuffer,
    ) -> Result<CommandPromise, String> {
        let (promise, resolver) = CommandPromise::new();

        let command = {
            let factories = self.factories.read();
            let factory = factories
                .get(name)
                .ok_or_else(|| format!("Unknown command: '{name}'"))?;
            factory.create(args)
                .map_err(|e| format!("Failed to create command '{name}': {e}"))?
        };

        buffer.enqueue_with_resolver(command, resolver);
        Ok(promise)
    }

    pub fn execute(&self, name: &str, args: Vec<PtrValue>, buffer: &CommandBuffer) {
        self.factories.read().get(name).map(|factory| {
            match factory.create(args) {
                Ok(command) => buffer.enqueue(command),
                Err(e) => log::error!("Failed to create command '{name}': {e}"),
            }
        }).unwrap_or_else(|| log::error!("Unknown command: '{name}'"));
    }

    pub fn list_commands(&self) -> Vec<String> {
        self.factories.read().keys().cloned().collect()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.factories.read().contains_key(name)
    }
}

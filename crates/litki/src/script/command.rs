use bevy::prelude::*;
use bevy::log;
use jinme::value::{Value, PtrValue};
use rand::Rng as _;
use crate::plugins::EntityRegistry;
use crate::{ComponentBuilderRegistry, plugins::StableId};

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
    struct CommandImpl<V, E> {
        validate_fn: V,
        execute_fn: E,
    }

    impl<V, E> Command for CommandImpl<V, E>
    where
        V: Fn(&World) -> Result<(), String> + Send + Sync + 'static,
        E: Fn(&mut World) -> CommandResult + Send + Sync + 'static,
    {
        fn validate(&self, world: &World) -> Result<(), String> { (self.validate_fn)(world) }
        fn execute(&self, world: &mut World) -> CommandResult { (self.execute_fn)(world) }
    }

    CommandImpl {
        validate_fn: validate,
        execute_fn: execute,
    }
}

/*
pub struct SpawnEntityCommand {
    id: Option<StableId>,
    components: Vec<String>,
}

impl SpawnEntityCommand {
    pub fn new(id: Option<StableId>, components: Vec<String>) -> Self {
        Self { id, components }
    }
}

impl Command for SpawnEntityCommand {
    fn validate(&self, _world: &World) -> Result<(), String> {
        // Basic validation - components should be registered
        Ok(())
    }

    fn execute(&self, world: &mut World) -> CommandResult {
        let mut entity = world.spawn_empty();

        // Assign StableId
        let stable_id = self.id.unwrap_or_else(|| {
            let mut rng = rand::thread_rng();
            StableId(rng.r#gen())
        });

        // Add StableId component
        entity.insert(stable_id.clone());

        // Add components using builders
        let component_builder_registry = world.resource_mut::<ComponentBuilderRegistry>();
        for component_name in &self.components {
            if let Some(builder) = component_builder_registry.get(component_name) {
                // Spawn entity and add components
                let mut entity_mut = world.spawn_empty();
                builder.build(&mut entity_mut, Vec::new()).unwrap();
            } else {
                log::error!("Unknown component builder: '{}'", component_name);
                return CommandResult::Error(Value::string_ptr(format!("Unknown component: '{}'", component_name)));
            }
        }

        let entity = entity.id();

        // Register entity in EntityRegistry
        let entity_registry = world.resource::<EntityRegistry>();
        entity_registry.register(stable_id, entity);

        CommandResult::Success(Value::string_ptr(format!("{}", stable_id.0)))
    }
}
*/

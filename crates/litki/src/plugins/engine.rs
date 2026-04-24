use std::{collections::HashMap, sync::{Arc, Mutex}};
use bevy::log;
use bevy::prelude::*;
use jinme::{prelude::*, value::optics as value_optics};
use short_id::{ShortId, ShortIdError};

use crate::{
    AnimationRegistry,
    PtrCommandBuilder,
    ComponentBuilderRegistry,
    TemplateRegistry,
    ComponentTypeRegistry,
    script::{self, CommandBuffer, CommandRegistry, Environment, ReplTimeout, CommandFunctionTimeout}
};

pub struct EnginePlugin;

impl Plugin for EnginePlugin {
    fn build(&self, app: &mut App) {
        // Add systems and resources specific to the game engine here
        app.init_resource::<ComponentBuilderRegistry>()
           .init_resource::<TemplateRegistry>()
           .init_resource::<AnimationRegistry>() // Placeholder for the actual AnimationRegistry resource
           .init_resource::<EntityRegistry>() // Entity registry for stable ID tracking
           .init_resource::<ComponentTypeRegistry>() // Component type registry for queries

           .init_resource::<CommandBuffer>()
           .init_resource::<CommandRegistry>()
           .init_resource::<ReplTimeout>()
           .init_resource::<CommandFunctionTimeout>()

           //.add_message::<TemplateModifiedEvent>()
           .insert_resource(Environment::from(crate::clojure::create_env()))

           .add_systems(Startup, (
                setup_core_builders,
                // bind_execute_command_function,
            ))
           .add_systems(Update, (script::process_commands,).chain())
           ;
    }
}

pub trait LitkiAppExts {
    fn register_component<T>(
        &mut self,
        name: &str,
        builder: Arc<dyn Fn(Vec<PtrValue>) -> Result<T, String> + Send + Sync>,
        spawner: Arc<dyn Fn(T, &mut EntityWorldMut) -> Result<(), String> + Send + Sync>,
        // factory: Arc<dyn Fn(&mut EntityWorldMut, Vec<PtrValue>) -> Result<(), String> + Send + Sync>,
    );
}

impl LitkiAppExts for App {
    fn register_component<T>(
        &mut self,
        name: &str,
        builder: Arc<dyn Fn(Vec<PtrValue>) -> Result<T, String> + Send + Sync>,
        spawner: Arc<dyn Fn(T, &mut EntityWorldMut) -> Result<(), String> + Send + Sync>,
    ) {
        let component_builder_registry = self.world_mut().resource_mut::<ComponentBuilderRegistry>();
        log::warn!("todo: impl LitkiAppExts::register_component");
        // component_builder_registry.register()
        // component_builder_registry.register(name, Arc::new(move |entity, args| {
        //     let component = builder(args)?;
        //     spawner(component, entity)?;
        //     Ok(())
        // }));
    }
}

fn setup_core_builders(factory: Res<ComponentBuilderRegistry>) {
    register_core_builders(&factory);
}

fn bind_execute_command_function(
    env: Res<Environment>,
    registry: Res<CommandRegistry>,
    buffer: Res<CommandBuffer>,
    timeout: Res<CommandFunctionTimeout>,
) {
    crate::clojure::bind_execute_command_function(
        &env,
        &registry,
        &buffer,
        timeout.0,
    );
}

#[derive(Component)]
pub struct Health {
    max: u64,
    cur: u64
}

impl Health {
    pub fn new(max: u64) -> Self {
        Self { max, cur: max }
    }

    pub fn max(&self) -> u64 {
        self.max
    }

    pub fn cur(&self) -> u64 {
        self.cur
    }

    pub fn take_damage(&mut self, damage: i64) {
        self.cur = (self.cur as i64 - damage) as u64;
    }
}


#[derive(Component)]
pub struct RadialVision {
    radius: u64,
}

impl RadialVision {
    pub fn new(radius: u64) -> Self {
        Self { radius }
    }

    pub fn radius(&self) -> u64 {
        self.radius
    }
}

#[derive(Component, Clone, Debug, PartialEq, Eq, Hash)]
pub struct StableId(pub(crate) ShortId);

impl Into<Value> for StableId {
    fn into(self) -> Value {
        Value::string(self.to_string())
    }
}

impl StableId {
    pub fn to_value(&self) -> Value {
        Value::string(self.to_string())
    }

    pub fn to_value_ptr(&self) -> PtrValue {
        Value::string(self.to_string()).into_value_ptr()
    }

    pub fn into_value(self) -> Value {
        Value::string(self.to_string())
    }

    pub fn into_value_ptr(self) -> PtrValue {
        Value::string(self.to_string()).into_value_ptr()
    }

    pub fn try_from_string(id: String) -> Result<Self, (String, ShortIdError)> {
        ShortId::try_from(id.as_str())
            .map(StableId)
            .map_err(|e| (id, e))
    }

    pub fn try_from_str(id: &str) -> Result<Self, (&str, ShortIdError)> {
        ShortId::try_from(id)
            .map(StableId)
            .map_err(|e| (id, e))
    }

    pub fn new_random() -> Self {
        Self(ShortId::random())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl ToString for StableId {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

#[derive(Resource)]
pub struct EntityRegistry {
    stable_id_to_entity_id: Arc<Mutex<HashMap<StableId, Entity>>>,
    entity_id_to_stable_id: Arc<Mutex<HashMap<Entity, StableId>>>,
}

impl EntityRegistry {
    pub fn new() -> Self {
        Self {
            stable_id_to_entity_id: Arc::new(Mutex::new(HashMap::new())),
            entity_id_to_stable_id: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register(&self, stable_id: StableId, entity_id: Entity) {
        self.stable_id_to_entity_id.lock().unwrap().insert(stable_id.clone(), entity_id);
        self.entity_id_to_stable_id.lock().unwrap().insert(entity_id, stable_id);
    }

    pub fn get_by_stable_id(&self, stable_id: &StableId) -> Option<Entity> {
        self.stable_id_to_entity_id.lock().unwrap().get(stable_id).copied()
    }

    pub fn get_by_entity_id(&self, entity_id: &Entity) -> Option<StableId> {
        self.entity_id_to_stable_id.lock().unwrap().get(entity_id).cloned()
    }

    pub fn deregister_by_stable_id(&self, stable_id: StableId) -> Option<(Entity, StableId)> {
        let entity_id = self.stable_id_to_entity_id.lock().unwrap().remove(&stable_id).unwrap();
        let stable_id = self.entity_id_to_stable_id.lock().unwrap().remove(&entity_id).unwrap();
        Some((entity_id, stable_id))
    }

    pub fn deregister_by_entity_id(&self, entity_id: Entity) -> Option<(Entity, StableId)> {
        let stable_id = self.entity_id_to_stable_id.lock().unwrap().remove(&entity_id).unwrap();
        let entity_id = self.stable_id_to_entity_id.lock().unwrap().remove(&stable_id).unwrap();
        Some((entity_id, stable_id))
    }

    pub fn contains(&self, stable_id: StableId) -> bool {
        self.stable_id_to_entity_id.lock().unwrap().contains_key(&stable_id)
    }

    pub fn count(&self) -> usize {
        self.stable_id_to_entity_id.lock().unwrap().len()
    }
}

impl Default for EntityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn register_core_builders(component_builder_registry: &ComponentBuilderRegistry) {
    component_builder_registry.register(
        "litki.vital/health",
        Arc::new(|entity: &mut EntityWorldMut, args: Vec<PtrValue>| {
            let opts = value_optics::preview_map(args.first().unwrap()).unwrap();
            let max = opts.get(&Value::keyword_unqualified_ptr("max")).unwrap();
            let max = value_optics::preview_integer(&max).unwrap();
            entity.insert(Health {
                max: max as u64,
                cur: max as u64,
            });
            Ok(())
        }),
    );
    component_builder_registry.register(
        "litki.vision/radial",
        Arc::new(|entity: &mut EntityWorldMut, args: Vec<PtrValue>| {
            let opts = value_optics::preview_map(args.first().unwrap()).unwrap();
            let radius = opts.get(&Value::keyword_unqualified_ptr("radius")).unwrap();
            let radius = value_optics::preview_integer(&radius).unwrap();
            entity.insert(RadialVision::new(radius as u64));
            Ok(())
        }),
    );
}

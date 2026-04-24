use std::{collections::HashMap, sync::Arc};
use parking_lot::RwLock;
use bevy::prelude::*;
use bevy::log;
use jinme::prelude::*;

pub mod prelude;

mod clojure;

pub mod plugins;
pub mod script;

cfg_select! {
    feature = "repl_server" => {
        mod repl_server;
        pub use repl_server::ReplServer;
    }
    _ => {}
}

pub type PtrCommandBuilder = Arc<dyn ComponentBuilder>;

pub trait ComponentBuilder: Send + Sync + 'static {
    fn build(
        &self,
        entity: &mut EntityWorldMut,
        args: Vec<PtrValue>,
    ) -> Result<(), String>;

    fn ptr(self) -> PtrCommandBuilder
    where
        Self: Sized + 'static,
    {
        Arc::new(self)
    }
}

impl<F> ComponentBuilder for F
where
    F: Fn(&mut EntityWorldMut, Vec<PtrValue>) -> Result<(), String> + Send + Sync + 'static,
{
    fn build(&self, entity: &mut EntityWorldMut, args: Vec<PtrValue>) -> Result<(), String> {
        (self)(entity, args)
    }
}

#[derive(Resource, Clone)]
pub struct ComponentBuilderRegistry {
    builders: Arc<RwLock<HashMap<String, PtrCommandBuilder>>>,
}

impl Default for ComponentBuilderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentBuilderRegistry {
    pub fn new() -> Self {
        Self {
            builders: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn names(&self) -> Vec<String> {
        self.builders.read().keys().map(|k| k.to_owned()).collect()
    }

    pub fn builders_cloned(&self) -> HashMap<String, PtrCommandBuilder> {
        self.builders.read().to_owned()
    }

    pub fn get(&self, name: &str) -> Option<PtrCommandBuilder> {
        log::info!("Looking up component builder for '{}'", name);
        self.builders.read().get(name).cloned()
    }

    pub fn register(&self, name: impl Into<String>, builder: PtrCommandBuilder) {
        let name = name.into();
        log::info!("Registering component builder for '{}'", name);
        self.builders.write().insert(name, builder);
    }
}

#[derive(Resource)]
pub struct TemplateRegistry {
    templates: Arc<RwLock<HashMap<String, Value>>>,
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateRegistry {
    pub fn new() -> Self {
        Self {
            templates: Arc::new(RwLock::new(HashMap::new()))
        }
    }

    pub fn names(&self) -> Vec<String> {
        self.templates.read().keys().map(|k| k.to_owned()).collect()
    }

    pub fn templates_cloned(&self) -> HashMap<String, Value> {
        self.templates.read().to_owned()
    }

    // pub fn entries_cloned(&self) -> Vec<(String, Value)> {
    //     self.templates.read().iter().map(|(s, v)| (s.to_owned(), v.to_owned())).collect()
    // }

    pub fn register(&self, name: impl Into<String>, template: Value) {
        self.templates.write().insert(name.into(), template);
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        self.templates.read().get(name).cloned()
    }

    pub fn update(&self, name: &str, update_fn: impl Fn(Value) -> Value) {
        if let Some(current_template) = self.get(name) {
            let new_template = update_fn(current_template);
            self.register(name, new_template);
        }
    }
}

// #[derive(Event, Clone)]
// pub struct TemplateModifiedEvent {}

#[derive(Resource)]
pub struct AnimationRegistry; // Placeholder for the actual AnimationRegistry resource

impl Default for AnimationRegistry {
    fn default() -> Self {
        Self
    }
}

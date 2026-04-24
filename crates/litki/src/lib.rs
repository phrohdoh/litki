use std::any::TypeId;
use std::{collections::HashMap, sync::Arc};
use parking_lot::RwLock;
use bevy::prelude::{EntityWorldMut, Resource, Result, String, ToOwned, Vec};
use bevy::log;
use jinme::prelude::PtrValue;

pub mod prelude;

pub mod clojure;
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
    /// Maps component name -> PtrCommandBuilder
    named: Arc<RwLock<HashMap<String, PtrCommandBuilder>>>,
}

impl Default for ComponentBuilderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentBuilderRegistry {
    pub fn new() -> Self {
        Self {
            named: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn entries(&self) -> Vec<(String, PtrCommandBuilder)> {
        self.named.read()
            .iter()
            .map(|(k, v)| (k.to_owned(), v.clone()))
            .collect()
    }

    pub fn names(&self) -> Vec<String> {
        self.named.read()
            .keys()
            .map(ToOwned::to_owned)
            .collect()
    }

    pub fn get(&self, name: &str) -> Option<PtrCommandBuilder> {
        log::info!("Looking up component builder for '{}'", name);
        self.named.read().get(name).cloned()
    }

    pub fn register(
        &self,
        name: impl Into<String>,
        builder: PtrCommandBuilder,
    ) {
        let name = name.into();
        log::info!("Registering component builder for '{}'", name);
        self.named.write().insert(name, builder.clone());
    }
}

#[derive(Resource)]
pub struct TemplateRegistry {
    templates: Arc<RwLock<HashMap<String, PtrValue>>>,
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

    pub fn entries(&self) -> Vec<(String, PtrValue)> {
        self.templates.read()
            .iter()
            .map(|(k, v)| (k.to_owned(), v.clone()))
            .collect()
    }

    pub fn names(&self) -> Vec<String> {
        self.templates.read()
            .keys()
            .map(|k| k.to_owned())
            .collect()
    }

    pub fn register(&self, name: impl Into<String>, template: impl Into<PtrValue>) {
        self.templates.write()
            .insert(
                name.into(),
                template.into(),
            );
    }

    pub fn get(&self, name: &str) -> Option<PtrValue> {
        self.templates.read()
            .get(name)
            .cloned()
    }

    pub fn update(&self, name: &str, update_fn: impl Fn(PtrValue) -> PtrValue) {
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

#[derive(Resource)]
pub struct ComponentTypeRegistry {
    named: Arc<RwLock<HashMap<String, TypeId>>>,
}

impl Default for ComponentTypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentTypeRegistry {
    pub fn new() -> Self {
        Self {
            named: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register(&self, name: impl Into<String>, type_id: TypeId) {
        self.named.write().insert(name.into(), type_id);
    }

    pub fn get_type_id(&self, name: &str) -> Option<TypeId> {
        self.named.read().get(name).copied()
    }
}

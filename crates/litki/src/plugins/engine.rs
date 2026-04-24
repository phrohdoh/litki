use std::sync::Arc;

use bevy::prelude::*;
use jinme::{prelude::*, value::optics as value_optics};

use crate::{
    AnimationRegistry,
    PtrCommandBuilder,
    ComponentBuilderRegistry,
    TemplateRegistry,
    script::{self, CommandBuffer, CommandRegistry, Environment, ReplTimeout, CommandFunctionTimeout}
};

pub struct EnginePlugin;

impl Plugin for EnginePlugin {
    fn build(&self, app: &mut App) {
        // Add systems and resources specific to the game engine here
        app.init_resource::<ComponentBuilderRegistry>()
           .init_resource::<TemplateRegistry>()
           .init_resource::<AnimationRegistry>() // Placeholder for the actual AnimationRegistry resource
           //.init_resource::<EntityRegistry>() // Placeholder for the actual EntityRegistry resource

           .init_resource::<CommandBuffer>()
           .init_resource::<CommandRegistry>()
           .init_resource::<ReplTimeout>()
           .init_resource::<CommandFunctionTimeout>()

           //.add_message::<TemplateModifiedEvent>()
           .insert_resource(Environment::new(crate::clojure::create_env()))

           .add_systems(Startup, (setup_core_builders, bind_execute_command_function))
           .add_systems(Update, (script::process_commands,).chain())
           ;
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

use std::sync::Arc;
use bevy::prelude::*;
use bevy::log;
use jinme::{prelude::*, value::Value, value::optics as value_optics, vector::optics as vector_optics};
use litki::prelude::*;
use litki::script::Command as _;
use litki::script::CommandResult;
use litki::{ComponentBuilderRegistry, TemplateRegistry, AnimationRegistry};
use litki::script::{CommandRegistry, closure_factory};

fn main() {
    println!("Launching Scenario 1");
    run_scenario();
}

fn run_scenario() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "RTS Game Engine - Scenario 1 (Wildlife)".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EnginePlugin)
        .add_plugins(ReplServerPlugin)
        .add_plugins(Scenario1Plugin)
        .run();
}

struct Scenario1Plugin;

impl Plugin for Scenario1Plugin {
    fn build(&self, app: &mut App) {
        // Add systems and resources specific to Scenario 1 here
        app.insert_resource({
                let command_registry = CommandRegistry::new();

                command_registry.register("echo",
                    closure_factory(|args| Ok(litki::script::command_fn(
                        |_| Ok(()),
                        move |_| CommandResult::Success((&args[0]).to_owned()),
                    ).boxed())),
                );

                command_registry.register("register-template",
                    closure_factory(|args| Ok(litki::script::command_fn(
                        |_| Ok(()),
                        move |world| CommandResult::Success({
                            let template_registry = world.resource::<TemplateRegistry>();
                            let opts = value_optics::preview_map(&args[0]).unwrap();
                            let name = {
                                let name = opts.get(&Value::keyword_unqualified_ptr("name")).unwrap();
                                let name = match name.as_ref() {
                                    Value::String(name, _) => name.to_owned(),
                                    Value::Symbol(Symbol::Unqualified(name), _) => name.name().to_owned(),
                                    Value::Keyword(Keyword::Unqualified(name), _) => name.name().to_owned(),
                                    Value::Symbol(Symbol::Qualified(name), _) => name.namespace().to_owned() + "/" + name.name(),
                                    Value::Keyword(Keyword::Qualified(name), _) => name.namespace().to_owned() + "/" + name.name(),
                                    _ => return CommandResult::Error(Value::vector_from(vec![
                                        Value::keyword_unqualified_ptr("invalid-template-name"),
                                        name,
                                    ]).into_value_ptr()),
                                };
                                name
                            };
                            let template = opts.get(&Value::keyword_unqualified_ptr("template")).unwrap();
                            template_registry.register(name.clone(), template.as_ref().to_owned());
                            Value::string_ptr(name)
                        }),
                    ).boxed())),
                );

                command_registry.register("get-component-names",
                    closure_factory(|_args| Ok(litki::script::command_fn(
                        |_| Ok(()),
                        |world| CommandResult::Success({
                            let component_builder_registry = world.resource::<ComponentBuilderRegistry>();
                            let component_names = component_builder_registry.names();
                            Arc::new(Value::vector_from(component_names.iter().map(|s| Value::string_ptr(s.to_owned())).collect()))
                        }),
                    ).boxed())),
                );

                command_registry.register("get-template-names",
                    closure_factory(|_args| Ok(litki::script::command_fn(
                        |_| Ok(()),
                        |world| CommandResult::Success({
                            let template_registry = world.resource::<TemplateRegistry>();
                            let template_names = template_registry.names();
                            Arc::new(Value::vector_from(template_names.iter().map(|s| Value::string_ptr(s.to_owned())).collect()))
                        }),
                    ).boxed())),
                );

                command_registry.register("get-template",
                    closure_factory(|args| Ok(litki::script::command_fn(
                        |_| Ok(()),
                        move |world| {
                            CommandResult::Success({
                                let template_name = {
                                    let name = args.first().cloned().unwrap();
                                    let name = match name.as_ref() {
                                        Value::String(name, _) => name.to_owned(),
                                        Value::Symbol(Symbol::Unqualified(name), _) => name.name().to_owned(),
                                        Value::Keyword(Keyword::Unqualified(name), _) => name.name().to_owned(),
                                        Value::Symbol(Symbol::Qualified(name), _) => name.namespace().to_owned() + "/" + name.name(),
                                        Value::Keyword(Keyword::Qualified(name), _) => name.namespace().to_owned() + "/" + name.name(),
                                        _ => return CommandResult::Error(Value::vector_from(vec![
                                            Value::keyword_unqualified_ptr("invalid-template-name"),
                                            name,
                                        ]).into_value_ptr()),
                                    };
                                    name
                                };
                                let template_registry = world.resource::<TemplateRegistry>();
                                let template = match template_registry.get(template_name.as_str()) {
                                    Some(template) => value_optics::preview_vector(&template).unwrap(),
                                    _ => return CommandResult::Error(Value::vector_from(vec![
                                            Value::keyword_unqualified_ptr("unknown-template"),
                                            Value::string_ptr(template_name),
                                        ]).into()),
                                };
                                Value::vector(template).into()
                            })
                        },
                    ).boxed())),
                );

                command_registry.register("collect-entity-ids",
                    closure_factory(|_args| Ok(litki::script::command_fn(
                        |_| Ok(()),
                        |world| CommandResult::Success({
                            let entities = world.query::<Entity>()
                                                .iter(world)
                                                .map(|e| Value::integer_ptr(e.index_u32() as i64))
                                                .collect::<Vec<_>>();
                            let entities = Value::vector_from(entities).into_value_ptr();
                            entities
                        }),
                    ).boxed())),
                );

                command_registry.register("query-entities-with-health", closure_factory(|_args| {
                    Ok(litki::script::command_fn(
                        |_| Ok(()),
                        move |world| {
                            let entities_with_health = world.query::<(Entity, &litki::plugins::Health)>()
                                                            .iter(world)
                                                            .map(|(e, h)| Arc::new(Value::map_from(vec![
                                                                (Value::keyword_unqualified_ptr("id"), Value::integer_ptr(e.index_u32() as i64)),
                                                                (Value::keyword_unqualified_ptr("health"), Value::integer_ptr(h.cur() as i64)),
                                                            ])))
                                                            .collect::<Vec<_>>();
                            CommandResult::Success(Arc::new(Value::vector_from(entities_with_health)))
                        }
                    ).boxed())
                }));

                command_registry.register("spawn", closure_factory(|args| {
                    Ok(litki::script::command_fn(
                        {
                            let args = args.clone();
                            move |_world| if args.is_empty() { Err("No arguments provided".into()) } else { Ok(()) }
                        },
                        move |world| {
                            let template = {
                                let template_registry = world.resource::<TemplateRegistry>();
                                let template_name = {
                                    let name = args.first().cloned().unwrap();
                                    let name = match name.as_ref() {
                                        Value::String(name, _) => name.to_owned(),
                                        Value::Symbol(Symbol::Unqualified(name), _) => name.name().to_owned(),
                                        Value::Keyword(Keyword::Unqualified(name), _) => name.name().to_owned(),
                                        Value::Symbol(Symbol::Qualified(name), _) => name.namespace().to_owned() + "/" + name.name(),
                                        Value::Keyword(Keyword::Qualified(name), _) => name.namespace().to_owned() + "/" + name.name(),
                                        _ => return CommandResult::Error(Value::vector_from(vec![
                                            Value::keyword_unqualified_ptr("invalid-template-name"),
                                            name,
                                        ]).into_value_ptr()),
                                    };
                                    name
                                };
                                let template = match template_registry.get(template_name.as_str()) {
                                    Some(template) => value_optics::preview_vector(&template).unwrap(),
                                    _ => return CommandResult::Error(Value::vector_from(vec![
                                            Value::keyword_unqualified_ptr("unknown-template"),
                                            Value::string_ptr(template_name),
                                        ]).into()),
                                };
                                template
                            };

                            let mut component_errors = Vec::new();
                            let components = {
                                let component_builder_registry = world.resource::<ComponentBuilderRegistry>();
                                let mut components = Vec::new();
                                for component in template.iter() {
                                    let component = value_optics::preview_vector(&component).unwrap();
                                    let component_name = {
                                        let name = vector_optics::view_first(&component).unwrap();
                                        let name = match name.as_ref() {
                                            Value::String(name, _) => name.to_owned(),
                                            Value::Symbol(Symbol::Unqualified(name), _) => name.name().to_owned(),
                                            Value::Keyword(Keyword::Unqualified(name), _) => name.name().to_owned(),
                                            Value::Symbol(Symbol::Qualified(name), _) => name.namespace().to_owned() + "/" + name.name(),
                                            Value::Keyword(Keyword::Qualified(name), _) => name.namespace().to_owned() + "/" + name.name(),
                                            _ => return CommandResult::Error(Value::vector_from(vec![
                                                Value::keyword_unqualified_ptr("invalid-component-name"),
                                                name,
                                            ]).into_value_ptr()),
                                        };
                                        name
                                    };
                                    let component_opts = component.collect_rest::<Vec<_>>();
                                    match component_builder_registry.get(component_name.as_str()) {
                                        Some(component_builder) => {
                                            components.push((
                                                component_name,
                                                component_builder,
                                                component_opts,
                                            ));
                                        },
                                        _ => {
                                            component_errors.push(Value::vector_from(vec![
                                                Value::keyword_unqualified_ptr("unknown-component"),
                                                Value::string_ptr(component_name),
                                            ]).into_value_ptr());
                                        },
                                    }
                                }
                                components
                            };

                            if !component_errors.is_empty() {
                                return CommandResult::Error(
                                    Value::vector_from(component_errors).into()
                                );
                            }

                            let mut new_entity = world.spawn_empty();

                            for (component_name, component_builder, component_opts) in components {
                                log::info!("Creating component {} with {}", component_name, jinme::value::Value::vector_from(component_opts.clone()));
                                component_builder.build(&mut new_entity, component_opts).unwrap();
                            }

                            CommandResult::Success(Value::integer_ptr(new_entity.id().index_u32() as i64))
                        }
                    ).boxed())
                }));

                command_registry.register("inflict-damage", closure_factory(|args| {
                    Ok(litki::script::command_fn(
                        {
                            let args = args.clone();
                            move |_world| if args.is_empty() { Err("No arguments provided".into()) } else { Ok(()) }
                        },
                        move |world| {
                            let entity_id = value::optics::preview_integer(args.get(0).unwrap()).unwrap();
                            let damage = value::optics::preview_integer(args.get(1).unwrap()).unwrap();

                            let mut entity = world.entity_mut(Entity::from_raw_u32(entity_id as u32).unwrap());
                            let health = entity.get_mut::<litki::plugins::Health>();

                            if let Some(mut health) = health {
                                let ante_cur = health.cur();
                                health.take_damage(damage);
                                let post_cur = health.cur();
                                log::info!("Inflicted {} damage to entity {}, health went from {} to {}", damage, entity_id, ante_cur, post_cur);
                                CommandResult::Success(Value::integer_ptr(post_cur as i64))
                            } else {
                                CommandResult::Error(Value::string_ptr("Entity does not have Health component".into()))
                            }
                        }
                    ).boxed())
                }));

                command_registry
            })
           .add_systems(Startup, (
               setup_world,
               load_scenario_data,
           ).chain())
           .add_systems(Update, (
                update_unit_movement,
                update_animation_timers,
                //print_state.run_if(on_timer(Duration::from_secs(1))),
            ).chain())
           ;
    }
}

fn setup_world(
    mut commands: Commands,
    assets: Res<AssetServer>,
) {
    log::info!("Setting up the world for Scenario 1...");

    // Spawn 2D orthographic camera
    // This camera renders the game world. The transform puts it at z=100
    // so it looks down at the z=0 plane where entities are positioned.
    commands.spawn(Camera2d);
    //let camera = Camera2dBundle {
    //    transform: Transform::from_xyz(640.0, 360.0, 100.0),
    //    projection: OrthographicProjection {
    //        far: 1000.0,
    //        near: -1000.0,
    //        ..default()
    //    },
    //    ..default()
    //};
    //log::info!("✓ Camera spawned at (640, 360, 100)");
    log::info!("✓ Camera spawned");

    // Spawn background sprite (simple colored rectangle)
    //let background = SpriteBundle {
    //    sprite: Sprite {
    //        color: Color::rgb(0.1, 0.1, 0.15),
    //        custom_size: Some(Vec2::new(1280.0, 720.0)),
    //        ..default()
    //    },
    //    transform: Transform::from_xyz(640.0, 360.0, -10.0),
    //    ..default()
    //};
    //commands.spawn(background);
    commands.spawn(Sprite {
        image: assets.load("sprites/background.png"),
        ..default()
    });
    log::info!("✓ Background sprite spawned");

    // Spawn grid lines for visualization (helps see unit positions)
    //spawn_grid_lines(&mut commands);
    //log::info!("✓ Grid visualization spawned");

    // Spawn UI text (will be updated by print_state system)
    spawn_ui_text(&mut commands, &assets);
    log::info!("✓ UI text canvas spawned");

    log::info!("✓ World setup complete!");
}

// /// Spawn grid visualization lines to help debug unit positioning
// fn spawn_grid_lines(commands: &mut Commands) {
//     let grid_size = 100.0;
//     let world_width = 1280.0;
//     let world_height = 720.0;
//
//     // Vertical lines
//     let mut x = 0.0;
//     while x < world_width {
//         commands.spawn(LineBundle {
//             line: Line {
//                 start: Vec2::new(x, 0.0),
//                 end: Vec2::new(x, world_height),
//                 width: 1.0,
//                 color: Color::rgb(0.2, 0.2, 0.2),
//             },
//             transform: Transform::from_xyz(0.0, 0.0, 0.0),
//             ..default()
//         });
//         x += grid_size;
//     }
//
//     // Horizontal lines
//     let mut y = 0.0;
//     while y < world_height {
//         commands.spawn(LineBundle {
//             line: Line {
//                 start: Vec2::new(0.0, y),
//                 end: Vec2::new(world_width, y),
//                 width: 1.0,
//                 color: Color::rgb(0.2, 0.2, 0.2),
//             },
//             transform: Transform::from_xyz(0.0, 0.0, 0.0),
//             ..default()
//         });
//         y += grid_size;
//     }
// }

/// Spawn UI text entity that will display game state
fn spawn_ui_text(
    commands: &mut Commands,
    assets: &AssetServer,
) {
    let font = assets.load("fonts/JetBrainsMono-Regular.ttf");
    commands.spawn((
        Text::new(
        "RTS Game Engine - Wildlife Scenario\n\
         REPL: nc localhost 7888\n\
         Commands:\n\
            \t(! :echo :hi)\n\
            \t(! :collect-entity-ids)\n\
            \t(! :get-template-names)\n\
            \t(! :register-template {:name :deer, :template [[:litki.sprite/file \"deer.png\"] [:litki.vital/health {:max 150}]]})\n\
            \t(! :spawn :deer)\n"
        ),
        TextFont::from(font.clone()).with_font_size(13.0),
        TextColor(Color::linear_rgb(0.8, 0.8, 0.8)),
    ));
}



fn load_scenario_data(
    _components: Res<ComponentBuilderRegistry>,
    _templates: Res<TemplateRegistry>,
    _animations: Res<AnimationRegistry>,
) {
    // Load any necessary data for Scenario 1 here
    // For example: map data, unit stats, etc.
    log::info!("Load scenario data for Scenario 1");
}

fn update_unit_movement() {
    // Update unit movement logic for Scenario 1 here
    // For example: pathfinding, collision avoidance, etc.
}

fn update_animation_timers() {
    // Update animation timers for Scenario 1 here
    // For example: handle animation state changes, transitions, etc.
}

// fn print_state() {
//     // Print the current state of the game for debugging purposes
//     log::info!("Current game state: ...");
// }

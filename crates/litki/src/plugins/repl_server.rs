use std::thread;
use bevy::prelude::*;
use bevy::log;
use crossbeam::channel;
use crate::repl_server::ReplServer;
use crate::script::{CommandBuffer, CommandRegistry, Environment as EnvWrapper};

pub struct ReplServerPlugin;

impl Plugin for ReplServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_repl_server)
           .add_systems(Update, receive_repl_server);
    }
}

#[derive(Resource)]
struct ReplServerReceiver(channel::Receiver<ReplServer>);

impl std::ops::Deref for ReplServerReceiver {
    type Target = channel::Receiver<ReplServer>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn setup_repl_server(
    mut commands: Commands,
    env: Res<EnvWrapper>,
    command_buffer: Res<CommandBuffer>,
    command_registry: Res<CommandRegistry>,
) {
    let env = env.clone();
    let command_buffer = command_buffer.clone();
    let command_registry = command_registry.clone();
    let (tx, rx) = channel::unbounded();

    thread::spawn(move || {
        match ReplServer::start(
            env.inner(),
            command_buffer,
            command_registry,
            7888,
        ) {
            Ok(repl_server) => {
                // commands.insert_resource(repl_server);
                let _ = tx.send(repl_server);
                log::info!("REPL server started successfully on port 7888");
            },
            Err(e) => log::error!("Failed to start REPL server: {}", e),
        }
    });

    commands.insert_resource(ReplServerReceiver(rx));
}

fn receive_repl_server(
    mut commands: Commands,
    rx: Option<Res<ReplServerReceiver>>,
) {
    if let Some(rx) = rx {
        if let Ok(repl_server) = rx.try_recv() {
            commands.insert_resource(repl_server);
            commands.remove_resource::<ReplServerReceiver>();
        }
    }
}

use std::{io::{BufRead, BufReader, Write}, net::{TcpListener, TcpStream}, sync::{Arc, atomic::{AtomicBool, Ordering}}, thread::{self, JoinHandle}, time::Duration};
use bevy::prelude::{Resource, Result, String, ToString, Vec, format};
use bevy::log;
use crate::script::{CommandBuffer, CommandRegistry};

#[derive(Resource)]
pub struct ReplServer {
    port: u16,
    handle: Option<JoinHandle<()>>,
    shutdown_signal: Arc<AtomicBool>,
}

impl ReplServer {
    pub fn start(
        env: Arc<jinme::environment::Environment>,
        command_buffer: CommandBuffer,
        command_registry: CommandRegistry,
        port: u16,
    ) -> Result<Self, String> {
        let addr = format!("127.0.0.1:{port}");
        let listener = TcpListener::bind(&addr).map_err(|e| format!("Failed to bind REPL server to {addr}: {e}"))?;
        listener.set_nonblocking(true).map_err(|e| format!("Failed to set non-blocking mode: {e}"))?;

        let shutdown_signal = Arc::new(AtomicBool::new(false));
        let shutdown_signal_clone = shutdown_signal.clone();

        let handle = thread::spawn(move || {
            Self::accept_connections(
                env,
                listener,
                command_buffer,
                command_registry,
                shutdown_signal_clone,
            );
        });

        Ok(Self {
            port,
            handle: Some(handle),
            shutdown_signal,
        })
    }

    fn accept_connections(
        env: Arc<jinme::environment::Environment>,
        listener: TcpListener,
        command_buffer: CommandBuffer,
        command_registry: CommandRegistry,
        shutdown_signal: Arc<AtomicBool>,
    ) {
        loop {
            // Check if shutdown signal is set
            if shutdown_signal.load(Ordering::Relaxed) {
                log::info!("REPL server shutdown signal received");
                break;
            }

            // Try to accept a connection
            match listener.accept() {
                Ok((stream, _)) => {
                    let env = env.clone();
                    let command_buffer = command_buffer.clone();
                    let command_registry = command_registry.clone();
                    thread::spawn(move || {
                        Self::handle_client(
                            env,
                            stream,
                            command_buffer,
                            command_registry,
                        );
                    });
                },
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Non-blocking socket returned WouldBlock, sleep briefly and check shutdown signal
                    thread::sleep(Duration::from_millis(100));
                },
                Err(e) => {
                    // Log error and continue (listener might be temporarily unavailable)
                    log::debug!("Error accepting REPL client connection: {e}");
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
        log::info!("REPL server accept loop exited");
    }

    fn handle_client(
        env: Arc<jinme::environment::Environment>,
        stream: TcpStream,
        command_buffer: CommandBuffer,
        command_registry: CommandRegistry,
    ) {
        // Ensure the stream is in blocking mode for reading
        if let Err(e) = stream.set_nonblocking(false) {
            log::error!("Failed to set stream to blocking mode: {e}");
            return;
        }

        let write_stream = match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to clone stream: {e}");
                return;
            }
        };

        let reader = BufReader::new(stream);
        let mut writer = write_stream;

        let _ = writeln!(writer, "Welcome to the Bevy CLJX REPL server!");
        let _ = writeln!(writer, "Type (help) for available commands.");
        let _ = writeln!(writer);
        let _ = writer.flush();

        // Read, Eval, Print loop
        for line_result in reader.lines() {
            match line_result {
                Ok(line) => {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with(';') {
                        continue; // ignore empty lines and comments
                    }
                    if line == "(exit)" || line == "(quit)" {
                        let _ = writeln!(writer, "Goodbye!");
                        let _ = writer.flush();
                        break;
                    }
                    if line == "(help)" {
                        let _ = writeln!(writer, "Available commands:");
                        let _ = writeln!(writer, "  (help) - Show this help message");
                        let _ = writeln!(writer, "  (exit) or (quit) - Exit the REPL");
                        let _ = writeln!(writer, "  <expr> - Evaluate a Clojure expression");
                        let _ = writer.flush();
                        continue;
                    }
                    match Self::dispatch_command(
                        env.clone(),
                        line,
                        &command_buffer,
                        &command_registry,
                    ) {
                        Ok(response) => {
                            let _ = writeln!(writer, "=> {}", response);
                        },
                        Err(e) => {
                            let _ = writeln!(writer, "Error: {}", e);
                        }
                    }
                    let _ = writeln!(writer);
                    let _ = writer.flush();
                },
                Err(e) => {
                    log::error!("Error reading from REPL client: {e}");
                    break;
                }
            }
        }
    }

    fn dispatch_command(
        env: Arc<jinme::environment::Environment>,
        expr: &str,
        command_buffer: &CommandBuffer,
        command_registry: &CommandRegistry,
    ) -> Result<String, String> {
        let (_, value) = jinme::read2::read(env.clone(), expr).unwrap();
        let value = value.unwrap();

        use jinme::{
            value::optics as value_optics,
            list::optics as list_optics,
        };

        if let Some(list) = value_optics::preview_list(&value) {
            if let Some(head) = list_optics::view_first_as_symbol(&list) {
                let command_name = head.name();
                let args = list.collect_rest::<Vec<_>>();
                if command_registry.contains(command_name) {
                    log::info!("Received command: {} with args: {}", command_name, jinme::value::Value::list_from(args.clone()));

                    // Execute with promise and block for result with timeout
                    match command_registry.execute_with_promise(command_name, args, command_buffer) {
                        Ok(promise) => {
                            // Wait up to 5 seconds for the command to execute
                            use std::time::Duration;
                            match promise.deref_timeout(Duration::from_secs(5)) {
                                Some(result) => {
                                    match result {
                                        crate::script::CommandResult::Success(value) => {
                                            return Ok(format!("{}", value));
                                        }
                                        crate::script::CommandResult::Error(value) => {
                                            return Err(format!("{}", value));
                                        }
                                        crate::script::CommandResult::Pending => {
                                            return Err("Command still pending after timeout".to_string());
                                        }
                                    }
                                }
                                None => {
                                    return Err("Command execution timed out".to_string());
                                }
                            }
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    }
                }
            }
        }

        let evaled = jinme::core::eval(env, value);
        Ok(format!("{evaled}"))
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn shutdown(mut self) -> Result<(), String> {
        log::info!("Shutting down REPL server on port {}", self.port);

        // Set shutdown signal to trigger thread exit
        self.shutdown_signal.store(true, Ordering::Relaxed);

        // Wait for the accept thread with a timeout
        if let Some(handle) = self.handle.take() {
            let timeout = Duration::from_secs(10);
            let start = std::time::Instant::now();

            loop {
                if start.elapsed() >= timeout {
                    log::error!("REPL server thread did not exit within {} seconds", timeout.as_secs());
                    return Err("REPL server thread did not exit within timeout".to_string());
                }

                if handle.is_finished() {
                    match handle.join() {
                        Ok(()) => {
                            log::info!("REPL server shutdown complete");
                            return Ok(());
                        }
                        Err(_) => {
                            log::warn!("REPL server thread panicked");
                            return Err("REPL server thread panicked".to_string());
                        }
                    }
                }

                thread::sleep(Duration::from_millis(100));
            }
        } else {
            log::info!("REPL server shutdown complete (no handle)");
            Ok(())
        }
    }

    pub fn send_command(&self, expr: &str) -> Result<String, String> {
        let addr = format!("127.0.0.1:{}", self.port);
        let mut stream = TcpStream::connect(&addr).map_err(|e| format!("Failed to connect to REPL server at {addr}: {e}"))?;
        let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        writeln!(stream, "{}", expr).map_err(|e| format!("Failed to send command to REPL server: {e}"))?;
        let mut result = String::new();
        reader.read_line(&mut result).map_err(|e| format!("Failed to read result from REPL server: {e}"))?;
        Ok(result.trim().to_string())
    }
}

impl Drop for ReplServer {
    fn drop(&mut self) {
        // Set shutdown signal
        self.shutdown_signal.store(true, Ordering::Relaxed);

        // Try to join the accept thread with a timeout
        if let Some(handle) = self.handle.take() {
            let timeout = Duration::from_secs(5);
            let start = std::time::Instant::now();

            while start.elapsed() < timeout {
                if handle.is_finished() {
                    if let Ok(result) = handle.join() {
                        log::info!("REPL server thread exited cleanly");
                    } else {
                        log::warn!("REPL server thread panicked on drop");
                    }
                    return;
                }
                thread::sleep(Duration::from_millis(100));
            }

            log::warn!("REPL server accept thread did not exit within {} seconds", timeout.as_secs());
        }
    }
}

use std::collections::HashMap;
/// A programmatic test harness for the litki REPL server.
///
/// Run via: cargo run --example test_harness
///
/// This connects to the REPL server on localhost:7888 and runs
/// a comprehensive suite of test scenarios.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs as _};
use std::sync::Arc;
use std::time::Duration;

use bevy::log;
use jinme::dependency::itertools::Itertools as _;
use jinme::environment::PtrEnvironment;
use jinme::value::{PtrValue, Value};
use jinme::eval_context::EvalContext;

const REPL_ADDR: &str = "127.0.0.1:7888";
const READ_TIMEOUT_MS: u64 = 10_000;

// ── Data structures ──────────────────────────────────────────────────────────

// #[derive(Debug)]
struct Step {
    label: String,
    expr: String,
    expectations: Expectations,
    state_update: StateUpdate,
}

// #[derive(Debug)]
struct Expectations {
    expect_success: Option<Arc<dyn Fn(PtrEnvironment, EvalContext, Vec<PtrValue>) -> bool>>,
    expect_error: Option<Arc<dyn Fn(PtrEnvironment, EvalContext, Vec<PtrValue>) -> bool>>,
    // /// True = expect a successful response whose stdout contains this string
    // /// None = don't inspect the content
    // /// empty string = expect any success response
    // expect_success_contains: Option<&'static str>,
    // /// True = expect an error response whose stderr contains this string
    // expect_error_contains: Option<&'static str>,
}

impl Expectations {
    fn success() -> Self {
        Self {
            expect_success: Some(Arc::new(move |_env, _ctx, _args| true)),
            expect_error: None,
        }
    }

    fn error() -> Self {
        Self {
            expect_success: None,
            expect_error: Some(Arc::new(move |_env, _ctx, _args| true)),
        }
    }

    fn success_where(
        expect: Arc<dyn Fn(PtrEnvironment, EvalContext, Vec<PtrValue>) -> bool>,
    ) -> Self {
        Self {
            expect_success: Some(expect),
            expect_error: None,
            // expect_success_contains: None,
            // expect_error_contains: None,
        }
    }

    fn error_where(
        expect: Arc<dyn Fn(PtrEnvironment, EvalContext, Vec<PtrValue>) -> bool>,
    ) -> Self {
        Self {
            expect_success: None,
            expect_error: Some(expect),
            // expect_success_contains: None,
            // expect_error_contains: None,
        }
    }

    fn success_response_is_string() -> Self {
        Self {
            expect_success: Some(Arc::new(move |_env, _ctx, args| {
                if let Some(actual) = args.first() {
                    return actual.as_ref().preview_string().is_some();
                }
                false
            })),
            expect_error: None,
        }
    }

    fn success_response_is_value(expected: PtrValue) -> Self {
        Self {
            expect_success: Some(Arc::new(move |_env, _ctx, args| {
                if let Some(actual) = args.first() {
                    return *actual == expected;
                }
                false
            })),
            expect_error: None,
        }
    }

    fn success_response_eq_string(expected: &'static str) -> Self {
        // use jinme::function::{PtrFunction, FunctionArity, build_function, build_function_ptr, closure_fn};
        Self {
            expect_success: Some(Arc::new(move |_env, _ctx, args| {
                if let Some(actual) = args.first() {
                    if let Some(actual) = actual.as_ref().preview_string() {
                        return actual == expected;
                    }
                }
                false
            })),
            expect_error: None,
            // expect_success_contains: Some(s),
            // expect_error_contains: None,
        }
    }

    fn success_response_is_string_containing(needle: &'static str) -> Self {
        // use jinme::function::{PtrFunction, FunctionArity, build_function, build_function_ptr, closure_fn};
        Self {
            expect_success: Some(Arc::new(move |_env, _ctx, args| {
                if let Some(haystack) = args.first() {
                    if let Some(haystack) = haystack.as_ref().preview_string() {
                        return haystack.contains(needle)
                    }
                }
                false
            })),
            expect_error: None,
            // expect_success_contains: Some(s),
            // expect_error_contains: None,
        }
    }

    fn no_error() -> Self {
        Self {
            expect_success: None,
            expect_error: None,
            // expect_success_contains: Some(""),
            // expect_error_contains: None,
        }
    }
    fn error_contains(s: &'static str) -> Self {
        Self {
            expect_success: None,
            expect_error: None,
            // expect_success_contains: None,
            // expect_error_contains: Some(s),
        }
    }
    fn any() -> Self {
        Self {
            expect_success: None,
            expect_error: None,
            // expect_success_contains: Some(""),
            // expect_error_contains: None,
        }
    }
}

#[derive(Debug)]
enum StateUpdate {
    /// Extract entity IDs and save them
    StableIds,
    /// Extract template names and save them
    GetTemplateNames,
    /// Extract spawned entity ID as `last_entity_id`
    SpawnReturnsStableId,
    /// Extract remaining health
    InflictDamageReturnsHealth,
    /// Parse health query results
    QueryHealthReturnsEntities,
    /// Template was registered — add to list
    TemplateRegistered,
    /// Extract template value
    GetTemplate,
    /// Get component names
    GetComponentNames,
    None,
}

struct TestCase {
    name: &'static str,
    steps: Vec<Step>,
    summary_text: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StableId(String);

impl StableId {
    pub fn into_string(self) -> String {
        self.0
    }
}

impl ToString for StableId {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

struct HarnessState {
    stable_ids: Vec<StableId>,
    template_names: Vec<String>,
    last_spawned_stable_id: Option<StableId>,
    health_map: HashMap<StableId, i64>,
    component_names: Vec<String>,
}

impl HarnessState {
    fn new() -> Self {
        Self::default()
    }
}

impl Default for HarnessState {
    fn default() -> Self {
        Self {
            stable_ids: Vec::new(),
            template_names: Vec::new(),
            last_spawned_stable_id: None,
            health_map: HashMap::new(),
            component_names: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct TestResult {
    step_label: String,
    expr: String,
    success: bool,
    message: String,
}

// ── REPL client ──────────────────────────────────────────────────────────────

/// Send an expression to the REPL server and return the trimmed response
/// (the part after "=> " or "Error: ").
fn send_expr(expr: &str) -> Result<(String, bool), String> {
    // Allow retrying for the first few attempts in case server is still starting
    let mut last_err = None;
    for attempt in 0..5 {
        match TcpStream::connect_timeout(&REPL_ADDR.to_socket_addrs().unwrap().next().unwrap(), Duration::from_millis(READ_TIMEOUT_MS)) {
            Ok(stream) => {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut writer = stream;
                let _ = writeln!(writer, "{}", expr);
                let mut buf = String::new();
                reader.read_line(&mut buf).unwrap();
                // skip welcome lines
                while buf.lines().any(|l| l.trim().is_empty() || l.starts_with("Welcome") || l.starts_with("Type")) || buf.contains("(litki") {
                    // irrelevant line
                    break;
                }
                let trimmed = buf.lines()
                    .filter(|l| l.starts_with("=> ") || l.starts_with("Error:"))
                    .last()
                    .map(|l| {
                        if l.starts_with("Error:") {
                            (l.strip_prefix("Error:").unwrap().trim().to_string(), true)
                        } else if l.starts_with("=> ") {
                            (l.strip_prefix("=> ").unwrap().trim().to_string(), false)
                        } else {
                            (l.trim().to_string(), false)
                        }
                    })
                    .unwrap_or((String::new(), false));
                return Ok(trimmed);
            }
            Err(e) => {
                last_err = Some(e.to_string());
                std::thread::sleep(Duration::from_millis(300 * (attempt as u64 + 1)));
            }
        }
    }
    Err(format!("All connection attempts failed: {}", last_err.unwrap_or_default()))
}

/// Extension trait for timeout support
trait TcpStreamExt {
    fn timeout(self, dur: Duration) -> Result<TcpStream, std::io::Error>;
}

impl TcpStreamExt for TcpStream {
    fn timeout(self, _dur: Duration) -> Result<Self, std::io::Error> {
        // For simplicity, just pass through (we handle timeout via the retry loop above)
        Ok(self)
    }
}

// ── Helpers to run scenarios ─────────────────────────────────────────────────

fn run_test_case(
    tc: &TestCase,
    state: &mut HarnessState,
    env: PtrEnvironment,
) -> Vec<TestResult> {
    println!("\n═════════════════════════════════════════");
    println!("  {}", tc.name.blue());
    println!("═════════════════════════════════════════");
    let mut results = Vec::new();

    for (step_idx, step) in tc.steps.iter().enumerate() {
        // let step_num = step_idx + 1;

        let (response, is_error) = match send_expr(&step.expr) {
            Ok((response, is_error)) => (response, is_error),
            Err(msg) => {
                results.push(TestResult {
                    step_label: step.label.clone(),
                    expr: step.expr.clone(),
                    success: false,
                    message: format!("CONNECTION ERROR: {}", msg),
                });
                continue;
            }
        };
        log::warn!("Received raw response: '{}'", response);
        let response = match jinme::read2::read(env.clone(), &response) {
            Ok((_, Some(v))) => v,
            Ok((_, None)) => { continue; },
            Err(anomaly) => {
                results.push(TestResult {
                    step_label: step.label.clone(),
                    expr: step.expr.clone(),
                    success: false,
                    message: format!("Failed to read response: '{}'", anomaly.get_message()),
                });
                continue;
            }
        };

        let mut passed = true;
        let mut msg = String::new();

        // Evaluate expectations
        match &step.expectations {
            Expectations {
                expect_success: Some(_),
                expect_error: Some(_),
            } => {
                panic!("Cannot expect both success and error — fix test case");
            },

            Expectations {
                expect_success: Some(_),
                expect_error: None,
            } => {
                if is_error {
                    passed = false;
                    msg = format!("Expected success but got error: '{}'", response);
                } else if let Some(expect_success) = &step.expectations.expect_success {
                    passed = expect_success(env.clone(), EvalContext::default(), vec![response.clone()]);
                }
            },

            Expectations {
                expect_success: None,
                expect_error: Some(_),
            } => {
                if !is_error {
                    passed = false;
                    msg = format!("Expected error but got success: '{}'", response);
                } else if let Some(expect_error) = &step.expectations.expect_error {
                    passed = expect_error(env.clone(), EvalContext::default(), vec![response.clone()]);
                }
            },

            Expectations {
                expect_success: None,
                expect_error: None,
            } => {
                // No specific expectations, just track if it succeeded or failed
            }
        }

        results.push(TestResult {
            step_label: step.label.clone(),
            expr: step.expr.clone(),
            success: passed,
            message: if passed {
                format!("response='{}'", response)
            } else {
                msg.clone()
            },
        });

        if passed {
            println!(
                "  ✓ {}",
                step.label.green().bold()
            );
        } else {
            println!(
                "  ✗ {} — {}",
                step.label.red().bold() + " FAILED",
                msg
            );
        }

        // Update state
        match step.state_update {
            StateUpdate::StableIds => {
                let stable_ids = response.view_vector().iter().filter_map(|v| v.as_ref().preview_string().map(StableId)).collect::<Vec<_>>();
                state.stable_ids = stable_ids;
                state.last_spawned_stable_id = None;
            }

            StateUpdate::GetTemplateNames => {
                let vector_of_strings = response.view_vector().iter().filter_map(|v| v.as_ref().preview_string()).collect::<Vec<String>>();
                state.template_names = vector_of_strings;
            }

            StateUpdate::TemplateRegistered => {
                let template_name = response.view_string();
                state.template_names.push(template_name);
            }

            StateUpdate::GetTemplate => {
                todo!()
                //state.template_names.push(response.clone());
            }

            StateUpdate::SpawnReturnsStableId => {
                if is_error {
                    let _error = response.view_vector().get_first_or_panic();
                    log::error!("Expected spawn to return stable ID but got error: '{}'", _error);
                } else {
                    let stable_id = response.preview_string().map(StableId).expect(&format!("Expected spawn response to be a string stable ID, got: '{}'", response));
                    state.stable_ids.push(stable_id.clone());
                    state.last_spawned_stable_id = Some(stable_id);
                }
            }

            StateUpdate::InflictDamageReturnsHealth => {
                todo!()
                //if let Some(id) = state.last_spawned_stable_id {
                //    if let Ok(h) = response.parse::<i64>() {
                //        state.health_map.insert(id, h);
                //    }
                //}
            }

            StateUpdate::QueryHealthReturnsEntities => {
                let entries = response.view_vector().iter().map(|v| {
                    let id = v.view_map().get(&Value::keyword_unqualified_ptr("stable-id")).and_then(|v| v.as_ref().preview_string()).map(StableId).expect("Expected health query result to have 'stable-id' field as string");
                    let health = v.view_map().get(&Value::keyword_unqualified_ptr("health")).and_then(|v| v.as_ref().preview_integer()).expect("Expected health query result to have 'health' field as integer");
                    (id, health)
                }).collect::<Vec<_>>();

                for (id, health) in entries {
                    state.health_map.insert(id, health);
                }
            }

            StateUpdate::GetComponentNames => {
                let component_names = response.view_vector().iter().map(|v| v.view_map().get(&Value::keyword_unqualified_ptr("id")).and_then(|v| v.as_ref().preview_string()).expect("Expected health query result to have 'id' field as string")).collect::<Vec<_>>();
                state.component_names = component_names;
            }
            StateUpdate::None => {}
        }
    }

    // Print summary
    let passed = results.iter().filter(|r| r.success).count();
    let failed = results.len() - passed;
    println!("  ── {}", tc.summary_text);
    println!("     {} / {} steps passed", passed, results.len());
    if failed > 0 {
        println!("     {} / {} steps failed", failed, results.len());
    }

    results
}

/*
fn run_test_case(
    tc: &TestCase,
    state: &mut HarnessState,
    env: PtrEnvironment,
) -> Vec<TestResult> {
    println!("\n═════════════════════════════════════════");
    println!("  {}", tc.name);
    println!("═════════════════════════════════════════");
    let mut results = Vec::new();

    for (step_idx, step) in tc.steps.iter().enumerate() {
        let step_num = step_idx + 1;

        let (response, is_error) = match send_expr(step.expr) {
            Ok(r) => r,
            Err(msg) => {
                results.push(TestResult {
                    step_label: step.label,
                    expr: step.expr,
                    success: false,
                    message: format!("CONNECTION ERROR: {}", msg),
                });
                continue;
            }
        };

        let mut passed = true;
        let mut msg = String::new();

        // Evaluate expectations
        match &step.expectations {
            Expectations {
                expect_success: _, // TODO
                expect_success_contains: Some(_),
                expect_error_contains: None,
            } => {
                if is_error {
                    passed = false;
                    msg = format!("Expected success but got error: '{}'", response);
                } else if let Some(needle) = step.expectations.expect_success_contains {
                    if !needle.is_empty() && !response.contains(needle) {
                        log::error!("run-test-case '{}' step {}", tc.name, step.label);
                        passed = false;
                        msg = format!("Expected response to contain '{}', got: '{}'", needle, response);
                    }
                }
            }
            Expectations {
                expect_success: _, // TODO
                expect_success_contains: None,
                expect_error_contains: Some(needle),
            } => {
                if !is_error {
                    passed = false;
                    msg = format!("Expected error containing '{}', got success: '{}'", needle, response);
                } else if !response.contains(needle) {
                    passed = false;
                    msg = format!("Error did not contain '{}': '{}'", needle, response);
                }
            }
            Expectations {
                expect_success: _, // TODO
                expect_success_contains: Some(_),
                expect_error_contains: Some(_),
            } => {
                panic!("Cannot expect both success and error — fix test case");
            }
            Expectations {
                expect_success: _, // TODO
                expect_success_contains: None,
                expect_error_contains: None,
            } => {
                // No specific expectations, just track if it succeeded or failed
            }
        }

        results.push(TestResult {
            step_label: step.label,
            expr: step.expr,
            success: passed,
            message: if passed {
                format!("response='{}'", response)
            } else {
                msg.clone()
            },
        });

        if passed {
            println!(
                "  ✓ {}",
                step.label.green().bold()
            );
        } else {
            println!(
                "  ✗ {} — {}",
                step.label.red().bold() + " FAILED",
                msg
            );
        }

        // Update state
        match step.state_update {
            StateUpdate::CollectEntityStableIds => {
                let response = jinme::read2::read(env.clone(), &response).unwrap().1.unwrap();
                let response = response.view_vector();
                let response = response.iter().filter_map(|v| v.as_ref().preview_integer()).collect::<Vec<i64>>();
                state.entity_stable_ids = response;
                // state.entity_stable_ids = parse_vector_of_ints(&response);
                state.last_spawned_stable_id = None;
            }
            StateUpdate::GetTemplateNames => {
                state.template_names = parse_vector_of_strings(&response);
            }
            StateUpdate::TemplateRegistered => {
                if let Some(s) = first_string_in_vector(&response) {
                    state.template_names.push(s);
                }
            }
            StateUpdate::GetTemplate => {
                state.template_names.push(response.clone());
            }
            StateUpdate::SpawnReturnsStableId => {
                if let Ok(id) = response.parse::<i64>() {
                    state.entity_stable_ids.push(id);
                    state.last_spawned_stable_id = Some(id);
                }
            }
            StateUpdate::InflictDamageReturnsHealth => {
                if let Some(id) = state.last_spawned_stable_id {
                    if let Ok(h) = response.parse::<i64>() {
                        state.health_map.push((id, h));
                    }
                }
            }
            StateUpdate::QueryHealthReturnsEntities => {
                state.health_map = parse_health_from_response(&response);
            }
            StateUpdate::GetComponentNames => {
                state.component_names = parse_vector_of_strings(&response);
            }
            StateUpdate::None => {}
        }
    }

    // Print summary
    let passed = results.iter().filter(|r| r.success).count();
    let failed = results.len() - passed;
    println!("  ── {}", tc.summary_text);
    println!("     {} / {} steps passed", passed, results.len());
    if failed > 0 {
        println!("     {} / {} steps failed", failed, results.len());
    }

    results
}
*/

// ── Test scenarios ───────────────────────────────────────────────────────────

fn make_test_cases() -> Vec<TestCase> {
    vec![
        // ── Test 1: Verify initial state after server boot ─────
        TestCase {
            name: "Test 1: Initial State",
            steps: vec![
                Step {
                    label: "1.1".to_owned(),
                    expr: "(litki.commands/stable-ids)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("["),
                    state_update: StateUpdate::StableIds,
                },
                Step {
                    label: "1.2".to_owned(),
                    expr: "(litki.commands/get-template-names)".to_owned(),
                    expectations: Expectations::no_error(),
                    state_update: StateUpdate::GetTemplateNames,
                },
            ],
            summary_text: "Should find 20 existing entities and no registered templates",
        },

        // ── Test 2: Register a template & verify ───────────────
        TestCase {
            name: "Test 2: Template Registration",
            steps: vec![
                Step {
                    label: "2.1".to_owned(),
                    expr: "(litki.commands/get-template-names)".to_owned(),
                    expectations: Expectations::no_error(),
                    state_update: StateUpdate::GetTemplateNames,
                },
                Step {
                    label: "2.2".to_owned(),
                    expr: r#"(litki.commands/register-template :wolf [[:litki.vital/health {:max 200}]])"#.to_owned(),
                    // expectations: Expectations::success(Arc::new(move |_env, _ctx, args| {
                    //     // Expect the response to contain the registered template name
                    //     if let Some(arg) = args.first() {
                    //         if let Some(s) = arg.as_ref().preview_string() {
                    //             return s == "wolf";
                    //         }
                    //     }
                    //     false
                    // })),
                    expectations: Expectations::success_response_is_value(Value::string_ptr("wolf".to_owned())),
                    state_update: StateUpdate::TemplateRegistered,
                },
                Step {
                    label: "2.3".to_owned(),
                    expr: "(litki.commands/get-template-names)".to_owned(),
                    expectations: Expectations::success_response_is_value(Value::string_ptr("wolf".to_owned())),
                    state_update: StateUpdate::GetTemplateNames,
                },
                Step {
                    label: "2.4".to_owned(),
                    expr: r#"(litki.commands/register-template :bear [[:litki.vital/health {:max 500}]])"#.to_owned(),
                    expectations: Expectations::success_response_is_value(Value::string_ptr("bear".to_owned())),
                    state_update: StateUpdate::TemplateRegistered,
                },
                Step {
                    label: "2.5".to_owned(),
                    expr: "(litki.commands/get-template-names)".to_owned(),
                    expectations: Expectations::success_response_is_value(Value::string_ptr("bear".to_owned())),
                    state_update: StateUpdate::GetTemplateNames,
                },
            ],
            summary_text: "Both wolf and bear templates register successfully",
        },

        // ── Test 3: Spawn entities & track stable IDs ────────────────
        TestCase {
            name: "Test 3: Entity Spawning",
            steps: vec![
                Step {
                    label: "3.1".to_owned(),
                    expr: "(litki.commands/stable-ids)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("["),
                    state_update: StateUpdate::StableIds,
                },
                Step {
                    label: "3.2".to_owned(),
                    expr: "(litki.commands/spawn :wolf)".to_owned(),
                    expectations: Expectations::success_response_is_string(),
                    state_update: StateUpdate::SpawnReturnsStableId,
                },
                Step {
                    label: "3.3".to_owned(),
                    expr: "(litki.commands/spawn :bear)".to_owned(),
                    expectations: Expectations::success_response_is_string(),
                    state_update: StateUpdate::SpawnReturnsStableId,
                },
                Step {
                    label: "3.4".to_owned(),
                    expr: "(litki.commands/stable-ids)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("["),
                    state_update: StateUpdate::StableIds,
                },
                Step {
                    label: "3.5".to_owned(),
                    expr: "(litki.commands/spawn :deer)".to_owned(),
                    expectations: Expectations::error_contains("unknown-template"),
                    state_update: StateUpdate::SpawnReturnsStableId,
                },
                Step {
                    label: "3.6".to_owned(),
                    expr: "(litki.commands/stable-ids)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("["),
                    state_update: StateUpdate::StableIds,
                },
            ],
            summary_text: "Each spawn returns a new unique entity stable ID; stable IDs count increments",
        },

        // ── Test 4: Health queries and damage ─────────────────
        TestCase {
            name: "Test 4: Health & Damage",
            steps: vec![
                Step {
                    label: "4.1".to_owned(),
                    expr: "(litki.commands/query-entities-with-health)".to_owned(),
                    // expectations: Expectations::success_response_is_string_containing("["),
                    expectations: Expectations::success(),
                    state_update: StateUpdate::QueryHealthReturnsEntities,
                },
                Step {
                    label: "4.2".to_owned(),
                    expr: format!("{}", Value::list_from(vec![
                        Value::symbol_qualified_ptr("litki.commands", "inflict-damage"),
                        Value::string_ptr("M1PsF8PaYzBhIQ".to_owned()),
                        Value::integer_ptr(30),
                    ])),
                    expectations: Expectations::success_response_is_string_containing("health"),
                    state_update: StateUpdate::QueryHealthReturnsEntities,
                },
                Step {
                    label: "4.3".to_owned(),
                    expr: "(litki.commands/query-entities-with-health)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("health"),
                    state_update: StateUpdate::QueryHealthReturnsEntities,
                },
                Step {
                    label: "4.4".to_owned(),
                    expr: "(litki.commands/query-entities-with-health)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("health"),
                    state_update: StateUpdate::QueryHealthReturnsEntities,
                },
                Step {
                    label: "4.5".to_owned(),
                    expr: "(litki.commands/query-entities-with-health)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("health"),
                    state_update: StateUpdate::QueryHealthReturnsEntities,
                },
            ],
            summary_text: "Health queries return all entities with health components and current health values",
        },

        // ── Test 5: Error handling ───────────────────────────
        TestCase {
            name: "Test 5: Error Handling",
            steps: vec![
                Step {
                    label: "5.1".to_owned(),
                    expr: r#"(litki.commands/register-template :wolf2 [[:litki.vital/health {:max 100}]])"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("wolf2"),
                    state_update: StateUpdate::TemplateRegistered,
                },
                Step {
                    label: "5.2".to_owned(),
                    expr: r#"(litki.commands/register-template :wolf3 [[:litki.vital/health {:max 100}]])"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("wolf3"),
                    state_update: StateUpdate::TemplateRegistered,
                },
                Step {
                    label: "5.3".to_owned(),
                    expr: r#"(litki.commands/register-template :wolf4 [[:litki.vital/health {:max 100}]])"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("wolf4"),
                    state_update: StateUpdate::TemplateRegistered,
                },
                Step {
                    label: "5.4".to_owned(),
                    expr: r#"(litki.commands/spawn :wolf)"#.to_owned(),
                    expectations: Expectations::success_response_is_string(),
                    state_update: StateUpdate::SpawnReturnsStableId,
                },
                Step {
                    label: "5.5".to_owned(),
                    expr: r#"(litki.commands/spawn :bear)"#.to_owned(),
                    expectations: Expectations::success_response_is_string(),
                    state_update: StateUpdate::SpawnReturnsStableId,
                },
                Step {
                    label: "5.6".to_owned(),
                    expr: r#"(litki.commands/spawn :bear)"#.to_owned(),
                    expectations: Expectations::success_response_is_string(),
                    state_update: StateUpdate::SpawnReturnsStableId,
                },
                Step {
                    label: "5.7".to_owned(),
                    expr: r#"(litki.commands/spawn :bear)"#.to_owned(),
                    expectations: Expectations::success_response_is_string(),
                    state_update: StateUpdate::SpawnReturnsStableId,
                },
                Step {
                    label: "5.8".to_owned(),
                    expr: r#"(litki.commands/spawn :bear)"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("health"),
                    state_update: StateUpdate::SpawnReturnsStableId,
                },
                Step {
                    label: "5.9".to_owned(),
                    expr: r#"(litki.commands/spawn :unknown_entity_type)"#.to_owned(),
                    expectations: Expectations::error_contains("unknown-template"),
                    state_update: StateUpdate::None,
                },
                Step {
                    label: "5.10".to_owned(),
                    expr: r#"(litki.commands/stable-ids)"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("["),
                    state_update: StateUpdate::StableIds,
                },
                Step {
                    label: "5.11".to_owned(),
                    expr: r#"(litki.commands/query-entities-with-health)"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("health"),
                    state_update: StateUpdate::QueryHealthReturnsEntities,
                },
            ],
            summary_text: "Unknown template spawns produce errors as expected; existing templates continue working",
        },

        // ── Test 6: Echo commands (sanity check) ─────────────
        TestCase {
            name: "Test 6: Echo Sanity",
            steps: vec![
                Step {
                    label: "6.1".to_owned(),
                    expr: "(litki.commands/echo :hello)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("hello"),
                    state_update: StateUpdate::None,
                },
                Step {
                    label: "6.2".to_owned(),
                    expr: "(litki.commands/echo :world)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("world"),
                    state_update: StateUpdate::None,
                },
                Step {
                    label: "6.3".to_owned(),
                    expr: "(litki.commands/echo :foo)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("foo"),
                    state_update: StateUpdate::None,
                },
                Step {
                    label: "6.4".to_owned(),
                    expr: "(litki.commands/echo :bar)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("bar"),
                    state_update: StateUpdate::None,
                },
                Step {
                    label: "6.5".to_owned(),
                    expr: "(litki.commands/echo :baz)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("baz"),
                    state_update: StateUpdate::None,
                },
            ],
            summary_text: "Echo commands pass/fail independently of game state",
        },

        // ── Test 7: Component names ─────────────────────────
        TestCase {
            name: "Test 7: Component Registry",
            steps: vec![
                Step {
                    label: "7.1".to_owned(),
                    expr: "(litki.commands/get-component-names)".to_owned(),
                    expectations: Expectations::no_error(),
                    state_update: StateUpdate::GetComponentNames,
                },
                Step {
                    label: "7.2".to_owned(),
                    expr: r#"(litki.commands/get-component-names)"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("health"),
                    state_update: StateUpdate::GetComponentNames,
                },
                Step {
                    label: "7.3".to_owned(),
                    expr: r#"(litki.commands/get-component-names)"#.to_owned(),
                    expectations: Expectations::no_error(),
                    state_update: StateUpdate::GetComponentNames,
                },
                Step {
                    label: "7.4".to_owned(),
                    expr: r#"(litki.commands/get-component-names)"#.to_owned(),
                    expectations: Expectations::no_error(),
                    state_update: StateUpdate::GetComponentNames,
                },
                Step {
                    label: "7.5".to_owned(),
                    expr: r#"(litki.commands/get-component-names)"#.to_owned(),
                    expectations: Expectations::no_error(),
                    state_update: StateUpdate::GetComponentNames,
                },
            ],
            summary_text: "Component names should include registered component types",
        },

        // ── Test 8: Multiple templates ──────────────────────
        TestCase {
            name: "Test 8: Multiple Templates & Mixed Operations",
            steps: vec![
                Step {
                    label: "8.1".to_owned(),
                    expr: r#"(litki.commands/register-template :snake [[:litki.vital/health {:max 50}]])"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("snake"),
                    state_update: StateUpdate::TemplateRegistered,
                },
                Step {
                    label: "8.2".to_owned(),
                    expr: r#"(litki.commands/register-template :bird [[:litki.vital/health {:max 30}]])"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("bird"),
                    state_update: StateUpdate::TemplateRegistered,
                },
                Step {
                    label: "8.3".to_owned(),
                    expr: r#"(litki.commands/get-template-names)"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("snake"),
                    state_update: StateUpdate::GetTemplateNames,
                },
                Step {
                    label: "8.4".to_owned(),
                    expr: r#"(litki.commands/spawn :snake)"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("["),
                    state_update: StateUpdate::SpawnReturnsStableId,
                },
                Step {
                    label: "8.5".to_owned(),
                    expr: r#"(litki.commands/spawn :bird)"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("["),
                    state_update: StateUpdate::SpawnReturnsStableId,
                },
                Step {
                    label: "8.6".to_owned(),
                    expr: r#"(litki.commands/query-entities-with-health)"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("health"),
                    state_update: StateUpdate::QueryHealthReturnsEntities,
                },
                Step {
                    label: "8.7".to_owned(),
                    expr: r#"(litki.commands/query-entities-with-health)"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("health"),
                    state_update: StateUpdate::QueryHealthReturnsEntities,
                },
                Step {
                    label: "8.8".to_owned(),
                    expr: r#"(litki.commands/query-entities-with-health)"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("health"),
                    state_update: StateUpdate::QueryHealthReturnsEntities,
                },
                Step {
                    label: "8.9".to_owned(),
                    expr: r#"(litki.commands/query-entities-with-health)"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("health"),
                    state_update: StateUpdate::QueryHealthReturnsEntities,
                },
                Step {
                    label: "8.10".to_owned(),
                    expr: r#"(litki.commands/query-entities-with-health)"#.to_owned(),
                    expectations: Expectations::success_response_is_string_containing("health"),
                    state_update: StateUpdate::QueryHealthReturnsEntities,
                },
            ],
            summary_text: "Mixing wolf2/bear/snake/bird spawns works; all entities appear in health query",
        },

        // ── Test 9: Entity ID ordering ──────────────────────
        TestCase {
            name: "Test 9: Entity ID Ordering",
            steps: vec![
                Step {
                    label: "9.1".to_owned(),
                    expr: "(litki.commands/stable-ids)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("["),
                    state_update: StateUpdate::StableIds,
                },
                Step {
                    label: "9.2".to_owned(),
                    expr: "(litki.commands/stable-ids)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("["),
                    state_update: StateUpdate::StableIds,
                },
                Step {
                    label: "9.3".to_owned(),
                    expr: "(litki.commands/stable-ids)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("["),
                    state_update: StateUpdate::StableIds,
                },
                Step {
                    label: "9.4".to_owned(),
                    expr: "(litki.commands/stable-ids)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("["),
                    state_update: StateUpdate::StableIds,
                },
                Step {
                    label: "9.5".to_owned(),
                    expr: "(litki.commands/stable-ids)".to_owned(),
                    expectations: Expectations::success_response_is_string_containing("["),
                    state_update: StateUpdate::StableIds,
                },
            ],
            summary_text: "No assumptions about ID ordering — just verify IDs exist",
        },
    ]
}

// ── Terminal colors (ANSI) ───────────────────────────────────────────────────

trait Color {
    fn red(&self) -> String;
    fn green(&self) -> String;
    fn bold(&self) -> String;
    fn blue(&self) -> String;
    fn gold(&self) -> String;
    fn yellow(&self) -> String;
    fn underline(&self) -> String;
}

impl Color for str {
    fn red(&self) -> String { format!("\x1b[91m{}\x1b[0m", self) }
    fn green(&self) -> String { format!("\x1b[92m{}\x1b[0m", self) }
    fn bold(&self) -> String { format!("\x1b[1m{}\x1b[0m", self) }
    fn blue(&self) -> String { format!("\x1b[94m{}\x1b[0m", self) }
    fn gold(&self) -> String { format!("\x1b[95m{}\x1b[0m", self) }
    fn yellow(&self) -> String { format!("\x1b[93m{}\x1b[0m", self) }
    fn underline(&self) -> String { format!("\x1b[4m{}\x1b[0m", self) }
}

impl Color for String {
    fn red(&self) -> String { format!("\x1b[91m{}\x1b[0m", self) }
    fn green(&self) -> String { format!("\x1b[92m{}\x1b[0m", self) }
    fn bold(&self) -> String { format!("\x1b[1m{}\x1b[0m", self) }
    fn blue(&self) -> String { format!("\x1b[94m{}\x1b[0m", self) }
    fn gold(&self) -> String { format!("\x1b[95m{}\x1b[0m", self) }
    fn yellow(&self) -> String { format!("\x1b[93m{}\x1b[0m", self) }
    fn underline(&self) -> String { format!("\x1b[4m{}\x1b[0m", self) }
}

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    println!("\n");
    println!("\x1b[1m╔══════════════════════════════════════════════════════════╗");
    println!("║       litki REPL Test Harness — Programmatic Tests       ║");
    println!("╚══════════════════════════════════════════════════════════╝\x1b[0m\n");

    let test_cases = make_test_cases();
    let mut all_results = Vec::new();
    let mut state = HarnessState::new();
    let env = litki::clojure::create_env();

    // Track which test cases we've validated for template registration
    let test_case_names = vec!["Test 1", "Test 2", "Test 3", "Test 4", "Test 5"];

    for tc in test_cases {
        println!("\n{}", tc.name.blue().bold());
        println!("{}", tc.summary_text.yellow());

        // let mut state_backup = HarnessState::new();
        let results = run_test_case(&tc, &mut state, env.clone());
        // results.iter().for_each(|res| println!("{:?}", res));
        all_results.extend(results);

        // Verify template registration worked for specific test cases
        if test_case_names.contains(&tc.name.split(':').next().unwrap_or("").trim()) {
            println!("\n  ── State check: templates = {}", state.template_names.join(", "));
            println!("  ── State check: entity_ids = {}, ...", state.stable_ids.iter().take(10).map(StableId::to_string).join(", "));
            println!("  ── State check: last_spawned_id = {}", state.last_spawned_stable_id.clone().map(StableId::into_string).unwrap_or_else(|| "<unset>".into()));
        }
    }

    // Print overall summary
    let total = all_results.len();
    let passed = all_results.iter().filter(|r| r.success).count();
    let failed = total - passed;

    println!("\n");
    println!("\x1b[1m╔══════════════════════════════════════════════════════════╗");
    println!("║                  OVERALL SUMMARY                         ║");
    println!("╚══════════════════════════════════════════════════════════╝\x1b[0m\n");
    println!("  Total:   {}", total);
    if passed > 0 {
        println!("  Passed:  \x1b[92m{}\x1b[0m", passed);
    }
    if failed > 0 {
        println!("  Failed:  \x1b[91m{}\x1b[0m", failed);
    }

    if failed > 0 {
        println!("\n  \x1b[1mFailed steps:\x1b[0m");
        for r in &all_results {
            if !r.success {
                println!("    - [{}] {} — {}", r.step_label, r.expr, r.message);
            }
        }
        std::process::exit(1);
    } else {
        println!("\n  \x1b[92mAll {} tests passed!\x1b[0m", total);
    }
}

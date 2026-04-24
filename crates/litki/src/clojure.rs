use std::{borrow::BorrowMut as _, io, rc::Rc, sync::Arc, time::Duration};
use bevy::{prelude::*, log};
use jinme::{prelude::*, handle::Handle};
use crate::script::{CommandPromise, CommandResult};

fn bind_stdioe(
    ns: &Namespace,
    in_name: &str,  // "*in*"
    get_in_handle: impl Fn() -> BufReadHandle,
    out_name: &str, // "*out*"
    get_out_handle: impl Fn() -> WriteHandle,
    err_name: &str, // "*err*"
    get_err_handle: impl Fn() -> WriteHandle,
) {
    log::info!("Creating stdin, stdout, and stderr handles.");
    ns.bind_value(in_name, Value::handle(Handle::new(get_in_handle())));
    ns.bind_value(out_name, Value::handle(Handle::new(get_out_handle())));
    ns.bind_value(err_name, Value::handle(Handle::new(get_err_handle())));
    log::info!("Created stdin, stdout, and stderr handles.");
}

pub fn create_env() -> Environment {
    let env = {
        let mut env_builder = Environment::builder();
        env_builder.set_current_namespace_var("clojure.core", "*ns*");
        env_builder.build()
    };

    let clojure_core = env.create_namespace("clojure.core");
    clojure_core.bind_value("*ns*", Value::handle(Handle::new(clojure_core.clone())));

    // (defn clojure.core/+ [& xs])
    // (clojure.core/+ a)
    // (clojure.core/+ a b)
    // (clojure.core/+ a b c ,,,)
    clojure_core.build_and_bind_function(
        "+",
        vec![
            closure_fn(FunctionArity::AtLeast(0), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                let any_arg_is_float = args.iter().map(Arc::as_ref).any(Value::is_float);
                if any_arg_is_float {
                    let mut x = 0f64;
                    for arg in args {
                        let arg_integer = value::optics::preview_integer(arg.as_ref());
                        let arg_float   = value::optics::preview_float(arg.as_ref());
                        match (arg_integer, arg_float) {
                            (Some(int), None) => x += int as f64,
                            (None, Some(float)) => x += float.as_f64(),
                            (Some(_), Some(_)) => unreachable!("value cannot be both integer and float"),
                            (None, None) => panic!("clojure.core/+ only supports integer and float arguments, but got: {:?}", arg),
                        }
                    }
                    Arc::new(Value::float(x.into()))
                } else {
                    let mut x = 0i64;
                    for arg in args {
                        let arg_integer = value::optics::preview_integer(arg.as_ref()).unwrap();
                        x += arg_integer;
                    }
                    Arc::new(Value::integer(x))
                }
            }),
        ],
    );

    // (defn clojure.core/- [x & xs])
    // (clojure.core/- a)
    // (clojure.core/- a b)
    // (clojure.core/- a b c ,,,)
    clojure_core.build_and_bind_function(
        "-",
        vec![
            closure_fn(FunctionArity::Exactly(1), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                let any_arg_is_float = args.iter().map(Arc::as_ref).any(Value::is_float);
                if any_arg_is_float {
                    let mut x = 0f64;
                    for arg in args {
                        let arg_integer = value::optics::preview_integer(arg.as_ref());
                        let arg_float   = value::optics::preview_float(arg.as_ref());
                        match (arg_integer, arg_float) {
                            (Some(int), None) => x -= int as f64,
                            (None, Some(float)) => x -= float.as_f64(),
                            (Some(_), Some(_)) => unreachable!("value cannot be both integer and float"),
                            (None, None) => panic!("clojure.core/- only supports integer and float arguments, but got: {:?}", arg),
                        }
                    }
                    Arc::new(Value::float(x.into()))
                } else {
                    let mut x = 0i64;
                    for arg in args {
                        let arg_integer = value::optics::preview_integer(arg.as_ref()).unwrap();
                        x -= arg_integer;
                    }
                    Arc::new(Value::integer(x))
                }
            }), closure_fn(FunctionArity::AtLeast(2), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                let any_arg_is_float = args.iter().map(Arc::as_ref).any(Value::is_float);
                if any_arg_is_float {
                    let mut x = 0f64;
                    for arg in args {
                        let arg_integer = value::optics::preview_integer(arg.as_ref());
                        let arg_float   = value::optics::preview_float(arg.as_ref());
                        match (arg_integer, arg_float) {
                            (Some(int), None) => x -= int as f64,
                            (None, Some(float)) => x -= float.as_f64(),
                            (Some(_), Some(_)) => unreachable!("value cannot be both integer and float"),
                            (None, None) => panic!("clojure.core/- only supports integer and float arguments, but got: {:?}", arg),
                        }
                    }
                    Arc::new(Value::float(x.into()))
                } else {
                    let mut x = value::optics::preview_integer(args[0].as_ref()).unwrap();
                    for arg in args.iter().skip(1) {
                        let arg_integer = value::optics::preview_integer(arg.as_ref()).unwrap();
                        x -= arg_integer;
                    }
                    Arc::new(Value::integer(x))
                }
            })],
    );

    // (defn clojure.core/map [f coll])
    // (clojure.core/map f coll)
    clojure_core.build_and_bind_function(
        "map",
        vec![
            closure_fn(FunctionArity::Exactly(2), |env: PtrEnvironment, args: Vec<PtrValue>| {
                let f = args[0].clone();
                let coll = args[1].clone();
                match coll.as_ref() {
                    Value::List(list, _) => Value::new_list_ptr(list.iter().map(|element| apply(env.clone(), f.clone(), vec![element.to_owned()])).collect()),
                    Value::Vector(vector, _) => Value::new_vector_ptr(vector.iter().map(|element| apply(env.clone(), f.clone(), vec![element.to_owned()])).collect()),
                    Value::Set(set, _) => Value::new_set_ptr(set.iter().map(|element| apply(env.clone(), f.clone(), vec![element.to_owned()])).collect()),
                    Value::Map(map, _) => Value::new_vector_ptr(map.iter().map(|(k, v)| {
                        let new_kv = apply(env.clone(), f.clone(), vec![
                            Arc::new(Value::vector_from(vec![
                                k.to_owned(),
                                v.to_owned(),
                            ])),
                        ]);
                        new_kv
                    })
                    .collect::<Vec<_>>()
                ),
                    _ => panic!("clojure.core/map requires a list, vector, set, or map as the second argument, but got: {:?}", coll),
                }
            }),
        ],
    );

    // (defn clojure.core/prn [v & vs])
    // (clojure.core/prn)
    // (clojure.core/prn x)
    // (clojure.core/prn x y)
    // (clojure.core/prn x y z ,,,)
    clojure_core.build_and_bind_function(
        "prn",
        vec![
            closure_fn(FunctionArity::AtLeast(0), |env: PtrEnvironment, args: Vec<PtrValue>| {
                let ns = env.get_namespace_or_panic("clojure.core");
                let out = ns.get_value_or_panic("*out*");

                // Extract the WriteHandle's inner Rc in the minimal scope
                let out = out.try_get_handle_ref::<WriteHandle>()
                    .expect(&format!("*out* must be a WriteHandle, but was: {:?}", out))
                    .inner();

                // Now use the Arc without any borrows on the Handle
                let mut out = out.lock().unwrap();
                let mut first = true;
                for arg in args.iter() {
                    if !first {
                        write!(out, " ").unwrap();
                    }
                    first = false;
                    write!(out, "{arg}").unwrap();
                }
                writeln!(out).unwrap();

                Value::nil().into()
            }),
        ],
    );

    // (clojure.core/symbol name)
    // (clojure.core/symbol ns_name name)
    clojure_core.build_and_bind_function(
        "symbol",
        vec![
            closure_fn(FunctionArity::Exactly(1), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                let name = value::optics::preview_string(args[0].as_ref())
                    .unwrap_or_else(|| panic!("symbol name must be a string, got {:?}", args[0]));
                Arc::new(Value::symbol_unqualified(&name))
            }), closure_fn(FunctionArity::Exactly(2), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                let ns_name = value::optics::preview_string(args[0].as_ref())
                    .unwrap_or_else(|| panic!("symbol namespace must be a string, got {:?}", args[0]));
                let name = value::optics::preview_string(args[1].as_ref())
                    .unwrap_or_else(|| panic!("symbol name must be a string, got {:?}", args[1]));
                Arc::new(Value::symbol_qualified(&ns_name, &name))
            }),
        ],
    );

    // (defn clojure.core/resolve [symbol])
    // (clojure.core/resolve symbol)
    clojure_core.build_and_bind_function(
        "resolve",
        vec![
            closure_fn(FunctionArity::Exactly(1), |env: PtrEnvironment, args: Vec<PtrValue>| {
                let symbol = value::optics::preview_symbol(args[0].as_ref())
                    .unwrap_or_else(|| panic!("clojure.core/resolve requires a symbol argument, but got: {:?}", args[0]));
                let var = try_resolve(env, &symbol).expect(&format!("unable to resolve: {}", symbol));
                Arc::new(Value::var(var))
            }),
        ],
    );

    // (clojure.core/deref var)
    // (clojure.core/deref promise timeout-ms timeout-val)
    clojure_core.build_and_bind_function(
        "deref",
        vec![
            // 1-arg variant: (deref x) - blocks indefinitely for promises
            closure_fn(FunctionArity::Exactly(1), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                let derefee = args.first().unwrap().to_owned();

                // Try to handle as CommandPromise (stored in Handle)
                if let Some(handle) = value::optics::preview_handle(derefee.as_ref()) {
                    if let Some(promise) = handle.downcast_ref::<CommandPromise>() {
                        let result = promise.deref_blocking();
                        return match result {
                            crate::script::CommandResult::Success(v) => v,
                            crate::script::CommandResult::Error(v) => v,
                            crate::script::CommandResult::Pending => {
                                Arc::new(Value::string("Command still pending".to_string()))
                            }
                        };
                    }
                }

                // Try to handle as Var
                value::optics::preview_var(derefee.as_ref())
                    .and_then(|var| var.deref())
                    .map(|v| v.clone())
                    .unwrap_or(derefee)
            }),
            // 3-arg variant: (deref promise timeout-ms timeout-val)
            closure_fn(FunctionArity::Exactly(3), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                let derefee = args.get(0).unwrap().to_owned();
                let timeout_ms = args.get(1).unwrap().to_owned();
                let timeout_val = args.get(2).unwrap().to_owned();

                // Extract timeout in milliseconds
                let timeout_ms_num = value::optics::preview_integer(timeout_ms.as_ref())
                    .unwrap_or(5000);
                let timeout = Duration::from_millis(timeout_ms_num as u64);

                // Try to handle as CommandPromise
                if let Some(handle) = value::optics::preview_handle(derefee.as_ref()) {
                    if let Some(promise) = handle.downcast_ref::<CommandPromise>() {
                        return match promise.deref_timeout(timeout) {
                            Some(result) => {
                                match result {
                                    crate::script::CommandResult::Success(v) => v,
                                    crate::script::CommandResult::Error(v) => v,
                                    crate::script::CommandResult::Pending => timeout_val,
                                }
                            }
                            None => timeout_val,
                        };
                    }
                }

                // For Var with timeout, just return the value or timeout-val
                value::optics::preview_var(derefee.as_ref())
                    .and_then(|var| var.deref())
                    .map(|v| v.clone())
                    .unwrap_or(timeout_val)
            }),
        ],
    );

    // (clojure.core/realized? x) - Check if a promise/var is resolved without blocking
    clojure_core.build_and_bind_function(
        "realized?",
        vec![
            closure_fn(FunctionArity::Exactly(1), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                let x = args.first().unwrap().to_owned();

                // Check if it's a CommandPromise
                if let Some(handle) = value::optics::preview_handle(x.as_ref()) {
                    if let Some(promise) = handle.downcast_ref::<CommandPromise>() {
                        return Arc::new(Value::boolean(promise.is_resolved()));
                    }
                }

                // Check if it's a Var with a value
                if let Some(var) = value::optics::preview_var(x.as_ref()) {
                    return Arc::new(Value::boolean(var.deref().is_some()));
                }

                // Otherwise, it's "realized" (not a promise/var)
                Arc::new(Value::boolean(true))
            }),
        ],
    );

    // (clojure.core/eval value)
    clojure_core.build_and_bind_function(
        "eval",
        vec![
            closure_fn(FunctionArity::Exactly(1), |env: PtrEnvironment, args: Vec<PtrValue>| eval(env, args[0].clone()))],
    );

    // (clojure.core/apply f)
    // (clojure.core/apply f args)
    clojure_core.build_and_bind_function(
        "apply",
        vec![
            closure_fn(FunctionArity::AtLeast(1), |env: PtrEnvironment, args: Vec<PtrValue>| apply(env, args[0].clone(), args[1..].to_vec()))],
    );

    clojure_core.build_and_bind_function(
        "list",
        vec![
            closure_fn(FunctionArity::AtLeast(0), |_: PtrEnvironment, args: Vec<PtrValue>| Value::new_list_ptr(args))],
    );

    // (clojure.core/all-ns)
    clojure_core.build_and_bind_function(
        "all-ns",
        vec![
            closure_fn(FunctionArity::Exactly(0), |env: PtrEnvironment, _args: Vec<PtrValue>| {
                Value::new_list_ptr(
                    env.all_namespaces()
                        .into_iter()
                        .map(|ns| Arc::new(Value::handle(Handle::new(ns))))
                        .collect()
                )
            }),
        ],
    );

    // (clojure.core/ns-map ns-name-symbol)
    // (clojure.core/ns-map (symbol "clojure.core"))
    clojure_core.build_and_bind_function(
        "ns-map",
        vec![
            closure_fn(FunctionArity::AtLeast(1), |env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let ns_sym = value::optics::preview_symbol(args.first().expect("clojure.core/ns-map requires at least one argument: the namespace to map").as_ref())
                    .expect("ns-map first argument must be a symbol naming the namespace to map");
                let ns = env.get_namespace_or_panic(ns_sym.name());
                Arc::new(Value::map_from(
                    ns.entries().into_iter()
                      .map(|(sym_name, var)| (
                        Arc::new(Value::symbol_unqualified(&sym_name)),
                        Arc::new(Value::var(var)),
                    )).collect::<Vec<(_, _)>>()
                ))
            }),
        ],
    );

    // (clojure.core/ns-map-2 (symbol "clojure.core"))
    clojure_core.build_and_bind_function(
        "ns-map-2",
        vec![
            closure_fn(FunctionArity::AtLeast(1), |env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let ns_name_symbol = args.first()
                    .expect("ns-map requires at least one argument: an unqualified symbol naming the namespace to introspect");
                let ns_name_symbol = value::optics::preview_symbol(ns_name_symbol.as_ref())
                    .expect("ns-map's argument must be an unqualified symbol naming the namespace to introspect");
                let ns_name = ns_name_symbol.name();
                let ns = env.get_namespace_or_panic(ns_name);
                Arc::new(Value::map_from(
                    ns.entries().into_iter()
                      .map(|(sym_name, var)| (
                        Arc::new(Value::symbol_qualified(ns_name, &sym_name)),
                        match var.deref() {
                            Some(value) => value.clone(),
                            None => Value::var(var.clone()).into(),
                        }
                    )).collect::<Vec<(_, _)>>()
                ))
            }),
        ],
    );

    // (clojure.core/in-ns sym)
    clojure_core.build_and_bind_function(
        "in-ns",
        vec![
            closure_fn(FunctionArity::Exactly(1), |env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let sym = value::optics::preview_symbol(args[0].as_ref())
                    .expect("in-ns argument must be a symbol");
                let ns = env.create_namespace(sym.name());
                env.get_namespace_or_panic("clojure.core")
                   .bind_value("*ns*", Value::handle(Handle::new(ns.clone())));
                Arc::new(Value::handle(Handle::new(ns)))
            }),
        ],
    );

    // (clojure.core/create-ns sym)
    clojure_core.build_and_bind_function(
        "create-ns",
        vec![
            closure_fn(FunctionArity::Exactly(1), |env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let sym = value::optics::preview_symbol(args[0].as_ref())
                    .expect("create-ns argument must be a symbol");
                let ns = env.create_namespace(sym.name());
                Arc::new(Value::handle(Handle::new(ns)))
            }),
        ],
    );

    // (clojure.core/find-ns sym)
    clojure_core.build_and_bind_function(
        "find-ns",
        vec![
            closure_fn(FunctionArity::Exactly(1), |env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let sym = value::optics::preview_symbol(args[0].as_ref())
                    .expect("find-ns argument must be a symbol");
                match env.try_get_namespace(sym.name()) {
                    Some(ns) => Arc::new(Value::handle(Handle::new(ns))),
                    None => Value::nil_ptr(),
                }
            }),
        ],
    );

    // (clojure.core/ns-name ns)
    clojure_core.build_and_bind_function(
        "ns-name",
        vec![
            closure_fn(FunctionArity::Exactly(1), |_env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let ns = args[0].try_get_handle::<PtrNamespace>()
                    .expect("ns-name argument must be a namespace handle");
                Arc::new(Value::symbol_unqualified(ns.name_str()))
            }),
        ],
    );

    // (clojure.core/ns-publics ns)
    clojure_core.build_and_bind_function(
        "ns-publics",
        vec![
            closure_fn(FunctionArity::Exactly(1), |_env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let ns = args[0].try_get_handle::<PtrNamespace>()
                    .expect("ns-publics argument must be a namespace handle");
                let refers = ns.refers();
                Arc::new(Value::map_from(
                    ns.entries().into_iter()
                        .filter(|(name, _)| !refers.contains_key(&SymbolUnqualified::new(name.as_str())))
                        .map(|(sym_name, var)| (
                            Arc::new(Value::symbol_unqualified(&sym_name)),
                            Arc::new(Value::var(var)),
                        ))
                        .collect::<Vec<_>>(),
                ))
            }),
        ],
    );

    // (clojure.core/ns-imports ns)
    clojure_core.build_and_bind_function(
        "ns-imports",
        vec![
            closure_fn(FunctionArity::Exactly(1), |_env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let ns = args[0].try_get_handle::<PtrNamespace>()
                    .expect("ns-imports argument must be a namespace handle");
                Arc::new(Value::map_from(
                    ns.imports().into_iter()
                        .map(|(sym, fqn)| (
                            Arc::new(Value::symbol_unqualified(sym.name())),
                            Arc::new(Value::string(fqn)),
                        ))
                        .collect::<Vec<_>>(),
                ))
            }),
        ],
    );

    // (clojure.core/ns-aliases ns)
    clojure_core.build_and_bind_function(
        "ns-aliases",
        vec![
            closure_fn(FunctionArity::Exactly(1), |_env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let ns = args[0].try_get_handle::<PtrNamespace>()
                    .expect("ns-aliases argument must be a namespace handle");
                Arc::new(Value::map_from(
                    ns.aliases().into_iter()
                        .map(|(sym, alias_ns)| (
                            Arc::new(Value::symbol_unqualified(sym.name())),
                            Arc::new(Value::handle(Handle::new(alias_ns))),
                        ))
                        .collect::<Vec<_>>(),
                ))
            }),
        ],
    );

    // (clojure.core/ns-refers ns)
    clojure_core.build_and_bind_function(
        "ns-refers",
        vec![
            closure_fn(FunctionArity::Exactly(1), |_env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let ns = args[0].try_get_handle::<PtrNamespace>()
                    .expect("ns-refers argument must be a namespace handle");
                Arc::new(Value::map_from(
                    ns.refers().into_iter()
                        .map(|(sym, var)| (
                            Arc::new(Value::symbol_unqualified(sym.name())),
                            Arc::new(Value::var(var)),
                        ))
                        .collect::<Vec<_>>(),
                ))
            }),
        ],
    );

    // (clojure.core/ns-resolve ns sym)
    clojure_core.build_and_bind_function(
        "ns-resolve",
        vec![
            closure_fn(FunctionArity::Exactly(2), |env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let ns = args[0].try_get_handle::<PtrNamespace>()
                    .expect("ns-resolve first argument must be a namespace handle");
                let sym = value::optics::preview_symbol(args[1].as_ref())
                    .expect("ns-resolve second argument must be a symbol");
                match &sym {
                    Symbol::Unqualified(s) => match ns.try_get_var(s.name()) {
                        Ok(var) => Arc::new(Value::var(var)),
                        Err(_) => Value::nil_ptr(),
                    },
                    Symbol::Qualified(s) => {
                        match env.try_get_namespace(s.namespace())
                            .and_then(|ns| ns.try_get_var(s.name()).ok())
                        {
                            Some(var) => Arc::new(Value::var(var)),
                            None => Value::nil_ptr(),
                        }
                    },
                }
            }),
        ],
    );

    // (clojure.core/resolve sym)
    clojure_core.build_and_bind_function(
        "resolve",
        vec![
            closure_fn(FunctionArity::Exactly(1), |env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                //let sym = value::optics::preview_symbol(&args[0])
                //    .map(|sym| jinme::core::try_resolve(env.clone(), sym))
                //    .unwrap_or_else(|| Ok(Value::nil_ptr()));
                let current_ns = env.get_namespace_or_panic("clojure.core")
                    .try_get_handle::<PtrNamespace>("*ns*")
                    .expect("*ns* must be a namespace handle");
                let sym = value::optics::preview_symbol(args[0].as_ref())
                    .expect("resolve argument must be a symbol");
                match &sym {
                    Symbol::Unqualified(s) => match current_ns.try_get_var(s.name()) {
                        Ok(var) => Arc::new(Value::var(var)),
                        Err(_) => Value::nil_ptr(),
                    },
                    Symbol::Qualified(s) => {
                        match env.try_get_namespace(s.namespace())
                            .and_then(|ns| ns.try_get_var(s.name()).ok())
                        {
                            Some(var) => Arc::new(Value::var(var)),
                            None => Value::nil_ptr(),
                        }
                    },
                }
            }),
        ],
    );

    // (clojure.core/remove-ns sym)
    clojure_core.build_and_bind_function(
        "remove-ns",
        vec![
            closure_fn(FunctionArity::Exactly(1), |env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let sym = value::optics::preview_symbol(args[0].as_ref())
                    .expect("remove-ns argument must be a symbol");
                env.remove_namespace(sym.name());
                Value::nil_ptr()
            }),
        ],
    );

    // (clojure.core/ns-unalias ns alias)
    clojure_core.build_and_bind_function(
        "ns-unalias",
        vec![
            closure_fn(FunctionArity::Exactly(2), |_env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let ns = args[0].try_get_handle::<PtrNamespace>()
                    .expect("ns-unalias first argument must be a namespace handle");
                let alias = value::optics::preview_symbol(args[1].as_ref())
                    .expect("ns-unalias second argument must be a symbol");
                ns.remove_alias(alias.name());
                Value::nil_ptr()
            }),
        ],
    );

    // (clojure.core/ns-unmap ns sym)
    clojure_core.build_and_bind_function(
        "ns-unmap",
        vec![
            closure_fn(FunctionArity::Exactly(2), |_env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let ns = args[0].try_get_handle::<PtrNamespace>()
                    .expect("ns-unmap first argument must be a namespace handle");
                let sym = value::optics::preview_symbol(args[1].as_ref())
                    .expect("ns-unmap second argument must be a symbol");
                ns.remove_var(sym.name());
                Value::nil_ptr()
            }),
        ],
    );

    // (clojure.core/intern ns sym)
    // (clojure.core/intern ns sym val)
    clojure_core.build_and_bind_function(
        "intern",
        vec![
            closure_fn(FunctionArity::Exactly(2), |env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let ns = match args[0].try_get_handle::<PtrNamespace>() {
                    Ok(ns) => ns,
                    Err(_) => {
                        let sym = value::optics::preview_symbol(args[0].as_ref())
                            .expect("intern first arg must be a namespace handle or symbol");
                        env.get_namespace_or_panic(sym.name())
                    }
                };
                let var_sym = value::optics::preview_symbol(args[1].as_ref())
                    .expect("intern second argument must be a symbol");
                let var = match ns.try_get_var(var_sym.name()) {
                    Ok(existing) => existing,
                    Err(_) => {
                        let new_var = Arc::new(Var::new_unbound());
                        ns.insert_var(var_sym.name(), new_var.clone());
                        new_var
                    }
                };
                Arc::new(Value::var(var))
            }), closure_fn(FunctionArity::Exactly(3), |env: PtrEnvironment, args: Vec<PtrValue>| -> PtrValue {
                let ns = match args[0].try_get_handle::<PtrNamespace>() {
                    Ok(ns) => ns,
                    Err(_) => {
                        let sym = value::optics::preview_symbol(args[0].as_ref())
                            .expect("intern first arg must be a namespace handle or symbol");
                        env.get_namespace_or_panic(sym.name())
                    }
                };
                let var_sym = value::optics::preview_symbol(args[1].as_ref())
                    .expect("intern second argument must be a symbol");
                let var = match ns.try_get_var(var_sym.name()) {
                    Ok(existing) => existing,
                    Err(_) => {
                        let new_var = Arc::new(Var::new_unbound());
                        ns.insert_var(var_sym.name(), new_var.clone());
                        new_var
                    }
                };
                var.bind(args[2].clone());
                Arc::new(Value::var(var))
            }),
        ],
    );

    // (clojure.core/meta obj)
    clojure_core.build_and_bind_function(
        "meta",
        vec![
            closure_fn(FunctionArity::Exactly(1), |_env, args: Vec<_>| {
                let obj = args.first().unwrap();
                value::optics::preview_meta(obj)
                    .map(|x| x.as_ref().clone())
                    .map(Value::map_ptr)
                    .unwrap_or_else(Value::nil_ptr)
            }),
        ],
    );

    // (clojure.core/with-meta obj meta)
    clojure_core.build_and_bind_function(
        "with-meta",
        vec![
            closure_fn(FunctionArity::Exactly(2), |_env, args: Vec<PtrValue>| {
                let mut args = args.into_iter();
                let value = args.next().unwrap();
                let meta = args.next().unwrap();
                let meta = value::optics::preview_map(meta.as_ref()).expect("with-meta meta argument must be a map");
                value.with_meta_ptr(Some(Arc::new(meta)))
            }),
        ],
    );

    clojure_core.build_and_bind_function(
        "count",
        vec![
            closure_fn(FunctionArity::Exactly(1), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                let arg = args.first().unwrap();
                match arg.as_ref() {
                    Value::List(list, _) => Arc::new(Value::integer(list.len() as i64)),
                    Value::Vector(vector, _) => Arc::new(Value::integer(vector.len() as i64)),
                    Value::Set(set, _) => Arc::new(Value::integer(set.len() as i64)),
                    Value::Map(map, _) => Arc::new(Value::integer(map.len() as i64)),
                    _ => panic!("count only supports list, vector, set, and map arguments, but got: {:?}", arg),
                }
            })
        ],
    );

    // (clojure.core/get m k)
    // (clojure.core/get m k d)
    clojure_core.build_and_bind_function(
        "get",
        vec![closure_fn(FunctionArity::Exactly(2), |env: PtrEnvironment, args: Vec<PtrValue>| {
            let mut args = args.into_iter();
            let m = args.next().unwrap();
            if m.is_nil() { return m; }
            let k = args.next().unwrap();
            let d = Value::nil_ptr();
            // (clojure.core/get m k nil)
            env.get_namespace_or_panic("clojure.core")
                .get_function_or_panic("get")
                .invoke(env.clone(), vec![m, k, d])
        }), closure_fn(FunctionArity::Exactly(3), |_env: PtrEnvironment, args: Vec<PtrValue>| {
            let mut args = args.into_iter();
            let m = args.next().unwrap();
            if m.is_nil() { return m; }
            let k = args.next().unwrap();
            let d = args.next().unwrap();
            match m.as_ref() {
                Value::Nil(_) => m,
                Value::Vector(vector, _) => {
                    let expect_message = format!("clojure.core/get vector branch: key is not an integer >= 0: {}", k.as_ref());
                    let k = value::optics::preview_integer(k.as_ref()).expect(&expect_message) as usize;
                    vector.get_nth_or(k, d)
                },
                Value::Map(map, _) => map.get_or(&k, d),
                _ => Value::nil_ptr(),
            }
        })],
    );

    // (clojure.core/get-in m ks)
    // (clojure.core/get-in m ks d)
    clojure_core.build_and_bind_function(
        "get-in",
        vec![(
            closure_fn(FunctionArity::Exactly(2), |env: PtrEnvironment, args: Vec<PtrValue>| {
                let mut args = args.into_iter();
                let m = args.next().unwrap();
                if m.is_nil() { return m; }
                let ks = args.next().unwrap();
                let ks = value::optics::preview_vector_ref(ks.as_ref()).unwrap();
                if ks.len() == 0 { return m; }
                let ks = ks.iter().into_iter().map(PtrValue::clone);
                let get_fn = env.get_namespace_or_panic("clojure.core").get_function_or_panic("get");
                let mut v = m;
                for k in ks {
                    v = get_fn.invoke(env.clone(), vec![v, k]);
                    if v.is_nil() { return v; }
                }
                v
            })
        )],
    );

    // (clojure.core/assoc m k v & kvs)
    clojure_core.build_and_bind_function(
        "assoc",
        vec![
            closure_fn(FunctionArity::AtLeast(3), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                let m = args[0].to_owned();
                // Validate even number of key-value pairs
                if (args.len() - 1) % 2 != 0 {
                    panic!("clojure.core/assoc requires an even number of key-value arguments");
                }
                let m = match m.as_ref() {
                    Value::Nil(meta) => Arc::new(Value::new_map_empty().with_meta(meta.clone())),
                    Value::Map(..) => m,
                    Value::Vector(..) => m,
                    _ => panic!("clojure.core/assoc requires a nil, map, or vector as the first argument"),
                };
                match m.as_ref() {
                    Value::Map(map, meta) => {
                        let mut new_map = map.clone();
                        // Apply all key-value pairs
                        for i in (1..args.len()).step_by(2) {
                            let k = args[i].to_owned();
                            let v = args[i + 1].to_owned();
                            new_map.insert(k, v);
                        }
                        Arc::new(Value::Map(new_map, meta.clone()))
                    },
                    Value::Vector(vector, meta) => {
                        let new_vector = vector.clone();
                        // TODO:
                        // - get k as int
                        // - bounds check
                        // - insert at index
                        Arc::new(Value::Vector(new_vector, meta.clone()))
                    },
                    _ => todo!(),
                }
            }),
        ],
    );

    // (clojure.core/assoc-in m ks v)
    clojure_core.build_and_bind_function(
        "assoc-in",
        vec![
            closure_fn(FunctionArity::Exactly(3), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                let m = args[0].to_owned();
                let ks_arg = args[1].to_owned();
                let v = args[2].to_owned();

                // Extract keys vector and panic if not a vector
                let ks_vec = value::optics::preview_vector_ref(ks_arg.as_ref())
                    .expect("clojure.core/assoc-in requires a vector of keys");

                // Convert to Vec for easier indexing
                let ks: Vec<PtrValue> = ks_vec.iter().cloned().collect();

                // If keys is empty at the top level, assoc with nil
                if ks.is_empty() {
                    let nil_key = Value::nil_ptr();
                    match m.as_ref() {
                        Value::Nil(meta) => {
                            let mut map = Map::new_empty();
                            map.insert(nil_key, v);
                            Arc::new(Value::Map(map, meta.clone()))
                        }
                        Value::Map(map, meta) => {
                            let mut new_map = map.clone();
                            new_map.insert(nil_key, v);
                            Arc::new(Value::Map(new_map, meta.clone()))
                        }
                        Value::Vector(_vec, _) => {
                            // For vectors, nil is not a valid integer key
                            let err_msg = format!("Key must be integer");
                            let _ = value::optics::preview_integer(nil_key.as_ref())
                                .expect(&err_msg);
                            todo!()
                        }
                        _ => m,
                    }
                } else {
                    // Define recursive helper for processing non-empty key paths
                    fn assoc_in_recursive(
                        m: PtrValue,
                        ks: &[PtrValue],
                        v: PtrValue,
                    ) -> PtrValue {
                        if ks.is_empty() {
                            // Base case: no more keys, return the value
                            return v;
                        }
                        // Recursive case: descend with first key, recurse with rest
                        let first_key = ks[0].clone();
                        let rest_keys = &ks[1..];

                        match m.as_ref() {
                            Value::Nil(_) => {
                                // Nil is treated as empty map when descending
                                let empty_map = Arc::new(Value::new_map_empty());
                                let nested = assoc_in_recursive(empty_map, rest_keys, v);
                                let mut result_map = Map::new_empty();
                                result_map.insert(first_key, nested);
                                Arc::new(Value::Map(result_map, None))
                            }
                            Value::Map(map, meta) => {
                                let current = map.get_or_nil(&first_key);
                                let nested = assoc_in_recursive(current, rest_keys, v);
                                let mut new_map = map.clone();
                                new_map.insert(first_key, nested);
                                Arc::new(Value::Map(new_map, meta.clone()))
                            }
                            Value::Vector(vec, meta) => {
                                let err_msg = format!("Key must be integer: {}", first_key.as_ref());
                                let idx = value::optics::preview_integer(first_key.as_ref())
                                    .expect(&err_msg) as usize;

                                // Panic if index is out of bounds (matching Babashka behavior)
                                if idx >= vec.len() {
                                    panic!("java.lang.IndexOutOfBoundsException: {}", idx);
                                }

                                let current = vec.get_nth_or_nil(idx);
                                let nested = assoc_in_recursive(current, rest_keys, v);

                                // Build a new vector with the updated value at index
                                let mut vec_items: Vec<PtrValue> = vec.iter().cloned().collect();
                                vec_items[idx] = nested;
                                Arc::new(Value::Vector(Vector::from(vec_items), meta.clone()))
                            }
                            _ => m,
                        }
                    }

                    assoc_in_recursive(m, &ks, v)
                }
            }),
        ],
    );

    // (clojure.core/dissoc m k & ks)
    clojure_core.build_and_bind_function(
        "dissoc",
        vec![
            closure_fn(FunctionArity::AtLeast(2), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                let m = args[0].to_owned();
                match m.as_ref() {
                    Value::Map(map, meta) => {
                        let mut new_map = map.clone();
                        // Remove all specified keys
                        for i in 1..args.len() {
                            let k = &args[i];
                            new_map.remove(k);
                        }
                        Arc::new(Value::Map(
                            new_map,
                            meta.clone(),
                        ))
                    },
                    _ => m,
                }
            }),
        ],
    );

    // (clojure.core/keys m)
    clojure_core.build_and_bind_function(
        "keys",
        vec![
            closure_fn(FunctionArity::Exactly(1), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                let Value::Map(m, _) = args[0].as_ref() else { unimplemented!() };
                List::new_value_ptr(m.keys())
            }),
        ],
    );

    // (clojure.core/vals m)
    clojure_core.build_and_bind_function(
        "vals",
        vec![
            closure_fn(FunctionArity::Exactly(1), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                let Value::Map(m, _) = args[0].as_ref() else { unimplemented!() };
                List::new_value_ptr(m.values())
            }),
        ],
    );

    // (clojure.core/first coll)
    clojure_core.build_and_bind_function(
        "first",
        vec![
            closure_fn(FunctionArity::Exactly(1), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                match args[0].as_ref() {
                    Value::List(list, _) => list.get_first().unwrap_or_else(Value::nil_ptr),
                    Value::Vector(vec, _) => vec.get_first().unwrap_or_else(Value::nil_ptr),
                    _ => Value::nil_ptr(),
                }
            }),
        ],
    );

    // (clojure.core/second coll)
    clojure_core.build_and_bind_function(
        "second",
        vec![
            closure_fn(FunctionArity::Exactly(1), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                match args[0].as_ref() {
                    Value::List(list, _) => list.get_second().unwrap_or_else(Value::nil_ptr),
                    Value::Vector(vec, _) => vec.get_second().unwrap_or_else(Value::nil_ptr),
                    _ => Value::nil_ptr(),
                }
            }),
        ],
    );

    // (clojure.core/last coll)
    clojure_core.build_and_bind_function(
        "last",
        vec![
            closure_fn(FunctionArity::Exactly(1), |_env: PtrEnvironment, args: Vec<PtrValue>| {
                match args[0].as_ref() {
                    Value::List(list, _) => list.get_last().unwrap_or_else(Value::nil_ptr),
                    Value::Vector(vec, _) => vec.get_last().unwrap_or_else(Value::nil_ptr),
                    _ => Value::nil_ptr(),
                }
            }),
        ],
    );

    bind_stdioe(
        clojure_core.as_ref(),
        "*in*", || BufReadHandle::new(io::BufReader::new(io::stdin())),
        "*out*", || WriteHandle::new(io::stdout()),
        "*err*", || WriteHandle::new(io::stderr()),
    );

    env
}

/// Bind the execute-command function (`!`) to the Clojure environment
/// 
/// This creates a single entry point for executing registered commands:
/// - (! :cmd-name arg1 arg2)           → blocks indefinitely
/// - (! :cmd-name arg1 arg2 :timeout 5000 :default result)  → timeout in ms
pub fn bind_execute_command_function(
    env: &Environment,
    command_registry: &crate::script::CommandRegistry,
    command_buffer: &crate::script::CommandBuffer,
    default_timeout: Duration,
) {
    use crate::script::CommandResult;

    let command_registry = command_registry.clone();
    let command_buffer = command_buffer.clone();

    let clojure_core = env.get_namespace_or_panic("clojure.core");

    clojure_core.build_and_bind_function(
        "!",
        vec![
            closure_fn(FunctionArity::AtLeast(1), move |_env: PtrEnvironment, args: Vec<PtrValue>| {
                if args.is_empty() {
                    return Arc::new(Value::string("! requires at least a command name".to_string()));
                }

                // First arg is command name (as keyword or symbol)
                let cmd_name_value = &args[0];
                let cmd_name = if let Some(kw) = value::optics::preview_keyword(cmd_name_value) {
                    kw.name().to_string()
                } else if let Some(sym) = value::optics::preview_symbol(cmd_name_value) {
                    sym.name().to_string()
                } else {
                    return Arc::new(Value::string("Command name must be a keyword or symbol".to_string()));
                };

                // Parse optional keyword arguments from the end
                let mut timeout = default_timeout;
                let mut default_on_timeout: Option<PtrValue> = None;
                let mut cmd_args = Vec::new();

                let mut i = 1;
                while i < args.len() {
                    let arg = &args[i];

                    // Check if it's a keyword
                    if let Some(kw) = value::optics::preview_keyword(arg) {
                        match kw.name() {
                            "timeout" => {
                                // Next arg should be the timeout in milliseconds
                                if i + 1 < args.len() {
                                    if let Some(ms) = value::optics::preview_integer(&args[i + 1]) {
                                        timeout = Duration::from_millis(ms as u64);
                                    }
                                    i += 2;
                                    continue;
                                }
                            }
                            "default" => {
                                // Next arg is the default value
                                if i + 1 < args.len() {
                                    default_on_timeout = Some(args[i + 1].clone());
                                    i += 2;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }

                    // Not a keyword arg, add to command args
                    cmd_args.push(arg.clone());
                    i += 1;
                }

                // Execute the command
                match command_registry.execute_with_promise(&cmd_name, cmd_args, &command_buffer) {
                    Ok(promise) => {
                        match promise.deref_timeout(timeout) {
                            Some(CommandResult::Success(v)) => v,
                            Some(CommandResult::Error(e)) => {
                                log::error!("Command '{cmd_name}' returned error: {e}");
                                e
                            }
                            Some(CommandResult::Pending) | None => {
                                if let Some(default) = default_on_timeout {
                                    log::warn!("Command '{cmd_name}' timed out, using default");
                                    default
                                } else {
                                    let err_msg = format!("Command '{cmd_name}' timed out");
                                    log::error!("{err_msg}");
                                    Arc::new(Value::string(err_msg))
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to execute command '{cmd_name}': {e}");
                        Arc::new(Value::string(format!("Failed to execute command '{cmd_name}': {e}")))
                    }
                }
            }),
        ],
    );

    clojure_core.insert_var(
        "execute-command",
        clojure_core.get_var_or_panic("!"),
    );
}

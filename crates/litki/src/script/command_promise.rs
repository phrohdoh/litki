use std::sync::{Arc, Mutex, Condvar};
use std::time::Duration;
use jinme::dependency::as_any::AsAny;
use jinme::prelude::*;

use crate::script::CommandResult;

/// Inner shared state for a command promise
struct CommandPromiseInner {
    result: Mutex<Option<CommandResult>>,
    condvar: Condvar,
}

/// A promise that will eventually resolve to a CommandResult
/// 
/// Can be shared across threads and cloned. Multiple threads can
/// wait on the same promise (all will be notified when resolved).
#[derive(Clone)]
pub struct CommandPromise {
    inner: Arc<CommandPromiseInner>,
}

/// The resolver half of a promise-resolver pair
/// 
/// Resolves the promise with a result. Consumed on resolve to prevent
/// double-resolution.
pub struct CommandPromiseResolver {
    inner: Arc<CommandPromiseInner>,
}

impl CommandPromise {
    /// Create a new promise-resolver pair
    pub fn new() -> (Self, CommandPromiseResolver) {
        let inner = Arc::new(CommandPromiseInner {
            result: Mutex::new(None),
            condvar: Condvar::new(),
        });
        (
            CommandPromise { inner: inner.clone() },
            CommandPromiseResolver { inner },
        )
    }

    /// Block indefinitely until result is available
    /// 
    /// Returns the CommandResult that resolved this promise.
    /// Panics if the mutex is poisoned.
    pub fn deref_blocking(&self) -> CommandResult {
        let mut guard = self.inner.result.lock().unwrap();
        while guard.is_none() {
            guard = self.inner.condvar.wait(guard).unwrap();
        }
        guard.take().unwrap()
    }

    /// Block until result is available or timeout expires
    /// 
    /// Returns `Some(result)` if the promise resolved within the timeout,
    /// `None` if the timeout expired before resolution.
    /// Panics if the mutex is poisoned.
    pub fn deref_timeout(&self, timeout: Duration) -> Option<CommandResult> {
        let mut guard = self.inner.result.lock().unwrap();
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(result) = guard.take() {
                return Some(result);
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return None;
            }
            let remaining = deadline - now;
            let (new_guard, timed_out) = self
                .inner
                .condvar
                .wait_timeout(guard, remaining)
                .unwrap();
            guard = new_guard;
            if timed_out.timed_out() {
                return None;
            }
        }
    }

    /// Non-blocking check: returns result if available, None if pending
    pub fn poll(&self) -> Option<CommandResult> {
        self.inner.result.lock().unwrap().clone()
    }

    /// Check if this promise has been resolved without blocking
    pub fn is_resolved(&self) -> bool {
        self.inner.result.lock().unwrap().is_some()
    }
}

impl Default for CommandPromise {
    fn default() -> Self {
        Self::new().0
    }
}

impl CommandPromiseResolver {
    /// Resolve this promise with a result
    /// 
    /// Wakes up any threads that are blocked waiting on this promise.
    /// This consumes the resolver to prevent double-resolution.
    pub fn resolve(self, result: CommandResult) {
        {
            let mut guard = self.inner.result.lock().unwrap();
            *guard = Some(result);
        }
        self.inner.condvar.notify_all();
    }
}

// Implement IHandle so CommandPromise can be stored in Value::Handle
// (AsAny is automatically implemented for all types that implement Any)
impl jinme::handle::IHandle for CommandPromise {}

impl std::fmt::Debug for CommandPromise {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let resolved = self.is_resolved();
        f.debug_struct("CommandPromise")
            .field("resolved", &resolved)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_resolve_and_deref() {
        let (promise, resolver) = CommandPromise::new();
        let result_val = jinme::prelude::Value::integer(42);
        let result = CommandResult::Success(std::sync::Arc::new(result_val));

        resolver.resolve(result.clone());

        let resolved = promise.deref_blocking();
        match resolved {
            CommandResult::Success(v) => {
                // Verify we got the same value
                assert_eq!(format!("{v}"), "42");
            }
            _ => panic!("Expected Success"),
        }
    }

    #[test]
    fn test_deref_blocks_until_resolve() {
        let (promise, resolver) = CommandPromise::new();
        let promise_clone = promise.clone();

        let handle = thread::spawn(move || {
            // This should block until the resolver resolves it
            let result = promise_clone.deref_blocking();
            match result {
                CommandResult::Success(_) => "ok",
                _ => "not ok",
            }
        });

        // Give the spawned thread time to start and block
        thread::sleep(Duration::from_millis(50));

        let result_val = jinme::prelude::Value::integer(123);
        let result = CommandResult::Success(std::sync::Arc::new(result_val));
        resolver.resolve(result);

        let outcome = handle.join().expect("thread panicked");
        assert_eq!(outcome, "ok");
    }

    #[test]
    fn test_deref_timeout_returns_none() {
        let (promise, _resolver) = CommandPromise::new();

        let result = promise.deref_timeout(Duration::from_millis(100));
        assert!(result.is_none());
    }

    #[test]
    fn test_deref_timeout_returns_some() {
        let (promise, resolver) = CommandPromise::new();
        let promise_clone = promise.clone();

        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            let result_val = jinme::prelude::Value::integer(999);
            let result = CommandResult::Success(std::sync::Arc::new(result_val));
            resolver.resolve(result);
        });

        let result = promise.deref_timeout(Duration::from_secs(5));
        assert!(result.is_some());

        handle.join().expect("thread panicked");
    }

    #[test]
    fn test_poll_non_blocking() {
        let (promise, resolver) = CommandPromise::new();

        assert!(promise.poll().is_none());
        assert!(!promise.is_resolved());

        let result_val = jinme::prelude::Value::integer(777);
        let result = CommandResult::Success(std::sync::Arc::new(result_val));
        resolver.resolve(result);

        assert!(promise.poll().is_some());
        assert!(promise.is_resolved());
    }

    #[test]
    fn test_multiple_threads_blocking_on_same_promise() {
        let (promise, resolver) = CommandPromise::new();

        let handles: Vec<_> = (0..3)
            .map(|_| {
                let p = promise.clone();
                thread::spawn(move || {
                    let result = p.deref_blocking();
                    match result {
                        CommandResult::Success(_) => "got it",
                        _ => "failed",
                    }
                })
            })
            .collect();

        thread::sleep(Duration::from_millis(50));

        let result_val = jinme::prelude::Value::integer(111);
        let result = CommandResult::Success(std::sync::Arc::new(result_val));
        resolver.resolve(result);

        for handle in handles {
            let outcome = handle.join().expect("thread panicked");
            assert_eq!(outcome, "got it");
        }
    }
}

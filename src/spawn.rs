//! Coroutine spawn helpers that carry the tracing context (may_tracing) when the
//! `tracing` feature is on, and degrade to a plain `may::go!` otherwise.

/// Spawn `f` on a new may coroutine that inherits the spawner's current span
/// (`may_tracing::current()`), so spans it creates nest under the request that
/// started it. Without the `tracing` feature this is `may::go!`.
///
/// Used for coroutines that do work *on behalf of the caller* (a query stream's
/// cursor coroutine). Process-scoped coroutines (logging, cache reflectors) keep
/// using `may::go!`, which starts with no context.
pub(crate) fn go_with_context<F, T>(f: F) -> may::coroutine::JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    #[cfg(feature = "tracing")]
    {
        // SAFETY: may's contract for `coroutine::spawn` — `f` must not block the OS
        // thread; the callers here are may-native (may_postgres I/O yields).
        unsafe { may_tracing::go_in_current_span(f) }
    }
    #[cfg(not(feature = "tracing"))]
    {
        may::go!(f)
    }
}

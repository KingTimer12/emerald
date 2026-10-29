//! Profiling with hotpath, only in builds made with `--features hotpath`
//! (time per measured function) or `--features hotpath-alloc` (bytes and
//! allocations too). Other builds compile none of it.
//!
//! The report is written when Emerald quits, to `HOTPATH_OUTPUT_PATH`
//! when set and to standard output otherwise.

#[cfg(feature = "hotpath")]
mod on {
    use std::sync::Mutex;

    use hotpath::{HotpathGuard, HotpathGuardBuilder};

    /// Counts what each measured function allocates; a plain pass-through
    /// without `hotpath-alloc`.
    #[global_allocator]
    static ALLOCATOR: hotpath::CountingAllocator = hotpath::CountingAllocator::new();

    /// Held until quitting: dropping it writes the report. gpui ends the
    /// process from inside its run loop, so `main` never returns to drop it.
    static GUARD: Mutex<Option<HotpathGuard>> = Mutex::new(None);

    pub fn start() {
        let guard = HotpathGuardBuilder::new("emerald").percentiles(&[50., 95., 99.]).build();
        *GUARD.lock().unwrap_or_else(|e| e.into_inner()) = Some(guard);
    }

    pub fn finish() {
        drop(GUARD.lock().unwrap_or_else(|e| e.into_inner()).take());
    }
}

#[cfg(feature = "hotpath")]
pub use on::{finish, start};

#[cfg(not(feature = "hotpath"))]
pub fn start() {}

#[cfg(not(feature = "hotpath"))]
pub fn finish() {}

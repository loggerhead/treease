#![allow(dead_code)]
/// Ultra-lightweight phase-timing utility for test and profiling scenarios.
///
/// ## When to use
///
/// - **Measuring per-operation latency** in integration tests (e.g. how long
///   does each `advance_job` call take under different chunk sizes).
/// - **Detecting unexpected rebuilds / repeated work** in streaming or
///   incremental-processing tests (use [`mark`] + [`count`] in assertions).
/// - **Printing a relative-timeline trace** during test debugging (call
///   [`mark`] at every interesting boundary, then [`report`] at the end).
///
/// ## Usage
///
/// ```ignore
/// // 1. (optional) reset the log before the section you care about
/// treease_core::test_timing::reset();
///
/// // 2. tick `mark` at the boundaries you want to measure
/// treease_core::test_timing::mark("my_op.start");
/// // … work …
/// treease_core::test_timing::mark("my_op.end");
///
/// // 3. query or report
/// let n = treease_core::test_timing::count("my_op.start");
/// assert!(n > 0, "my_op should have been entered");
/// treease_core::test_timing::report(); // prints a timeline to stderr
/// ```
///
/// All state is thread-local, so parallel tests do not interfere.
///
/// ## Warning
///
/// These functions are intentionally not called from production paths;
/// they are gated behind `#[cfg(test)]` and carry `#![expect(dead_code)]`
/// to avoid spurious warnings when a particular function (e.g. `report`)
use std::cell::RefCell;
use std::time::Instant;

thread_local! {
    static PHASES: RefCell<Vec<(&'static str, Instant)>> = const { RefCell::new(Vec::new()) };
}

pub fn mark(name: &'static str) {
    PHASES.with(|p| p.borrow_mut().push((name, Instant::now())));
}

pub fn report() {
    PHASES.with(|p| {
        let phases = p.borrow();
        if phases.is_empty() {
            return;
        }
        let base = phases[0].1;
        eprintln!("--- timing trace ---");
        for (name, t) in phases.iter() {
            let elapsed = t.duration_since(base).as_secs_f64() * 1000.0;
            eprintln!("  {:>10.3}ms  {}", elapsed, name);
        }
        eprintln!("--- end ---");
    });
}

pub fn reset() {
    PHASES.with(|p| p.borrow_mut().clear());
}

pub fn count(name: &'static str) -> usize {
    PHASES.with(|p| {
        p.borrow()
            .iter()
            .filter(|(phase_name, _)| *phase_name == name)
            .count()
    })
}

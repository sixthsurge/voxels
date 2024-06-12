/// Measure the time taken by a computation, and log it as trace
macro_rules! measure_time {
    ($expression:expr) => {{
        let now = std::time::Instant::now();
        let result = $expression;
        log::trace!(
            "{:40} {}s",
            stringify!($expression),
            now.elapsed().as_secs_f64(),
        );
        result
    }};
}

pub(crate) use measure_time;

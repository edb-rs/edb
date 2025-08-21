mod analyzer;
pub use analyzer::*;

mod common;
pub use common::*;

mod hook;
pub use hook::*;

mod step;
pub use step::*;

mod variable;
pub use variable::*;

mod annotation;
pub use annotation::*;

mod visitor;
pub use visitor::*;

mod log {
    pub(crate) const LOG_TARGET: &str = "analysis";

    macro_rules! debug {
        ($($arg:tt)*) => {
            tracing::debug!(target: LOG_TARGET, $($arg)*)
        };
    }

    macro_rules! trace {
        ($($arg:tt)*) => {
            tracing::trace!(target: LOG_TARGET, $($arg)*)
        };
    }

    pub(crate) use debug;
    pub(crate) use trace;
}

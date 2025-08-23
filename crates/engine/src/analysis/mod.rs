// EDB - Ethereum Debugger
// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.


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

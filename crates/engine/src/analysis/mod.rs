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

mod contract;
pub use contract::*;

mod function;
pub use function::*;

mod hook;
pub use hook::*;

mod step;
pub use step::*;

mod types;
pub use types::*;

mod variable;
pub use variable::*;

mod annotation;
pub use annotation::*;

mod visitor;
pub use visitor::*;

mod macros {
    macro_rules! universal_id {
        (
            $(#[$attr:meta])*
            $name:ident => $initial_value:expr
        ) => {
            $(#[$attr])*
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, PartialOrd, Ord, Serialize, Deserialize)]
            pub struct $name(u64);

            paste::paste! {
                lazy_static::lazy_static! {
                    /// The global counter for the $name.
                    #[doc = "The global counter for the " $name " object."]
                    pub static ref [<NEXT_ $name>]: std::sync::Mutex<$name> = std::sync::Mutex::new($name($initial_value));
                }
            }

            paste::paste! {
                impl $name {
                    /// Get the next value and increment the global counter.
                    pub fn next() -> Self {
                        let mut counter = [<NEXT_ $name>].lock().unwrap();
                        let value = *counter;
                        counter.0 += 1;
                        value
                    }
                }
            }

            impl From<$name> for u64 {
                fn from(value: $name) -> Self {
                    value.0
                }
            }

            impl From<$name> for alloy_primitives::U256 {
                fn from(value: $name) -> Self {
                    Self::from(value.0)
                }
            }

            impl From<u64> for $name {
                fn from(value: u64) -> Self {
                    Self(value)
                }
            }

            impl TryFrom<alloy_primitives::U256> for $name {
                type Error = alloy_primitives::ruint::FromUintError<u64>;
                fn try_from(value: alloy_primitives::U256) -> Result<Self, alloy_primitives::ruint::FromUintError<u64>> {
                    value.try_into().map(Self)
                }
            }

            impl std::fmt::Display for $name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}", self.0)
                }
            }
        };
    }

    pub(crate) use universal_id;
}

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
mod ast;
mod body;
mod common;
mod contract;
mod function;
mod scope;
mod step;
mod types;
mod variable;

pub use analyzer::*;
pub use ast::*;
pub use body::*;
pub use common::*;
pub use contract::*;
pub use function::*;
pub use scope::*;
pub use step::*;
pub use types::*;
pub use variable::*;

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

    macro_rules! define_ref {
        (
            $(#[$attr:meta])*
            $ref_name:ident($inner_type:ty) {
                $(clone_field: {
                    $($(&)? $clone_field:ident : $clone_type:ty),* $(,)?
                })?
                $(cached_field: {
                    $($cached_field:ident : $cached_field_type:ty),* $(,)?
                })?
                $(cached_method: {
                    $($cached_method_field:ident : $cached_method_type:ty),* $(,)?
                })?
                $(delegate: {
                    $($(#[$delegate_attr:meta])* fn $delegate_method:ident(&self) -> $delegate_return:ty);* $(;)?
                })?
                $(additional_cache: {
                    $($additional_cache_field:ident : $additional_cache_type:ty),* $(,)?
                })?
            }
        ) => {
            $(#[$attr])*
            #[derive(Clone, derive_more::Debug)]
            pub struct $ref_name {
                inner: std::sync::Arc<parking_lot::RwLock<$inner_type>>,
                $($(
                    #[debug(ignore)]
                    $additional_cache_field: once_cell::sync::OnceCell<$additional_cache_type>,
                )*)?
                $($(
                    #[debug(ignore)]
                    $cached_field: once_cell::sync::OnceCell<$cached_field_type>,
                )*)?
                $($(
                    #[debug(ignore)]
                    $cached_method_field: once_cell::sync::OnceCell<$cached_method_type>,
                )*)?
            }

            impl From<$inner_type> for $ref_name {
                fn from(value: $inner_type) -> Self {
                    Self::new(value)
                }
            }

            impl $ref_name {
                /// Creates a new reference from the inner type.
                pub fn new(inner: $inner_type) -> Self {
                    Self {
                        inner: std::sync::Arc::new(parking_lot::RwLock::new(inner)),
                        $($(
                            $additional_cache_field: once_cell::sync::OnceCell::new(),
                        )*)?
                        $($(
                            $cached_field: once_cell::sync::OnceCell::new(),
                        )*)?
                        $($(
                            $cached_method_field: once_cell::sync::OnceCell::new(),
                        )*)?
                    }
                }

                /// Returns a read lock guard for the inner type.
                #[allow(dead_code)]
                pub(in crate::analysis) fn read(&self) -> parking_lot::RwLockReadGuard<'_, $inner_type> {
                    self.inner.read()
                }

                /// Returns a write lock guard for the inner type.
                #[allow(dead_code)]
                pub(in crate::analysis) fn write(&self) -> parking_lot::RwLockWriteGuard<'_, $inner_type> {
                    self.inner.write()
                }

                /// Clears all cached values.
                #[allow(dead_code)]
                pub fn clear_cache(&mut self) {
                    $($(
                        self.$additional_cache_field.take();
                    )*)?
                    $($(
                        self.$cached_field.take();
                    )*)?
                    $($(
                        self.$cached_method_field.take();
                    )*)?
                }

                $($(
                    /// Returns the value by reading from the inner type.
                    pub fn $clone_field(&self) -> $clone_type {
                        self.read().$clone_field.clone()
                    }
                )*)?

                $($(
                    /// Returns a cached reference to the value.
                    pub fn $cached_field(&self) -> &$cached_field_type {
                        self.$cached_field.get_or_init(|| self.read().$cached_field.clone())
                    }
                )*)?

                $($(
                    /// Returns a cached reference to the value.
                    pub fn $cached_method_field(&self) -> &$cached_method_type {
                        self.$cached_method_field.get_or_init(|| self.read().$cached_method_field())
                    }
                )*)?

                $($(
                    #[allow(missing_docs)]
                    $(#[$delegate_attr])*
                    pub fn $delegate_method(&self) -> $delegate_return {
                        self.read().$delegate_method()
                    }
                )*)?
            }

            impl serde::Serialize for $ref_name {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: serde::Serializer,
                {
                    // Serialize the inner type directly (cache fields are transient)
                    self.inner.read().serialize(serializer)
                }
            }

            impl<'de> serde::Deserialize<'de> for $ref_name {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: serde::Deserializer<'de>,
                {
                    // Deserialize the inner type and wrap it (caches will be empty)
                    let value = <$inner_type>::deserialize(deserializer)?;
                    Ok(Self::new(value))
                }
            }
        };
    }

    pub(crate) use define_ref;
}

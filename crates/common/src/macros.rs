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

//! Conditional assertion macros for EDB testing
//!
//! These macros provide assertions that only execute when the `EDB_TEST_STRICT` environment
//! variable is set. This allows for optional strict validation during testing without
//! affecting production or normal test runs.

/// Assert a condition only when `EDB_TEST_STRICT` environment variable is set.
///
/// This macro behaves like the standard `assert!` macro, but only executes when
/// the `EDB_TEST_STRICT` environment variable is set at compile time.
///
/// # Examples
///
/// ```ignore
/// use edb_common::edb_assert;
///
/// let value = 42;
/// edb_assert!(value == 42);
/// edb_assert!(value == 42, "value should be 42, got {}", value);
/// ```
#[macro_export]
macro_rules! edb_assert {
    ($($arg:tt)*) => {
        if option_env!("EDB_TEST_STRICT").is_some() {
            assert!($($arg)*);
        }
    };
}

/// Assert two expressions are equal only when `EDB_TEST_STRICT` environment variable is set.
///
/// This macro behaves like the standard `assert_eq!` macro, but only executes when
/// the `EDB_TEST_STRICT` environment variable is set at compile time.
///
/// # Examples
///
/// ```ignore
/// use edb_common::edb_assert_eq;
///
/// let a = 1 + 1;
/// edb_assert_eq!(a, 2);
/// edb_assert_eq!(a, 2, "expected {} to equal 2", a);
/// ```
#[macro_export]
macro_rules! edb_assert_eq {
    ($($arg:tt)*) => {
        if option_env!("EDB_TEST_STRICT").is_some() {
            assert_eq!($($arg)*);
        }
    };
}

/// Assert two expressions are not equal only when `EDB_TEST_STRICT` environment variable is set.
///
/// This macro behaves like the standard `assert_ne!` macro, but only executes when
/// the `EDB_TEST_STRICT` environment variable is set at compile time.
///
/// # Examples
///
/// ```ignore
/// use edb_common::edb_assert_ne;
///
/// let a = 1 + 1;
/// edb_assert_ne!(a, 3);
/// edb_assert_ne!(a, 3, "expected {} to not equal 3", a);
/// ```
#[macro_export]
macro_rules! edb_assert_ne {
    ($($arg:tt)*) => {
        if option_env!("EDB_TEST_STRICT").is_some() {
            assert_ne!($($arg)*);
        }
    };
}

/// Debug assert that only executes when `EDB_TEST_STRICT` is set and in debug builds.
///
/// This macro combines the behavior of `debug_assert!` with the conditional execution
/// based on the `EDB_TEST_STRICT` environment variable.
///
/// # Examples
///
/// ```ignore
/// use edb_common::edb_debug_assert;
///
/// let value = 42;
/// edb_debug_assert!(value == 42);
/// ```
#[macro_export]
macro_rules! edb_debug_assert {
    ($($arg:tt)*) => {
        if option_env!("EDB_TEST_STRICT").is_some() {
            debug_assert!($($arg)*);
        }
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_edb_assert_macros() {
        // These should not panic when EDB_TEST_STRICT is not set
        edb_assert!(1 == 1);
        edb_assert_eq!(1, 1);
        edb_assert_ne!(1, 2);
        edb_debug_assert!(1 == 1);
    }

    #[test]
    fn test_edb_assert_with_message() {
        edb_assert!(true, "this should not panic");
        edb_assert_eq!(1, 1, "these should be equal");
        edb_assert_ne!(1, 2, "these should not be equal");
    }
}

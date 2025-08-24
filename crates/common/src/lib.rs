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

// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
// SPDX-License-Identifier: AGPL-3.0
//! EDB Utils - Shared functionality for EDB components
//!
//! This crate provides shared utilities used by both the edb binary
//! and the engine crate, including chain forking and transaction replay.

#![allow(unused_imports)]

pub mod cache;
pub mod context;
pub mod forking;
pub mod logging;
pub mod opcode;
pub mod spec_id;
pub mod types;

pub use cache::*;
pub use context::*;
pub use forking::*;
pub use logging::*;
pub use opcode::*;
pub use spec_id::*;

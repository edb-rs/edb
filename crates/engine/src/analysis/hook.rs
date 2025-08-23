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


use serde::{Deserialize, Serialize};

use crate::{USID, UVID};

/// Contains information that should be recoreded before and after each step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepHook {
    /// The hook to mark that a step is about to be executed. The debugger will pause here during step-by-step execution.
    BeforeStep(USID),

    /// The hook to mark which variables becomes in scope.
    VariableInScope(UVID),

    /// The hook to mark that a variable becomes out of scope.
    VariableOutOfScope(UVID),

    /// The hook to mark that a variable is updated.
    VariableUpdate(UVID),
}

impl StepHook {
    /// Returns the variant name of this hook.
    ///
    /// # Returns
    ///
    /// A string slice representing the variant name.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::BeforeStep(_) => "BeforeStep",
            Self::VariableInScope(_) => "VariableInScope",
            Self::VariableOutOfScope(_) => "VariableOutOfScope",
            Self::VariableUpdate(_) => "VariableUpdate",
        }
    }
}

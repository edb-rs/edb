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

use std::sync::Arc;

use alloy_dyn_abi::DynSolValue;
use eyre::Result;
use revm::{database::CacheDB, Database, DatabaseCommit, DatabaseRef};

use crate::{EngineContext, ExpressionEvaluator};

/// Evaluate a Solidity expression string within the context of a specific snapshot.
pub fn eval_on_snapshot<DB>(
    context: Arc<EngineContext<DB>>,
    expr: &str,
    snapshot_id: usize,
) -> Result<DynSolValue>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    let evaluator = ExpressionEvaluator::new_edb(context.clone());
    evaluator.eval(expr, snapshot_id)
}

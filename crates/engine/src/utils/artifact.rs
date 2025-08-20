use alloy_primitives::Bytes;
use foundry_block_explorers::contract::Metadata;
use foundry_compilers::artifacts::{CompilerOutput, Contract, SolcInput};
use serde::{Deserialize, Serialize};
use tracing::error;

/// Artifact for a compiled contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// Metadata about the contract.
    pub meta: Metadata,
    /// Input for the Solidity compiler.
    pub input: SolcInput,
    /// Output from the Solidity compiler.
    pub output: CompilerOutput,
}

impl Artifact {
    /// Returns the contract name.
    pub fn contract_name(&self) -> &str {
        self.meta.contract_name.as_str()
    }

    /// Returns the compiler version.
    pub fn compiler_version(&self) -> &str {
        self.meta.compiler_version.as_str()
    }

    /// Returns the constructor arguments.
    pub fn constructor_arguments(&self) -> &Bytes {
        &self.meta.constructor_arguments
    }

    /// Subject contract
    pub fn contract(&self) -> Option<&Contract> {
        let contract_name = self.contract_name();

        self.output
            .contracts
            .values()
            .into_iter()
            .find(|c| c.contains_key(contract_name))
            .and_then(|contracts| contracts.get(contract_name))
    }

    /// Find creation hooks (one-to-one mapping)
    pub fn find_creation_hooks<'a>(
        &'a self,
        recompiled: &'a Self,
    ) -> Vec<(&'a Contract, &'a Contract, &'a Bytes)> {
        let mut hooks = Vec::new();

        for (path, contracts) in &self.output.contracts {
            for (name, contract) in contracts {
                if let Some(recompiled_contract) =
                    recompiled.output.contracts.get(path).and_then(|c| c.get(name))
                {
                    hooks.push((contract, recompiled_contract, self.constructor_arguments()));
                } else {
                    error!("No recompiled contract found for {} in {}", name, path.display());
                }
            }
        }

        hooks
    }
}

//! Contract deployment inspector for replacing creation bytecode
//!
//! This inspector intercepts contract creation calls and can replace the init code
//! with custom bytecode when the deployment would create a specific target address.

use alloy_primitives::{Address, Bytes};
use revm::{
    context::{ContextTr, CreateScheme, JournalTr},
    interpreter::{CreateInputs, CreateOutcome},
    Inspector,
};
use tracing::{debug, info};

/// Inspector that intercepts and modifies contract deployments
#[derive(Debug)]
pub struct TweakInspector {
    /// Target address we're looking for
    target_address: Address,

    /// Custom init code to use (contract creation bytecode)
    init_code: Bytes,

    /// Constructor arguments to append to init code
    constructor_args: Bytes,

    /// The deployed bytecode we captured (filled after successful deployment)
    deployed_code: Option<Bytes>,

    /// Whether we found and replaced the target deployment
    found_target: bool,
}

impl TweakInspector {
    /// Create a new deployment inspector
    pub fn new(target_address: Address, init_code: Bytes, constructor_args: Bytes) -> Self {
        Self {
            target_address,
            init_code,
            constructor_args,
            deployed_code: None,
            found_target: false,
        }
    }

    /// Get the deployed bytecode if the target was found and deployed
    pub fn deployed_code(&self) -> Option<&Bytes> {
        self.deployed_code.as_ref()
    }

    /// Check if the target deployment was found
    pub fn found_target(&self) -> bool {
        self.found_target
    }

    /// Combine init code with constructor arguments
    fn get_full_init_code(&self) -> Bytes {
        let mut full_code = self.init_code.to_vec();
        full_code.extend_from_slice(&self.constructor_args);
        Bytes::from(full_code)
    }
}

impl<CTX: ContextTr + JournalTr> Inspector<CTX> for TweakInspector {
    fn create(&mut self, context: &mut CTX, inputs: &mut CreateInputs) -> Option<CreateOutcome> {
        // Get the nonce from the caller account
        let account = context.load_account(inputs.caller).ok()?;
        let nonce = account.info.nonce;

        // Calculate what address would be created using the built-in method
        let predicted_address = inputs.created_address(nonce);

        debug!(
            "CREATE intercepted: deployer={:?}, predicted={:?}, target={:?}",
            inputs.caller, predicted_address, self.target_address
        );

        // Check if this is our target deployment
        if predicted_address == self.target_address {
            info!(
                "Found target deployment! Replacing init code for address {:?}",
                self.target_address
            );

            self.found_target = true;

            // Replace the init code with our custom code + constructor args
            inputs.init_code = self.get_full_init_code();

            // Force the address to be our target (in case of any calculation differences)
            // Convert to Custom scheme to ensure the exact address
            inputs.scheme = CreateScheme::Custom { address: self.target_address };

            debug!(
                "Replaced init code: {} bytes (code: {}, args: {})",
                inputs.init_code.len(),
                self.init_code.len(),
                self.constructor_args.len()
            );
        }

        // Continue with normal execution
        None
    }

    fn create_end(
        &mut self,
        context: &mut CTX,
        inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        // Check if this was our target deployment and it succeeded
        if self.found_target
            && matches!(inputs.scheme, CreateScheme::Custom { address } if address == self.target_address)
        {
            if outcome.result.is_ok() {
                // Get the deployed bytecode from the context
                if let Some(created_address) = outcome.address {
                    if created_address == self.target_address {
                        // Get deployed code from outcome's output (runtime bytecode)
                        self.deployed_code = Some(outcome.result.output.clone());
                        info!(
                            "Successfully captured deployed bytecode for {:?}: {} bytes",
                            self.target_address,
                            outcome.result.output.len()
                        );
                    }
                }
            } else {
                info!(
                    "Target deployment failed for {:?}: {:?}",
                    self.target_address, outcome.result
                );
            }
        }
    }
}

/// Builder pattern for constructing DeployInspector
pub struct TweakInspectorBuilder {
    target_address: Option<Address>,
    init_code: Option<Bytes>,
    constructor_args: Option<Bytes>,
}

impl TweakInspectorBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self { target_address: None, init_code: None, constructor_args: None }
    }

    /// Set the target address to intercept
    pub fn target_address(mut self, address: Address) -> Self {
        self.target_address = Some(address);
        self
    }

    /// Set the custom init code (contract creation bytecode)
    pub fn init_code(mut self, code: Bytes) -> Self {
        self.init_code = Some(code);
        self
    }

    /// Set the constructor arguments
    pub fn constructor_args(mut self, args: Bytes) -> Self {
        self.constructor_args = Some(args);
        self
    }

    /// Build the DeployInspector
    pub fn build(self) -> Result<TweakInspector, &'static str> {
        let target_address = self.target_address.ok_or("target_address is required")?;
        let init_code = self.init_code.ok_or("init_code is required")?;
        let constructor_args = self.constructor_args.unwrap_or_default();

        Ok(TweakInspector::new(target_address, init_code, constructor_args))
    }
}

impl Default for TweakInspectorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_pattern() {
        let target = Address::from([0x66; 20]);
        let init_code = Bytes::from(vec![0x60, 0x80, 0x60, 0x40]);
        let args = Bytes::from(vec![0x00, 0x01, 0x02]);

        let inspector = TweakInspectorBuilder::new()
            .target_address(target)
            .init_code(init_code.clone())
            .constructor_args(args.clone())
            .build()
            .unwrap();

        assert_eq!(inspector.target_address, target);
        assert_eq!(inspector.init_code, init_code);
        assert_eq!(inspector.constructor_args, args);
        assert!(!inspector.found_target());
        assert!(inspector.deployed_code().is_none());
    }
}

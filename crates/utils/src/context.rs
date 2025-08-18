use revm::{
    context::{BlockEnv, CfgEnv, TxEnv},
    Context,
};

/// Type alias for the EDB context
pub type EDBContext<DB> = Context<BlockEnv, TxEnv, CfgEnv, DB>;

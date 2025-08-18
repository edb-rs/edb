use revm::{
    context::{BlockEnv, CfgEnv, TxEnv},
    Context,
};

pub type EDBContext<DB> = Context<BlockEnv, TxEnv, CfgEnv, DB>;

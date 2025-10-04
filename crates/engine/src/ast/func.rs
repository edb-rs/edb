use serde::{Deserialize, Serialize};

use super::Blk;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Func {
    // The function body. A function may not have body if it is virtual and not implemented.
    pub body: Option<Blk>,
}

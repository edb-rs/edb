use std::sync::Arc;

/// A more easy-to-use unit location, which includes the corresponding source code.
#[derive(Clone, Debug)]
pub struct UnitLocation {
    pub start: usize,
    pub length: usize,
    pub index: usize,
    pub code: Arc<String>,
}

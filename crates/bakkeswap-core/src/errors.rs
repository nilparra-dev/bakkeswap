use thiserror::Error;

#[derive(Debug, Error)]
pub enum BakkesSwapError {
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
    #[error("unsafe operation blocked: {0}")]
    UnsafeOperation(&'static str),
}

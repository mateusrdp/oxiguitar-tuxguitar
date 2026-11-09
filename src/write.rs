//! placeholder
use crate::log::Log;
use oxiguitar_model::Score;
pub(crate) fn write(_score: &Score) -> Result<(Vec<u8>, Log), String> {
    Err("not yet".into())
}

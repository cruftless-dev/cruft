
use crate::Value;

pub mod datetime;
pub mod json;
pub mod scalar;

pub fn dispatch(name: &str, args: &[Value]) -> Option<Result<Value, String>> {
    scalar::call(name, args)
        .or_else(|| datetime::call(name, args))
        .or_else(|| json::call(name, args))
}

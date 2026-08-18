
use rusty_js_runtime::{Runtime, RuntimeError};

#[derive(Default)]
pub struct PollPass {

    pub had_ws_sessions: bool,
}

pub trait HostSurface {
    fn name(&self) -> &'static str;
    fn poll(&mut self, rt: &mut Runtime, pass: &mut PollPass) -> Result<bool, RuntimeError>;
}

struct NamedSurface<F> {
    name: &'static str,
    f: F,
}

impl<F> HostSurface for NamedSurface<F>
where
    F: FnMut(&mut Runtime, &mut PollPass) -> Result<bool, RuntimeError>,
{
    fn name(&self) -> &'static str {
        self.name
    }
    fn poll(&mut self, rt: &mut Runtime, pass: &mut PollPass) -> Result<bool, RuntimeError> {
        (self.f)(rt, pass)
    }
}

pub fn named<F>(name: &'static str, f: F) -> Box<dyn HostSurface>
where
    F: FnMut(&mut Runtime, &mut PollPass) -> Result<bool, RuntimeError> + 'static,
{
    Box::new(NamedSurface { name, f })
}

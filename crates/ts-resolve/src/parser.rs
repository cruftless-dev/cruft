
use crate::ts_ast::TypeWitness;
use rusty_js_ast::Module;

#[derive(Debug)]
pub struct TsParseError {
    pub message: String,
}

impl From<rusty_js_parser::ParseError> for TsParseError {
    fn from(e: rusty_js_parser::ParseError) -> Self {
        TsParseError {
            message: format!("{:?}", e),
        }
    }
}

impl std::fmt::Display for TsParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TsParseError {}

pub struct TsParser<'src> {
    src: &'src str,
}

impl<'src> TsParser<'src> {
    pub fn new(src: &'src str) -> Result<Self, TsParseError> {
        Ok(TsParser { src })
    }

    pub fn parse_module(&mut self) -> Result<(Module, Vec<TypeWitness>), TsParseError> {
        let (stripped, witnesses) = crate::strip::strip_ts(self.src).map_err(|e| TsParseError {
            message: format!("strip: {}", e),
        })?;
        let module = rusty_js_parser::parse_module(&stripped)?;
        Ok((module, witnesses))
    }
}

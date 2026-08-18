
pub mod erase;
pub mod lexer;
pub mod parser;
pub mod strip;
pub mod transform;
pub mod ts_ast;

pub use parser::{TsParseError, TsParser};
pub use ts_ast::{TsAnnotation, TsLiteralVal, TsTypeRef};

pub fn parse_and_erase(src: &str) -> Result<rusty_js_ast::Module, TsParseError> {
    let mut p = TsParser::new(src)?;
    let (module, _witnesses) = p.parse_module()?;
    Ok(erase::erase_module(module))
}

pub fn parse_with_witnesses(
    src: &str,
) -> Result<(rusty_js_ast::Module, Vec<ts_ast::TypeWitness>), TsParseError> {
    let mut p = TsParser::new(src)?;
    let (module, witnesses) = p.parse_module()?;
    Ok((erase::erase_module(module), witnesses))
}

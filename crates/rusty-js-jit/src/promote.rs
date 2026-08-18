
use rusty_js_bytecode::compiler::FunctionProto;
use rusty_js_bytecode::op::{op_from_byte, Op};

pub fn promote_to_typed_i64(proto: &FunctionProto) -> Option<FunctionProto> {
    let new_bytecode = rewrite_bytecode(&proto.bytecode)?;
    let mut out = proto.clone();
    out.bytecode = new_bytecode;
    Some(out)
}

fn rewrite_bytecode(bc: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(bc.len());
    let mut pc = 0;
    while pc < bc.len() {
        let opcode = op_from_byte(bc[pc])?;
        let operand_bytes = opcode.operand_size();

        let new_opcode = match opcode {

            Op::AddI64
            | Op::SubI64
            | Op::MulI64
            | Op::IncI64
            | Op::DecI64
            | Op::LtI64
            | Op::LeI64
            | Op::GtI64
            | Op::GeI64
            | Op::EqI64
            | Op::NeI64 => opcode,

            Op::Add => Op::AddI64,
            Op::Sub => Op::SubI64,
            Op::Mul => Op::MulI64,
            Op::Inc => Op::IncI64,
            Op::Dec => Op::DecI64,
            Op::Lt => Op::LtI64,
            Op::Le => Op::LeI64,
            Op::Gt => Op::GtI64,
            Op::Ge => Op::GeI64,

            Op::Eq | Op::StrictEq => Op::EqI64,
            Op::Ne | Op::StrictNe => Op::NeI64,

            Op::LoadArg
            | Op::LoadLocal
            | Op::StoreLocal
            | Op::InitLocal
            | Op::PushI32
            | Op::PushUndef
            | Op::ToNumeric
            | Op::Pop
            | Op::Dup
            | Op::Jump
            | Op::JumpIfTrue
            | Op::JumpIfFalse
            | Op::Return
            | Op::ReturnUndef

            | Op::BitAnd
            | Op::BitOr
            | Op::BitXor
            | Op::Nop => opcode,

            _ => return None,
        };

        out.push(new_opcode as u8);
        for i in 0..operand_bytes {
            out.push(bc[pc + 1 + i]);
        }
        pc += 1 + operand_bytes;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_js_bytecode::compile_module;
    use rusty_js_bytecode::Constant;

    #[test]
    #[ignore = "Φ-EXT 3: i64-specific behavior; revisit at Move 2 typed-i64 fast path"]
    fn promotes_sum_to_typed_i64() {
        let src =
            r#"function sum(n) { var s = 0; for (var i = 0; i < n; i++) s = s + i; return s; }"#;
        let m = compile_module(src).expect("compile module");
        let sum_proto = m
            .constants
            .entries()
            .iter()
            .find_map(|c| match c {
                Constant::Function(p) if p.display_name == "sum" => Some((**p).clone()),
                _ => None,
            })
            .expect("find sum proto");
        let promoted = promote_to_typed_i64(&sum_proto).expect("promote sum");

        let mut plain_count = 0;
        let mut typed_count = 0;
        let mut pc = 0;
        while pc < promoted.bytecode.len() {
            let op = op_from_byte(promoted.bytecode[pc]).unwrap();
            match op {
                Op::Add
                | Op::Sub
                | Op::Mul
                | Op::Inc
                | Op::Dec
                | Op::Lt
                | Op::Le
                | Op::Gt
                | Op::Ge => plain_count += 1,
                Op::AddI64
                | Op::SubI64
                | Op::MulI64
                | Op::IncI64
                | Op::DecI64
                | Op::LtI64
                | Op::LeI64
                | Op::GtI64
                | Op::GeI64 => typed_count += 1,
                _ => {}
            }
            pc += 1 + op.operand_size();
        }
        assert_eq!(
            plain_count, 0,
            "no plain arithmetic ops should remain after promotion"
        );
        assert!(
            typed_count >= 3,
            "expected at least Add+Lt+Inc to be promoted; got {}",
            typed_count
        );
    }

    #[test]
    fn refuses_function_with_unsupported_ops() {

        let src = r#"function getx(o) { return o.x; }"#;
        let m = compile_module(src).expect("compile module");
        let proto = m
            .constants
            .entries()
            .iter()
            .find_map(|c| match c {
                Constant::Function(p) if p.display_name == "getx" => Some((**p).clone()),
                _ => None,
            })
            .expect("find getx proto");
        let result = promote_to_typed_i64(&proto);
        assert!(
            result.is_none(),
            "function with GetProp should not be promotable"
        );
    }

    #[test]
    fn promotes_tonumeric_loop_update_with_bitwise_control() {
        let src = r#"
            function run() {
              var N = 1024;
              var mask = N - 1;
              var checksum = 0;
              var M_OUTER = 4;
              for (var rep = 0; rep < M_OUTER; rep++) {
                for (var j = 0; j < N; j++) {
                  checksum = (checksum
                    + ((j + rep) & mask)
                    + ((j * 2) & mask)
                    + ((j ^ 7) & mask)) & 0x3fffffff;
                }
              }
              return checksum;
            }
        "#;
        let m = compile_module(src).expect("compile module");
        let proto = m
            .constants
            .entries()
            .iter()
            .find_map(|c| match c {
                Constant::Function(p) if p.display_name == "run" => Some((**p).clone()),
                _ => None,
            })
            .expect("find run proto");
        let promoted = promote_to_typed_i64(&proto)
            .expect("ToNumeric loop update should not block typed-i64 promotion");

        let mut has_tonumeric = false;
        let mut has_typed_arith = false;
        let mut pc = 0;
        while pc < promoted.bytecode.len() {
            let op = op_from_byte(promoted.bytecode[pc]).unwrap();
            match op {
                Op::ToNumeric => has_tonumeric = true,
                Op::AddI64 | Op::SubI64 | Op::MulI64 | Op::LtI64 | Op::IncI64 => {
                    has_typed_arith = true;
                }
                Op::Add | Op::Sub | Op::Mul | Op::Lt | Op::Inc => {
                    panic!("plain arithmetic op {op:?} should be promoted")
                }
                _ => {}
            }
            pc += 1 + op.operand_size();
        }
        assert!(
            has_tonumeric,
            "test shape should retain compiler-emitted ToNumeric"
        );
        assert!(
            has_typed_arith,
            "computed-index control shape should carry typed arithmetic"
        );
    }

    #[ignore = "Φ-EXT 3: i64-specific behavior; revisit at Move 2 typed-i64 fast path"]
    #[test]
    fn promoted_sum_jit_compiles_and_runs() {
        let src =
            r#"function sum(n) { var s = 0; for (var i = 0; i < n; i++) s = s + i; return s; }"#;
        let m = compile_module(src).expect("compile module");
        let sum_proto = m
            .constants
            .entries()
            .iter()
            .find_map(|c| match c {
                Constant::Function(p) if p.display_name == "sum" => Some((**p).clone()),
                _ => None,
            })
            .expect("find sum proto");
        let promoted = promote_to_typed_i64(&sum_proto).expect("promote sum");
        let jit = crate::compile_function(&promoted).expect("JIT compile promoted sum");
        assert_eq!(jit.func.call1(0 as f64), 0 as f64);
        assert_eq!(jit.func.call1(5 as f64), 10 as f64);
        assert_eq!(jit.func.call1(100 as f64), 4950 as f64);
        assert_eq!(jit.func.call1(1_000_000 as f64), 499_999_500_000_i64 as f64);
    }
}

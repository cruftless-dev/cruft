
use crate::compiler::CompiledModule;
use crate::op::*;

pub fn disassemble(m: &CompiledModule) -> String {
    let mut out = String::new();
    let mut off = 0;
    while off < m.bytecode.len() {
        let op_byte = m.bytecode[off];
        let op = match op_from_byte(op_byte) {
            Some(op) => op,
            None => {
                out.push_str(&format!("{:5}  <invalid 0x{:02X}>\n", off, op_byte));
                off += 1;
                continue;
            }
        };
        let opname = format!("{:?}", op);
        if op == Op::CallMethodThenInlineArrow {
            if let Some((payload, next)) =
                decode_call_method_then_inline_arrow_payload(&m.bytecode, off + 1)
            {
                let captures = payload
                    .captures
                    .iter()
                    .map(|capture| {
                        let source = match capture.source {
                            LazyArrowCaptureSource::Local => "local",
                            LazyArrowCaptureSource::Upvalue => "upvalue",
                        };
                        format!("{}:{}", source, capture.slot)
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                out.push_str(&format!(
                    "{:5}  {} proto={} captures=[{}]\n",
                    off, opname, payload.proto_idx, captures
                ));
                off = next;
                continue;
            }
            out.push_str(&format!("{:5}  {} <truncated>\n", off, opname));
            break;
        }
        let osize = op.operand_size();
        let operand_str = match osize {
            0 => String::new(),
            1 => format!(" {}", m.bytecode[off + 1]),
            2 => format!(" {}", decode_u16(&m.bytecode, off + 1)),
            4 => match op {
                Op::PushI32 => format!(" {}", decode_i32(&m.bytecode, off + 1)),
                _ => format!(" {}", decode_i32(&m.bytecode, off + 1)),
            },
            _ => String::new(),
        };

        let const_resolved = match op {
            Op::PushConst
            | Op::LoadWithName
            | Op::StoreWithName
            | Op::ResolveWithName
            | Op::LoadWithNameRef
            | Op::StoreWithNameRef
            | Op::LoadGlobal
            | Op::StoreGlobal
            | Op::GetProp
            | Op::SetProp
            | Op::SetPropStrict
            | Op::InitProp
            | Op::InitPropStaticSlot
            | Op::LoadLocal
            | Op::StoreLocal
            | Op::LoadArg
            | Op::StoreArg
            | Op::EnterPrivateHomeLocal
            | Op::LoadUpvalue
            | Op::StoreUpvalue
            | Op::DefineLocal => {
                let idx = decode_u16(&m.bytecode, off + 1) as usize;
                m.constants
                    .entries()
                    .get(idx)
                    .map(|c| format!("  ; {}", render_constant(c)))
            }
            _ => None,
        };
        out.push_str(&format!(
            "{:5}  {}{}{}\n",
            off,
            opname,
            operand_str,
            const_resolved.unwrap_or_default()
        ));
        off += instruction_len_at(&m.bytecode, off).unwrap_or(1 + osize);
    }
    out
}

fn render_constant(c: &crate::constants::Constant) -> String {
    use crate::constants::Constant::*;
    match c {
        Number(v) => format!("{}", v),
        BigInt(s) => format!("{}n", s),
        String(s) => format!("{:?}", s),
        WtfString(u) => format!("{:?}", std::string::String::from_utf16_lossy(u)),
        Regex { body, flags } => format!("/{}/{}", body, flags),
        Function(_) => "<function>".to_string(),
        LazyFunction(_) => "<lazy-function>".to_string(),
    }
}

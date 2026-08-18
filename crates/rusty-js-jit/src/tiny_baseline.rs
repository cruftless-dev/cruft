
#[derive(Debug, Clone)]
pub struct TinyBaselineMetadata {

    pub jit_fn_ptr: usize,

    pub params: u16,

    pub bytecode_len: usize,

    pub tb_eligible: bool,
}

pub const TB_BYTECODE_LEN_THRESHOLD: usize = 60;

impl TinyBaselineMetadata {

    pub fn build(jit_fn_ptr: usize, params: u16, bytecode_len: usize) -> Self {
        let tb_eligible = bytecode_len <= TB_BYTECODE_LEN_THRESHOLD && (params == 1 || params == 2);
        Self {
            jit_fn_ptr,
            params,
            bytecode_len,
            tb_eligible,
        }
    }

    pub fn eligible(&self) -> bool {
        self.tb_eligible
    }
}

pub fn lejit_tb_enabled() -> bool {
    std::env::var("CRUFT_LEJIT_TB")
        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_minimal_eligible() {
        let m = TinyBaselineMetadata::build(0xdeadbeef, 1, 20);
        assert_eq!(m.jit_fn_ptr, 0xdeadbeef);
        assert_eq!(m.params, 1);
        assert_eq!(m.bytecode_len, 20);
        assert!(m.tb_eligible);
        assert!(m.eligible());
    }

    #[test]
    fn ineligible_due_to_size() {
        let m = TinyBaselineMetadata::build(0x1000, 1, 100);
        assert!(!m.eligible(), "100-byte function exceeds threshold");
    }

    #[test]
    fn ineligible_due_to_arity() {
        let m0 = TinyBaselineMetadata::build(0x1000, 0, 10);
        let m3 = TinyBaselineMetadata::build(0x1000, 3, 10);
        assert!(!m0.eligible(), "0-arg ineligible at first cut");
        assert!(!m3.eligible(), "3+-arg ineligible at first cut");
    }

    #[test]
    fn boundary_at_threshold() {
        let m_at = TinyBaselineMetadata::build(0x1000, 1, TB_BYTECODE_LEN_THRESHOLD);
        let m_over = TinyBaselineMetadata::build(0x1000, 1, TB_BYTECODE_LEN_THRESHOLD + 1);
        assert!(m_at.eligible(), "at-threshold is inclusive");
        assert!(!m_over.eligible(), "over-threshold ineligible");
    }

    #[test]
    fn two_arg_eligible() {
        let m = TinyBaselineMetadata::build(0x1000, 2, 10);
        assert!(m.eligible());
    }

    #[test]
    fn env_flag_default_on_post_tb_ext_8() {

        std::env::remove_var("CRUFT_LEJIT_TB");
        assert!(lejit_tb_enabled());
    }

    #[test]
    fn env_flag_opt_out_via_zero() {
        std::env::set_var("CRUFT_LEJIT_TB", "0");
        assert!(!lejit_tb_enabled());
        std::env::remove_var("CRUFT_LEJIT_TB");
    }

    #[test]
    fn env_flag_opt_out_via_false_case_insensitive() {
        std::env::set_var("CRUFT_LEJIT_TB", "FaLsE");
        assert!(!lejit_tb_enabled());
        std::env::remove_var("CRUFT_LEJIT_TB");
    }

    #[test]
    fn env_flag_on_via_one_explicit() {
        std::env::set_var("CRUFT_LEJIT_TB", "1");
        assert!(lejit_tb_enabled());
        std::env::remove_var("CRUFT_LEJIT_TB");
    }
}

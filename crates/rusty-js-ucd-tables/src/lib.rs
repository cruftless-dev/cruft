
pub const UNICODE_VERSION: &str = "17.0.0";

pub mod decomp_tables;
pub mod identifier_tables;
pub mod normalization;
pub mod normalization_tables;

pub use normalization::{normalize_str, NormalizationForm};

pub const S_BASE: u32 = 0xAC00;
pub const L_BASE: u32 = 0x1100;
pub const V_BASE: u32 = 0x1161;
pub const T_BASE: u32 = 0x11A7;
pub const L_COUNT: u32 = 19;
pub const V_COUNT: u32 = 21;
pub const T_COUNT: u32 = 28;
pub const N_COUNT: u32 = V_COUNT * T_COUNT;
pub const S_COUNT: u32 = L_COUNT * N_COUNT;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lowered {
    Hangul {
        l_index: u32,
        v_index: u32,
        t_index: u32,
    },
    Positional {
        cp: u32,
        decomposition: &'static [u32],
    },
    Orthographic {
        cp: u32,
        decomposition: &'static [u32],
    },
    LegacyAlias {
        cp: u32,
        decomposition: &'static [u32],
    },
    CursiveShape {
        cp: u32,
        decomposition: &'static [u32],
    },
    Residual {
        cp: u32,
        decomposition: &'static [u32],
    },
}

pub trait UcdDecompResolver {
    fn lower(&self, cp: u32) -> Option<Lowered>;
    fn lift(&self, lowered: &Lowered) -> Vec<u32>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct HangulResolver;

impl UcdDecompResolver for HangulResolver {
    fn lower(&self, cp: u32) -> Option<Lowered> {
        if !(S_BASE..S_BASE + S_COUNT).contains(&cp) {
            return None;
        }
        let s_index = cp - S_BASE;
        let l_index = s_index / N_COUNT;
        let v_index = (s_index % N_COUNT) / T_COUNT;
        let t_index = s_index % T_COUNT;
        Some(Lowered::Hangul {
            l_index,
            v_index,
            t_index,
        })
    }

    fn lift(&self, lowered: &Lowered) -> Vec<u32> {
        match lowered {
            Lowered::Hangul {
                l_index,
                v_index,
                t_index,
            } => {
                let l = L_BASE + l_index;
                let v = V_BASE + v_index;
                if *t_index == 0 {
                    vec![l, v]
                } else {
                    vec![l, v, T_BASE + t_index]
                }
            }
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PositionalResolver;
#[derive(Debug, Default, Clone, Copy)]
pub struct OrthographicResolver;
#[derive(Debug, Default, Clone, Copy)]
pub struct LegacyAliasResolver;
#[derive(Debug, Default, Clone, Copy)]
pub struct CursiveShapeResolver;
#[derive(Debug, Default, Clone, Copy)]
pub struct ResidualResolver;

impl UcdDecompResolver for PositionalResolver {
    fn lower(&self, cp: u32) -> Option<Lowered> {
        decomp_tables::lookup_positional(cp)
            .map(|decomposition| Lowered::Positional { cp, decomposition })
    }
    fn lift(&self, lowered: &Lowered) -> Vec<u32> {
        match lowered {
            Lowered::Positional { decomposition, .. } => decomposition.to_vec(),
            _ => Vec::new(),
        }
    }
}

impl UcdDecompResolver for OrthographicResolver {
    fn lower(&self, cp: u32) -> Option<Lowered> {
        decomp_tables::lookup_orthographic(cp)
            .map(|decomposition| Lowered::Orthographic { cp, decomposition })
    }
    fn lift(&self, lowered: &Lowered) -> Vec<u32> {
        match lowered {
            Lowered::Orthographic { decomposition, .. } => decomposition.to_vec(),
            _ => Vec::new(),
        }
    }
}

impl UcdDecompResolver for LegacyAliasResolver {
    fn lower(&self, cp: u32) -> Option<Lowered> {
        decomp_tables::lookup_legacy(cp)
            .map(|decomposition| Lowered::LegacyAlias { cp, decomposition })
    }
    fn lift(&self, lowered: &Lowered) -> Vec<u32> {
        match lowered {
            Lowered::LegacyAlias { decomposition, .. } => decomposition.to_vec(),
            _ => Vec::new(),
        }
    }
}

impl UcdDecompResolver for CursiveShapeResolver {
    fn lower(&self, cp: u32) -> Option<Lowered> {
        decomp_tables::lookup_cursive(cp)
            .map(|decomposition| Lowered::CursiveShape { cp, decomposition })
    }
    fn lift(&self, lowered: &Lowered) -> Vec<u32> {
        match lowered {
            Lowered::CursiveShape { decomposition, .. } => decomposition.to_vec(),
            _ => Vec::new(),
        }
    }
}

impl UcdDecompResolver for ResidualResolver {
    fn lower(&self, cp: u32) -> Option<Lowered> {
        decomp_tables::lookup_residual(cp)
            .map(|decomposition| Lowered::Residual { cp, decomposition })
    }
    fn lift(&self, lowered: &Lowered) -> Vec<u32> {
        match lowered {
            Lowered::Residual { decomposition, .. } => decomposition.to_vec(),
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UcdResolution {
    hangul: HangulResolver,
    positional: PositionalResolver,
    orthographic: OrthographicResolver,
    legacy_alias: LegacyAliasResolver,
    cursive_shape: CursiveShapeResolver,
    residual: ResidualResolver,
}

impl UcdResolution {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lower(&self, cp: u32) -> Option<Lowered> {
        self.hangul
            .lower(cp)
            .or_else(|| self.positional.lower(cp))
            .or_else(|| self.orthographic.lower(cp))
            .or_else(|| self.legacy_alias.lower(cp))
            .or_else(|| self.cursive_shape.lower(cp))
            .or_else(|| self.residual.lower(cp))
    }

    pub fn lift(&self, lowered: &Lowered) -> Vec<u32> {
        match lowered {
            Lowered::Hangul { .. } => self.hangul.lift(lowered),
            Lowered::Positional { .. } => self.positional.lift(lowered),
            Lowered::Orthographic { .. } => self.orthographic.lift(lowered),
            Lowered::LegacyAlias { .. } => self.legacy_alias.lift(lowered),
            Lowered::CursiveShape { .. } => self.cursive_shape.lift(lowered),
            Lowered::Residual { .. } => self.residual.lift(lowered),
        }
    }

    pub fn decompose(&self, cp: u32) -> Option<Vec<u32>> {
        self.lower(cp).map(|lowered| self.lift(&lowered))
    }
}

pub fn residual_lookup(cp: u32) -> Option<Vec<u32>> {
    UcdResolution::new().decompose(cp)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected_hangul_decomposition(cp: u32) -> Vec<u32> {
        let s_index = cp - S_BASE;
        let l = L_BASE + (s_index / N_COUNT);
        let v = V_BASE + ((s_index % N_COUNT) / T_COUNT);
        let t_index = s_index % T_COUNT;
        if t_index == 0 {
            vec![l, v]
        } else {
            vec![l, v, T_BASE + t_index]
        }
    }

    #[test]
    fn hangul_bounds() {
        let resolver = UcdResolution::new();
        assert_eq!(resolver.decompose(S_BASE - 1), None);
        assert_eq!(resolver.decompose(S_BASE), Some(vec![0x1100, 0x1161]));
        assert_eq!(
            resolver.decompose(0xAC01),
            Some(vec![0x1100, 0x1161, 0x11A8])
        );
        assert_eq!(
            resolver.decompose(S_BASE + S_COUNT - 1),
            Some(vec![0x1112, 0x1175, 0x11C2])
        );
        assert_eq!(resolver.decompose(S_BASE + S_COUNT), None);
    }

    #[test]
    fn hangul_closure_law_all_syllables() {
        let resolver = UcdResolution::new();
        let mut claimed = 0u32;
        for cp in S_BASE..S_BASE + S_COUNT {
            let lowered = resolver.lower(cp).expect("Hangul syllable must be claimed");
            let lifted = resolver.lift(&lowered);
            assert_eq!(lifted, expected_hangul_decomposition(cp), "U+{cp:04X}");
            claimed += 1;
        }
        assert_eq!(claimed, 11_172);
    }

    #[test]
    fn statistical_resolvers_claim_representative_rows() {
        let resolver = UcdResolution::new();
        assert_eq!(resolver.decompose(0xFF21), Some(vec![0x0041]));
        assert_eq!(resolver.decompose(0x00C0), Some(vec![0x0041, 0x0300]));
        assert_eq!(resolver.decompose(0x00A0), Some(vec![0x0020]));
        assert_eq!(resolver.decompose(0xFE91), Some(vec![0x0628]));
    }

    #[test]
    fn explicit_decomposition_closure_law_all_rows() {
        let resolver = UcdResolution::new();
        let mut checked = 0usize;
        for (cp, expected) in decomp_tables::explicit_decompositions() {
            let lowered = resolver.lower(cp).expect("explicit row must be claimed");
            assert_eq!(resolver.lift(&lowered), expected, "U+{cp:04X}");
            checked += 1;
        }
        assert_eq!(checked, decomp_tables::EXPLICIT_DECOMPOSITION_COUNT);
        assert_eq!(checked, 5_914);
    }
}

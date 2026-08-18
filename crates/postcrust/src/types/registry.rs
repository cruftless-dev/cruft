
use super::composite::CompositeInfo;
use super::domains::DomainInfo;
use super::enums::EnumInfo;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct TypeRegistries {

    pub enums: HashMap<u32, EnumInfo>,

    pub enums_by_name: HashMap<String, EnumInfo>,

    pub domains: HashMap<String, DomainInfo>,

    pub composites: HashMap<u32, CompositeInfo>,

    pub composites_by_name: HashMap<String, CompositeInfo>,

    pub functions: HashMap<String, crate::catalog::FunctionDef>,

    pub operators: HashMap<String, String>,

    pub aggregates: HashMap<String, crate::catalog::AggregateDef>,

    pub casts: HashMap<(u32, u32), crate::catalog::CastDef>,
}

impl TypeRegistries {

    pub fn is_composite(&self, oid: u32) -> bool {
        self.composites.contains_key(&oid)
    }

    pub fn composite(&self, oid: u32) -> Option<&CompositeInfo> {
        self.composites.get(&oid)
    }

    pub fn composite_by_name(&self, name: &str) -> Option<&CompositeInfo> {
        self.composites_by_name.get(name)
    }

    pub fn composite_oid_by_name(&self, name: &str) -> Option<u32> {
        self.composites
            .iter()
            .find(|(_, ci)| ci.name == name)
            .map(|(oid, _)| *oid)
    }

    pub fn is_enum(&self, oid: u32) -> bool {
        self.enums.contains_key(&oid)
    }

    pub fn enum_name(&self, oid: u32) -> Option<&str> {
        self.enums.get(&oid).map(|i| i.name.as_str())
    }

    pub fn labels(&self, oid: u32) -> Option<&[String]> {
        self.enums.get(&oid).map(|i| i.labels.as_slice())
    }

    pub fn ordinal(&self, oid: u32, label: &str) -> Option<usize> {
        self.enums
            .get(&oid)
            .and_then(|i| i.labels.iter().position(|l| l == label))
    }

    pub fn labels_by_name(&self, name: &str) -> Option<&[String]> {
        self.enums_by_name.get(name).map(|i| i.labels.as_slice())
    }

    pub fn domain(&self, name: &str) -> Option<&DomainInfo> {
        self.domains.get(name)
    }
}

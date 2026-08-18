
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

thread_local! {
    static NARROW_OPERAND_SEEDED_POOL: ConstantsPool = ConstantsPool::build_narrow_operand_seeded();
}

const NARROW_OPERAND_SEED_STRINGS: &[&str] = &[
    "length",
    "done",
    "value",
    "next",
    "@@iterator",
    "Array",
    "Object",
    "String",
    "Number",
    "Boolean",
    "BigInt",
    "Symbol",
    "RegExp",
    "Map",
    "Set",
    "Promise",
    "Math",
    "JSON",
    "Reflect",
    "Proxy",
    "TypeError",
    "RangeError",
    "SyntaxError",
    "ReferenceError",
    "console",
    "log",
    "assert",
    "sameValue",
    "fromCharCode",
    "toUpperCase",
    "toLowerCase",
    "prototype",
    "constructor",
    "__apply",
    "__array_extend",
    "__array_push_single",
    "__await",
    "__construct",
    "__createRegExp",
    "__define_public_field__",
    "__destr_iter_close",
    "__destr_iter_open",
    "__destr_iter_rest",
    "__destr_iter_step",
    "__destr_object_check",
    "__destr_object_rest",
    "__for_await_iter_open",
    "__for_in_keys",
    "__for_in_receiver",
    "__global_binding_exists",
    "__init_private_field__",
    "__install_accessor__",
    "__install_accessor_obj__",
    "__install_method__",
    "__iter_result_check",
    "__mark_private_name__",
    "__object_spread",
    "__script_var_global_bind",
    "__super_apply",
    "__super_base_home",
    "__super_delete",
    "__super_get",
    "__super_get_base",
    "__super_set",
    "__template_object__",
    "__to_property_key__",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionConstantMetadata {
    pub display_name: String,
    pub function_length: u16,
    pub is_async: bool,
    pub is_generator: bool,
    pub is_method: bool,
    pub strict: bool,
}

impl FunctionConstantMetadata {
    pub fn from_proto(proto: &crate::compiler::FunctionProto) -> Self {
        Self {
            display_name: proto.display_name.clone(),
            function_length: proto.function_length,
            is_async: proto.is_async,
            is_generator: proto.is_generator,
            is_method: proto.is_method,
            strict: proto.strict,
        }
    }

    pub fn is_annex_b_legacy_target(&self, is_arrow: bool) -> bool {
        !self.strict
            && !is_arrow
            && !self.is_generator
            && !self.is_async
            && !self.is_method
            && !self.display_name.starts_with("get ")
            && !self.display_name.starts_with("set ")
    }
}

impl Default for FunctionConstantMetadata {
    fn default() -> Self {
        Self {
            display_name: String::new(),
            function_length: 0,
            is_async: false,
            is_generator: false,
            is_method: false,
            strict: false,
        }
    }
}

pub struct LazyFunctionConstant {
    metadata: FunctionConstantMetadata,
    captures: Vec<crate::compiler::UpvalueDescriptor>,
    cache: Rc<
        RefCell<Option<Result<Rc<crate::compiler::FunctionProto>, crate::compiler::CompileError>>>,
    >,
    materialize:
        Rc<dyn Fn() -> Result<Rc<crate::compiler::FunctionProto>, crate::compiler::CompileError>>,
    materialization_count: Rc<Cell<u32>>,
}

impl LazyFunctionConstant {
    pub fn new(materialize: impl Fn() -> Rc<crate::compiler::FunctionProto> + 'static) -> Self {
        Self::new_with_metadata(FunctionConstantMetadata::default(), materialize)
    }

    pub fn new_with_metadata(
        metadata: FunctionConstantMetadata,
        materialize: impl Fn() -> Rc<crate::compiler::FunctionProto> + 'static,
    ) -> Self {
        Self::new_with_metadata_and_captures(metadata, Vec::new(), materialize)
    }

    pub fn new_with_metadata_and_captures(
        metadata: FunctionConstantMetadata,
        captures: Vec<crate::compiler::UpvalueDescriptor>,
        materialize: impl Fn() -> Rc<crate::compiler::FunctionProto> + 'static,
    ) -> Self {
        Self::new_fallible_with_metadata_and_captures(metadata, captures, move || Ok(materialize()))
    }

    pub fn new_fallible_with_metadata_and_captures(
        metadata: FunctionConstantMetadata,
        captures: Vec<crate::compiler::UpvalueDescriptor>,
        materialize: impl Fn() -> Result<Rc<crate::compiler::FunctionProto>, crate::compiler::CompileError>
            + 'static,
    ) -> Self {
        Self {
            metadata,
            captures,
            cache: Rc::new(RefCell::new(None)),
            materialize: Rc::new(materialize),
            materialization_count: Rc::new(Cell::new(0)),
        }
    }

    pub fn metadata(&self) -> &FunctionConstantMetadata {
        &self.metadata
    }

    pub fn captures(&self) -> &[crate::compiler::UpvalueDescriptor] {
        &self.captures
    }

    pub fn proto_result(
        &self,
    ) -> Result<Rc<crate::compiler::FunctionProto>, crate::compiler::CompileError> {
        if let Some(result) = self.cache.borrow().as_ref().cloned() {
            return result;
        }
        let result = (self.materialize)();
        self.materialization_count
            .set(self.materialization_count.get() + 1);
        *self.cache.borrow_mut() = Some(result.clone());
        result
    }

    pub fn proto_rc(&self) -> Rc<crate::compiler::FunctionProto> {
        self.proto_result()
            .unwrap_or_else(|err| panic!("lazy function materialization failed: {}", err.message))
    }

    pub fn materialization_count_for_tests(&self) -> u32 {
        self.materialization_count.get()
    }
}

impl Clone for LazyFunctionConstant {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata.clone(),
            captures: self.captures.clone(),
            cache: self.cache.clone(),
            materialize: self.materialize.clone(),
            materialization_count: self.materialization_count.clone(),
        }
    }
}

impl fmt::Debug for LazyFunctionConstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LazyFunctionConstant")
            .field("metadata", &self.metadata)
            .field("captures", &self.captures)
            .field("cached", &self.cache.borrow().is_some())
            .field("materialization_count", &self.materialization_count.get())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub enum Constant {
    Number(f64),
    BigInt(String),
    String(String),

    WtfString(Vec<u16>),
    Regex {
        body: String,
        flags: String,
    },

    Function(std::rc::Rc<crate::compiler::FunctionProto>),

    LazyFunction(LazyFunctionConstant),
}

#[derive(Debug, Default, Clone)]
pub struct ConstantsPool {
    entries: Vec<Constant>,
    dedup_index: HashMap<DedupKey, u32>,

    literals: Vec<Constant>,
    literals_dedup: HashMap<DedupKey, u32>,

    overflowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum DedupKey {
    WtfString(Vec<u16>),
    Number(u64),
    BigInt(String),
    String(String),
    Regex { body: String, flags: String },
}

impl DedupKey {
    fn from_constant(c: &Constant) -> Option<Self> {
        match c {
            Constant::Number(x) => Some(DedupKey::Number(x.to_bits())),
            Constant::BigInt(x) => Some(DedupKey::BigInt(x.clone())),
            Constant::String(x) => Some(DedupKey::String(x.clone())),
            Constant::WtfString(x) => Some(DedupKey::WtfString(x.clone())),
            Constant::Regex { body, flags } => Some(DedupKey::Regex {
                body: body.clone(),
                flags: flags.clone(),
            }),
            Constant::Function(_) | Constant::LazyFunction(_) => None,
        }
    }
}

impl ConstantsPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_narrow_operand_seed() -> Self {
        NARROW_OPERAND_SEEDED_POOL.with(Clone::clone)
    }

    fn build_narrow_operand_seeded() -> Self {
        let mut pool = Self::new();
        for name in NARROW_OPERAND_SEED_STRINGS {
            pool.intern(Constant::String((*name).into()));
        }
        pool
    }

    pub fn intern(&mut self, c: Constant) -> u16 {
        let idx = self.intern_u32(c);
        if idx >= u16::MAX as u32 {

            self.overflowed = true;
            return u16::MAX;
        }
        idx as u16
    }

    pub fn overflowed(&self) -> bool {
        self.overflowed
    }

    pub fn intern_u32(&mut self, c: Constant) -> u32 {
        if let Some(key) = DedupKey::from_constant(&c) {
            if let Some(&idx) = self.dedup_index.get(&key) {
                return idx;
            }
            let idx = self.entries.len();
            assert!(idx <= u32::MAX as usize, "constants pool overflow");
            self.entries.push(c);
            self.dedup_index.insert(key, idx as u32);
            return idx as u32;
        }

        let idx = self.entries.len();
        assert!(idx <= u32::MAX as usize, "constants pool overflow");
        self.entries.push(c);
        idx as u32
    }

    pub fn get(&self, idx: u16) -> Option<&Constant> {
        self.entries.get(idx as usize)
    }

    pub fn get_u32(&self, idx: u32) -> Option<&Constant> {
        self.entries.get(idx as usize)
    }

    pub fn intern_literal(&mut self, c: Constant) -> u32 {
        if let Some(key) = DedupKey::from_constant(&c) {
            if let Some(&idx) = self.literals_dedup.get(&key) {
                return idx;
            }
            let idx = self.literals.len() as u32;
            self.literals.push(c);
            self.literals_dedup.insert(key, idx);
            return idx;
        }

        let idx = self.literals.len() as u32;
        self.literals.push(c);
        idx
    }

    pub fn get_literal(&self, idx: u32) -> Option<&Constant> {
        self.literals.get(idx as usize)
    }

    pub fn entries(&self) -> &[Constant] {
        &self.entries
    }

    pub fn literals(&self) -> &[Constant] {
        &self.literals
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn size_accounting(&self) -> (usize, usize, usize) {
        let backbone = self.entries.capacity() * std::mem::size_of::<Constant>();
        let mut payload = 0usize;
        for e in &self.entries {
            payload += match e {
                Constant::Number(_) => 0,
                Constant::BigInt(s) | Constant::String(s) => s.capacity(),
                Constant::WtfString(v) => v.capacity() * 2,
                Constant::Regex { body, flags } => body.capacity() + flags.capacity(),
                Constant::Function(_) | Constant::LazyFunction(_) => 0,
            };
        }

        let mut dedup = self.dedup_index.capacity()
            * (std::mem::size_of::<DedupKey>() + std::mem::size_of::<u32>() + 1);
        for k in self.dedup_index.keys() {
            dedup += match k {
                DedupKey::Number(_) => 0,
                DedupKey::BigInt(s) | DedupKey::String(s) => s.capacity(),
                DedupKey::WtfString(v) => v.capacity() * 2,
                DedupKey::Regex { body, flags } => body.capacity() + flags.capacity(),
            };
        }
        (backbone, payload, dedup)
    }

    pub fn shrink_to_fit(&mut self) {
        self.entries.shrink_to_fit();
        self.literals.shrink_to_fit();
    }

    pub fn drop_dedup_index(&mut self) {
        self.dedup_index = HashMap::new();
        self.literals_dedup = HashMap::new();
    }
}

fn same_constant(a: &Constant, b: &Constant) -> bool {
    match (a, b) {
        (Constant::Number(x), Constant::Number(y)) => x.to_bits() == y.to_bits(),
        (Constant::BigInt(x), Constant::BigInt(y)) => x == y,
        (Constant::String(x), Constant::String(y)) => x == y,
        (Constant::WtfString(x), Constant::WtfString(y)) => x == y,
        (
            Constant::Regex {
                body: b1,
                flags: f1,
            },
            Constant::Regex {
                body: b2,
                flags: f2,
            },
        ) => b1 == b2 && f1 == f2,

        (Constant::Function(_), Constant::Function(_)) => false,
        (Constant::LazyFunction(_), Constant::LazyFunction(_)) => false,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Constant, ConstantsPool, FunctionConstantMetadata, LazyFunctionConstant};
    use crate::compiler::{CompileError, FunctionProto};
    use crate::compiler::{UpvalueDescriptor, UpvalueSource};
    use std::rc::Rc;

    fn empty_proto(display_name: &str) -> FunctionProto {
        FunctionProto {
            bytecode: Vec::new(),
            constants: ConstantsPool::new(),
            params: 0,
            display_name: display_name.to_string(),
            function_length: 0,
            has_simple_parameters: true,
            locals: Vec::new(),
            catch_param_names: Vec::new(),
            upvalues: Vec::new(),
            rest_param_slot: None,
            arguments_slot: None,
            self_name_slot: None,
            self_name_is_immutable: false,
            param_prologue_end: 0,
            is_generator: false,
            line_starts: Vec::new().into(),
            source_map: Vec::new(),
            construct_tags: Vec::new(),
            source_url: String::new(),
            source_text: None,
            diagnostic_source_text: None,
            is_async: false,
            strict: false,
            is_method: false,
            arguments_forbidden: false,
            is_class_constructor: false,
        }
    }

    #[test]
    fn narrow_operand_seeded_pool_preserves_low_common_string_indexes() {
        let mut pool = ConstantsPool::new_with_narrow_operand_seed();

        let length = pool.intern(Constant::String("length".into()));
        let helper = pool.intern(Constant::String("__to_property_key__".into()));
        let fresh = pool.intern(Constant::String("__not_seeded_for_test__".into()));
        let console = pool.intern(Constant::String("console".into()));
        let log = pool.intern(Constant::String("log".into()));

        assert_eq!(length, 0);
        assert!(helper < fresh);
        assert!(console < fresh);
        assert!(log < fresh);
        assert!(helper < u16::MAX);
        assert_eq!(
            pool.intern(Constant::String("__to_property_key__".into())),
            helper
        );
    }

    #[test]
    fn lazy_function_constant_materializes_once_and_is_not_deduped() {
        let calls = Rc::new(std::cell::Cell::new(0_u32));
        let materializer_calls = calls.clone();
        let metadata = FunctionConstantMetadata {
            display_name: "lazy-meta-test".to_string(),
            function_length: 2,
            is_async: false,
            is_generator: false,
            is_method: false,
            strict: false,
        };
        let lazy = LazyFunctionConstant::new_with_metadata(metadata.clone(), move || {
            materializer_calls.set(materializer_calls.get() + 1);
            Rc::new(empty_proto("lazy-constant-test"))
        });
        let lazy_clone = lazy.clone();

        let mut pool = ConstantsPool::new();
        let first_idx = pool.intern(Constant::LazyFunction(lazy.clone()));
        let second_idx = pool.intern(Constant::LazyFunction(lazy_clone.clone()));

        assert_ne!(first_idx, second_idx);
        assert_eq!(lazy.metadata(), &metadata);
        assert!(lazy.captures().is_empty());
        assert!(lazy.metadata().is_annex_b_legacy_target(false));
        assert_eq!(lazy.materialization_count_for_tests(), 0);
        assert_eq!(calls.get(), 0);

        let first = lazy.proto_rc();
        let second = lazy_clone.proto_rc();

        assert_eq!(lazy.materialization_count_for_tests(), 1);
        assert_eq!(calls.get(), 1);
        assert!(Rc::ptr_eq(&first, &second));
        assert_eq!(first.display_name, "lazy-constant-test");
    }

    #[test]
    fn lazy_function_constant_carries_preflight_captures_without_materializing() {
        let calls = Rc::new(std::cell::Cell::new(0_u32));
        let materializer_calls = calls.clone();
        let captures = vec![UpvalueDescriptor {
            source: UpvalueSource::Local(7),
            name: "outer".to_string(),
            is_fn_self_name: false,
        }];
        let lazy = LazyFunctionConstant::new_with_metadata_and_captures(
            FunctionConstantMetadata::default(),
            captures.clone(),
            move || {
                materializer_calls.set(materializer_calls.get() + 1);
                Rc::new(empty_proto("lazy-capture-preflight"))
            },
        );
        let lazy_clone = lazy.clone();

        assert_eq!(lazy.captures(), captures.as_slice());
        assert_eq!(lazy_clone.captures(), captures.as_slice());
        assert_eq!(lazy.materialization_count_for_tests(), 0);
        assert_eq!(calls.get(), 0);

        let first = lazy_clone.proto_rc();

        assert_eq!(lazy.materialization_count_for_tests(), 1);
        assert_eq!(calls.get(), 1);
        assert_eq!(first.display_name, "lazy-capture-preflight");
    }

    #[test]
    fn lazy_function_constant_caches_materialization_failure_once() {
        let calls = Rc::new(std::cell::Cell::new(0_u32));
        let materializer_calls = calls.clone();
        let lazy = LazyFunctionConstant::new_fallible_with_metadata_and_captures(
            FunctionConstantMetadata::default(),
            Vec::new(),
            move || {
                materializer_calls.set(materializer_calls.get() + 1);
                Err(CompileError {
                    span: rusty_js_ast::Span::new(1, 2),
                    message: "lazy failure".to_string(),
                })
            },
        );
        let lazy_clone = lazy.clone();

        let first = lazy.proto_result().unwrap_err();
        let second = lazy_clone.proto_result().unwrap_err();

        assert_eq!(lazy.materialization_count_for_tests(), 1);
        assert_eq!(calls.get(), 1);
        assert_eq!(first.message, "lazy failure");
        assert_eq!(second.message, "lazy failure");
        assert_eq!(first.span, second.span);
    }
}

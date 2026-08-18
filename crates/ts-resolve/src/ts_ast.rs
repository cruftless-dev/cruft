
use rusty_js_ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TsTypeRef {

    Primitive(String),

    Named {
        name: String,
        type_args: Vec<TsTypeRef>,
    },

    Array(Box<TsTypeRef>),

    Tuple(Vec<TsTypeRef>),

    Union(Vec<TsTypeRef>),

    Intersection(Vec<TsTypeRef>),

    ObjectLit(Vec<TsObjectMember>),

    FnType {
        params: Vec<TsFnParam>,
        ret: Box<TsTypeRef>,
    },

    Indexed {
        target: Box<TsTypeRef>,
        index: Box<TsTypeRef>,
    },

    TypeOf(String),

    Literal(TsLiteralVal),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TsObjectMember {
    pub name: String,
    pub ty: TsTypeRef,
    pub optional: bool,
    pub readonly: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TsFnParam {
    pub name: String,
    pub ty: TsTypeRef,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TsLiteralVal {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
    Undefined,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TsAnnotation {
    pub ty: TsTypeRef,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeWitness {
    pub kind: TypeWitnessKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeWitnessKind {

    LocalBinding { name: String, ty: TsTypeRef },

    FnParam {
        fn_name: Option<String>,
        param_idx: u8,
        ty: TsTypeRef,
    },

    FnReturn {
        fn_name: Option<String>,
        ty: TsTypeRef,
    },

    ClassField {
        class_name: String,
        field: String,
        ty: TsTypeRef,
    },

    EnumLowering {
        name: String,
        members: Vec<(String, f64)>,
    },
}

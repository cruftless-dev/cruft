
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    NullLiteral {
        span: Span,
    },
    BoolLiteral {
        value: bool,
        span: Span,
    },
    NumberLiteral {
        value: f64,
        span: Span,
    },
    BigIntLiteral {
        digits: String,
        span: Span,
    },
    StringLiteral {
        value: String,
        span: Span,
    },

    WtfStringLiteral {
        units: Vec<u16>,
        span: Span,
    },
    Identifier {
        name: String,
        span: Span,
    },
    This {
        span: Span,
    },
    Super {
        span: Span,
    },
    MetaProperty {
        meta: String,
        property: String,
        span: Span,
    },
    Array {
        elements: Vec<ArrayElement>,
        trailing_comma_after_spread: bool,
        span: Span,
    },
    Object {
        properties: Vec<ObjectProperty>,

        trailing_comma_after_spread: bool,
        span: Span,
    },
    Parenthesized {
        expr: Box<Expr>,
        span: Span,
    },
    Member {
        object: Box<Expr>,
        property: Box<MemberProperty>,
        optional: bool,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Argument>,
        optional: bool,
        span: Span,
    },
    New {
        callee: Box<Expr>,
        arguments: Vec<Argument>,
        span: Span,
    },
    Update {
        operator: UpdateOp,
        argument: Box<Expr>,
        prefix: bool,
        span: Span,
    },
    Unary {
        operator: UnaryOp,
        argument: Box<Expr>,
        span: Span,
    },
    Binary {
        operator: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    Conditional {
        test: Box<Expr>,
        consequent: Box<Expr>,
        alternate: Box<Expr>,
        span: Span,
    },
    Assign {
        operator: AssignOp,
        target: Box<Expr>,
        value: Box<Expr>,
        span: Span,
    },
    Sequence {
        expressions: Vec<Expr>,
        span: Span,
    },

    Function {
        name: Option<BindingIdentifier>,
        is_async: bool,
        is_generator: bool,

        is_method: bool,
        params: Vec<Parameter>,
        body: Vec<Stmt>,
        span: Span,
    },

    Class {
        decorators: Vec<String>,
        name: Option<BindingIdentifier>,
        super_class: Option<Box<Expr>>,
        members: Vec<ClassMember>,
        span: Span,
    },

    Arrow {
        is_async: bool,
        params: Vec<Parameter>,
        body: ArrowBody,
        span: Span,
    },

    TemplateLiteral {
        quasis: Vec<std::rc::Rc<String>>,
        raw_quasis: Vec<std::rc::Rc<String>>,
        expressions: Vec<Expr>,
        span: Span,
    },

    TemplateObject {
        cooked: Vec<Option<std::rc::Rc<String>>>,
        raw: Vec<std::rc::Rc<String>>,
        span: Span,
    },

    RegExp {
        pattern: std::rc::Rc<String>,
        flags: std::rc::Rc<String>,
        span: Span,
    },

    Opaque {
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrowBody {

    Expression(Box<Expr>),

    Block(Vec<Stmt>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassMember {

    Method {
        name: ClassMemberName,
        kind: MethodKind,
        is_static: bool,
        is_async: bool,
        is_generator: bool,
        params: Vec<Parameter>,
        body: Vec<Stmt>,
        span: Span,
    },

    Field {
        name: ClassMemberName,
        is_static: bool,
        init: Option<Expr>,
        span: Span,
    },

    StaticBlock { body: Vec<Stmt>, span: Span },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    Method,
    Constructor,
    Getter,
    Setter,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassMemberName {
    Identifier { name: String, span: Span },
    String { value: String, span: Span },
    Number { value: f64, span: Span },
    Computed { expr: Expr, span: Span },
    Private { name: String, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub enum MemberProperty {
    Identifier { name: String, span: Span },
    Computed { expr: Expr, span: Span },
    Private { name: String, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Argument {
    Expr(Expr),
    Spread { expr: Expr, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayElement {
    Elision { span: Span },
    Expr(Expr),
    Spread { expr: Expr, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectProperty {
    Property {
        key: ObjectKey,
        value: Expr,
        shorthand: bool,
        kind: ObjectPropertyKind,
        span: Span,
    },
    Spread {
        expr: Expr,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectPropertyKind {
    Init,
    Get,
    Set,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectKey {
    Identifier { name: String, span: Span },
    String { value: String, span: Span },
    Number { value: f64, span: Span },
    Computed { expr: Expr, span: Span },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOp {
    Inc,
    Dec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
    BitNot,
    LogicalNot,
    Typeof,
    Void,
    Delete,
    Await,
    Yield,
    YieldDelegate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Shl,
    Shr,
    UShr,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    StrictEq,
    StrictNe,
    Instanceof,
    In,
    BitAnd,
    BitOr,
    BitXor,
    LogicalAnd,
    LogicalOr,
    NullishCoalesce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    PowAssign,
    ShlAssign,
    ShrAssign,
    UShrAssign,
    BitAndAssign,
    BitOrAssign,
    BitXorAssign,
    LogicalAndAssign,
    LogicalOrAssign,
    NullishAssign,
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::NullLiteral { span }
            | Expr::BoolLiteral { span, .. }
            | Expr::NumberLiteral { span, .. }
            | Expr::BigIntLiteral { span, .. }
            | Expr::StringLiteral { span, .. }
            | Expr::WtfStringLiteral { span, .. }
            | Expr::Identifier { span, .. }
            | Expr::This { span }
            | Expr::Super { span }
            | Expr::MetaProperty { span, .. }
            | Expr::Array { span, .. }
            | Expr::Object { span, .. }
            | Expr::Parenthesized { span, .. }
            | Expr::Member { span, .. }
            | Expr::Call { span, .. }
            | Expr::New { span, .. }
            | Expr::Update { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Conditional { span, .. }
            | Expr::Assign { span, .. }
            | Expr::Sequence { span, .. }
            | Expr::Function { span, .. }
            | Expr::Class { span, .. }
            | Expr::Arrow { span, .. }
            | Expr::TemplateLiteral { span, .. }
            | Expr::TemplateObject { span, .. }
            | Expr::RegExp { span, .. }
            | Expr::Opaque { span } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub span: Span,
    pub body: Vec<ModuleItem>,
    pub import_entries: Vec<ImportEntry>,

    pub local_export_entries: Vec<ExportEntry>,

    pub indirect_export_entries: Vec<ExportEntry>,

    pub star_export_entries: Vec<ExportEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModuleItem {
    Import(ImportDeclaration),
    Export(ExportDeclaration),
    Statement(Stmt),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Variable(VariableStatement),
    Expression {
        expr: Expr,
        span: Span,
    },
    Block {
        body: Vec<Stmt>,
        span: Span,
    },
    Empty {
        span: Span,
    },

    FunctionDecl {
        name: Option<BindingIdentifier>,
        is_async: bool,
        is_generator: bool,
        params: Vec<Parameter>,
        body: Vec<Stmt>,
        span: Span,
    },

    ClassDecl {
        decorators: Vec<String>,
        name: Option<BindingIdentifier>,
        super_class: Option<Expr>,
        members: Vec<ClassMember>,
        span: Span,
    },

    If {
        test: Expr,
        consequent: Box<Stmt>,
        alternate: Option<Box<Stmt>>,
        span: Span,
    },

    For {
        init: Option<ForInit>,
        test: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
        span: Span,
    },

    ForIn {
        left: ForBinding,
        right: Expr,
        body: Box<Stmt>,
        span: Span,
    },

    ForOf {
        left: ForBinding,
        right: Expr,
        body: Box<Stmt>,
        await_: bool,
        span: Span,
    },

    While {
        test: Expr,
        body: Box<Stmt>,
        span: Span,
    },

    With {
        object: Expr,
        body: Box<Stmt>,
        span: Span,
    },

    DoWhile {
        body: Box<Stmt>,
        test: Expr,
        span: Span,
    },

    Switch {
        discriminant: Expr,
        cases: Vec<SwitchCase>,
        span: Span,
    },

    Try {
        block: Box<Stmt>,
        handler: Option<CatchClause>,
        finalizer: Option<Box<Stmt>>,
        span: Span,

        disposal: bool,
    },

    Return {
        argument: Option<Expr>,
        span: Span,
    },

    Throw {
        argument: Expr,
        span: Span,
    },

    Break {
        label: Option<BindingIdentifier>,
        span: Span,
    },

    Continue {
        label: Option<BindingIdentifier>,
        span: Span,
    },

    Debugger {
        span: Span,
    },

    Labelled {
        label: BindingIdentifier,
        body: Box<Stmt>,
        span: Span,
    },

    Opaque {
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ForInit {
    Variable(VariableStatement),
    Expression(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ForBinding {

    Decl {
        kind: VariableKind,
        target: BindingPattern,
        span: Span,
    },

    Pattern(BindingPattern),

    AssignmentTarget(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {

    pub test: Option<Expr>,
    pub consequent: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {

    pub target: BindingPattern,

    pub default: Option<Expr>,

    pub rest: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatchClause {

    pub param: Option<BindingPattern>,
    pub body: Box<Stmt>,
    pub span: Span,
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Variable(v) => v.span,
            Stmt::Expression { span, .. }
            | Stmt::Block { span, .. }
            | Stmt::Empty { span }
            | Stmt::FunctionDecl { span, .. }
            | Stmt::ClassDecl { span, .. }
            | Stmt::If { span, .. }
            | Stmt::For { span, .. }
            | Stmt::ForIn { span, .. }
            | Stmt::ForOf { span, .. }
            | Stmt::While { span, .. }
            | Stmt::With { span, .. }
            | Stmt::DoWhile { span, .. }
            | Stmt::Switch { span, .. }
            | Stmt::Try { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Throw { span, .. }
            | Stmt::Break { span, .. }
            | Stmt::Continue { span, .. }
            | Stmt::Debugger { span }
            | Stmt::Labelled { span, .. }
            | Stmt::Opaque { span } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariableStatement {
    pub kind: VariableKind,
    pub declarators: Vec<VariableDeclarator>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableKind {
    Let,
    Const,
    Var,

    Using,

    AwaitUsing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariableDeclarator {

    pub target: BindingPattern,
    pub init: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BindingPattern {

    Identifier(BindingIdentifier),

    Array(ArrayPattern),

    Object(ObjectPattern),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayPattern {

    pub elements: Vec<Option<BindingElement>>,

    pub rest: Option<Box<BindingPattern>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingElement {
    pub target: BindingPattern,

    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectPattern {
    pub properties: Vec<ObjectPatternProperty>,

    pub rest: Option<Box<BindingIdentifier>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectPatternProperty {
    pub key: PropertyKey,
    pub value: BindingElement,

    pub shorthand: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyKey {
    Identifier(BindingIdentifier),
    String(std::rc::Rc<String>),
    Number(f64),

    Computed(Expr),
}

impl BindingPattern {

    pub fn collect_names(&self) -> Vec<&BindingIdentifier> {
        let mut out = Vec::new();
        self.collect_names_into(&mut out);
        out
    }
    pub fn collect_names_into<'a>(&'a self, out: &mut Vec<&'a BindingIdentifier>) {
        match self {
            BindingPattern::Identifier(id) => out.push(id),
            BindingPattern::Array(arr) => {
                for elem in &arr.elements {
                    if let Some(e) = elem {
                        e.target.collect_names_into(out);
                    }
                }
                if let Some(rest) = &arr.rest {
                    rest.collect_names_into(out);
                }
            }
            BindingPattern::Object(obj) => {
                for prop in &obj.properties {
                    prop.value.target.collect_names_into(out);
                }
                if let Some(rest) = &obj.rest {
                    out.push(rest);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportDeclaration {
    pub span: Span,

    pub source_phase: bool,

    pub import_defer: bool,
    pub specifier: ModuleSpecifier,
    pub default_binding: Option<BindingIdentifier>,
    pub namespace_binding: Option<BindingIdentifier>,
    pub named_imports: Vec<ImportSpecifier>,
    pub attributes: Vec<ImportAttribute>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportSpecifier {
    pub span: Span,

    pub imported: ModuleExportName,

    pub local: BindingIdentifier,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportDeclaration {

    Declaration {
        span: Span,

        decl_span: Span,

        names: Vec<BindingIdentifier>,

        decl_stmt: Option<Box<Stmt>>,
    },

    Named {
        span: Span,
        specifiers: Vec<ExportSpecifier>,

        source: Option<ModuleSpecifier>,
        attributes: Vec<ImportAttribute>,
    },

    StarFrom {
        span: Span,
        source: ModuleSpecifier,
        attributes: Vec<ImportAttribute>,
    },

    StarAsFrom {
        span: Span,
        exported: ModuleExportName,
        source: ModuleSpecifier,
        attributes: Vec<ImportAttribute>,
    },

    Default {
        span: Span,

        body: DefaultExportBody,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DefaultExportBody {

    HoistableFunction {
        name: Option<BindingIdentifier>,
        params: Vec<Parameter>,
        body: Vec<Stmt>,
        is_async: bool,
        is_generator: bool,
    },

    Class {
        name: Option<BindingIdentifier>,
        super_class: Option<Expr>,
        members: Vec<ClassMember>,
    },

    Expression { expr: Expr },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportSpecifier {
    pub span: Span,

    pub local: ModuleExportName,

    pub exported: ModuleExportName,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BindingIdentifier {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModuleExportName {

    Ident(BindingIdentifier),

    String { value: String, span: Span },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleSpecifier {
    pub value: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportAttribute {
    pub span: Span,
    pub key: ModuleExportName,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportEntry {
    pub module_request: String,

    pub import_name: ImportName,
    pub local_name: String,

    pub import_defer: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportName {

    Source,

    Default,

    Namespace,

    Single(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportEntry {
    pub export_name: Option<String>,
    pub module_request: Option<String>,
    pub import_name: Option<ExportImportName>,
    pub local_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportImportName {
    All,
    AllButDefault,
    Default,
    Single(String),
}

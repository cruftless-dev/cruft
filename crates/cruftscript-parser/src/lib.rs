use rusty_js_ast::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CruftScriptSourceUnit {
    pub source_kind: SourceKind,
    pub span: Span,
    pub items: Vec<SourceItem>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKind {
    NativeCruftScript,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceItem {
    BoundaryDefault(BoundaryDefaultDecl),
    BoundaryPolicy(BoundaryPolicyDecl),
    Compartment(CompartmentDecl),
    TypeAlias(TypeAliasDecl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryDefaultDecl {
    pub policy_name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryPolicyDecl {
    pub name: String,
    pub span: Span,
    pub body_span: Span,
    pub has_process_clause: bool,
    pub has_call_clause: bool,
    pub process_mode: Option<BoundaryPolicyProcessMode>,
    pub sanitizer_defaults: Vec<SanitizerDefaultDecl>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryPolicyProcessMode {
    Strict,
    Debug,
    Sanitize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizerDefaultDecl {
    pub target_type: TypeExpr,
    pub expr: SanitizerDefaultExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SanitizerDefaultExpr {
    StringLiteral(String),
    NumberLiteral(String),
    BooleanLiteral(bool),
    Null,
    Undefined,
    ArrayLiteral(Vec<ExprEnvelope>),
    ObjectLiteral(Vec<ObjectLiteralProperty>),
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompartmentDecl {
    pub name: String,
    pub boundary_clause: Option<BoundaryClause>,
    pub endowments: Vec<EndowmentDecl>,
    pub span: Span,
    pub body_span: Span,
    pub body_items: Vec<CompartmentItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndowmentDecl {
    pub name: String,
    pub type_annotation: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompartmentItem {
    Import(BoundaryQualifiedImport),
    Compartment(CompartmentDecl),
    Class(ClassDecl),
    ExportFunction(ExportedFunction),
    TypeAlias(TypeAliasDecl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryQualifiedImport {
    pub source: String,
    pub imported_names: Vec<String>,
    pub boundary_clause: Option<BoundaryClause>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDecl {
    pub name: String,
    pub exported: bool,
    pub fields: Vec<ClassFieldDecl>,
    pub static_fields: Vec<ClassFieldDecl>,
    pub methods: Vec<ClassMethodDecl>,
    pub static_methods: Vec<ClassMethodDecl>,
    pub constructor: Option<ClassConstructorDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassFieldDecl {
    pub name: String,
    pub ty: TypeExpr,
    pub readonly: bool,
    pub optional: bool,
    pub initializer: Option<ExprEnvelope>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassConstructorDecl {
    pub params: Vec<FunctionParam>,
    pub body: FunctionBody,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassMethodDecl {
    pub name: String,
    pub params: Vec<FunctionParam>,
    pub return_type: Option<TypeExpr>,
    pub body: FunctionBody,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedFunction {
    pub name: String,

    pub exported: bool,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<FunctionParam>,
    pub return_type: Option<TypeExpr>,
    pub boundary_clause: Option<BoundaryClause>,
    pub body_span: Span,
    pub body: FunctionBody,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionBody {
    pub span: Span,
    pub statements: Vec<BodyStmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyStmt {
    LocalDecl(LocalDecl),
    Assignment(AssignmentStmt),
    Update(UpdateStmt),
    Return(ReturnStmt),
    IfGuard(IfGuardStmt),
    If(IfStmt),
    While(WhileStmt),
    For(ForStmt),
    DoWhile(DoWhileStmt),
    Switch(SwitchStmt),
    Unsupported(UnsupportedStmt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedStmt {
    pub spelling: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhileStmt {
    pub subject: ExprEnvelope,
    pub guard: NarrowingGuard,
    pub body: FunctionBody,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForStmt {
    pub initializer: Option<ForInitializer>,
    pub subject: ExprEnvelope,
    pub guard: NarrowingGuard,
    pub update: Option<ForUpdate>,
    pub body: FunctionBody,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoWhileStmt {
    pub subject: ExprEnvelope,
    pub guard: NarrowingGuard,
    pub body: FunctionBody,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForInitializer {
    LocalDecl(LocalDecl),
    Assignment(AssignmentStmt),
    Update(UpdateStmt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForUpdate {
    Assignment(AssignmentStmt),
    Update(UpdateStmt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDecl {
    pub name: String,
    pub mutable: bool,
    pub type_annotation: Option<TypeExpr>,
    pub initializer: Option<ExprEnvelope>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentStmt {
    pub target: AssignmentTarget,
    pub value: ExprEnvelope,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateStmt {
    pub target: UpdateTarget,
    pub op: UpdateOp,
    pub position: UpdatePosition,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOp {
    Increment,
    Decrement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdatePosition {
    Prefix,
    Postfix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateTarget {
    Local(String),
    Property {
        object: ExprEnvelope,
        property: String,
        optional: bool,
    },
    Element {
        object: ExprEnvelope,
        index: ExprEnvelope,
        optional: bool,
    },
    Unsupported(ExprEnvelope),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignmentTarget {
    Local(String),
    Property {
        object: ExprEnvelope,
        property: String,
        optional: bool,
    },
    Element {
        object: ExprEnvelope,
        index: ExprEnvelope,
        optional: bool,
    },
    Unsupported(ExprEnvelope),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnStmt {
    pub argument: Option<ExprEnvelope>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfGuardStmt {
    pub subject: ExprEnvelope,
    pub guard: NarrowingGuard,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfStmt {
    pub subject: ExprEnvelope,
    pub guard: NarrowingGuard,
    pub then_body: FunctionBody,
    pub else_body: Option<FunctionBody>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchStmt {
    pub subject: ExprEnvelope,
    pub cases: Vec<SwitchCase>,
    pub default: Option<FunctionBody>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchCase {
    pub test: ExprEnvelope,
    pub body: FunctionBody,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NarrowingGuard {
    TypeofEquals { expected: String, span: Span },
    Instanceof { constructor: String, span: Span },
    OwnProperty { property: String, span: Span },
    DynamicOwnProperty { key: Box<ExprEnvelope>, span: Span },
    Truthy { span: Span },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExprEnvelope {
    pub kind: ExprEnvelopeKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprEnvelopeKind {
    Identifier(String),
    StringLiteral(String),
    NumberLiteral(String),
    BooleanLiteral(bool),
    NullLiteral,
    ArrayLiteral(Vec<ExprEnvelope>),
    ObjectLiteral(Vec<ObjectLiteralProperty>),
    PropertyAccess {
        object: Box<ExprEnvelope>,
        property: String,
        optional: bool,
    },
    ElementAccess {
        object: Box<ExprEnvelope>,
        index: Box<ExprEnvelope>,
        optional: bool,
    },
    Call {
        callee: Box<ExprEnvelope>,
        args: Vec<ExprEnvelope>,
        optional: bool,
    },
    New {
        class_name: String,
        args: Vec<ExprEnvelope>,
    },
    BinaryAdd {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinarySub {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinaryMul {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinaryPow {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinaryDiv {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinaryRem {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    NullishCoalesce {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinaryBitwiseAnd {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinaryBitwiseOr {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinaryBitwiseXor {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinaryShiftLeft {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinaryShiftRight {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinaryLessThan {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinaryGreaterThan {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinaryLessThanOrEqual {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinaryGreaterThanOrEqual {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinaryStrictEqual {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    BinaryStrictNotEqual {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    LogicalAnd {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    LogicalOr {
        left: Box<ExprEnvelope>,
        right: Box<ExprEnvelope>,
    },
    LogicalNot {
        expr: Box<ExprEnvelope>,
    },
    UnaryMinus {
        expr: Box<ExprEnvelope>,
    },
    Update {
        target: Box<ExprEnvelope>,
        op: UpdateOp,
        position: UpdatePosition,
    },
    Delete {
        target: Box<ExprEnvelope>,
    },
    Conditional {
        subject: Box<ExprEnvelope>,
        guard: NarrowingGuard,
        consequent: Box<ExprEnvelope>,
        alternate: Box<ExprEnvelope>,
    },

    Ternary {
        condition: Box<ExprEnvelope>,
        consequent: Box<ExprEnvelope>,
        alternate: Box<ExprEnvelope>,
    },

    LetIn {
        name: String,
        value: Box<ExprEnvelope>,
        body: Box<ExprEnvelope>,
    },
    Assertion {
        expr: Box<ExprEnvelope>,
        target: TypeExpr,

        satisfies: bool,
    },
    NonNull {
        expr: Box<ExprEnvelope>,
    },

    Arrow {
        params: Vec<FunctionParam>,
        body: Box<ExprEnvelope>,
    },
    Opaque(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectLiteralProperty {
    pub name: String,
    pub value: ExprEnvelope,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAliasDecl {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub aliased_type: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParam {
    pub name: String,
    pub constraint: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionParam {
    pub name: String,
    pub ty: TypeExpr,
    pub default: Option<ExprEnvelope>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionTypeParam {
    pub name: Option<String>,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeExpr {
    pub kind: TypeExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeExprKind {
    Named(String),
    TypeRef {
        name: String,
        type_args: Vec<TypeExpr>,
    },
    Infer {
        name: String,
        constraint: Option<Box<TypeExpr>>,
    },
    Union(Vec<TypeExpr>),
    Function {
        type_params: Vec<TypeParam>,
        params: Vec<FunctionTypeParam>,
        return_type: Box<TypeExpr>,
        boundary_clause: Option<BoundaryClause>,
    },
    Constructor {
        params: Vec<FunctionTypeParam>,
        instance_type: Box<TypeExpr>,
        is_abstract: bool,
    },
    Tuple(Vec<TypeExpr>),
    Conditional {
        check: Box<TypeExpr>,
        extends: Box<TypeExpr>,
        true_type: Box<TypeExpr>,
        false_type: Box<TypeExpr>,
    },
    Mapped(MappedTypeExpr),
    Object(ObjectTypeExpr),
    IndexedAccess {
        object: Box<TypeExpr>,
        index: Box<TypeExpr>,
    },
    Opaque(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectTypeExpr {
    pub properties: Vec<ObjectTypeProperty>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectTypeProperty {
    pub name: String,
    pub ty: TypeExpr,
    pub readonly: bool,
    pub optional: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedTypeExpr {
    pub key_name: String,
    pub key_constraint: MappedKeyConstraint,
    pub value_type: Box<TypeExpr>,
    pub readonly_modifier: MappedModifier,
    pub optional_modifier: MappedModifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappedKeyConstraint {
    Keyof { target: Box<TypeExpr> },
    Type(Box<TypeExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappedModifier {
    Inherit,
    Add,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryClause {
    pub span: Span,
    pub site: BoundarySite,
    pub directives: Vec<BoundaryDirective>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundarySite {
    Import,
    FunctionDeclaration,
    FunctionType,
    Compartment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryDirective {
    Policy(String),
    DefaultShorthand,
    WeakenTo(String),
    Override(String),
    SkipReturnValidation,
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCode {
    UnexpectedToken,
    UnsupportedClassDeclaration,
    UnsupportedDecorator,
    SkipReturnValidationOnImport,
    SkipReturnValidationNonExport,
    MalformedTypeExpression,
    MalformedBoundaryDirective,
}

pub fn parse_source_unit(src: &str) -> CruftScriptSourceUnit {
    let mut parser = Parser {
        src,
        diagnostics: Vec::new(),
    };
    parser.parse()
}

struct Parser<'a> {
    src: &'a str,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
    fn parse(&mut self) -> CruftScriptSourceUnit {
        let mut items = Vec::new();
        if let Some(default) = self.parse_boundary_default() {
            items.push(SourceItem::BoundaryDefault(default));
        }
        let mut cursor = 0usize;
        while cursor < self.src.len() {
            let rest = &self.src[cursor..];
            if rest.starts_with(char::is_whitespace) {
                cursor += rest.chars().next().map(char::len_utf8).unwrap_or(1);
                continue;
            }

            if rest.starts_with("boundary default") {
                cursor = next_line_or_char(self.src, cursor);
                continue;
            }

            if rest.starts_with("boundary ") {
                if let Some(policy) = self.parse_boundary_policy_at(cursor) {
                    cursor = policy.span.end;
                    items.push(SourceItem::BoundaryPolicy(policy));
                    continue;
                }
            }

            if rest.starts_with("type ") {
                if let Some(type_alias) = self.parse_type_alias_at(self.src, 0, cursor) {
                    cursor = type_alias.span.end;
                    items.push(SourceItem::TypeAlias(type_alias));
                    continue;
                }
            }

            if rest.starts_with("compartment ") {
                if let Some(compartment) = self.parse_compartment_at(cursor) {
                    cursor = compartment.span.end;
                    items.push(SourceItem::Compartment(compartment));
                    continue;
                }
            }

            if rest.starts_with("export function ") {
                if let Some(function) = self.parse_export_function_at(self.src, 0, cursor) {
                    cursor = function.span.end;
                    push_top_level_export_function(&mut items, function);
                    continue;
                }
            }

            if starts_with_class_declaration(rest) {
                let end = next_line_or_char(self.src, cursor);
                self.diagnostics.push(Diagnostic {
                    code: DiagnosticCode::UnsupportedClassDeclaration,
                    span: Span::new(cursor, end),
                    message:
                        "CruftScript class declarations must appear inside an explicit compartment"
                            .to_string(),
                });
                cursor = end;
                continue;
            }

            if rest.starts_with('@') {
                let end = next_line_or_char(self.src, cursor);
                self.diagnostics.push(Diagnostic {
                    code: DiagnosticCode::UnsupportedDecorator,
                    span: Span::new(cursor, end),
                    message: "CruftScript decorators are runtime-bearing and are not derived yet"
                        .to_string(),
                });
                cursor = end;
                continue;
            }

            cursor = next_line_or_char(self.src, cursor);
        }
        CruftScriptSourceUnit {
            source_kind: SourceKind::NativeCruftScript,
            span: Span::new(0, self.src.len()),
            items,
            diagnostics: self.diagnostics.clone(),
        }
    }

    fn parse_boundary_default(&self) -> Option<BoundaryDefaultDecl> {
        let start = self.src.find("boundary default")?;
        let line_end = line_end(self.src, start);
        let line = &self.src[start..line_end];
        let eq = line.find('=')?;
        let policy_name = line[eq + 1..].trim().to_string();
        if policy_name.is_empty() {
            return None;
        }
        Some(BoundaryDefaultDecl {
            policy_name,
            span: Span::new(start, line_end),
        })
    }

    fn parse_boundary_policy_at(&self, start: usize) -> Option<BoundaryPolicyDecl> {
        let name_start = start + "boundary ".len();
        let name_end = scan_ident_end(self.src, name_start);
        let name = self.src[name_start..name_end].trim().to_string();
        if name.is_empty() || name == "default" {
            return None;
        }
        let after_name = &self.src[name_end..];
        let eq_rel = after_name.find('=')?;
        let open = self.src[name_end + eq_rel..].find('{')? + name_end + eq_rel;
        let close = matching_close_brace(self.src, open)?;
        let body_span = Span::new(open + 1, close);
        let body = &self.src[body_span.start..body_span.end];
        Some(BoundaryPolicyDecl {
            name,
            span: Span::new(start, close + 1),
            body_span,
            has_process_clause: body.contains("at process"),
            has_call_clause: body.contains("at call"),
            process_mode: parse_boundary_policy_process_mode(body),
            sanitizer_defaults: parse_sanitizer_defaults(body, body_span.start),
        })
    }

    fn parse_compartment_at(&mut self, start: usize) -> Option<CompartmentDecl> {
        let name_start = start + "compartment ".len();
        let name_end = scan_ident_end(self.src, name_start);
        let name = self.src[name_start..name_end].trim().to_string();
        if name.is_empty() {
            return None;
        }
        let (open, endowments) = parse_compartment_header(self.src, 0, name_end)?;
        let close = matching_close_brace(self.src, open)?;
        let header = &self.src[start..open];
        let boundary_clause = parse_boundary_clause_in(
            header,
            start,
            BoundarySite::Compartment,
            &mut self.diagnostics,
        );
        let body_span = Span::new(open + 1, close);
        let body = &self.src[body_span.start..body_span.end];
        let body_items = self.parse_compartment_body_items(body, body_span.start);
        Some(CompartmentDecl {
            name,
            boundary_clause,
            endowments,
            span: Span::new(start, close + 1),
            body_span,
            body_items,
        })
    }

    fn parse_compartment_body_items(
        &mut self,
        body: &str,
        body_offset: usize,
    ) -> Vec<CompartmentItem> {
        let mut items = Vec::new();
        let mut cursor = 0usize;

        while cursor < body.len() {
            let rest = &body[cursor..];
            if rest.starts_with(char::is_whitespace) {
                cursor += rest.chars().next().map(char::len_utf8).unwrap_or(1);
                continue;
            }

            if rest.starts_with("import ") {
                if let Some(import) = self.parse_import_at(body, body_offset, cursor) {
                    cursor = import.span.end - body_offset;
                    items.push(CompartmentItem::Import(import));
                    continue;
                }
            }

            if rest.starts_with("type ") {
                if let Some(type_alias) = self.parse_type_alias_at(body, body_offset, cursor) {
                    cursor = type_alias.span.end - body_offset;
                    items.push(CompartmentItem::TypeAlias(type_alias));
                    continue;
                }
            }

            if rest.starts_with("compartment ") {
                if let Some(compartment) =
                    self.parse_nested_compartment_at(body, body_offset, cursor)
                {
                    cursor = compartment.span.end - body_offset;
                    items.push(CompartmentItem::Compartment(compartment));
                    continue;
                }
            }

            if rest.starts_with("export function ") {
                if let Some(function) = self.parse_export_function_at(body, body_offset, cursor) {
                    cursor = function.span.end - body_offset;
                    items.push(CompartmentItem::ExportFunction(function));
                    continue;
                }
            }

            if rest.starts_with("function ") {
                if let Some(function) = self.parse_export_function_at(body, body_offset, cursor) {
                    cursor = function.span.end - body_offset;
                    items.push(CompartmentItem::ExportFunction(function));
                    continue;
                }
            }

            if starts_with_class_declaration(rest) || starts_with_export_class_declaration(rest) {
                if let Some(class_decl) = self.parse_class_at(body, body_offset, cursor) {
                    cursor = class_decl.span.end - body_offset;
                    items.push(CompartmentItem::Class(class_decl));
                    continue;
                }
                let end =
                    class_decl_end(body, cursor).unwrap_or_else(|| next_line_or_char(body, cursor));
                let unsupported_class = &body[cursor..end];
                let has_member_decorator = unsupported_class
                    .lines()
                    .any(|line| line.trim_start().starts_with('@'));
                let header = unsupported_class
                    .find('{')
                    .map(|open| unsupported_class[..open].trim())
                    .unwrap_or_else(|| unsupported_class.lines().next().unwrap_or("").trim());
                let (code, message) = if has_member_decorator {
                    (
                        DiagnosticCode::UnsupportedDecorator,
                        "CruftScript decorators are runtime-bearing and are not derived yet"
                            .to_string(),
                    )
                } else if header.contains(" extends ") || header.contains(" implements ") {
                    (
                        DiagnosticCode::UnsupportedClassDeclaration,
                        "CruftScript heritage clauses require prototype, subclass allocation, inherited dispatch, and super receiver authority that are not derived yet"
                            .to_string(),
                    )
                } else {
                    (
                        DiagnosticCode::UnsupportedClassDeclaration,
                        "CruftScript bounded classes support only declared data fields in this rung"
                            .to_string(),
                    )
                };
                self.diagnostics.push(Diagnostic {
                    code,
                    span: Span::new(body_offset + cursor, body_offset + end),
                    message,
                });
                cursor = end;
                continue;
            }

            if rest.starts_with('@') {
                let end = next_line_or_char(body, cursor);
                self.diagnostics.push(Diagnostic {
                    code: DiagnosticCode::UnsupportedDecorator,
                    span: Span::new(body_offset + cursor, body_offset + end),
                    message: "CruftScript decorators are runtime-bearing and are not derived yet"
                        .to_string(),
                });
                cursor = end;
                continue;
            }

            cursor = next_line_or_char(body, cursor);
        }

        items
    }

    fn parse_nested_compartment_at(
        &mut self,
        body: &str,
        body_offset: usize,
        local_start: usize,
    ) -> Option<CompartmentDecl> {
        let start = body_offset + local_start;
        let name_start = local_start + "compartment ".len();
        let name_end = scan_ident_end(body, name_start);
        let name = body[name_start..name_end].trim().to_string();
        if name.is_empty() {
            return None;
        }
        let (open, endowments) = parse_compartment_header(body, body_offset, name_end)?;
        let close = matching_close_brace(body, open)?;
        let header = &body[local_start..open];
        let boundary_clause = parse_boundary_clause_in(
            header,
            start,
            BoundarySite::Compartment,
            &mut self.diagnostics,
        );
        let body_span = Span::new(body_offset + open + 1, body_offset + close);
        let nested_body = &body[open + 1..close];
        let body_items = self.parse_compartment_body_items(nested_body, body_span.start);
        Some(CompartmentDecl {
            name,
            boundary_clause,
            endowments,
            span: Span::new(start, body_offset + close + 1),
            body_span,
            body_items,
        })
    }

    fn parse_import_at(
        &mut self,
        body: &str,
        body_offset: usize,
        local_start: usize,
    ) -> Option<BoundaryQualifiedImport> {
        let start = body_offset + local_start;
        let local_end = line_end(body, local_start);
        let line = &body[local_start..local_end];
        let imported_names = between(line, "{", "}")
            .map(|names| {
                names
                    .split(',')
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let boundary_start = find_keyword_outside_strings(line, "boundary").unwrap_or(line.len());
        let source = line
            .find("from")
            .map(|from| &line[from + "from".len()..boundary_start])
            .map(|s| s.trim().trim_matches('"').to_string())
            .unwrap_or_default();
        let boundary_clause =
            parse_boundary_clause_in(line, start, BoundarySite::Import, &mut self.diagnostics);
        Some(BoundaryQualifiedImport {
            source,
            imported_names,
            boundary_clause,
            span: Span::new(start, body_offset + local_end),
        })
    }

    fn parse_type_alias_at(
        &mut self,
        text: &str,
        base_offset: usize,
        local_start: usize,
    ) -> Option<TypeAliasDecl> {
        let start = base_offset + local_start;
        let header_end = line_end(text, local_start);
        let line = &text[local_start..header_end];
        let eq = line.find('=')?;
        let value_start = local_start + eq + 1;
        let value_cursor = skip_ws(text, value_start, text.len());
        let (type_end, local_end) = if text[value_cursor..].starts_with('{') {
            let close = matching_close_brace(text, value_cursor)?;
            let after_close = skip_ws(text, close + 1, text.len());

            if text[after_close..].starts_with('&') || text[after_close..].starts_with('|') {
                let type_end =
                    find_top_level_char(text, value_cursor, header_end, ';').unwrap_or(header_end);
                (type_end, header_end)
            } else {
                let decl_end = if text[after_close..].starts_with(';') {
                    after_close + ';'.len_utf8()
                } else {
                    close + 1
                };
                (close + 1, decl_end)
            }
        } else {

            let type_end =
                find_top_level_char(text, value_cursor, header_end, ';').unwrap_or(header_end);
            (type_end, header_end)
        };
        let name_start = "type ".len();
        let name_end = scan_ident_end(line, name_start);
        let name = line[name_start..name_end].trim().to_string();
        if name.is_empty() {
            return None;
        }
        let type_params = if line[name_end..].trim_start().starts_with('<') {
            parse_type_params(line, name_end, start)
        } else {
            Vec::new()
        };
        let aliased_type = parse_type_expr(text, value_start, type_end, base_offset)?;
        Some(TypeAliasDecl {
            name,
            type_params,
            aliased_type,
            span: Span::new(start, base_offset + local_end),
        })
    }

    fn parse_class_at(
        &mut self,
        body: &str,
        body_offset: usize,
        local_start: usize,
    ) -> Option<ClassDecl> {
        let start = body_offset + local_start;
        let open = body[local_start..].find('{')? + local_start;
        let close = matching_close_brace(body, open)?;
        let header = body[local_start..open].trim();
        if header.contains(" extends ") {
            return None;
        }

        let (exported, name_start) = if let Some(rest) = header.strip_prefix("export") {
            let ws = header.len() - rest.trim_start().len();
            if ws == "export".len() || !rest.starts_with(char::is_whitespace) {
                return None;
            }
            (true, ws + "class ".len())
        } else {
            (false, "class ".len())
        };
        if !header[name_start.saturating_sub("class ".len())..].starts_with("class ") {
            return None;
        }
        let name_end = scan_ident_end(header, name_start);
        let name = header[name_start..name_end].trim().to_string();
        if name.is_empty() || !header[name_end..].trim().is_empty() {
            return None;
        }

        let mut fields = Vec::new();
        let mut static_fields = Vec::new();
        let mut methods = Vec::new();
        let mut static_methods = Vec::new();
        let mut constructor = None;
        let mut cursor = open + 1;
        while cursor < close {
            cursor = skip_class_body_ws(body, cursor, close);
            if cursor >= close {
                break;
            }
            let line_end = line_end(body, cursor).min(close);
            let raw = &body[cursor..line_end];
            let line = raw.trim();
            let line_start = cursor + raw.find(line).unwrap_or(0);
            let line_end_trimmed = line_start + line.len();
            if line.is_empty() {
                cursor = if line_end < close {
                    line_end + 1
                } else {
                    close
                };
                continue;
            }
            if line.starts_with("constructor") {
                if constructor.is_some() {
                    return None;
                }
                let (ctor, next_cursor) =
                    parse_class_constructor(body, body_offset, line_start, close)?;
                constructor = Some(ctor);
                cursor = next_cursor;
                continue;
            }
            if class_line_starts_static_method(line) || class_line_starts_plain_method(line) {
                let is_static = class_line_starts_static_method(line);
                let (method, next_cursor) =
                    parse_class_method(body, body_offset, line_start, close, is_static)?;
                if is_static {
                    static_methods.push(method);
                } else {
                    methods.push(method);
                }
                cursor = next_cursor;
                continue;
            }
            if let Some(rest) = line.strip_prefix("static ") {
                let trimmed = rest.trim_start();
                let trimmed_start = line_start + "static ".len() + (rest.len() - trimmed.len());
                let field = parse_class_field(
                    trimmed,
                    body_offset + trimmed_start,
                    trimmed_start,
                    line_end_trimmed,
                )?;
                static_fields.push(field);
                cursor = if line_end < close {
                    line_end + 1
                } else {
                    close
                };
                continue;
            }
            let field =
                parse_class_field(line, body_offset + line_start, line_start, line_end_trimmed)?;
            fields.push(field);
            cursor = if line_end < close {
                line_end + 1
            } else {
                close
            };
        }

        Some(ClassDecl {
            name,
            exported,
            fields,
            static_fields,
            methods,
            static_methods,
            constructor,
            span: Span::new(start, body_offset + close + 1),
        })
    }

    fn parse_export_function_at(
        &mut self,
        body: &str,
        body_offset: usize,
        local_start: usize,
    ) -> Option<ExportedFunction> {
        let start = body_offset + local_start;
        let brace = find_top_level_function_body_open(body, local_start, body.len())?;
        let signature = body[local_start..brace].trim();
        let (exported, name_start) = if signature.starts_with("export function ") {
            (true, "export function ".len())
        } else if signature.starts_with("function ") {
            (false, "function ".len())
        } else {
            return None;
        };
        let name_end = scan_ident_end(signature, name_start);
        let name = signature[name_start..name_end].to_string();

        let type_params = if signature[name_end..].trim_start().starts_with('<') {
            parse_type_params(signature, name_end, start)
        } else {
            Vec::new()
        };

        let params_start = signature.find('(')?;
        let params_end = find_matching(signature, params_start, '(', ')')?;
        let params = parse_params(signature, params_start, params_end, start);

        let return_type = signature[params_end + 1..].find(':').and_then(|colon_rel| {
            let ty_start = params_end + 1 + colon_rel + 1;
            let ty_end = signature[ty_start..]
                .find("boundary")
                .map(|idx| ty_start + idx)
                .unwrap_or(signature.len());
            parse_type_expr(signature, ty_start, ty_end, start)
        });

        let boundary_clause = parse_boundary_clause_in(
            signature,
            start,
            BoundarySite::FunctionDeclaration,
            &mut self.diagnostics,
        );

        if !exported {
            if let Some(clause) = &boundary_clause {
                if clause
                    .directives
                    .iter()
                    .any(|d| matches!(d, BoundaryDirective::SkipReturnValidation))
                {
                    self.diagnostics.push(Diagnostic {
                        code: DiagnosticCode::SkipReturnValidationNonExport,
                        span: clause.span,
                        message: "skip return validation is permitted only on exported functions"
                            .to_string(),
                    });
                }
            }
        }
        let close = find_matching(body, brace, '{', '}')?;
        let body_span = Span::new(body_offset + brace + 1, body_offset + close);
        let function_body = parse_function_body(&body[brace + 1..close], body_span.start);
        let end = body_offset + close + 1;

        Some(ExportedFunction {
            name,
            exported,
            type_params,
            params,
            return_type,
            boundary_clause,
            body_span,
            body: function_body,
            span: Span::new(start, end),
        })
    }
}

fn push_top_level_export_function(items: &mut Vec<SourceItem>, function: ExportedFunction) {
    const DEFAULT_COMPARTMENT: &str = "Default";
    if let Some(SourceItem::Compartment(compartment)) = items.iter_mut().find(|item| {
        matches!(
            item,
            SourceItem::Compartment(compartment) if compartment.name == DEFAULT_COMPARTMENT
                && compartment.boundary_clause.is_none()
                && compartment.endowments.is_empty()
        )
    }) {
        compartment.span = Span::new(
            compartment.span.start.min(function.span.start),
            compartment.span.end.max(function.span.end),
        );
        compartment.body_span = Span::new(
            compartment.body_span.start.min(function.span.start),
            compartment.body_span.end.max(function.span.end),
        );
        compartment
            .body_items
            .push(CompartmentItem::ExportFunction(function));
        return;
    }

    items.push(SourceItem::Compartment(CompartmentDecl {
        name: DEFAULT_COMPARTMENT.to_string(),
        boundary_clause: None,
        endowments: Vec::new(),
        span: function.span,
        body_span: function.span,
        body_items: vec![CompartmentItem::ExportFunction(function)],
    }));
}

fn parse_function_body(body: &str, body_offset: usize) -> FunctionBody {
    let lines = body_lines(body, body_offset);
    let (statements, _) = parse_body_statements(&lines, 0, false);
    FunctionBody {
        span: Span::new(body_offset, body_offset + body.len()),
        statements,
    }
}

fn skip_class_body_ws(src: &str, mut cursor: usize, end: usize) -> usize {
    while cursor < end {
        let Some(ch) = src[cursor..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        cursor += ch.len_utf8();
    }
    cursor
}

fn parse_class_constructor(
    src: &str,
    body_offset: usize,
    local_start: usize,
    class_close: usize,
) -> Option<(ClassConstructorDecl, usize)> {
    let open_paren = src[local_start..class_close].find('(')? + local_start;
    if !src[local_start..open_paren].trim().eq("constructor") {
        return None;
    }
    let close_paren = find_matching(src, open_paren, '(', ')')?;
    let after_params = skip_ws(src, close_paren + 1, class_close);
    if !src[after_params..].starts_with('{') {
        return None;
    }
    let open_brace = after_params;
    let close_brace = find_matching(src, open_brace, '{', '}')?;
    if close_brace > class_close {
        return None;
    }
    if !constructor_params_are_plain(src, open_paren + 1, close_paren) {
        return None;
    }
    let params = parse_params(src, open_paren, close_paren, body_offset);
    let body_span = Span::new(body_offset + open_brace + 1, body_offset + close_brace);
    let body = parse_function_body(&src[open_brace + 1..close_brace], body_span.start);
    Some((
        ClassConstructorDecl {
            params,
            body,
            span: Span::new(body_offset + local_start, body_offset + close_brace + 1),
        },
        close_brace + 1,
    ))
}

fn class_line_starts_plain_method(line: &str) -> bool {
    if line.starts_with('#')
        || line.starts_with("static ")
        || line.starts_with("get ")
        || line.starts_with("set ")
        || line.starts_with("constructor")
    {
        return false;
    }
    let Some(open) = line.find('(') else {
        return false;
    };
    let name = line[..open].trim();
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

fn class_line_starts_static_method(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("static ") else {
        return false;
    };
    if rest.starts_with('#')
        || rest.starts_with("get ")
        || rest.starts_with("set ")
        || rest.starts_with("accessor ")
        || rest.starts_with('[')
    {
        return false;
    }
    let Some(open) = rest.find('(') else {
        return false;
    };
    if rest
        .find([':', '='])
        .is_some_and(|field_marker| field_marker < open)
    {
        return false;
    }
    let name = rest[..open].trim();
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

fn parse_class_method(
    src: &str,
    body_offset: usize,
    local_start: usize,
    class_close: usize,
    is_static: bool,
) -> Option<(ClassMethodDecl, usize)> {
    let open_paren = src[local_start..class_close].find('(')? + local_start;
    let raw_name = src[local_start..open_paren].trim();
    let name = if is_static {
        raw_name.strip_prefix("static ")?.trim()
    } else {
        raw_name
    };
    if name.is_empty()
        || name == "constructor"
        || name.starts_with('#')
        || name.starts_with("get ")
        || name.starts_with("set ")
        || name.contains('[')
    {
        return None;
    }
    let close_paren = find_matching(src, open_paren, '(', ')')?;
    let after_params = skip_ws(src, close_paren + 1, class_close);
    let open_brace = if src[after_params..].starts_with(':') {
        let type_start = skip_ws(src, after_params + 1, class_close);
        let brace = src[type_start..class_close].find('{')? + type_start;
        if brace > class_close {
            return None;
        }
        brace
    } else {
        after_params
    };
    if !src[open_brace..].starts_with('{') {
        return None;
    }
    let close_brace = find_matching(src, open_brace, '{', '}')?;
    if close_brace > class_close {
        return None;
    }
    if !constructor_params_are_plain(src, open_paren + 1, close_paren) {
        return None;
    }
    let return_type = if src[after_params..].starts_with(':') {
        parse_type_expr(src, after_params + 1, open_brace, body_offset)
    } else {
        None
    };
    let params = parse_params(src, open_paren, close_paren, body_offset);
    let body_span = Span::new(body_offset + open_brace + 1, body_offset + close_brace);
    let body = parse_function_body(&src[open_brace + 1..close_brace], body_span.start);
    Some((
        ClassMethodDecl {
            name: name.to_string(),
            params,
            return_type,
            body,
            span: Span::new(body_offset + local_start, body_offset + close_brace + 1),
        },
        close_brace + 1,
    ))
}

fn constructor_params_are_plain(src: &str, start: usize, end: usize) -> bool {
    split_top_level(src, start, end, ',')
        .into_iter()
        .all(|(part_start, part_end)| {
            let raw = src[part_start..part_end].trim();
            if raw.is_empty() {
                return true;
            }
            let Some(colon) = find_top_level_char(src, part_start, part_end, ':') else {
                return false;
            };
            let name = src[part_start..colon].trim();
            is_ident(name)
                && !matches!(
                    name,
                    "readonly" | "public" | "private" | "protected" | "static"
                )
        })
}

#[derive(Debug, Clone, Copy)]
struct BodyLine<'a> {
    trimmed: &'a str,
    end: usize,
    stmt_start: usize,

    body: &'a str,
    local_start: usize,
    local_end: usize,
}

fn body_lines(body: &str, body_offset: usize) -> Vec<BodyLine<'_>> {
    let mut lines = Vec::new();
    let mut cursor = 0usize;

    for line in body.lines() {
        let line_start = cursor;
        let line_end = line_start + line.len();
        cursor = line_end + 1;

        let trimmed = line.trim();
        let leading_ws = line.find(trimmed).unwrap_or(0);
        lines.push(BodyLine {
            trimmed,
            end: body_offset + line_end,
            stmt_start: body_offset + line_start + leading_ws,
            body,
            local_start: line_start + leading_ws,
            local_end: line_end,
        });
    }

    lines
}

fn bracket_balance(s: &str) -> i32 {
    let mut depth = 0i32;
    let mut idx = 0usize;
    let mut operand = true;
    while idx < s.len() {
        if let Some(string_end) = string_literal_end(s, idx) {
            idx = string_end;
            operand = false;
            continue;
        }
        let ch = match s[idx..].chars().next() {
            Some(ch) => ch,
            None => break,
        };
        if ch == '/' && operand {
            if let Some(regex_end) = regex_literal_end(s, idx) {
                idx = regex_end;
                operand = false;
                continue;
            }
        }
        match ch {
            '(' | '[' | '{' => {
                depth += 1;
                operand = true;
            }
            ')' | ']' | '}' => {
                depth -= 1;
                operand = false;
            }
            c if c.is_whitespace() => {}
            c => {
                operand = !(c.is_alphanumeric() || c == '_');
            }
        }
        idx += ch.len_utf8();
    }
    depth
}

fn line_starts_block_construct(trimmed: &str) -> bool {
    const KEYWORDS: [&str; 9] = [
        "try", "catch", "finally", "if", "else", "while", "for", "switch", "do",
    ];
    for keyword in KEYWORDS {
        if let Some(rest) = trimmed.strip_prefix(keyword) {
            match rest.chars().next() {
                None => return true,
                Some(c) if !(c.is_alphanumeric() || c == '_') => return true,
                _ => {}
            }
        }
    }
    trimmed.starts_with('{') || trimmed.starts_with('}')
}

fn starts_with_keyword(text: &str, keyword: &str) -> bool {
    let Some(rest) = text.strip_prefix(keyword) else {
        return false;
    };
    match rest.chars().next() {
        None => true,
        Some(ch) => !(ch.is_alphanumeric() || ch == '_'),
    }
}

fn line_is_chain_continuation(trimmed: &str) -> bool {
    trimmed.starts_with('.') || trimmed.starts_with("?.")
}

fn has_unclosed_template_literal(s: &str) -> bool {
    let mut idx = 0usize;
    let mut in_template = false;
    let mut interpolation_depth = 0usize;
    while idx < s.len() {
        let Some(ch) = s[idx..].chars().next() else {
            break;
        };
        if ch == '\\' {
            idx += ch.len_utf8();
            if let Some(next) = s[idx..].chars().next() {
                idx += next.len_utf8();
            }
            continue;
        }
        if in_template {
            if interpolation_depth == 0 {
                if ch == '`' {
                    in_template = false;
                } else if ch == '$' && s[idx + ch.len_utf8()..].starts_with('{') {
                    interpolation_depth = 1;
                    idx += ch.len_utf8() + 1;
                    continue;
                }
            } else {
                match ch {
                    '{' => interpolation_depth += 1,
                    '}' => interpolation_depth = interpolation_depth.saturating_sub(1),
                    _ => {}
                }
            }
        } else if ch == '`' {
            in_template = true;
        }
        idx += ch.len_utf8();
    }
    in_template
}

fn merged_logical_statement<'a>(
    lines: &[BodyLine<'a>],
    cursor: usize,
) -> Option<(&'a str, usize, usize, usize)> {
    let first = lines[cursor];
    let mut k = cursor;
    loop {
        let last = lines[k];
        let slice = &first.body[first.local_start..last.local_end];
        let next_continues = lines
            .get(k + 1)
            .is_some_and(|next| line_is_chain_continuation(next.trimmed));
        if bracket_balance(slice) <= 0 && !has_unclosed_template_literal(slice) && !next_continues {
            return Some((slice.trim(), first.stmt_start, last.end, k + 1));
        }
        k += 1;
        if k >= lines.len() {
            let slice = &first.body[first.local_start..last.local_end];
            return Some((slice.trim(), first.stmt_start, last.end, k));
        }
    }
}

fn is_comment_only_line(trimmed: &str) -> bool {
    trimmed.starts_with("//")
        || (trimmed.starts_with("/*") && trimmed.ends_with("*/") && trimmed.len() >= 4)
}

fn parse_body_statements(
    lines: &[BodyLine<'_>],
    mut cursor: usize,
    stop_on_closing: bool,
) -> (Vec<BodyStmt>, usize) {
    let mut statements = Vec::new();

    while cursor < lines.len() {
        let line = lines[cursor];
        let trimmed = line.trimmed;
        if trimmed.is_empty() || trimmed == "}" || trimmed == "{" || is_comment_only_line(trimmed) {
            if trimmed == "}" && stop_on_closing {
                return (statements, cursor + 1);
            }
            cursor += 1;
            continue;
        }
        if trimmed.starts_with("} else") && stop_on_closing {
            return (statements, cursor);
        }

        if let Some((if_stmt, next_cursor)) = parse_if_stmt(lines, cursor) {
            statements.push(BodyStmt::If(if_stmt));
            cursor = next_cursor;
            continue;
        }
        if let Some((while_stmt, next_cursor)) = parse_while_stmt(lines, cursor) {
            statements.push(BodyStmt::While(while_stmt));
            cursor = next_cursor;
            continue;
        }
        if let Some((for_stmt, next_cursor)) = parse_for_stmt(lines, cursor) {
            statements.push(BodyStmt::For(for_stmt));
            cursor = next_cursor;
            continue;
        }
        if let Some((do_while_stmt, next_cursor)) = parse_do_while_stmt(lines, cursor) {
            statements.push(BodyStmt::DoWhile(do_while_stmt));
            cursor = next_cursor;
            continue;
        }
        if let Some((switch_stmt, next_cursor)) = parse_switch_stmt(lines, cursor) {
            statements.push(BodyStmt::Switch(switch_stmt));
            cursor = next_cursor;
            continue;
        }

        let next_is_chain = lines
            .get(cursor + 1)
            .is_some_and(|next| line_is_chain_continuation(next.trimmed));
        let (trimmed, stmt_start, stmt_end, advance) = if (bracket_balance(trimmed) > 0
            || has_unclosed_template_literal(trimmed)
            || next_is_chain)
            && !line_starts_block_construct(trimmed)
        {
            match merged_logical_statement(lines, cursor) {
                Some((merged, start, end, next)) => (merged, start, end, next),
                None => (trimmed, line.stmt_start, line.end, cursor + 1),
            }
        } else {
            (trimmed, line.stmt_start, line.end, cursor + 1)
        };

        if let Some(local) = parse_local_decl(trimmed, stmt_start) {
            statements.push(BodyStmt::LocalDecl(local));
            cursor = advance;
            continue;
        }
        if let Some(assignment) = parse_assignment_stmt(trimmed, stmt_start) {
            statements.push(BodyStmt::Assignment(assignment));
            cursor = advance;
            continue;
        }
        if let Some(ret) = parse_return_stmt(trimmed, stmt_start, stmt_end) {
            statements.push(BodyStmt::Return(ret));
            cursor = advance;
            continue;
        }
        if let Some(update) = parse_update_stmt(trimmed, stmt_start) {
            statements.push(BodyStmt::Update(update));
            cursor = advance;
            continue;
        }
        if let Some(guard) = parse_if_guard_stmt(trimmed, stmt_start, stmt_end) {
            statements.push(BodyStmt::IfGuard(guard));
            cursor = advance;
            continue;
        }
        statements.push(BodyStmt::Unsupported(UnsupportedStmt {
            spelling: trimmed.to_string(),
            span: Span::new(stmt_start, stmt_end),
        }));
        cursor = advance;
    }

    (statements, cursor)
}

fn parse_if_stmt(lines: &[BodyLine<'_>], cursor: usize) -> Option<(IfStmt, usize)> {
    let line = *lines.get(cursor)?;
    if !starts_with_keyword(line.trimmed, "if") {
        return None;
    }
    let guard = parse_if_guard_stmt(line.trimmed, line.stmt_start, line.end)?;

    if !line.trimmed.contains('{') {
        let close = find_matching(line.trimmed, line.trimmed.find('(')?, '(', ')')?;
        let tail = line.trimmed[close + 1..].trim_start();
        let tail_offset =
            line.stmt_start + close + 1 + line.trimmed[close + 1..].find(tail).unwrap_or(0);
        let return_stmt = parse_return_stmt(tail, tail_offset, line.end)?;
        let then_body = FunctionBody {
            span: Span::new(tail_offset, line.end),
            statements: vec![BodyStmt::Return(return_stmt)],
        };
        return Some((
            IfStmt {
                subject: guard.subject,
                guard: guard.guard,
                then_body,
                else_body: None,
                span: Span::new(line.stmt_start, line.end),
            },
            cursor + 1,
        ));
    }

    if let Some(open) = line.trimmed.find('{') {
        if let Some(close) = find_matching(line.trimmed, open, '{', '}') {
            let then_src = &line.trimmed[open + 1..close];
            let then_abs = line.stmt_start + open + 1;
            let (then_statements, _) =
                parse_body_statements(&body_lines(then_src, then_abs), 0, false);
            let then_body = FunctionBody {
                span: Span::new(line.stmt_start + open, then_abs + then_src.len()),
                statements: then_statements,
            };
            let mut else_body = None;
            let mut end = line.stmt_start + close + 1;
            let after = line.trimmed[close + 1..].trim_start();
            if after.starts_with("else") {
                if let Some(eopen_rel) = line.trimmed[close + 1..].find('{') {
                    let eopen = close + 1 + eopen_rel;
                    if let Some(eclose) = find_matching(line.trimmed, eopen, '{', '}') {
                        let else_src = &line.trimmed[eopen + 1..eclose];
                        let else_abs = line.stmt_start + eopen + 1;
                        let (else_statements, _) =
                            parse_body_statements(&body_lines(else_src, else_abs), 0, false);
                        else_body = Some(FunctionBody {
                            span: Span::new(line.stmt_start + eopen, else_abs + else_src.len()),
                            statements: else_statements,
                        });
                        end = line.stmt_start + eclose + 1;
                    }
                }
            }
            return Some((
                IfStmt {
                    subject: guard.subject,
                    guard: guard.guard,
                    then_body,
                    else_body,
                    span: Span::new(line.stmt_start, end),
                },
                cursor + 1,
            ));
        }
    }

    let (then_statements, mut next_cursor) = parse_body_statements(lines, cursor + 1, true);
    let then_end = then_statements
        .last()
        .map(body_stmt_span)
        .map(|span| span.end)
        .unwrap_or(line.end);
    let then_body = FunctionBody {
        span: Span::new(line.end, then_end),
        statements: then_statements,
    };

    let mut else_body = None;
    if let Some(else_line) = lines.get(next_cursor).copied() {
        if else_line.trimmed.starts_with("} else") || else_line.trimmed.starts_with("else") {
            let (else_statements, after_else) = parse_body_statements(lines, next_cursor + 1, true);
            let else_end = else_statements
                .last()
                .map(body_stmt_span)
                .map(|span| span.end)
                .unwrap_or(else_line.end);
            else_body = Some(FunctionBody {
                span: Span::new(else_line.end, else_end),
                statements: else_statements,
            });
            next_cursor = after_else;
        }
    }

    let end = else_body
        .as_ref()
        .map(|body| body.span.end)
        .unwrap_or(then_body.span.end);
    Some((
        IfStmt {
            subject: guard.subject,
            guard: guard.guard,
            then_body,
            else_body,
            span: Span::new(line.stmt_start, end),
        },
        next_cursor,
    ))
}

fn parse_while_stmt(lines: &[BodyLine<'_>], cursor: usize) -> Option<(WhileStmt, usize)> {
    let line = *lines.get(cursor)?;
    if !line.trimmed.starts_with("while ") || !line.trimmed.contains('{') {
        return None;
    }
    let open = line.trimmed.find('(')?;
    let close = find_matching(line.trimmed, open, '(', ')')?;
    let condition = line.trimmed[open + 1..close].trim();
    let condition_offset =
        line.stmt_start + open + 1 + line.trimmed[open + 1..close].find(condition)?;
    let (subject, guard) = parse_narrowing_guard(condition, condition_offset)?;
    let (body_statements, next_cursor) = parse_body_statements(lines, cursor + 1, true);
    let body_end = body_statements
        .last()
        .map(body_stmt_span)
        .map(|span| span.end)
        .unwrap_or(line.end);
    let body = FunctionBody {
        span: Span::new(line.end, body_end),
        statements: body_statements,
    };
    Some((
        WhileStmt {
            subject,
            guard,
            span: Span::new(line.stmt_start, body.span.end),
            body,
        },
        next_cursor,
    ))
}

fn parse_for_stmt(lines: &[BodyLine<'_>], cursor: usize) -> Option<(ForStmt, usize)> {
    let line = *lines.get(cursor)?;
    if !line.trimmed.starts_with("for ") || !line.trimmed.contains('{') {
        return None;
    }
    let open = line.trimmed.find('(')?;
    let close = find_matching(line.trimmed, open, '(', ')')?;
    let parts = split_for_header_parts(line.trimmed, open + 1, close);
    if parts.len() != 3 {
        return None;
    }

    let initializer = {
        let (start, end) = parts[0];
        let clause = line.trimmed[start..end].trim();
        if clause.is_empty() {
            None
        } else {
            let offset = line.stmt_start + start + line.trimmed[start..end].find(clause)?;
            parse_local_decl(clause, offset)
                .map(ForInitializer::LocalDecl)
                .or_else(|| parse_assignment_stmt(clause, offset).map(ForInitializer::Assignment))
                .or_else(|| parse_update_stmt(clause, offset).map(ForInitializer::Update))
        }
    };

    let (condition_start, condition_end) = parts[1];
    let condition = line.trimmed[condition_start..condition_end].trim();
    if condition.is_empty() {
        return None;
    }
    let condition_offset = line.stmt_start
        + condition_start
        + line.trimmed[condition_start..condition_end].find(condition)?;
    let (subject, guard) = parse_narrowing_guard(condition, condition_offset)?;

    let update = {
        let (start, end) = parts[2];
        let clause = line.trimmed[start..end].trim();
        if clause.is_empty() {
            None
        } else {
            let offset = line.stmt_start + start + line.trimmed[start..end].find(clause)?;
            parse_assignment_stmt(clause, offset)
                .map(ForUpdate::Assignment)
                .or_else(|| parse_update_stmt(clause, offset).map(ForUpdate::Update))
        }
    };

    let (body_statements, next_cursor) = parse_body_statements(lines, cursor + 1, true);
    let body_end = body_statements
        .last()
        .map(body_stmt_span)
        .map(|span| span.end)
        .unwrap_or(line.end);
    let body = FunctionBody {
        span: Span::new(line.end, body_end),
        statements: body_statements,
    };
    Some((
        ForStmt {
            initializer,
            subject,
            guard,
            update,
            span: Span::new(line.stmt_start, body.span.end),
            body,
        },
        next_cursor,
    ))
}

fn split_for_header_parts(src: &str, start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut parts = Vec::new();
    let mut part_start = start;
    let mut idx = start;
    while idx < end {
        let ch = match src[idx..end].chars().next() {
            Some(ch) => ch,
            None => break,
        };
        if ch == '(' || ch == '[' || ch == '{' {
            let close_ch = match ch {
                '(' => ')',
                '[' => ']',
                '{' => '}',
                _ => unreachable!(),
            };
            let Some(close) = find_matching(src, idx, ch, close_ch) else {
                break;
            };
            idx = close + 1;
            continue;
        }
        if ch == ';' {
            parts.push((part_start, idx));
            part_start = idx + ch.len_utf8();
        }
        idx += ch.len_utf8();
    }
    parts.push((part_start, end));
    parts
}

fn parse_do_while_stmt(lines: &[BodyLine<'_>], cursor: usize) -> Option<(DoWhileStmt, usize)> {
    let line = *lines.get(cursor)?;
    if line.trimmed != "do {" {
        return None;
    }
    let mut close_cursor = cursor + 1;
    while close_cursor < lines.len() {
        if lines[close_cursor].trimmed.starts_with("} while ") {
            break;
        }
        close_cursor += 1;
    }
    let close_line = *lines.get(close_cursor)?;
    let (body_statements, _) = parse_body_statements(&lines[cursor + 1..close_cursor], 0, false);
    let body_end = body_statements
        .last()
        .map(body_stmt_span)
        .map(|span| span.end)
        .unwrap_or(line.end);
    let body = FunctionBody {
        span: Span::new(line.end, body_end),
        statements: body_statements,
    };
    let open = close_line.trimmed.find('(')?;
    let close = find_matching(close_line.trimmed, open, '(', ')')?;
    let condition = close_line.trimmed[open + 1..close].trim();
    let condition_offset =
        close_line.stmt_start + open + 1 + close_line.trimmed[open + 1..close].find(condition)?;
    let (subject, guard) = parse_narrowing_guard(condition, condition_offset)?;
    Some((
        DoWhileStmt {
            subject,
            guard,
            body,
            span: Span::new(line.stmt_start, close_line.end),
        },
        close_cursor + 1,
    ))
}

fn parse_switch_stmt(lines: &[BodyLine<'_>], cursor: usize) -> Option<(SwitchStmt, usize)> {
    let header = *lines.get(cursor)?;
    if !header.trimmed.starts_with("switch ") || !header.trimmed.contains('{') {
        return None;
    }
    let open = header.trimmed.find('(')?;
    let close = find_matching(header.trimmed, open, '(', ')')?;
    let subject = parse_expr_envelope(header.trimmed, open + 1, close, header.stmt_start)?;
    if !header.trimmed[close + 1..].trim_start().starts_with('{') {
        return None;
    }

    let mut cases = Vec::new();
    let mut default = None;
    let mut next_cursor = cursor + 1;
    let mut end = header.end;
    while next_cursor < lines.len() {
        let line = lines[next_cursor];
        let trimmed = line.trimmed;
        if trimmed == "}" {
            end = line.end;
            next_cursor += 1;
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("case ") {
            let colon = rest.rfind(':')?;
            let test_start = trimmed.find(rest)?;
            let test =
                parse_expr_envelope(trimmed, test_start, test_start + colon, line.stmt_start)?;
            let inline_body = rest[colon + 1..].trim();
            if !inline_body.is_empty() && inline_body != "{" {
                let tail = &rest[colon + 1..];
                let body_start = line.stmt_start
                    + test_start
                    + colon
                    + 1
                    + (tail.len() - tail.trim_start().len());
                let (body, consumed_to_close) =
                    switch_inline_case_body(inline_body, body_start, line.end)?;
                let span_end = body
                    .statements
                    .last()
                    .map(body_stmt_span)
                    .map(|span| span.end)
                    .unwrap_or(line.end);
                cases.push(SwitchCase {
                    test,
                    body,
                    span: Span::new(line.stmt_start, span_end),
                });
                if consumed_to_close {
                    end = line.end;
                    next_cursor += 1;
                    break;
                }
                next_cursor += 1;
                continue;
            }

            let (body_start, body_end, advance) = if rest[colon + 1..].trim() == "{" {
                switch_case_block_extent(lines, next_cursor)?
            } else {
                let body_start = next_cursor + 1;
                let mut body_end = body_start;
                while body_end < lines.len()
                    && lines[body_end].trimmed != "}"
                    && !lines[body_end].trimmed.starts_with("case ")
                    && !lines[body_end].trimmed.starts_with("default:")
                {
                    body_end += 1;
                }
                (body_start, body_end, body_end)
            };
            let body = switch_case_body(lines, body_start, body_end, line.end);
            let span_end = body
                .statements
                .last()
                .map(body_stmt_span)
                .map(|span| span.end)
                .unwrap_or(line.end);
            cases.push(SwitchCase {
                test,
                body,
                span: Span::new(line.stmt_start, span_end),
            });
            next_cursor = advance;
            continue;
        }
        if trimmed.starts_with("default:") {
            let after_label = trimmed["default:".len()..].trim();
            if !after_label.is_empty() && after_label != "{" {
                let body_start =
                    line.stmt_start + "default:".len() + trimmed["default:".len()..].len()
                        - trimmed["default:".len()..].trim_start().len();
                let (body, consumed_to_close) =
                    switch_inline_case_body(after_label, body_start, line.end)?;
                default = Some(body);
                if consumed_to_close {
                    end = line.end;
                    next_cursor += 1;
                    break;
                }
                next_cursor += 1;
                continue;
            }
            let (body_start, body_end, advance) = if after_label == "{" {
                switch_case_block_extent(lines, next_cursor)?
            } else {
                let body_start = next_cursor + 1;
                let mut body_end = body_start;
                while body_end < lines.len()
                    && lines[body_end].trimmed != "}"
                    && !lines[body_end].trimmed.starts_with("case ")
                    && !lines[body_end].trimmed.starts_with("default:")
                {
                    body_end += 1;
                }
                (body_start, body_end, body_end)
            };
            default = Some(switch_case_body(lines, body_start, body_end, line.end));
            next_cursor = advance;
            continue;
        }
        return None;
    }

    Some((
        SwitchStmt {
            subject,
            cases,
            default,
            span: Span::new(header.stmt_start, end),
        },
        next_cursor,
    ))
}

fn switch_inline_case_body(
    raw: &str,
    base_offset: usize,
    label_end: usize,
) -> Option<(FunctionBody, bool)> {
    let (stmt_raw, consumed_to_close) = trim_inline_switch_tail(raw);
    let stmt = stmt_raw.trim();
    let return_expr = stmt.strip_prefix("return")?.trim();
    if return_expr.is_empty() {
        return None;
    }
    let local_start = stmt_raw.find(return_expr)?;
    let argument = parse_expr_envelope(stmt_raw, local_start, stmt_raw.len(), base_offset)?;
    let span = Span::new(base_offset, base_offset + stmt_raw.len());
    Some((
        FunctionBody {
            span: Span::new(label_end, span.end),
            statements: vec![BodyStmt::Return(ReturnStmt {
                argument: Some(argument),
                span,
            })],
        },
        consumed_to_close,
    ))
}

fn trim_inline_switch_tail(raw: &str) -> (&str, bool) {
    let mut depth = 0i32;
    let mut in_str: Option<char> = None;
    let mut escape = false;
    for (idx, ch) in raw.char_indices() {
        if let Some(q) = in_str {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == q {
                in_str = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' | '`' => in_str = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' => depth -= 1,
            '}' if depth == 0 => return (&raw[..idx], true),
            '}' => depth -= 1,
            'c' if depth == 0 && raw[idx..].starts_with("case ") => return (&raw[..idx], false),
            'd' if depth == 0 && raw[idx..].starts_with("default:") => return (&raw[..idx], false),
            _ => {}
        }
    }
    (raw, false)
}

fn switch_case_block_extent(
    lines: &[BodyLine<'_>],
    label_cursor: usize,
) -> Option<(usize, usize, usize)> {
    let mut depth = bracket_balance(lines.get(label_cursor)?.trimmed);
    if depth <= 0 {
        return None;
    }
    let body_start = label_cursor + 1;
    let mut i = body_start;
    while i < lines.len() {
        let balance = bracket_balance(lines[i].trimmed);
        if depth + balance <= 0 {

            return Some((body_start, i, i + 1));
        }
        depth += balance;
        i += 1;
    }
    None
}

fn switch_case_body(
    lines: &[BodyLine<'_>],
    start: usize,
    end: usize,
    label_end: usize,
) -> FunctionBody {
    let (statements, _) = parse_body_statements(&lines[start..end], 0, false);
    let body_end = statements
        .last()
        .map(body_stmt_span)
        .map(|span| span.end)
        .unwrap_or(label_end);
    FunctionBody {
        span: Span::new(label_end, body_end),
        statements,
    }
}

fn body_stmt_span(statement: &BodyStmt) -> Span {
    match statement {
        BodyStmt::LocalDecl(local) => local.span,
        BodyStmt::Assignment(assignment) => assignment.span,
        BodyStmt::Update(update) => update.span,
        BodyStmt::Return(ret) => ret.span,
        BodyStmt::IfGuard(guard) => guard.span,
        BodyStmt::If(if_stmt) => if_stmt.span,
        BodyStmt::While(while_stmt) => while_stmt.span,
        BodyStmt::For(for_stmt) => for_stmt.span,
        BodyStmt::DoWhile(do_while_stmt) => do_while_stmt.span,
        BodyStmt::Switch(switch_stmt) => switch_stmt.span,
        BodyStmt::Unsupported(stmt) => stmt.span,
    }
}

fn parse_local_decl(line: &str, base_offset: usize) -> Option<LocalDecl> {
    let keyword_len = if line.starts_with("const ") {
        "const ".len()
    } else if line.starts_with("let ") {
        "let ".len()
    } else {
        return None;
    };
    let rest = line[keyword_len..].trim_start();
    let rest_offset = base_offset + keyword_len + line[keyword_len..].find(rest).unwrap_or(0);
    let name_end = scan_ident_end(rest, 0);
    let name = rest[..name_end].trim().to_string();
    if name.is_empty() {
        return None;
    }
    let after_name = &rest[name_end..];
    let eq_rel = after_name.find('=');
    let annotation_end = eq_rel.unwrap_or(after_name.len());
    let type_annotation = after_name[..annotation_end]
        .find(':')
        .and_then(|colon_rel| {
            let type_start = rest_offset + name_end + colon_rel + 1;
            let type_end = eq_rel
                .map(|eq_rel| rest_offset + name_end + eq_rel)
                .unwrap_or(base_offset + line.len());
            parse_type_expr(
                line,
                type_start - base_offset,
                type_end - base_offset,
                base_offset,
            )
        });
    let initializer = line.find('=').and_then(|eq| {
        let init_start = eq + 1;
        parse_expr_envelope(
            line,
            init_start,
            trim_expression_statement_end(line, init_start, line.len()),
            base_offset,
        )
    });
    Some(LocalDecl {
        name,
        mutable: line.starts_with("let "),
        type_annotation,
        initializer,
        span: Span::new(base_offset, base_offset + line.len()),
    })
}

fn parse_assignment_stmt(line: &str, base_offset: usize) -> Option<AssignmentStmt> {
    if let Some(assignment) = parse_local_compound_add_assignment_stmt(line, base_offset) {
        return Some(assignment);
    }
    let eq = find_plain_assignment_operator(line)?;
    let target_expr = parse_expr_envelope(line, 0, eq, base_offset)?;
    let target = match target_expr.kind {
        ExprEnvelopeKind::Identifier(name) => AssignmentTarget::Local(name),
        ExprEnvelopeKind::PropertyAccess {
            object,
            property,
            optional,
        } => AssignmentTarget::Property {
            object: *object,
            property,
            optional,
        },
        ExprEnvelopeKind::ElementAccess {
            object,
            index,
            optional,
        } => AssignmentTarget::Element {
            object: *object,
            index: *index,
            optional,
        },
        _ => AssignmentTarget::Unsupported(target_expr),
    };
    if matches!(
        target,
        AssignmentTarget::Unsupported(ExprEnvelope {
            kind: ExprEnvelopeKind::Opaque(_),
            ..
        })
    ) {
        return None;
    }
    let value_start = eq + 1;
    let value = parse_expr_envelope(line, value_start, line.len(), base_offset)?;
    Some(AssignmentStmt {
        target,
        value,
        span: Span::new(base_offset, base_offset + line.len()),
    })
}

fn parse_local_compound_add_assignment_stmt(
    line: &str,
    base_offset: usize,
) -> Option<AssignmentStmt> {
    let op = find_top_level_compound_add_assignment_operator(line)?;
    let target_raw = line[..op].trim();
    if !is_ident(target_raw) {
        return None;
    }
    let value_start = op + 2;
    let value = parse_expr_envelope(
        line,
        value_start,
        trim_expression_statement_end(line, value_start, line.len()),
        base_offset,
    )?;
    let target_expr = ExprEnvelope {
        kind: ExprEnvelopeKind::Identifier(target_raw.to_string()),
        span: Span::new(
            base_offset + line[..op].find(target_raw)?,
            base_offset + line[..op].find(target_raw)? + target_raw.len(),
        ),
    };
    Some(AssignmentStmt {
        target: AssignmentTarget::Local(target_raw.to_string()),
        value: ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryAdd {
                left: Box::new(target_expr),
                right: Box::new(value),
            },
            span: Span::new(base_offset, base_offset + line.len()),
        },
        span: Span::new(base_offset, base_offset + line.len()),
    })
}

fn find_top_level_compound_add_assignment_operator(spelling: &str) -> Option<usize> {
    let bytes = spelling.as_bytes();
    let mut depth = 0i32;
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if string_literal_end(spelling, i).is_some() {
            i = string_literal_end(spelling, i)?;
            continue;
        }
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'+' if depth == 0 && bytes.get(i + 1) == Some(&b'=') => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

fn parse_update_stmt(line: &str, base_offset: usize) -> Option<UpdateStmt> {
    let expr = parse_expr_envelope(line, 0, line.len(), base_offset)?;
    let ExprEnvelopeKind::Update {
        target,
        op,
        position,
    } = expr.kind
    else {
        return None;
    };
    let target = match target.kind {
        ExprEnvelopeKind::Identifier(name) => UpdateTarget::Local(name),
        ExprEnvelopeKind::PropertyAccess {
            object,
            property,
            optional,
        } => UpdateTarget::Property {
            object: *object,
            property,
            optional,
        },
        ExprEnvelopeKind::ElementAccess {
            object,
            index,
            optional,
        } => UpdateTarget::Element {
            object: *object,
            index: *index,
            optional,
        },
        _ => UpdateTarget::Unsupported(*target),
    };
    Some(UpdateStmt {
        target,
        op,
        position,
        span: expr.span,
    })
}

fn find_plain_assignment_operator(spelling: &str) -> Option<usize> {
    let bytes = spelling.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'=' {
            continue;
        }
        let prev = index.checked_sub(1).and_then(|i| bytes.get(i)).copied();
        let next = bytes.get(index + 1).copied();
        if matches!(
            prev,
            Some(b'=' | b'!' | b'<' | b'>' | b'+' | b'-' | b'*' | b'/' | b'%' | b'?' | b'|' | b'&')
        ) || matches!(next, Some(b'=' | b'>'))
        {
            continue;
        }
        return Some(index);
    }
    None
}

fn parse_return_stmt(line: &str, base_offset: usize, stmt_end: usize) -> Option<ReturnStmt> {
    if line != "return" && !line.starts_with("return ") {
        return None;
    }
    let argument = if line.len() > "return".len() {
        parse_expr_envelope(
            line,
            "return".len(),
            trim_expression_statement_end(line, "return".len(), line.len()),
            base_offset,
        )
    } else {
        None
    };
    Some(ReturnStmt {
        argument,
        span: Span::new(base_offset, stmt_end),
    })
}

fn parse_if_guard_stmt(line: &str, base_offset: usize, stmt_end: usize) -> Option<IfGuardStmt> {
    if !starts_with_keyword(line, "if") {
        return None;
    }
    let open = line.find('(')?;
    let close = find_matching(line, open, '(', ')')?;
    let condition = line[open + 1..close].trim();
    let condition_offset = base_offset + open + 1 + line[open + 1..close].find(condition)?;
    parse_narrowing_guard(condition, condition_offset).map(|(subject, guard)| IfGuardStmt {
        subject,
        guard,
        span: Span::new(base_offset, stmt_end),
    })
}

fn parse_narrowing_guard(
    condition: &str,
    base_offset: usize,
) -> Option<(ExprEnvelope, NarrowingGuard)> {
    if let Some(rest) = condition.strip_prefix("typeof ") {
        let (subject_name, expected) = rest.split_once("===").or_else(|| rest.split_once("=="))?;
        let subject_name = subject_name.trim();
        let expected_raw = expected.trim();
        let expected = parse_string_literal(expected_raw)?;
        let subject_start = condition.find(subject_name)?;
        let subject = parse_expr_envelope(
            condition,
            subject_start,
            subject_start + subject_name.len(),
            base_offset,
        )?;
        let guard_span = Span::new(base_offset, base_offset + condition.len());
        return Some((
            subject,
            NarrowingGuard::TypeofEquals {
                expected,
                span: guard_span,
            },
        ));
    }

    if let Some((subject_name, constructor)) = condition.split_once(" instanceof ") {
        let subject_name = subject_name.trim();
        let constructor = constructor.trim();
        let subject_start = condition.find(subject_name)?;
        let subject = parse_expr_envelope(
            condition,
            subject_start,
            subject_start + subject_name.len(),
            base_offset,
        )?;
        return Some((
            subject,
            NarrowingGuard::Instanceof {
                constructor: constructor.to_string(),
                span: Span::new(base_offset, base_offset + condition.len()),
            },
        ));
    }

    if let Some((property_raw, subject_name)) = condition.split_once(" in ") {
        let property_raw = property_raw.trim();
        let subject_name = subject_name.trim();
        if !subject_name
            .chars()
            .all(|c| c == '_' || c.is_ascii_alphanumeric())
        {
            return None;
        }
        let subject_start = condition.find(subject_name)?;
        let guard_span = Span::new(base_offset, base_offset + condition.len());
        let property = parse_string_literal(property_raw);
        let key = if property.is_none()
            && property_raw
                .chars()
                .all(|c| c == '_' || c.is_ascii_alphanumeric())
        {
            let key_start = condition.find(property_raw)?;
            Some(ExprEnvelope {
                kind: ExprEnvelopeKind::Identifier(property_raw.to_string()),
                span: Span::new(
                    base_offset + key_start,
                    base_offset + key_start + property_raw.len(),
                ),
            })
        } else {
            None
        };
        return Some((
            ExprEnvelope {
                kind: ExprEnvelopeKind::Identifier(subject_name.to_string()),
                span: Span::new(
                    base_offset + subject_start,
                    base_offset + subject_start + subject_name.len(),
                ),
            },
            if let Some(property) = property {
                NarrowingGuard::OwnProperty {
                    property,
                    span: guard_span,
                }
            } else {
                NarrowingGuard::DynamicOwnProperty {
                    key: Box::new(key?),
                    span: guard_span,
                }
            },
        ));
    }

    if condition
        .chars()
        .all(|c| c == '_' || c.is_ascii_alphanumeric())
    {
        return Some((
            ExprEnvelope {
                kind: ExprEnvelopeKind::Identifier(condition.to_string()),
                span: Span::new(base_offset, base_offset + condition.len()),
            },
            NarrowingGuard::Truthy {
                span: Span::new(base_offset, base_offset + condition.len()),
            },
        ));
    }

    if let Some(call) = parse_expr_envelope(condition, 0, condition.len(), base_offset) {
        if matches!(
            call.kind,
            ExprEnvelopeKind::Call { .. }
                | ExprEnvelopeKind::BinaryStrictEqual { .. }
                | ExprEnvelopeKind::BinaryStrictNotEqual { .. }
                | ExprEnvelopeKind::BinaryLessThan { .. }
                | ExprEnvelopeKind::BinaryGreaterThan { .. }
                | ExprEnvelopeKind::BinaryLessThanOrEqual { .. }
                | ExprEnvelopeKind::BinaryGreaterThanOrEqual { .. }
                | ExprEnvelopeKind::LogicalAnd { .. }
                | ExprEnvelopeKind::LogicalOr { .. }
                | ExprEnvelopeKind::LogicalNot { .. }
        ) {
            return Some((
                call,
                NarrowingGuard::Truthy {
                    span: Span::new(base_offset, base_offset + condition.len()),
                },
            ));
        }
    }

    None
}

fn parse_template_literal(raw: &str, span: Span, base_offset: usize) -> Option<ExprEnvelope> {
    let bytes = raw.as_bytes();
    if bytes.first() != Some(&b'`') {
        return None;
    }

    let mut j = 1;
    let mut close = None;
    while j < bytes.len() {
        match bytes[j] {
            b'\\' => {
                j += 2;
                continue;
            }
            b'`' => {
                close = Some(j);
                break;
            }
            _ => j += 1,
        }
    }
    if close? != raw.len() - 1 {
        return None;
    }
    let inner = &raw[1..raw.len() - 1];
    let lit = |text: String| ExprEnvelope {
        kind: ExprEnvelopeKind::StringLiteral(text),
        span,
    };
    let mut parts: Vec<ExprEnvelope> = Vec::new();
    let mut buf = String::new();
    let mut i = 0usize;
    let ib = inner.as_bytes();
    while i < ib.len() {
        if ib[i] == b'\\' {
            if let Some(ch) = inner[i + 1..].chars().next() {
                push_cooked_escape(&mut buf, ch);
                i += 1 + ch.len_utf8();
            } else {
                i += 1;
            }
            continue;
        }
        if ib[i] == b'$' && ib.get(i + 1) == Some(&b'{') {
            parts.push(lit(std::mem::take(&mut buf)));
            let end = find_matching(inner, i + 1, '{', '}')?;
            let expr_src = &inner[i + 2..end];
            let expr = parse_expr_envelope(expr_src, 0, expr_src.len(), base_offset + i + 2)?;
            parts.push(expr);
            i = end + 1;
            continue;
        }
        let ch = inner[i..].chars().next()?;
        buf.push(ch);
        i += ch.len_utf8();
    }
    parts.push(lit(buf));

    let mut iter = parts.into_iter();
    let mut acc = iter.next()?;
    for part in iter {
        acc = ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryAdd {
                left: Box::new(acc),
                right: Box::new(part),
            },
            span,
        };
    }
    Some(acc)
}

fn push_cooked_escape(out: &mut String, escaped: char) {
    match escaped {
        'n' => out.push('\n'),
        'r' => out.push('\r'),
        't' => out.push('\t'),
        'b' => out.push('\u{0008}'),
        'f' => out.push('\u{000C}'),
        'v' => out.push('\u{000B}'),
        '0' => out.push('\0'),
        '\\' => out.push('\\'),
        '"' => out.push('"'),
        '\'' => out.push('\''),
        '`' => out.push('`'),
        '$' => out.push('$'),
        '\n' | '\r' => {}
        other => out.push(other),
    }
}

fn parse_block_arrow_body(body: &str, base_offset: usize) -> Option<ExprEnvelope> {
    if !body.starts_with('{') {
        return None;
    }
    let close = find_matching(body, 0, '{', '}')?;
    if close != body.len() - 1 {
        return None;
    }
    let statements: Vec<(usize, usize)> = split_block_statements(body, '{'.len_utf8(), close)
        .into_iter()
        .filter(|(s, e)| !body[*s..*e].trim().is_empty())
        .collect();
    if statements.is_empty() {
        return None;
    }
    fold_block_if_return_chain(body, &statements, 0, base_offset)
}

fn fold_block_if_return_chain(
    body: &str,
    statements: &[(usize, usize)],
    index: usize,
    base_offset: usize,
) -> Option<ExprEnvelope> {
    let (seg_start, seg_end) = statements[index];
    let is_last = index + 1 == statements.len();
    let span = Span::new(base_offset + seg_start, base_offset + seg_end);

    if let Some((name, (value_start, value_end))) =
        block_const_binding_ranges(body, seg_start, seg_end)
    {
        if is_last {
            return None;
        }
        let value = parse_expr_envelope(body, value_start, value_end, base_offset)?;
        let rest = fold_block_if_return_chain(body, statements, index + 1, base_offset)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::LetIn {
                name,
                value: Box::new(value),
                body: Box::new(rest),
            },
            span,
        });
    }

    if is_last {
        let (expr_start, expr_end) = block_return_expr_range(body, seg_start, seg_end)?;
        return parse_expr_envelope(body, expr_start, expr_end, base_offset);
    }
    let ((cond_start, cond_end), (expr_start, expr_end)) =
        block_if_return_ranges(body, seg_start, seg_end)?;
    let condition = parse_expr_envelope(body, cond_start, cond_end, base_offset)?;
    let consequent = parse_expr_envelope(body, expr_start, expr_end, base_offset)?;
    let alternate = fold_block_if_return_chain(body, statements, index + 1, base_offset)?;
    Some(ExprEnvelope {
        kind: ExprEnvelopeKind::Ternary {
            condition: Box::new(condition),
            consequent: Box::new(consequent),
            alternate: Box::new(alternate),
        },
        span,
    })
}

fn block_const_binding_ranges(
    body: &str,
    seg_start: usize,
    seg_end: usize,
) -> Option<(String, (usize, usize))> {
    let seg = &body[seg_start..seg_end];
    let lead = seg.len() - seg.trim_start().len();
    let stmt_start = seg_start + lead;
    let stmt = body[stmt_start..seg_end].trim_end();
    let stmt_end = stmt_start + stmt.len();
    let after_kw = stmt
        .strip_prefix("const ")
        .or_else(|| stmt.strip_prefix("let "))?;
    let decl_start = stmt_end - after_kw.len();
    let eq = find_assignment_eq(body, decl_start, stmt_end)?;

    let name_region = body[decl_start..eq].trim();
    let name = name_region.split(':').next()?.trim();
    if !is_ident(name) {
        return None;
    }
    let value_raw_start = eq + '='.len_utf8();
    let value_slice = &body[value_raw_start..stmt_end];
    let value_lead = value_slice.len() - value_slice.trim_start().len();
    let value_start = value_raw_start + value_lead;
    if value_start >= stmt_end {
        return None;
    }
    Some((name.to_string(), (value_start, stmt_end)))
}

fn find_assignment_eq(body: &str, start: usize, end: usize) -> Option<usize> {
    let mut idx = start;
    let mut depth = 0usize;
    let mut operand = true;
    let mut prev: Option<char> = None;
    while idx < end {
        if let Some(string_end) = string_literal_end(body, idx) {
            idx = string_end.min(end);
            operand = false;
            prev = Some('"');
            continue;
        }
        let ch = body[idx..end].chars().next()?;
        if ch == '/' && operand {
            if let Some(regex_end) = regex_literal_end(body, idx) {
                idx = regex_end.min(end);
                operand = false;
                prev = Some('/');
                continue;
            }
        }
        match ch {
            '(' | '[' | '{' => {
                depth += 1;
                operand = true;
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                operand = false;
            }
            '=' if depth == 0 => {
                let next = body[idx + 1..end].chars().next();
                let part_of_operator = next == Some('=')
                    || matches!(prev, Some('=') | Some('!') | Some('<') | Some('>'));
                if !part_of_operator {
                    return Some(idx);
                }
            }
            _ => {}
        }
        if !ch.is_whitespace() {
            operand = !(ch.is_alphanumeric() || ch == '_');
        }
        prev = Some(ch);
        idx += ch.len_utf8();
    }
    None
}

fn block_return_expr_range(body: &str, seg_start: usize, seg_end: usize) -> Option<(usize, usize)> {
    let seg = &body[seg_start..seg_end];
    let lead = seg.len() - seg.trim_start().len();
    let stmt_start = seg_start + lead;
    let stmt = body[stmt_start..seg_end].trim_end();
    let stmt_end = stmt_start + stmt.len();
    let after = stmt.strip_prefix("return")?;
    let first = after.chars().next()?;
    if first.is_alphanumeric() || first == '_' {
        return None;
    }
    let expr_lead = after.len() - after.trim_start().len();
    let expr_start = stmt_end - after.len() + expr_lead;
    if expr_start >= stmt_end {
        return None;
    }
    Some((
        expr_start,
        trim_expression_statement_end(body, expr_start, stmt_end),
    ))
}

fn trim_expression_statement_end(text: &str, start: usize, end: usize) -> usize {
    let trimmed = text[start..end].trim_end();
    let trimmed_end = start + trimmed.len();
    if trimmed.ends_with(';') {
        trimmed_end - 1
    } else {
        trimmed_end
    }
}

#[allow(clippy::type_complexity)]
fn block_if_return_ranges(
    body: &str,
    seg_start: usize,
    seg_end: usize,
) -> Option<((usize, usize), (usize, usize))> {
    let seg = &body[seg_start..seg_end];
    let lead = seg.len() - seg.trim_start().len();
    let stmt_start = seg_start + lead;
    let stmt = body[stmt_start..seg_end].trim_end();
    let stmt_end = stmt_start + stmt.len();
    let after_if = stmt.strip_prefix("if")?;
    let first = after_if.chars().next()?;
    if first.is_alphanumeric() || first == '_' {
        return None;
    }

    let after_if_trimmed = after_if.trim_start();
    if !after_if_trimmed.starts_with('(') {
        return None;
    }
    let paren_open = stmt_end - after_if_trimmed.len();
    let paren_close = find_matching(body, paren_open, '(', ')')?;
    let cond_start = paren_open + 1;
    let cond_end = paren_close;
    if cond_start >= cond_end {
        return None;
    }

    let rest_start = paren_close + '('.len_utf8();
    let (expr_start, expr_end) = block_return_expr_range(body, rest_start, stmt_end)?;
    Some(((cond_start, cond_end), (expr_start, expr_end)))
}

fn split_block_statements(body: &str, start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut parts = Vec::new();
    let mut part_start = start;
    let mut idx = start;
    let mut depth = 0usize;
    let mut operand = true;
    while idx < end {
        if let Some(string_end) = string_literal_end(body, idx) {
            idx = string_end.min(end);
            operand = false;
            continue;
        }
        let ch = match body[idx..end].chars().next() {
            Some(ch) => ch,
            None => break,
        };
        if ch == '/' && operand {
            if let Some(regex_end) = regex_literal_end(body, idx) {
                idx = regex_end.min(end);
                operand = false;
                continue;
            }
        }
        match ch {
            '(' | '[' | '{' => {
                depth += 1;
                operand = true;
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                operand = false;
            }
            ';' | '\n' if depth == 0 => {
                parts.push((part_start, idx));
                part_start = idx + ch.len_utf8();
                operand = true;
            }
            c if c.is_whitespace() => {}
            c => {
                operand = !(c.is_alphanumeric() || c == '_');
            }
        }
        idx += ch.len_utf8();
    }
    parts.push((part_start, end));
    parts
}

fn find_top_level_arrow(raw: &str) -> Option<usize> {
    let bytes = raw.as_bytes();
    let mut depth = 0i32;
    let mut i = 0usize;
    let mut in_str: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'"' | b'\'' | b'`' => in_str = Some(b),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'=' if depth == 0 && bytes.get(i + 1) == Some(&b'>') => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

fn parse_regex_literal(raw: &str) -> Option<(String, String)> {
    let bytes = raw.as_bytes();
    if bytes.first() != Some(&b'/') {
        return None;
    }
    if raw.starts_with("//") || raw.starts_with("/*") {
        return None;
    }
    let mut i = 1;
    let mut in_class = false;
    let mut closing = None;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i += 2;
                continue;
            }
            b'[' => in_class = true,
            b']' => in_class = false,
            b'/' if !in_class => {
                closing = Some(i);
                break;
            }
            _ => {}
        }
        i += 1;
    }
    let close = closing?;
    let pattern = &raw[1..close];
    if pattern.is_empty() {
        return None;
    }
    let flags = &raw[close + 1..];
    if !flags.bytes().all(|b| b.is_ascii_lowercase()) {
        return None;
    }
    Some((pattern.to_string(), flags.to_string()))
}

fn parse_expr_envelope(
    text: &str,
    start: usize,
    end: usize,
    base_offset: usize,
) -> Option<ExprEnvelope> {
    let raw = text[start..end].trim();
    if raw.is_empty() {
        return None;
    }
    let leading_ws = text[start..end].len() - text[start..end].trim_start().len();
    let trailing_ws = text[start..end].len() - text[start..end].trim_end().len();
    let expr_start = start + leading_ws;
    let expr_end = end - trailing_ws;
    let span = Span::new(base_offset + expr_start, base_offset + expr_end);

    if raw.starts_with('`') {
        if let Some(expr) = parse_template_literal(raw, span, base_offset + expr_start) {
            return Some(expr);
        }
    }

    if let Some((pattern, flags)) = parse_regex_literal(raw) {
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::New {
                class_name: "RegExp".to_string(),
                args: vec![
                    ExprEnvelope {
                        kind: ExprEnvelopeKind::StringLiteral(pattern),
                        span,
                    },
                    ExprEnvelope {
                        kind: ExprEnvelopeKind::StringLiteral(flags),
                        span,
                    },
                ],
            },
            span,
        });
    }

    if let Some(arrow) = find_top_level_arrow(raw) {
        let params_raw = raw[..arrow].trim();
        let body_raw = raw[arrow + 2..].trim();
        if params_raw.starts_with('(') {
            let close = find_matching(params_raw, 0, '(', ')')?;
            let return_annotation = params_raw[close + 1..].trim();
            let return_annotation_ok = match return_annotation.strip_prefix(':') {
                None => return_annotation.is_empty(),
                Some(ty) => {
                    let ty_start = params_raw.len() - ty.len();
                    !ty.trim().is_empty()
                        && parse_type_expr(
                            params_raw,
                            ty_start,
                            params_raw.len(),
                            base_offset + expr_start,
                        )
                        .is_some()
                }
            };
            if return_annotation_ok {
                let params = parse_params_opt(params_raw, 0, close, base_offset + expr_start, true);

                let body = if body_raw.starts_with('{') {
                    parse_block_arrow_body(body_raw, base_offset + arrow + 2)?
                } else {
                    parse_expr_envelope(body_raw, 0, body_raw.len(), base_offset + arrow + 2)?
                };
                return Some(ExprEnvelope {
                    kind: ExprEnvelopeKind::Arrow {
                        params,
                        body: Box::new(body),
                    },
                    span,
                });
            }
        }
    }

    if raw.starts_with('(') && raw.ends_with(')') {
        let close = find_matching(raw, 0, '(', ')')?;
        if close == raw.len() - 1 {
            return parse_expr_envelope(raw, 1, close, span.start);
        }
    }

    if let Some(value) = parse_string_literal(raw) {
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::StringLiteral(value),
            span,
        });
    }
    if is_decimal_number_spelling(raw) {
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::NumberLiteral(raw.to_string()),
            span,
        });
    }
    if raw == "true" || raw == "false" {
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BooleanLiteral(raw == "true"),
            span,
        });
    }
    if raw == "null" {
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::NullLiteral,
            span,
        });
    }
    if let Some(target_raw) = raw.strip_prefix("delete ") {
        let target_start = raw.len() - target_raw.len();
        let target = parse_expr_envelope(raw, target_start, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::Delete {
                target: Box::new(target),
            },
            span,
        });
    }
    if let Some((target_start, target_end, op, position)) = split_update_expression(raw) {
        let target = parse_expr_envelope(raw, target_start, target_end, span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::Update {
                target: Box::new(target),
                op,
                position,
            },
            span,
        });
    }
    if let Some((expr_start, expr_end, type_start, type_end)) = split_satisfies_operator(raw) {
        let expr = parse_expr_envelope(raw, expr_start, expr_end, span.start)?;
        let target = parse_type_expr(raw, type_start, type_end, span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::Assertion {
                expr: Box::new(expr),
                target,
                satisfies: true,
            },
            span,
        });
    }
    if let Some((expr_start, expr_end, type_start, type_end)) = split_as_assertion(raw) {
        let expr = parse_expr_envelope(raw, expr_start, expr_end, span.start)?;
        let target = parse_type_expr(raw, type_start, type_end, span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::Assertion {
                expr: Box::new(expr),
                target,
                satisfies: false,
            },
            span,
        });
    }
    if let Some((expr_start, expr_end)) = split_non_null_assertion(raw) {
        let expr = parse_expr_envelope(raw, expr_start, expr_end, span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::NonNull {
                expr: Box::new(expr),
            },
            span,
        });
    }
    if let Some(rest) = raw.strip_prefix('!') {
        if !rest.is_empty() && !rest.starts_with('=') {
            let expr = parse_expr_envelope(raw, 1, raw.len(), span.start)?;
            return Some(ExprEnvelope {
                kind: ExprEnvelopeKind::LogicalNot {
                    expr: Box::new(expr),
                },
                span,
            });
        }
    }
    if raw.starts_with('-') && raw.len() > 1 && !has_top_level_binary_operator_after_prefix(raw) {
        let expr = parse_expr_envelope(raw, 1, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::UnaryMinus {
                expr: Box::new(expr),
            },
            span,
        });
    }
    if raw.starts_with('[') && raw.ends_with(']') {
        let close = find_matching(raw, 0, '[', ']')?;
        if close == raw.len() - 1 {
            let mut spread_expr: Option<ExprEnvelope> = None;
            let mut pending_elements = Vec::new();
            for (element_start, element_end) in split_top_level(raw, 1, close, ',') {
                let element_raw = raw[element_start..element_end].trim();
                if element_raw.is_empty() {
                    continue;
                }
                if let Some(spread_raw) = element_raw.strip_prefix("...") {
                    let spread_start = element_end - spread_raw.len();
                    let spread_arg =
                        parse_expr_envelope(raw, spread_start, element_end, span.start)?;
                    let base = spread_expr.take().unwrap_or(ExprEnvelope {
                        kind: ExprEnvelopeKind::ArrayLiteral(std::mem::take(&mut pending_elements)),
                        span,
                    });
                    spread_expr = Some(ExprEnvelope {
                        kind: ExprEnvelopeKind::Call {
                            callee: Box::new(ExprEnvelope {
                                kind: ExprEnvelopeKind::PropertyAccess {
                                    object: Box::new(base),
                                    property: "concat".to_string(),
                                    optional: false,
                                },
                                span,
                            }),
                            args: vec![spread_arg],
                            optional: false,
                        },
                        span,
                    });
                } else {
                    pending_elements.push(parse_expr_envelope(
                        raw,
                        element_start,
                        element_end,
                        span.start,
                    )?);
                }
            }
            if let Some(base) = spread_expr {
                if pending_elements.is_empty() {
                    return Some(base);
                }
                return Some(ExprEnvelope {
                    kind: ExprEnvelopeKind::Call {
                        callee: Box::new(ExprEnvelope {
                            kind: ExprEnvelopeKind::PropertyAccess {
                                object: Box::new(base),
                                property: "concat".to_string(),
                                optional: false,
                            },
                            span,
                        }),
                        args: vec![ExprEnvelope {
                            kind: ExprEnvelopeKind::ArrayLiteral(pending_elements),
                            span,
                        }],
                        optional: false,
                    },
                    span,
                });
            }
            let elements = pending_elements;
            return Some(ExprEnvelope {
                kind: ExprEnvelopeKind::ArrayLiteral(elements),
                span,
            });
        }
    }
    if raw.starts_with('{') && raw.ends_with('}') {
        let close = find_matching(raw, 0, '{', '}')?;
        if close == raw.len() - 1 {
            let mut properties = Vec::new();
            for (prop_start, prop_end) in split_top_level(raw, 1, close, ',') {
                let colon = find_top_level_char(raw, prop_start, prop_end, ':')?;
                let name_raw = raw[prop_start..colon].trim();
                let name = parse_object_literal_key(name_raw)?;
                let key_start = raw[prop_start..colon].find(name_raw).unwrap_or(0) + prop_start;
                let value = parse_expr_envelope(raw, colon + 1, prop_end, span.start)?;
                properties.push(ObjectLiteralProperty {
                    name,
                    value,
                    span: Span::new(span.start + key_start, span.start + prop_end),
                });
            }
            return Some(ExprEnvelope {
                kind: ExprEnvelopeKind::ObjectLiteral(properties),
                span,
            });
        }
    }
    if let Some(nullish) = find_top_level_nullish_coalesce_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, nullish, span.start)?;
        let right = parse_expr_envelope(raw, nullish + 2, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::NullishCoalesce {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(question) = find_top_level_conditional_question(raw, 0, raw.len()) {
        let colon = find_conditional_colon(raw, question + 1, raw.len())?;
        let consequent = parse_expr_envelope(raw, question + 1, colon, span.start)?;
        let alternate = parse_expr_envelope(raw, colon + 1, raw.len(), span.start)?;

        if let Some((subject, guard)) = parse_narrowing_guard(raw[..question].trim(), span.start) {
            return Some(ExprEnvelope {
                kind: ExprEnvelopeKind::Conditional {
                    subject: Box::new(subject),
                    guard,
                    consequent: Box::new(consequent),
                    alternate: Box::new(alternate),
                },
                span,
            });
        }
        let condition = parse_expr_envelope(raw, 0, question, span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::Ternary {
                condition: Box::new(condition),
                consequent: Box::new(consequent),
                alternate: Box::new(alternate),
            },
            span,
        });
    }
    if let Some(plus) = find_top_level_add_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, plus, span.start)?;
        let right = parse_expr_envelope(raw, plus + 1, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryAdd {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(minus) = find_top_level_sub_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, minus, span.start)?;
        let right = parse_expr_envelope(raw, minus + 1, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinarySub {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(star) = find_top_level_mul_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, star, span.start)?;
        let right = parse_expr_envelope(raw, star + 1, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryMul {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(pow) = find_top_level_pow_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, pow, span.start)?;
        let right = parse_expr_envelope(raw, pow + 2, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryPow {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(slash) = find_top_level_div_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, slash, span.start)?;
        let right = parse_expr_envelope(raw, slash + 1, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryDiv {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(percent) = find_top_level_rem_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, percent, span.start)?;
        let right = parse_expr_envelope(raw, percent + 1, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryRem {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(bitwise_and) = find_top_level_bitwise_and_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, bitwise_and, span.start)?;
        let right = parse_expr_envelope(raw, bitwise_and + 1, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryBitwiseAnd {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(bitwise_or) = find_top_level_bitwise_or_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, bitwise_or, span.start)?;
        let right = parse_expr_envelope(raw, bitwise_or + 1, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryBitwiseOr {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(bitwise_xor) = find_top_level_bitwise_xor_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, bitwise_xor, span.start)?;
        let right = parse_expr_envelope(raw, bitwise_xor + 1, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryBitwiseXor {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(shift_left) = find_top_level_shift_left_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, shift_left, span.start)?;
        let right = parse_expr_envelope(raw, shift_left + 2, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryShiftLeft {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(shift_right) = find_top_level_shift_right_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, shift_right, span.start)?;
        let right = parse_expr_envelope(raw, shift_right + 2, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryShiftRight {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(less_than_or_equal) = find_top_level_less_than_or_equal_operator(raw, 0, raw.len())
    {
        let left = parse_expr_envelope(raw, 0, less_than_or_equal, span.start)?;
        let right = parse_expr_envelope(raw, less_than_or_equal + 2, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryLessThanOrEqual {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(less_than) = find_top_level_less_than_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, less_than, span.start)?;
        let right = parse_expr_envelope(raw, less_than + 1, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryLessThan {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(greater_than_or_equal) =
        find_top_level_greater_than_or_equal_operator(raw, 0, raw.len())
    {
        let left = parse_expr_envelope(raw, 0, greater_than_or_equal, span.start)?;
        let right = parse_expr_envelope(raw, greater_than_or_equal + 2, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryGreaterThanOrEqual {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(greater_than) = find_top_level_greater_than_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, greater_than, span.start)?;
        let right = parse_expr_envelope(raw, greater_than + 1, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryGreaterThan {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(strict_equal) = find_top_level_strict_equal_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, strict_equal, span.start)?;
        let right = parse_expr_envelope(raw, strict_equal + 3, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryStrictEqual {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(strict_not_equal) = find_top_level_strict_not_equal_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, strict_not_equal, span.start)?;
        let right = parse_expr_envelope(raw, strict_not_equal + 3, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryStrictNotEqual {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(logical_and) = find_top_level_logical_and_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, logical_and, span.start)?;
        let right = parse_expr_envelope(raw, logical_and + 2, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::LogicalAnd {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some(logical_or) = find_top_level_logical_or_operator(raw, 0, raw.len()) {
        let left = parse_expr_envelope(raw, 0, logical_or, span.start)?;
        let right = parse_expr_envelope(raw, logical_or + 2, raw.len(), span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::LogicalOr {
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        });
    }
    if let Some((class_name, args_start, args_end)) = split_new_expression(raw) {
        let args_inner = &raw[args_start..args_end];
        let args = split_args(args_inner)
            .into_iter()
            .filter_map(|(arg_start, arg_end)| {
                parse_expr_envelope(
                    raw,
                    args_start + arg_start,
                    args_start + arg_end,
                    span.start,
                )
            })
            .collect();
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::New { class_name, args },
            span,
        });
    }
    if let Some((callee_start, callee_end, args_start, args_end, optional)) = split_call(raw) {
        let callee = parse_expr_envelope(raw, callee_start, callee_end, span.start)?;
        let args_inner = &raw[args_start..args_end];
        let args = split_args(args_inner)
            .into_iter()
            .filter_map(|(arg_start, arg_end)| {
                parse_expr_envelope(
                    raw,
                    args_start + arg_start,
                    args_start + arg_end,
                    span.start,
                )
            })
            .collect();
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::Call {
                callee: Box::new(callee),
                args,
                optional,
            },
            span,
        });
    }
    if let Some((object_start, object_end, property_start, property_end, optional)) =
        split_property_access(raw)
    {
        let object = parse_expr_envelope(raw, object_start, object_end, span.start)?;
        let property = raw[property_start..property_end].to_string();
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::PropertyAccess {
                object: Box::new(object),
                property,
                optional,
            },
            span,
        });
    }
    if let Some((object_start, object_end, index_start, index_end, optional)) =
        split_element_access(raw)
    {
        let object = parse_expr_envelope(raw, object_start, object_end, span.start)?;
        let index = parse_expr_envelope(raw, index_start, index_end, span.start)?;
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::ElementAccess {
                object: Box::new(object),
                index: Box::new(index),
                optional,
            },
            span,
        });
    }
    if raw.chars().all(|c| c == '_' || c.is_ascii_alphanumeric()) {
        return Some(ExprEnvelope {
            kind: ExprEnvelopeKind::Identifier(raw.to_string()),
            span,
        });
    }
    Some(ExprEnvelope {
        kind: ExprEnvelopeKind::Opaque(raw.to_string()),
        span,
    })
}

fn split_new_expression(raw: &str) -> Option<(String, usize, usize)> {
    let rest = raw.strip_prefix("new ")?;
    let class_start = raw.len() - rest.len();
    let rest = rest.trim_start();
    let class_start = class_start + (raw[class_start..].len() - rest.len());
    let class_end = scan_ident_end(raw, class_start);
    let class_name = raw[class_start..class_end].to_string();
    if class_name.is_empty() {
        return None;
    }
    let open = skip_ws(raw, class_end, raw.len());
    if !raw[open..].starts_with('(') {
        return None;
    }
    let close = find_matching(raw, open, '(', ')')?;
    if !raw[close + 1..].trim().is_empty() {
        return None;
    }
    Some((class_name, open + 1, close))
}

fn split_update_expression(raw: &str) -> Option<(usize, usize, UpdateOp, UpdatePosition)> {
    if let Some(rest) = raw.strip_prefix("++") {
        let target_start = raw.len() - rest.len();
        if !rest.trim().is_empty() {
            return Some((
                target_start,
                raw.len(),
                UpdateOp::Increment,
                UpdatePosition::Prefix,
            ));
        }
    }
    if let Some(rest) = raw.strip_prefix("--") {
        let target_start = raw.len() - rest.len();
        if !rest.trim().is_empty() {
            return Some((
                target_start,
                raw.len(),
                UpdateOp::Decrement,
                UpdatePosition::Prefix,
            ));
        }
    }
    if let Some(target) = raw.strip_suffix("++") {
        if !target.trim().is_empty() {
            return Some((
                0,
                target.len(),
                UpdateOp::Increment,
                UpdatePosition::Postfix,
            ));
        }
    }
    if let Some(target) = raw.strip_suffix("--") {
        if !target.trim().is_empty() {
            return Some((
                0,
                target.len(),
                UpdateOp::Decrement,
                UpdatePosition::Postfix,
            ));
        }
    }
    None
}

fn parse_object_literal_key(raw: &str) -> Option<String> {
    if is_ident(raw) {
        return Some(raw.to_string());
    }
    parse_string_literal(raw)
}

fn split_as_assertion(raw: &str) -> Option<(usize, usize, usize, usize)> {
    split_typed_operator(raw, "as")
}

fn split_satisfies_operator(raw: &str) -> Option<(usize, usize, usize, usize)> {
    split_typed_operator(raw, "satisfies")
}

fn split_typed_operator(raw: &str, keyword: &str) -> Option<(usize, usize, usize, usize)> {
    let kw_len = keyword.len();
    let mut idx = 0usize;
    let mut found = None;
    while idx < raw.len() {
        let ch = raw[idx..].chars().next()?;
        if ch == '(' || ch == '[' || ch == '{' || ch == '<' {
            let close_ch = match ch {
                '(' => ')',
                '[' => ']',
                '{' => '}',
                '<' => '>',
                _ => unreachable!(),
            };
            idx = find_matching(raw, idx, ch, close_ch)? + 1;
            continue;
        }
        if raw[idx..].starts_with(keyword) {
            let before = raw[..idx].chars().next_back();
            let after = raw[idx + kw_len..].chars().next();
            if before.is_some_and(|c| c.is_whitespace()) && after.is_some_and(|c| c.is_whitespace())
            {
                found = Some(idx);
            }
            idx += kw_len;
            continue;
        }
        idx += ch.len_utf8();
    }
    let kw_idx = found?;
    let expr_raw = raw[..kw_idx].trim();
    let type_raw = raw[kw_idx + kw_len..].trim();
    if expr_raw.is_empty() || type_raw.is_empty() {
        return None;
    }
    let expr_start = raw[..kw_idx].find(expr_raw).unwrap_or(0);
    let type_start = kw_idx + kw_len + raw[kw_idx + kw_len..].find(type_raw).unwrap_or(0);
    Some((
        expr_start,
        expr_start + expr_raw.len(),
        type_start,
        type_start + type_raw.len(),
    ))
}

fn split_non_null_assertion(raw: &str) -> Option<(usize, usize)> {
    let trimmed = raw.trim_end();
    if !trimmed.ends_with('!') || trimmed.ends_with("!=") || trimmed.ends_with("!==") {
        return None;
    }
    let expr_raw = trimmed[..trimmed.len() - 1].trim_end();
    if expr_raw.is_empty() {
        return None;
    }
    Some((0, expr_raw.len()))
}

fn is_decimal_number_spelling(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    match raw.split_once('.') {
        None => bytes.iter().all(u8::is_ascii_digit),
        Some((int_part, frac_part)) => {
            !int_part.is_empty()
                && !frac_part.is_empty()
                && int_part.bytes().all(|b| b.is_ascii_digit())
                && frac_part.bytes().all(|b| b.is_ascii_digit())
        }
    }
}

fn split_property_access(raw: &str) -> Option<(usize, usize, usize, usize, bool)> {
    let mut idx = 0usize;
    let mut found = None;
    while idx < raw.len() {
        let ch = raw[idx..].chars().next()?;
        if ch == '(' || ch == '[' || ch == '{' {
            let close_ch = match ch {
                '(' => ')',
                '[' => ']',
                '{' => '}',
                _ => unreachable!(),
            };
            idx = find_matching(raw, idx, ch, close_ch)? + 1;
            continue;
        }
        if ch == '.' {

            let prev_is_digit = idx > 0 && raw.as_bytes()[idx - 1].is_ascii_digit();
            let next_is_digit = raw[idx + 1..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit());
            if !(prev_is_digit && next_is_digit) {
                found = Some((idx, false));
            }
        } else if ch == '?' && raw[idx..].starts_with("?.") {
            found = Some((idx, true));
            idx += "?. ".trim().len();
            continue;
        }
        idx += ch.len_utf8();
    }
    let (dot, optional) = found?;
    let property_start = dot + if optional { 2 } else { 1 };
    let property = raw[property_start..].trim();
    if !is_ident(property) {
        return None;
    }
    let property_start = property_start + raw[property_start..].find(property).unwrap_or(0);
    Some((
        0,
        dot,
        property_start,
        property_start + property.len(),
        optional,
    ))
}

fn split_element_access(raw: &str) -> Option<(usize, usize, usize, usize, bool)> {
    let mut idx = 0usize;
    let mut found = None;
    while idx < raw.len() {
        let ch = raw[idx..].chars().next()?;
        if ch == '(' || ch == '{' {
            let close_ch = match ch {
                '(' => ')',
                '{' => '}',
                _ => unreachable!(),
            };
            idx = find_matching(raw, idx, ch, close_ch)? + 1;
            continue;
        }
        if ch == '[' {
            let close = find_matching(raw, idx, '[', ']')?;
            found = Some((idx, close, false));
            idx = close + 1;
            continue;
        }
        if ch == '?' && raw[idx..].starts_with("?.[") {
            let bracket = idx + 2;
            let close = find_matching(raw, bracket, '[', ']')?;
            found = Some((bracket, close, true));
            idx = close + 1;
            continue;
        }
        idx += ch.len_utf8();
    }
    let (bracket, close, optional) = found?;
    let object_end = if optional { bracket - 2 } else { bracket };
    if object_end == 0 || close != raw.len() - 1 {
        return None;
    }
    let index_raw = raw[bracket + 1..close].trim();
    if index_raw.is_empty() {
        return None;
    }
    let index_start = bracket + 1 + raw[bracket + 1..close].find(index_raw).unwrap_or(0);
    Some((
        0,
        object_end,
        index_start,
        index_start + index_raw.len(),
        optional,
    ))
}

fn split_call(raw: &str) -> Option<(usize, usize, usize, usize, bool)> {
    if !raw.ends_with(')') {
        return None;
    }
    let mut idx = 0usize;
    let mut found = None;
    while idx < raw.len() {
        let ch = raw[idx..].chars().next()?;
        if ch == '[' || ch == '{' {
            let close_ch = match ch {
                '[' => ']',
                '{' => '}',
                _ => unreachable!(),
            };
            idx = find_matching(raw, idx, ch, close_ch)? + 1;
            continue;
        }
        if ch == '(' {
            let close = find_matching(raw, idx, '(', ')')?;
            found = Some((idx, close, false));
            idx = close + 1;
            continue;
        }
        if ch == '?' && raw[idx..].starts_with("?.(") {
            let open = idx + 2;
            let close = find_matching(raw, open, '(', ')')?;
            found = Some((open, close, true));
            idx = close + 1;
            continue;
        }
        idx += ch.len_utf8();
    }
    let (open, close, optional) = found?;
    let callee_end = if optional { open - 2 } else { open };
    if callee_end == 0 || close != raw.len() - 1 {
        return None;
    }
    Some((0, callee_end, open + 1, close, optional))
}

fn parse_string_literal(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        return cook_string_literal_text(&raw[1..raw.len() - 1]);
    }
    None
}

fn cook_string_literal_text(raw: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let escaped = chars.next()?;
        match escaped {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'b' => out.push('\u{0008}'),
            'f' => out.push('\u{000C}'),
            'v' => out.push('\u{000B}'),
            '0' => out.push('\0'),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            '\'' => out.push('\''),
            '`' => out.push('`'),
            '$' => out.push('$'),
            '\n' => {}
            '\r' => {
                if matches!(chars.clone().next(), Some('\n')) {
                    chars.next();
                }
            }
            other => out.push(other),
        }
    }
    Some(out)
}

fn string_literal_end(s: &str, idx: usize) -> Option<usize> {
    if idx > s.len() || !s.is_char_boundary(idx) {
        return None;
    }
    let mut chars = s[idx..].char_indices();
    let (_, quote) = chars.next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let mut escaped = false;
    for (rel, ch) in chars {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Some(idx + rel + ch.len_utf8());
        }
    }
    Some(s.len())
}

fn regex_literal_end(s: &str, idx: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.get(idx) != Some(&b'/') {
        return None;
    }
    if s[idx..].starts_with("//") || s[idx..].starts_with("/*") {
        return None;
    }
    let mut i = idx + 1;
    let mut in_class = false;
    let mut closing = None;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i += 2;
                continue;
            }
            b'[' => in_class = true,
            b']' => in_class = false,
            b'/' if !in_class => {
                closing = Some(i);
                break;
            }
            b'\n' => return None,
            _ => {}
        }
        i += 1;
    }
    let close = closing?;
    if close == idx + 1 {
        return None;
    }
    let mut end = close + 1;
    while end < bytes.len() && bytes[end].is_ascii_lowercase() {
        end += 1;
    }
    Some(end)
}

fn split_args(args: &str) -> Vec<(usize, usize)> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut idx = 0usize;
    let mut paren_depth = 0usize;

    let mut operand = true;
    while idx < args.len() {

        if let Some(string_end) = string_literal_end(args, idx) {
            idx = string_end;
            operand = false;
            continue;
        }
        let ch = args[idx..].chars().next().expect("valid argument char");

        if ch == '/' && operand {
            if let Some(regex_end) = regex_literal_end(args, idx) {
                idx = regex_end;
                operand = false;
                continue;
            }
        }
        match ch {
            '(' => {
                paren_depth += 1;
                operand = true;
            }
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
                operand = false;
            }
            '[' | '{' if paren_depth == 0 => {
                let close_ch = if ch == '[' { ']' } else { '}' };
                if let Some(close) = find_matching(args, idx, ch, close_ch) {
                    idx = close + 1;
                    operand = false;
                    continue;
                }
                operand = false;
            }
            ',' if paren_depth == 0 => {
                operand = true;
                push_arg_span(args, start, idx, &mut parts);
                start = idx + 1;
            }
            c if c.is_whitespace() => {   }
            c => {

                operand = !(c.is_alphanumeric() || c == '_');
            }
        }
        idx += ch.len_utf8();
    }
    push_arg_span(args, start, args.len(), &mut parts);
    parts
}

fn push_arg_span(args: &str, start: usize, end: usize, parts: &mut Vec<(usize, usize)>) {
    let raw = &args[start..end];
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }
    let leading_ws = raw.find(trimmed).unwrap_or(0);
    parts.push((start + leading_ws, start + leading_ws + trimmed.len()));
}

fn parse_boundary_clause_in(
    text: &str,
    base_offset: usize,
    site: BoundarySite,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<BoundaryClause> {
    let boundary = find_keyword_outside_strings(text, "boundary")?;
    let start = base_offset + boundary;
    let after = text[boundary + "boundary".len()..].trim_start();
    let ws = text[boundary + "boundary".len()..].len() - after.len();
    if !after.starts_with('(') {
        return Some(BoundaryClause {
            span: Span::new(start, start + "boundary".len()),
            site,
            directives: vec![BoundaryDirective::DefaultShorthand],
        });
    }
    let open = boundary + "boundary".len() + ws;
    let close = find_matching(text, open, '(', ')')?;
    let inner = &text[open + 1..close];
    let directives = parse_boundary_directives(inner);

    for directive in &directives {
        if let BoundaryDirective::Unsupported(part) = directive {
            diagnostics.push(Diagnostic {
                code: DiagnosticCode::MalformedBoundaryDirective,
                span: Span::new(base_offset + open + 1, base_offset + close),
                message: format!("malformed boundary directive: '{part}'"),
            });
        }
    }
    if site == BoundarySite::Import
        && directives
            .iter()
            .any(|d| matches!(d, BoundaryDirective::SkipReturnValidation))
    {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::SkipReturnValidationOnImport,
            span: Span::new(base_offset + open + 1, base_offset + close),
            message: "skip return validation is not allowed on imports".to_string(),
        });
    }
    Some(BoundaryClause {
        span: Span::new(start, base_offset + close + 1),
        site,
        directives,
    })
}

fn find_keyword_outside_strings(text: &str, keyword: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let keyword_bytes = keyword.as_bytes();
    let mut quote = None;
    let mut escaped = false;
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active_quote {
                quote = None;
            }
            i += 1;
            continue;
        }
        if byte == b'"' || byte == b'\'' || byte == b'`' {
            quote = Some(byte);
            i += 1;
            continue;
        }
        if text[i..].starts_with(keyword) && is_keyword_boundary(bytes, i, keyword_bytes.len()) {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn is_keyword_boundary(bytes: &[u8], start: usize, len: usize) -> bool {
    let before = start
        .checked_sub(1)
        .and_then(|idx| bytes.get(idx))
        .copied()
        .is_none_or(|byte| !is_ident_byte(byte));
    let after = bytes
        .get(start + len)
        .copied()
        .is_none_or(|byte| !is_ident_byte(byte));
    before && after
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' || byte == b'-'
}

fn parse_compartment_header(
    text: &str,
    base_offset: usize,
    name_end: usize,
) -> Option<(usize, Vec<EndowmentDecl>)> {
    let mut cursor = name_end;
    let mut endowments = Vec::new();

    while cursor < text.len() {
        let open = text[cursor..].find('{')? + cursor;
        if header_prefix_ends_with_keyword(&text[cursor..open], "endow") {
            let close = matching_close_brace(text, open)?;
            endowments.extend(parse_endowment_decls(text, base_offset, open, close));
            cursor = close + 1;
            continue;
        }
        return Some((open, endowments));
    }

    None
}

fn header_prefix_ends_with_keyword(prefix: &str, keyword: &str) -> bool {
    let trimmed = prefix.trim_end();
    let Some(before) = trimmed.strip_suffix(keyword) else {
        return false;
    };
    before
        .chars()
        .next_back()
        .map_or(true, |ch| !(ch == '_' || ch.is_ascii_alphanumeric()))
}

fn parse_endowment_decls(
    text: &str,
    base_offset: usize,
    open: usize,
    close: usize,
) -> Vec<EndowmentDecl> {
    split_top_level(text, open + 1, close, ',')
        .into_iter()
        .filter_map(|(entry_start, entry_end)| {
            let colon = find_top_level_char(text, entry_start, entry_end, ':')?;
            let raw_name = text[entry_start..colon].trim();
            if !is_ident(raw_name) {
                return None;
            }
            let name_start = text[entry_start..colon].find(raw_name).unwrap_or(0) + entry_start;
            let type_annotation = parse_type_expr(text, colon + 1, entry_end, base_offset)?;
            Some(EndowmentDecl {
                name: raw_name.to_string(),
                type_annotation,
                span: Span::new(base_offset + name_start, base_offset + entry_end),
            })
        })
        .collect()
}

fn parse_boundary_directives(inner: &str) -> Vec<BoundaryDirective> {
    inner
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            if part == "skip return validation" {
                BoundaryDirective::SkipReturnValidation
            } else if let Some(policy) = part.strip_prefix("weaken to ") {
                BoundaryDirective::WeakenTo(policy.trim().to_string())
            } else if let Some(policy) = part.strip_prefix("override:") {
                BoundaryDirective::Override(policy.trim().to_string())
            } else if part.chars().all(|c| c == '_' || c.is_ascii_alphanumeric()) {
                BoundaryDirective::Policy(part.to_string())
            } else {
                BoundaryDirective::Unsupported(part.to_string())
            }
        })
        .collect()
}

fn parse_type_params(signature: &str, name_end: usize, base_offset: usize) -> Vec<TypeParam> {
    let open = signature[name_end..].find('<').map(|idx| name_end + idx);
    let Some(open) = open else {
        return Vec::new();
    };
    let Some(close) = find_matching(signature, open, '<', '>') else {
        return Vec::new();
    };
    let inner = &signature[open + 1..close];
    inner
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            let local_offset = signature[open + 1..].find(part).unwrap_or(0) + open + 1;
            if let Some((name, _constraint)) = part.split_once("extends") {
                let name = name.trim().to_string();
                let constraint_start =
                    local_offset + part.find("extends").unwrap() + "extends".len();
                let constraint = parse_type_expr(
                    signature,
                    constraint_start,
                    local_offset + part.len(),
                    base_offset,
                );
                TypeParam {
                    name,
                    constraint,
                    span: Span::new(
                        base_offset + local_offset,
                        base_offset + local_offset + part.len(),
                    ),
                }
            } else {
                TypeParam {
                    name: part.to_string(),
                    constraint: None,
                    span: Span::new(
                        base_offset + local_offset,
                        base_offset + local_offset + part.len(),
                    ),
                }
            }
        })
        .collect()
}

fn parse_params(
    signature: &str,
    params_start: usize,
    params_end: usize,
    base_offset: usize,
) -> Vec<FunctionParam> {
    parse_params_opt(signature, params_start, params_end, base_offset, false)
}

fn parse_params_opt(
    signature: &str,
    params_start: usize,
    params_end: usize,
    base_offset: usize,
    allow_unannotated: bool,
) -> Vec<FunctionParam> {
    split_top_level(signature, params_start + 1, params_end, ',')
        .into_iter()
        .filter_map(|part| {
            let (part_start, part_end) = part;
            let raw = signature[part_start..part_end].trim();
            if raw.is_empty() {
                return None;
            }
            let local_offset = signature[part_start..part_end].find(raw)? + part_start;
            match find_top_level_char(signature, local_offset, part_end, ':') {
                Some(colon) => {
                    let name = signature[local_offset..colon].trim();
                    let type_start = colon + ':'.len_utf8();
                    let default_eq = find_top_level_char(signature, type_start, part_end, '=');
                    let type_end = default_eq.unwrap_or(part_end);
                    let ty = parse_type_expr(signature, type_start, type_end, base_offset)?;
                    let default = default_eq.and_then(|eq| {
                        parse_expr_envelope(signature, eq + '='.len_utf8(), part_end, base_offset)
                    });
                    Some(FunctionParam {
                        name: name.trim().to_string(),
                        ty,
                        default,
                        span: Span::new(base_offset + local_offset, base_offset + part_end),
                    })
                }
                None if allow_unannotated => {
                    let span = Span::new(base_offset + local_offset, base_offset + part_end);
                    Some(FunctionParam {
                        name: raw.to_string(),
                        ty: TypeExpr {
                            kind: TypeExprKind::Named("unknown".to_string()),
                            span,
                        },
                        default: None,
                        span,
                    })
                }
                None => None,
            }
        })
        .collect()
}

fn parse_array_shorthand_type_expr(
    text: &str,
    start: usize,
    end: usize,
    base_offset: usize,
    span: Span,
) -> Option<TypeExpr> {

    let trailing_ws = text[start..end].len() - text[start..end].trim_end().len();
    let content_end = end - trailing_ws;

    if content_end < start + 2 || &text[content_end - 2..content_end] != "[]" {
        return None;
    }
    let element_end = content_end - 2;
    let element = parse_type_expr(text, start, element_end, base_offset)?;
    Some(TypeExpr {
        kind: TypeExprKind::TypeRef {
            name: "Array".to_string(),
            type_args: vec![element],
        },
        span,
    })
}

fn parse_type_expr(text: &str, start: usize, end: usize, base_offset: usize) -> Option<TypeExpr> {
    let raw = text[start..end].trim();
    if raw.is_empty() {
        return None;
    }
    let leading_ws = text[start..end].len() - text[start..end].trim_start().len();
    let trailing_ws = text[start..end].len() - text[start..end].trim_end().len();
    let span = Span::new(
        base_offset + start + leading_ws,
        base_offset + end - trailing_ws,
    );

    if let Some(constructor_type) = parse_constructor_type_expr(text, start, end, base_offset, span)
    {
        return Some(constructor_type);
    }

    if let Some(function_type) = parse_function_type_expr(text, start, end, base_offset, span) {
        return Some(function_type);
    }

    if let Some(tuple_type) = parse_tuple_type_expr(text, start, end, base_offset, span) {
        return Some(tuple_type);
    }

    if let Some(mapped_type) = parse_mapped_type_expr(text, start, end, base_offset, span) {
        return Some(mapped_type);
    }

    if let Some(predicate_type) = parse_type_predicate_expr(text, start, end, span) {
        return Some(predicate_type);
    }

    if let Some(object_type) = parse_object_type_expr(text, start, end, base_offset, span) {
        return Some(object_type);
    }

    if let Some(extends_start) = find_top_level_keyword(text, start, end, " extends ") {
        let after_extends = extends_start + " extends ".len();
        if let Some(question) = find_top_level_char(text, after_extends, end, '?') {
            if let Some(colon) = find_conditional_colon(text, question + 1, end) {
                return Some(TypeExpr {
                    kind: TypeExprKind::Conditional {
                        check: Box::new(parse_type_expr(text, start, extends_start, base_offset)?),
                        extends: Box::new(parse_type_expr(
                            text,
                            after_extends,
                            question,
                            base_offset,
                        )?),
                        true_type: Box::new(parse_type_expr(
                            text,
                            question + 1,
                            colon,
                            base_offset,
                        )?),
                        false_type: Box::new(parse_type_expr(text, colon + 1, end, base_offset)?),
                    },
                    span,
                });
            }
        }
    }

    let union_parts = split_top_level(text, start, end, '|');
    if union_parts.len() > 1 {
        let mut members = Vec::new();
        for (part_start, part_end) in union_parts {
            members.push(parse_type_expr(text, part_start, part_end, base_offset)?);
        }
        return Some(TypeExpr {
            kind: TypeExprKind::Union(members),
            span,
        });
    }

    let intersection_parts = split_top_level(text, start, end, '&');
    if intersection_parts.len() > 1 {
        let mut merged_properties = Vec::new();
        let mut all_objects = true;
        for (part_start, part_end) in &intersection_parts {
            let member = parse_type_expr(text, *part_start, *part_end, base_offset)?;
            if let TypeExprKind::Object(object) = member.kind {
                merged_properties.extend(object.properties);
            } else {
                all_objects = false;
                break;
            }
        }
        if all_objects {
            return Some(TypeExpr {
                kind: TypeExprKind::Object(ObjectTypeExpr {
                    properties: merged_properties,
                }),
                span,
            });
        }
    }

    if let Some(array_shorthand) =
        parse_array_shorthand_type_expr(text, start, end, base_offset, span)
    {
        return Some(array_shorthand);
    }

    if let Some(infer) = parse_infer_type_expr(text, start, end, base_offset, span) {
        return Some(infer);
    }

    if let Some(type_ref) = parse_type_ref_expr(text, start, end, base_offset, span) {
        return Some(type_ref);
    }

    if let Some(indexed_access) =
        parse_indexed_access_type_expr(text, start, end, base_offset, span)
    {
        return Some(indexed_access);
    }

    let kind = if is_ident(raw) {
        TypeExprKind::Named(raw.to_string())
    } else {
        TypeExprKind::Opaque(raw.to_string())
    };
    Some(TypeExpr { kind, span })
}

fn parse_type_predicate_expr(text: &str, start: usize, end: usize, span: Span) -> Option<TypeExpr> {
    let is_start = find_top_level_keyword(text, start, end, " is ")?;
    let subject = text[start..is_start].trim();
    let target = text[is_start + " is ".len()..end].trim();
    if !is_ident(subject) || target.is_empty() {
        return None;
    }
    Some(TypeExpr {
        kind: TypeExprKind::Opaque(text[start..end].trim().to_string()),
        span,
    })
}

fn parse_infer_type_expr(
    text: &str,
    start: usize,
    end: usize,
    base_offset: usize,
    span: Span,
) -> Option<TypeExpr> {
    let raw = text[start..end].trim();
    let infer_rest = raw.strip_prefix("infer ")?;
    let raw_start = text[start..end].find(raw).unwrap_or(0) + start;
    let rest_start = raw_start + "infer ".len();
    if let Some(extends_rel) = infer_rest.find(" extends ") {
        let name = infer_rest[..extends_rel].trim();
        if !is_ident(name) {
            return None;
        }
        let constraint_start = rest_start + extends_rel + " extends ".len();
        let constraint = parse_type_expr(text, constraint_start, end, base_offset)?;
        return Some(TypeExpr {
            kind: TypeExprKind::Infer {
                name: name.to_string(),
                constraint: Some(Box::new(constraint)),
            },
            span,
        });
    }
    let name = infer_rest.trim();
    if !is_ident(name) {
        return None;
    }
    Some(TypeExpr {
        kind: TypeExprKind::Infer {
            name: name.to_string(),
            constraint: None,
        },
        span,
    })
}

fn parse_type_ref_expr(
    text: &str,
    start: usize,
    end: usize,
    base_offset: usize,
    span: Span,
) -> Option<TypeExpr> {
    let raw = text[start..end].trim();
    let raw_start = text[start..end].find(raw).unwrap_or(0) + start;
    let open_rel = raw.find('<')?;
    if !raw.ends_with('>') {
        return None;
    }
    let name = raw[..open_rel].trim();
    if !is_ident(name) {
        return None;
    }
    let open = raw_start + open_rel;
    let close = find_matching(text, open, '<', '>')?;
    if close != raw_start + raw.len() - 1 {
        return None;
    }
    let type_args = split_top_level(text, open + 1, close, ',')
        .into_iter()
        .filter_map(|(arg_start, arg_end)| parse_type_expr(text, arg_start, arg_end, base_offset))
        .collect();
    Some(TypeExpr {
        kind: TypeExprKind::TypeRef {
            name: name.to_string(),
            type_args,
        },
        span,
    })
}

fn parse_mapped_type_expr(
    text: &str,
    start: usize,
    end: usize,
    base_offset: usize,
    span: Span,
) -> Option<TypeExpr> {
    let raw = text[start..end].trim();
    let raw_start = text[start..end].find(raw).unwrap_or(0) + start;
    if !raw.starts_with('{') || !raw.ends_with('}') {
        return None;
    }
    let close = find_matching(text, raw_start, '{', '}')?;
    if close != raw_start + raw.len() - 1 {
        return None;
    }

    let inner_start = raw_start + 1;
    let inner_end = close;
    let inner = text[inner_start..inner_end].trim();
    let inner_start = text[inner_start..inner_end].find(inner).unwrap_or(0) + inner_start;
    let mut cursor = inner_start;

    let readonly_modifier = if text[cursor..inner_end].starts_with("+readonly") {
        cursor += "+readonly".len();
        MappedModifier::Add
    } else if text[cursor..inner_end].starts_with("-readonly") {
        cursor += "-readonly".len();
        MappedModifier::Remove
    } else if text[cursor..inner_end].starts_with("readonly") {
        cursor += "readonly".len();
        MappedModifier::Add
    } else {
        MappedModifier::Inherit
    };
    cursor = skip_ws(text, cursor, inner_end);

    if !text[cursor..inner_end].starts_with('[') {
        return None;
    }
    let bracket_open = cursor;
    let bracket_close = find_matching(text, bracket_open, '[', ']')?;
    let bracket_inner = &text[bracket_open + 1..bracket_close];
    let in_rel = bracket_inner.find(" in ")?;
    let key_name = bracket_inner[..in_rel].trim();
    if !is_ident(key_name) {
        return None;
    }
    let constraint_start = bracket_open + 1 + in_rel + " in ".len();
    let constraint_end = bracket_close;
    let constraint_raw = text[constraint_start..constraint_end].trim();
    let constraint_trim_start = constraint_start
        + text[constraint_start..constraint_end]
            .find(constraint_raw)
            .unwrap_or(0);
    let key_constraint = if let Some(target_raw) = constraint_raw.strip_prefix("keyof ") {
        let target_start = constraint_trim_start + "keyof ".len();
        let target_end = target_start + target_raw.trim().len();
        MappedKeyConstraint::Keyof {
            target: Box::new(parse_type_expr(
                text,
                target_start,
                target_end,
                base_offset,
            )?),
        }
    } else {
        MappedKeyConstraint::Type(Box::new(parse_type_expr(
            text,
            constraint_trim_start,
            constraint_trim_start + constraint_raw.len(),
            base_offset,
        )?))
    };

    cursor = skip_ws(text, bracket_close + 1, inner_end);
    let optional_modifier = if text[cursor..inner_end].starts_with("+?") {
        cursor += "+?".len();
        MappedModifier::Add
    } else if text[cursor..inner_end].starts_with("-?") {
        cursor += "-?".len();
        MappedModifier::Remove
    } else if text[cursor..inner_end].starts_with('?') {
        cursor += "?".len();
        MappedModifier::Add
    } else {
        MappedModifier::Inherit
    };
    cursor = skip_ws(text, cursor, inner_end);
    if !text[cursor..inner_end].starts_with(':') {
        return None;
    }
    cursor += ':'.len_utf8();
    let value_end = text[cursor..inner_end]
        .rfind(';')
        .map(|idx| cursor + idx)
        .unwrap_or(inner_end);
    let value_type = parse_type_expr(text, cursor, value_end, base_offset)?;

    Some(TypeExpr {
        kind: TypeExprKind::Mapped(MappedTypeExpr {
            key_name: key_name.to_string(),
            key_constraint,
            value_type: Box::new(value_type),
            readonly_modifier,
            optional_modifier,
        }),
        span,
    })
}

fn parse_object_type_expr(
    text: &str,
    start: usize,
    end: usize,
    base_offset: usize,
    span: Span,
) -> Option<TypeExpr> {
    let raw = text[start..end].trim();
    let raw_start = text[start..end].find(raw).unwrap_or(0) + start;
    if !raw.starts_with('{') || !raw.ends_with('}') {
        return None;
    }
    let close = find_matching(text, raw_start, '{', '}')?;
    if close != raw_start + raw.len() - 1 {
        return None;
    }
    let mut properties = Vec::new();
    for (part_start, part_end) in split_type_members(text, raw_start + 1, close) {
        let property = parse_object_type_property(text, part_start, part_end, base_offset)?;
        properties.push(property);
    }
    if properties.is_empty() {
        return None;
    }
    Some(TypeExpr {
        kind: TypeExprKind::Object(ObjectTypeExpr { properties }),
        span,
    })
}

fn parse_object_type_property(
    text: &str,
    start: usize,
    end: usize,
    base_offset: usize,
) -> Option<ObjectTypeProperty> {
    let raw = text[start..end].trim();
    if raw.is_empty() {
        return None;
    }
    let local_start = text[start..end].find(raw).unwrap_or(0) + start;
    let mut cursor = local_start;
    let readonly = if text[cursor..end].starts_with("readonly") {
        cursor += "readonly".len();
        true
    } else {
        false
    };
    cursor = skip_ws(text, cursor, end);
    let name_end = scan_ident_end(text, cursor);
    let name = text[cursor..name_end].trim();
    if !is_ident(name) {
        return None;
    }
    cursor = skip_ws(text, name_end, end);
    let optional = if text[cursor..end].starts_with('?') {
        cursor += '?'.len_utf8();
        true
    } else {
        false
    };
    cursor = skip_ws(text, cursor, end);
    if !text[cursor..end].starts_with(':') {
        return None;
    }
    cursor += ':'.len_utf8();
    let ty = parse_type_expr(text, cursor, end, base_offset)?;
    Some(ObjectTypeProperty {
        name: name.to_string(),
        ty,
        readonly,
        optional,
        span: Span::new(base_offset + local_start, base_offset + end),
    })
}

fn parse_indexed_access_type_expr(
    text: &str,
    start: usize,
    end: usize,
    base_offset: usize,
    span: Span,
) -> Option<TypeExpr> {
    let raw = text[start..end].trim();
    let raw_start = text[start..end].find(raw).unwrap_or(0) + start;
    if !raw.ends_with(']') {
        return None;
    }
    let bracket_open = find_top_level_last_char(text, raw_start, raw_start + raw.len(), '[')?;
    let bracket_close = find_matching(text, bracket_open, '[', ']')?;
    if bracket_close != raw_start + raw.len() - 1 || bracket_open == raw_start {
        return None;
    }
    Some(TypeExpr {
        kind: TypeExprKind::IndexedAccess {
            object: Box::new(parse_type_expr(text, raw_start, bracket_open, base_offset)?),
            index: Box::new(parse_type_expr(
                text,
                bracket_open + 1,
                bracket_close,
                base_offset,
            )?),
        },
        span,
    })
}

fn parse_tuple_type_expr(
    text: &str,
    start: usize,
    end: usize,
    base_offset: usize,
    span: Span,
) -> Option<TypeExpr> {
    let raw = text[start..end].trim();
    let raw_start = text[start..end].find(raw).unwrap_or(0) + start;
    if !raw.starts_with('[') || !raw.ends_with(']') {
        return None;
    }
    let close = find_matching(text, raw_start, '[', ']')?;
    if close != raw_start + raw.len() - 1 {
        return None;
    }
    let elements = split_top_level(text, raw_start + 1, close, ',')
        .into_iter()
        .filter_map(|(element_start, element_end)| {
            parse_type_expr(text, element_start, element_end, base_offset)
        })
        .collect();
    Some(TypeExpr {
        kind: TypeExprKind::Tuple(elements),
        span,
    })
}

fn parse_constructor_type_expr(
    text: &str,
    start: usize,
    end: usize,
    base_offset: usize,
    span: Span,
) -> Option<TypeExpr> {
    let raw = text[start..end].trim();
    let raw_start = text[start..end].find(raw).unwrap_or(0) + start;
    let (is_abstract, mut cursor) = if raw.starts_with("abstract new ") {
        (true, raw_start + "abstract new ".len())
    } else if raw.starts_with("new ") {
        (false, raw_start + "new ".len())
    } else {
        return None;
    };
    cursor = skip_ws(text, cursor, end);
    if !text[cursor..end].starts_with('(') {
        return None;
    }
    let params_start = cursor;
    let params_end = find_matching(text, params_start, '(', ')')?;
    let arrow_start = text[params_end + 1..end].find("=>")? + params_end + 1;
    let params = parse_function_type_params(text, params_start, params_end, base_offset);
    let instance_type = parse_type_expr(text, arrow_start + "=>".len(), end, base_offset)?;
    Some(TypeExpr {
        kind: TypeExprKind::Constructor {
            params,
            instance_type: Box::new(instance_type),
            is_abstract,
        },
        span,
    })
}

fn parse_function_type_expr(
    text: &str,
    start: usize,
    end: usize,
    base_offset: usize,
    span: Span,
) -> Option<TypeExpr> {
    let raw = text[start..end].trim();
    let raw_start = text[start..end].find(raw).unwrap_or(0) + start;
    let mut cursor = raw_start;
    let mut type_params = Vec::new();

    if text[cursor..end].trim_start().starts_with('<') {
        let ws = text[cursor..end].len() - text[cursor..end].trim_start().len();
        let open = cursor + ws;
        let close = find_matching(text, open, '<', '>')?;
        type_params = parse_type_params(text, open, base_offset);
        cursor = close + 1;
    }

    let ws = text[cursor..end].len() - text[cursor..end].trim_start().len();
    let params_start = cursor + ws;
    if !text[params_start..end].starts_with('(') {
        return None;
    }
    let params_end = find_matching(text, params_start, '(', ')')?;
    let arrow_start = text[params_end + 1..end].find("=>")? + params_end + 1;
    let params = parse_function_type_params(text, params_start, params_end, base_offset);
    let return_start = arrow_start + "=>".len();
    let return_end = text[return_start..end]
        .find("boundary")
        .map(|idx| return_start + idx)
        .unwrap_or(end);
    let return_type = parse_type_expr(text, return_start, return_end, base_offset)?;
    let boundary_clause = parse_boundary_clause_in(
        &text[return_end..end],
        base_offset + return_end,
        BoundarySite::FunctionType,
        &mut Vec::new(),
    );
    Some(TypeExpr {
        kind: TypeExprKind::Function {
            type_params,
            params,
            return_type: Box::new(return_type),
            boundary_clause,
        },
        span,
    })
}

fn parse_function_type_params(
    text: &str,
    params_start: usize,
    params_end: usize,
    base_offset: usize,
) -> Vec<FunctionTypeParam> {
    split_top_level(text, params_start + 1, params_end, ',')
        .into_iter()
        .filter_map(|part| {
            let (part_start, part_end) = part;
            let raw = text[part_start..part_end].trim();
            if raw.is_empty() {
                return None;
            }
            let local_offset = text[part_start..part_end].find(raw)? + part_start;
            let (name, type_start, type_end) =
                if let Some(colon) = find_top_level_char(text, local_offset, part_end, ':') {
                    (
                        Some(text[local_offset..colon].trim().to_string()),
                        colon + ':'.len_utf8(),
                        part_end,
                    )
                } else {
                    (None, local_offset, part_end)
                };
            let ty = parse_type_expr(text, type_start, type_end, base_offset)?;
            Some(FunctionTypeParam {
                name,
                ty,
                span: Span::new(base_offset + local_offset, base_offset + part_end),
            })
        })
        .collect()
}

fn between<'a>(text: &'a str, left: &str, right: &str) -> Option<&'a str> {
    let start = text.find(left)? + left.len();
    let end = if right.is_empty() {
        text.len()
    } else {
        text[start..].find(right)? + start
    };
    Some(&text[start..end])
}

fn line_end(src: &str, start: usize) -> usize {
    src[start..]
        .find('\n')
        .map(|idx| start + idx)
        .unwrap_or(src.len())
}

fn next_line_or_char(src: &str, start: usize) -> usize {
    src[start..]
        .find('\n')
        .map(|idx| start + idx + 1)
        .unwrap_or_else(|| start + src[start..].chars().next().map(char::len_utf8).unwrap_or(1))
}

fn class_decl_end(src: &str, start: usize) -> Option<usize> {
    let open = src[start..].find('{')? + start;
    matching_close_brace(src, open).map(|close| close + 1)
}

fn parse_class_field(
    line: &str,
    absolute_line_start: usize,
    local_line_start: usize,
    local_line_end: usize,
) -> Option<ClassFieldDecl> {
    if line
        .find('(')
        .is_some_and(|open| line.find('=').is_none_or(|eq| open < eq))
        || line.starts_with('#')
        || line.starts_with("static ")
        || line.starts_with("get ")
        || line.starts_with("set ")
        || line.starts_with("constructor")
    {
        return None;
    }

    let mut field = line.trim_end_matches(';').trim_end_matches(',').trim();
    let readonly = if let Some(rest) = field.strip_prefix("readonly ") {
        field = rest.trim_start();
        true
    } else {
        false
    };
    let initializer = field.find('=').and_then(|eq| {
        let init_start = eq + 1;
        parse_expr_envelope(
            field,
            init_start,
            field.len(),
            absolute_line_start + line.find(field).unwrap_or(0),
        )
    });
    let annotation_end = field.find('=').unwrap_or(field.len());
    let annotation = field[..annotation_end].trim_end();
    let colon = annotation.find(':')?;
    let mut name = annotation[..colon].trim();
    let optional = if let Some(stripped) = name.strip_suffix('?') {
        name = stripped.trim_end();
        true
    } else {
        false
    };
    if name.is_empty() || scan_ident_end(name, 0) != name.len() {
        return None;
    }

    let field_offset = line.find(field).unwrap_or(0);
    let ty_start = annotation.find(':')? + 1;
    let ty = parse_type_expr(
        annotation,
        ty_start,
        annotation.len(),
        absolute_line_start + field_offset,
    )?;
    Some(ClassFieldDecl {
        name: name.to_string(),
        ty,
        readonly,
        optional,
        initializer,
        span: Span::new(
            absolute_line_start,
            absolute_line_start + (local_line_end - local_line_start),
        ),
    })
}

fn starts_with_class_declaration(rest: &str) -> bool {
    rest.strip_prefix("class")
        .and_then(|tail| tail.chars().next())
        .is_some_and(|c| c.is_ascii_whitespace())
}

fn starts_with_export_class_declaration(rest: &str) -> bool {
    let Some(tail) = rest.strip_prefix("export") else {
        return false;
    };
    let trimmed = tail.trim_start();
    trimmed.len() < tail.len() && starts_with_class_declaration(trimmed)
}

fn scan_ident_end(src: &str, start: usize) -> usize {
    src[start..]
        .find(|c: char| !(c == '_' || c.is_ascii_alphanumeric()))
        .map(|idx| start + idx)
        .unwrap_or(src.len())
}

fn skip_ws(src: &str, mut cursor: usize, end: usize) -> usize {
    while cursor < end {
        let Some(ch) = src[cursor..end].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        cursor += ch.len_utf8();
    }
    cursor
}

fn parse_boundary_policy_process_mode(body: &str) -> Option<BoundaryPolicyProcessMode> {
    let mode = body.find("mode:")?;
    let value_start = skip_ws(body, mode + "mode:".len(), body.len());
    let value_end = scan_ident_end(body, value_start);
    match body[value_start..value_end].trim() {
        "strict" => Some(BoundaryPolicyProcessMode::Strict),
        "debug" => Some(BoundaryPolicyProcessMode::Debug),
        "sanitize" => Some(BoundaryPolicyProcessMode::Sanitize),
        _ => None,
    }
}

fn parse_sanitizer_defaults(body: &str, base_offset: usize) -> Vec<SanitizerDefaultDecl> {
    let Some(at_sanitize) = body.find("at sanitize") else {
        return Vec::new();
    };
    let Some(open_rel) = body[at_sanitize..].find('{').map(|idx| at_sanitize + idx) else {
        return Vec::new();
    };
    let Some(close_rel) = matching_close_brace(body, open_rel) else {
        return Vec::new();
    };

    let mut defaults = Vec::new();
    let mut cursor = open_rel + 1;
    while cursor < close_rel {
        let line_end_rel = line_end(body, cursor).min(close_rel);
        let line = &body[cursor..line_end_rel];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            cursor = line_end_rel.saturating_add(1);
            continue;
        }
        let Some(default_rel) = line.find("default") else {
            cursor = line_end_rel.saturating_add(1);
            continue;
        };
        let default_start = cursor + default_rel;
        let after_default = default_start + "default".len();
        let Some(eq_rel) = body[after_default..line_end_rel].find('=') else {
            cursor = line_end_rel.saturating_add(1);
            continue;
        };
        let eq = after_default + eq_rel;
        let Some(target_type) = parse_type_expr(body, after_default, eq, base_offset) else {
            cursor = line_end_rel.saturating_add(1);
            continue;
        };
        let expr = parse_sanitizer_default_expr(body, eq + 1, line_end_rel, base_offset);
        defaults.push(SanitizerDefaultDecl {
            target_type,
            expr,
            span: Span::new(base_offset + default_start, base_offset + line_end_rel),
        });
        cursor = line_end_rel.saturating_add(1);
    }
    defaults
}

fn parse_sanitizer_default_expr(
    text: &str,
    start: usize,
    end: usize,
    base_offset: usize,
) -> SanitizerDefaultExpr {
    let raw = text[start..end].trim();
    match raw {
        "null" => return SanitizerDefaultExpr::Null,
        "undefined" => return SanitizerDefaultExpr::Undefined,
        _ => {}
    }
    match parse_expr_envelope(text, start, end, base_offset).map(|expr| expr.kind) {
        Some(ExprEnvelopeKind::StringLiteral(value)) => SanitizerDefaultExpr::StringLiteral(value),
        Some(ExprEnvelopeKind::NumberLiteral(value)) => SanitizerDefaultExpr::NumberLiteral(value),
        Some(ExprEnvelopeKind::BooleanLiteral(value)) => {
            SanitizerDefaultExpr::BooleanLiteral(value)
        }
        Some(ExprEnvelopeKind::ArrayLiteral(elements)) => {
            SanitizerDefaultExpr::ArrayLiteral(elements)
        }
        Some(ExprEnvelopeKind::ObjectLiteral(properties)) => {
            SanitizerDefaultExpr::ObjectLiteral(properties)
        }
        _ => SanitizerDefaultExpr::Unsupported(raw.to_string()),
    }
}

fn is_ident(src: &str) -> bool {
    !src.is_empty() && src.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
}

fn matching_close_brace(src: &str, open: usize) -> Option<usize> {
    find_matching(src, open, '{', '}')
}

fn find_top_level_function_body_open(src: &str, start: usize, end: usize) -> Option<usize> {
    let mut idx = start;
    while idx < end {
        let ch = src[idx..end].chars().next()?;
        if ch == '{' {
            if let Some(close) = find_matching(src, idx, '{', '}') {
                let after = skip_ws(src, close + 1, end);
                if after < end && src[after..end].starts_with("boundary") {
                    idx = close + 1;
                    continue;
                }
            }
            return Some(idx);
        }
        if ch == '<' || ch == '(' || ch == '[' {
            let close_ch = match ch {
                '<' => '>',
                '(' => ')',
                '[' => ']',
                _ => unreachable!(),
            };
            idx = find_matching(src, idx, ch, close_ch)? + 1;
            continue;
        }
        idx += ch.len_utf8();
    }
    None
}

fn find_matching(src: &str, open: usize, open_ch: char, close_ch: char) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in src[open..].char_indices() {
        if ch == open_ch {
            depth += 1;
        } else if ch == close_ch {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(open + idx);
            }
        }
    }
    None
}

fn find_top_level_keyword(src: &str, start: usize, end: usize, keyword: &str) -> Option<usize> {
    let mut idx = start;
    while idx < end {
        let ch = src[idx..end].chars().next()?;
        if ch == '<' || ch == '(' || ch == '[' || ch == '{' {
            let close_ch = match ch {
                '<' => '>',
                '(' => ')',
                '[' => ']',
                '{' => '}',
                _ => unreachable!(),
            };
            idx = find_matching(src, idx, ch, close_ch)? + 1;
            continue;
        }
        if src[idx..end].starts_with(keyword) {
            return Some(idx);
        }
        idx += ch.len_utf8();
    }
    None
}

fn find_top_level_conditional_question(src: &str, start: usize, end: usize) -> Option<usize> {
    let mut idx = start;
    while idx < end {
        let ch = src[idx..end].chars().next()?;
        if ch == '<' || ch == '(' || ch == '[' || ch == '{' {
            let close_ch = match ch {
                '<' => '>',
                '(' => ')',
                '[' => ']',
                '{' => '}',
                _ => unreachable!(),
            };
            idx = find_matching(src, idx, ch, close_ch)? + 1;
            continue;
        }
        if ch == '?' {
            let next = src[idx + ch.len_utf8()..end].chars().next();
            if next == Some('.') {
                idx += ch.len_utf8();
                continue;
            }
            return Some(idx);
        }
        idx += ch.len_utf8();
    }
    None
}

fn find_top_level_char(src: &str, start: usize, end: usize, needle: char) -> Option<usize> {
    let mut idx = start;
    while idx < end {
        let ch = src[idx..end].chars().next()?;
        if ch == '<' || ch == '(' || ch == '[' || ch == '{' {
            let close_ch = match ch {
                '<' => '>',
                '(' => ')',
                '[' => ']',
                '{' => '}',
                _ => unreachable!(),
            };
            idx = find_matching(src, idx, ch, close_ch)? + 1;
            continue;
        }
        if ch == needle {
            return Some(idx);
        }
        idx += ch.len_utf8();
    }
    None
}

fn find_top_level_add_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_binary_operator(src, start, end, '+')
}

fn find_top_level_sub_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_binary_operator(src, start, end, '-')
}

fn find_top_level_mul_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    let mut idx = find_top_level_binary_operator(src, start, end, '*')?;
    while src.as_bytes().get(idx + 1) == Some(&b'*')
        || idx.checked_sub(1).and_then(|i| src.as_bytes().get(i)) == Some(&b'*')
    {
        idx = find_top_level_binary_operator(src, idx + 1, end, '*')?;
    }
    Some(idx)
}

fn find_top_level_pow_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_binary_pair_operator_without_angle_groups(src, start, end, "**")
}

fn find_top_level_div_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_binary_operator(src, start, end, '/')
}

fn find_top_level_rem_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_binary_operator(src, start, end, '%')
}

fn find_top_level_nullish_coalesce_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_binary_pair_operator_without_angle_groups(src, start, end, "??")
}

fn find_top_level_bitwise_and_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_single_char_operator_without_angle_groups(src, start, end, '&')
}

fn find_top_level_bitwise_or_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_single_char_operator_without_angle_groups(src, start, end, '|')
}

fn find_top_level_bitwise_xor_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_single_char_operator_without_angle_groups(src, start, end, '^')
}

fn find_top_level_shift_left_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_binary_pair_operator_without_angle_groups(src, start, end, "<<")
}

fn find_top_level_shift_right_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_binary_pair_operator_without_angle_groups(src, start, end, ">>")
}

fn find_top_level_less_than_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_binary_operator_without_angle_groups(src, start, end, '<')
}

fn find_top_level_less_than_or_equal_operator(
    src: &str,
    start: usize,
    end: usize,
) -> Option<usize> {
    find_top_level_binary_pair_operator_without_angle_groups(src, start, end, "<=")
}

fn find_top_level_greater_than_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_binary_operator_without_angle_groups(src, start, end, '>')
}

fn find_top_level_greater_than_or_equal_operator(
    src: &str,
    start: usize,
    end: usize,
) -> Option<usize> {
    find_top_level_binary_pair_operator_without_angle_groups(src, start, end, ">=")
}

fn find_top_level_strict_equal_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_binary_pair_operator_without_angle_groups(src, start, end, "===")
}

fn find_top_level_strict_not_equal_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_binary_pair_operator_without_angle_groups(src, start, end, "!==")
}

fn find_top_level_logical_and_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_binary_pair_operator_without_angle_groups(src, start, end, "&&")
}

fn find_top_level_logical_or_operator(src: &str, start: usize, end: usize) -> Option<usize> {
    find_top_level_binary_pair_operator_without_angle_groups(src, start, end, "||")
}

fn has_top_level_binary_operator_after_prefix(src: &str) -> bool {
    let start = 1;
    let end = src.len();
    find_top_level_add_operator(src, start, end).is_some()
        || find_top_level_sub_operator(src, start, end).is_some()
        || find_top_level_nullish_coalesce_operator(src, start, end).is_some()
        || find_top_level_pow_operator(src, start, end).is_some()
        || find_top_level_mul_operator(src, start, end).is_some()
        || find_top_level_div_operator(src, start, end).is_some()
        || find_top_level_rem_operator(src, start, end).is_some()
        || find_top_level_less_than_or_equal_operator(src, start, end).is_some()
        || find_top_level_less_than_operator(src, start, end).is_some()
        || find_top_level_greater_than_or_equal_operator(src, start, end).is_some()
        || find_top_level_greater_than_operator(src, start, end).is_some()
        || find_top_level_strict_equal_operator(src, start, end).is_some()
        || find_top_level_strict_not_equal_operator(src, start, end).is_some()
        || find_top_level_bitwise_and_operator(src, start, end).is_some()
        || find_top_level_bitwise_or_operator(src, start, end).is_some()
        || find_top_level_bitwise_xor_operator(src, start, end).is_some()
        || find_top_level_shift_left_operator(src, start, end).is_some()
        || find_top_level_shift_right_operator(src, start, end).is_some()
        || find_top_level_logical_and_operator(src, start, end).is_some()
        || find_top_level_logical_or_operator(src, start, end).is_some()
}

fn find_top_level_single_char_operator_without_angle_groups(
    src: &str,
    start: usize,
    end: usize,
    operator: char,
) -> Option<usize> {
    let mut found = None;
    let mut idx = start;
    let mut quote = None;
    while idx < end {
        let ch = src[idx..end].chars().next()?;
        if let Some(q) = quote {
            if ch == '\\' {
                idx += ch.len_utf8();
                if idx < end {
                    idx += src[idx..end].chars().next()?.len_utf8();
                }
                continue;
            }
            if ch == q {
                quote = None;
            }
            idx += ch.len_utf8();
            continue;
        }
        if ch == '"' || ch == '\'' || ch == '`' {
            quote = Some(ch);
            idx += ch.len_utf8();
            continue;
        }
        if ch == '(' || ch == '[' || ch == '{' {
            let close_ch = match ch {
                '(' => ')',
                '[' => ']',
                '{' => '}',
                _ => unreachable!(),
            };
            idx = find_matching(src, idx, ch, close_ch)? + 1;
            continue;
        }
        if ch == operator {
            let width = ch.len_utf8();
            let right_start = idx + width;
            let left_is_same_operator = idx > start && src[..idx].ends_with(operator);
            if src[right_start..end].starts_with(operator)
                || src[right_start..end].starts_with('=')
                || left_is_same_operator
            {
                idx += width;
                continue;
            }
            let left = src[start..idx].trim();
            let right = src[right_start..end].trim();
            if !left.is_empty() && !right.is_empty() {
                found = Some(idx);
            }
        }
        idx += ch.len_utf8();
    }
    found
}

fn find_top_level_binary_pair_operator_without_angle_groups(
    src: &str,
    start: usize,
    end: usize,
    operator: &str,
) -> Option<usize> {
    let mut found = None;
    let mut idx = start;
    let mut quote = None;
    while idx < end {
        let ch = src[idx..end].chars().next()?;
        if let Some(q) = quote {
            if ch == '\\' {
                idx += ch.len_utf8();
                if idx < end {
                    idx += src[idx..end].chars().next()?.len_utf8();
                }
                continue;
            }
            if ch == q {
                quote = None;
            }
            idx += ch.len_utf8();
            continue;
        }
        if ch == '"' || ch == '\'' || ch == '`' {
            quote = Some(ch);
            idx += ch.len_utf8();
            continue;
        }
        if ch == '(' || ch == '[' || ch == '{' {
            let close_ch = match ch {
                '(' => ')',
                '[' => ']',
                '{' => '}',
                _ => unreachable!(),
            };
            idx = find_matching(src, idx, ch, close_ch)? + 1;
            continue;
        }
        if src[idx..end].starts_with(operator) {
            let right_start = idx + operator.len();
            let left = src[start..idx].trim();
            let right = src[right_start..end].trim();
            if !left.is_empty() && !right.is_empty() {
                found = Some(idx);
            }
        }
        idx += ch.len_utf8();
    }
    found
}

fn find_top_level_binary_operator(
    src: &str,
    start: usize,
    end: usize,
    operator: char,
) -> Option<usize> {
    let mut found = None;
    let mut idx = start;
    let mut quote = None;
    while idx < end {
        let ch = src[idx..end].chars().next()?;
        if let Some(q) = quote {
            if ch == '\\' {
                idx += ch.len_utf8();
                if idx < end {
                    idx += src[idx..end].chars().next()?.len_utf8();
                }
                continue;
            }
            if ch == q {
                quote = None;
            }
            idx += ch.len_utf8();
            continue;
        }
        if ch == '"' || ch == '\'' || ch == '`' {
            quote = Some(ch);
            idx += ch.len_utf8();
            continue;
        }
        if ch == '<' || ch == '(' || ch == '[' || ch == '{' {
            let close_ch = match ch {
                '<' => '>',
                '(' => ')',
                '[' => ']',
                '{' => '}',
                _ => unreachable!(),
            };
            idx = find_matching(src, idx, ch, close_ch)? + 1;
            continue;
        }
        if ch == operator {
            let left = src[start..idx].trim();
            let right = src[idx + ch.len_utf8()..end].trim();
            if !left.is_empty() && !right.is_empty() {
                found = Some(idx);
            }
        }
        idx += ch.len_utf8();
    }
    found
}

fn find_top_level_binary_operator_without_angle_groups(
    src: &str,
    start: usize,
    end: usize,
    operator: char,
) -> Option<usize> {
    let mut found = None;
    let mut idx = start;
    let mut quote = None;
    while idx < end {
        let ch = src[idx..end].chars().next()?;
        if let Some(q) = quote {
            if ch == '\\' {
                idx += ch.len_utf8();
                if idx < end {
                    idx += src[idx..end].chars().next()?.len_utf8();
                }
                continue;
            }
            if ch == q {
                quote = None;
            }
            idx += ch.len_utf8();
            continue;
        }
        if ch == '"' || ch == '\'' || ch == '`' {
            quote = Some(ch);
            idx += ch.len_utf8();
            continue;
        }
        if ch == '(' || ch == '[' || ch == '{' {
            let close_ch = match ch {
                '(' => ')',
                '[' => ']',
                '{' => '}',
                _ => unreachable!(),
            };
            idx = find_matching(src, idx, ch, close_ch)? + 1;
            continue;
        }
        if ch == operator {
            let right_start = idx + ch.len_utf8();
            if src[right_start..end].starts_with('=') || src[right_start..end].starts_with('<') {
                idx += ch.len_utf8();
                continue;
            }
            let left = src[start..idx].trim();
            let right = src[right_start..end].trim();
            if !left.is_empty() && !right.is_empty() {
                found = Some(idx);
            }
        }
        idx += ch.len_utf8();
    }
    found
}

fn find_top_level_last_char(src: &str, start: usize, end: usize, needle: char) -> Option<usize> {
    let mut found = None;
    let mut idx = start;
    while idx < end {
        let ch = src[idx..end].chars().next()?;
        if ch == '<' || ch == '(' || ch == '{' {
            let close_ch = match ch {
                '<' => '>',
                '(' => ')',
                '{' => '}',
                _ => unreachable!(),
            };
            idx = find_matching(src, idx, ch, close_ch)? + 1;
            continue;
        }
        if ch == needle {
            found = Some(idx);
        }
        idx += ch.len_utf8();
    }
    found
}

fn find_conditional_colon(src: &str, start: usize, end: usize) -> Option<usize> {
    let mut idx = start;
    let mut conditional_depth = 0usize;
    while idx < end {
        let ch = src[idx..end].chars().next()?;
        if ch == '<' || ch == '(' || ch == '[' || ch == '{' {
            let close_ch = match ch {
                '<' => '>',
                '(' => ')',
                '[' => ']',
                '{' => '}',
                _ => unreachable!(),
            };
            idx = find_matching(src, idx, ch, close_ch)? + 1;
            continue;
        }
        if ch == '?' {
            conditional_depth += 1;
        } else if ch == ':' {
            if conditional_depth == 0 {
                return Some(idx);
            }
            conditional_depth -= 1;
        }
        idx += ch.len_utf8();
    }
    None
}

fn split_top_level(src: &str, start: usize, end: usize, delimiter: char) -> Vec<(usize, usize)> {
    let mut parts = Vec::new();
    let mut part_start = start;
    let mut idx = start;
    while idx < end {
        let ch = match src[idx..end].chars().next() {
            Some(ch) => ch,
            None => break,
        };
        if ch == '<' || ch == '(' || ch == '[' || ch == '{' {
            let close_ch = match ch {
                '<' => '>',
                '(' => ')',
                '[' => ']',
                '{' => '}',
                _ => unreachable!(),
            };
            let Some(close) = find_matching(src, idx, ch, close_ch) else {
                break;
            };
            idx = close + 1;
            continue;
        }
        if ch == delimiter {
            push_trimmed_type_part(src, part_start, idx, &mut parts);
            part_start = idx + ch.len_utf8();
        }
        idx += ch.len_utf8();
    }
    push_trimmed_type_part(src, part_start, end, &mut parts);
    parts
}

fn split_type_members(src: &str, start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut parts = Vec::new();
    let mut part_start = start;
    let mut idx = start;
    while idx < end {
        let ch = match src[idx..end].chars().next() {
            Some(ch) => ch,
            None => break,
        };
        if ch == '<' || ch == '(' || ch == '[' || ch == '{' {
            let close_ch = match ch {
                '<' => '>',
                '(' => ')',
                '[' => ']',
                '{' => '}',
                _ => unreachable!(),
            };
            let Some(close) = find_matching(src, idx, ch, close_ch) else {
                break;
            };
            idx = close + 1;
            continue;
        }
        if ch == ';' || ch == ',' || ch == '\n' {
            push_trimmed_type_part(src, part_start, idx, &mut parts);
            part_start = idx + ch.len_utf8();
        }
        idx += ch.len_utf8();
    }
    push_trimmed_type_part(src, part_start, end, &mut parts);
    parts
}

fn push_trimmed_type_part(src: &str, start: usize, end: usize, parts: &mut Vec<(usize, usize)>) {
    let raw = &src[start..end];
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }
    let leading_ws = raw.find(trimmed).unwrap_or(0);
    parts.push((start + leading_ws, start + leading_ws + trimmed.len()));
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"boundary default = secure

compartment Core boundary(secure) {
  import { getUser } from "./legacy.js" boundary(debug)

  export function process<T extends User>(
    id: number,
    value: T
  ): T extends User ? Result | unknown : unknown
    boundary(weaken to debug, skip return validation) {
    return getUser(id)
  }
}
"#;

    #[test]
    fn emits_minimal_source_unit_envelope() {
        let unit = parse_source_unit(MINIMAL);
        assert_eq!(unit.source_kind, SourceKind::NativeCruftScript);
        assert_eq!(unit.span, Span::new(0, MINIMAL.len()));
        assert!(unit.diagnostics.is_empty());
        assert!(matches!(
            unit.items.first(),
            Some(SourceItem::BoundaryDefault(default)) if default.policy_name == "secure"
        ));

        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        assert_eq!(compartment.name, "Core");
        assert!(matches!(
            compartment.boundary_clause.as_ref().and_then(|b| b.directives.first()),
            Some(BoundaryDirective::Policy(policy)) if policy == "secure"
        ));
        assert_eq!(compartment.body_items.len(), 2);
    }

    #[test]
    fn emits_boundary_import_and_exported_generic_function() {
        let unit = parse_source_unit(MINIMAL);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let import = match &compartment.body_items[0] {
            CompartmentItem::Import(import) => import,
            _ => panic!("expected import"),
        };
        assert_eq!(import.imported_names, vec!["getUser"]);
        assert_eq!(import.source, "./legacy.js");
        assert!(matches!(
            import.boundary_clause.as_ref().and_then(|b| b.directives.first()),
            Some(BoundaryDirective::Policy(policy)) if policy == "debug"
        ));

        let function = match &compartment.body_items[1] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        assert_eq!(function.name, "process");
        assert_eq!(function.type_params[0].name, "T");
        assert_eq!(function.params.len(), 2);
        assert!(matches!(
            function.boundary_clause.as_ref().map(|b| &b.directives[..]),
            Some([BoundaryDirective::WeakenTo(policy), BoundaryDirective::SkipReturnValidation]) if policy == "debug"
        ));
        assert!(matches!(
            function.return_type.as_ref().map(|ty| &ty.kind),
            Some(TypeExprKind::Conditional { .. })
        ));
    }

    #[test]
    fn parses_import_boundary_clause_after_parent_path_specifier() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  import { legacy } from "../module/js-boundary-secure/legacy.js" boundary(debug)
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let import = match &compartment.body_items[0] {
            CompartmentItem::Import(import) => import,
            _ => panic!("expected import"),
        };
        assert_eq!(import.source, "../module/js-boundary-secure/legacy.js");
        assert!(matches!(
            import.boundary_clause.as_ref().and_then(|b| b.directives.first()),
            Some(BoundaryDirective::Policy(policy)) if policy == "debug"
        ));
    }

    #[test]
    fn diagnoses_import_side_skip_return_validation() {
        let src = r#"boundary default = secure
compartment Bad boundary(secure) {
  import { bad } from "./legacy.js" boundary(skip return validation)
}
"#;
        let unit = parse_source_unit(src);
        assert!(unit.diagnostics.iter().any(|diag| {
            diag.code == DiagnosticCode::SkipReturnValidationOnImport
                && src[diag.span.start..diag.span.end].contains("skip return validation")
        }));
    }

    #[test]
    fn diagnoses_malformed_boundary_directive() {

        let src = r#"boundary default = secure
compartment X boundary(???) {
  export function main(): string { return "x" }
}
"#;
        let unit = parse_source_unit(src);
        assert!(
            unit.diagnostics
                .iter()
                .any(|diag| diag.code == DiagnosticCode::MalformedBoundaryDirective),
            "malformed boundary(???) was not diagnosed"
        );
    }

    #[test]
    fn valid_boundary_policy_not_diagnosed_as_malformed() {

        let src = r#"boundary default = secure
compartment Core boundary(secure) {
  export function main(): string boundary(secure) { return "ok" }
}
"#;
        let unit = parse_source_unit(src);
        assert!(
            !unit
                .diagnostics
                .iter()
                .any(|diag| diag.code == DiagnosticCode::MalformedBoundaryDirective),
            "valid boundary(secure) wrongly flagged malformed"
        );
    }

    #[test]
    fn emits_named_boundary_policy_block() {
        let src = r#"boundary default = secure

boundary secure = {
  at process { install: wrapper, mode: strict }
  at call {
    on violation(expected: unknown, received: unknown) {
      throw new CruftTypeError("boundary")
    }
  }
}

compartment Core boundary(secure) {
  export function main(): string boundary(secure) {
    return "ok"
  }
}
"#;
        let unit = parse_source_unit(src);
        assert!(unit.diagnostics.is_empty(), "{:#?}", unit.diagnostics);
        let policy = match &unit.items[1] {
            SourceItem::BoundaryPolicy(policy) => policy,
            _ => panic!("expected boundary policy block"),
        };
        assert_eq!(policy.name, "secure");
        assert!(policy.has_process_clause);
        assert!(policy.has_call_clause);
        assert_eq!(policy.process_mode, Some(BoundaryPolicyProcessMode::Strict));
        assert!(src[policy.body_span.start..policy.body_span.end].contains("at process"));
    }

    #[test]
    fn emits_sanitize_boundary_policy_defaults() {
        let src = r#"boundary tolerant = {
  at process { install: wrapper, mode: sanitize }
  at sanitize {
    default string = ""
    default number = 0
    default boolean = false
    default null = null
    default undefined = undefined
    default Array<string> = []
    default User = { name: "", role: "" }
  }
}

type User = { name: string; role: string };

compartment Core boundary(tolerant) {
  export function main(): string boundary(tolerant) {
    return "ok"
  }
}
"#;
        let unit = parse_source_unit(src);
        assert!(unit.diagnostics.is_empty(), "{:#?}", unit.diagnostics);
        let policy = match &unit.items[0] {
            SourceItem::BoundaryPolicy(policy) => policy,
            _ => panic!("expected boundary policy block"),
        };
        assert_eq!(
            policy.process_mode,
            Some(BoundaryPolicyProcessMode::Sanitize)
        );
        assert_eq!(policy.sanitizer_defaults.len(), 7);
        assert!(matches!(
            policy.sanitizer_defaults[0].target_type.kind,
            TypeExprKind::Named(ref name) if name == "string"
        ));
        assert!(matches!(
            policy.sanitizer_defaults[0].expr,
            SanitizerDefaultExpr::StringLiteral(ref value) if value.is_empty()
        ));
        assert!(matches!(
            policy.sanitizer_defaults[3].expr,
            SanitizerDefaultExpr::Null
        ));
        assert!(matches!(
            policy.sanitizer_defaults[4].expr,
            SanitizerDefaultExpr::Undefined
        ));
        assert!(matches!(
            policy.sanitizer_defaults[5].target_type.kind,
            TypeExprKind::TypeRef { ref name, .. } if name == "Array"
        ));
        assert!(matches!(
            policy.sanitizer_defaults[6].expr,
            SanitizerDefaultExpr::ObjectLiteral(_)
        ));
    }

    #[test]
    fn emits_top_level_type_alias_envelope() {
        let src = r#"boundary default = secure
type Box<T extends User> = T | unknown;

compartment Core boundary(secure) {
  export function process(input: Box): Box boundary(secure) {
    return input
  }
}
"#;
        let unit = parse_source_unit(src);
        let alias = match &unit.items[1] {
            SourceItem::TypeAlias(alias) => alias,
            _ => panic!("expected top-level type alias"),
        };
        assert_eq!(alias.name, "Box");
        assert_eq!(alias.type_params[0].name, "T");
        assert!(matches!(
            alias.type_params[0].constraint.as_ref().map(|ty| &ty.kind),
            Some(TypeExprKind::Named(name)) if name == "User"
        ));
        assert!(matches!(alias.aliased_type.kind, TypeExprKind::Union(_)));
    }

    #[test]
    fn emits_compartment_type_alias_envelope() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  type Result<T> = T extends User ? T : unknown;
  export function process(input: Result): Result boundary(secure) {
    return input
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let alias = match &compartment.body_items[0] {
            CompartmentItem::TypeAlias(alias) => alias,
            _ => panic!("expected compartment type alias"),
        };
        assert_eq!(alias.name, "Result");
        assert_eq!(alias.type_params[0].name, "T");
        assert!(matches!(
            alias.aliased_type.kind,
            TypeExprKind::Conditional { .. }
        ));
    }

    #[test]
    fn emits_mapped_type_alias_envelope() {
        let src = r#"boundary default = secure
type PartialUser<T> = { +readonly [K in keyof T]+?: T };

compartment Core boundary(secure) {
  export function process(input: PartialUser<User>): PartialUser<User> boundary(secure) {
    return input
  }
}
"#;
        let unit = parse_source_unit(src);
        let alias = match &unit.items[1] {
            SourceItem::TypeAlias(alias) => alias,
            _ => panic!("expected top-level type alias"),
        };
        let TypeExprKind::Mapped(mapped) = &alias.aliased_type.kind else {
            panic!("expected mapped type");
        };
        assert_eq!(mapped.key_name, "K");
        assert_eq!(mapped.readonly_modifier, MappedModifier::Add);
        assert_eq!(mapped.optional_modifier, MappedModifier::Add);
        let MappedKeyConstraint::Keyof { target } = &mapped.key_constraint else {
            panic!("expected keyof constraint");
        };
        assert!(matches!(target.kind, TypeExprKind::Named(ref name) if name == "T"));
        assert!(matches!(mapped.value_type.kind, TypeExprKind::Named(ref name) if name == "T"));
    }

    #[test]
    fn emits_object_and_indexed_access_type_envelopes() {
        let src = r#"boundary default = secure
type UserShape = { readonly id: number; name?: string };

compartment Core boundary(secure) {
  type UserName = UserShape[name];
  export function process(input: UserName): UserName boundary(secure) {
    return input
  }
}
"#;
        let unit = parse_source_unit(src);
        let object_alias = match &unit.items[1] {
            SourceItem::TypeAlias(alias) => alias,
            _ => panic!("expected object type alias"),
        };
        let TypeExprKind::Object(object) = &object_alias.aliased_type.kind else {
            panic!("expected object type");
        };
        assert_eq!(object.properties.len(), 2);
        assert_eq!(object.properties[0].name, "id");
        assert!(object.properties[0].readonly);
        assert!(!object.properties[0].optional);
        assert_eq!(object.properties[1].name, "name");
        assert!(object.properties[1].optional);

        let compartment = match &unit.items[2] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let indexed_alias = match &compartment.body_items[0] {
            CompartmentItem::TypeAlias(alias) => alias,
            _ => panic!("expected indexed alias"),
        };
        let TypeExprKind::IndexedAccess { object, index } = &indexed_alias.aliased_type.kind else {
            panic!("expected indexed access");
        };
        assert!(matches!(object.kind, TypeExprKind::Named(ref name) if name == "UserShape"));
        assert!(matches!(index.kind, TypeExprKind::Named(ref name) if name == "name"));
    }

    #[test]
    fn emits_multiline_object_type_alias_envelope() {
        let src = r#"boundary default = secure
type Profile = {
  name: string
}

compartment Core boundary(secure) {
  export function process(input: Profile): string boundary(secure) {
    return input.name
  }
}
"#;
        let unit = parse_source_unit(src);
        let alias = match &unit.items[1] {
            SourceItem::TypeAlias(alias) => alias,
            _ => panic!("expected top-level type alias"),
        };
        let TypeExprKind::Object(object) = &alias.aliased_type.kind else {
            panic!("expected object type");
        };
        assert_eq!(object.properties.len(), 1);
        assert_eq!(object.properties[0].name, "name");
        assert!(matches!(
            object.properties[0].ty.kind,
            TypeExprKind::Named(ref name) if name == "string"
        ));
    }

    #[test]
    fn emits_multiline_object_type_alias_multiple_newline_members() {
        let src = r#"boundary default = secure
type User = {
  pair: [string, number]
  scores: Record<string, number>
}

compartment Core boundary(secure) {
  export function process(input: User): string boundary(secure) {
    return input.pair[0]
  }
}
"#;
        let unit = parse_source_unit(src);
        let alias = match &unit.items[1] {
            SourceItem::TypeAlias(alias) => alias,
            _ => panic!("expected top-level type alias"),
        };
        let TypeExprKind::Object(object) = &alias.aliased_type.kind else {
            panic!("expected object type");
        };
        assert_eq!(object.properties.len(), 2);
        assert_eq!(object.properties[0].name, "pair");
        assert!(matches!(
            object.properties[0].ty.kind,
            TypeExprKind::Tuple(ref elements) if elements.len() == 2
        ));
        assert_eq!(object.properties[1].name, "scores");
        assert!(matches!(
            object.properties[1].ty.kind,
            TypeExprKind::TypeRef { ref name, .. } if name == "Record"
        ));
    }

    #[test]
    fn emits_multiple_top_level_type_aliases_in_source_order() {
        let src = r#"boundary default = secure
type UserShape = { readonly id: number; name?: string };
type Picked = Pick<UserShape, id>;

compartment Core boundary(secure) {
  export function process(input: Picked): Picked boundary(secure) {
    return input
  }
}
"#;
        let unit = parse_source_unit(src);

        assert!(matches!(unit.items[0], SourceItem::BoundaryDefault(_)));
        let first = match &unit.items[1] {
            SourceItem::TypeAlias(alias) => alias,
            _ => panic!("expected first top-level type alias"),
        };
        let second = match &unit.items[2] {
            SourceItem::TypeAlias(alias) => alias,
            _ => panic!("expected second top-level type alias"),
        };
        assert_eq!(first.name, "UserShape");
        assert_eq!(second.name, "Picked");
        assert!(first.span.end < second.span.start);
        assert!(matches!(
            second.aliased_type.kind,
            TypeExprKind::TypeRef { ref name, ref type_args }
                if name == "Pick" && type_args.len() == 2
        ));
        assert!(matches!(unit.items[3], SourceItem::Compartment(_)));
    }

    #[test]
    fn function_params_do_not_split_type_argument_commas() {
        let src = r#"boundary default = secure
type UserShape = { readonly id: number; name?: string };

compartment Core boundary(secure) {
  export function process(input: Pick<UserShape, id>, fallback: unknown): Pick<UserShape, id> boundary(secure) {
    return input
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[2] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected exported function"),
        };

        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].name, "input");
        let TypeExprKind::TypeRef { name, type_args } = &function.params[0].ty.kind else {
            panic!("expected Pick type ref");
        };
        assert_eq!(name, "Pick");
        assert_eq!(type_args.len(), 2);
        assert!(matches!(type_args[0].kind, TypeExprKind::Named(ref name) if name == "UserShape"));
        assert!(matches!(type_args[1].kind, TypeExprKind::Named(ref name) if name == "id"));
        assert_eq!(function.params[1].name, "fallback");
    }

    #[test]
    fn function_params_preserve_union_key_type_argument() {
        let src = r#"boundary default = secure
type UserShape = { readonly id: number; name?: string };

compartment Core boundary(secure) {
  export function process(input: Pick<UserShape, id | name>): Pick<UserShape, id | name> boundary(secure) {
    return input
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[2] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected exported function"),
        };
        let TypeExprKind::TypeRef { name, type_args } = &function.params[0].ty.kind else {
            panic!("expected Pick type ref");
        };

        assert_eq!(name, "Pick");
        assert_eq!(type_args.len(), 2);
        let TypeExprKind::Union(members) = &type_args[1].kind else {
            panic!("expected union key set");
        };
        assert_eq!(members.len(), 2);
        assert!(matches!(members[0].kind, TypeExprKind::Named(ref name) if name == "id"));
        assert!(matches!(members[1].kind, TypeExprKind::Named(ref name) if name == "name"));
    }

    #[test]
    fn emits_multiple_compartment_items_in_source_order() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  import { getUser } from "./legacy.js" boundary(debug)
  type Result<T> = T | unknown;
  export function id<T>(value: T): T boundary(secure) {
    if (value) {
      return value
    }
    return value
  }
  export function process(input: string): string boundary(secure) {
    return id(input)
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };

        assert_eq!(compartment.body_items.len(), 4);
        assert!(matches!(
            compartment.body_items[0],
            CompartmentItem::Import(_)
        ));
        assert!(matches!(
            compartment.body_items[1],
            CompartmentItem::TypeAlias(_)
        ));
        let id = match &compartment.body_items[2] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected id function"),
        };
        let process = match &compartment.body_items[3] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected process function"),
        };

        assert_eq!(id.name, "id");
        assert_eq!(process.name, "process");
        assert!(id.span.start < id.span.end);
        assert!(id.span.end < process.span.start);
        assert!(process.span.end <= compartment.body_span.end);
        assert_eq!(id.body.statements.len(), 2);
        assert!(matches!(id.body.statements[0], BodyStmt::If(_)));
    }

    #[test]
    fn emits_boundary_qualified_function_type_alias() {
        let src = r#"boundary default = secure
type Handler<T extends User> = (input: T, fallback: unknown) => Result boundary(secure);

compartment Core boundary(secure) {
  export function process(input: Handler): Handler boundary(secure) {
    return input
  }
}
"#;
        let unit = parse_source_unit(src);
        let alias = match &unit.items[1] {
            SourceItem::TypeAlias(alias) => alias,
            _ => panic!("expected top-level type alias"),
        };
        assert_eq!(alias.type_params[0].name, "T");
        let TypeExprKind::Function {
            type_params,
            params,
            return_type,
            boundary_clause,
        } = &alias.aliased_type.kind
        else {
            panic!("expected function type");
        };
        assert!(type_params.is_empty());
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name.as_deref(), Some("input"));
        assert!(matches!(return_type.kind, TypeExprKind::Named(ref name) if name == "Result"));
        assert!(matches!(
            boundary_clause.as_ref().and_then(|clause| clause.directives.first()),
            Some(BoundaryDirective::Policy(policy)) if policy == "secure"
        ));
    }

    #[test]
    fn emits_generic_function_type_alias() {
        let src = r#"boundary default = secure
type Mapper = <T extends User>(input: T) => T;

compartment Core boundary(secure) {
  export function process(input: Mapper): Mapper boundary(secure) {
    return input
  }
}
"#;
        let unit = parse_source_unit(src);
        let alias = match &unit.items[1] {
            SourceItem::TypeAlias(alias) => alias,
            _ => panic!("expected top-level type alias"),
        };
        let TypeExprKind::Function {
            type_params,
            params,
            return_type,
            boundary_clause,
        } = &alias.aliased_type.kind
        else {
            panic!("expected function type");
        };
        assert_eq!(type_params[0].name, "T");
        assert_eq!(params[0].name.as_deref(), Some("input"));
        assert!(matches!(return_type.kind, TypeExprKind::Named(ref name) if name == "T"));
        assert!(boundary_clause.is_none());
    }

    #[test]
    fn emits_tuple_constructor_and_explicit_this_type_aliases() {
        let src = r#"boundary default = secure
type Args = [string, number]
type Factory = abstract new (id: string, count: number) => User
type Handler = (this: User, input: string) => number

compartment Core boundary(secure) {
  export function process(input: string): string boundary(secure) {
    return input
  }
}
"#;
        let unit = parse_source_unit(src);
        let args = match &unit.items[1] {
            SourceItem::TypeAlias(alias) => alias,
            _ => panic!("expected tuple type alias"),
        };
        let TypeExprKind::Tuple(elements) = &args.aliased_type.kind else {
            panic!("expected tuple type");
        };
        assert_eq!(elements.len(), 2);
        assert!(matches!(elements[0].kind, TypeExprKind::Named(ref name) if name == "string"));
        assert!(matches!(elements[1].kind, TypeExprKind::Named(ref name) if name == "number"));

        let factory = match &unit.items[2] {
            SourceItem::TypeAlias(alias) => alias,
            _ => panic!("expected constructor type alias"),
        };
        let TypeExprKind::Constructor {
            params,
            instance_type,
            is_abstract,
        } = &factory.aliased_type.kind
        else {
            panic!("expected constructor type");
        };
        assert!(*is_abstract);
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name.as_deref(), Some("id"));
        assert!(matches!(instance_type.kind, TypeExprKind::Named(ref name) if name == "User"));

        let handler = match &unit.items[3] {
            SourceItem::TypeAlias(alias) => alias,
            _ => panic!("expected handler type alias"),
        };
        let TypeExprKind::Function { params, .. } = &handler.aliased_type.kind else {
            panic!("expected function type");
        };
        assert_eq!(params[0].name.as_deref(), Some("this"));
        assert!(matches!(params[0].ty.kind, TypeExprKind::Named(ref name) if name == "User"));
    }

    #[test]
    fn emits_exported_function_body_envelope_for_narrowing() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function process(input: unknown): string boundary(secure) {
    const raw: unknown = input
    if (typeof raw === "string") {
      return raw
    }
    return "fallback"
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        assert_eq!(function.body_span, function.body.span);
        assert_eq!(function.body.statements.len(), 3);

        let BodyStmt::LocalDecl(local) = &function.body.statements[0] else {
            panic!("expected local decl");
        };
        assert_eq!(local.name, "raw");
        assert!(matches!(
            local.type_annotation.as_ref().map(|ty| &ty.kind),
            Some(TypeExprKind::Named(name)) if name == "unknown"
        ));
        assert!(matches!(
            local.initializer.as_ref().map(|expr| &expr.kind),
            Some(ExprEnvelopeKind::Identifier(name)) if name == "input"
        ));

        let BodyStmt::If(if_stmt) = &function.body.statements[1] else {
            panic!("expected if statement");
        };
        assert!(matches!(
            &if_stmt.subject.kind,
            ExprEnvelopeKind::Identifier(name) if name == "raw"
        ));
        assert!(matches!(
            &if_stmt.guard,
            NarrowingGuard::TypeofEquals { expected, .. } if expected == "string"
        ));
        assert!(if_stmt.else_body.is_none());

        let BodyStmt::Return(first_return) = &if_stmt.then_body.statements[0] else {
            panic!("expected first return");
        };
        assert!(matches!(
            first_return.argument.as_ref().map(|expr| &expr.kind),
            Some(ExprEnvelopeKind::Identifier(name)) if name == "raw"
        ));

        let BodyStmt::Return(second_return) = &function.body.statements[2] else {
            panic!("expected second return");
        };
        assert!(matches!(
            second_return.argument.as_ref().map(|expr| &expr.kind),
            Some(ExprEnvelopeKind::StringLiteral(value)) if value == "fallback"
        ));
    }

    #[test]
    fn emits_bounded_body_expression_envelopes() {
        let src = r#"boundary default = secure
type Shape = { name: string };

compartment Core boundary(secure) {
  export function process(input: Shape): string boundary(secure) {
    const object = { name: input.name, active: true }
    const list = [object.name, "fallback"]
    return object.name
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[2] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::LocalDecl(object_local) = &function.body.statements[0] else {
            panic!("expected object local");
        };
        let Some(ExprEnvelopeKind::ObjectLiteral(properties)) =
            object_local.initializer.as_ref().map(|expr| &expr.kind)
        else {
            panic!("expected object literal");
        };
        assert_eq!(properties.len(), 2);
        assert_eq!(properties[0].name, "name");
        assert!(matches!(
            properties[0].value.kind,
            ExprEnvelopeKind::PropertyAccess {
                ref property,
                optional: false,
                ..
            } if property == "name"
        ));
        assert!(matches!(
            properties[1].value.kind,
            ExprEnvelopeKind::BooleanLiteral(true)
        ));

        let BodyStmt::LocalDecl(list_local) = &function.body.statements[1] else {
            panic!("expected list local");
        };
        let Some(ExprEnvelopeKind::ArrayLiteral(elements)) =
            list_local.initializer.as_ref().map(|expr| &expr.kind)
        else {
            panic!("expected array literal");
        };
        assert_eq!(elements.len(), 2);

        let BodyStmt::Return(ret) = &function.body.statements[2] else {
            panic!("expected return");
        };
        assert!(matches!(
            ret.argument.as_ref().map(|expr| &expr.kind),
            Some(ExprEnvelopeKind::PropertyAccess {
                property,
                optional: false,
                ..
            }) if property == "name"
        ));
    }

    #[test]
    fn emits_optional_element_access_expression_envelope() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function main(): unknown boundary(secure) {
    const items: Array<string> = ["alpha", "beta"]
    return items?.[0]
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Return(ret) = &function.body.statements[1] else {
            panic!("expected return");
        };
        assert!(matches!(
            ret.argument.as_ref().map(|expr| &expr.kind),
            Some(ExprEnvelopeKind::ElementAccess {
                optional: true,
                index,
                ..
            }) if matches!(index.kind, ExprEnvelopeKind::NumberLiteral(ref value) if value == "0")
        ));
    }

    #[test]
    fn emits_optional_direct_call_expression_envelope() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function helper(): string boundary(secure) {
    return "called"
  }

  export function main(): unknown boundary(secure) {
    return helper?.()
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[1] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Return(ret) = &function.body.statements[0] else {
            panic!("expected return");
        };
        assert!(matches!(
            ret.argument.as_ref().map(|expr| &expr.kind),
            Some(ExprEnvelopeKind::Call {
                optional: true,
                callee,
                args,
            }) if args.is_empty() && matches!(callee.kind, ExprEnvelopeKind::Identifier(ref name) if name == "helper")
        ));
    }

    #[test]
    fn cooks_newline_escape_in_string_literal_expression() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function main(a: string): string boundary(secure) {
    return a + "\n" + a
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Return(ret) = &function.body.statements[0] else {
            panic!("expected return");
        };
        let Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryAdd { right, .. },
            ..
        }) = ret.argument.as_ref()
        else {
            panic!("expected outer binary add");
        };
        assert!(matches!(
            right.kind,
            ExprEnvelopeKind::Identifier(ref name) if name == "a"
        ));
        let Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryAdd { right, .. },
            ..
        }) = ret.argument.as_ref().and_then(|expr| match &expr.kind {
            ExprEnvelopeKind::BinaryAdd { left, .. } => Some(left.as_ref()),
            _ => None,
        })
        else {
            panic!("expected inner binary add");
        };
        assert!(matches!(
            right.kind,
            ExprEnvelopeKind::StringLiteral(ref value) if value == "\n"
        ));
    }

    #[test]
    fn cooks_newline_escape_in_template_literal_segment() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function main(a: string, b: string): string boundary(secure) {
    return `${a}\n${b}`
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Return(ret) = &function.body.statements[0] else {
            panic!("expected return");
        };
        fn contains_newline_literal(expr: &ExprEnvelope) -> bool {
            match &expr.kind {
                ExprEnvelopeKind::StringLiteral(value) => value == "\n",
                ExprEnvelopeKind::BinaryAdd { left, right } => {
                    contains_newline_literal(left) || contains_newline_literal(right)
                }
                _ => false,
            }
        }
        assert!(contains_newline_literal(
            ret.argument.as_ref().expect("return expression")
        ));
    }

    #[test]
    fn template_literal_unicode_before_interpolation_does_not_panic() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function main(name: string): string boundary(secure) {
    return `label — ${name}`
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Return(ret) = &function.body.statements[0] else {
            panic!("expected return");
        };

        assert!(matches!(
            ret.argument.as_ref().map(|expr| &expr.kind),
            Some(ExprEnvelopeKind::BinaryAdd { .. })
        ));
    }

    #[test]
    fn multiline_template_literal_return_is_single_statement() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function main(name: string): string boundary(secure) {
    return `alpha
${name}
omega`
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };

        assert_eq!(function.body.statements.len(), 1);
        assert!(matches!(function.body.statements[0], BodyStmt::Return(_)));
    }

    #[test]
    fn trims_return_expression_terminating_semicolon() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function capitalize(s: string): string boundary(secure) {
    return s.charAt(0).toUpperCase() + s.slice(1);
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Return(ret) = &function.body.statements[0] else {
            panic!("expected return");
        };
        let Some(ExprEnvelope {
            kind: ExprEnvelopeKind::BinaryAdd { right, .. },
            ..
        }) = ret.argument.as_ref()
        else {
            panic!("expected binary add return expression");
        };
        assert!(matches!(
            right.kind,
            ExprEnvelopeKind::Call { ref callee, .. }
                if matches!(callee.kind, ExprEnvelopeKind::PropertyAccess {
                    ref property,
                    ..
                } if property == "slice")
        ));
    }

    #[test]
    fn trims_local_initializer_terminating_semicolon() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function main(mount: string): string boundary(secure) {
    const shell = mount.replace(/^\//, "");
    return shell
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::LocalDecl(local) = &function.body.statements[0] else {
            panic!("expected local declaration");
        };

        assert!(matches!(
            local.initializer.as_ref().map(|expr| &expr.kind),
            Some(ExprEnvelopeKind::Call { .. })
        ));
    }

    #[test]
    fn parses_inline_if_return_chain() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function escape(c: string): string boundary(secure) {
    if (c === "&") return "&amp;";
    if (c === "<") return "&lt;";
    return c;
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };

        assert!(matches!(function.body.statements[0], BodyStmt::If(_)));
        assert!(matches!(function.body.statements[1], BodyStmt::If(_)));
        assert!(matches!(function.body.statements[2], BodyStmt::Return(_)));
    }

    #[test]
    fn desugars_local_compound_add_assignment() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function f(a: number): number boundary(secure) {
    let x: number = a
    x += 5
    return x
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Assignment(assignment) = &function.body.statements[1] else {
            panic!("expected compound assignment to parse as assignment");
        };
        assert!(matches!(
            assignment.target,
            AssignmentTarget::Local(ref name) if name == "x"
        ));
        assert!(matches!(
            assignment.value.kind,
            ExprEnvelopeKind::BinaryAdd { ref left, ref right }
                if matches!(left.kind, ExprEnvelopeKind::Identifier(ref name) if name == "x")
                    && matches!(right.kind, ExprEnvelopeKind::NumberLiteral(ref value) if value == "5")
        ));
    }

    #[test]
    fn parses_exponent_operator_before_multiply() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function f(a: number): number boundary(secure) {
    return a ** 2
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Return(ret) = &function.body.statements[0] else {
            panic!("expected return");
        };
        assert!(matches!(
            ret.argument.as_ref().map(|expr| &expr.kind),
            Some(ExprEnvelopeKind::BinaryPow { left, right })
                if matches!(left.kind, ExprEnvelopeKind::Identifier(ref name) if name == "a")
                    && matches!(right.kind, ExprEnvelopeKind::NumberLiteral(ref value) if value == "2")
        ));
    }

    #[test]
    fn parses_nullish_coalescing_expression() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function f(a: number): number boundary(secure) {
    return a ?? 0
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Return(ret) = &function.body.statements[0] else {
            panic!("expected return");
        };
        assert!(matches!(
            ret.argument.as_ref().map(|expr| &expr.kind),
            Some(ExprEnvelopeKind::NullishCoalesce { left, right })
                if matches!(left.kind, ExprEnvelopeKind::Identifier(ref name) if name == "a")
                    && matches!(right.kind, ExprEnvelopeKind::NumberLiteral(ref value) if value == "0")
        ));
    }

    #[test]
    fn erases_arrow_return_type_annotation_in_callback_expression() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function f(xs: Array<number>): Array<number> boundary(secure) {
    return xs.map((n: number): number => n + 1)
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Return(ret) = &function.body.statements[0] else {
            panic!("expected return");
        };
        let Some(ExprEnvelopeKind::Call { args, .. }) =
            ret.argument.as_ref().map(|expr| &expr.kind)
        else {
            panic!("expected map call");
        };
        let Some(ExprEnvelopeKind::Arrow { params, body }) = args.first().map(|expr| &expr.kind)
        else {
            panic!("expected arrow callback");
        };
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "n");
        assert!(matches!(params[0].ty.kind, TypeExprKind::Named(ref name) if name == "number"));
        assert!(matches!(
            body.kind,
            ExprEnvelopeKind::BinaryAdd { ref left, ref right }
                if matches!(left.kind, ExprEnvelopeKind::Identifier(ref name) if name == "n")
                    && matches!(right.kind, ExprEnvelopeKind::NumberLiteral(ref value) if value == "1")
        ));
    }

    #[test]
    fn desugars_array_literal_spread_to_concat_chain() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function f(xs: Array<number>): Array<number> boundary(secure) {
    return [0, ...xs, 3]
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Return(ret) = &function.body.statements[0] else {
            panic!("expected return");
        };
        let Some(ExprEnvelopeKind::Call { callee, args, .. }) =
            ret.argument.as_ref().map(|expr| &expr.kind)
        else {
            panic!("expected trailing concat call");
        };
        assert_eq!(args.len(), 1);
        assert!(
            matches!(args[0].kind, ExprEnvelopeKind::ArrayLiteral(ref elements)
                if elements.len() == 1
                    && matches!(elements[0].kind, ExprEnvelopeKind::NumberLiteral(ref value) if value == "3")
            )
        );
        assert!(matches!(
            callee.kind,
            ExprEnvelopeKind::PropertyAccess { ref property, .. } if property == "concat"
        ));
    }

    #[test]
    fn parses_default_parameter_initializer() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function f(a: number, b: number = 10): number boundary(secure) {
    return a + b
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[1].name, "b");
        assert!(matches!(
            function.params[1].ty.kind,
            TypeExprKind::Named(ref name) if name == "number"
        ));
        assert!(matches!(
            function.params[1].default.as_ref().map(|expr| &expr.kind),
            Some(ExprEnvelopeKind::NumberLiteral(value)) if value == "10"
        ));
    }

    #[test]
    fn parses_inline_switch_case_return_bodies() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function f(n: number): string boundary(secure) {
    switch (n) {
      case 1: return "a"
      case 2: return "b"
      default: return "z" }
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Switch(switch_stmt) = &function.body.statements[0] else {
            panic!("expected switch");
        };
        assert_eq!(switch_stmt.cases.len(), 2);
        assert!(matches!(
            switch_stmt.cases[1].body.statements.as_slice(),
            [BodyStmt::Return(ReturnStmt {
                argument: Some(ExprEnvelope {
                    kind: ExprEnvelopeKind::StringLiteral(value),
                    ..
                }),
                ..
            })] if value == "b"
        ));
        assert!(switch_stmt.default.as_ref().is_some_and(|body| matches!(
            body.statements.as_slice(),
            [BodyStmt::Return(ReturnStmt {
                argument: Some(ExprEnvelope {
                    kind: ExprEnvelopeKind::StringLiteral(value),
                    ..
                }),
                ..
            })] if value == "z"
        )));
    }

    #[test]
    fn distinguishes_optional_direct_call_from_optional_method_like_call() {
        let src = r#"boundary default = secure
type Shape = { method: string };

compartment Core boundary(secure) {
  export function main(): unknown boundary(secure) {
    const value: Shape = { method: "no-call" }
    return value.method?.()
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[2] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Return(ret) = &function.body.statements[1] else {
            panic!("expected return");
        };
        assert!(matches!(
            ret.argument.as_ref().map(|expr| &expr.kind),
            Some(ExprEnvelopeKind::Call {
                optional: true,
                callee,
                ..
            }) if matches!(callee.kind, ExprEnvelopeKind::PropertyAccess {
                ref property,
                optional: false,
                ..
            } if property == "method")
        ));
    }

    #[test]
    fn preserves_receiver_method_call_expression_envelope() {
        let src = r#"boundary default = secure
type Shape = { method: string };

compartment Core boundary(secure) {
  export function main(): unknown boundary(secure) {
    const value: Shape = { method: "no-call" }
    return value?.method()
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[2] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Return(ret) = &function.body.statements[1] else {
            panic!("expected return");
        };
        assert!(matches!(
            ret.argument.as_ref().map(|expr| &expr.kind),
            Some(ExprEnvelopeKind::Call {
                optional: false,
                callee,
                ..
            }) if matches!(callee.kind, ExprEnvelopeKind::PropertyAccess {
                ref property,
                optional: true,
                ..
            } if property == "method")
        ));
    }

    #[test]
    fn emits_if_else_body_envelope_for_control_flow_narrowing() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function process(input: unknown): string boundary(secure) {
    if (typeof input === "string") {
      return input
    } else {
      return "fallback"
    }
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        assert_eq!(function.body.statements.len(), 1);
        let BodyStmt::If(if_stmt) = &function.body.statements[0] else {
            panic!("expected if statement");
        };
        assert_eq!(if_stmt.then_body.statements.len(), 1);
        let else_body = if_stmt.else_body.as_ref().expect("expected else body");
        assert_eq!(else_body.statements.len(), 1);
        assert!(matches!(
            &if_stmt.subject.kind,
            ExprEnvelopeKind::Identifier(name) if name == "input"
        ));
    }

    #[test]
    fn emits_typeof_property_subject_for_conditional_expression() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function process(input: { value: string | number }): string | number boundary(secure) {
    return typeof input.value === "string" ? input.value : 7
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Return(ret) = &function.body.statements[0] else {
            panic!("expected return");
        };
        let Some(ExprEnvelope {
            kind:
                ExprEnvelopeKind::Conditional {
                    subject,
                    consequent,
                    alternate,
                    ..
                },
            ..
        }) = ret.argument.as_ref()
        else {
            panic!("expected conditional expression");
        };
        assert!(matches!(
            &subject.kind,
            ExprEnvelopeKind::PropertyAccess {
                property,
                optional: false,
                ..
            } if property == "value"
        ));
        assert!(matches!(
            &consequent.kind,
            ExprEnvelopeKind::PropertyAccess {
                property,
                optional: false,
                ..
            } if property == "value"
        ));
        assert!(matches!(&alternate.kind, ExprEnvelopeKind::NumberLiteral(value) if value == "7"));
    }

    #[test]
    fn emits_recursive_conditional_decrease_envelope() {
        let src = r#"boundary default = secure
type DeepUnwrap<T> = T extends Array<infer U> ? U extends Promise<infer V> ? DeepUnwrap<V> : DeepUnwrap<U> : T;

compartment Core boundary(secure) {
  export function process(input: DeepUnwrap): DeepUnwrap boundary(secure) {
    return input
  }
}
"#;
        let unit = parse_source_unit(src);
        let alias = match &unit.items[1] {
            SourceItem::TypeAlias(alias) => alias,
            _ => panic!("expected top-level type alias"),
        };
        let TypeExprKind::Conditional {
            check,
            extends,
            true_type,
            false_type,
        } = &alias.aliased_type.kind
        else {
            panic!("expected outer conditional");
        };
        assert!(matches!(check.kind, TypeExprKind::Named(ref name) if name == "T"));
        let TypeExprKind::TypeRef { name, type_args } = &extends.kind else {
            panic!("expected generic extends operand");
        };
        assert_eq!(name, "Array");
        assert!(matches!(
            type_args.first().map(|ty| &ty.kind),
            Some(TypeExprKind::Infer { name, constraint: None }) if name == "U"
        ));
        assert!(matches!(false_type.kind, TypeExprKind::Named(ref name) if name == "T"));

        let TypeExprKind::Conditional {
            check: nested_check,
            extends: nested_extends,
            true_type: nested_true,
            false_type: nested_false,
        } = &true_type.kind
        else {
            panic!("expected nested conditional");
        };
        assert!(matches!(nested_check.kind, TypeExprKind::Named(ref name) if name == "U"));
        let TypeExprKind::TypeRef {
            name: nested_extends_name,
            type_args: nested_extends_args,
        } = &nested_extends.kind
        else {
            panic!("expected nested generic extends operand");
        };
        assert_eq!(nested_extends_name, "Promise");
        assert!(matches!(
            nested_extends_args.first().map(|ty| &ty.kind),
            Some(TypeExprKind::Infer { name, constraint: None }) if name == "V"
        ));
        assert!(matches!(
            &nested_true.kind,
            TypeExprKind::TypeRef { name, type_args }
                if name == "DeepUnwrap"
                    && matches!(type_args.first().map(|ty| &ty.kind), Some(TypeExprKind::Named(arg)) if arg == "V")
        ));
        assert!(matches!(
            &nested_false.kind,
            TypeExprKind::TypeRef { name, type_args }
                if name == "DeepUnwrap"
                    && matches!(type_args.first().map(|ty| &ty.kind), Some(TypeExprKind::Named(arg)) if arg == "U")
        ));
    }

    #[test]
    fn emits_diagnostic_for_runtime_bearing_decorator_syntax() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  @trace
  export function main(): string boundary(secure) {
    return "decorated"
  }
}
"#;
        let unit = parse_source_unit(src);

        assert!(unit.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::UnsupportedDecorator
                && diagnostic.message.contains("runtime-bearing")
        }));
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        assert_eq!(compartment.body_items.len(), 1);
        assert!(matches!(
            compartment.body_items[0],
            CompartmentItem::ExportFunction(_)
        ));
    }

    #[test]
    fn emits_diagnostic_for_runtime_bearing_class_syntax() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export class Box {
    constructor(readonly value: string) {}
  }
  export function main(): string boundary(secure) {
    return "after-class"
  }
}
"#;
        let unit = parse_source_unit(src);

        assert!(unit.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::UnsupportedClassDeclaration
                && diagnostic.message.contains("declared data fields")
        }));
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        assert_eq!(compartment.body_items.len(), 1);
        assert!(matches!(
            compartment.body_items[0],
            CompartmentItem::ExportFunction(_)
        ));
    }

    #[test]
    fn emits_diagnostic_for_class_heritage_syntax() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export class Child extends Base {
    value: string
  }
  export function main(): string boundary(secure) {
    return "after-class"
  }
}
"#;
        let unit = parse_source_unit(src);

        assert!(unit.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::UnsupportedClassDeclaration
                && diagnostic
                    .message
                    .contains("heritage clauses require prototype")
        }));
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        assert_eq!(compartment.body_items.len(), 1);
        assert!(matches!(
            compartment.body_items[0],
            CompartmentItem::ExportFunction(_)
        ));
    }

    #[test]
    fn emits_diagnostic_for_class_implements_syntax() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  type Named = { name: string }
  export class User implements Named {
    name: string
  }
}
"#;
        let unit = parse_source_unit(src);

        assert!(unit.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::UnsupportedClassDeclaration
                && diagnostic
                    .message
                    .contains("heritage clauses require prototype")
        }));
    }

    #[test]
    fn parses_bounded_structural_class_declaration() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export class Box {
    readonly label: string;
    value?: number,
    initialized: boolean = true
  }
}
"#;
        let unit = parse_source_unit(src);

        assert!(unit.diagnostics.is_empty());
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let class_decl = match &compartment.body_items[0] {
            CompartmentItem::Class(class_decl) => class_decl,
            _ => panic!("expected class"),
        };
        assert_eq!(class_decl.name, "Box");
        assert!(class_decl.exported);
        assert_eq!(class_decl.fields.len(), 3);
        assert_eq!(class_decl.fields[0].name, "label");
        assert!(class_decl.fields[0].readonly);
        assert_eq!(class_decl.fields[1].name, "value");
        assert!(class_decl.fields[1].optional);
        assert_eq!(class_decl.fields[2].name, "initialized");
        assert!(matches!(
            class_decl.fields[2]
                .initializer
                .as_ref()
                .map(|expr| &expr.kind),
            Some(ExprEnvelopeKind::BooleanLiteral(true))
        ));
    }

    #[test]
    fn parses_static_field_initializer_call_without_method_misclassification() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export class Box {
    static slug: string = Box.prefix() + "-field"

    static prefix(): string {
      return "helper"
    }
  }
}
"#;
        let unit = parse_source_unit(src);

        assert!(unit.diagnostics.is_empty(), "{:#?}", unit.diagnostics);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let class_decl = match &compartment.body_items[0] {
            CompartmentItem::Class(class_decl) => class_decl,
            _ => panic!("expected class"),
        };
        assert_eq!(class_decl.static_fields.len(), 1);
        assert_eq!(class_decl.static_fields[0].name, "slug");
        assert_eq!(class_decl.static_methods.len(), 1);
        assert_eq!(class_decl.static_methods[0].name, "prefix");
    }

    #[test]
    fn parses_plain_class_constructor_body() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export class Box {
    label: string
    value: number
    constructor(label: string, value: number) {
      this.label = label
      this.value = value
    }
  }
}
"#;
        let unit = parse_source_unit(src);

        assert!(unit.diagnostics.is_empty(), "{:#?}", unit.diagnostics);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let class_decl = match &compartment.body_items[0] {
            CompartmentItem::Class(class_decl) => class_decl,
            _ => panic!("expected class"),
        };
        let constructor = class_decl
            .constructor
            .as_ref()
            .expect("expected constructor");
        assert_eq!(constructor.params.len(), 2);
        assert_eq!(constructor.body.statements.len(), 2);
        assert!(matches!(
            constructor.body.statements[0],
            BodyStmt::Assignment(_)
        ));
    }

    #[test]
    fn parses_class_method_inline_tuple_predicate_return_type() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  class Reader {
    isPair(input: unknown): input is [string, number] {
      let checked: [string, number] = input as [string, number]
      return true
    }

    label(input: unknown): string {
      if (this.isPair(input)) {
        return input[0]
      }
      return "fallback"
    }
  }
}
"#;
        let unit = parse_source_unit(src);

        assert!(unit.diagnostics.is_empty(), "{:#?}", unit.diagnostics);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let class_decl = match &compartment.body_items[0] {
            CompartmentItem::Class(class_decl) => class_decl,
            _ => panic!("expected class"),
        };
        assert_eq!(class_decl.methods.len(), 2);
        let predicate = &class_decl.methods[0];
        assert_eq!(predicate.name, "isPair");
        assert!(matches!(
            predicate.return_type.as_ref().map(|ty| &ty.kind),
            Some(TypeExprKind::Opaque(raw)) if raw == "input is [string, number]"
        ));
    }

    #[test]
    fn parses_constructor_call_arguments_structurally() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function make(): unknown boundary(secure) {
    return new Token("x", 1)
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Return(ret) = &function.body.statements[0] else {
            panic!("expected return");
        };
        let expr = ret.argument.as_ref().expect("expected return expression");
        let ExprEnvelopeKind::New { class_name, args } = &expr.kind else {
            panic!("expected constructor expression");
        };
        assert_eq!(class_name, "Token");
        assert_eq!(args.len(), 2);
    }

    #[test]
    fn parses_constructor_call_array_literal_argument_as_one_arg() {
        let src = r#"boundary default = secure

compartment Core boundary(secure) {
  export function make(): unknown boundary(secure) {
    return new Token(["x", 1])
  }
}
"#;
        let unit = parse_source_unit(src);
        let compartment = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected compartment"),
        };
        let function = match &compartment.body_items[0] {
            CompartmentItem::ExportFunction(function) => function,
            _ => panic!("expected function"),
        };
        let BodyStmt::Return(ret) = &function.body.statements[0] else {
            panic!("expected return");
        };
        let expr = ret.argument.as_ref().expect("expected return expression");
        let ExprEnvelopeKind::New { class_name, args } = &expr.kind else {
            panic!("expected constructor expression");
        };
        assert_eq!(class_name, "Token");
        assert_eq!(args.len(), 1);
        assert!(matches!(args[0].kind, ExprEnvelopeKind::ArrayLiteral(_)));
    }

    #[test]
    fn parses_nested_compartment_as_body_item_without_flattening_exports() {
        let src = r#"boundary default = secure

compartment Outer boundary(secure) {
  compartment Inner boundary(weaken to debug) {
    import { legacy } from "./legacy.js"

    export function main(): unknown boundary(secure) {
      return legacy()
    }
  }
}
"#;
        let unit = parse_source_unit(src);
        assert!(unit.diagnostics.is_empty(), "{:#?}", unit.diagnostics);
        let outer = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected outer compartment"),
        };
        assert_eq!(outer.name, "Outer");
        assert_eq!(outer.body_items.len(), 1);
        let inner = match &outer.body_items[0] {
            CompartmentItem::Compartment(compartment) => compartment,
            _ => panic!("expected nested compartment body item"),
        };
        assert_eq!(inner.name, "Inner");
        assert_eq!(inner.body_items.len(), 2);
        assert!(matches!(inner.body_items[0], CompartmentItem::Import(_)));
        assert!(matches!(
            inner.body_items[1],
            CompartmentItem::ExportFunction(_)
        ));
    }

    #[test]
    fn parses_compartment_endowment_header_without_confusing_body_braces() {
        let src = r#"boundary default = secure

compartment Outer boundary(secure) endow { clock: Clock, logger: Logger } {
  compartment Inner boundary(secure) endow { cap: Capability } {
    export function main(): string boundary(secure) {
      return "ok"
    }
  }
}
"#;
        let unit = parse_source_unit(src);
        assert!(unit.diagnostics.is_empty(), "{:#?}", unit.diagnostics);
        let outer = match &unit.items[1] {
            SourceItem::Compartment(compartment) => compartment,
            _ => panic!("expected outer compartment"),
        };
        assert_eq!(outer.name, "Outer");
        assert_eq!(outer.endowments.len(), 2);
        assert_eq!(outer.endowments[0].name, "clock");
        assert_eq!(outer.endowments[1].name, "logger");
        assert!(matches!(
            outer.endowments[0].type_annotation.kind,
            TypeExprKind::Named(ref name) if name == "Clock"
        ));
        assert_eq!(outer.body_items.len(), 1);

        let inner = match &outer.body_items[0] {
            CompartmentItem::Compartment(compartment) => compartment,
            _ => panic!("expected nested compartment"),
        };
        assert_eq!(inner.name, "Inner");
        assert_eq!(inner.endowments.len(), 1);
        assert_eq!(inner.endowments[0].name, "cap");
        assert!(matches!(
            inner.endowments[0].type_annotation.kind,
            TypeExprKind::Named(ref name) if name == "Capability"
        ));
        assert_eq!(inner.body_items.len(), 1);
        assert!(matches!(
            inner.body_items[0],
            CompartmentItem::ExportFunction(_)
        ));
    }
}

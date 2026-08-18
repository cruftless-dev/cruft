
use super::ast::{FromItem, SelectItem, SelectStmt};
use super::lower::{inferred_name, run_select, QueryResult};
use crate::catalog::{Catalog, View};
use crate::catalog::{
    CheckConstraint, DomainDef, ForeignKey, RefAction, TableConstraints, UniqueKey,
};
use crate::catalog::{FuncBody, FunctionDef, RetShape, Volatility};
use crate::catalog::{Lang, TrigEvent, TrigTiming, TriggerDef};
use crate::expr::ast::{BinOp, Expr};
use crate::expr::bind::{lower, lower_pred, resolve, Schema};
use crate::expr::eval::{eval_ctx, eval_row, EvalCtx};
use crate::expr::lexer::Tok;
use crate::expr::parser::parse_expr_at;
use crate::types::registry::TypeRegistries;
use crate::types::{self, PgError};
use sql_core::{Pred, Row, Scalar, SqlValue};
use std::sync::Arc;

fn err(input: String) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "query",
        input,
    }
}

fn sql_quote_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn token_sql(tok: &Tok) -> Option<String> {
    Some(match tok {
        Tok::Int(value) => value.to_string(),
        Tok::Float(value) => value.to_string(),
        Tok::Str(value) => sql_quote_string(value),
        Tok::Ident(value) => value.clone(),
        Tok::Op(value) => value.clone(),
        Tok::Plus => "+".to_string(),
        Tok::Minus => "-".to_string(),
        Tok::Star => "*".to_string(),
        Tok::Slash => "/".to_string(),
        Tok::Percent => "%".to_string(),
        Tok::Caret => "^".to_string(),
        Tok::Lt => "<".to_string(),
        Tok::Gt => ">".to_string(),
        Tok::Eq => "=".to_string(),
        Tok::LtEq => "<=".to_string(),
        Tok::GtEq => ">=".to_string(),
        Tok::NotEq => "<>".to_string(),
        Tok::LParen => "(".to_string(),
        Tok::RParen => ")".to_string(),
        Tok::LBracket => "[".to_string(),
        Tok::RBracket => "]".to_string(),
        Tok::Comma => ",".to_string(),
        Tok::Dot => ".".to_string(),
        Tok::Cast => "::".to_string(),
        Tok::Eof => return None,
    })
}

fn render_query_tokens(toks: &[Tok]) -> String {
    toks.iter()
        .filter_map(token_sql)
        .collect::<Vec<_>>()
        .join(" ")
}

struct C<'a> {
    toks: &'a [Tok],
    pos: usize,
    src: &'a str,
}

impl<'a> C<'a> {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos]
    }

    fn at_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), Tok::Ident(s) if s == kw)
    }

    fn eat_kw(&mut self, kw: &str) -> bool {
        if self.at_kw(kw) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect_kw(&mut self, kw: &str) -> Result<(), PgError> {
        if self.eat_kw(kw) {
            Ok(())
        } else {
            Err(err(self.src.to_string()))
        }
    }

    fn expect(&mut self, t: &Tok) -> Result<(), PgError> {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(t) {
            self.pos += 1;
            Ok(())
        } else {
            Err(err(self.src.to_string()))
        }
    }

    fn ident(&mut self) -> Result<String, PgError> {
        match self.peek() {
            Tok::Ident(s) => {
                let s = s.clone();
                self.pos += 1;
                Ok(s)
            }
            _ => Err(err(self.src.to_string())),
        }
    }

    fn str_lit(&mut self) -> Result<String, PgError> {
        match self.peek() {
            Tok::Str(s) => {
                let s = s.clone();
                self.pos += 1;
                Ok(s)
            }
            _ => Err(err(self.src.to_string())),
        }
    }

    fn expr(&mut self) -> Result<crate::expr::ast::Expr, PgError> {
        let (e, next) = parse_expr_at(self.toks, self.pos, self.src)?;
        self.pos = next;
        Ok(e)
    }

    fn skip_to_column_delim(&mut self) {
        let mut depth = 0i32;
        loop {
            match self.peek() {
                Tok::Eof => return,
                Tok::LParen | Tok::LBracket => depth += 1,
                Tok::RParen | Tok::RBracket if depth > 0 => depth -= 1,
                Tok::RParen | Tok::Comma if depth == 0 => return,
                _ => {}
            }
            self.pos += 1;
        }
    }

    fn skip_paren_group(&mut self) {
        if !matches!(self.peek(), Tok::LParen) {
            return;
        }
        let mut depth = 0i32;
        loop {
            match self.peek() {
                Tok::Eof => return,
                Tok::LParen => depth += 1,
                Tok::RParen => {
                    depth -= 1;
                    self.pos += 1;
                    if depth == 0 {
                        return;
                    }
                    continue;
                }
                _ => {}
            }
            self.pos += 1;
        }
    }
}

pub fn run(toks: &[Tok], sql: &str, catalog: &mut Catalog) -> Result<QueryResult, PgError> {

    let bare = !catalog.in_transaction() && !catalog.in_autocommit_stmt();
    if bare {
        catalog.stmt_write_begin();
    }
    let mut c = C {
        toks,
        pos: 0,
        src: sql,
    };
    let result = match c.peek() {
        Tok::Ident(kw) => match kw.as_str() {
            "create" => run_create(&mut c, catalog),
            "insert" => run_insert(&mut c, catalog),
            "delete" => run_delete(&mut c, catalog),
            "drop" => run_drop(&mut c, catalog),
            "update" => run_update(&mut c, catalog),
            "merge" => run_merge(&mut c, catalog),
            "refresh" => run_refresh(&mut c, catalog),
            "alter" => run_alter(&mut c, catalog),
            "comment" => run_comment(&mut c, catalog),
            _ => Err(err(sql.to_string())),
        },
        _ => Err(err(sql.to_string())),
    };
    if bare {
        catalog.stmt_write_end(result.is_ok());
    }
    result
}

fn empty() -> QueryResult {
    QueryResult {
        columns: vec![],
        col_types: vec![],
        rows: vec![],
    }
}

fn parse_returning(c: &mut C) -> Result<Option<Vec<SelectItem>>, PgError> {
    if !c.eat_kw("returning") {
        return Ok(None);
    }
    let mut items = Vec::new();
    loop {
        if matches!(c.peek(), Tok::Star) {
            c.pos += 1;
            items.push(SelectItem::Star);
        } else {
            let expr = c.expr()?;

            let alias = if c.eat_kw("as") {
                Some(c.ident()?)
            } else if let Tok::Ident(a) = c.peek() {
                let a = a.clone();
                c.pos += 1;
                Some(a)
            } else {
                None
            };
            items.push(SelectItem::Expr { expr, alias });
        }
        if matches!(c.peek(), Tok::Comma) {
            c.pos += 1;
        } else {
            break;
        }
    }
    Ok(Some(items))
}

fn project_returning(
    items: &[SelectItem],
    schema: &Schema,
    col_types: &[u32],
    rows: &[Row],
    regs: &Arc<TypeRegistries>,
) -> Result<QueryResult, PgError> {
    let mut cols: Vec<Scalar> = Vec::new();
    let mut columns: Vec<String> = Vec::new();
    let mut out_types: Vec<u32> = Vec::new();
    for item in items {
        match item {
            SelectItem::Star => {
                for (i, name) in schema.names().iter().enumerate() {
                    cols.push(lower(&Expr::ColumnRef(i), schema, regs.clone())?);
                    columns.push(name.clone());
                    out_types.push(col_types.get(i).copied().unwrap_or(0));
                }
            }
            SelectItem::Expr { expr, alias } => {
                cols.push(lower(expr, schema, regs.clone())?);
                columns.push(alias.clone().unwrap_or_else(|| inferred_name(expr)));
                out_types.push(crate::expr::infer::infer(expr, schema, col_types).unwrap_or(0));
            }
        }
    }
    let mut out_rows = Vec::with_capacity(rows.len());
    for r in rows {
        let mut orow = Vec::with_capacity(cols.len());
        for scal in &cols {
            orow.push(scal(r).map_err(err)?);
        }
        out_rows.push(orow);
    }
    Ok(QueryResult {
        columns,
        col_types: out_types,
        rows: out_rows,
    })
}

fn run_create(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("create")?;

    let or_replace = if c.eat_kw("or") {
        c.expect_kw("replace")?;
        true
    } else {
        false
    };

    let _temp = c.eat_kw("temporary") || c.eat_kw("temp");
    c.eat_kw("unlogged");
    if c.at_kw("sequence") {
        return run_create_sequence(c, catalog);
    }
    if c.at_kw("view") {
        return run_create_view(c, catalog, or_replace);
    }
    if c.at_kw("materialized") {
        return run_create_matview(c, catalog);
    }
    if c.at_kw("index") || c.at_kw("unique") {
        return run_create_index(c, catalog);
    }
    if c.at_kw("domain") {
        return run_create_domain(c, catalog);
    }
    if c.at_kw("function") {
        return run_create_function(c, catalog, or_replace);
    }
    if c.at_kw("trigger") {
        return run_create_trigger(c, catalog);
    }
    if c.at_kw("operator") {
        return run_create_operator(c, catalog);
    }
    if c.at_kw("aggregate") {
        return run_create_aggregate(c, catalog);
    }
    if c.at_kw("cast") {
        return run_create_cast(c, catalog);
    }
    if c.at_kw("type") {
        if or_replace {

            return Err(err(c.src.to_string()));
        }
        return run_create_type(c, catalog);
    }
    if or_replace {

        return Err(err(c.src.to_string()));
    }
    run_create_table(c, catalog)
}

fn run_create_domain(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("domain")?;
    let name = c.ident()?;

    if catalog.get_domain(&name).is_some() {
        return Err(err(format!("type \"{name}\" already exists")));
    }

    c.eat_kw("as");
    let (base_oid, base_typmod) = read_type(c, catalog)?;

    let mut not_null = false;
    let mut default: Option<Expr> = None;
    let mut checks: Vec<CheckConstraint> = Vec::new();

    loop {

        let cname: Option<String> = if c.at_kw("constraint") {
            c.pos += 1;
            Some(c.ident()?)
        } else {
            None
        };
        match c.peek() {
            Tok::Eof => break,
            Tok::Ident(s) => match s.as_str() {
                "default" => {
                    c.pos += 1;
                    default = Some(c.expr()?);
                }
                "not" => {
                    c.pos += 1;
                    c.expect_kw("null")?;
                    not_null = true;
                }
                "null" => {
                    c.pos += 1;
                }
                "check" => {
                    c.pos += 1;
                    let expr = parse_check_expr(c)?;

                    let cn = cname.unwrap_or_else(|| format!("{name}_check"));
                    checks.push(CheckConstraint {
                        name: Some(cn),
                        expr,
                    });
                }
                _ => break,
            },
            _ => break,
        }
    }

    catalog.create_domain(DomainDef {
        name,
        base_oid,
        base_typmod,
        not_null,
        default,
        checks,
    });
    Ok(empty())
}

fn run_drop_domain(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("domain")?;
    let if_exists = if c.eat_kw("if") {
        c.expect_kw("exists")?;
        true
    } else {
        false
    };
    let name = c.ident()?;
    let existed = catalog.drop_domain(&name);
    if !existed && !if_exists {
        return Err(err(format!("type \"{name}\" does not exist")));
    }
    Ok(empty())
}

fn run_create_cast(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("cast")?;
    c.expect(&Tok::LParen)?;
    let source_oid = read_type(c, catalog)?.0;
    c.expect_kw("as")?;
    let target_oid = read_type(c, catalog)?.0;
    c.expect(&Tok::RParen)?;

    if !(c.eat_kw("with") && c.eat_kw("function")) {
        return Err(err(
            "only WITH FUNCTION casts are supported (WITHOUT FUNCTION / WITH INOUT deferred)"
                .to_string(),
        ));
    }

    let mut func = c.ident()?;
    if matches!(c.peek(), Tok::Dot) {
        c.pos += 1;
        func = c.ident()?;
    }
    if matches!(c.peek(), Tok::LParen) {

        let mut depth = 0usize;
        loop {
            match c.peek() {
                Tok::LParen => depth += 1,
                Tok::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        c.pos += 1;
                        break;
                    }
                }
                Tok::Eof => return Err(err(c.src.to_string())),
                _ => {}
            }
            c.pos += 1;
        }
    }

    if c.eat_kw("as") {
        let _ = c.ident()?;
    }
    catalog.create_cast(source_oid, target_oid, &func)?;
    Ok(empty())
}

fn run_create_aggregate(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("aggregate")?;
    let name = c.ident()?;

    c.expect(&Tok::LParen)?;
    if matches!(c.peek(), Tok::Star) {
        c.pos += 1;
    } else if !matches!(c.peek(), Tok::RParen) {
        let _ = read_type(c, catalog)?;
    }
    c.expect(&Tok::RParen)?;

    c.expect(&Tok::LParen)?;
    let mut sfunc: Option<String> = None;
    let mut stype_oid: Option<u32> = None;
    let mut initcond: Option<String> = None;
    let mut finalfunc: Option<String> = None;
    let read_fn_name = |c: &mut C| -> Result<String, PgError> {
        let mut n = c.ident()?;
        if matches!(c.peek(), Tok::Dot) {
            c.pos += 1;
            n = c.ident()?;
        }
        Ok(n)
    };
    loop {
        let key = c.ident()?.to_ascii_lowercase();
        c.expect(&Tok::Eq)?;
        match key.as_str() {
            "sfunc" => sfunc = Some(read_fn_name(c)?),
            "finalfunc" => finalfunc = Some(read_fn_name(c)?),
            "stype" => stype_oid = Some(read_type(c, catalog)?.0),
            "initcond" => initcond = Some(c.str_lit()?),
            _ => {
                while !matches!(c.peek(), Tok::Comma | Tok::RParen | Tok::Eof) {
                    c.pos += 1;
                }
            }
        }
        if matches!(c.peek(), Tok::Comma) {
            c.pos += 1;
        } else {
            break;
        }
    }
    c.expect(&Tok::RParen)?;
    let missing = |what: &str| err(format!("CREATE AGGREGATE {name} requires {what}"));
    let def = crate::catalog::AggregateDef {
        name: name.clone(),
        sfunc: sfunc.ok_or_else(|| missing("SFUNC"))?,
        stype_oid: stype_oid.ok_or_else(|| missing("STYPE"))?,
        initcond: initcond.ok_or_else(|| missing("INITCOND"))?,
        finalfunc,
    };
    catalog.create_aggregate(def)?;
    Ok(empty())
}

fn run_create_operator(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("operator")?;
    let symbol = match c.peek() {
        Tok::Op(s) => {
            let s = s.clone();
            c.pos += 1;
            s
        }
        _ => return Err(err(c.src.to_string())),
    };
    c.expect(&Tok::LParen)?;
    let mut func: Option<String> = None;
    loop {
        let key = c.ident()?.to_ascii_lowercase();
        c.expect(&Tok::Eq)?;
        if key == "function" || key == "procedure" {

            let mut name = c.ident()?;
            if matches!(c.peek(), Tok::Dot) {
                c.pos += 1;
                name = c.ident()?;
            }
            func = Some(name);
        } else {

            while !matches!(c.peek(), Tok::Comma | Tok::RParen | Tok::Eof) {
                c.pos += 1;
            }
        }
        if matches!(c.peek(), Tok::Comma) {
            c.pos += 1;
        } else {
            break;
        }
    }
    c.expect(&Tok::RParen)?;
    let func = func.ok_or_else(|| {
        err(format!(
            "operator function is required for CREATE OPERATOR {symbol}"
        ))
    })?;
    catalog.create_operator(&symbol, &func)?;
    Ok(empty())
}

fn run_comment(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("comment")?;
    c.expect_kw("on")?;
    if c.eat_kw("table") {
        let name = c.ident()?;
        if catalog.get(&name).is_none() {
            return Err(err(format!("relation \"{name}\" does not exist")));
        }
        let text = parse_comment_value(c)?;
        catalog.set_comment(&name, 0, text.as_deref());
        Ok(empty())
    } else if c.eat_kw("column") {
        let table = c.ident()?;
        c.expect(&Tok::Dot)?;
        let col = c.ident()?;
        let subid = {
            let t = catalog
                .get(&table)
                .ok_or_else(|| err(format!("relation \"{table}\" does not exist")))?;
            let idx = t.schema.index_of(&col).ok_or_else(|| {
                err(format!(
                    "column \"{col}\" of relation \"{table}\" does not exist"
                ))
            })?;
            (idx + 1) as i32
        };
        let text = parse_comment_value(c)?;
        catalog.set_comment(&table, subid, text.as_deref());
        Ok(empty())
    } else {
        Err(err(c.src.to_string()))
    }
}

fn parse_comment_value(c: &mut C) -> Result<Option<String>, PgError> {
    c.expect_kw("is")?;
    if c.eat_kw("null") {
        Ok(None)
    } else {
        Ok(Some(c.str_lit()?))
    }
}

fn run_create_function(
    c: &mut C,
    catalog: &mut Catalog,
    or_replace: bool,
) -> Result<QueryResult, PgError> {
    c.expect_kw("function")?;
    let name = c.ident()?;
    c.expect(&Tok::LParen)?;

    let mut args: Vec<(Option<String>, u32, i32)> = Vec::new();
    if !matches!(c.peek(), Tok::RParen) {
        loop {
            let argname = if matches!(c.peek(), Tok::Ident(_))
                && matches!(c.toks.get(c.pos + 1), Some(Tok::Ident(_)))
            {
                Some(c.ident()?)
            } else {
                None
            };
            let (oid, typmod) = read_type(c, catalog)?;
            args.push((argname, oid, typmod));
            if matches!(c.peek(), Tok::Comma) {
                c.pos += 1;
            } else {
                break;
            }
        }
    }
    c.expect(&Tok::RParen)?;

    c.expect_kw("returns")?;

    let mut ret_oid = 0u32;
    let mut ret_typmod = types::typmod::NONE;
    let returns: RetShape = if c.eat_kw("trigger") {

        ret_oid = 2279;
        RetShape::Scalar
    } else if c.eat_kw("setof") {

        if let Tok::Ident(word) = c.peek() {
            if catalog.get(word).is_some() {
                c.pos += 1;
                RetShape::SetofRel
            } else {
                let (oid, _) = read_type(c, catalog)?;
                RetShape::SetofScalar { oid }
            }
        } else {
            let (oid, _) = read_type(c, catalog)?;
            RetShape::SetofScalar { oid }
        }
    } else if c.eat_kw("table") {

        c.expect(&Tok::LParen)?;
        let mut cols: Vec<(String, u32, i32)> = Vec::new();
        if !matches!(c.peek(), Tok::RParen) {
            loop {
                let cname = c.ident()?;
                let (oid, typmod) = read_type(c, catalog)?;
                cols.push((cname, oid, typmod));
                if matches!(c.peek(), Tok::Comma) {
                    c.pos += 1;
                } else {
                    break;
                }
            }
        }
        c.expect(&Tok::RParen)?;
        RetShape::SetofTable(cols)
    } else {
        let (oid, typmod) = read_type(c, catalog)?;
        ret_oid = oid;
        ret_typmod = typmod;
        RetShape::Scalar
    };

    let mut body_text: Option<String> = None;
    let mut language: Option<String> = None;
    let mut strict = false;
    let mut volatility = Volatility::Volatile;
    loop {
        if c.eat_kw("as") {

            match c.peek() {
                Tok::Str(s) => {
                    body_text = Some(s.clone());
                    c.pos += 1;
                }
                _ => return Err(err(c.src.to_string())),
            }
            continue;
        }
        if c.eat_kw("language") {
            language = Some(c.ident()?);
            continue;
        }
        if c.eat_kw("immutable") {
            volatility = Volatility::Immutable;
            continue;
        }
        if c.eat_kw("stable") {
            volatility = Volatility::Stable;
            continue;
        }
        if c.eat_kw("volatile") {
            volatility = Volatility::Volatile;
            continue;
        }
        if c.eat_kw("strict") {
            strict = true;
            continue;
        }
        if c.eat_kw("called") {

            c.expect_kw("on")?;
            c.expect_kw("null")?;
            c.expect_kw("input")?;
            strict = false;
            continue;
        }
        if c.eat_kw("returns") {

            c.expect_kw("null")?;
            c.expect_kw("on")?;
            c.expect_kw("null")?;
            c.expect_kw("input")?;
            strict = true;
            continue;
        }

        if c.eat_kw("cost") || c.eat_kw("rows") {

            if matches!(c.peek(), Tok::Int(_) | Tok::Float(_)) {
                c.pos += 1;
            }
            continue;
        }
        if c.eat_kw("parallel") {

            let _ = c.ident();
            continue;
        }
        if c.eat_kw("leakproof")
            || c.eat_kw("window")
            || c.eat_kw("security")
            || c.eat_kw("external")
            || c.eat_kw("definer")
            || c.eat_kw("invoker")
        {
            continue;
        }
        if c.eat_kw("not") {
            c.eat_kw("leakproof");
            continue;
        }
        break;
    }

    match language.as_deref() {
        Some("sql") | Some("plpgsql") => {}
        Some(other) => {
            return Err(err(format!("language \"{other}\" is not supported")));
        }
        None => return Err(err(c.src.to_string())),
    }
    let body_text = body_text.ok_or_else(|| err(c.src.to_string()))?;

    if language.as_deref() == Some("plpgsql") {
        let block = crate::stmt::plpgsql::parse_block(&body_text)?;
        if catalog.has_function(&name) && !or_replace {
            let types: Vec<&str> = args
                .iter()
                .map(|(_, oid, _)| types::type_name(*oid))
                .collect();
            return Err(err(format!(
                "function {name}({}) already exists",
                types.join(", ")
            )));
        }
        catalog.create_function(FunctionDef {
            name,
            args,
            ret_oid,
            ret_typmod,
            returns: RetShape::Scalar,

            body: FuncBody::Expr(Expr::Null),
            strict,
            volatility,
            language: Lang::PlPgSql,
            pl_body: Some(block),
            source_body: Some(body_text),
        });
        return Ok(empty());
    }

    let trimmed = body_text.trim();
    let body_sql = if trimmed.to_ascii_lowercase().starts_with("select") {
        trimmed.to_string()
    } else {
        format!("SELECT {trimmed}")
    };
    let stmt = super::parser::parse(&body_sql)
        .map_err(|_| err(format!("could not parse SQL function body: {trimmed}")))?;
    let super::ast::Stmt::Select(sel) = stmt;

    if returns.is_setof() {
        let body = FuncBody::Query(Box::new(sel));
        if catalog.has_function(&name) && !or_replace {
            let types: Vec<&str> = args
                .iter()
                .map(|(_, oid, _)| types::type_name(*oid))
                .collect();
            return Err(err(format!(
                "function {name}({}) already exists",
                types.join(", ")
            )));
        }
        catalog.create_function(FunctionDef {
            name,
            args,
            ret_oid,
            ret_typmod,
            returns,
            body,
            strict,
            volatility,
            language: Lang::Sql,
            pl_body: None,
            source_body: Some(body_text),
        });
        return Ok(empty());
    }

    if sel.projection.len() != 1 {
        return Err(err(
            "a scalar SQL function body must return a single column".to_string(),
        ));
    }
    let body = if sel.from.is_none()
        && sel.group_by.is_empty()
        && sel.grouping_sets.is_empty()
        && sel.having.is_none()
        && !sel.distinct
        && sel.distinct_on.is_empty()
        && sel.tail.is_empty()
    {

        match sel.projection.into_iter().next().unwrap() {
            SelectItem::Expr { expr, .. } => FuncBody::Expr(expr),
            SelectItem::Star => {
                return Err(err(
                    "a scalar SQL function body must return a single column".to_string(),
                ))
            }
        }
    } else {

        FuncBody::Query(Box::new(sel))
    };
    let source_body = if matches!(body, FuncBody::Query(_)) {
        Some(body_text)
    } else {
        None
    };

    if catalog.has_function(&name) && !or_replace {
        let types: Vec<&str> = args
            .iter()
            .map(|(_, oid, _)| types::type_name(*oid))
            .collect();
        return Err(err(format!(
            "function {name}({}) already exists",
            types.join(", ")
        )));
    }

    catalog.create_function(FunctionDef {
        name,
        args,
        ret_oid,
        ret_typmod,
        returns: RetShape::Scalar,
        body,
        strict,
        volatility,
        language: Lang::Sql,
        pl_body: None,
        source_body,
    });
    Ok(empty())
}

fn run_create_trigger(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("trigger")?;
    let name = c.ident()?;

    let timing = if c.eat_kw("before") {
        TrigTiming::Before
    } else if c.eat_kw("after") {
        TrigTiming::After
    } else if c.at_kw("instead") {
        return Err(err(
            "INSTEAD OF (view) triggers are not supported (deferred)".to_string(),
        ));
    } else {
        return Err(err(c.src.to_string()));
    };

    let mut events: Vec<TrigEvent> = Vec::new();
    loop {
        if c.eat_kw("insert") {
            events.push(TrigEvent::Insert);
        } else if c.eat_kw("update") {

            if c.eat_kw("of") {
                loop {
                    let _ = c.ident()?;
                    if matches!(c.peek(), Tok::Comma) {
                        c.pos += 1;
                    } else {
                        break;
                    }
                }
            }
            events.push(TrigEvent::Update);
        } else if c.eat_kw("delete") {
            events.push(TrigEvent::Delete);
        } else if c.at_kw("truncate") {
            return Err(err(
                "TRUNCATE triggers are not supported (deferred)".to_string()
            ));
        } else {
            return Err(err(c.src.to_string()));
        }
        if !c.eat_kw("or") {
            break;
        }
    }

    c.expect_kw("on")?;
    let table = c.ident()?;

    c.expect_kw("for")?;
    c.eat_kw("each");
    if c.eat_kw("statement") {
        return Err(err(
            "FOR EACH STATEMENT triggers are not supported (deferred)".to_string(),
        ));
    }
    c.expect_kw("row")?;

    let when = if c.eat_kw("when") {
        c.expect(&Tok::LParen)?;
        let cond = c.expr()?;
        c.expect(&Tok::RParen)?;
        Some(cond)
    } else {
        None
    };

    c.expect_kw("execute")?;
    if !c.eat_kw("function") && !c.eat_kw("procedure") {
        return Err(err(c.src.to_string()));
    }
    let func = c.ident()?;
    c.expect(&Tok::LParen)?;
    c.expect(&Tok::RParen)?;

    if catalog.has_trigger(&name, &table) {
        return Err(err(format!(
            "trigger \"{name}\" for relation \"{table}\" already exists"
        )));
    }
    catalog.create_trigger(TriggerDef {
        name,
        timing,
        events,
        table,
        when,
        func,
    });
    Ok(empty())
}

fn run_drop_trigger(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("trigger")?;
    let if_exists = if c.eat_kw("if") {
        c.expect_kw("exists")?;
        true
    } else {
        false
    };
    let name = c.ident()?;
    c.expect_kw("on")?;
    let table = c.ident()?;
    let existed = catalog.drop_trigger(&name, &table);
    if !existed && !if_exists {
        return Err(err(format!(
            "trigger \"{name}\" for table \"{table}\" does not exist"
        )));
    }
    Ok(empty())
}

fn tg_vars(
    tr: &TriggerDef,
    table: &str,
    timing: TrigTiming,
    event: TrigEvent,
) -> Vec<(String, String)> {
    let op = match event {
        TrigEvent::Insert => "INSERT",
        TrigEvent::Update => "UPDATE",
        TrigEvent::Delete => "DELETE",
    };
    let when = match timing {
        TrigTiming::Before => "BEFORE",
        TrigTiming::After => "AFTER",
    };
    vec![
        ("tg_op".to_string(), op.to_string()),
        ("tg_table_name".to_string(), table.to_string()),
        ("tg_name".to_string(), tr.name.clone()),
        ("tg_when".to_string(), when.to_string()),
        ("tg_level".to_string(), "ROW".to_string()),
    ]
}

#[allow(clippy::too_many_arguments)]
fn fire_row_triggers(
    catalog: &mut Catalog,
    table: &str,
    schema: &Schema,
    col_types: &[u32],
    col_typmods: &[i32],
    timing: TrigTiming,
    event: TrigEvent,
    mut new: Option<Row>,
    old: Option<Row>,
) -> Result<Option<Row>, PgError> {
    use crate::stmt::plpgsql::{self, PlEnv, PlOutcome};

    let is_delete = event == TrigEvent::Delete;
    let trigs = catalog.matching_triggers(table, timing, event);
    if trigs.is_empty() {
        return Ok(if is_delete { old } else { new });
    }
    let regs = Arc::new(catalog.type_registries());
    let col_names: Vec<String> = schema.names().to_vec();
    for tr in &trigs {
        let mut env = PlEnv {
            col_names: col_names.clone(),
            col_oids: col_types.to_vec(),
            col_typmods: col_typmods.to_vec(),
            new: new.clone(),
            old: old.clone(),
            tg: tg_vars(tr, table, timing, event),
            vars: std::collections::HashMap::new(),
            var_types: std::collections::HashMap::new(),
            is_function: false,
        };

        if let Some(cond) = &tr.when {
            let v = plpgsql::eval_ast(cond, &env, &regs)?;
            if !plpgsql::truthy(&v) {
                continue;
            }
        }

        let block = {
            let fdef = catalog
                .get_function(&tr.func)
                .ok_or_else(|| err(format!("function {}() does not exist", tr.func)))?;
            match (fdef.language, &fdef.pl_body) {
                (Lang::PlPgSql, Some(b)) => b.clone(),
                _ => {
                    return Err(err(format!(
                        "trigger function \"{}\" is not a plpgsql function",
                        tr.func
                    )))
                }
            }
        };
        let outcome = plpgsql::exec_block(&block, &mut env, catalog, &regs)?;
        if timing == TrigTiming::Before {
            match outcome {

                PlOutcome::Return(Some(row)) => {
                    if event != TrigEvent::Delete {
                        new = Some(row);
                    }
                }

                PlOutcome::Return(None) | PlOutcome::Fell => return Ok(None),
            }
        }

    }
    Ok(if is_delete { old } else { new })
}

fn run_drop_function(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("function")?;
    let if_exists = if c.eat_kw("if") {
        c.expect_kw("exists")?;
        true
    } else {
        false
    };
    let name = c.ident()?;

    if matches!(c.peek(), Tok::LParen) {
        c.pos += 1;
        if !matches!(c.peek(), Tok::RParen) {
            loop {

                if matches!(c.peek(), Tok::Ident(_))
                    && matches!(c.toks.get(c.pos + 1), Some(Tok::Ident(_)))
                {
                    let _ = c.ident();
                }
                let _ = read_type(c, catalog)?;
                if matches!(c.peek(), Tok::Comma) {
                    c.pos += 1;
                } else {
                    break;
                }
            }
        }
        c.expect(&Tok::RParen)?;
    }
    let existed = catalog.drop_function(&name);
    if !existed && !if_exists {
        return Err(err(format!("function \"{name}\" does not exist")));
    }
    Ok(empty())
}

fn read_type_or_domain(
    c: &mut C,
    catalog: &Catalog,
) -> Result<(u32, i32, Option<String>), PgError> {
    if let Tok::Ident(word) = c.peek() {
        if let Some(d) = catalog.get_domain(word) {
            let dname = word.clone();
            let (base_oid, base_typmod) = (d.base_oid, d.base_typmod);
            c.pos += 1;
            return Ok((base_oid, base_typmod, Some(dname)));
        }
    }
    let (oid, typmod) = read_type(c, catalog)?;
    Ok((oid, typmod, None))
}

fn enforce_domain_columns(
    catalog: &Catalog,
    col_domains: &[Option<String>],
    rows: &[Row],
) -> Result<(), PgError> {
    if col_domains.iter().all(|d| d.is_none()) {
        return Ok(());
    }
    for row in rows {
        for (i, dslot) in col_domains.iter().enumerate() {
            let Some(dname) = dslot else { continue };
            let val = row.get(i).cloned().unwrap_or(SqlValue::Null);
            enforce_domain(catalog, dname, &val)?;
        }
    }
    Ok(())
}

fn enforce_domain(catalog: &Catalog, dname: &str, val: &SqlValue) -> Result<(), PgError> {
    let regs = Arc::new(catalog.type_registries());
    let regs = &regs;
    let d = catalog
        .get_domain(dname)
        .ok_or_else(|| err(format!("type \"{dname}\" does not exist")))?;
    if matches!(val, SqlValue::Null) {
        if d.not_null {
            return Err(err(format!("domain {dname} does not allow null values")));
        }

        return Ok(());
    }

    let schema = Schema::new(["value"]);
    let vrow: Row = vec![val.clone()];
    for check in &d.checks {
        let bound = resolve(&check.expr, &schema)?;

        if matches!(
            eval_row(&bound, &vrow, EvalCtx::new(regs))?,
            SqlValue::Int(0)
        ) {
            let cn = check.name.as_deref().unwrap_or("");
            return Err(err(format!(
                "value for domain {dname} violates check constraint \"{cn}\""
            )));
        }
    }
    Ok(())
}

fn run_create_table(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("table")?;

    let if_not_exists = if c.eat_kw("if") {
        c.expect_kw("not")?;
        c.expect_kw("exists")?;
        true
    } else {
        false
    };
    let name = c.ident()?;
    if catalog.get(&name).is_some() {
        if if_not_exists {
            return Ok(empty());
        }
        return Err(err(format!("relation \"{name}\" already exists")));
    }

    if c.at_kw("partition") {
        return run_create_partition_child(c, catalog, &name);
    }
    c.expect(&Tok::LParen)?;

    let mut cols: Vec<(String, u32, i32)> = Vec::new();

    let mut not_null: Vec<bool> = Vec::new();

    let mut defaults: Vec<Option<Expr>> = Vec::new();

    let mut generated: Vec<Option<Expr>> = Vec::new();

    let mut col_domains: Vec<Option<String>> = Vec::new();
    let mut col_uniques: Vec<(Vec<usize>, bool, Option<String>)> = Vec::new();
    let mut named_uniques: Vec<(Vec<String>, bool, Option<String>)> = Vec::new();
    let mut checks: Vec<CheckConstraint> = Vec::new();

    let mut fk_specs: Vec<FkSpec> = Vec::new();

    let mut identity: Vec<crate::catalog::IdentityKind> = Vec::new();
    let mut implicit_seqs: Vec<PendingSeq> = Vec::new();

    loop {

        if matches!(c.peek(), Tok::RParen) {
            break;
        }

        let named_prefix: Option<String> = if c.at_kw("constraint") {
            c.pos += 1;
            Some(c.ident()?)
        } else {
            None
        };

        if is_table_constraint_kw(c.peek()) {
            parse_table_constraint(
                c,
                named_prefix,
                &mut named_uniques,
                &mut checks,
                &mut fk_specs,
            )?;
        } else {
            let col = c.ident()?;

            let serial_oid = serial_type_oid(c.peek());
            let (oid, typmod, domain) = if let Some(soid) = serial_oid {
                c.pos += 1;
                (soid, types::typmod::NONE, None)
            } else {
                read_type_or_domain(c, catalog)?
            };
            let idx = cols.len();
            cols.push((col.clone(), oid, typmod));
            not_null.push(false);
            defaults.push(None);
            generated.push(None);
            col_domains.push(domain);
            identity.push(crate::catalog::IdentityKind::None);
            if serial_oid.is_some() {
                let seq_nm = format!("{name}_{col}_seq");
                not_null[idx] = true;
                defaults[idx] = Some(nextval_default(&seq_nm));
                implicit_seqs.push(PendingSeq {
                    name: seq_nm,
                    opts: SeqOptions::default(),
                });
            }

            parse_column_constraints(
                c,
                catalog,
                idx,
                &col,
                &name,
                named_prefix,
                &mut not_null,
                &mut defaults,
                &mut generated,
                &mut identity,
                &mut implicit_seqs,
                &mut col_uniques,
                &mut checks,
                &mut fk_specs,
            )?;
        }

        if matches!(c.peek(), Tok::Comma) {
            c.pos += 1;
        } else {
            break;
        }
    }
    c.expect(&Tok::RParen)?;

    if cols.is_empty() {
        return Err(err(c.src.to_string()));
    }

    let names: Vec<String> = cols.iter().map(|(n, _, _)| n.clone()).collect();
    let mut uniques: Vec<UniqueKey> = Vec::new();
    for (idxs, is_pk, cname) in col_uniques {
        if is_pk {
            for &i in &idxs {
                not_null[i] = true;
            }
        }
        let name = cname.or_else(|| default_key_name(&name, &names, &idxs, is_pk));
        uniques.push(UniqueKey {
            cols: idxs,
            is_primary: is_pk,
            name,
        });
    }
    for (colnames, is_pk, cname) in named_uniques {
        let mut idxs = Vec::new();
        for cn in &colnames {
            let i = names
                .iter()
                .position(|n| n == cn)
                .ok_or_else(|| err(format!("column \"{cn}\" named in key does not exist")))?;
            if is_pk {
                not_null[i] = true;
            }
            idxs.push(i);
        }
        let kname = cname.or_else(|| default_key_name(&name, &names, &idxs, is_pk));
        uniques.push(UniqueKey {
            cols: idxs,
            is_primary: is_pk,
            name: kname,
        });
    }

    let mut foreign_keys: Vec<ForeignKey> = Vec::new();
    for spec in fk_specs {
        let child_cols = resolve_col_indices(&names, &spec.child_cols)
            .ok_or_else(|| err("column named in key does not exist".to_string()))?;

        let (parent_names, parent_uniques): (Vec<String>, Vec<crate::catalog::UniqueKey>) =
            if spec.parent == name {
                (names.clone(), uniques.clone())
            } else {
                let p = catalog
                    .get(&spec.parent)
                    .ok_or_else(|| err(format!("relation \"{}\" does not exist", spec.parent)))?;
                (p.schema.names(), p.constraints.uniques.clone())
            };

        let parent_cols = match &spec.parent_cols {
            Some(cns) => resolve_col_indices(&parent_names, cns).ok_or_else(|| {
                err(format!(
                    "column named in foreign key does not exist in referenced table \"{}\"",
                    spec.parent
                ))
            })?,
            None => parent_uniques
                .iter()
                .find(|u| u.is_primary)
                .map(|u| u.cols.clone())
                .ok_or_else(|| {
                    err(format!(
                        "there is no primary key for referenced table \"{}\"",
                        spec.parent
                    ))
                })?,
        };

        let matches_unique = parent_uniques
            .iter()
            .any(|u| same_set(&u.cols, &parent_cols));
        if !matches_unique {
            return Err(err(format!(
                "there is no unique constraint matching given keys for referenced table \"{}\"",
                spec.parent
            )));
        }
        if child_cols.len() != parent_cols.len() {
            return Err(err(
                "number of referencing and referenced columns for foreign key disagree".to_string(),
            ));
        }
        let fk_name = spec
            .name
            .clone()
            .or_else(|| default_fk_name(&name, &names, &child_cols));
        foreign_keys.push(ForeignKey {
            cols: child_cols,
            parent: spec.parent,
            parent_cols,
            on_delete: spec.on_delete,
            on_update: spec.on_update,
            name: fk_name,
            deferrable: spec.deferrable,
            initially_deferred: spec.initially_deferred,
        });
    }

    let constraints = crate::catalog::TableConstraints {
        not_null,
        uniques,
        checks,
        foreign_keys,
    };
    catalog.create_typed_with_constraints(&name, cols, constraints, Vec::new());

    if defaults.iter().any(|d| d.is_some()) {
        catalog.get_table_mut(&name).unwrap().defaults = defaults;
    }

    if generated.iter().any(|g| g.is_some()) {
        catalog.get_table_mut(&name).unwrap().generated = generated;
    }

    if col_domains.iter().any(|d| d.is_some()) {
        catalog.get_table_mut(&name).unwrap().col_domains = col_domains;
    }

    if identity
        .iter()
        .any(|k| *k != crate::catalog::IdentityKind::None)
    {
        catalog.get_table_mut(&name).unwrap().identity = identity;
    }

    for ps in implicit_seqs {
        catalog.create_sequence_implicit(ps.opts.into_seq(ps.name));
    }

    if c.eat_kw("partition") {
        c.expect_kw("by")?;
        if !(c.eat_kw("range") || c.eat_kw("list")) {
            return Err(err(
                "only PARTITION BY RANGE or LIST is supported (HASH deferred)".to_string(),
            ));
        }
        c.expect(&Tok::LParen)?;
        let keycol = c.ident()?;
        c.expect(&Tok::RParen)?;
        let key_idx = catalog
            .get(&name)
            .and_then(|t| t.schema.index_of(&keycol))
            .ok_or_else(|| {
                err(format!(
                    "column \"{keycol}\" named in partition key does not exist"
                ))
            })?;
        catalog.mark_partitioned(&name, key_idx);
    }
    Ok(empty())
}

fn run_create_partition_child(
    c: &mut C,
    catalog: &mut Catalog,
    child: &str,
) -> Result<QueryResult, PgError> {
    c.expect_kw("partition")?;
    c.expect_kw("of")?;
    let parent = c.ident()?;

    let (pcols, keyname) = {
        let key_col = catalog
            .partition_info(&parent)
            .ok_or_else(|| err(format!("\"{parent}\" is not partitioned")))?
            .key_col;
        let pt = catalog
            .get(&parent)
            .ok_or_else(|| err(format!("relation \"{parent}\" does not exist")))?;
        let names = pt.schema.names();
        let cols: Vec<(String, u32, i32)> = names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                (
                    n.clone(),
                    pt.col_types.get(i).copied().unwrap_or(0),
                    pt.col_typmods.get(i).copied().unwrap_or(-1),
                )
            })
            .collect();
        (cols, names[key_col].clone())
    };

    if c.eat_kw("default") {
        let ncols = pcols.len();
        let constraints = TableConstraints {
            not_null: vec![false; ncols],
            uniques: Vec::new(),
            checks: Vec::new(),
            foreign_keys: Vec::new(),
        };
        catalog.create_typed_with_constraints(child, pcols, constraints, Vec::new());
        catalog.set_default_partition(&parent, child)?;
        return Ok(empty());
    }

    c.expect_kw("for")?;
    c.expect_kw("values")?;
    let keyref = || Box::new(Expr::Column(keyname.clone()));
    let eq = |v: Expr| Expr::Binary {
        op: BinOp::Eq,
        left: keyref(),
        right: Box::new(v),
    };
    let check_expr = if c.eat_kw("in") {

        c.expect(&Tok::LParen)?;
        let mut vals = vec![c.expr()?];
        while matches!(c.peek(), Tok::Comma) {
            c.pos += 1;
            vals.push(c.expr()?);
        }
        c.expect(&Tok::RParen)?;
        let mut it = vals.into_iter();
        let mut expr = eq(it
            .next()
            .expect("FOR VALUES IN requires at least one value"));
        for v in it {
            expr = Expr::Binary {
                op: BinOp::Or,
                left: Box::new(expr),
                right: Box::new(eq(v)),
            };
        }
        expr
    } else {

        c.expect_kw("from")?;
        c.expect(&Tok::LParen)?;
        let lo = c.expr()?;
        c.expect(&Tok::RParen)?;
        c.expect_kw("to")?;
        c.expect(&Tok::LParen)?;
        let hi = c.expr()?;
        c.expect(&Tok::RParen)?;
        Expr::Binary {
            op: BinOp::And,
            left: Box::new(Expr::Binary {
                op: BinOp::GtEq,
                left: keyref(),
                right: Box::new(lo),
            }),
            right: Box::new(Expr::Binary {
                op: BinOp::Lt,
                left: keyref(),
                right: Box::new(hi),
            }),
        }
    };
    let checks = vec![CheckConstraint {
        name: Some(format!("{child}_partition_check")),
        expr: check_expr,
    }];
    let ncols = pcols.len();
    let constraints = TableConstraints {
        not_null: vec![false; ncols],
        uniques: Vec::new(),
        checks,
        foreign_keys: Vec::new(),
    };
    catalog.create_typed_with_constraints(child, pcols, constraints, Vec::new());
    catalog.add_partition_child(&parent, child)?;
    Ok(empty())
}

fn route_partitioned_insert(
    catalog: &mut Catalog,
    parent: &str,
    schema: &Schema,
    new_rows: Vec<Row>,
    regs: &crate::types::registry::TypeRegistries,
) -> Result<Vec<Row>, PgError> {
    let (children, default_child) = catalog
        .partition_info(parent)
        .map(|p| (p.children.clone(), p.default_child.clone()))
        .unwrap_or_default();

    let mut checks: Vec<(String, Expr)> = Vec::new();
    for child in &children {
        if let Some(ck) = catalog
            .get(child)
            .and_then(|t| t.constraints.checks.first())
        {
            checks.push((child.clone(), resolve(&ck.expr, schema)?));
        }
    }
    let mut buckets: std::collections::HashMap<String, Vec<Row>> = std::collections::HashMap::new();
    let mut affected: Vec<Row> = Vec::new();
    for row in new_rows {
        let mut target: Option<String> = None;
        for (child, bound) in &checks {
            if matches!(eval_row(bound, &row, EvalCtx::new(regs))?, SqlValue::Int(1)) {
                target = Some(child.clone());
                break;
            }
        }

        let dest = target.or_else(|| default_child.clone());
        match dest {
            Some(child) => {
                buckets.entry(child).or_default().push(row.clone());
                affected.push(row);
            }
            None => {
                return Err(err(format!(
                    "no partition of relation \"{parent}\" found for row"
                )));
            }
        }
    }
    for (child, rows) in buckets {
        catalog
            .mvcc_insert(&child, rows)
            .ok_or_else(|| err(format!("relation \"{child}\" does not exist")))?;
    }
    Ok(affected)
}

fn serial_type_oid(t: &Tok) -> Option<u32> {
    use crate::types::oid;
    match t {
        Tok::Ident(s) => match s.as_str() {
            "serial" | "serial4" => Some(oid::INT4),
            "bigserial" | "serial8" => Some(oid::INT8),
            "smallserial" | "serial2" => Some(oid::INT2),
            _ => None,
        },
        _ => None,
    }
}

fn nextval_default(seq: &str) -> Expr {
    Expr::Func {
        name: "nextval".to_string(),
        args: vec![Expr::Str(seq.to_string())],
        distinct: false,
        filter: None,
        order_by: Vec::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overriding {
    None,
    System,
    User,
}

struct PendingSeq {
    name: String,
    opts: SeqOptions,
}

#[derive(Default, Clone)]
struct SeqOptions {
    increment: Option<i64>,
    min: Option<Option<i64>>,
    max: Option<Option<i64>>,
    start: Option<i64>,
    cache: Option<i64>,
    cycle: Option<bool>,
}

impl SeqOptions {

    fn into_seq(self, name: String) -> crate::catalog::SequenceDef {
        let increment = self.increment.unwrap_or(1);
        let asc = increment >= 0;
        let min = match self.min {
            Some(Some(m)) => m,
            _ => {
                if asc {
                    1
                } else {
                    i64::MIN
                }
            }
        };
        let max = match self.max {
            Some(Some(m)) => m,
            _ => {
                if asc {
                    i64::MAX
                } else {
                    -1
                }
            }
        };
        let start = self.start.unwrap_or(if asc { min } else { max });
        crate::catalog::SequenceDef {
            name,
            increment,
            min,
            max,
            start,
            cache: self.cache.unwrap_or(1),
            cycle: self.cycle.unwrap_or(false),
            current: None,
        }
    }
}

fn read_signed_int(c: &mut C) -> Result<i64, PgError> {
    let neg = matches!(c.peek(), Tok::Minus);
    if neg {
        c.pos += 1;
    }
    match c.peek() {
        Tok::Int(n) => {
            let v = *n;
            c.pos += 1;
            Ok(if neg { -v } else { v })
        }
        _ => Err(err(c.src.to_string())),
    }
}

fn parse_seq_options(
    c: &mut C,
    catalog: &Catalog,
    until_rparen: bool,
) -> Result<SeqOptions, PgError> {
    let mut o = SeqOptions::default();
    loop {
        match c.peek() {
            Tok::Eof => break,
            Tok::RParen if until_rparen => break,
            Tok::Comma => {
                c.pos += 1;
                continue;
            }
            _ => {}
        }
        if c.eat_kw("increment") {
            c.eat_kw("by");
            o.increment = Some(read_signed_int(c)?);
        } else if c.eat_kw("minvalue") {
            o.min = Some(Some(read_signed_int(c)?));
        } else if c.eat_kw("maxvalue") {
            o.max = Some(Some(read_signed_int(c)?));
        } else if c.eat_kw("start") {
            c.eat_kw("with");
            o.start = Some(read_signed_int(c)?);
        } else if c.eat_kw("cache") {
            o.cache = Some(read_signed_int(c)?);
        } else if c.eat_kw("cycle") {
            o.cycle = Some(true);
        } else if c.eat_kw("no") {
            if c.eat_kw("cycle") {
                o.cycle = Some(false);
            } else if c.eat_kw("minvalue") {
                o.min = Some(None);
            } else if c.eat_kw("maxvalue") {
                o.max = Some(None);
            } else {
                return Err(err(c.src.to_string()));
            }
        } else if c.eat_kw("as") {
            let _ = read_type(c, catalog)?;
        } else if c.eat_kw("owned") {
            c.expect_kw("by")?;

            let _ = c.ident();
            if matches!(c.peek(), Tok::Dot) {
                c.pos += 1;
                let _ = c.ident();
            }
        } else {
            break;
        }
    }
    Ok(o)
}

fn run_create_sequence(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("sequence")?;
    let if_not_exists = if c.eat_kw("if") {
        c.expect_kw("not")?;
        c.expect_kw("exists")?;
        true
    } else {
        false
    };
    let name = c.ident()?;
    let opts = parse_seq_options(c, catalog, false)?;
    catalog.create_sequence(opts.into_seq(name), if_not_exists)?;
    Ok(empty())
}

fn run_alter_sequence(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("sequence")?;
    let if_exists = c.eat_kw("if") && c.eat_kw("exists");
    let name = c.ident()?;
    if catalog.get_sequence(&name).is_none() {
        if if_exists {
            return Ok(empty());
        }
        return Err(err(format!("relation \"{name}\" does not exist")));
    }

    let restart: Option<Option<i64>> = if c.eat_kw("restart") {
        c.eat_kw("with");
        let with = if matches!(c.peek(), Tok::Int(_) | Tok::Minus) {
            Some(read_signed_int(c)?)
        } else {
            None
        };
        Some(with)
    } else {
        None
    };
    let opts = parse_seq_options(c, catalog, false)?;
    {
        let s = catalog.get_sequence_mut(&name).unwrap();
        if let Some(inc) = opts.increment {
            s.increment = inc;
        }
        if let Some(m) = opts.min {
            s.min = m.unwrap_or(if s.increment >= 0 { 1 } else { i64::MIN });
        }
        if let Some(m) = opts.max {
            s.max = m.unwrap_or(if s.increment >= 0 { i64::MAX } else { -1 });
        }
        if let Some(st) = opts.start {
            s.start = st;
        }
        if let Some(cc) = opts.cache {
            s.cache = cc;
        }
        if let Some(cy) = opts.cycle {
            s.cycle = cy;
        }
    }
    if let Some(with) = restart {
        catalog.seq_restart(&name, with)?;
    }
    Ok(empty())
}

struct FkSpec {
    child_cols: Vec<String>,
    parent: String,
    parent_cols: Option<Vec<String>>,
    on_delete: RefAction,
    on_update: RefAction,

    name: Option<String>,
    deferrable: bool,
    initially_deferred: bool,
}

fn default_key_name(
    table: &str,
    col_names: &[String],
    idxs: &[usize],
    is_pk: bool,
) -> Option<String> {
    if is_pk {
        return Some(format!("{table}_pkey"));
    }
    let mut parts = vec![table.to_string()];
    for &i in idxs {
        parts.push(col_names.get(i)?.clone());
    }
    parts.push("key".to_string());
    Some(parts.join("_"))
}

fn default_fk_name(table: &str, col_names: &[String], idxs: &[usize]) -> Option<String> {
    let mut parts = vec![table.to_string()];
    for &i in idxs {
        parts.push(col_names.get(i)?.clone());
    }
    parts.push("fkey".to_string());
    Some(parts.join("_"))
}

fn resolve_col_indices(names: &[String], wanted: &[String]) -> Option<Vec<usize>> {
    wanted
        .iter()
        .map(|w| names.iter().position(|n| n == w))
        .collect()
}

fn expr_refs_col(e: &Expr, name: &str) -> bool {
    use Expr::*;
    match e {
        Column(n) => n == name,
        Unary { expr, .. } | GenUnary { expr, .. } | Cast { expr, .. } | IsNull { expr, .. } => {
            expr_refs_col(expr, name)
        }
        Binary { left, right, .. } | GenBinary { left, right, .. } => {
            expr_refs_col(left, name) || expr_refs_col(right, name)
        }
        Func { args, .. } => args.iter().any(|a| expr_refs_col(a, name)),
        Case {
            operand,
            whens,
            else_,
        } => {
            operand.as_deref().is_some_and(|o| expr_refs_col(o, name))
                || whens
                    .iter()
                    .any(|(w, r)| expr_refs_col(w, name) || expr_refs_col(r, name))
                || else_.as_deref().is_some_and(|e| expr_refs_col(e, name))
        }
        _ => false,
    }
}

fn run_alter(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let regs = Arc::new(catalog.type_registries());
    let regs = &regs;
    c.expect_kw("alter")?;
    if c.at_kw("sequence") {
        return run_alter_sequence(c, catalog);
    }
    if c.at_kw("type") {
        return run_alter_type(c, catalog);
    }
    c.expect_kw("table")?;
    let if_exists = c.eat_kw("if") && c.eat_kw("exists");
    let name = c.ident()?;
    if catalog.get(&name).is_none() {
        if if_exists {
            return Ok(empty());
        }
        return Err(err(format!("relation \"{name}\" does not exist")));
    }

    if c.eat_kw("rename") {
        if c.eat_kw("to") {
            let newname = c.ident()?;
            if !catalog.rename_table(&name, &newname) {
                return Err(err(format!("relation \"{newname}\" already exists")));
            }
            return Ok(empty());
        }
        c.eat_kw("column");
        let old = c.ident()?;
        c.expect_kw("to")?;
        let new = c.ident()?;
        let t = catalog.get_table_mut(&name).unwrap();
        let names = t.schema.names();
        let idx = names.iter().position(|n| n == &old).ok_or_else(|| {
            err(format!(
                "column \"{old}\" of relation \"{name}\" does not exist"
            ))
        })?;
        if t.constraints
            .checks
            .iter()
            .any(|ck| expr_refs_col(&ck.expr, &old))
        {
            return Err(err(format!(
                "cannot rename column \"{old}\": referenced by a CHECK constraint"
            )));
        }
        let mut new_names = names;
        new_names[idx] = new;
        t.schema = Schema::new(new_names);
        return Ok(empty());
    }

    if c.eat_kw("add") {

        let cname: Option<String> = if c.eat_kw("constraint") {
            Some(c.ident()?)
        } else {
            None
        };
        if cname.is_some() || is_table_constraint_kw(c.peek()) {
            let mut named_uniques: Vec<(Vec<String>, bool, Option<String>)> = Vec::new();
            let mut checks: Vec<CheckConstraint> = Vec::new();
            let mut fks: Vec<FkSpec> = Vec::new();
            parse_table_constraint(c, cname, &mut named_uniques, &mut checks, &mut fks)?;

            if !fks.is_empty() {
                return alter_add_foreign_key(catalog, &name, fks.remove(0));
            }

            let vis_rows = catalog.visible_rows(&name).unwrap_or_default();
            let t = catalog.get_table_mut(&name).unwrap();
            let names = t.schema.names();
            let mut cand = t.constraints.clone();
            for (colnames, is_pk, kname) in named_uniques {
                let idxs = resolve_col_indices(&names, &colnames)
                    .ok_or_else(|| err("column named in key does not exist".to_string()))?;
                if is_pk {
                    for &i in &idxs {
                        cand.not_null[i] = true;
                    }
                }
                let key_name = kname.or_else(|| default_key_name(&name, &names, &idxs, is_pk));
                cand.uniques.push(UniqueKey {
                    cols: idxs,
                    is_primary: is_pk,
                    name: key_name,
                });
            }
            cand.checks.extend(checks);

            validate_constraints(&name, &Schema::new(names), &cand, &vis_rows, regs)?;
            t.constraints = cand;
            return Ok(empty());
        }

        c.eat_kw("column");
        let col = c.ident()?;
        let (oid, typmod) = read_type(c, catalog)?;

        let mut notnull = false;
        let mut default: Option<Expr> = None;
        loop {
            if c.eat_kw("not") {
                c.expect_kw("null")?;
                notnull = true;
            } else if c.eat_kw("null") {

            } else if c.eat_kw("default") {
                default = Some(c.expr()?);
            } else {
                break;
            }
        }

        let fill = match &default {
            Some(dexpr) => coerce(
                eval_ctx(dexpr, EvalCtx::new(regs))?,
                Some(oid),
                typmod,
                regs,
            )?,
            None => SqlValue::Null,
        };

        let has_visible_rows = catalog
            .visible_rows(&name)
            .map(|r| !r.is_empty())
            .unwrap_or(false);
        let t = catalog.get_table_mut(&name).unwrap();
        if notnull && default.is_none() && has_visible_rows {
            return Err(err(format!(
                "column \"{col}\" of relation \"{name}\" contains null values"
            )));
        }
        let mut names = t.schema.names();
        names.push(col);
        t.schema = Schema::new(names);
        t.col_types.push(oid);
        t.col_typmods.push(typmod);
        t.constraints.not_null.push(notnull);

        if !t.defaults.is_empty() || default.is_some() {
            t.defaults.resize(t.col_types.len() - 1, None);
            t.defaults.push(default);
        }
        for row in &mut t.rows {
            row.push(fill.clone());
        }
        return Ok(empty());
    }

    if c.at_kw("drop") && matches!(c.toks.get(c.pos + 1), Some(Tok::Ident(s)) if s == "constraint")
    {
        c.pos += 2;
        let con_if_exists = c.eat_kw("if") && c.eat_kw("exists");
        let cname = c.ident()?;
        return alter_drop_constraint(catalog, &name, &cname, con_if_exists);
    }

    if c.eat_kw("drop") {
        c.eat_kw("column");
        let col_if_exists = c.eat_kw("if") && c.eat_kw("exists");
        let col = c.ident()?;
        let t = catalog.get_table_mut(&name).unwrap();
        let names = t.schema.names();
        let idx = match names.iter().position(|n| n == &col) {
            Some(i) => i,
            None if col_if_exists => return Ok(empty()),
            None => {
                return Err(err(format!(
                    "column \"{col}\" of relation \"{name}\" does not exist"
                )))
            }
        };

        let in_key = t.constraints.uniques.iter().any(|k| k.cols.contains(&idx))
            || t.constraints
                .foreign_keys
                .iter()
                .any(|f| f.cols.contains(&idx));
        let in_check = t
            .constraints
            .checks
            .iter()
            .any(|ck| expr_refs_col(&ck.expr, &col));
        if in_key || in_check {
            return Err(err(format!(
                "cannot drop column \"{col}\" because a constraint depends on it"
            )));
        }

        let mut new_names = names;
        new_names.remove(idx);
        t.schema = Schema::new(new_names);
        t.col_types.remove(idx);
        t.col_typmods.remove(idx);
        if idx < t.constraints.not_null.len() {
            t.constraints.not_null.remove(idx);
        }
        for row in &mut t.rows {
            if idx < row.len() {
                row.remove(idx);
            }
        }
        let shift = |cols: &mut Vec<usize>| {
            for i in cols.iter_mut() {
                if *i > idx {
                    *i -= 1;
                }
            }
        };
        for k in &mut t.constraints.uniques {
            shift(&mut k.cols);
        }
        for f in &mut t.constraints.foreign_keys {
            shift(&mut f.cols);
        }
        return Ok(empty());
    }

    if c.eat_kw("alter") {
        c.eat_kw("column");
        let col = c.ident()?;

        let idx = {
            let t = catalog.get(&name).unwrap();
            t.schema
                .names()
                .iter()
                .position(|n| n == &col)
                .ok_or_else(|| {
                    err(format!(
                        "column \"{col}\" of relation \"{name}\" does not exist"
                    ))
                })?
        };

        c.eat_kw("set");
        let saw_data = c.eat_kw("data");
        if c.eat_kw("type") {
            let (new_oid, new_typmod) = read_type(c, catalog)?;
            let using = if c.eat_kw("using") {
                Some(c.expr()?)
            } else {
                None
            };
            let t = catalog.get_table_mut(&name).unwrap();
            let mut new_cells: Vec<SqlValue> = Vec::with_capacity(t.rows.len());
            match &using {
                Some(e) => {
                    let scalar = lower(e, &t.schema, regs.clone())?;
                    for r in &t.rows {
                        let v = scalar(r).map_err(err)?;
                        new_cells.push(coerce(v, Some(new_oid), new_typmod, regs)?);
                    }
                }
                None => {
                    for r in &t.rows {
                        let v = r.get(idx).cloned().unwrap_or(SqlValue::Null);
                        new_cells.push(coerce(v, Some(new_oid), new_typmod, regs)?);
                    }
                }
            }
            for (r, cell) in t.rows.iter_mut().zip(new_cells) {
                if idx < r.len() {
                    r[idx] = cell;
                }
            }
            if idx < t.col_types.len() {
                t.col_types[idx] = new_oid;
            }
            if idx < t.col_typmods.len() {
                t.col_typmods[idx] = new_typmod;
            }
            return Ok(empty());
        }

        if saw_data {
            return Err(err(c.src.to_string()));
        }

        let vis_has_null = catalog
            .visible_rows(&name)
            .map(|rows| {
                rows.iter()
                    .any(|r| matches!(r.get(idx), Some(SqlValue::Null)))
            })
            .unwrap_or(false);
        let t = catalog.get_table_mut(&name).unwrap();
        if t.constraints.not_null.len() < t.col_types.len() {
            t.constraints.not_null.resize(t.col_types.len(), false);
        }
        if c.at_kw("not") {
            c.expect_kw("not")?;
            c.expect_kw("null")?;
            if vis_has_null {
                return Err(err(format!(
                    "column \"{col}\" of relation \"{name}\" contains null values"
                )));
            }
            t.constraints.not_null[idx] = true;
            return Ok(empty());
        }
        if c.eat_kw("default") {
            let dexpr = c.expr()?;
            if t.defaults.len() < t.col_types.len() {
                t.defaults.resize(t.col_types.len(), None);
            }
            t.defaults[idx] = Some(dexpr);
            return Ok(empty());
        }

        if c.eat_kw("drop") {
            if c.eat_kw("not") {
                c.expect_kw("null")?;
                t.constraints.not_null[idx] = false;
                return Ok(empty());
            }
            if c.eat_kw("default") {
                if idx < t.defaults.len() {
                    t.defaults[idx] = None;
                }
                return Ok(empty());
            }
            return Err(err(c.src.to_string()));
        }
        return Err(err(c.src.to_string()));
    }

    Err(err(c.src.to_string()))
}

fn alter_drop_constraint(
    catalog: &mut Catalog,
    name: &str,
    cname: &str,
    if_exists: bool,
) -> Result<QueryResult, PgError> {

    let t = catalog.get(name).unwrap();
    let uniq_pos = t
        .constraints
        .uniques
        .iter()
        .position(|k| k.name.as_deref() == Some(cname));
    let check_pos = t
        .constraints
        .checks
        .iter()
        .position(|k| k.name.as_deref() == Some(cname));
    let fk_pos = t
        .constraints
        .foreign_keys
        .iter()
        .position(|k| k.name.as_deref() == Some(cname));

    if uniq_pos.is_none() && check_pos.is_none() && fk_pos.is_none() {
        if if_exists {
            return Ok(empty());
        }
        return Err(err(format!(
            "constraint \"{cname}\" of relation \"{name}\" does not exist"
        )));
    }

    if let Some(pos) = uniq_pos {
        let key_cols = t.constraints.uniques[pos].cols.clone();
        let depended_on = catalog.tables_iter().any(|(_, ct)| {
            ct.constraints
                .foreign_keys
                .iter()
                .any(|fk| fk.parent == name && same_set(&fk.parent_cols, &key_cols))
        });
        if depended_on {
            return Err(err(format!(
                "cannot drop constraint {cname} on table {name} because other objects depend on it"
            )));
        }
    }

    let t = catalog.get_table_mut(name).unwrap();
    if let Some(pos) = uniq_pos {
        t.constraints.uniques.remove(pos);
    } else if let Some(pos) = check_pos {
        t.constraints.checks.remove(pos);
    } else if let Some(pos) = fk_pos {
        t.constraints.foreign_keys.remove(pos);
    }
    Ok(empty())
}

fn alter_add_foreign_key(
    catalog: &mut Catalog,
    name: &str,
    spec: FkSpec,
) -> Result<QueryResult, PgError> {
    let names = catalog.get(name).unwrap().schema.names();
    let child_cols = resolve_col_indices(&names, &spec.child_cols)
        .ok_or_else(|| err("column named in key does not exist".to_string()))?;

    let (parent_names, parent_uniques): (Vec<String>, Vec<UniqueKey>) = if spec.parent == name {
        let t = catalog.get(name).unwrap();
        (t.schema.names(), t.constraints.uniques.clone())
    } else {
        let p = catalog
            .get(&spec.parent)
            .ok_or_else(|| err(format!("relation \"{}\" does not exist", spec.parent)))?;
        (p.schema.names(), p.constraints.uniques.clone())
    };
    let parent_cols = match &spec.parent_cols {
        Some(cns) => resolve_col_indices(&parent_names, cns).ok_or_else(|| {
            err(format!(
                "column named in foreign key does not exist in referenced table \"{}\"",
                spec.parent
            ))
        })?,
        None => parent_uniques
            .iter()
            .find(|u| u.is_primary)
            .map(|u| u.cols.clone())
            .ok_or_else(|| {
                err(format!(
                    "there is no primary key for referenced table \"{}\"",
                    spec.parent
                ))
            })?,
    };
    if !parent_uniques
        .iter()
        .any(|u| same_set(&u.cols, &parent_cols))
    {
        return Err(err(format!(
            "there is no unique constraint matching given keys for referenced table \"{}\"",
            spec.parent
        )));
    }
    if child_cols.len() != parent_cols.len() {
        return Err(err(
            "number of referencing and referenced columns for foreign key disagree".to_string(),
        ));
    }

    let fk_name = spec
        .name
        .clone()
        .or_else(|| default_fk_name(name, &names, &child_cols));
    let fk = ForeignKey {
        cols: child_cols,
        parent: spec.parent,
        parent_cols,
        on_delete: spec.on_delete,
        on_update: spec.on_update,
        name: fk_name,
        deferrable: spec.deferrable,
        initially_deferred: spec.initially_deferred,
    };

    let cons = TableConstraints {
        foreign_keys: vec![fk.clone()],
        ..Default::default()
    };
    let child_rows = catalog.visible_rows(name).unwrap();
    validate_foreign_keys(catalog, name, &cons, &child_rows)?;

    catalog
        .get_table_mut(name)
        .unwrap()
        .constraints
        .foreign_keys
        .push(fk);
    Ok(empty())
}

fn same_set(a: &[usize], b: &[usize]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().all(|x| b.contains(x))
}

fn is_table_constraint_kw(t: &Tok) -> bool {
    matches!(t, Tok::Ident(s) if matches!(s.as_str(),
        "primary" | "unique" | "check" | "foreign" | "exclude"))
}

fn parse_table_constraint(
    c: &mut C,
    cname: Option<String>,
    named_uniques: &mut Vec<(Vec<String>, bool, Option<String>)>,
    checks: &mut Vec<CheckConstraint>,
    fks: &mut Vec<FkSpec>,
) -> Result<(), PgError> {
    if c.eat_kw("primary") {
        c.expect_kw("key")?;
        named_uniques.push((parse_col_name_list(c)?, true, cname));
    } else if c.eat_kw("unique") {
        named_uniques.push((parse_col_name_list(c)?, false, cname));
    } else if c.eat_kw("check") {
        checks.push(CheckConstraint {
            name: cname,
            expr: parse_check_expr(c)?,
        });
    } else if c.eat_kw("foreign") {

        c.expect_kw("key")?;
        let child_cols = parse_col_name_list(c)?;
        c.expect_kw("references")?;
        fks.push(parse_references(c, child_cols, cname)?);
    } else {

        c.skip_to_column_delim();
    }
    Ok(())
}

fn parse_references(
    c: &mut C,
    child_cols: Vec<String>,
    cname: Option<String>,
) -> Result<FkSpec, PgError> {
    let parent = c.ident()?;
    let parent_cols = if matches!(c.peek(), Tok::LParen) {
        Some(parse_col_name_list(c)?)
    } else {
        None
    };
    let mut on_delete = RefAction::NoAction;
    let mut on_update = RefAction::NoAction;
    let mut deferrable = false;
    let mut initially_deferred = false;
    loop {
        if c.eat_kw("on") {
            if c.eat_kw("delete") {
                on_delete = parse_ref_action(c)?;
            } else if c.eat_kw("update") {
                on_update = parse_ref_action(c)?;
            } else {
                return Err(err(c.src.to_string()));
            }
        } else if c.eat_kw("match") {

            let _ = c.ident()?;
        } else if c.eat_kw("not") {

            c.expect_kw("deferrable")?;
            deferrable = false;
        } else if c.eat_kw("deferrable") {
            deferrable = true;
        } else if c.eat_kw("initially") {

            if c.eat_kw("deferred") {
                initially_deferred = true;
                deferrable = true;
            } else if c.eat_kw("immediate") {
                initially_deferred = false;
            } else {
                return Err(err(c.src.to_string()));
            }
        } else {
            break;
        }
    }
    Ok(FkSpec {
        child_cols,
        parent,
        parent_cols,
        on_delete,
        on_update,
        name: cname,
        deferrable,
        initially_deferred,
    })
}

fn parse_ref_action(c: &mut C) -> Result<RefAction, PgError> {
    if c.eat_kw("no") {
        c.expect_kw("action")?;
        Ok(RefAction::NoAction)
    } else if c.eat_kw("restrict") {
        Ok(RefAction::Restrict)
    } else if c.eat_kw("cascade") {
        Ok(RefAction::Cascade)
    } else if c.eat_kw("set") {
        if c.eat_kw("null") {
            Ok(RefAction::SetNull)
        } else if c.eat_kw("default") {
            Ok(RefAction::SetNull)
        } else {
            Err(err(c.src.to_string()))
        }
    } else {
        Err(err(c.src.to_string()))
    }
}

fn parse_col_name_list(c: &mut C) -> Result<Vec<String>, PgError> {
    c.expect(&Tok::LParen)?;
    let mut names = Vec::new();
    loop {
        names.push(c.ident()?);
        if matches!(c.peek(), Tok::Comma) {
            c.pos += 1;
        } else {
            break;
        }
    }
    c.expect(&Tok::RParen)?;
    Ok(names)
}

fn parse_check_expr(c: &mut C) -> Result<Expr, PgError> {
    if !matches!(c.peek(), Tok::LParen) {
        return Err(err(c.src.to_string()));
    }
    c.expr()
}

fn parse_column_constraints(
    c: &mut C,
    catalog: &Catalog,
    idx: usize,
    col_name: &str,
    table_name: &str,
    mut cname: Option<String>,
    not_null: &mut [bool],
    defaults: &mut [Option<Expr>],
    generated: &mut [Option<Expr>],
    identity: &mut [crate::catalog::IdentityKind],
    implicit_seqs: &mut Vec<PendingSeq>,
    col_uniques: &mut Vec<(Vec<usize>, bool, Option<String>)>,
    checks: &mut Vec<CheckConstraint>,
    fks: &mut Vec<FkSpec>,
) -> Result<(), PgError> {
    loop {
        match c.peek() {
            Tok::Eof => return Ok(()),
            Tok::RParen | Tok::Comma => return Ok(()),
            Tok::Ident(s) => match s.as_str() {
                "not" => {
                    c.pos += 1;
                    c.expect_kw("null")?;
                    not_null[idx] = true;
                }
                "null" => {
                    c.pos += 1;
                }
                "primary" => {
                    c.pos += 1;
                    c.expect_kw("key")?;
                    not_null[idx] = true;
                    col_uniques.push((vec![idx], true, cname.take()));
                }
                "unique" => {
                    c.pos += 1;
                    col_uniques.push((vec![idx], false, cname.take()));
                }
                "check" => {
                    c.pos += 1;
                    checks.push(CheckConstraint {
                        name: cname.take(),
                        expr: parse_check_expr(c)?,
                    });
                }
                "default" => {
                    c.pos += 1;

                    defaults[idx] = Some(c.expr()?);
                }

                "generated" => {
                    c.pos += 1;
                    let by_default = if c.eat_kw("always") {
                        false
                    } else if c.eat_kw("by") {
                        c.expect_kw("default")?;
                        true
                    } else {
                        return Err(err(c.src.to_string()));
                    };
                    c.expect_kw("as")?;
                    if matches!(c.peek(), Tok::LParen) {

                        if by_default {
                            return Err(err(c.src.to_string()));
                        }
                        let gexpr = c.expr()?;
                        if c.eat_kw("stored") {
                            generated[idx] = Some(gexpr);
                        } else if c.at_kw("virtual") {
                            return Err(err(
                                "VIRTUAL generated columns are not supported".to_string()
                            ));
                        } else {
                            return Err(err(c.src.to_string()));
                        }
                    } else {

                        c.expect_kw("identity")?;
                        let opts = if matches!(c.peek(), Tok::LParen) {
                            c.expect(&Tok::LParen)?;
                            let o = parse_seq_options(c, catalog, true)?;
                            c.expect(&Tok::RParen)?;
                            o
                        } else {
                            SeqOptions::default()
                        };
                        let seq_nm = format!("{table_name}_{col_name}_seq");
                        not_null[idx] = true;
                        identity[idx] = if by_default {
                            crate::catalog::IdentityKind::ByDefault
                        } else {
                            crate::catalog::IdentityKind::Always
                        };
                        defaults[idx] = Some(nextval_default(&seq_nm));
                        implicit_seqs.push(PendingSeq { name: seq_nm, opts });
                    }
                }
                "references" => {

                    c.pos += 1;
                    fks.push(parse_references(
                        c,
                        vec![col_name.to_string()],
                        cname.take(),
                    )?);
                }

                "constraint" => {
                    c.pos += 1;
                    cname = Some(c.ident()?);
                }

                "collate" => {
                    c.pos += 1;
                    let mut name = c.ident()?;
                    if matches!(c.peek(), Tok::Dot) {
                        c.pos += 1;
                        name = c.ident()?;
                    }
                    crate::collation::validate_for_comparison(&name)?;
                }

                _ => {
                    c.pos += 1;
                }
            },

            _ => {
                c.pos += 1;
            }
        }
    }
}

fn run_create_view(
    c: &mut C,
    catalog: &mut Catalog,
    or_replace: bool,
) -> Result<QueryResult, PgError> {
    c.expect_kw("view")?;
    let name = c.ident()?;

    let columns: Option<Vec<String>> = if matches!(c.peek(), Tok::LParen) {
        c.pos += 1;
        let mut names = Vec::new();
        loop {
            names.push(c.ident()?);
            if matches!(c.peek(), Tok::Comma) {
                c.pos += 1;
            } else {
                break;
            }
        }
        c.expect(&Tok::RParen)?;
        Some(names)
    } else {
        None
    };

    c.expect_kw("as")?;

    let query_start = c.pos;
    let (query, next) = crate::stmt::parser::parse_query_at(c.toks, c.pos, c.src)?;
    let query_sql = render_query_tokens(&c.toks[query_start..next]);
    c.pos = next;

    let check_option = if c.eat_kw("with") {
        let _ = c.eat_kw("local") || c.eat_kw("cascaded");
        c.expect_kw("check")?;
        c.expect_kw("option")?;
        true
    } else {
        false
    };

    if !matches!(c.peek(), Tok::Eof) {
        return Err(err(c.src.to_string()));
    }

    if catalog.get(&name).is_some() {
        return Err(err(format!("relation \"{name}\" already exists")));
    }
    if catalog.get_view(&name).is_some() && !or_replace {
        return Err(err(format!("relation \"{name}\" already exists")));
    }

    catalog.create_view(&name, query, query_sql, columns, check_option);
    Ok(empty())
}

fn parse_view_head(
    c: &mut C,
) -> Result<(String, Option<Vec<String>>, SelectStmt, String), PgError> {
    let name = c.ident()?;
    let columns: Option<Vec<String>> = if matches!(c.peek(), Tok::LParen) {
        c.pos += 1;
        let mut names = Vec::new();
        loop {
            names.push(c.ident()?);
            if matches!(c.peek(), Tok::Comma) {
                c.pos += 1;
            } else {
                break;
            }
        }
        c.expect(&Tok::RParen)?;
        Some(names)
    } else {
        None
    };
    c.expect_kw("as")?;
    let query_start = c.pos;
    let (query, next) = crate::stmt::parser::parse_query_at(c.toks, c.pos, c.src)?;
    let query_sql = render_query_tokens(&c.toks[query_start..next]);
    c.pos = next;
    if !matches!(c.peek(), Tok::Eof) {
        return Err(err(c.src.to_string()));
    }
    Ok((name, columns, query, query_sql))
}

fn materialize(
    query: &SelectStmt,
    columns: &Option<Vec<String>>,
    catalog: &Catalog,
) -> Result<(Vec<String>, Vec<u32>, Vec<Row>), PgError> {
    let r = run_select(query, catalog)?;
    let mut cols = r.columns.clone();
    if let Some(aliases) = columns {
        for (i, a) in aliases.iter().enumerate() {
            if i < cols.len() {
                cols[i] = a.clone();
            }
        }
    }
    let mut oids = r.col_types.clone();
    oids.resize(cols.len(), 0);
    Ok((cols, oids, r.rows))
}

fn run_create_matview(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("materialized")?;
    c.expect_kw("view")?;
    let (name, columns, query, query_sql) = parse_view_head(c)?;

    if catalog.get(&name).is_some() || catalog.get_view(&name).is_some() {
        return Err(err(format!("relation \"{name}\" already exists")));
    }

    let (col_names, col_types, rows) = materialize(&query, &columns, catalog)?;
    catalog.create_matview(&name, query, query_sql, columns, col_names, col_types, rows);
    Ok(empty())
}

fn run_refresh(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("refresh")?;
    c.expect_kw("materialized")?;
    c.expect_kw("view")?;
    let name = c.ident()?;

    let (query, columns) = match catalog.get_view(&name) {
        Some(v) if v.materialized => (v.query.clone(), v.columns.clone()),
        Some(_) => return Err(err(format!("\"{name}\" is not a materialized view"))),
        None => return Err(err(format!("relation \"{name}\" does not exist"))),
    };
    let (col_names, col_types, rows) = materialize(&query, &columns, catalog)?;
    catalog
        .refresh_matview(&name, col_names, col_types, rows)
        .ok_or_else(|| err(format!("\"{name}\" is not a materialized view")))?;
    Ok(empty())
}

struct Updatable {

    base: String,

    map: Vec<(String, String)>,

    filter: Option<Expr>,

    check: bool,
}

fn rename_columns(e: &Expr, name_map: &std::collections::HashMap<String, String>) -> Expr {
    let rn = |b: &Expr| Box::new(rename_columns(b, name_map));
    match e {
        Expr::Column(n) => Expr::Column(name_map.get(n).cloned().unwrap_or_else(|| n.clone())),
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: rn(expr),
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: rn(left),
            right: rn(right),
        },
        Expr::GenBinary { op, left, right } => Expr::GenBinary {
            op: op.clone(),
            left: rn(left),
            right: rn(right),
        },
        Expr::GenUnary { op, expr } => Expr::GenUnary {
            op: op.clone(),
            expr: rn(expr),
        },
        Expr::Cast { expr, type_name } => Expr::Cast {
            expr: rn(expr),
            type_name: type_name.clone(),
        },
        Expr::IsNull { expr, negated } => Expr::IsNull {
            expr: rn(expr),
            negated: *negated,
        },
        Expr::Func {
            name,
            args,
            distinct,
            filter,
            order_by,
        } => Expr::Func {
            name: name.clone(),
            args: args.iter().map(|a| rename_columns(a, name_map)).collect(),
            distinct: *distinct,
            filter: filter.as_ref().map(|f| rn(f)),
            order_by: order_by.clone(),
        },
        Expr::Case {
            operand,
            whens,
            else_,
        } => Expr::Case {
            operand: operand.as_ref().map(|o| rn(o)),
            whens: whens
                .iter()
                .map(|(c, r)| (rename_columns(c, name_map), rename_columns(r, name_map)))
                .collect(),
            else_: else_.as_ref().map(|d| rn(d)),
        },

        other => other.clone(),
    }
}

fn and_opt(a: Option<Expr>, b: Option<Expr>) -> Option<Expr> {
    match (a, b) {
        (Some(x), Some(y)) => Some(Expr::Binary {
            op: BinOp::And,
            left: Box::new(x),
            right: Box::new(y),
        }),
        (Some(x), None) => Some(x),
        (None, y) => y,
    }
}

fn view_updatable(v: &View, catalog: &Catalog) -> Option<Updatable> {
    if v.materialized {
        return None;
    }
    let q = &v.query;
    if q.distinct
        || !q.distinct_on.is_empty()
        || !q.group_by.is_empty()
        || !q.grouping_sets.is_empty()
        || q.having.is_some()
        || !q.tail.is_empty()
        || !q.windows.is_empty()
    {
        return None;
    }
    let source = match &q.from {
        Some(FromItem::Table { name, .. }) => name.clone(),
        _ => return None,
    };

    let (source_names, inner): (Vec<String>, Option<Updatable>) =
        if let Some(t) = catalog.get(&source) {
            (t.schema.names().to_vec(), None)
        } else {

            let inner_view = catalog.get_view(&source)?;
            let up = view_updatable(inner_view, catalog)?;
            let names = up.map.iter().map(|(o, _)| o.clone()).collect();
            (names, Some(up))
        };

    let mut proj: Vec<(String, String)> = Vec::new();
    for item in &q.projection {
        match item {
            SelectItem::Star => {
                for n in &source_names {
                    proj.push((n.clone(), n.clone()));
                }
            }
            SelectItem::Expr {
                expr: Expr::Column(src_col),
                alias,
            } => {
                let out = alias.clone().unwrap_or_else(|| src_col.clone());
                proj.push((out, src_col.clone()));
            }

            _ => return None,
        }
    }

    if let Some(aliases) = &v.columns {
        for (i, a) in aliases.iter().enumerate() {
            if let Some(e) = proj.get_mut(i) {
                e.0 = a.clone();
            }
        }
    }

    match inner {

        None => Some(Updatable {
            base: source,
            map: proj,
            filter: q.filter.clone(),
            check: v.check_option,
        }),

        Some(up) => {
            let src_to_base: std::collections::HashMap<String, String> =
                up.map.iter().map(|(o, b)| (o.clone(), b.clone())).collect();
            let mut map: Vec<(String, String)> = Vec::new();
            for (out, src_col) in &proj {
                let base_col = src_to_base.get(src_col)?.clone();
                map.push((out.clone(), base_col));
            }

            let this_filter = q.filter.as_ref().map(|e| rename_columns(e, &src_to_base));
            let filter = and_opt(up.filter, this_filter);
            Some(Updatable {
                base: up.base,
                map,
                filter,
                check: v.check_option || up.check,
            })
        }
    }
}

fn not_updatable(verb: &str, name: &str) -> PgError {
    err(format!(
        "cannot {verb} view \"{name}\" because it does not select from a single \
         table using only column references and is not updatable"
    ))
}

fn run_insert_view(c: &mut C, catalog: &mut Catalog, vname: &str) -> Result<QueryResult, PgError> {
    let regs = Arc::new(catalog.type_registries());
    let regs = &regs;
    let v = catalog.get_view(vname).expect("view exists").clone();
    if v.materialized {
        return Err(err(format!("cannot change materialized view \"{vname}\"")));
    }
    let up = view_updatable(&v, catalog).ok_or_else(|| not_updatable("insert into", vname))?;

    let (base_schema, base_types, base_typmods, base_constraints, base_rows) = {
        let t = catalog
            .get(&up.base)
            .ok_or_else(|| err(format!("relation \"{}\" does not exist", up.base)))?;
        (
            t.schema.clone(),
            t.col_types.clone(),
            t.col_typmods.clone(),
            t.constraints.clone(),
            catalog.visible_rows(&up.base).unwrap(),
        )
    };
    let width = base_schema.width();
    let typmod_of = |slot: usize| {
        base_typmods
            .get(slot)
            .copied()
            .unwrap_or(types::typmod::NONE)
    };

    let view_cols: Vec<String> = if matches!(c.peek(), Tok::LParen) {
        c.pos += 1;
        let mut cols = Vec::new();
        loop {
            cols.push(c.ident()?);
            if matches!(c.peek(), Tok::Comma) {
                c.pos += 1;
            } else {
                break;
            }
        }
        c.expect(&Tok::RParen)?;
        cols
    } else {
        up.map.iter().map(|(o, _)| o.clone()).collect()
    };

    let mut target: Vec<usize> = Vec::new();
    for vc in &view_cols {
        let base_col = up
            .map
            .iter()
            .find(|(o, _)| o == vc)
            .map(|(_, b)| b.clone())
            .ok_or_else(|| {
                err(format!(
                    "column \"{vc}\" of relation \"{vname}\" does not exist"
                ))
            })?;
        let idx = base_schema.index_of(&base_col).ok_or_else(|| {
            err(format!(
                "column \"{base_col}\" of relation \"{}\" does not exist",
                up.base
            ))
        })?;
        target.push(idx);
    }

    c.expect_kw("values")?;
    let mut new_rows: Vec<Row> = Vec::new();
    loop {
        c.expect(&Tok::LParen)?;
        let mut vals: Vec<SqlValue> = Vec::new();
        loop {
            let e = c.expr()?;
            vals.push(eval_ctx(&e, EvalCtx::new(regs))?);
            if matches!(c.peek(), Tok::Comma) {
                c.pos += 1;
            } else {
                break;
            }
        }
        c.expect(&Tok::RParen)?;
        if vals.len() != target.len() {
            return Err(err(format!(
                "INSERT has more expressions than target columns in relation \"{vname}\""
            )));
        }
        let mut row: Row = vec![SqlValue::Null; width];
        for (slot, val) in target.iter().zip(vals) {
            let oid = base_types.get(*slot).copied();
            row[*slot] = coerce(val, oid, typmod_of(*slot), regs)?;
        }
        new_rows.push(row);
        if matches!(c.peek(), Tok::Comma) {
            c.pos += 1;
        } else {
            break;
        }
    }
    let returning = parse_returning(c)?;

    for row in &new_rows {
        enforce_check_option(&up, &base_schema, row, vname, regs)?;
    }

    let mut all = base_rows;
    all.extend(new_rows.iter().cloned());
    validate_constraints(&up.base, &base_schema, &base_constraints, &all, regs)?;
    validate_unique_indexes(catalog, &up.base, &all)?;
    catalog
        .mvcc_insert(&up.base, new_rows.clone())
        .ok_or_else(|| err(format!("relation \"{}\" does not exist", up.base)))?;
    match returning {
        Some(items) => {
            let (_vs, base_idx) = view_binding(&up, &base_schema)?;
            view_returning(
                &items,
                &up,
                &base_schema,
                &base_types,
                &base_idx,
                &new_rows,
                regs,
            )
        }
        None => Ok(empty()),
    }
}

fn view_binding(up: &Updatable, base_schema: &Schema) -> Result<(Schema, Vec<usize>), PgError> {
    let view_names: Vec<String> = up.map.iter().map(|(o, _)| o.clone()).collect();
    let mut base_idx = Vec::with_capacity(up.map.len());
    for (_, b) in &up.map {
        base_idx.push(
            base_schema
                .index_of(b)
                .ok_or_else(|| err(format!("column \"{b}\" of relation does not exist")))?,
        );
    }
    Ok((Schema::new(view_names), base_idx))
}

fn enforce_check_option(
    up: &Updatable,
    base_schema: &Schema,
    row: &Row,
    vname: &str,
    regs: &Arc<TypeRegistries>,
) -> Result<(), PgError> {
    if !up.check {
        return Ok(());
    }
    if let Some(f) = &up.filter {
        let p = lower_pred(f, base_schema, regs.clone())?;
        if !p(row).map_err(err)? {
            return Err(err(format!(
                "new row violates check option for view \"{vname}\""
            )));
        }
    }
    Ok(())
}

fn view_returning(
    items: &[SelectItem],
    up: &Updatable,
    base_schema: &Schema,
    base_types: &[u32],
    base_idx: &[usize],
    rows: &[Row],
    regs: &Arc<TypeRegistries>,
) -> Result<QueryResult, PgError> {
    let view_names: Vec<String> = up.map.iter().map(|(o, _)| o.clone()).collect();
    let view_schema = Schema::new(view_names);
    let view_types: Vec<u32> = base_idx
        .iter()
        .map(|&bi| base_types.get(bi).copied().unwrap_or(0))
        .collect();
    let view_rows: Vec<Row> = rows
        .iter()
        .map(|r| base_idx.iter().map(|&bi| r[bi].clone()).collect())
        .collect();
    let _ = base_schema;
    project_returning(items, &view_schema, &view_types, &view_rows, regs)
}

fn run_update_view(c: &mut C, catalog: &mut Catalog, vname: &str) -> Result<QueryResult, PgError> {
    let regs = Arc::new(catalog.type_registries());
    let regs = &regs;
    let v = catalog.get_view(vname).expect("view exists").clone();
    if v.materialized {
        return Err(err(format!("cannot change materialized view \"{vname}\"")));
    }
    let up = view_updatable(&v, catalog).ok_or_else(|| not_updatable("update", vname))?;

    if c.eat_kw("as") {
        c.ident()?;
    } else if let Tok::Ident(a) = c.peek() {
        if a != "set" {
            c.pos += 1;
        }
    }
    c.expect_kw("set")?;

    let (base_schema, base_types, base_typmods, base_constraints, base_rows_wp) = {
        let t = catalog
            .get(&up.base)
            .ok_or_else(|| err(format!("relation \"{}\" does not exist", up.base)))?;
        (
            t.schema.clone(),
            t.col_types.clone(),
            t.col_typmods.clone(),
            t.constraints.clone(),
            catalog.visible_rows_with_pos(&up.base).unwrap(),
        )
    };
    let base_positions: Vec<usize> = base_rows_wp.iter().map(|(p, _)| *p).collect();
    let mut base_rows: Vec<Row> = base_rows_wp.into_iter().map(|(_, r)| r).collect();
    let typmod_of = |slot: usize| {
        base_typmods
            .get(slot)
            .copied()
            .unwrap_or(types::typmod::NONE)
    };
    let (view_schema, base_idx) = view_binding(&up, &base_schema)?;
    let view_names = view_schema.names();

    let mut assigns: Vec<(usize, Scalar, Option<u32>, i32)> = Vec::new();
    loop {
        let col = c.ident()?;
        let vpos = view_names.iter().position(|n| n == &col).ok_or_else(|| {
            err(format!(
                "column \"{col}\" of relation \"{vname}\" does not exist"
            ))
        })?;
        let bidx = base_idx[vpos];
        c.expect(&Tok::Eq)?;
        let rhs = c.expr()?;
        let scalar = lower(&rhs, &view_schema, regs.clone())?;
        assigns.push((bidx, scalar, base_types.get(bidx).copied(), typmod_of(bidx)));
        if matches!(c.peek(), Tok::Comma) {
            c.pos += 1;
        } else {
            break;
        }
    }

    let dml_pred = if c.eat_kw("where") {
        let e = c.expr()?;
        Some(lower_pred(&e, &view_schema, regs.clone())?)
    } else {
        None
    };
    let returning = parse_returning(c)?;

    let view_pred = match &up.filter {
        Some(e) => Some(lower_pred(e, &base_schema, regs.clone())?),
        None => None,
    };

    let mut affected: Vec<Row> = Vec::new();
    let mut changes: Vec<(usize, Row)> = Vec::new();
    for (i, row) in base_rows.iter_mut().enumerate() {
        if let Some(p) = &view_pred {
            if !p(row).map_err(err)? {
                continue;
            }
        }
        let vrow: Row = base_idx.iter().map(|&bi| row[bi].clone()).collect();
        if let Some(p) = &dml_pred {
            if !p(&vrow).map_err(err)? {
                continue;
            }
        }

        let mut writes: Vec<(usize, SqlValue)> = Vec::with_capacity(assigns.len());
        for (bidx, scalar, oid, tm) in &assigns {
            let val = scalar(&vrow).map_err(err)?;
            writes.push((*bidx, coerce(val, *oid, *tm, regs)?));
        }
        for (bidx, val) in writes {
            row[bidx] = val;
        }

        enforce_check_option(&up, &base_schema, row, vname, regs)?;
        affected.push(row.clone());
        changes.push((base_positions[i], row.clone()));
    }

    validate_constraints(&up.base, &base_schema, &base_constraints, &base_rows, regs)?;
    validate_unique_indexes(catalog, &up.base, &base_rows)?;
    catalog
        .mvcc_update(&up.base, changes)?
        .ok_or_else(|| err(format!("relation \"{}\" does not exist", up.base)))?;
    match returning {
        Some(items) => view_returning(
            &items,
            &up,
            &base_schema,
            &base_types,
            &base_idx,
            &affected,
            regs,
        ),
        None => Ok(empty()),
    }
}

fn run_delete_view(c: &mut C, catalog: &mut Catalog, vname: &str) -> Result<QueryResult, PgError> {
    let regs = Arc::new(catalog.type_registries());
    let regs = &regs;
    let v = catalog.get_view(vname).expect("view exists").clone();
    if v.materialized {
        return Err(err(format!("cannot change materialized view \"{vname}\"")));
    }
    let up = view_updatable(&v, catalog).ok_or_else(|| not_updatable("delete from", vname))?;

    if c.eat_kw("as") {
        c.ident()?;
    } else if let Tok::Ident(a) = c.peek() {
        if a != "where" {
            c.pos += 1;
        }
    }

    let (base_schema, base_types, base_rows) = {
        let t = catalog
            .get(&up.base)
            .ok_or_else(|| err(format!("relation \"{}\" does not exist", up.base)))?;
        (
            t.schema.clone(),
            t.col_types.clone(),
            catalog.visible_rows_with_pos(&up.base).unwrap(),
        )
    };
    let (view_schema, base_idx) = view_binding(&up, &base_schema)?;

    let dml_pred = if c.eat_kw("where") {
        let e = c.expr()?;
        Some(lower_pred(&e, &view_schema, regs.clone())?)
    } else {
        None
    };
    let returning = parse_returning(c)?;

    let view_pred = match &up.filter {
        Some(e) => Some(lower_pred(e, &base_schema, regs.clone())?),
        None => None,
    };

    let mut del_positions: Vec<usize> = Vec::new();
    let mut deleted: Vec<Row> = Vec::new();
    for (pos, row) in base_rows {
        let visible = match &view_pred {
            Some(p) => p(&row).map_err(err)?,
            None => true,
        };
        if !visible {
            continue;
        }
        let vrow: Row = base_idx.iter().map(|&bi| row[bi].clone()).collect();
        let hit = match &dml_pred {
            Some(p) => p(&vrow).map_err(err)?,
            None => true,
        };
        if hit {
            del_positions.push(pos);
            deleted.push(row);
        }
    }
    catalog
        .mvcc_delete(&up.base, del_positions)?
        .ok_or_else(|| err(format!("relation \"{}\" does not exist", up.base)))?;
    match returning {
        Some(items) => view_returning(
            &items,
            &up,
            &base_schema,
            &base_types,
            &base_idx,
            &deleted,
            regs,
        ),
        None => Ok(empty()),
    }
}

fn read_type(c: &mut C, catalog: &Catalog) -> Result<(u32, i32), PgError> {

    let mut words: Vec<String> = Vec::new();
    let mut i = c.pos;
    while words.len() < 4 {
        match &c.toks[i] {
            Tok::Ident(s) => {
                words.push(s.clone());
                i += 1;
            }
            _ => break,
        }
    }
    if words.is_empty() {
        return Err(err(c.src.to_string()));
    }

    if let Some(def) = catalog.enum_by_name(&words[0]) {
        c.pos += 1;
        return Ok((def.oid, types::typmod::NONE));
    }

    if let Some(def) = catalog.composite_by_name(&words[0]) {
        c.pos += 1;
        return Ok((def.oid, types::typmod::NONE));
    }

    for take in (1..=words.len()).rev() {
        let candidate = words[..take].join(" ");
        if let Some(base_oid) = types::oid_for_type_name(&candidate) {
            c.pos += take;

            let typmod = if matches!(c.peek(), Tok::LParen) {
                read_typmod(c, base_oid)?
            } else {
                types::typmod::NONE
            };

            if matches!(c.peek(), Tok::LBracket) {
                c.pos += 1;
                if !matches!(c.peek(), Tok::RBracket) {
                    return Err(err(c.src.to_string()));
                }
                c.pos += 1;
                let arr = format!("{candidate}[]");
                let arr_oid = types::oid_for_type_name(&arr)
                    .ok_or_else(|| err(format!("type \"{arr}\" does not exist")))?;
                return Ok((arr_oid, types::typmod::NONE));
            }
            return Ok((base_oid, typmod));
        }
    }
    Err(err(format!("type \"{}\" does not exist", words[0])))
}

fn read_typmod(c: &mut C, base_oid: u32) -> Result<i32, PgError> {
    c.expect(&Tok::LParen)?;
    let mut nums: Vec<i64> = Vec::new();
    loop {
        match c.peek() {
            Tok::Int(n) => {
                nums.push(*n);
                c.pos += 1;
            }
            _ => return Err(err(c.src.to_string())),
        }
        if matches!(c.peek(), Tok::Comma) {
            c.pos += 1;
        } else {
            break;
        }
    }
    c.expect(&Tok::RParen)?;

    use crate::types::oid;
    let tm = match base_oid {
        oid::NUMERIC => {
            let p = *nums.first().ok_or_else(|| err(c.src.to_string()))? as i32;
            let s = nums.get(1).copied().unwrap_or(0) as i32;
            types::typmod::make_numeric(p, s)
        }
        oid::VARCHAR | oid::BPCHAR => {
            let n = *nums.first().ok_or_else(|| err(c.src.to_string()))? as i32;
            types::typmod::make_len(n)
        }
        _ => types::typmod::NONE,
    };
    Ok(tm)
}

fn run_insert(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let regs = Arc::new(catalog.type_registries());
    let regs = &regs;
    c.expect_kw("insert")?;
    c.expect_kw("into")?;
    let name = c.ident()?;

    if catalog.get(&name).is_none() && catalog.get_view(&name).is_some() {
        return run_insert_view(c, catalog, &name);
    }

    let (schema, col_types, col_typmods, constraints, defaults, generated, col_domains, identity) = {
        let t = catalog
            .get(&name)
            .ok_or_else(|| err(format!("relation \"{name}\" does not exist")))?;
        (
            t.schema.clone(),
            t.col_types.clone(),
            t.col_typmods.clone(),
            t.constraints.clone(),
            t.defaults.clone(),
            t.generated.clone(),
            t.col_domains.clone(),
            t.identity.clone(),
        )
    };
    let is_generated = |slot: usize| matches!(generated.get(slot), Some(Some(_)));
    let identity_of = |slot: usize| {
        identity
            .get(slot)
            .copied()
            .unwrap_or(crate::catalog::IdentityKind::None)
    };
    let width = schema.width();
    let typmod_of = |slot: usize| {
        col_typmods
            .get(slot)
            .copied()
            .unwrap_or(types::typmod::NONE)
    };

    let target: Vec<usize> = if matches!(c.peek(), Tok::LParen) {
        c.pos += 1;
        let mut idxs = Vec::new();
        loop {
            let col = c.ident()?;
            let idx = schema.index_of(&col).ok_or_else(|| {
                err(format!(
                    "column \"{col}\" of relation \"{name}\" does not exist"
                ))
            })?;
            idxs.push(idx);
            if matches!(c.peek(), Tok::Comma) {
                c.pos += 1;
            } else {
                break;
            }
        }
        c.expect(&Tok::RParen)?;
        idxs
    } else {
        Vec::new()
    };

    let overriding = if c.eat_kw("overriding") {
        let o = if c.eat_kw("system") {
            Overriding::System
        } else if c.eat_kw("user") {
            Overriding::User
        } else {
            return Err(err(c.src.to_string()));
        };
        c.expect_kw("value")?;
        o
    } else {
        Overriding::None
    };

    let tuples: Vec<(Vec<SqlValue>, Vec<bool>)> = if c.eat_kw("default") {

        c.expect_kw("values")?;
        vec![(Vec::new(), Vec::new())]
    } else if c.eat_kw("values") {
        let mut ts: Vec<(Vec<SqlValue>, Vec<bool>)> = Vec::new();
        loop {
            c.expect(&Tok::LParen)?;

            let mut vals: Vec<SqlValue> = Vec::new();
            let mut is_default: Vec<bool> = Vec::new();
            loop {
                if c.at_kw("default") {
                    c.pos += 1;
                    vals.push(SqlValue::Null);
                    is_default.push(true);
                } else {
                    let e = c.expr()?;
                    vals.push(eval_ctx(&e, EvalCtx::new(regs))?);
                    is_default.push(false);
                }
                if matches!(c.peek(), Tok::Comma) {
                    c.pos += 1;
                } else {
                    break;
                }
            }
            c.expect(&Tok::RParen)?;
            ts.push((vals, is_default));
            if matches!(c.peek(), Tok::Comma) {
                c.pos += 1;
            } else {
                break;
            }
        }
        ts
    } else {

        let ret_pos = {
            let mut depth = 0i32;
            let mut found = None;
            for (i, t) in c.toks.iter().enumerate().skip(c.pos) {
                match t {
                    Tok::LParen => depth += 1,
                    Tok::RParen => depth -= 1,
                    Tok::Ident(s) if depth == 0 && s == "returning" => {
                        found = Some(i);
                        break;
                    }
                    _ => {}
                }
            }
            found
        };

        let eof_at = c.toks[c.pos..]
            .iter()
            .position(|t| matches!(t, Tok::Eof))
            .map(|o| c.pos + o)
            .unwrap_or(c.toks.len());
        let src_end = ret_pos.unwrap_or(eof_at);
        let mut src_toks: Vec<Tok> = c.toks[c.pos..src_end].to_vec();
        src_toks.push(Tok::Eof);
        let (query, _) = crate::stmt::parser::parse_select_at(&src_toks, 0, c.src)?;
        c.pos = src_end;
        let result = run_select(&query, catalog)?;
        result
            .rows
            .into_iter()
            .map(|r| {
                let n = r.len();
                (r, vec![false; n])
            })
            .collect()
    };

    let mut new_rows: Vec<Row> = Vec::new();
    for (vals, is_default) in tuples {

        let indices: Vec<usize> = if target.is_empty() {
            if vals.len() > width {
                return Err(err(format!(
                    "INSERT has more expressions than target columns in relation \"{name}\""
                )));
            }
            (0..vals.len()).collect()
        } else {
            if vals.len() != target.len() {
                return Err(err(format!(
                    "INSERT has more expressions than target columns in relation \"{name}\""
                )));
            }
            target.clone()
        };

        let mut supplied = vec![false; width];

        if overriding != Overriding::System {
            for &slot in &indices {
                if identity_of(slot) == crate::catalog::IdentityKind::Always {
                    return Err(PgError::CannotInsertIntoIdentity {
                        column: schema.names()[slot].clone(),
                    });
                }
            }
        }

        let mut row: Row = vec![SqlValue::Null; width];
        for ((slot, val), dflt) in indices.iter().copied().zip(vals).zip(&is_default) {
            if *dflt {
                continue;
            }

            if is_generated(slot) {
                return Err(PgError::GeneratedInsert {
                    col: schema.names().get(slot).cloned().unwrap_or_default(),
                });
            }
            supplied[slot] = true;
            let oid = col_types.get(slot).copied();
            row[slot] = coerce(val, oid, typmod_of(slot), regs)?;
        }

        for slot in 0..width {
            let force_default = overriding == Overriding::User
                && identity_of(slot) != crate::catalog::IdentityKind::None;
            if is_generated(slot) || (supplied[slot] && !force_default) {
                continue;
            }
            if let Some(Some(dexpr)) = defaults.get(slot) {
                let v = crate::stmt::seq::eval_default(dexpr, catalog)?;
                let oid = col_types.get(slot).copied();
                row[slot] = coerce(v, oid, typmod_of(slot), regs)?;
            } else if let Some(Some(dname)) = col_domains.get(slot) {

                if let Some(dexpr) = catalog.get_domain(dname).and_then(|d| d.default.clone()) {
                    let v = eval_ctx(&dexpr, EvalCtx::new(regs))?;
                    let oid = col_types.get(slot).copied();
                    row[slot] = coerce(v, oid, typmod_of(slot), regs)?;
                }
            }
        }

        apply_generated(
            &mut row,
            &schema,
            &generated,
            &col_types,
            &col_typmods,
            regs,
        )?;
        new_rows.push(row);
    }

    if catalog.has_triggers_for(&name) {
        let mut kept: Vec<Row> = Vec::with_capacity(new_rows.len());
        for row in new_rows {
            if let Some(nr) = fire_row_triggers(
                catalog,
                &name,
                &schema,
                &col_types,
                &col_typmods,
                TrigTiming::Before,
                TrigEvent::Insert,
                Some(row),
                None,
            )? {
                kept.push(nr);
            }
        }
        new_rows = kept;
    }

    enforce_domain_columns(catalog, &col_domains, &new_rows)?;

    let affected: Vec<Row> = if catalog.partition_info(&name).is_some() {

        route_partitioned_insert(catalog, &name, &schema, new_rows, regs)?
    } else if c.eat_kw("on") {
        run_on_conflict(
            c,
            catalog,
            &name,
            &schema,
            &col_types,
            &col_typmods,
            &constraints,
            new_rows,
        )?
    } else {

        let existing = catalog
            .visible_rows(&name)
            .ok_or_else(|| err(format!("relation \"{name}\" does not exist")))?;
        let mut all = existing;
        all.extend(new_rows.iter().cloned());
        validate_constraints(&name, &schema, &constraints, &all, regs)?;
        validate_unique_indexes(catalog, &name, &all)?;

        validate_foreign_keys(catalog, &name, &constraints, &new_rows)?;
        catalog
            .mvcc_insert(&name, new_rows.clone())
            .ok_or_else(|| err(format!("relation \"{name}\" does not exist")))?;
        new_rows
    };

    if catalog.has_triggers_for(&name) {
        for row in &affected {
            fire_row_triggers(
                catalog,
                &name,
                &schema,
                &col_types,
                &col_typmods,
                TrigTiming::After,
                TrigEvent::Insert,
                Some(row.clone()),
                None,
            )?;
        }
    }

    match parse_returning(c)? {
        Some(items) => project_returning(&items, &schema, &col_types, &affected, regs),
        None => Ok(empty()),
    }
}

fn run_on_conflict(
    c: &mut C,
    catalog: &mut Catalog,
    name: &str,
    schema: &Schema,
    col_types: &[u32],
    col_typmods: &[i32],
    constraints: &TableConstraints,
    new_rows: Vec<Row>,
) -> Result<Vec<Row>, PgError> {
    let regs = Arc::new(catalog.type_registries());
    let regs = &regs;
    c.expect_kw("conflict")?;
    let typmod_of = |slot: usize| {
        col_typmods
            .get(slot)
            .copied()
            .unwrap_or(types::typmod::NONE)
    };

    let unique_indexes: Vec<(String, Vec<usize>)> = catalog
        .unique_indexes_for(name)
        .map(|idx| (idx.name.clone(), idx.cols.clone()))
        .collect();

    let arbiters: Vec<Vec<usize>> = if c.at_kw("on") {
        c.pos += 1;
        c.expect_kw("constraint")?;
        let cname = c.ident()?;

        let key_cols = constraints
            .uniques
            .iter()
            .find(|k| k.name.as_deref() == Some(&cname))
            .map(|k| k.cols.clone())
            .or_else(|| {
                unique_indexes
                    .iter()
                    .find(|(iname, _)| iname == &cname)
                    .map(|(_, cols)| cols.clone())
            })
            .ok_or_else(|| {
                err(format!(
                    "constraint \"{cname}\" for table \"{name}\" does not exist"
                ))
            })?;
        vec![key_cols]
    } else if matches!(c.peek(), Tok::LParen) {

        c.pos += 1;
        let mut targets: Vec<usize> = Vec::new();
        loop {
            let col = c.ident()?;
            let idx = schema.index_of(&col).ok_or_else(|| {
                err(format!(
                    "column \"{col}\" of relation \"{name}\" does not exist"
                ))
            })?;
            targets.push(idx);
            if matches!(c.peek(), Tok::Comma) {
                c.pos += 1;
            } else {
                break;
            }
        }
        c.expect(&Tok::RParen)?;

        if !constraints.uniques.is_empty() || !unique_indexes.is_empty() {
            let same_set = |cols: &[usize]| {
                cols.len() == targets.len()
                    && cols.iter().all(|ci| targets.contains(ci))
                    && targets.iter().all(|ti| cols.contains(ti))
            };
            let matches_key = constraints.uniques.iter().any(|k| same_set(&k.cols))
                || unique_indexes.iter().any(|(_, cols)| same_set(cols));
            if !matches_key {
                return Err(err(
                    "there is no unique or exclusion constraint matching the ON CONFLICT specification"
                        .to_string(),
                ));
            }
        }
        vec![targets]
    } else {

        if constraints.uniques.is_empty() && unique_indexes.is_empty() {
            return Err(err(
                "there is no unique or exclusion constraint matching the ON CONFLICT specification"
                    .to_string(),
            ));
        }
        constraints
            .uniques
            .iter()
            .map(|k| k.cols.clone())
            .chain(unique_indexes.iter().map(|(_, cols)| cols.clone()))
            .collect()
    };
    c.expect_kw("do")?;

    let combined = schema
        .clone()
        .qualified(name)
        .concat(schema.clone().qualified("excluded"));

    let do_nothing = c.eat_kw("nothing");
    let (assigns, pred): (Vec<(usize, Scalar, Option<u32>, i32)>, Option<Pred>) = if do_nothing {
        (Vec::new(), None)
    } else {
        c.expect_kw("update")?;
        c.expect_kw("set")?;
        let mut assigns = Vec::new();
        loop {
            let col = c.ident()?;
            let idx = schema.index_of(&col).ok_or_else(|| {
                err(format!(
                    "column \"{col}\" of relation \"{name}\" does not exist"
                ))
            })?;
            c.expect(&Tok::Eq)?;
            let rhs = c.expr()?;
            let scalar = lower(&rhs, &combined, regs.clone())?;
            assigns.push((idx, scalar, col_types.get(idx).copied(), typmod_of(idx)));
            if matches!(c.peek(), Tok::Comma) {
                c.pos += 1;
            } else {
                break;
            }
        }
        let pred = if c.eat_kw("where") {
            let pred_expr = c.expr()?;
            Some(lower_pred(&pred_expr, &combined, regs.clone())?)
        } else {
            None
        };
        (assigns, pred)
    };

    let rows_with_pos = catalog
        .visible_rows_with_pos(name)
        .ok_or_else(|| err(format!("relation \"{name}\" does not exist")))?;

    let mut rows: Vec<Row> = Vec::with_capacity(rows_with_pos.len());
    let mut pos_of: Vec<Option<usize>> = Vec::with_capacity(rows_with_pos.len());
    for (p, r) in rows_with_pos {
        rows.push(r);
        pos_of.push(Some(p));
    }
    let mut dirty: Vec<bool> = vec![false; rows.len()];
    let mut affected: Vec<Row> = Vec::new();

    let mut pairs: Vec<(Row, Row)> = Vec::new();
    for proposed in new_rows {

        let hit = rows.iter().position(|r| {
            arbiters
                .iter()
                .any(|key| key.iter().all(|&ti| values_equal(&r[ti], &proposed[ti])))
        });
        match hit {
            None => {
                rows.push(proposed.clone());
                pos_of.push(None);
                dirty.push(false);
                affected.push(proposed);
            }
            Some(_) if do_nothing => {   }
            Some(i) => {

                let mut combined_row: Row = rows[i].clone();
                combined_row.extend(proposed.iter().cloned());
                let fire = match &pred {
                    Some(p) => p(&combined_row).map_err(err)?,
                    None => true,
                };
                if fire {
                    let old = rows[i].clone();
                    let mut writes: Vec<(usize, SqlValue)> = Vec::with_capacity(assigns.len());
                    for (idx, scalar, oid, tm) in &assigns {
                        let v = scalar(&combined_row).map_err(err)?;
                        writes.push((*idx, coerce(v, *oid, *tm, regs)?));
                    }
                    for (idx, v) in writes {
                        rows[i][idx] = v;
                    }
                    dirty[i] = true;

                    affected.push(rows[i].clone());
                    pairs.push((old, rows[i].clone()));
                }
            }
        }
    }

    validate_constraints(name, schema, constraints, &rows, regs)?;

    validate_unique_indexes(catalog, name, &rows)?;

    validate_foreign_keys(catalog, name, constraints, &affected)?;

    let mut changes: Vec<(usize, Row)> = Vec::new();
    let mut inserts: Vec<Row> = Vec::new();
    for (i, row) in rows.into_iter().enumerate() {
        match pos_of[i] {
            Some(p) if dirty[i] => changes.push((p, row)),
            Some(_) => {}
            None => inserts.push(row),
        }
    }
    catalog
        .mvcc_update(name, changes)?
        .ok_or_else(|| err(format!("relation \"{name}\" does not exist")))?;
    if !inserts.is_empty() {
        catalog.mvcc_insert(name, inserts);
    }

    if !pairs.is_empty() {
        enforce_parent_update(catalog, name, &pairs)?;
    }
    Ok(affected)
}

fn values_equal(a: &SqlValue, b: &SqlValue) -> bool {
    if matches!(a, SqlValue::Null) || matches!(b, SqlValue::Null) {
        return false;
    }
    match crate::types::numeric::value_cmp(a, b) {
        Some(ord) => ord == std::cmp::Ordering::Equal,
        None => a == b,
    }
}

fn validate_constraints(
    name: &str,
    schema: &Schema,
    cons: &TableConstraints,
    rows: &[Row],
    regs: &Arc<TypeRegistries>,
) -> Result<(), PgError> {
    if cons.not_null.iter().all(|n| !n) && cons.uniques.is_empty() && cons.checks.is_empty() {
        return Ok(());
    }
    let names = schema.names();

    for row in rows {
        for (i, &nn) in cons.not_null.iter().enumerate() {
            if nn && matches!(row.get(i), Some(SqlValue::Null) | None) {
                let col = names.get(i).cloned().unwrap_or_default();
                return Err(err(format!(
                    "null value in column \"{col}\" of relation \"{name}\" violates not-null constraint"
                )));
            }
        }
        for check in &cons.checks {
            let bound = resolve(&check.expr, schema)?;

            if matches!(eval_row(&bound, row, EvalCtx::new(regs))?, SqlValue::Int(0)) {
                return Err(err(format!(
                    "new row for relation \"{name}\" violates check constraint"
                )));
            }
        }
    }

    for key in &cons.uniques {
        for a in 0..rows.len() {

            if key
                .cols
                .iter()
                .any(|&ci| matches!(rows[a].get(ci), Some(SqlValue::Null) | None))
            {
                continue;
            }
            for b in (a + 1)..rows.len() {
                if key
                    .cols
                    .iter()
                    .any(|&ci| matches!(rows[b].get(ci), Some(SqlValue::Null) | None))
                {
                    continue;
                }
                let dup = key
                    .cols
                    .iter()
                    .all(|&ci| values_equal(&rows[a][ci], &rows[b][ci]));
                if dup {
                    return Err(err(
                        "duplicate key value violates unique constraint".to_string()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_unique_indexes(catalog: &Catalog, name: &str, rows: &[Row]) -> Result<(), PgError> {
    for idx in catalog.unique_indexes_for(name) {
        let cols = &idx.cols;
        for a in 0..rows.len() {
            if cols
                .iter()
                .any(|&ci| matches!(rows[a].get(ci), Some(SqlValue::Null) | None))
            {
                continue;
            }
            for b in (a + 1)..rows.len() {
                if cols
                    .iter()
                    .any(|&ci| matches!(rows[b].get(ci), Some(SqlValue::Null) | None))
                {
                    continue;
                }
                if cols
                    .iter()
                    .all(|&ci| values_equal(&rows[a][ci], &rows[b][ci]))
                {
                    return Err(err(format!(
                        "duplicate key value violates unique constraint \"{}\"",
                        idx.name
                    )));
                }
            }
        }
    }
    Ok(())
}

fn render_key_value(v: &SqlValue) -> String {
    match v {
        SqlValue::Null => String::new(),
        SqlValue::Int(i) => i.to_string(),
        SqlValue::Real(r) => r.to_string(),
        SqlValue::Text(s) => s.clone(),
        SqlValue::Blob(_) => "\\x".to_string(),
    }
}

fn fk_row_null(cols: &[usize], row: &Row) -> bool {
    cols.iter()
        .any(|&ci| matches!(row.get(ci), Some(SqlValue::Null) | None))
}

fn validate_foreign_keys(
    catalog: &Catalog,
    name: &str,
    cons: &TableConstraints,
    rows: &[Row],
) -> Result<(), PgError> {
    for fk in &cons.foreign_keys {

        if fk_is_deferred(catalog, fk) {
            continue;
        }

        let mut parent_rows: Vec<Row> = catalog
            .visible_rows(&fk.parent)
            .ok_or_else(|| err(format!("relation \"{}\" does not exist", fk.parent)))?;
        if fk.parent == name {
            parent_rows.extend(rows.iter().cloned());
        }
        for row in rows {
            if fk_row_null(&fk.cols, row) {
                continue;
            }
            let found = parent_rows.iter().any(|pr| {
                fk.cols
                    .iter()
                    .zip(&fk.parent_cols)
                    .all(|(&ci, &pi)| values_equal(&row[ci], &pr[pi]))
            });
            if !found {
                return Err(err(format!(
                    "insert or update on table \"{name}\" violates foreign key constraint"
                )));
            }
        }
    }
    Ok(())
}

fn fk_is_deferred(catalog: &Catalog, fk: &crate::catalog::ForeignKey) -> bool {
    if !fk.deferrable || !catalog.in_transaction() {
        return false;
    }
    match catalog.constraints_deferred_mode() {
        Some(mode) => mode,
        None => fk.initially_deferred,
    }
}

pub fn validate_deferred_foreign_keys(catalog: &Catalog) -> Result<(), PgError> {

    let targets: Vec<(String, TableConstraints)> = catalog
        .tables_iter()
        .filter(|(_, t)| t.constraints.foreign_keys.iter().any(|fk| fk.deferrable))
        .map(|(name, t)| (name.clone(), t.constraints.clone()))
        .collect();
    for (name, cons) in targets {
        let rows = catalog.visible_rows(&name).unwrap_or_default();
        for fk in &cons.foreign_keys {
            if !fk.deferrable {
                continue;
            }
            let mut parent_rows: Vec<Row> = catalog
                .visible_rows(&fk.parent)
                .ok_or_else(|| err(format!("relation \"{}\" does not exist", fk.parent)))?;
            if fk.parent == name {
                parent_rows.extend(rows.iter().cloned());
            }
            for row in &rows {
                if fk_row_null(&fk.cols, row) {
                    continue;
                }
                let found = parent_rows.iter().any(|pr| {
                    fk.cols
                        .iter()
                        .zip(&fk.parent_cols)
                        .all(|(&ci, &pi)| values_equal(&row[ci], &pr[pi]))
                });
                if !found {
                    return Err(err(format!(
                        "insert or update on table \"{name}\" violates foreign key constraint"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn referencing_children(catalog: &Catalog, parent: &str) -> Vec<(String, ForeignKey)> {
    let mut out = Vec::new();
    for (cname, table) in catalog.tables_iter() {
        for fk in &table.constraints.foreign_keys {
            if fk.parent == parent {
                out.push((cname.clone(), fk.clone()));
            }
        }
    }
    out
}

fn enforce_parent_delete(
    catalog: &mut Catalog,
    parent: &str,
    deleted: &[Row],
) -> Result<(), PgError> {
    let mut work: Vec<(String, Vec<Row>)> = vec![(parent.to_string(), deleted.to_vec())];
    while let Some((pname, drows)) = work.pop() {
        if drows.is_empty() {
            continue;
        }
        for (cname, fk) in referencing_children(catalog, &pname) {
            let crows = match catalog.visible_rows_with_pos(&cname) {
                Some(rows) => rows,
                None => continue,
            };

            let mut del_positions: Vec<usize> = Vec::new();
            let mut changes: Vec<(usize, Row)> = Vec::new();
            let mut cascaded: Vec<Row> = Vec::new();
            for (pos, crow) in crows {
                let referenced = !fk_row_null(&fk.cols, &crow)
                    && drows.iter().any(|pr| {
                        fk.cols
                            .iter()
                            .zip(&fk.parent_cols)
                            .all(|(&ci, &pi)| values_equal(&crow[ci], &pr[pi]))
                    });
                if !referenced {
                    continue;
                }
                match fk.on_delete {
                    RefAction::NoAction | RefAction::Restrict => {
                        return Err(err(format!(
                            "update or delete on table \"{pname}\" violates foreign key constraint on table \"{cname}\""
                        )));
                    }
                    RefAction::Cascade => {
                        del_positions.push(pos);
                        cascaded.push(crow);
                    }
                    RefAction::SetNull => {
                        let mut nc = crow;
                        for &ci in &fk.cols {
                            nc[ci] = SqlValue::Null;
                        }
                        changes.push((pos, nc));
                    }
                }
            }
            if !del_positions.is_empty() {
                catalog.mvcc_delete(&cname, del_positions)?;
            }
            if !changes.is_empty() {
                catalog.mvcc_update(&cname, changes)?;
            }
            if !cascaded.is_empty() {
                work.push((cname, cascaded));
            }
        }
    }
    Ok(())
}

fn enforce_parent_update(
    catalog: &mut Catalog,
    parent: &str,
    pairs: &[(Row, Row)],
) -> Result<(), PgError> {
    let mut work: Vec<(String, Vec<(Row, Row)>)> = vec![(parent.to_string(), pairs.to_vec())];
    while let Some((pname, prs)) = work.pop() {
        for (cname, fk) in referencing_children(catalog, &pname) {

            let relevant: Vec<(Vec<SqlValue>, Vec<SqlValue>)> = prs
                .iter()
                .filter_map(|(o, n)| {
                    let ov: Vec<SqlValue> =
                        fk.parent_cols.iter().map(|&pi| o[pi].clone()).collect();
                    let nv: Vec<SqlValue> =
                        fk.parent_cols.iter().map(|&pi| n[pi].clone()).collect();
                    if ov.iter().zip(&nv).all(|(a, b)| values_equal(a, b)) {
                        None
                    } else {
                        Some((ov, nv))
                    }
                })
                .collect();
            if relevant.is_empty() {
                continue;
            }
            let crows = match catalog.visible_rows_with_pos(&cname) {
                Some(rows) => rows,
                None => continue,
            };

            let mut changes: Vec<(usize, Row)> = Vec::new();
            let mut cascaded: Vec<(Row, Row)> = Vec::new();
            for (pos, crow) in crows {
                let matched = if fk_row_null(&fk.cols, &crow) {
                    None
                } else {
                    relevant.iter().find(|(ov, _)| {
                        fk.cols
                            .iter()
                            .zip(ov)
                            .all(|(&ci, o)| values_equal(&crow[ci], o))
                    })
                };
                match matched {
                    None => {}
                    Some((_, nv)) => match fk.on_update {
                        RefAction::NoAction | RefAction::Restrict => {
                            return Err(err(format!(
                                "update or delete on table \"{pname}\" violates foreign key constraint on table \"{cname}\""
                            )));
                        }
                        RefAction::Cascade => {
                            let old = crow.clone();
                            let mut nc = crow;
                            for (k, &ci) in fk.cols.iter().enumerate() {
                                nc[ci] = nv[k].clone();
                            }
                            cascaded.push((old, nc.clone()));
                            changes.push((pos, nc));
                        }
                        RefAction::SetNull => {
                            let mut nc = crow;
                            for &ci in &fk.cols {
                                nc[ci] = SqlValue::Null;
                            }
                            changes.push((pos, nc));
                        }
                    },
                }
            }
            if !changes.is_empty() {
                catalog.mvcc_update(&cname, changes)?;
            }
            if !cascaded.is_empty() {
                work.push((cname, cascaded));
            }
        }
    }
    Ok(())
}

pub(crate) fn coerce(
    v: SqlValue,
    oid: Option<u32>,
    typmod: i32,
    regs: &Arc<TypeRegistries>,
) -> Result<SqlValue, PgError> {
    let oid = match oid {
        Some(o) => o,
        None => return Ok(v),
    };

    if regs.is_enum(oid) {
        return match v {
            SqlValue::Null => Ok(SqlValue::Null),
            SqlValue::Text(s) => {
                if regs.ordinal(oid, &s).is_some() {
                    Ok(SqlValue::Text(s))
                } else {
                    Err(PgError::InvalidEnumInput {
                        enum_name: regs.enum_name(oid).unwrap_or_default().to_string(),
                        input: s,
                    })
                }
            }

            SqlValue::Int(n) => Err(PgError::InvalidEnumInput {
                enum_name: regs.enum_name(oid).unwrap_or_default().to_string(),
                input: n.to_string(),
            }),
            SqlValue::Real(f) => Err(PgError::InvalidEnumInput {
                enum_name: regs.enum_name(oid).unwrap_or_default().to_string(),
                input: format!("{f}"),
            }),
            SqlValue::Blob(_) => Err(PgError::InvalidEnumInput {
                enum_name: regs.enum_name(oid).unwrap_or_default().to_string(),
                input: String::new(),
            }),
        };
    }

    if let Some(info) = regs.composite(oid) {
        return match v {
            SqlValue::Null => Ok(SqlValue::Null),
            SqlValue::Text(s) => crate::expr::eval::coerce_composite(info, &s),
            other => Ok(other),
        };
    }
    let coerced = match v {
        SqlValue::Null => SqlValue::Null,
        SqlValue::Text(s) => types::input(oid, &s)?,
        SqlValue::Int(n) => types::input(oid, &n.to_string())?,
        SqlValue::Real(f) => types::input(oid, &format!("{f}"))?,

        other => other,
    };

    types::apply_typmod(oid, typmod, coerced)
}

fn apply_generated(
    row: &mut Row,
    schema: &Schema,
    generated: &[Option<Expr>],
    col_types: &[u32],
    col_typmods: &[i32],
    regs: &Arc<TypeRegistries>,
) -> Result<(), PgError> {
    for (slot, g) in generated.iter().enumerate() {
        if let Some(gexpr) = g {
            let scalar = lower(gexpr, schema, regs.clone())?;
            let v = scalar(row).map_err(err)?;
            let oid = col_types.get(slot).copied();
            let tm = col_typmods
                .get(slot)
                .copied()
                .unwrap_or(types::typmod::NONE);
            row[slot] = coerce(v, oid, tm, regs)?;
        }
    }
    Ok(())
}

fn run_delete(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let regs = Arc::new(catalog.type_registries());
    let regs = &regs;
    c.expect_kw("delete")?;
    c.expect_kw("from")?;
    let name = c.ident()?;

    if catalog.get(&name).is_none() && catalog.get_view(&name).is_some() {
        return run_delete_view(c, catalog, &name);
    }

    if c.eat_kw("as") {
        c.ident()?;
    } else if let Tok::Ident(a) = c.peek() {
        if a != "where" && a != "using" {
            c.pos += 1;
        }
    }

    let (schema, rows_with_pos, col_types, col_typmods) = {
        let t = catalog
            .get(&name)
            .ok_or_else(|| err(format!("relation \"{name}\" does not exist")))?;
        (
            t.schema.clone(),
            catalog.visible_rows_with_pos(&name).unwrap(),
            t.col_types.clone(),
            t.col_typmods.clone(),
        )
    };

    let has_using = c.eat_kw("using");
    let mut using_rows: Vec<Row> = Vec::new();
    let combined_schema: Schema = if has_using {
        let uname = c.ident()?;
        let uqual = if c.eat_kw("as") {
            c.ident()?
        } else if let Tok::Ident(a) = c.peek() {
            if a != "where" {
                c.ident()?
            } else {
                uname.clone()
            }
        } else {
            uname.clone()
        };
        let uschema = catalog
            .get(&uname)
            .ok_or_else(|| err(format!("relation \"{uname}\" does not exist")))?
            .schema
            .clone();
        using_rows = catalog.visible_rows(&uname).unwrap_or_default();
        schema
            .clone()
            .qualified(&name)
            .concat(uschema.qualified(&uqual))
    } else {
        schema.clone()
    };

    let where_raw: Option<Expr> = if c.eat_kw("where") {
        Some(c.expr()?)
    } else {
        None
    };
    let qschema = schema.clone().qualified(&name);
    let correlated = !has_using
        && where_raw.as_ref().is_some_and(|e| {
            crate::stmt::lower::expr_is_correlated(e, &qschema, catalog).unwrap_or(false)
        });
    let pred = if correlated {
        None
    } else {
        match &where_raw {
            Some(w) => {
                let r = crate::stmt::lower::resolve_sub(w, catalog)?;
                Some(lower_pred(&r, &combined_schema, regs.clone())?)
            }
            None => None,
        }
    };
    let mut matched: Vec<(usize, Row)> = Vec::new();
    for (pos, r) in rows_with_pos {
        let hit = if correlated {
            match &where_raw {
                Some(w) => {
                    let folded = crate::stmt::lower::fold_correlated(w, &qschema, &r, catalog)?;
                    lower_pred(&folded, &qschema, regs.clone())?(&r).map_err(err)?
                }
                None => true,
            }
        } else if has_using {
            let mut any = false;
            for u in &using_rows {
                let cand: Row = r.iter().cloned().chain(u.iter().cloned()).collect();
                let ok = match &pred {
                    Some(p) => p(&cand).map_err(err)?,
                    None => true,
                };
                if ok {
                    any = true;
                    break;
                }
            }
            any
        } else {
            match &pred {
                Some(p) => p(&r).map_err(err)?,
                None => true,
            }
        };
        if hit {
            matched.push((pos, r));
        }
    }

    let has_triggers = catalog.has_triggers_for(&name);
    let mut del_positions: Vec<usize> = Vec::new();
    let mut deleted: Vec<Row> = Vec::new();
    for (pos, r) in matched {
        if has_triggers {
            match fire_row_triggers(
                catalog,
                &name,
                &schema,
                &col_types,
                &col_typmods,
                TrigTiming::Before,
                TrigEvent::Delete,
                None,
                Some(r.clone()),
            )? {
                None => continue,
                Some(_) => {}
            }
        }
        del_positions.push(pos);
        deleted.push(r);
    }

    let returning = parse_returning(c)?;

    catalog
        .mvcc_delete(&name, del_positions)?
        .ok_or_else(|| err(format!("relation \"{name}\" does not exist")))?;

    enforce_parent_delete(catalog, &name, &deleted)?;

    if has_triggers {
        for r in &deleted {
            fire_row_triggers(
                catalog,
                &name,
                &schema,
                &col_types,
                &col_typmods,
                TrigTiming::After,
                TrigEvent::Delete,
                None,
                Some(r.clone()),
            )?;
        }
    }

    match returning {
        Some(items) => project_returning(&items, &schema, &col_types, &deleted, regs),
        None => Ok(empty()),
    }
}

fn run_update(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let regs = Arc::new(catalog.type_registries());
    let regs = &regs;
    c.expect_kw("update")?;
    let name = c.ident()?;

    if catalog.get(&name).is_none() && catalog.get_view(&name).is_some() {
        return run_update_view(c, catalog, &name);
    }

    if c.eat_kw("as") {
        c.ident()?;
    } else if let Tok::Ident(a) = c.peek() {
        if a != "set" {
            c.pos += 1;
        }
    }

    c.expect_kw("set")?;

    let (schema, rows_with_pos, col_types, col_typmods, constraints, generated, col_domains) = {
        let t = catalog
            .get(&name)
            .ok_or_else(|| err(format!("relation \"{name}\" does not exist")))?;
        (
            t.schema.clone(),
            catalog.visible_rows_with_pos(&name).unwrap(),
            t.col_types.clone(),
            t.col_typmods.clone(),
            t.constraints.clone(),
            t.generated.clone(),
            t.col_domains.clone(),
        )
    };

    let positions: Vec<usize> = rows_with_pos.iter().map(|(p, _)| *p).collect();
    let mut rows: Vec<Row> = rows_with_pos.into_iter().map(|(_, r)| r).collect();
    let typmod_of = |slot: usize| {
        col_typmods
            .get(slot)
            .copied()
            .unwrap_or(types::typmod::NONE)
    };
    let has_generated = generated.iter().any(|g| g.is_some());
    let has_triggers = catalog.has_triggers_for(&name);

    let mut assigns_raw: Vec<(usize, Expr, Option<u32>, i32)> = Vec::new();
    loop {
        let col = c.ident()?;
        let idx = schema.index_of(&col).ok_or_else(|| {
            err(format!(
                "column \"{col}\" of relation \"{name}\" does not exist"
            ))
        })?;
        c.expect(&Tok::Eq)?;

        if matches!(generated.get(idx), Some(Some(_))) {
            if c.at_kw("default") {
                c.pos += 1;
                if matches!(c.peek(), Tok::Comma) {
                    c.pos += 1;
                    continue;
                } else {
                    break;
                }
            }
            return Err(PgError::GeneratedUpdate { col });
        }

        assigns_raw.push((idx, c.expr()?, col_types.get(idx).copied(), typmod_of(idx)));
        if matches!(c.peek(), Tok::Comma) {
            c.pos += 1;
        } else {
            break;
        }
    }

    let has_from = c.eat_kw("from");
    let mut from_rows: Vec<Row> = Vec::new();
    let combined_schema: Schema = if has_from {
        let from_name = c.ident()?;
        let from_qual = if c.eat_kw("as") {
            c.ident()?
        } else if let Tok::Ident(a) = c.peek() {
            if a != "where" {
                c.ident()?
            } else {
                from_name.clone()
            }
        } else {
            from_name.clone()
        };
        let from_schema = catalog
            .get(&from_name)
            .ok_or_else(|| err(format!("relation \"{from_name}\" does not exist")))?
            .schema
            .clone();
        from_rows = catalog.visible_rows(&from_name).unwrap_or_default();
        schema
            .clone()
            .qualified(&name)
            .concat(from_schema.qualified(&from_qual))
    } else {
        schema.clone()
    };

    let where_raw: Option<Expr> = if c.eat_kw("where") {
        Some(c.expr()?)
    } else {
        None
    };

    let qschema = schema.clone().qualified(&name);
    let is_corr =
        |e: &Expr| crate::stmt::lower::expr_is_correlated(e, &qschema, catalog).unwrap_or(false);
    let correlated = !has_from
        && (assigns_raw.iter().any(|(_, e, _, _)| is_corr(e))
            || where_raw.as_ref().is_some_and(|e| is_corr(e)));

    let (assigns, pred): (Vec<(usize, Scalar, Option<u32>, i32)>, Option<Pred>) = if correlated {
        (Vec::new(), None)
    } else {
        let a = assigns_raw
            .iter()
            .map(|(idx, rhs, oid, tm)| {
                let r = crate::stmt::lower::resolve_sub(rhs, catalog)?;
                lower(&r, &combined_schema, regs.clone()).map(|s| (*idx, s, *oid, *tm))
            })
            .collect::<Result<_, PgError>>()?;
        let p = match &where_raw {
            Some(w) => {
                let r = crate::stmt::lower::resolve_sub(w, catalog)?;
                Some(lower_pred(&r, &combined_schema, regs.clone())?)
            }
            None => None,
        };
        (a, p)
    };

    let mut affected: Vec<Row> = Vec::new();

    let mut pairs: Vec<(Row, Row)> = Vec::new();

    let mut changes: Vec<(usize, Row)> = Vec::new();
    for (i, row) in rows.iter_mut().enumerate() {

        let (old, writes): (Row, Vec<(usize, SqlValue)>) = if correlated {

            let hit = match &where_raw {
                Some(w) => {
                    let folded = crate::stmt::lower::fold_correlated(w, &qschema, row, catalog)?;
                    lower_pred(&folded, &qschema, regs.clone())?(row).map_err(err)?
                }
                None => true,
            };
            if !hit {
                continue;
            }
            let mut w: Vec<(usize, SqlValue)> = Vec::with_capacity(assigns_raw.len());
            for (idx, rhs, oid, tm) in &assigns_raw {
                let folded = crate::stmt::lower::fold_correlated(rhs, &qschema, row, catalog)?;
                let v = lower(&folded, &qschema, regs.clone())?(row).map_err(err)?;
                w.push((*idx, coerce(v, *oid, *tm, regs)?));
            }
            (row.clone(), w)
        } else if has_from {
            let mut matched: Option<Row> = None;
            for frow in &from_rows {
                let cand: Row = row.iter().cloned().chain(frow.iter().cloned()).collect();
                let ok = match &pred {
                    Some(p) => p(&cand).map_err(err)?,
                    None => true,
                };
                if ok {
                    matched = Some(cand);
                    break;
                }
            }
            let Some(cand) = matched else { continue };
            let mut w: Vec<(usize, SqlValue)> = Vec::with_capacity(assigns.len());
            for (idx, scalar, oid, tm) in &assigns {
                w.push((*idx, coerce(scalar(&cand).map_err(err)?, *oid, *tm, regs)?));
            }
            (row.clone(), w)
        } else {
            let hit = match &pred {
                Some(p) => p(row).map_err(err)?,
                None => true,
            };
            if !hit {
                continue;
            }
            let mut w: Vec<(usize, SqlValue)> = Vec::with_capacity(assigns.len());
            for (idx, scalar, oid, tm) in &assigns {
                w.push((*idx, coerce(scalar(row).map_err(err)?, *oid, *tm, regs)?));
            }
            (row.clone(), w)
        };
        for (idx, v) in writes {
            row[idx] = v;
        }

        if has_generated {
            apply_generated(row, &schema, &generated, &col_types, &col_typmods, regs)?;
        }

        if has_triggers {
            match fire_row_triggers(
                catalog,
                &name,
                &schema,
                &col_types,
                &col_typmods,
                TrigTiming::Before,
                TrigEvent::Update,
                Some(row.clone()),
                Some(old.clone()),
            )? {
                Some(nr) => *row = nr,
                None => {
                    *row = old.clone();
                    continue;
                }
            }
        }
        affected.push(row.clone());
        pairs.push((old, row.clone()));
        changes.push((positions[i], row.clone()));
    }

    let returning = parse_returning(c)?;

    validate_constraints(&name, &schema, &constraints, &rows, regs)?;
    validate_unique_indexes(catalog, &name, &rows)?;

    enforce_domain_columns(catalog, &col_domains, &affected)?;

    validate_foreign_keys(catalog, &name, &constraints, &affected)?;

    catalog
        .mvcc_update(&name, changes)?
        .ok_or_else(|| err(format!("relation \"{name}\" does not exist")))?;

    enforce_parent_update(catalog, &name, &pairs)?;

    if has_triggers {
        for (old, new) in &pairs {
            fire_row_triggers(
                catalog,
                &name,
                &schema,
                &col_types,
                &col_typmods,
                TrigTiming::After,
                TrigEvent::Update,
                Some(new.clone()),
                Some(old.clone()),
            )?;
        }
    }

    match returning {
        Some(items) => project_returning(&items, &schema, &col_types, &affected, regs),
        None => Ok(empty()),
    }
}

enum MatchedAction {

    Update(Vec<(usize, Scalar, Option<u32>)>),

    Delete,
}

struct MatchedClause {
    cond: Option<Pred>,
    action: MatchedAction,
}

struct NotMatchedClause {
    cond: Option<Pred>,
    targets: Vec<usize>,
    vals: Vec<Scalar>,
}

fn run_merge(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let regs = Arc::new(catalog.type_registries());
    let regs = &regs;
    c.expect_kw("merge")?;
    c.expect_kw("into")?;
    let target_name = c.ident()?;

    let target_alias = if c.eat_kw("as") {
        c.ident()?
    } else if matches!(c.peek(), Tok::Ident(a) if a != "using") {
        c.ident()?
    } else {
        target_name.clone()
    };

    let (target_schema, target_rows_wp, target_types, target_typmods, constraints) = {
        let t = catalog
            .get(&target_name)
            .ok_or_else(|| err(format!("relation \"{target_name}\" does not exist")))?;
        (
            t.schema.clone(),
            catalog.visible_rows_with_pos(&target_name).unwrap(),
            t.col_types.clone(),
            t.col_typmods.clone(),
            t.constraints.clone(),
        )
    };
    let target_width = target_schema.width();
    let typmod_of = |slot: usize| {
        target_typmods
            .get(slot)
            .copied()
            .unwrap_or(crate::types::typmod::NONE)
    };

    c.expect_kw("using")?;
    let (source_schema, source_alias, source_rows) = parse_merge_source(c, catalog)?;

    c.expect_kw("on")?;

    let combined = target_schema
        .clone()
        .qualified(&target_alias)
        .concat(source_schema.clone().qualified(&source_alias));
    let on_expr = c.expr()?;
    let on_pred = lower_pred(&on_expr, &combined, regs.clone())?;

    let mut matched: Vec<MatchedClause> = Vec::new();
    let mut not_matched: Vec<NotMatchedClause> = Vec::new();
    while c.eat_kw("when") {
        let is_not_matched = c.eat_kw("not");
        c.expect_kw("matched")?;
        if is_not_matched && c.at_kw("by") {

            return Err(err(
                "MERGE ... WHEN NOT MATCHED BY SOURCE/TARGET is not supported".to_string(),
            ));
        }

        let cond = if c.eat_kw("and") {
            let cexpr = c.expr()?;
            Some(lower_pred(&cexpr, &combined, regs.clone())?)
        } else {
            None
        };
        c.expect_kw("then")?;

        if c.at_kw("do") {
            return Err(err("MERGE ... THEN DO NOTHING is not supported".to_string()));
        }

        if is_not_matched {

            c.expect_kw("insert")?;
            let targets: Vec<usize> = if matches!(c.peek(), Tok::LParen) {
                c.pos += 1;
                let mut idxs = Vec::new();
                loop {
                    let col = c.ident()?;
                    let idx = target_schema.index_of(&col).ok_or_else(|| {
                        err(format!(
                            "column \"{col}\" of relation \"{target_name}\" does not exist"
                        ))
                    })?;
                    idxs.push(idx);
                    if matches!(c.peek(), Tok::Comma) {
                        c.pos += 1;
                    } else {
                        break;
                    }
                }
                c.expect(&Tok::RParen)?;
                idxs
            } else {
                Vec::new()
            };
            c.expect_kw("values")?;
            c.expect(&Tok::LParen)?;
            let mut vals: Vec<Scalar> = Vec::new();
            loop {
                let e = c.expr()?;
                vals.push(lower(&e, &combined, regs.clone())?);
                if matches!(c.peek(), Tok::Comma) {
                    c.pos += 1;
                } else {
                    break;
                }
            }
            c.expect(&Tok::RParen)?;
            let targets = if targets.is_empty() {
                (0..vals.len()).collect()
            } else {
                if targets.len() != vals.len() {
                    return Err(err(
                        "MERGE INSERT has a different number of columns and values".to_string(),
                    ));
                }
                targets
            };
            not_matched.push(NotMatchedClause {
                cond,
                targets,
                vals,
            });
        } else if c.eat_kw("update") {
            c.expect_kw("set")?;
            let mut assigns: Vec<(usize, Scalar, Option<u32>)> = Vec::new();
            loop {
                let col = c.ident()?;
                let idx = target_schema.index_of(&col).ok_or_else(|| {
                    err(format!(
                        "column \"{col}\" of relation \"{target_name}\" does not exist"
                    ))
                })?;
                c.expect(&Tok::Eq)?;
                let rhs = c.expr()?;
                let scalar = lower(&rhs, &combined, regs.clone())?;
                assigns.push((idx, scalar, target_types.get(idx).copied()));
                if matches!(c.peek(), Tok::Comma) {
                    c.pos += 1;
                } else {
                    break;
                }
            }
            matched.push(MatchedClause {
                cond,
                action: MatchedAction::Update(assigns),
            });
        } else if c.eat_kw("delete") {
            matched.push(MatchedClause {
                cond,
                action: MatchedAction::Delete,
            });
        } else {
            return Err(err(c.src.to_string()));
        }
    }

    let positions: Vec<usize> = target_rows_wp.iter().map(|(p, _)| *p).collect();
    let mut work: Vec<Option<Row>> = target_rows_wp.into_iter().map(|(_, r)| Some(r)).collect();

    let mut dirty: Vec<bool> = vec![false; work.len()];
    let mut inserted: Vec<Row> = Vec::new();

    let mut updated: Vec<Row> = Vec::new();

    for srow in &source_rows {

        let mut hit: Option<usize> = None;
        for (i, slot) in work.iter().enumerate() {
            if let Some(trow) = slot {
                let mut combined_row: Row = trow.clone();
                combined_row.extend(srow.iter().cloned());
                if on_pred(&combined_row).map_err(err)? {
                    hit = Some(i);
                    break;
                }
            }
        }
        match hit {
            Some(i) => {
                let mut combined_row: Row = work[i].clone().unwrap();
                combined_row.extend(srow.iter().cloned());
                for clause in &matched {
                    let fire = match &clause.cond {
                        Some(p) => p(&combined_row).map_err(err)?,
                        None => true,
                    };
                    if !fire {
                        continue;
                    }
                    match &clause.action {
                        MatchedAction::Update(assigns) => {
                            let mut writes: Vec<(usize, SqlValue)> =
                                Vec::with_capacity(assigns.len());
                            for (idx, scalar, oid) in assigns {
                                let v = scalar(&combined_row).map_err(err)?;
                                writes.push((*idx, coerce(v, *oid, typmod_of(*idx), regs)?));
                            }
                            let row = work[i].as_mut().unwrap();
                            for (idx, v) in writes {
                                row[idx] = v;
                            }
                            dirty[i] = true;
                            updated.push(row.clone());
                        }
                        MatchedAction::Delete => {
                            work[i] = None;
                        }
                    }
                    break;
                }
            }
            None => {

                let mut combined_row: Row = vec![SqlValue::Null; target_width];
                combined_row.extend(srow.iter().cloned());
                for clause in &not_matched {
                    let fire = match &clause.cond {
                        Some(p) => p(&combined_row).map_err(err)?,
                        None => true,
                    };
                    if !fire {
                        continue;
                    }
                    let mut row: Row = vec![SqlValue::Null; target_width];
                    for (slot, scalar) in clause.targets.iter().zip(&clause.vals) {
                        let v = scalar(&combined_row).map_err(err)?;
                        row[*slot] =
                            coerce(v, target_types.get(*slot).copied(), typmod_of(*slot), regs)?;
                    }
                    inserted.push(row);
                    break;
                }
            }
        }
    }

    let mut del_positions: Vec<usize> = Vec::new();
    let mut changes: Vec<(usize, Row)> = Vec::new();
    for (i, slot) in work.iter().enumerate() {
        match slot {
            None => del_positions.push(positions[i]),
            Some(row) if dirty[i] => changes.push((positions[i], row.clone())),
            Some(_) => {}
        }
    }
    let final_rows: Vec<Row> = work.into_iter().flatten().chain(inserted.clone()).collect();

    validate_constraints(
        &target_name,
        &target_schema,
        &constraints,
        &final_rows,
        regs,
    )?;
    validate_unique_indexes(catalog, &target_name, &final_rows)?;
    let mut touched: Vec<Row> = updated;
    touched.extend(inserted.clone());
    validate_foreign_keys(catalog, &target_name, &constraints, &touched)?;

    catalog
        .mvcc_delete(&target_name, del_positions)?
        .ok_or_else(|| err(format!("relation \"{target_name}\" does not exist")))?;
    catalog.mvcc_update(&target_name, changes)?;
    if !inserted.is_empty() {
        catalog.mvcc_insert(&target_name, inserted);
    }
    Ok(empty())
}

fn parse_merge_source(
    c: &mut C,
    catalog: &mut Catalog,
) -> Result<(Schema, String, Vec<Row>), PgError> {
    let regs = Arc::new(catalog.type_registries());
    let regs = &regs;
    if matches!(c.peek(), Tok::LParen) {

        let is_values = matches!(&c.toks[c.pos + 1], Tok::Ident(s) if s == "values");
        if is_values {
            c.pos += 1;
            c.expect_kw("values")?;
            let mut rows: Vec<Row> = Vec::new();
            loop {
                c.expect(&Tok::LParen)?;
                let mut vals: Vec<SqlValue> = Vec::new();
                loop {
                    let e = c.expr()?;
                    vals.push(eval_ctx(&e, EvalCtx::new(regs))?);
                    if matches!(c.peek(), Tok::Comma) {
                        c.pos += 1;
                    } else {
                        break;
                    }
                }
                c.expect(&Tok::RParen)?;
                rows.push(vals);
                if matches!(c.peek(), Tok::Comma) {
                    c.pos += 1;
                } else {
                    break;
                }
            }
            c.expect(&Tok::RParen)?;

            c.eat_kw("as");
            let alias = c.ident()?;
            let width = rows.first().map(|r| r.len()).unwrap_or(0);
            let names: Vec<String> = if matches!(c.peek(), Tok::LParen) {
                c.pos += 1;
                let mut ns = Vec::new();
                loop {
                    ns.push(c.ident()?);
                    if matches!(c.peek(), Tok::Comma) {
                        c.pos += 1;
                    } else {
                        break;
                    }
                }
                c.expect(&Tok::RParen)?;
                ns
            } else {
                (0..width).map(|i| format!("column{}", i + 1)).collect()
            };
            let schema = Schema::new(names);
            Ok((schema, alias, rows))
        } else {

            c.pos += 1;
            let (query, next) = crate::stmt::parser::parse_query_at(c.toks, c.pos, c.src)?;
            c.pos = next;
            c.expect(&Tok::RParen)?;
            c.eat_kw("as");
            let alias = c.ident()?;
            let result = super::lower::run_select(&query, catalog)?;
            let schema = Schema::new(result.columns);
            Ok((schema, alias, result.rows))
        }
    } else {

        let name = c.ident()?;
        let alias = if c.eat_kw("as") {
            c.ident()?
        } else if matches!(c.peek(), Tok::Ident(a) if a != "on") {
            c.ident()?
        } else {
            name.clone()
        };
        let t = catalog
            .get(&name)
            .ok_or_else(|| err(format!("relation \"{name}\" does not exist")))?;
        Ok((
            t.schema.clone(),
            alias,
            catalog.visible_rows(&name).unwrap(),
        ))
    }
}

fn run_create_type(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("type")?;
    let name = c.ident()?;
    c.expect_kw("as")?;

    if matches!(c.peek(), Tok::LParen) {
        return run_create_composite(c, catalog, name);
    }
    c.expect_kw("enum")?;
    c.expect(&Tok::LParen)?;
    let mut labels: Vec<String> = Vec::new();
    if !matches!(c.peek(), Tok::RParen) {
        loop {
            let label = c.str_lit()?;
            if labels.iter().any(|l| l == &label) {
                return Err(err(format!("enum label \"{label}\" already exists")));
            }
            labels.push(label);
            if matches!(c.peek(), Tok::Comma) {
                c.pos += 1;
            } else {
                break;
            }
        }
    }
    c.expect(&Tok::RParen)?;
    catalog
        .create_enum(&name, labels)
        .map_err(|_| err(format!("type \"{name}\" already exists")))?;
    Ok(empty())
}

fn run_create_composite(
    c: &mut C,
    catalog: &mut Catalog,
    name: String,
) -> Result<QueryResult, PgError> {
    c.expect(&Tok::LParen)?;
    let mut fields: Vec<(String, u32, i32)> = Vec::new();
    if !matches!(c.peek(), Tok::RParen) {
        loop {
            let fname = c.ident()?;
            let (oid, typmod) = read_type(c, catalog)?;
            fields.push((fname, oid, typmod));
            if matches!(c.peek(), Tok::Comma) {
                c.pos += 1;
            } else {
                break;
            }
        }
    }
    c.expect(&Tok::RParen)?;
    catalog
        .create_composite(&name, fields)
        .map_err(|_| err(format!("type \"{name}\" already exists")))?;
    Ok(empty())
}

fn run_drop_type(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("type")?;
    let if_exists = if c.eat_kw("if") {
        c.expect_kw("exists")?;
        true
    } else {
        false
    };
    let name = c.ident()?;
    let existed = catalog.drop_enum(&name);
    if !existed && !if_exists {
        return Err(err(format!("type \"{name}\" does not exist")));
    }
    Ok(empty())
}

fn run_alter_type(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("type")?;
    let name = c.ident()?;
    if catalog.enum_by_name(&name).is_none() {
        return Err(err(format!("type \"{name}\" does not exist")));
    }
    c.expect_kw("add")?;
    c.expect_kw("value")?;

    let if_not_exists = c.eat_kw("if") && c.eat_kw("not") && c.eat_kw("exists");
    let label = c.str_lit()?;
    let position = if c.eat_kw("before") {
        Some((true, c.str_lit()?))
    } else if c.eat_kw("after") {
        Some((false, c.str_lit()?))
    } else {
        None
    };
    let already = catalog
        .enum_by_name(&name)
        .unwrap()
        .labels
        .iter()
        .any(|l| l == &label);
    if already {
        if if_not_exists {
            return Ok(empty());
        }
        return Err(err(format!("enum label \"{label}\" already exists")));
    }
    catalog
        .enum_add_value(&name, label, position)
        .map_err(|_| err(format!("could not add value to enum type \"{name}\"")))?;
    Ok(empty())
}

fn run_drop(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("drop")?;
    if c.at_kw("index") {
        return run_drop_index(c, catalog);
    }
    if c.at_kw("domain") {
        return run_drop_domain(c, catalog);
    }
    if c.at_kw("function") {
        return run_drop_function(c, catalog);
    }
    if c.at_kw("trigger") {
        return run_drop_trigger(c, catalog);
    }
    if c.eat_kw("sequence") {
        let if_exists = if c.eat_kw("if") {
            c.expect_kw("exists")?;
            true
        } else {
            false
        };
        let name = c.ident()?;
        let existed = catalog.drop_sequence(&name);
        if !existed && !if_exists {
            return Err(err(format!("sequence \"{name}\" does not exist")));
        }
        return Ok(empty());
    }
    if c.at_kw("type") {
        return run_drop_type(c, catalog);
    }

    let is_view = if c.eat_kw("materialized") {
        c.expect_kw("view")?;
        true
    } else if c.eat_kw("view") {
        true
    } else {
        c.expect_kw("table")?;
        false
    };
    let if_exists = if c.eat_kw("if") {
        c.expect_kw("exists")?;
        true
    } else {
        false
    };
    let name = c.ident()?;
    let existed = if is_view {
        catalog.drop_view(&name)
    } else {
        catalog.drop_table(&name)
    };
    if !existed && !if_exists {
        let kind = if is_view { "view" } else { "table" };
        return Err(err(format!("{kind} \"{name}\" does not exist")));
    }
    Ok(empty())
}

fn run_create_index(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    let unique = c.eat_kw("unique");
    c.expect_kw("index")?;
    let if_not_exists = if c.eat_kw("if") {
        c.expect_kw("not")?;
        c.expect_kw("exists")?;
        true
    } else {
        false
    };
    let name = c.ident()?;

    if catalog.get_index(&name).is_some() {
        if if_not_exists {
            return Ok(empty());
        }
        return Err(err(format!("relation \"{name}\" already exists")));
    }
    c.expect_kw("on")?;
    let table = c.ident()?;
    let t = catalog
        .get(&table)
        .ok_or_else(|| err(format!("relation \"{table}\" does not exist")))?;
    let names = t.schema.names();
    let colnames = parse_col_name_list(c)?;
    let mut cols = Vec::with_capacity(colnames.len());
    for cn in &colnames {
        let idx = names
            .iter()
            .position(|n| n == cn)
            .ok_or_else(|| err(format!("column \"{cn}\" does not exist")))?;
        cols.push(idx);
    }

    if unique {
        let rows = &t.rows;
        for a in 0..rows.len() {
            if cols
                .iter()
                .any(|&ci| matches!(rows[a].get(ci), Some(SqlValue::Null) | None))
            {
                continue;
            }
            for b in (a + 1)..rows.len() {
                if cols
                    .iter()
                    .any(|&ci| matches!(rows[b].get(ci), Some(SqlValue::Null) | None))
                {
                    continue;
                }
                if cols
                    .iter()
                    .all(|&ci| values_equal(&rows[a][ci], &rows[b][ci]))
                {
                    let key_cols = colnames.join(", ");
                    let key_vals = cols
                        .iter()
                        .map(|&ci| render_key_value(&rows[a][ci]))
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(err(format!(
                        "could not create unique index \"{name}\": \
                         Key ({key_cols})=({key_vals}) is duplicated."
                    )));
                }
            }
        }
    }
    catalog.add_index(crate::catalog::IndexDef {
        name,
        table,
        cols,
        unique,
    });
    Ok(empty())
}

fn run_drop_index(c: &mut C, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
    c.expect_kw("index")?;
    let if_exists = if c.eat_kw("if") {
        c.expect_kw("exists")?;
        true
    } else {
        false
    };
    let name = c.ident()?;
    if !catalog.drop_index(&name) && !if_exists {
        return Err(err(format!("index \"{name}\" does not exist")));
    }
    Ok(empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::lexer::lex;

    fn exec(sql: &str, catalog: &mut Catalog) -> Result<QueryResult, PgError> {
        let toks = lex(sql).expect("lex ok");
        run(&toks, sql, catalog)
    }

    fn seeded() -> Catalog {
        let mut c = Catalog::new();
        exec("CREATE TABLE t (id int, name text, age int)", &mut c).expect("create");
        exec(
            "INSERT INTO t (id, name, age) VALUES (1, 'alice', 30), (2, 'bob', 25), (3, 'carol', 40)",
            &mut c,
        )
        .expect("insert");
        c
    }

    #[test]
    fn create_and_insert_land_typed_rows() {
        let c = seeded();
        let t = c.get("t").expect("table exists");
        assert_eq!(t.schema.names(), &["id", "name", "age"]);
        assert_eq!(t.rows.len(), 3);
        assert_eq!(
            t.rows[0],
            vec![
                SqlValue::Int(1),
                SqlValue::Text("alice".into()),
                SqlValue::Int(30)
            ]
        );

        assert_eq!(t.rows[2][2], SqlValue::Int(40));
    }

    #[test]
    fn insert_without_column_list_maps_in_order() {
        let mut c = seeded();
        exec("INSERT INTO t VALUES (4, 'dave', 50)", &mut c).expect("insert");
        let t = c.get("t").unwrap();
        assert_eq!(t.rows.len(), 4);
        assert_eq!(t.rows[3][1], SqlValue::Text("dave".into()));
    }

    #[test]
    fn insert_partial_column_list_nulls_the_rest() {
        let mut c = seeded();
        exec("INSERT INTO t (id) VALUES (9)", &mut c).expect("insert");
        let t = c.get("t").unwrap();
        let r = t.rows.last().unwrap();
        assert_eq!(r[0], SqlValue::Int(9));
        assert_eq!(r[1], SqlValue::Null);
        assert_eq!(r[2], SqlValue::Null);
    }

    #[test]
    fn delete_where_drops_matching_rows() {
        let mut c = seeded();
        exec("DELETE FROM t WHERE age > 28", &mut c).expect("delete");
        let t = c.get("t").unwrap();

        assert_eq!(t.rows.len(), 1);
        assert_eq!(t.rows[0][1], SqlValue::Text("bob".into()));
    }

    #[test]
    fn delete_no_where_clears_all_rows() {
        let mut c = seeded();
        exec("DELETE FROM t", &mut c).expect("delete");
        assert_eq!(c.get("t").unwrap().rows.len(), 0);
    }

    #[test]
    fn drop_table_removes_it() {
        let mut c = seeded();
        exec("DROP TABLE t", &mut c).expect("drop");
        assert!(c.get("t").is_none());
    }

    #[test]
    fn drop_if_exists_is_a_noop_on_missing() {
        let mut c = Catalog::new();
        assert!(exec("DROP TABLE IF EXISTS nope", &mut c).is_ok());
        assert!(exec("DROP TABLE nope", &mut c).is_err());
    }

    #[test]
    fn unknown_type_errors() {
        let mut c = Catalog::new();
        assert!(exec("CREATE TABLE bad (x notatype)", &mut c).is_err());
    }

    #[test]
    fn insert_into_unknown_table_errors() {
        let mut c = Catalog::new();
        assert!(exec("INSERT INTO ghost VALUES (1)", &mut c).is_err());
    }

    #[test]
    fn constraints_and_typmods_are_skipped() {
        let mut c = Catalog::new();
        exec(
            "CREATE TABLE p (id int primary key, tag varchar(50) not null, note text default 'x')",
            &mut c,
        )
        .expect("create with constraints");
        let t = c.get("p").unwrap();
        assert_eq!(t.schema.names(), &["id", "tag", "note"]);
        assert_eq!(t.col_types.len(), 3);
    }

    #[test]
    fn typmod_enforced_on_insert_and_update() {
        let mut c = Catalog::new();
        exec(
            "CREATE TABLE m (n numeric(6,2), v varchar(10), c char(5))",
            &mut c,
        )
        .expect("create");
        exec(
            "INSERT INTO m (n, v, c) VALUES (12.345, 'hi', 'ab')",
            &mut c,
        )
        .expect("insert");
        let t = c.get("m").unwrap();
        assert_eq!(t.rows[0][0], SqlValue::Text("12.35".into()));
        assert_eq!(t.rows[0][1], SqlValue::Text("hi".into()));
        assert_eq!(t.rows[0][2], SqlValue::Text("ab   ".into()));

        assert!(exec("INSERT INTO m (n) VALUES (12345.6)", &mut c).is_err());

        assert!(exec("INSERT INTO m (v) VALUES ('way too long value')", &mut c).is_err());

        exec("UPDATE m SET n = 2.5 WHERE v = 'hi'", &mut c).expect("update");
        assert_eq!(
            c.get("m").unwrap().rows[0][0],
            SqlValue::Text("2.50".into())
        );
        assert!(exec("UPDATE m SET n = 99999 WHERE v = 'hi'", &mut c).is_err());
    }

    #[test]
    fn no_typmod_column_is_unconstrained() {
        let mut c = Catalog::new();
        exec("CREATE TABLE u (n numeric, v varchar, c char)", &mut c).expect("create");
        exec(
            "INSERT INTO u (n, v, c) VALUES (1.23456789, 'anything long', 'xyz')",
            &mut c,
        )
        .expect("insert");
        let t = c.get("u").unwrap();
        assert_eq!(t.rows[0][0], SqlValue::Text("1.23456789".into()));
        assert_eq!(t.rows[0][1], SqlValue::Text("anything long".into()));
        assert_eq!(t.rows[0][2], SqlValue::Text("xyz".into()));
    }

    #[test]
    fn update_where_rewrites_matching_rows() {
        let mut c = seeded();
        exec("UPDATE t SET age = 99 WHERE name = 'bob'", &mut c).expect("update");
        let t = c.get("t").unwrap();
        assert_eq!(t.rows[0][2], SqlValue::Int(30));
        assert_eq!(t.rows[1][2], SqlValue::Int(99));
        assert_eq!(t.rows[2][2], SqlValue::Int(40));
    }

    #[test]
    fn update_self_referential_increments_all_rows() {
        let mut c = seeded();
        exec("UPDATE t SET age = age + 1", &mut c).expect("update");
        let t = c.get("t").unwrap();
        assert_eq!(t.rows[0][2], SqlValue::Int(31));
        assert_eq!(t.rows[1][2], SqlValue::Int(26));
        assert_eq!(t.rows[2][2], SqlValue::Int(41));
    }

    #[test]
    fn update_multi_column_is_simultaneous() {
        let mut c = seeded();

        exec(
            "UPDATE t SET id = age, age = id WHERE name = 'alice'",
            &mut c,
        )
        .expect("update");
        let t = c.get("t").unwrap();
        assert_eq!(t.rows[0][0], SqlValue::Int(30));
        assert_eq!(t.rows[0][2], SqlValue::Int(1));
    }

    #[test]
    fn update_can_set_null() {
        let mut c = seeded();
        exec("UPDATE t SET name = NULL WHERE id = 2", &mut c).expect("update");
        assert_eq!(c.get("t").unwrap().rows[1][1], SqlValue::Null);
    }

    #[test]
    fn update_unknown_table_errors() {
        let mut c = seeded();
        assert!(exec("UPDATE ghost SET x = 1", &mut c).is_err());
    }

    #[test]
    fn update_unknown_column_errors() {
        let mut c = seeded();
        assert!(exec("UPDATE t SET nope = 1", &mut c).is_err());
    }

    #[test]
    fn create_view_stores_body_and_columns() {
        let mut c = seeded();
        exec(
            "CREATE VIEW adult AS SELECT id, name FROM t WHERE age >= 30",
            &mut c,
        )
        .expect("create view");
        assert!(c.get_view("adult").is_some());
        assert!(c.get("adult").is_none());
        exec("CREATE VIEW cnt(g, n) AS SELECT name, age FROM t", &mut c).expect("create view cols");
        assert_eq!(
            c.get_view("cnt").unwrap().columns.as_deref(),
            Some(&["g".to_string(), "n".to_string()][..])
        );
    }

    #[test]
    fn create_view_over_table_name_errors() {
        let mut c = seeded();
        assert!(exec("CREATE VIEW t AS SELECT 1", &mut c).is_err());
    }

    #[test]
    fn create_view_replace_requires_or_replace() {
        let mut c = seeded();
        exec("CREATE VIEW v AS SELECT id FROM t", &mut c).expect("create");
        assert!(exec("CREATE VIEW v AS SELECT name FROM t", &mut c).is_err());
        exec("CREATE OR REPLACE VIEW v AS SELECT name FROM t", &mut c).expect("or replace");
    }

    #[test]
    fn drop_view_removes_it_and_if_exists_is_noop() {
        let mut c = seeded();
        exec("CREATE VIEW v AS SELECT id FROM t", &mut c).expect("create");
        exec("DROP VIEW v", &mut c).expect("drop");
        assert!(c.get_view("v").is_none());
        assert!(exec("DROP VIEW v", &mut c).is_err());
        assert!(exec("DROP VIEW IF EXISTS v", &mut c).is_ok());
    }

    #[test]
    fn insert_returning_projects_new_row() {
        let mut c = seeded();
        let r = exec(
            "INSERT INTO t (id, name, age) VALUES (7, 'zoe', 22) RETURNING id, age",
            &mut c,
        )
        .expect("insert returning");
        assert_eq!(r.columns, vec!["id", "age"]);
        assert_eq!(r.rows, vec![vec![SqlValue::Int(7), SqlValue::Int(22)]]);

        assert_eq!(c.get("t").unwrap().rows.len(), 4);
    }

    #[test]
    fn insert_returning_star_and_expr() {
        let mut c = seeded();
        let r = exec("INSERT INTO t (id, age) VALUES (8, 5) RETURNING *", &mut c).expect("star");
        assert_eq!(r.columns, vec!["id", "name", "age"]);
        assert_eq!(
            r.rows,
            vec![vec![SqlValue::Int(8), SqlValue::Null, SqlValue::Int(5)]]
        );
        let r = exec(
            "INSERT INTO t (id, age) VALUES (9, 4) RETURNING age + 1 AS a1",
            &mut c,
        )
        .expect("expr");
        assert_eq!(r.columns, vec!["a1"]);
        assert_eq!(r.rows, vec![vec![SqlValue::Int(5)]]);
    }

    #[test]
    fn update_returning_new_values() {
        let mut c = seeded();
        let r = exec(
            "UPDATE t SET age = age + 1 WHERE id = 2 RETURNING id, age",
            &mut c,
        )
        .expect("update returning");

        assert_eq!(r.rows, vec![vec![SqlValue::Int(2), SqlValue::Int(26)]]);
    }

    #[test]
    fn delete_returning_deleted_rows() {
        let mut c = seeded();
        let r =
            exec("DELETE FROM t WHERE age > 28 RETURNING id", &mut c).expect("delete returning");

        assert_eq!(r.rows, vec![vec![SqlValue::Int(1)], vec![SqlValue::Int(3)]]);
        assert_eq!(c.get("t").unwrap().rows.len(), 1);
    }

    #[test]
    fn returning_zero_rows_is_empty_with_columns() {
        let mut c = seeded();
        let r = exec(
            "UPDATE t SET age = 0 WHERE id = 999 RETURNING id, age",
            &mut c,
        )
        .expect("zero-row returning");
        assert_eq!(r.columns, vec!["id", "age"]);
        assert!(r.rows.is_empty());
    }

    #[test]
    fn returning_unknown_column_errors() {
        let mut c = seeded();
        assert!(exec("INSERT INTO t (id) VALUES (1) RETURNING nope", &mut c).is_err());
    }

    #[test]
    fn no_returning_still_yields_empty_result() {
        let mut c = seeded();
        let r = exec("INSERT INTO t (id) VALUES (11)", &mut c).expect("insert");
        assert!(r.columns.is_empty() && r.rows.is_empty());
    }

    fn kv() -> Catalog {
        let mut c = Catalog::new();
        exec("CREATE TABLE t (id int, v int)", &mut c).expect("create");
        exec("INSERT INTO t (id, v) VALUES (1, 10), (2, 20)", &mut c).expect("insert");
        c
    }

    #[test]
    fn upsert_do_nothing_skips_conflict() {
        let mut c = kv();
        exec(
            "INSERT INTO t (id, v) VALUES (1, 999) ON CONFLICT (id) DO NOTHING",
            &mut c,
        )
        .expect("upsert");
        let t = c.get("t").unwrap();
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0], vec![SqlValue::Int(1), SqlValue::Int(10)]);
    }

    #[test]
    fn upsert_do_nothing_inserts_when_no_conflict() {
        let mut c = kv();
        exec(
            "INSERT INTO t (id, v) VALUES (3, 30) ON CONFLICT (id) DO NOTHING",
            &mut c,
        )
        .expect("upsert");
        assert_eq!(c.get("t").unwrap().rows.len(), 3);
    }

    #[test]
    fn upsert_do_update_uses_excluded() {
        let mut c = kv();
        exec(
            "INSERT INTO t (id, v) VALUES (1, 5) ON CONFLICT (id) DO UPDATE SET v = EXCLUDED.v",
            &mut c,
        )
        .expect("upsert");
        let t = c.get("t").unwrap();
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0][1], SqlValue::Int(5));
    }

    #[test]
    fn upsert_do_update_accumulates_with_both_sides() {
        let mut c = kv();
        exec(
            "INSERT INTO t (id, v) VALUES (1, 5) ON CONFLICT (id) DO UPDATE SET v = t.v + EXCLUDED.v",
            &mut c,
        )
        .expect("upsert");
        assert_eq!(c.get("t").unwrap().rows[0][1], SqlValue::Int(15));
    }

    #[test]
    fn upsert_do_update_where_gates() {
        let mut c = kv();

        exec(
            "INSERT INTO t (id, v) VALUES (2, 1) ON CONFLICT (id) DO UPDATE SET v = EXCLUDED.v WHERE t.v < 15",
            &mut c,
        )
        .expect("upsert");
        assert_eq!(c.get("t").unwrap().rows[1][1], SqlValue::Int(20));
    }

    #[test]
    fn upsert_multi_row_mixed() {
        let mut c = kv();
        exec(
            "INSERT INTO t (id, v) VALUES (1, 100), (3, 30) ON CONFLICT (id) DO UPDATE SET v = EXCLUDED.v",
            &mut c,
        )
        .expect("upsert");
        let t = c.get("t").unwrap();
        assert_eq!(t.rows.len(), 3);
        assert_eq!(t.rows[0][1], SqlValue::Int(100));
        assert_eq!(t.rows[2], vec![SqlValue::Int(3), SqlValue::Int(30)]);
    }

    #[test]
    fn upsert_no_target_errors() {
        let mut c = kv();
        assert!(exec(
            "INSERT INTO t (id, v) VALUES (1, 1) ON CONFLICT DO NOTHING",
            &mut c
        )
        .is_err());
    }

    #[test]
    fn not_null_rejected_on_insert_and_update() {
        let mut c = Catalog::new();
        exec(
            "CREATE TABLE p (id int primary key, name text not null)",
            &mut c,
        )
        .expect("create");
        exec("INSERT INTO p VALUES (1, 'a')", &mut c).expect("valid insert");
        assert!(exec("INSERT INTO p (id, name) VALUES (2, NULL)", &mut c).is_err());
        assert!(exec("UPDATE p SET name = NULL WHERE id = 1", &mut c).is_err());

        assert_eq!(c.get("p").unwrap().rows[0][1], SqlValue::Text("a".into()));
    }

    #[test]
    fn primary_key_implies_not_null_and_uniqueness() {
        let mut c = Catalog::new();
        exec("CREATE TABLE p (id int primary key, v int)", &mut c).expect("create");
        exec("INSERT INTO p VALUES (1, 10)", &mut c).expect("insert");

        assert!(exec("INSERT INTO p (v) VALUES (20)", &mut c).is_err());

        assert!(exec("INSERT INTO p VALUES (1, 99)", &mut c).is_err());
        assert_eq!(c.get("p").unwrap().rows.len(), 1);
    }

    #[test]
    fn check_passes_on_null_rejects_false() {
        let mut c = Catalog::new();
        exec(
            "CREATE TABLE p (id int primary key, age int check (age >= 0))",
            &mut c,
        )
        .expect("create");

        exec("INSERT INTO p (id) VALUES (1)", &mut c).expect("null passes check");
        assert!(exec("INSERT INTO p VALUES (2, -1)", &mut c).is_err());

        exec("INSERT INTO p VALUES (3, 5)", &mut c).expect("insert");
        assert!(exec("UPDATE p SET age = -5 WHERE id = 3", &mut c).is_err());
    }

    #[test]
    fn unique_allows_multiple_nulls_but_rejects_dup() {
        let mut c = Catalog::new();
        exec("CREATE TABLE u (id int primary key, e text unique)", &mut c).expect("create");
        exec("INSERT INTO u VALUES (1, NULL), (2, NULL)", &mut c).expect("nulls distinct");
        exec("INSERT INTO u VALUES (3, 'x')", &mut c).expect("insert");
        assert!(exec("INSERT INTO u VALUES (4, 'x')", &mut c).is_err());
        assert_eq!(c.get("u").unwrap().rows.len(), 3);
    }

    #[test]
    fn composite_primary_key_uniqueness() {
        let mut c = Catalog::new();
        exec(
            "CREATE TABLE e (s int, co int, primary key (s, co))",
            &mut c,
        )
        .expect("create");
        exec("INSERT INTO e VALUES (1, 100), (1, 200), (2, 100)", &mut c).expect("distinct pairs");
        assert!(exec("INSERT INTO e VALUES (1, 100)", &mut c).is_err());
        exec("INSERT INTO e VALUES (2, 200)", &mut c).expect("new pair ok");
        assert_eq!(c.get("e").unwrap().rows.len(), 4);
    }

    #[test]
    fn upsert_on_constraint_errors() {
        let mut c = kv();
        assert!(exec(
            "INSERT INTO t (id, v) VALUES (1, 1) ON CONFLICT ON CONSTRAINT t_pkey DO NOTHING",
            &mut c
        )
        .is_err());
    }

    fn merge_seed() -> Catalog {
        let mut c = Catalog::new();
        exec("CREATE TABLE tgt (id int, v int)", &mut c).expect("create tgt");
        exec("INSERT INTO tgt (id, v) VALUES (1, 10), (2, 20)", &mut c).expect("seed tgt");
        exec("CREATE TABLE src (id int, v int)", &mut c).expect("create src");
        exec("INSERT INTO src (id, v) VALUES (2, 5), (3, 30)", &mut c).expect("seed src");
        c
    }

    #[test]
    fn merge_upsert_updates_matched_and_inserts_unmatched() {
        let mut c = merge_seed();
        exec(
            "MERGE INTO tgt t USING src s ON t.id = s.id \
             WHEN MATCHED THEN UPDATE SET v = t.v + s.v \
             WHEN NOT MATCHED THEN INSERT (id, v) VALUES (s.id, s.v)",
            &mut c,
        )
        .expect("merge");
        let t = c.get("tgt").unwrap();
        assert_eq!(t.rows.len(), 3);
        assert_eq!(t.rows[0], vec![SqlValue::Int(1), SqlValue::Int(10)]);
        assert_eq!(t.rows[1], vec![SqlValue::Int(2), SqlValue::Int(25)]);
        assert_eq!(t.rows[2], vec![SqlValue::Int(3), SqlValue::Int(30)]);
    }

    #[test]
    fn merge_matched_delete() {
        let mut c = merge_seed();
        exec(
            "MERGE INTO tgt t USING src s ON t.id = s.id WHEN MATCHED THEN DELETE",
            &mut c,
        )
        .expect("merge delete");
        let t = c.get("tgt").unwrap();
        assert_eq!(t.rows.len(), 1);
        assert_eq!(t.rows[0][0], SqlValue::Int(1));
    }

    #[test]
    fn merge_values_source_and_per_clause_conditions() {
        let mut c = merge_seed();
        exec(
            "MERGE INTO tgt t USING (VALUES (1, 100), (2, -1)) AS s(id, d) ON t.id = s.id \
             WHEN MATCHED AND s.d < 0 THEN DELETE \
             WHEN MATCHED THEN UPDATE SET v = t.v + s.d",
            &mut c,
        )
        .expect("merge values");
        let t = c.get("tgt").unwrap();
        assert_eq!(t.rows.len(), 1);
        assert_eq!(t.rows[0], vec![SqlValue::Int(1), SqlValue::Int(110)]);
    }

    #[test]
    fn merge_condition_false_leaves_row_unchanged() {
        let mut c = merge_seed();
        exec(
            "MERGE INTO tgt t USING (VALUES (1)) AS s(id) ON t.id = s.id \
             WHEN MATCHED AND t.v > 1000 THEN UPDATE SET v = 0",
            &mut c,
        )
        .expect("merge noop");
        assert_eq!(c.get("tgt").unwrap().rows[0][1], SqlValue::Int(10));
    }

    #[test]
    fn merge_unknown_target_errors() {
        let mut c = merge_seed();
        assert!(exec(
            "MERGE INTO ghost t USING src s ON t.id = s.id WHEN MATCHED THEN DELETE",
            &mut c
        )
        .is_err());
    }

    #[test]
    fn merge_do_nothing_is_deferred() {
        let mut c = merge_seed();
        assert!(exec(
            "MERGE INTO tgt t USING src s ON t.id = s.id WHEN NOT MATCHED THEN DO NOTHING",
            &mut c
        )
        .is_err());
    }

    fn fk_seed() -> Catalog {
        let mut c = Catalog::new();
        exec(
            "CREATE TABLE parent (id int PRIMARY KEY, name text)",
            &mut c,
        )
        .unwrap();
        exec("INSERT INTO parent VALUES (1, 'a'), (2, 'b')", &mut c).unwrap();
        c
    }

    #[test]
    fn fk_child_insert_requires_existing_parent() {
        let mut c = fk_seed();
        exec(
            "CREATE TABLE child (id int, pid int REFERENCES parent(id))",
            &mut c,
        )
        .unwrap();
        exec("INSERT INTO child VALUES (10, 1)", &mut c).expect("valid ref");
        assert!(
            exec("INSERT INTO child VALUES (11, 99)", &mut c).is_err(),
            "dangling ref"
        );
    }

    #[test]
    fn fk_null_child_skips_check() {
        let mut c = fk_seed();
        exec(
            "CREATE TABLE child (id int, pid int REFERENCES parent(id))",
            &mut c,
        )
        .unwrap();
        exec("INSERT INTO child VALUES (10, NULL)", &mut c).expect("null fk allowed");
        assert_eq!(c.get("child").unwrap().rows.len(), 1);
    }

    #[test]
    fn fk_requires_unique_parent_column() {
        let mut c = Catalog::new();
        exec("CREATE TABLE p (id int, tag int)", &mut c).unwrap();

        assert!(exec("CREATE TABLE ch (x int REFERENCES p(tag))", &mut c).is_err());
    }

    #[test]
    fn fk_on_delete_restrict_rejects_parent_delete() {
        let mut c = fk_seed();
        exec(
            "CREATE TABLE child (id int, pid int REFERENCES parent(id) ON DELETE RESTRICT)",
            &mut c,
        )
        .unwrap();
        exec("INSERT INTO child VALUES (10, 1)", &mut c).unwrap();
        assert!(exec("DELETE FROM parent WHERE id = 1", &mut c).is_err());

        exec("DELETE FROM parent WHERE id = 2", &mut c).expect("unreferenced delete ok");
    }

    #[test]
    fn fk_on_delete_cascade_removes_children() {
        let mut c = fk_seed();
        exec(
            "CREATE TABLE child (id int, pid int REFERENCES parent(id) ON DELETE CASCADE)",
            &mut c,
        )
        .unwrap();
        exec("INSERT INTO child VALUES (10, 1), (11, 1), (12, 2)", &mut c).unwrap();
        exec("DELETE FROM parent WHERE id = 1", &mut c).expect("cascade delete");
        let rows = &c.get("child").unwrap().rows;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], SqlValue::Int(12));
    }

    #[test]
    fn fk_on_delete_set_null_nulls_children() {
        let mut c = fk_seed();
        exec(
            "CREATE TABLE child (id int, pid int REFERENCES parent(id) ON DELETE SET NULL)",
            &mut c,
        )
        .unwrap();
        exec("INSERT INTO child VALUES (10, 1)", &mut c).unwrap();
        exec("DELETE FROM parent WHERE id = 1", &mut c).expect("set null delete");
        assert_eq!(c.get("child").unwrap().rows[0][1], SqlValue::Null);
    }

    #[test]
    fn fk_on_update_cascade_rewrites_children() {
        let mut c = fk_seed();
        exec(
            "CREATE TABLE child (id int, pid int REFERENCES parent(id) ON UPDATE CASCADE)",
            &mut c,
        )
        .unwrap();
        exec("INSERT INTO child VALUES (10, 1)", &mut c).unwrap();
        exec("UPDATE parent SET id = 5 WHERE id = 1", &mut c).expect("cascade update");
        assert_eq!(c.get("child").unwrap().rows[0][1], SqlValue::Int(5));
    }

    #[test]
    fn fk_table_level_composite() {
        let mut c = Catalog::new();
        exec("CREATE TABLE p (a int, b int, PRIMARY KEY (a, b))", &mut c).unwrap();
        exec("INSERT INTO p VALUES (1, 2)", &mut c).unwrap();
        exec(
            "CREATE TABLE ch (x int, y int, FOREIGN KEY (x, y) REFERENCES p (a, b))",
            &mut c,
        )
        .unwrap();
        exec("INSERT INTO ch VALUES (1, 2)", &mut c).expect("valid composite ref");
        assert!(
            exec("INSERT INTO ch VALUES (1, 3)", &mut c).is_err(),
            "dangling composite"
        );
    }

    #[test]
    fn fk_update_child_into_dangling_rejected() {
        let mut c = fk_seed();
        exec(
            "CREATE TABLE child (id int, pid int REFERENCES parent(id))",
            &mut c,
        )
        .unwrap();
        exec("INSERT INTO child VALUES (10, 1)", &mut c).unwrap();
        assert!(exec("UPDATE child SET pid = 77 WHERE id = 10", &mut c).is_err());
        exec("UPDATE child SET pid = 2 WHERE id = 10", &mut c).expect("valid re-point");
    }

    #[test]
    fn fk_default_no_action_rejects_parent_delete() {
        let mut c = fk_seed();

        exec(
            "CREATE TABLE child (id int, pid int REFERENCES parent(id))",
            &mut c,
        )
        .unwrap();
        exec("INSERT INTO child VALUES (10, 1)", &mut c).unwrap();
        assert!(exec("DELETE FROM parent WHERE id = 1", &mut c).is_err());
    }
}

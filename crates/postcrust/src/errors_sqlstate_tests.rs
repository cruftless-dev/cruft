
#![cfg(test)]

use crate::catalog::Catalog;
use crate::stmt::run_mut;
use crate::types::PgError;
use sql_core::SqlValue;

fn state(sql: &str) -> &'static str {
    let mut c = Catalog::new();
    c.create(
        "t",
        ["id", "name"],
        vec![
            vec![SqlValue::Int(1), SqlValue::Text("alice".into())],
            vec![SqlValue::Int(2), SqlValue::Text("bob".into())],
        ],
    );
    run_mut(sql, &mut c)
        .err()
        .unwrap_or_else(|| panic!("expected an error from: {sql}"))
        .sqlstate()
}

#[test]
fn undefined_table_is_42p01() {
    assert_eq!(state("SELECT * FROM no_such_table"), "42P01");
}

#[test]
fn undefined_column_is_42703() {
    assert_eq!(state("SELECT nope FROM t"), "42703");
}

#[test]
fn division_by_zero_is_22012() {
    assert_eq!(state("SELECT 1 / 0"), "22012");
    assert_eq!(state("SELECT 5 % 0"), "22012");
}

#[test]
fn invalid_text_representation_is_22p02() {

    assert_eq!(state("SELECT 'abc'::integer"), "22P02");
    assert_eq!(state("SELECT 'xyz'::boolean"), "22P02");
}

#[test]
fn numeric_out_of_range_is_22003() {

    assert_eq!(state("SELECT '99999999999'::integer"), "22003");
}

#[test]
fn undefined_function_is_42883() {
    assert_eq!(state("SELECT no_such_fn(1)"), "42883");
}

#[test]
fn syntax_error_is_42601() {
    assert_eq!(state("SELECT * FROM t WHERE"), "42601");
    assert_eq!(state("SELECT * FROM t GROUP"), "42601");
    assert_eq!(state("INSERT INTO t"), "42601");
}

#[test]
fn duplicate_table_is_42p07() {

    let e = PgError::InvalidInputSyntax {
        typ: "query",
        input: "relation \"t\" already exists".to_string(),
    };
    assert_eq!(e.sqlstate(), "42P07");
}

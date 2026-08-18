
#![allow(dead_code)]

pub mod types;

pub mod expr;

pub mod catalog;

pub mod collation;

pub mod crizzle;

pub mod stmt;

#[cfg(test)]
mod errors_sqlstate_tests;

pub const POSTGRES_ORACLE_VERSION: &str = "PG 17 (REL_17_STABLE @ 0cb713b)";

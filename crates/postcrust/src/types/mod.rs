
use sql_core::SqlValue;

pub mod arrays;
pub mod bit;
pub mod boolean;
pub mod boxtype;
pub mod bytea;
pub mod circle;
pub mod composite;
pub mod date;
pub mod domains;
pub mod enums;
pub mod floats;
pub mod inet;
pub mod integers;
pub mod interval;
pub mod json;
pub mod jsonb;
pub mod line;
pub mod lseg;
pub mod macaddr;
pub mod money;
pub mod multiranges;
pub mod numeric;
pub mod oidtype;
pub mod path;
pub mod pglsn;
pub mod point;
pub mod polygon;
pub mod ranges;
pub mod registry;
pub mod text;
pub mod tid;
pub mod time;
pub mod timestamp;
pub mod timestamptz;
pub mod timetz;
pub mod tsquery;
pub mod tsvector;
pub mod uuid;

pub mod oid {
    pub const BOOL: u32 = 16;
    pub const INT8: u32 = 20;
    pub const INT2: u32 = 21;
    pub const INT4: u32 = 23;
    pub const TEXT: u32 = 25;
    pub const JSON: u32 = 114;
    pub const FLOAT4: u32 = 700;
    pub const FLOAT8: u32 = 701;
    pub const BPCHAR: u32 = 1042;
    pub const VARCHAR: u32 = 1043;
    pub const DATE: u32 = 1082;
    pub const TIME: u32 = 1083;
    pub const TIMESTAMP: u32 = 1114;
    pub const NUMERIC: u32 = 1700;
    pub const UUID: u32 = 2950;
    pub const MONEY: u32 = 790;
    pub const CIDR: u32 = 650;
    pub const MACADDR8: u32 = 774;
    pub const MACADDR: u32 = 829;
    pub const INET: u32 = 869;
    pub const BIT: u32 = 1560;
    pub const VARBIT: u32 = 1562;
    pub const INTERVAL: u32 = 1186;
    pub const TIMETZ: u32 = 1266;
    pub const BYTEA: u32 = 17;
    pub const TIMESTAMPTZ: u32 = 1184;
    pub const JSONB: u32 = 3802;

    pub const BOOL_ARRAY: u32 = 1000;
    pub const INT2_ARRAY: u32 = 1005;
    pub const INT4_ARRAY: u32 = 1007;
    pub const TEXT_ARRAY: u32 = 1009;
    pub const INT8_ARRAY: u32 = 1016;
    pub const FLOAT8_ARRAY: u32 = 1022;
    pub const NUMERIC_ARRAY: u32 = 1231;

    pub const INT4RANGE: u32 = 3904;
    pub const NUMRANGE: u32 = 3906;
    pub const TSRANGE: u32 = 3908;
    pub const DATERANGE: u32 = 3912;
    pub const INT8RANGE: u32 = 3926;

    pub const INT4MULTIRANGE: u32 = 4451;
    pub const NUMMULTIRANGE: u32 = 4532;
    pub const TSMULTIRANGE: u32 = 4533;
    pub const DATEMULTIRANGE: u32 = 4535;
    pub const INT8MULTIRANGE: u32 = 4536;

    pub const OID: u32 = 26;
    pub const TID: u32 = 27;
    pub const POINT: u32 = 600;
    pub const LSEG: u32 = 601;
    pub const PATH: u32 = 602;
    pub const BOX: u32 = 603;
    pub const POLYGON: u32 = 604;
    pub const LINE: u32 = 628;
    pub const CIRCLE: u32 = 718;
    pub const PG_LSN: u32 = 3220;

    pub const TSVECTOR: u32 = 3614;
    pub const TSQUERY: u32 = 3615;
}

pub fn oid_for_type_name(name: &str) -> Option<u32> {
    Some(match name {
        "int" | "int4" | "integer" => oid::INT4,
        "int8" | "bigint" => oid::INT8,
        "int2" | "smallint" => oid::INT2,
        "float4" | "real" => oid::FLOAT4,
        "float8" | "double precision" => oid::FLOAT8,
        "numeric" | "decimal" => oid::NUMERIC,
        "bool" | "boolean" => oid::BOOL,
        "text" => oid::TEXT,
        "varchar" | "character varying" => oid::VARCHAR,
        "char" | "bpchar" | "character" => oid::BPCHAR,
        "date" => oid::DATE,
        "time" => oid::TIME,
        "timetz" | "time with time zone" => oid::TIMETZ,
        "timestamp" => oid::TIMESTAMP,
        "timestamptz" | "timestamp with time zone" => oid::TIMESTAMPTZ,
        "interval" => oid::INTERVAL,
        "uuid" => oid::UUID,
        "json" => oid::JSON,
        "jsonb" => oid::JSONB,
        "bytea" => oid::BYTEA,
        "inet" => oid::INET,
        "cidr" => oid::CIDR,
        "macaddr" => oid::MACADDR,
        "macaddr8" => oid::MACADDR8,
        "bit" => oid::BIT,
        "varbit" | "bit varying" => oid::VARBIT,
        "money" => oid::MONEY,

        "int4[]" | "integer[]" | "int[]" => oid::INT4_ARRAY,
        "int8[]" | "bigint[]" => oid::INT8_ARRAY,
        "int2[]" | "smallint[]" => oid::INT2_ARRAY,
        "text[]" => oid::TEXT_ARRAY,
        "bool[]" | "boolean[]" => oid::BOOL_ARRAY,
        "float8[]" | "double precision[]" => oid::FLOAT8_ARRAY,
        "numeric[]" | "decimal[]" => oid::NUMERIC_ARRAY,

        "int4range" => oid::INT4RANGE,
        "int8range" => oid::INT8RANGE,
        "numrange" => oid::NUMRANGE,
        "tsrange" => oid::TSRANGE,
        "daterange" => oid::DATERANGE,

        "int4multirange" => oid::INT4MULTIRANGE,
        "int8multirange" => oid::INT8MULTIRANGE,
        "nummultirange" => oid::NUMMULTIRANGE,
        "tsmultirange" => oid::TSMULTIRANGE,
        "datemultirange" => oid::DATEMULTIRANGE,

        "oid" => oid::OID,
        "tid" => oid::TID,
        "pg_lsn" => oid::PG_LSN,
        "tsvector" => oid::TSVECTOR,
        "tsquery" => oid::TSQUERY,
        "point" => oid::POINT,
        "lseg" => oid::LSEG,
        "path" => oid::PATH,
        "box" => oid::BOX,
        "polygon" => oid::POLYGON,
        "line" => oid::LINE,
        "circle" => oid::CIRCLE,
        _ => return None,
    })
}

pub fn type_name(oid: u32) -> &'static str {
    match oid {
        oid::BOOL => "boolean",
        oid::INT8 => "bigint",
        oid::INT2 => "smallint",
        oid::INT4 => "integer",
        oid::TEXT => "text",
        oid::FLOAT4 => "real",
        oid::FLOAT8 => "double precision",
        oid::BPCHAR => "character",
        oid::VARCHAR => "character varying",
        oid::JSON => "json",
        oid::DATE => "date",
        oid::TIME => "time without time zone",
        oid::TIMESTAMP => "timestamp without time zone",
        oid::NUMERIC => "numeric",
        oid::UUID => "uuid",
        oid::MONEY => "money",
        oid::CIDR => "cidr",
        oid::MACADDR8 => "macaddr8",
        oid::MACADDR => "macaddr",
        oid::INET => "inet",
        oid::BIT => "bit",
        oid::VARBIT => "bit varying",
        oid::INTERVAL => "interval",
        oid::TIMETZ => "time with time zone",
        oid::BYTEA => "bytea",
        oid::TIMESTAMPTZ => "timestamp with time zone",
        oid::JSONB => "jsonb",
        oid::BOOL_ARRAY => "boolean[]",
        oid::INT2_ARRAY => "smallint[]",
        oid::INT4_ARRAY => "integer[]",
        oid::TEXT_ARRAY => "text[]",
        oid::INT8_ARRAY => "bigint[]",
        oid::FLOAT8_ARRAY => "double precision[]",
        oid::NUMERIC_ARRAY => "numeric[]",
        oid::INT4RANGE => "int4range",
        oid::NUMRANGE => "numrange",
        oid::TSRANGE => "tsrange",
        oid::DATERANGE => "daterange",
        oid::INT8RANGE => "int8range",
        oid::INT4MULTIRANGE => "int4multirange",
        oid::NUMMULTIRANGE => "nummultirange",
        oid::TSMULTIRANGE => "tsmultirange",
        oid::DATEMULTIRANGE => "datemultirange",
        oid::INT8MULTIRANGE => "int8multirange",
        oid::OID => "oid",
        oid::TID => "tid",
        oid::POINT => "point",
        oid::LSEG => "lseg",
        oid::PATH => "path",
        oid::BOX => "box",
        oid::POLYGON => "polygon",
        oid::LINE => "line",
        oid::CIRCLE => "circle",
        oid::PG_LSN => "pg_lsn",
        oid::TSVECTOR => "tsvector",
        oid::TSQUERY => "tsquery",
        _ => "unknown",
    }
}

pub fn is_array(oid: u32) -> bool {
    matches!(
        oid,
        oid::BOOL_ARRAY
            | oid::INT2_ARRAY
            | oid::INT4_ARRAY
            | oid::TEXT_ARRAY
            | oid::INT8_ARRAY
            | oid::FLOAT8_ARRAY
            | oid::NUMERIC_ARRAY
    )
}

#[derive(Debug, Clone, PartialEq)]
pub enum PgError {

    InvalidInputSyntax { typ: &'static str, input: String },

    InvalidEnumInput { enum_name: String, input: String },

    OutOfRange { typ: &'static str, input: String },

    Overflow { typ: &'static str },

    CannotCast {
        from: &'static str,
        to: &'static str,
    },

    DivisionByZero,

    TsquerySyntax { input: String },

    NumericFieldOverflow,

    ValueTooLong { typ: String },

    TransactionAborted,

    SerializationFailure,

    SetTransactionTooLate,

    LockNotAvailable { rel: String },

    GeneratedInsert { col: String },

    GeneratedUpdate { col: String },

    SequenceReachedBound { name: String, max: bool, bound: i64 },

    CurrvalNotYetDefined { name: String },

    CannotInsertIntoIdentity { column: String },

    RaiseException { message: String },

    CollationDoesNotExist { name: String },

    CollationUnsupported { name: String },
}

impl PgError {
    pub fn message(&self) -> String {
        match self {
            PgError::InvalidInputSyntax { typ, input } => {
                format!("invalid input syntax for type {typ}: \"{input}\"")
            }
            PgError::InvalidEnumInput { enum_name, input } => {
                format!("invalid input value for enum {enum_name}: \"{input}\"")
            }
            PgError::OutOfRange { typ, input } => {
                format!("value \"{input}\" is out of range for type {typ}")
            }
            PgError::Overflow { typ } => format!("{typ} out of range"),
            PgError::CannotCast { from, to } => format!("cannot cast type {from} to {to}"),
            PgError::DivisionByZero => "division by zero".to_string(),
            PgError::TsquerySyntax { input } => {
                format!("syntax error in tsquery: \"{input}\"")
            }
            PgError::NumericFieldOverflow => "numeric field overflow".to_string(),
            PgError::ValueTooLong { typ } => format!("value too long for type {typ}"),
            PgError::TransactionAborted => {
                "current transaction is aborted, commands ignored until end of transaction block"
                    .to_string()
            }
            PgError::SerializationFailure => {
                "could not serialize access due to concurrent update".to_string()
            }
            PgError::SetTransactionTooLate => {
                "SET TRANSACTION ISOLATION LEVEL must be called before any query".to_string()
            }
            PgError::LockNotAvailable { rel } => {
                format!("could not obtain lock on row in relation \"{rel}\"")
            }
            PgError::GeneratedInsert { col } => {
                format!("cannot insert a non-DEFAULT value into column \"{col}\"")
            }
            PgError::GeneratedUpdate { col } => {
                format!("column \"{col}\" can only be updated to DEFAULT")
            }
            PgError::SequenceReachedBound { name, max, bound } => {
                let which = if *max { "maximum" } else { "minimum" };
                format!("nextval: reached {which} value of sequence \"{name}\" ({bound})")
            }
            PgError::CurrvalNotYetDefined { name } => {
                format!("currval of sequence \"{name}\" is not yet defined in this session")
            }
            PgError::CannotInsertIntoIdentity { column } => {
                format!("cannot insert a non-DEFAULT value into column \"{column}\"")
            }
            PgError::RaiseException { message } => message.clone(),
            PgError::CollationDoesNotExist { name } => {
                format!("collation \"{name}\" for encoding \"UTF8\" does not exist")
            }
            PgError::CollationUnsupported { name } => format!(
                "collation \"{name}\" is not supported: postcrust reproduces only \
                 deterministic byte-ordered collations (C, POSIX, ucs_basic); \
                 locale/ICU collations require ICU"
            ),
        }
    }

    pub fn sqlstate(&self) -> &'static str {
        match self {

            PgError::InvalidEnumInput { .. } => "22P02",

            PgError::DivisionByZero => "22012",

            PgError::OutOfRange { .. } | PgError::Overflow { .. } => "22003",

            PgError::CannotCast { .. } => "42846",

            PgError::TsquerySyntax { .. } => "42601",

            PgError::NumericFieldOverflow => "22003",

            PgError::ValueTooLong { .. } => "22001",

            PgError::TransactionAborted => "25P02",

            PgError::SerializationFailure => "40001",

            PgError::SetTransactionTooLate => "25001",

            PgError::LockNotAvailable { .. } => "55P03",

            PgError::GeneratedInsert { .. } | PgError::GeneratedUpdate { .. } => "428C9",

            PgError::SequenceReachedBound { .. } => "2200H",

            PgError::CurrvalNotYetDefined { .. } => "55000",

            PgError::CannotInsertIntoIdentity { .. } => "428C9",

            PgError::RaiseException { .. } => "P0001",

            PgError::CollationDoesNotExist { .. } => "42704",

            PgError::CollationUnsupported { .. } => "0A000",
            PgError::InvalidInputSyntax { typ, input } => {

                if input.starts_with("cursor \"") && input.contains("already exists") {
                    "42P03"
                } else if input.starts_with("cursor \"") && input.contains("does not exist") {
                    "34000"
                } else if input.contains("cursor can only scan forward") {
                    "55000"
                } else if input.contains("prepared statement") && input.contains("already exists") {
                    "42P05"
                } else if input.contains("prepared statement") && input.contains("does not exist") {
                    "26000"
                } else if input.contains("does not exist") {
                    if input.contains("column") {
                        "42703"
                    } else if input.contains("function") || input.contains("operator") {
                        "42883"
                    } else if input.contains("type") {
                        "42704"
                    } else if input.contains("savepoint") {
                        "3B001"
                    } else if input.contains("index") {
                        "42704"
                    } else if input.contains("relation") || input.contains("sequence") {
                        "42P01"
                    } else {
                        "42601"
                    }
                } else if input.contains("already exists") {
                    "42P07"
                }

                else if input.contains("division by zero") {
                    "22012"
                } else if input.contains("out of range") {
                    "22003"
                } else if input.contains("invalid input syntax for type") {
                    "22P02"
                }

                else if input.contains("not-null constraint")
                    || input.contains("does not allow null values")
                {
                    "23502"
                } else if input.contains("foreign key constraint") {
                    "23503"
                } else if input.contains("unique constraint")
                    || input.contains("duplicate key")
                    || input.contains("could not create unique index")
                {
                    "23505"
                } else if input.contains("check constraint") {
                    "23514"
                } else if input.contains("is ambiguous") {
                    "42702"
                }

                else if *typ == "query" || *typ == "expression" {
                    "42601"
                } else {
                    "22P02"
                }
            }
        }
    }
}

pub mod typmod {

    pub const NONE: i32 = -1;
    const VARHDRSZ: i32 = 4;

    pub fn make_numeric(p: i32, s: i32) -> i32 {
        ((p << 16) | (s & 0x7ff)) + VARHDRSZ
    }

    pub fn make_len(n: i32) -> i32 {
        n + VARHDRSZ
    }

    pub fn char_len(tm: i32) -> Option<i32> {
        if tm == NONE {
            None
        } else {
            Some(tm - VARHDRSZ)
        }
    }

    pub fn numeric_precision(tm: i32) -> Option<i32> {
        if tm == NONE {
            None
        } else {
            Some(((tm - VARHDRSZ) >> 16) & 0xffff)
        }
    }

    pub fn numeric_scale(tm: i32) -> Option<i32> {
        if tm == NONE {
            None
        } else {
            Some((tm - VARHDRSZ) & 0x7ff)
        }
    }
}

pub fn udt_name(oid: u32) -> &'static str {
    match oid {
        oid::BOOL => "bool",
        oid::INT2 => "int2",
        oid::INT4 => "int4",
        oid::INT8 => "int8",
        oid::TEXT => "text",
        oid::FLOAT4 => "float4",
        oid::FLOAT8 => "float8",
        oid::BPCHAR => "bpchar",
        oid::VARCHAR => "varchar",
        oid::NUMERIC => "numeric",
        oid::DATE => "date",
        oid::TIME => "time",
        oid::TIMETZ => "timetz",
        oid::TIMESTAMP => "timestamp",
        oid::TIMESTAMPTZ => "timestamptz",
        oid::INTERVAL => "interval",
        oid::UUID => "uuid",
        oid::JSON => "json",
        oid::JSONB => "jsonb",
        oid::BYTEA => "bytea",
        _ => type_name(oid),
    }
}

pub fn apply_typmod(oid: u32, tm: i32, v: SqlValue) -> Result<SqlValue, PgError> {
    if tm < 0 || matches!(v, SqlValue::Null) {
        return Ok(v);
    }
    match oid {
        oid::NUMERIC => numeric::apply_typmod(tm, v),
        oid::VARCHAR | oid::BPCHAR => text::apply_typmod(oid, tm, v),
        _ => Ok(v),
    }
}

pub fn input(oid: u32, text: &str) -> Result<SqlValue, PgError> {
    match oid {
        oid::INT2 | oid::INT4 | oid::INT8 => integers::input(oid, text),
        oid::FLOAT4 | oid::FLOAT8 => floats::input(oid, text),
        oid::TEXT | oid::VARCHAR | oid::BPCHAR => text::input(oid, text),
        oid::BOOL => boolean::input(oid, text),
        oid::JSON => json::input(oid, text),
        oid::DATE => date::input(oid, text),
        oid::TIME => time::input(oid, text),
        oid::TIMESTAMP => timestamp::input(oid, text),
        oid::NUMERIC => numeric::input(oid, text),
        oid::UUID => uuid::input(oid, text),
        oid::MONEY => money::input(oid, text),
        oid::INET | oid::CIDR => inet::input(oid, text),
        oid::MACADDR | oid::MACADDR8 => macaddr::input(oid, text),
        oid::BIT | oid::VARBIT => bit::input(oid, text),
        oid::INTERVAL => interval::input(oid, text),
        oid::TIMETZ => timetz::input(oid, text),
        oid::BYTEA => bytea::input(oid, text),
        oid::TIMESTAMPTZ => timestamptz::input(oid, text),
        oid::JSONB => jsonb::input(oid, text),
        oid::BOOL_ARRAY
        | oid::INT2_ARRAY
        | oid::INT4_ARRAY
        | oid::TEXT_ARRAY
        | oid::INT8_ARRAY
        | oid::FLOAT8_ARRAY
        | oid::NUMERIC_ARRAY => arrays::input(oid, text),
        oid::INT4RANGE | oid::NUMRANGE | oid::TSRANGE | oid::DATERANGE | oid::INT8RANGE => {
            ranges::input(oid, text)
        }
        oid::INT4MULTIRANGE
        | oid::NUMMULTIRANGE
        | oid::TSMULTIRANGE
        | oid::DATEMULTIRANGE
        | oid::INT8MULTIRANGE => multiranges::input(oid, text),
        oid::OID => oidtype::input(oid, text),
        oid::TID => tid::input(oid, text),
        oid::POINT => point::input(oid, text),
        oid::LSEG => lseg::input(oid, text),
        oid::PATH => path::input(oid, text),
        oid::BOX => boxtype::input(oid, text),
        oid::POLYGON => polygon::input(oid, text),
        oid::LINE => line::input(oid, text),
        oid::CIRCLE => circle::input(oid, text),
        oid::PG_LSN => pglsn::input(oid, text),
        oid::TSVECTOR => tsvector::input(oid, text),
        oid::TSQUERY => tsquery::input(oid, text),
        _ => Err(PgError::InvalidInputSyntax {
            typ: type_name(oid),
            input: text.to_string(),
        }),
    }
}

pub fn output(oid: u32, v: &SqlValue) -> String {
    if matches!(v, SqlValue::Null) {
        return String::new();
    }
    match oid {
        oid::INT2 | oid::INT4 | oid::INT8 => integers::output(oid, v),
        oid::FLOAT4 | oid::FLOAT8 => floats::output(oid, v),
        oid::TEXT | oid::VARCHAR | oid::BPCHAR => text::output(oid, v),
        oid::BOOL => boolean::output(oid, v),
        oid::JSON => json::output(oid, v),
        oid::DATE => date::output(oid, v),
        oid::TIME => time::output(oid, v),
        oid::TIMESTAMP => timestamp::output(oid, v),
        oid::NUMERIC => numeric::output(oid, v),
        oid::UUID => uuid::output(oid, v),
        oid::MONEY => money::output(oid, v),
        oid::INET | oid::CIDR => inet::output(oid, v),
        oid::MACADDR | oid::MACADDR8 => macaddr::output(oid, v),
        oid::BIT | oid::VARBIT => bit::output(oid, v),
        oid::INTERVAL => interval::output(oid, v),
        oid::TIMETZ => timetz::output(oid, v),
        oid::BYTEA => bytea::output(oid, v),
        oid::TIMESTAMPTZ => timestamptz::output(oid, v),
        oid::JSONB => jsonb::output(oid, v),
        oid::BOOL_ARRAY
        | oid::INT2_ARRAY
        | oid::INT4_ARRAY
        | oid::TEXT_ARRAY
        | oid::INT8_ARRAY
        | oid::FLOAT8_ARRAY
        | oid::NUMERIC_ARRAY => arrays::output(oid, v),
        oid::INT4RANGE | oid::NUMRANGE | oid::TSRANGE | oid::DATERANGE | oid::INT8RANGE => {
            ranges::output(oid, v)
        }
        oid::INT4MULTIRANGE
        | oid::NUMMULTIRANGE
        | oid::TSMULTIRANGE
        | oid::DATEMULTIRANGE
        | oid::INT8MULTIRANGE => multiranges::output(oid, v),
        oid::OID => oidtype::output(oid, v),
        oid::TID => tid::output(oid, v),
        oid::POINT => point::output(oid, v),
        oid::LSEG => lseg::output(oid, v),
        oid::PATH => path::output(oid, v),
        oid::BOX => boxtype::output(oid, v),
        oid::POLYGON => polygon::output(oid, v),
        oid::LINE => line::output(oid, v),
        oid::CIRCLE => circle::output(oid, v),
        oid::PG_LSN => pglsn::output(oid, v),
        oid::TSVECTOR => tsvector::output(oid, v),
        oid::TSQUERY => tsquery::output(oid, v),
        _ => String::new(),
    }
}

pub fn cast(v: &SqlValue, from: u32, to: u32) -> Result<SqlValue, PgError> {
    if from == to || matches!(v, SqlValue::Null) {
        return Ok(v.clone());
    }
    input(to, &output(from, v))
}

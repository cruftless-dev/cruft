
use std::collections::{BTreeMap, HashSet};

use crate::{affinity_type_name, parse_one, Affinity, ColumnDef, SqlResult, Stmt, Table, Value};

const MAGIC: &[u8] = b"SQLite format 3\0";
const MAX_BTREE_DEPTH: usize = 128;

fn checked_slice<'a>(data: &'a [u8], off: usize, len: usize, what: &str) -> SqlResult<&'a [u8]> {
    let end = off
        .checked_add(len)
        .ok_or_else(|| format!("{what} offset overflow"))?;
    data.get(off..end)
        .ok_or_else(|| format!("truncated sqlite file while reading {what}"))
}

fn read_u16(data: &[u8], off: usize, what: &str) -> SqlResult<u16> {
    let bytes = checked_slice(data, off, 2, what)?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], off: usize, what: &str) -> SqlResult<u32> {
    let bytes = checked_slice(data, off, 4, what)?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn page_count(data: &[u8], page_size: usize) -> usize {
    data.len() / page_size
}

fn page_start(data: &[u8], page_size: usize, pgno: usize) -> SqlResult<usize> {
    if pgno == 0 {
        return Err("sqlite page number 0 is invalid".into());
    }
    let start = (pgno - 1)
        .checked_mul(page_size)
        .ok_or_else(|| "sqlite page offset overflow".to_string())?;
    if start + page_size > data.len() {
        return Err(format!("sqlite page {pgno} is outside the database image"));
    }
    Ok(start)
}

fn read_varint(data: &[u8], off: usize) -> SqlResult<(i64, usize)> {
    let mut result: u64 = 0;
    let mut i = 0;
    while i < 9 {
        let byte = *data
            .get(off + i)
            .ok_or_else(|| "truncated sqlite varint".to_string())?;
        if i == 8 {
            result = (result << 8) | byte as u64;
            i += 1;
            break;
        }
        result = (result << 7) | (byte & 0x7f) as u64;
        i += 1;
        if byte & 0x80 == 0 {
            break;
        }
    }
    Ok((result as i64, i))
}

fn be_signed(bytes: &[u8]) -> i64 {
    let mut v: u64 = 0;
    for &b in bytes {
        v = (v << 8) | b as u64;
    }
    let shift = 64 - bytes.len() * 8;
    ((v << shift) as i64) >> shift
}

fn decode_text(bytes: &[u8], enc: u32) -> String {
    match enc {
        2 => {
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        }
        3 => {
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        }
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

fn read_serial(rec: &[u8], serial: i64, enc: u32) -> SqlResult<(Value, usize)> {
    Ok(match serial {
        0 => (Value::Null, 0),
        1 => (
            Value::Int(be_signed(checked_slice(rec, 0, 1, "record int1")?)),
            1,
        ),
        2 => (
            Value::Int(be_signed(checked_slice(rec, 0, 2, "record int2")?)),
            2,
        ),
        3 => (
            Value::Int(be_signed(checked_slice(rec, 0, 3, "record int3")?)),
            3,
        ),
        4 => (
            Value::Int(be_signed(checked_slice(rec, 0, 4, "record int4")?)),
            4,
        ),
        5 => (
            Value::Int(be_signed(checked_slice(rec, 0, 6, "record int6")?)),
            6,
        ),
        6 => (
            Value::Int(be_signed(checked_slice(rec, 0, 8, "record int8")?)),
            8,
        ),
        7 => {
            let mut b = [0u8; 8];
            b.copy_from_slice(checked_slice(rec, 0, 8, "record real")?);
            (Value::Real(f64::from_be_bytes(b)), 8)
        }
        8 => (Value::Int(0), 0),
        9 => (Value::Int(1), 0),
        10 | 11 => return Err("reserved record serial type".into()),
        n if n >= 12 && n % 2 == 0 => {
            let len = ((n - 12) / 2) as usize;
            (
                Value::Blob(checked_slice(rec, 0, len, "record blob")?.to_vec()),
                len,
            )
        }
        n => {

            let len = ((n - 13) / 2) as usize;
            (
                Value::Text(decode_text(checked_slice(rec, 0, len, "record text")?, enc)),
                len,
            )
        }
    })
}

fn parse_record(rec: &[u8], enc: u32) -> SqlResult<Vec<Value>> {
    let (hdr_len, n) = read_varint(rec, 0)?;
    let hdr_len = hdr_len as usize;
    if hdr_len < n || hdr_len > rec.len() {
        return Err("sqlite record header length is outside the payload".into());
    }
    let mut off = n;
    let mut serials = Vec::new();
    while off < hdr_len {
        let (s, sn) = read_varint(rec, off)?;
        serials.push(s);
        off += sn;
    }
    let mut body = hdr_len;
    let mut vals = Vec::with_capacity(serials.len());
    for s in serials {
        let (v, len) = read_serial(&rec[body..], s, enc)?;
        vals.push(v);
        body += len;
    }
    Ok(vals)
}

fn read_payload(
    data: &[u8],
    page_size: usize,
    usable: usize,
    max_local: usize,
    start: usize,
    payload_len: usize,
) -> SqlResult<Vec<u8>> {
    if payload_len > data.len() {
        return Err("sqlite cell payload length exceeds database image".into());
    }
    if payload_len <= max_local {
        return Ok(checked_slice(data, start, payload_len, "local payload")?.to_vec());
    }
    if usable <= 12 {
        return Err("sqlite usable page size is too small for overflow payload".into());
    }
    let min_local = (usable - 12) * 32 / 255 - 23;
    let mut local = min_local + (payload_len - min_local) % (usable - 4);
    if local > max_local {
        local = min_local;
    }
    checked_slice(data, start, local + 4, "overflow payload pointer")?;
    let mut payload = Vec::with_capacity(payload_len);
    payload.extend_from_slice(checked_slice(data, start, local, "local overflow payload")?);
    let mut next = read_u32(data, start + local, "first overflow page pointer")? as usize;
    let mut seen = HashSet::new();
    while next != 0 && payload.len() < payload_len {
        if !seen.insert(next) {
            return Err("sqlite overflow page cycle".into());
        }
        let pg = page_start(data, page_size, next)?;
        let nptr = read_u32(data, pg, "overflow next-page pointer")? as usize;
        let take = std::cmp::min(usable - 4, payload_len - payload.len());
        payload.extend_from_slice(checked_slice(data, pg + 4, take, "overflow payload bytes")?);
        next = nptr;
    }
    if payload.len() != payload_len {
        return Err("sqlite overflow chain ended before payload was complete".into());
    }
    Ok(payload)
}

fn read_table_btree(
    data: &[u8],
    page_size: usize,
    usable: usize,
    enc: u32,
    pgno: usize,
    out: &mut Vec<(i64, Vec<Value>)>,
    seen: &mut HashSet<usize>,
    depth: usize,
) -> SqlResult<()> {
    if depth > MAX_BTREE_DEPTH || depth > page_count(data, page_size) {
        return Err("sqlite table b-tree depth limit exceeded".into());
    }
    if !seen.insert(pgno) {
        return Err("sqlite table b-tree page cycle".into());
    }
    let page_start = page_start(data, page_size, pgno)?;

    let hdr = if pgno == 1 {
        page_start + 100
    } else {
        page_start
    };
    let ptype = *checked_slice(data, hdr, 1, "table b-tree page type")?
        .first()
        .unwrap();
    let cell_count = read_u16(data, hdr + 3, "table b-tree cell count")? as usize;
    let (ptr_array, is_leaf) = match ptype {
        0x0d => (hdr + 8, true),
        0x05 => (hdr + 12, false),
        other => return Err(format!("unsupported b-tree page type {other:#x}")),
    };
    checked_slice(
        data,
        ptr_array,
        cell_count * 2,
        "table b-tree cell pointer array",
    )?;
    for i in 0..cell_count {
        let p = ptr_array + i * 2;
        let cptr = read_u16(data, p, "table b-tree cell pointer")? as usize;
        if cptr >= page_size {
            return Err("sqlite table b-tree cell pointer outside page".into());
        }
        let cell = page_start + cptr;
        if is_leaf {
            let (payload_len, n1) = read_varint(data, cell)?;
            let (rowid, n2) = read_varint(data, cell + n1)?;
            let rec = cell + n1 + n2;
            let payload = read_payload(
                data,
                page_size,
                usable,
                usable - 35,
                rec,
                payload_len as usize,
            )?;
            let values = parse_record(&payload, enc)?;
            out.push((rowid, values));
        } else {
            let child = read_u32(data, cell, "table b-tree child page")?;
            read_table_btree(
                data,
                page_size,
                usable,
                enc,
                child as usize,
                out,
                seen,
                depth + 1,
            )?;
        }
    }
    if !is_leaf {

        let r = hdr + 8;
        let right = read_u32(data, r, "table b-tree right child page")?;
        read_table_btree(
            data,
            page_size,
            usable,
            enc,
            right as usize,
            out,
            seen,
            depth + 1,
        )?;
    }
    Ok(())
}

fn read_index_btree(
    data: &[u8],
    page_size: usize,
    usable: usize,
    enc: u32,
    pgno: usize,
    out: &mut Vec<Vec<Value>>,
    seen: &mut HashSet<usize>,
    depth: usize,
) -> SqlResult<()> {
    if depth > MAX_BTREE_DEPTH || depth > page_count(data, page_size) {
        return Err("sqlite index b-tree depth limit exceeded".into());
    }
    if !seen.insert(pgno) {
        return Err("sqlite index b-tree page cycle".into());
    }
    let page_start = page_start(data, page_size, pgno)?;
    let hdr = if pgno == 1 {
        page_start + 100
    } else {
        page_start
    };
    let ptype = *checked_slice(data, hdr, 1, "index b-tree page type")?
        .first()
        .unwrap();
    let cell_count = read_u16(data, hdr + 3, "index b-tree cell count")? as usize;
    let (ptr_array, is_leaf) = match ptype {
        0x0a => (hdr + 8, true),
        0x02 => (hdr + 12, false),
        other => return Err(format!("unsupported index b-tree page type {other:#x}")),
    };
    checked_slice(
        data,
        ptr_array,
        cell_count * 2,
        "index b-tree cell pointer array",
    )?;
    for i in 0..cell_count {
        let p = ptr_array + i * 2;
        let cptr = read_u16(data, p, "index b-tree cell pointer")? as usize;
        if cptr >= page_size {
            return Err("sqlite index b-tree cell pointer outside page".into());
        }
        let cell = page_start + cptr;
        if is_leaf {
            let (payload_len, n1) = read_varint(data, cell)?;
            let payload = read_payload(
                data,
                page_size,
                usable,
                (usable - 12) * 64 / 255 - 23,
                cell + n1,
                payload_len as usize,
            )?;
            out.push(parse_record(&payload, enc)?);
        } else {

            let child = read_u32(data, cell, "index b-tree child page")?;
            read_index_btree(
                data,
                page_size,
                usable,
                enc,
                child as usize,
                out,
                seen,
                depth + 1,
            )?;
            let (payload_len, n1) = read_varint(data, cell + 4)?;
            let payload = read_payload(
                data,
                page_size,
                usable,
                (usable - 12) * 64 / 255 - 23,
                cell + 4 + n1,
                payload_len as usize,
            )?;
            out.push(parse_record(&payload, enc)?);
        }
    }
    if !is_leaf {
        let r = hdr + 8;
        let right = read_u32(data, r, "index b-tree right child page")?;
        read_index_btree(
            data,
            page_size,
            usable,
            enc,
            right as usize,
            out,
            seen,
            depth + 1,
        )?;
    }
    Ok(())
}

pub fn is_sqlite_file(bytes: &[u8]) -> bool {
    bytes.len() >= 16 && &bytes[..16] == MAGIC
}

fn wal_checksum(data: &[u8], be: bool, mut s0: u32, mut s1: u32) -> (u32, u32) {
    let mut i = 0;
    while i + 8 <= data.len() {
        let (x0, x1) = if be {
            (
                u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]),
                u32::from_be_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]),
            )
        } else {
            (
                u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]),
                u32::from_le_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]),
            )
        };
        s0 = s0.wrapping_add(x0).wrapping_add(s1);
        s1 = s1.wrapping_add(x1).wrapping_add(s0);
        i += 8;
    }
    (s0, s1)
}

pub fn apply_wal(main: &[u8], wal: &[u8]) -> Vec<u8> {
    if wal.len() < 32 {
        return main.to_vec();
    }
    let magic = u32::from_be_bytes([wal[0], wal[1], wal[2], wal[3]]);
    if magic != 0x377f_0682 && magic != 0x377f_0683 {
        return main.to_vec();
    }
    let page_size = u32::from_be_bytes([wal[8], wal[9], wal[10], wal[11]]) as usize;
    if page_size == 0 {
        return main.to_vec();
    }
    let (salt1, salt2) = (&wal[16..20], &wal[20..24]);
    let frame_size = 24 + page_size;

    let be = (magic & 1) == 1;
    let (mut s0, mut s1) = (
        u32::from_be_bytes([wal[24], wal[25], wal[26], wal[27]]),
        u32::from_be_bytes([wal[28], wal[29], wal[30], wal[31]]),
    );

    let mut committed: std::collections::HashMap<usize, &[u8]> = std::collections::HashMap::new();
    let mut db_pages = 0usize;
    let mut pending: Vec<(usize, &[u8])> = Vec::new();
    let mut off = 32;
    while off + frame_size <= wal.len() {
        let fh = &wal[off..off + 24];
        if &fh[8..12] != salt1 || &fh[12..16] != salt2 {
            break;
        }

        let (n0, n1) = wal_checksum(&wal[off..off + 8], be, s0, s1);
        let (n0, n1) = wal_checksum(&wal[off + 24..off + 24 + page_size], be, n0, n1);
        let stored = (
            u32::from_be_bytes([fh[16], fh[17], fh[18], fh[19]]),
            u32::from_be_bytes([fh[20], fh[21], fh[22], fh[23]]),
        );
        if (n0, n1) != stored {
            break;
        }
        s0 = n0;
        s1 = n1;
        let pgno = u32::from_be_bytes([fh[0], fh[1], fh[2], fh[3]]) as usize;
        let commit_size = u32::from_be_bytes([fh[4], fh[5], fh[6], fh[7]]) as usize;
        let page = &wal[off + 24..off + 24 + page_size];
        pending.push((pgno, page));
        if commit_size != 0 {
            for (p, pg) in pending.drain(..) {
                committed.insert(p, pg);
            }
            db_pages = commit_size;
        }
        off += frame_size;
    }
    if committed.is_empty() {
        return main.to_vec();
    }

    let mut out = vec![0u8; db_pages * page_size];
    for pg in 1..=db_pages {
        let dst = (pg - 1) * page_size;
        if let Some(data) = committed.get(&pg) {
            out[dst..dst + page_size].copy_from_slice(data);
        } else {
            let src = (pg - 1) * page_size;
            if src + page_size <= main.len() {
                out[dst..dst + page_size].copy_from_slice(&main[src..src + page_size]);
            }
        }
    }
    out
}

pub fn read_sqlite_file(bytes: &[u8]) -> SqlResult<BTreeMap<String, Table>> {
    if !is_sqlite_file(bytes) {
        return Err("not a sqlite3 file".into());
    }
    if bytes.len() < 100 {
        return Err("truncated sqlite database header".into());
    }
    let ps = read_u16(bytes, 16, "sqlite page size")? as usize;
    let page_size = if ps == 1 { 65536 } else { ps };
    if page_size == 0
        || page_size > bytes.len()
        || bytes.len() % page_size != 0
        || page_size < 512
        || page_size > 65536
        || (page_size & (page_size - 1)) != 0
    {
        return Err("bad page size".into());
    }

    let enc = read_u32(bytes, 56, "sqlite text encoding")?;
    if enc > 3 {
        return Err(format!("unknown text encoding {enc}"));
    }

    let reserved = bytes[20] as usize;
    if reserved >= page_size || page_size - reserved < 480 {
        return Err("bad sqlite reserved page size".into());
    }
    let usable = page_size - reserved;

    let mut master = Vec::new();
    read_table_btree(
        bytes,
        page_size,
        usable,
        enc,
        1,
        &mut master,
        &mut HashSet::new(),
        0,
    )?;

    let mut tables = BTreeMap::new();
    for (_rowid, cols) in &master {
        if cols.len() < 5 {
            continue;
        }
        let kind = text_of(&cols[0]);
        let name = text_of(&cols[1]);

        if kind != "table" || (name.starts_with("sqlite_") && name != "sqlite_sequence") {
            continue;
        }
        let rootpage = match &cols[3] {
            Value::Int(n) if *n > 0 => *n as usize,
            _ => continue,
        };
        page_start(bytes, page_size, rootpage)?;
        let sql = text_of(&cols[4]);
        let Stmt::CreateTable {
            columns,
            table_uniques,
            checks,
            foreign_keys,
            ..
        } = parse_one(&sql)?.0
        else {
            continue;
        };
        let pk_int = columns
            .iter()
            .position(|c| c.pk && c.affinity == Affinity::Integer);

        let mut rows = Vec::new();
        let mut row_ids = Vec::new();
        let mut max_rowid = 0i64;

        let root_start = page_start(bytes, page_size, rootpage)?;
        let root_hdr = if rootpage == 1 { 100 } else { root_start };
        let without_rowid = matches!(bytes.get(root_hdr).copied(), Some(0x0a) | Some(0x02));
        if without_rowid {
            let mut recs = Vec::new();
            read_index_btree(
                bytes,
                page_size,
                usable,
                enc,
                rootpage,
                &mut recs,
                &mut HashSet::new(),
                0,
            )?;
            for (i, mut vals) in recs.into_iter().enumerate() {
                while vals.len() < columns.len() {
                    vals.push(Value::Null);
                }
                vals.truncate(columns.len());
                rows.push(vals);
                row_ids.push(i as i64 + 1);
                max_rowid = i as i64 + 1;
            }
        } else {
            let mut raw = Vec::new();
            read_table_btree(
                bytes,
                page_size,
                usable,
                enc,
                rootpage,
                &mut raw,
                &mut HashSet::new(),
                0,
            )?;
            for (rowid, mut vals) in raw {

                while vals.len() < columns.len() {
                    vals.push(Value::Null);
                }
                vals.truncate(columns.len());

                if let Some(i) = pk_int {
                    vals[i] = Value::Int(rowid);
                }
                rows.push(vals);
                row_ids.push(rowid);
                max_rowid = max_rowid.max(rowid);
            }
        }
        tables.insert(
            name.clone(),
            build_table(
                columns,
                rows,
                row_ids,
                max_rowid + 1,
                checks,
                table_uniques,
                foreign_keys,
            ),
        );
    }
    Ok(tables)
}

const PAGE_SIZE: usize = 4096;

fn encode_varint(mut v: u64) -> Vec<u8> {
    if v == 0 {
        return vec![0];
    }
    let mut bytes = Vec::new();
    while v > 0 {
        bytes.push((v & 0x7f) as u8);
        v >>= 7;
    }
    bytes.reverse();
    let n = bytes.len();
    for b in bytes.iter_mut().take(n - 1) {
        *b |= 0x80;
    }
    bytes
}

fn encode_serial(v: &Value) -> (u64, Vec<u8>) {
    match v {
        Value::Null => (0, vec![]),
        Value::Int(0) => (8, vec![]),
        Value::Int(1) => (9, vec![]),
        Value::Int(i) => {
            let i = *i;
            let (serial, len) = if i >= -128 && i <= 127 {
                (1u64, 1)
            } else if i >= -32768 && i <= 32767 {
                (2, 2)
            } else if i >= -8_388_608 && i <= 8_388_607 {
                (3, 3)
            } else if i >= -2_147_483_648 && i <= 2_147_483_647 {
                (4, 4)
            } else if i >= -140_737_488_355_328 && i <= 140_737_488_355_327 {
                (5, 6)
            } else {
                (6, 8)
            };
            let be = i.to_be_bytes();
            (serial, be[8 - len..].to_vec())
        }
        Value::Real(r) => (7, r.to_be_bytes().to_vec()),
        Value::Text(s) => (13 + 2 * s.len() as u64, s.as_bytes().to_vec()),
        Value::Blob(b) => (12 + 2 * b.len() as u64, b.clone()),
    }
}

fn encode_record(vals: &[Value], pk_int: Option<usize>) -> Vec<u8> {
    let mut serials = Vec::with_capacity(vals.len());
    let mut body = Vec::new();
    for (i, v) in vals.iter().enumerate() {
        if pk_int == Some(i) {
            serials.push(encode_varint(0));
            continue;
        }
        let (serial, bytes) = encode_serial(v);
        serials.push(encode_varint(serial));
        body.extend_from_slice(&bytes);
    }
    let serials_len: usize = serials.iter().map(|s| s.len()).sum();

    let mut header_len = serials_len + 1;
    if header_len >= 128 {
        header_len = serials_len + 2;
    }
    let mut rec = encode_varint(header_len as u64);
    for s in &serials {
        rec.extend_from_slice(s);
    }
    rec.extend_from_slice(&body);
    rec
}

fn create_sql(name: &str, tbl: &Table) -> String {
    let columns = &tbl.columns;
    let mut parts: Vec<String> = columns
        .iter()
        .map(|c| {
            let mut s = format!("{} {}", c.name, affinity_type_name(c.affinity));
            if c.pk {
                s.push_str(" PRIMARY KEY");
                if c.autoincrement {
                    s.push_str(" AUTOINCREMENT");
                }
            }
            if c.not_null {
                s.push_str(" NOT NULL");
            }
            if c.unique && !c.pk {
                s.push_str(" UNIQUE");
            }
            if let Some(d) = &c.default {
                s.push_str(&format!(" DEFAULT ({})", crate::expr_to_sql(d)));
            }
            s
        })
        .collect();

    for cols in &tbl.table_uniques {
        let names: Vec<String> = cols
            .iter()
            .filter_map(|&i| columns.get(i).map(|c| c.name.clone()))
            .collect();
        parts.push(format!("UNIQUE({})", names.join(", ")));
    }

    for (_, src) in &tbl.checks {
        parts.push(format!("CHECK ({src})"));
    }

    for fk in &tbl.foreign_keys {
        if fk.parent_col.is_empty() {
            parts.push(format!(
                "FOREIGN KEY({}) REFERENCES {}",
                fk.col, fk.parent_table
            ));
        } else {
            parts.push(format!(
                "FOREIGN KEY({}) REFERENCES {}({})",
                fk.col, fk.parent_table, fk.parent_col
            ));
        }
    }

    format!("CREATE TABLE {name} ({})", parts.join(", "))
}

struct Pager {
    pages: Vec<[u8; PAGE_SIZE]>,
}

impl Pager {
    fn new() -> Self {

        Pager {
            pages: vec![[0u8; PAGE_SIZE]],
        }
    }

    fn alloc(&mut self) -> usize {
        self.pages.push([0u8; PAGE_SIZE]);
        self.pages.len()
    }
    fn page_mut(&mut self, pgno: usize) -> &mut [u8; PAGE_SIZE] {
        &mut self.pages[pgno - 1]
    }
    fn total(&self) -> usize {
        self.pages.len()
    }
}

fn make_leaf_cell(pager: &mut Pager, usable: usize, rowid: i64, record: &[u8]) -> Vec<u8> {
    let p = record.len();
    let max_local = usable - 35;
    let mut cell = encode_varint(p as u64);
    cell.extend_from_slice(&encode_varint(rowid as u64));
    if p <= max_local {
        cell.extend_from_slice(record);
        return cell;
    }

    let min_local = (usable - 12) * 32 / 255 - 23;
    let mut local = min_local + (p - min_local) % (usable - 4);
    if local > max_local {
        local = min_local;
    }
    cell.extend_from_slice(&record[..local]);

    let first_ov = pager.alloc();
    let mut rest = &record[local..];
    let mut cur = first_ov;
    loop {
        let take = std::cmp::min(usable - 4, rest.len());
        let next = if rest.len() > take { pager.alloc() } else { 0 };
        let page = pager.page_mut(cur);
        page[0..4].copy_from_slice(&(next as u32).to_be_bytes());
        page[4..4 + take].copy_from_slice(&rest[..take]);
        rest = &rest[take..];
        if next == 0 {
            break;
        }
        cur = next;
    }
    cell.extend_from_slice(&(first_ov as u32).to_be_bytes());
    cell
}

fn write_leaf_page(pager: &mut Pager, pgno: usize, base: usize, cells: &[Vec<u8>]) {
    let page = pager.page_mut(pgno);
    let hdr = base;
    let ptr_array = hdr + 8;
    let mut content = PAGE_SIZE;
    let mut ptrs = Vec::with_capacity(cells.len());
    for cell in cells {
        content -= cell.len();
        page[content..content + cell.len()].copy_from_slice(cell);
        ptrs.push(content as u16);
    }
    page[hdr] = 0x0d;
    page[hdr + 1..hdr + 3].copy_from_slice(&0u16.to_be_bytes());
    page[hdr + 3..hdr + 5].copy_from_slice(&(cells.len() as u16).to_be_bytes());
    page[hdr + 5..hdr + 7].copy_from_slice(&(content as u16).to_be_bytes());
    page[hdr + 7] = 0;
    for (i, p) in ptrs.iter().enumerate() {
        page[ptr_array + i * 2..ptr_array + i * 2 + 2].copy_from_slice(&p.to_be_bytes());
    }
}

fn write_interior_page(
    pager: &mut Pager,
    pgno: usize,
    base: usize,
    entries: &[(usize, i64)],
    right: usize,
) {
    let page = pager.page_mut(pgno);
    let hdr = base;
    let ptr_array = hdr + 12;
    let mut content = PAGE_SIZE;
    let mut ptrs = Vec::with_capacity(entries.len());
    for (child, key) in entries {
        let mut cell = (*child as u32).to_be_bytes().to_vec();
        cell.extend_from_slice(&encode_varint(*key as u64));
        content -= cell.len();
        page[content..content + cell.len()].copy_from_slice(&cell);
        ptrs.push(content as u16);
    }
    page[hdr] = 0x05;
    page[hdr + 1..hdr + 3].copy_from_slice(&0u16.to_be_bytes());
    page[hdr + 3..hdr + 5].copy_from_slice(&(entries.len() as u16).to_be_bytes());
    page[hdr + 5..hdr + 7].copy_from_slice(&(content as u16).to_be_bytes());
    page[hdr + 7] = 0;
    page[hdr + 8..hdr + 12].copy_from_slice(&(right as u32).to_be_bytes());
    for (i, p) in ptrs.iter().enumerate() {
        page[ptr_array + i * 2..ptr_array + i * 2 + 2].copy_from_slice(&p.to_be_bytes());
    }
}

fn build_btree(
    pager: &mut Pager,
    usable: usize,
    root_page: usize,
    root_base: usize,
    cells: &[Vec<u8>],
    rowids: &[i64],
) {

    let single: usize = 8 + cells.iter().map(|c| c.len() + 2).sum::<usize>();
    if single <= usable - root_base {
        write_leaf_page(pager, root_page, root_base, cells);
        return;
    }

    let mut level: Vec<(usize, i64)> = Vec::new();
    let mut i = 0;
    while i < cells.len() {
        let mut cost = 8usize;
        let mut end = i;
        while end < cells.len() {
            let c = cells[end].len() + 2;
            if end > i && cost + c > usable {
                break;
            }
            cost += c;
            end += 1;
        }
        let pg = pager.alloc();
        write_leaf_page(pager, pg, 0, &cells[i..end]);
        level.push((pg, rowids[end - 1]));
        i = end;
    }

    let cap = usable - root_base - 12;
    loop {

        let mut groups: Vec<Vec<(usize, i64)>> = Vec::new();
        let mut cur: Vec<(usize, i64)> = Vec::new();
        let mut cost = 0usize;
        for &(child, key) in &level {
            let cell_cost = 4 + encode_varint(key as u64).len() + 2;
            if !cur.is_empty() && cost + cell_cost > cap {
                groups.push(std::mem::take(&mut cur));
                cost = 0;
            }
            cur.push((child, key));
            cost += cell_cost;
        }
        if !cur.is_empty() {
            groups.push(cur);
        }

        if groups.len() == 1 {

            let g = &groups[0];
            let (right, _) = *g.last().unwrap();
            let entries: Vec<(usize, i64)> = g[..g.len() - 1].to_vec();
            write_interior_page(pager, root_page, root_base, &entries, right);
            return;
        }

        let mut next_level: Vec<(usize, i64)> = Vec::with_capacity(groups.len());
        for g in &groups {
            let pg = pager.alloc();
            let (right, max_key) = *g.last().unwrap();
            let entries: Vec<(usize, i64)> = g[..g.len() - 1].to_vec();
            write_interior_page(pager, pg, 0, &entries, right);
            next_level.push((pg, max_key));
        }
        level = next_level;
    }
}

pub fn write_sqlite_file(tables: &BTreeMap<String, Table>) -> Option<Vec<u8>> {
    let usable = PAGE_SIZE;
    let mut pager = Pager::new();

    let ordered: Vec<(&String, &Table)> = tables.iter().collect();
    let mut roots: Vec<usize> = Vec::with_capacity(ordered.len());
    for _ in &ordered {
        roots.push(pager.alloc());
    }

    let mut master_cells = Vec::new();
    let mut master_rowids = Vec::new();
    for (idx, (name, tbl)) in ordered.iter().enumerate() {
        let rootpage = roots[idx];
        let pk_int = tbl
            .columns
            .iter()
            .position(|c| c.pk && c.affinity == Affinity::Integer);

        let mut cells = Vec::with_capacity(tbl.rows.len());
        let mut rowids = Vec::with_capacity(tbl.rows.len());
        for (r, row) in tbl.rows.iter().enumerate() {
            let rowid = tbl.row_ids.get(r).copied().unwrap_or(r as i64 + 1);
            let rec = encode_record(row, pk_int);
            cells.push(make_leaf_cell(&mut pager, usable, rowid, &rec));
            rowids.push(rowid);
        }
        build_btree(&mut pager, usable, rootpage, 0, &cells, &rowids);

        let sql = create_sql(name, tbl);
        let master_row = vec![
            Value::Text("table".into()),
            Value::Text((*name).clone()),
            Value::Text((*name).clone()),
            Value::Int(rootpage as i64),
            Value::Text(sql),
        ];
        let rec = encode_record(&master_row, None);
        let mrowid = idx as i64 + 1;
        master_cells.push(make_leaf_cell(&mut pager, usable, mrowid, &rec));
        master_rowids.push(mrowid);
    }

    build_btree(&mut pager, usable, 1, 100, &master_cells, &master_rowids);

    let total_pages = pager.total();
    let mut out = vec![0u8; total_pages * PAGE_SIZE];
    for (i, pg) in pager.pages.iter().enumerate() {
        out[i * PAGE_SIZE..(i + 1) * PAGE_SIZE].copy_from_slice(pg);
    }
    write_header(&mut out, total_pages);
    Some(out)
}

fn write_header(out: &mut [u8], total_pages: usize) {
    out[..16].copy_from_slice(MAGIC);
    out[16..18].copy_from_slice(&(PAGE_SIZE as u16).to_be_bytes());
    out[18] = 1;
    out[19] = 1;
    out[20] = 0;
    out[21] = 64;
    out[22] = 32;
    out[23] = 32;
    out[24..28].copy_from_slice(&1u32.to_be_bytes());
    out[28..32].copy_from_slice(&(total_pages as u32).to_be_bytes());
    out[32..36].copy_from_slice(&0u32.to_be_bytes());
    out[36..40].copy_from_slice(&0u32.to_be_bytes());
    out[40..44].copy_from_slice(&1u32.to_be_bytes());
    out[44..48].copy_from_slice(&4u32.to_be_bytes());
    out[48..52].copy_from_slice(&0u32.to_be_bytes());
    out[52..56].copy_from_slice(&0u32.to_be_bytes());
    out[56..60].copy_from_slice(&1u32.to_be_bytes());
    out[60..64].copy_from_slice(&0u32.to_be_bytes());
    out[64..68].copy_from_slice(&0u32.to_be_bytes());

    out[92..96].copy_from_slice(&1u32.to_be_bytes());
    out[96..100].copy_from_slice(&3_045_000u32.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINI: &[u8] = include_bytes!("testdata/mini.sqlite");

    #[test]
    fn reads_bun_written_file() {
        let tables = read_sqlite_file(MINI).unwrap();
        let t = tables.get("t").expect("table t");
        assert_eq!(t.rows.len(), 3);

        assert!(matches!(&t.rows[0][0], Value::Int(1)));
        assert!(matches!(&t.rows[0][1], Value::Text(s) if s == "alice"));
        assert!(matches!(&t.rows[0][2], Value::Int(100)));
        assert!(matches!(&t.rows[0][3], Value::Real(r) if (*r - 1.5).abs() < 1e-9));
        assert!(matches!(&t.rows[0][4], Value::Null));

        assert!(matches!(&t.rows[1][2], Value::Int(-7)));
        assert!(matches!(&t.rows[1][3], Value::Null));
        assert!(matches!(&t.rows[1][4], Value::Blob(b) if b == &[1, 2, 3]));

        assert!(matches!(&t.rows[2][0], Value::Int(3)));
        assert!(matches!(&t.rows[2][1], Value::Null));
        assert!(matches!(&t.rows[2][2], Value::Int(999999999999)));

        assert_eq!(t.row_ids, vec![1, 2, 3]);
        assert_eq!(t.next_rowid, 4);
    }

    #[test]
    fn rejects_truncated_sqlite_header_without_panic() {
        let err = read_sqlite_file(&MINI[..32]).unwrap_err();
        assert!(err.contains("header") || err.contains("page size"), "{err}");
    }

    #[test]
    fn rejects_bad_reserved_page_size_without_underflow() {
        let mut db = MINI.to_vec();
        db[16..18].copy_from_slice(&512u16.to_be_bytes());
        db[20] = 255;
        let err = read_sqlite_file(&db).unwrap_err();
        assert!(
            err.contains("reserved") || err.contains("page size"),
            "{err}"
        );
    }

    #[test]
    fn rejects_cell_pointer_outside_page_without_panic() {
        let mut db = MINI.to_vec();
        let hdr = 100usize;
        db[hdr] = 0x0d;
        db[hdr + 3..hdr + 5].copy_from_slice(&1u16.to_be_bytes());
        db[hdr + 8..hdr + 10].copy_from_slice(&(4096u16).to_be_bytes());
        let err = read_sqlite_file(&db).unwrap_err();
        assert!(
            err.contains("cell pointer") || err.contains("outside"),
            "{err}"
        );
    }

    #[test]
    fn rejects_table_btree_page_cycle_without_stack_overflow() {
        let mut db = MINI.to_vec();
        let hdr = 100usize;
        db[hdr] = 0x05;
        db[hdr + 3..hdr + 5].copy_from_slice(&0u16.to_be_bytes());
        db[hdr + 8..hdr + 12].copy_from_slice(&1u32.to_be_bytes());
        let err = read_sqlite_file(&db).unwrap_err();
        assert!(err.contains("cycle") || err.contains("depth"), "{err}");
    }

    const OVERFLOW: &[u8] = include_bytes!("testdata/overflow.sqlite");

    #[test]
    fn reads_overflow_payload() {
        let tables = read_sqlite_file(OVERFLOW).unwrap();
        let t = tables.get("t").expect("table t");
        assert_eq!(t.rows.len(), 2);
        match &t.rows[0][1] {
            Value::Text(s) => {
                assert_eq!(s.len(), 9000);
                assert!(s.bytes().all(|b| b == b'Q'), "overflow content corrupted");
            }
            other => panic!("expected text, got {other:?}"),
        }
        assert!(matches!(&t.rows[1][1], Value::Text(s) if s == "z"));
    }

    #[test]
    fn rejects_overflow_page_cycle_without_unbounded_loop() {
        let mut db = OVERFLOW.to_vec();
        let first_overflow = (1..=db.len() / 4096)
            .find(|&pg| {
                let off = (pg - 1) * 4096;
                pg != 1 && db.get(off).copied() == Some(0)
            })
            .expect("overflow test fixture has overflow pages");
        let off = (first_overflow - 1) * 4096;
        db[off..off + 4].copy_from_slice(&(first_overflow as u32).to_be_bytes());
        let err = read_sqlite_file(&db).unwrap_err();
        assert!(err.contains("overflow") && err.contains("cycle"), "{err}");
    }

    const UTF16LE: &[u8] = include_bytes!("testdata/utf16le.sqlite");

    #[test]
    fn reads_utf16le_text() {
        let tables = read_sqlite_file(UTF16LE).unwrap();
        let t = tables.get("t").expect("table t");
        assert_eq!(t.rows.len(), 2);
        assert!(
            matches!(&t.rows[0][1], Value::Text(s) if s == "héllo-wörld"),
            "got {:?}",
            t.rows[0][1]
        );
        assert!(matches!(&t.rows[1][1], Value::Text(s) if s == "日本語"));
    }

    const WITHOUT_ROWID: &[u8] = include_bytes!("testdata/without_rowid.sqlite");

    #[test]
    fn reads_without_rowid_index_btree() {
        let tables = read_sqlite_file(WITHOUT_ROWID).unwrap();
        let t = tables.get("t").expect("table t");
        assert_eq!(t.rows.len(), 4);

        assert!(matches!(&t.rows[0][0], Value::Text(s) if s == "apple"));
        assert!(matches!(&t.rows[0][1], Value::Int(1)));
        assert!(matches!(&t.rows[0][2], Value::Text(s) if s == "red"));
        assert!(matches!(&t.rows[1][2], Value::Null));
        assert!(matches!(&t.rows[3][0], Value::Text(s) if s == "date"));
    }

    const WAL_MAIN: &[u8] = include_bytes!("testdata/wal_main.sqlite");
    const WAL_SIDE: &[u8] = include_bytes!("testdata/wal_side.sqlite-wal");

    #[test]
    fn reads_wal_overlay() {

        let overlaid = apply_wal(WAL_MAIN, WAL_SIDE);
        let tables = read_sqlite_file(&overlaid).expect("read overlaid image");
        let t = tables.get("t").expect("table t (from the WAL)");
        assert_eq!(t.rows.len(), 3);
        assert!(matches!(&t.rows[0][1], Value::Text(s) if s == "alpha"));
        assert!(matches!(&t.rows[2][1], Value::Text(s) if s == "gamma"));
    }

    #[test]
    fn write_then_read_round_trips() {

        let orig = read_sqlite_file(MINI).unwrap();
        let bytes = write_sqlite_file(&orig).expect("single-leaf writer covers mini");
        assert!(is_sqlite_file(&bytes));
        let round = read_sqlite_file(&bytes).unwrap();
        let a = orig.get("t").unwrap();
        let b = round.get("t").unwrap();

        assert_eq!(format!("{:?}", a.rows), format!("{:?}", b.rows));
        assert_eq!(a.row_ids, b.row_ids);
        assert_eq!(a.columns.len(), b.columns.len());
    }
}

fn text_of(v: &Value) -> String {
    match v {
        Value::Text(s) => s.clone(),
        _ => String::new(),
    }
}

fn build_table(
    columns: Vec<ColumnDef>,
    rows: Vec<Vec<Value>>,
    row_ids: Vec<i64>,
    next_rowid: i64,
    checks: Vec<(crate::Expr, String)>,
    table_uniques: Vec<Vec<usize>>,
    foreign_keys: Vec<crate::ForeignKey>,
) -> Table {
    let max_rowid = row_ids.iter().copied().max().unwrap_or(0);
    crate::Table {
        columns,
        rows,
        row_ids,
        next_rowid,
        max_rowid,
        checks,
        table_uniques,
        indexes: Vec::new(),
        foreign_keys,
        eq_indexes: std::collections::HashMap::new(),
    }
}

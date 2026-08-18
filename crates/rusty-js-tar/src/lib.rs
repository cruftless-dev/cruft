use std::fmt;
use std::path::{Component, Path, PathBuf};

const BLOCK: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TarError {
    TruncatedHeader,
    TruncatedEntry {
        path: String,
        expected: u64,
        got: usize,
    },
    BadChecksum {
        path: String,
        expected: u64,
        actual: u64,
    },
    BadNumber {
        field: &'static str,
    },
    BadPath,
    UnsafePath(String),
    BadPax(String),
    Io(String),
}

impl fmt::Display for TarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TarError::TruncatedHeader => write!(f, "truncated tar header"),
            TarError::TruncatedEntry {
                path,
                expected,
                got,
            } => {
                write!(
                    f,
                    "truncated tar entry {path}: expected {expected} bytes, got {got}"
                )
            }
            TarError::BadChecksum {
                path,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "bad tar checksum {path}: expected {expected}, got {actual}"
                )
            }
            TarError::BadNumber { field } => write!(f, "bad tar numeric field {field}"),
            TarError::BadPath => write!(f, "bad tar path"),
            TarError::UnsafePath(path) => write!(f, "unsafe tar path {path}"),
            TarError::BadPax(e) => write!(f, "bad pax header: {e}"),
            TarError::Io(e) => write!(f, "tar extraction I/O error: {e}"),
        }
    }
}

impl std::error::Error for TarError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Directory,
    Other(u8),
}

impl EntryKind {
    pub fn is_file(self) -> bool {
        matches!(self, EntryKind::File)
    }

    pub fn is_dir(self) -> bool {
        matches!(self, EntryKind::Directory)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub kind: EntryKind,
    pub data: Vec<u8>,
    pub mode: u64,
    pub mtime: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Pax {
    path: Option<PathBuf>,
}

pub fn parse_archive(bytes: &[u8]) -> Result<Vec<Entry>, TarError> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    let mut pending_pax = Pax::default();

    loop {
        if offset + BLOCK > bytes.len() {
            if bytes[offset..].iter().all(|b| *b == 0) {
                return Ok(out);
            }
            return Err(TarError::TruncatedHeader);
        }
        let header = &bytes[offset..offset + BLOCK];
        offset += BLOCK;

        if header.iter().all(|b| *b == 0) {
            if offset + BLOCK <= bytes.len()
                && bytes[offset..offset + BLOCK].iter().all(|b| *b == 0)
            {
                return Ok(out);
            }
            return Ok(out);
        }

        let raw_path = header_path(header)?;
        let path_for_error = raw_path.to_string_lossy().to_string();
        let expected = parse_number(&header[148..156], "checksum")?;
        let actual = checksum(header);
        if expected != actual {
            return Err(TarError::BadChecksum {
                path: path_for_error,
                expected,
                actual,
            });
        }

        let size = parse_number(&header[124..136], "size")?;
        let mode = parse_number(&header[100..108], "mode")?;
        let mtime = parse_number(&header[136..148], "mtime")?;
        let kind = match header[156] {
            0 | b'0' => EntryKind::File,
            b'5' => EntryKind::Directory,
            b'x' => EntryKind::Other(b'x'),
            b'g' => EntryKind::Other(b'g'),
            other => EntryKind::Other(other),
        };

        let size_usize =
            usize::try_from(size).map_err(|_| TarError::BadNumber { field: "size" })?;
        let data_end = offset + size_usize;
        let padded_end = offset + round_block(size_usize);
        if data_end > bytes.len() || padded_end > bytes.len() {
            return Err(TarError::TruncatedEntry {
                path: path_for_error,
                expected: size,
                got: bytes.len().saturating_sub(offset),
            });
        }
        let data = &bytes[offset..data_end];
        offset = padded_end;

        match header[156] {
            b'x' => {
                pending_pax = parse_pax(data)?;
                continue;
            }
            b'g' => continue,
            _ => {}
        }

        let path = pending_pax.path.take().unwrap_or(raw_path);
        out.push(Entry {
            path,
            kind,
            data: data.to_vec(),
            mode,
            mtime,
        });
    }
}

pub fn sanitize_entry_path(path: &Path) -> Result<PathBuf, TarError> {
    if path_has_windows_root_shape(path) {
        return Err(TarError::UnsafePath(path.display().to_string()));
    }

    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) if is_windows_drive_component(part) => {
                return Err(TarError::UnsafePath(path.display().to_string()));
            }
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(TarError::UnsafePath(path.display().to_string()));
            }
        }
    }
    Ok(out)
}

pub fn extract_archive(bytes: &[u8], dest: &Path) -> Result<usize, TarError> {
    let entries = parse_archive(bytes)?;
    let mut file_count = 0usize;
    for entry in entries {
        let safe = sanitize_entry_path(&entry.path)?;
        if safe.as_os_str().is_empty() {
            continue;
        }
        let full = dest.join(&safe);
        match entry.kind {
            EntryKind::File => {
                if let Some(parent) = full.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| TarError::Io(format!("mkdir {}: {e}", parent.display())))?;
                }
                std::fs::write(&full, entry.data)
                    .map_err(|e| TarError::Io(format!("write {}: {e}", full.display())))?;
                file_count += 1;
            }
            EntryKind::Directory => {
                std::fs::create_dir_all(&full)
                    .map_err(|e| TarError::Io(format!("mkdir {}: {e}", full.display())))?;
            }
            EntryKind::Other(kind) => {
                return Err(TarError::UnsafePath(format!(
                    "{} (unsupported entry kind {kind})",
                    entry.path.display()
                )));
            }
        }
    }
    Ok(file_count)
}

fn path_has_windows_root_shape(path: &Path) -> bool {
    let text = path.to_string_lossy();
    text.contains('\\') || text.starts_with("//") || looks_like_drive_prefix(&text)
}

fn is_windows_drive_component(part: &std::ffi::OsStr) -> bool {
    part.to_str().is_some_and(looks_like_drive_prefix)
}

fn looks_like_drive_prefix(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

pub fn build_ustar_archive(files: &[(&str, &[u8])]) -> Result<Vec<u8>, TarError> {
    let mut out = Vec::new();
    for (path, data) in files {
        append_header(&mut out, path, data.len() as u64, 0o644, 0, b'0')?;
        out.extend_from_slice(data);
        pad_block(&mut out, data.len());
    }
    out.extend_from_slice(&[0u8; BLOCK]);
    out.extend_from_slice(&[0u8; BLOCK]);
    Ok(out)
}

fn round_block(n: usize) -> usize {
    (n + BLOCK - 1) / BLOCK * BLOCK
}

fn pad_block(out: &mut Vec<u8>, len: usize) {
    let pad = round_block(len) - len;
    out.resize(out.len() + pad, 0);
}

fn checksum(header: &[u8]) -> u64 {
    header
        .iter()
        .enumerate()
        .map(|(i, b)| {
            if (148..156).contains(&i) {
                b' ' as u64
            } else {
                *b as u64
            }
        })
        .sum()
}

fn parse_number(bytes: &[u8], field: &'static str) -> Result<u64, TarError> {
    if bytes.first().is_some_and(|b| b & 0x80 != 0) {
        let mut value = (bytes[0] & 0x7f) as u64;
        for b in &bytes[1..] {
            value = (value << 8) | (*b as u64);
        }
        return Ok(value);
    }

    let text = bytes
        .iter()
        .copied()
        .take_while(|b| *b != 0 && *b != b' ')
        .collect::<Vec<_>>();
    if text.is_empty() {
        return Ok(0);
    }
    let s = std::str::from_utf8(&text).map_err(|_| TarError::BadNumber { field })?;
    u64::from_str_radix(s.trim(), 8).map_err(|_| TarError::BadNumber { field })
}

fn header_path(header: &[u8]) -> Result<PathBuf, TarError> {
    let name = nul_string(&header[0..100])?;
    let prefix = nul_string(&header[345..500])?;
    if name.is_empty() {
        return Err(TarError::BadPath);
    }
    if prefix.is_empty() {
        Ok(PathBuf::from(name))
    } else {
        Ok(PathBuf::from(prefix).join(name))
    }
}

fn nul_string(bytes: &[u8]) -> Result<String, TarError> {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8(bytes[..end].to_vec()).map_err(|_| TarError::BadPath)
}

fn parse_pax(data: &[u8]) -> Result<Pax, TarError> {
    let mut pax = Pax::default();
    let mut i = 0usize;
    while i < data.len() {
        let Some(space_rel) = data[i..].iter().position(|b| *b == b' ') else {
            return Err(TarError::BadPax("missing length separator".into()));
        };
        let len_text = std::str::from_utf8(&data[i..i + space_rel])
            .map_err(|_| TarError::BadPax("non-utf8 length".into()))?;
        let len = len_text
            .parse::<usize>()
            .map_err(|_| TarError::BadPax("invalid length".into()))?;
        if len == 0 || i + len > data.len() {
            return Err(TarError::BadPax("record length out of bounds".into()));
        }
        let record = &data[i + space_rel + 1..i + len];
        if !record.ends_with(b"\n") {
            return Err(TarError::BadPax("record missing newline".into()));
        }
        let body = &record[..record.len() - 1];
        if let Some(eq) = body.iter().position(|b| *b == b'=') {
            let key = std::str::from_utf8(&body[..eq])
                .map_err(|_| TarError::BadPax("non-utf8 key".into()))?;
            let value = std::str::from_utf8(&body[eq + 1..])
                .map_err(|_| TarError::BadPax("non-utf8 value".into()))?;
            if key == "path" {
                pax.path = Some(PathBuf::from(value));
            }
        }
        i += len;
    }
    Ok(pax)
}

fn append_header(
    out: &mut Vec<u8>,
    path: &str,
    size: u64,
    mode: u64,
    mtime: u64,
    kind: u8,
) -> Result<(), TarError> {
    let mut header = [0u8; BLOCK];
    let path_bytes = path.as_bytes();
    if path_bytes.len() > 100 {
        return Err(TarError::BadPath);
    }
    header[0..path_bytes.len()].copy_from_slice(path_bytes);
    write_octal(&mut header[100..108], mode);
    write_octal(&mut header[108..116], 0);
    write_octal(&mut header[116..124], 0);
    write_octal(&mut header[124..136], size);
    write_octal(&mut header[136..148], mtime);
    header[148..156].fill(b' ');
    header[156] = kind;
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let sum = checksum(&header);
    write_checksum(&mut header[148..156], sum);
    out.extend_from_slice(&header);
    Ok(())
}

fn write_octal(field: &mut [u8], value: u64) {
    field.fill(0);
    let width = field.len() - 1;
    let s = format!("{value:0width$o}");
    let bytes = s.as_bytes();
    let start = width.saturating_sub(bytes.len());
    field[start..start + bytes.len()].copy_from_slice(bytes);
}

fn write_checksum(field: &mut [u8], value: u64) {
    field.fill(0);
    let s = format!("{value:06o}");
    field[..6].copy_from_slice(s.as_bytes());
    field[6] = 0;
    field[7] = b' ';
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_one_file_archive() {
        let tar = build_ustar_archive(&[("package/index.js", b"module.exports = 42;\n")]).unwrap();
        let entries = parse_archive(&tar).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, PathBuf::from("package/index.js"));
        assert_eq!(entries[0].kind, EntryKind::File);
        assert_eq!(entries[0].data, b"module.exports = 42;\n");
    }

    #[test]
    fn rejects_bad_checksum() {
        let mut tar = build_ustar_archive(&[("package/index.js", b"x")]).unwrap();
        tar[0] = b'X';
        assert!(matches!(
            parse_archive(&tar),
            Err(TarError::BadChecksum { .. })
        ));
    }

    #[test]
    fn rejects_truncated_entry() {
        let mut tar = build_ustar_archive(&[("package/index.js", b"abcdef")]).unwrap();
        tar.truncate(520);
        assert!(matches!(
            parse_archive(&tar),
            Err(TarError::TruncatedEntry { .. })
        ));
    }

    #[test]
    fn accepts_two_zero_block_end() {
        let tar = vec![0u8; BLOCK * 2];
        assert_eq!(parse_archive(&tar).unwrap(), Vec::<Entry>::new());
    }

    #[test]
    fn parses_directory_then_file() {
        let mut tar = Vec::new();
        append_header(&mut tar, "package/lib/", 0, 0o755, 0, b'5').unwrap();
        append_header(&mut tar, "package/lib/index.js", 2, 0o644, 0, b'0').unwrap();
        tar.extend_from_slice(b"ok");
        pad_block(&mut tar, 2);
        tar.extend_from_slice(&[0u8; BLOCK * 2]);

        let entries = parse_archive(&tar).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, PathBuf::from("package/lib/"));
        assert_eq!(entries[0].kind, EntryKind::Directory);
        assert_eq!(entries[1].path, PathBuf::from("package/lib/index.js"));
        assert_eq!(entries[1].kind, EntryKind::File);
        assert_eq!(entries[1].data, b"ok");
    }

    #[test]
    fn applies_pax_path_to_following_entry() {
        let pax_body = pax_record("path=package/very/long/path.js");
        let mut tar = Vec::new();
        append_header(&mut tar, "pax", pax_body.len() as u64, 0o644, 0, b'x').unwrap();
        tar.extend_from_slice(pax_body.as_bytes());
        pad_block(&mut tar, pax_body.len());
        append_header(&mut tar, "ignored", 2, 0o644, 0, b'0').unwrap();
        tar.extend_from_slice(b"ok");
        pad_block(&mut tar, 2);
        tar.extend_from_slice(&[0u8; BLOCK * 2]);

        let entries = parse_archive(&tar).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, PathBuf::from("package/very/long/path.js"));
        assert_eq!(entries[0].data, b"ok");
    }

    #[test]
    fn sanitizes_relative_paths_and_rejects_traversal() {
        assert_eq!(
            sanitize_entry_path(Path::new("package/lib/index.js")).unwrap(),
            PathBuf::from("package/lib/index.js")
        );
        assert!(matches!(
            sanitize_entry_path(Path::new("../evil.js")),
            Err(TarError::UnsafePath(_))
        ));
        assert!(matches!(
            sanitize_entry_path(Path::new("package/../evil.js")),
            Err(TarError::UnsafePath(_))
        ));
        assert!(matches!(
            sanitize_entry_path(Path::new("/etc/passwd")),
            Err(TarError::UnsafePath(_))
        ));
        assert!(matches!(
            sanitize_entry_path(Path::new("C:/temp/evil.js")),
            Err(TarError::UnsafePath(_))
        ));
        assert!(matches!(
            sanitize_entry_path(Path::new(r"package\evil.js")),
            Err(TarError::UnsafePath(_))
        ));
    }

    #[test]
    fn safe_extract_rejects_traversal_from_real_archive() {
        let tar = build_ustar_archive(&[("../evil.js", b"bad")]).unwrap();
        let mut dest = std::env::temp_dir();
        dest.push(format!("rusty-js-tar-traversal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(&dest).unwrap();

        assert!(matches!(
            extract_archive(&tar, &dest),
            Err(TarError::UnsafePath(_))
        ));
        assert!(!dest.join("evil.js").exists());
        let _ = std::fs::remove_dir_all(dest);
    }

    fn pax_record(body: &str) -> String {
        let mut len = body.len() + 3;
        loop {
            let record = format!("{len} {body}\n");
            if record.len() == len {
                return record;
            }
            len = record.len();
        }
    }
}

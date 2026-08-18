
pub mod multipart;
pub use multipart::MultipartError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormDataEntryValue {
    String(String),
    File {
        name: String,
        content_type: String,
        bytes: Vec<u8>,
    },
}

impl FormDataEntryValue {

    pub fn filename(&self) -> Option<&str> {
        match self {
            FormDataEntryValue::String(_) => None,
            FormDataEntryValue::File { name, .. } => Some(name),
        }
    }

    pub fn is_file(&self) -> bool {
        matches!(self, FormDataEntryValue::File { .. })
    }

    pub fn as_str_value(&self) -> &str {
        match self {
            FormDataEntryValue::String(s) => s,
            FormDataEntryValue::File { name, .. } => name,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            FormDataEntryValue::String(s) => s.as_bytes(),
            FormDataEntryValue::File { bytes, .. } => bytes,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FormData {
    entries: Vec<(String, FormDataEntryValue)>,
}

impl FormData {

    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&mut self, name: &str, value: &str) {
        self.entries.push((
            name.to_string(),
            FormDataEntryValue::String(value.to_string()),
        ));
    }

    pub fn append_file(
        &mut self,
        name: &str,
        bytes: Vec<u8>,
        content_type: &str,
        filename: Option<&str>,
    ) {
        self.entries.push((
            name.to_string(),
            FormDataEntryValue::File {
                name: filename.unwrap_or("blob").to_string(),
                content_type: content_type.to_string(),
                bytes,
            },
        ));
    }

    pub fn get(&self, name: &str) -> Option<&FormDataEntryValue> {
        self.entries.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    pub fn get_all(&self, name: &str) -> Vec<&FormDataEntryValue> {
        self.entries
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v)
            .collect()
    }

    pub fn has(&self, name: &str) -> bool {
        self.entries.iter().any(|(n, _)| n == name)
    }

    pub fn set(&mut self, name: &str, value: &str) {
        self.set_entry(name, FormDataEntryValue::String(value.to_string()));
    }

    pub fn set_file(
        &mut self,
        name: &str,
        bytes: Vec<u8>,
        content_type: &str,
        filename: Option<&str>,
    ) {
        self.set_entry(
            name,
            FormDataEntryValue::File {
                name: filename.unwrap_or("blob").to_string(),
                content_type: content_type.to_string(),
                bytes,
            },
        );
    }

    fn set_entry(&mut self, name: &str, value: FormDataEntryValue) {
        let mut placed = false;
        let mut out: Vec<(String, FormDataEntryValue)> = Vec::with_capacity(self.entries.len());
        for (n, v) in self.entries.drain(..) {
            if n == name {
                if !placed {
                    out.push((name.to_string(), value.clone()));
                    placed = true;
                }

            } else {
                out.push((n, v));
            }
        }
        if !placed {
            out.push((name.to_string(), value));
        }
        self.entries = out;
    }

    pub fn delete(&mut self, name: &str) {
        self.entries.retain(|(n, _)| n != name);
    }

    pub fn entries(&self) -> impl Iterator<Item = (&str, &FormDataEntryValue)> {
        self.entries.iter().map(|(n, v)| (n.as_str(), v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(n, _)| n.as_str())
    }

    pub fn values(&self) -> impl Iterator<Item = &FormDataEntryValue> {
        self.entries.iter().map(|(_, v)| v)
    }

    pub fn for_each<F: FnMut(&str, &FormDataEntryValue)>(&self, mut f: F) {
        for (n, v) in &self.entries {
            f(n, v);
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

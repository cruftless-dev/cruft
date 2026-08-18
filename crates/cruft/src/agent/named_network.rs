#[derive(Clone)]
pub(crate) struct AgentOsvFixture {
    pub(crate) json: String,
}

pub(crate) fn agent_load_osv_fixture(path: &str) -> Result<String, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read OSV fixture {path}: {e}"))?;
    let value = serde_json::from_str::<serde_json::Value>(&source)
        .map_err(|e| format!("cannot parse OSV fixture {path}: {e}"))?;
    let Some(object) = value.as_object() else {
        return Err(format!("OSV fixture {path} must be a JSON object"));
    };
    let Some(queries) = object.get("queries").and_then(|v| v.as_array()) else {
        return Err(format!("OSV fixture {path} must contain queries array"));
    };
    for query in queries {
        let Some(query) = query.as_object() else {
            return Err(format!(
                "OSV fixture {path} queries entries must be objects"
            ));
        };
        let package = query
            .get("package")
            .and_then(|v| v.as_object())
            .ok_or_else(|| format!("OSV fixture {path} query package must be an object"))?;
        for key in ["ecosystem", "name"] {
            if package
                .get(key)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .is_none()
            {
                return Err(format!(
                    "OSV fixture {path} query package.{key} must be a non-empty string"
                ));
            }
        }
        if let Some(version) = query.get("version") {
            if version.as_str().filter(|s| !s.is_empty()).is_none() {
                return Err(format!(
                    "OSV fixture {path} query version must be a non-empty string"
                ));
            }
        }
        if query.get("response").and_then(|v| v.as_object()).is_none() {
            return Err(format!(
                "OSV fixture {path} query response must be an object"
            ));
        }
    }
    Ok(value.to_string())
}

pub(crate) fn agent_osv_fixture_js(fixture: Option<&str>) -> String {
    fixture.unwrap_or("null").to_string()
}

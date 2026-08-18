#[derive(Clone)]
pub(crate) struct AgentModelFixture {
    pub(crate) json: String,
}

pub(crate) fn agent_load_model_fixture(path: &str) -> Result<String, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read model fixture {path}: {e}"))?;
    let value = serde_json::from_str::<serde_json::Value>(&source)
        .map_err(|e| format!("cannot parse model fixture {path}: {e}"))?;
    let Some(object) = value.as_object() else {
        return Err(format!("model fixture {path} must be a JSON object"));
    };
    let Some(responses) = object.get("responses").and_then(|v| v.as_array()) else {
        return Err(format!("model fixture {path} must contain responses array"));
    };
    for response in responses {
        let Some(response) = response.as_object() else {
            return Err(format!(
                "model fixture {path} responses entries must be objects"
            ));
        };
        if response
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Err(format!(
                "model fixture {path} response id must be a non-empty string"
            ));
        }
        if let Some(model) = response.get("model") {
            if model.as_str().filter(|s| !s.is_empty()).is_none() {
                return Err(format!(
                    "model fixture {path} response model must be a non-empty string"
                ));
            }
        }
        if response
            .get("response")
            .and_then(|v| v.as_object())
            .is_none()
        {
            return Err(format!(
                "model fixture {path} response response must be an object"
            ));
        }
    }
    Ok(value.to_string())
}

pub(crate) fn agent_model_fixture_js(fixture: Option<&str>) -> String {
    fixture.unwrap_or("null").to_string()
}

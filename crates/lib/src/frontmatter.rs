use serde_yaml::Value;
use std::collections::HashMap;

pub fn parse_frontmatter(content: &str) -> Option<HashMap<String, Value>> {
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 || !parts[0].trim().is_empty() {
        return None;
    }
    serde_yaml::from_str(parts[1]).ok()
}

pub fn extract_tags(frontmatter: &HashMap<String, Value>) -> Vec<String> {
    frontmatter
        .get("tags")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

use std::collections::HashMap;

use crate::errors::*;
use crate::http;
use anyhow::Context;
use reqwest::Url;
use serde::{Deserialize, Serialize};

const COMMUNITY_URL: &str =
    "https://raw.githubusercontent.com/kpcyrd/rebuilderd-community/refs/heads/main/README.md";

#[derive(Debug, Clone)]
pub struct Selectable<T> {
    pub active: bool,
    pub item: T,
}

impl<T: Clone> From<Selectable<&T>> for Selectable<T> {
    fn from(selectable: Selectable<&T>) -> Self {
        Selectable {
            active: selectable.active,
            item: selectable.item.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rebuilder {
    pub name: String,
    pub url: Url,
    pub distributions: Vec<String>,
    pub country: Option<String>,
    pub contact: Option<String>,
}

impl Rebuilder {
    pub fn reconfigure(&mut self, name: Option<String>) {
        if let Some(name) = name {
            self.name = name;
        }
    }
}

pub async fn fetch_rebuilderd_community() -> Result<Vec<Rebuilder>> {
    // TODO: request timeouts
    let http = http::client();
    let response = http
        .get(COMMUNITY_URL)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    parse(response.as_str())
}

fn parse(text: &str) -> Result<Vec<Rebuilder>> {
    let mut start = None;
    let mut end = None;

    for (idx, line) in text.lines().enumerate() {
        if line.starts_with("```") {
            if start.is_none() {
                start = Some(idx + 1);
            } else if end.is_none() {
                end = Some(idx);
                break;
            }
        }
    }

    let start_line = start.context("Failed to find start of TOML data")?;
    let end_line = end.context("Failed to find end of TOML data")?;

    // Extract the lines between start and end
    let toml_content: Vec<&str> = text
        .lines()
        .skip(start_line)
        .take(end_line - start_line)
        .collect();
    let toml_str = toml_content.join("\n");

    let mut list = toml::from_str::<HashMap<String, Vec<Rebuilder>>>(&toml_str)?;
    let list = list.remove("rebuilder").unwrap_or_default();
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let data = r#"# Rebuilderd Community Rebuilders

this is
`some text`

```toml
[[rebuilder]]
name = "Rebuilder One"
url = "https://one.example.com"
distributions = ["archlinux"]
country = "DEU"
contact = "Hello!"

[[rebuilder]]
name = "Rebuilder Two"
url = "https://two.example.com"
distributions = ["archlinux", "debian"]
```

"#;
        let rebuilders = parse(data).unwrap();
        assert_eq!(
            rebuilders,
            &[
                Rebuilder {
                    name: "Rebuilder One".to_string(),
                    url: "https://one.example.com".parse().unwrap(),
                    distributions: vec!["archlinux".to_string()],
                    country: Some("DEU".to_string()),
                    contact: Some("Hello!".to_string()),
                },
                Rebuilder {
                    name: "Rebuilder Two".to_string(),
                    url: "https://two.example.com".parse().unwrap(),
                    distributions: vec!["archlinux".to_string(), "debian".to_string()],
                    country: None,
                    contact: None,
                },
            ]
        );
    }

    #[test]
    fn test_parse_empty() {
        let data = "```\n```";
        let list = parse(data).unwrap();
        assert_eq!(list, &[]);
    }

    #[test]
    fn test_parse_fully_empty() {
        let data = "";
        let list = parse(data);
        assert!(list.is_err());
    }
}

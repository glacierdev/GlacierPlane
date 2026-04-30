use serde::Deserialize;
use serde_json::Value;

use crate::error::AppError;

#[derive(Clone, Default)]
pub struct Parser;

#[derive(Deserialize)]
struct PipelineConfig {
    #[serde(default)]
    steps: Vec<Value>,
}

impl Parser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, config: &[u8]) -> Result<Vec<Value>, AppError> {
        let pipeline: PipelineConfig = serde_yaml::from_slice(config)?;
        Ok(pipeline.steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_pipeline() {
        let parser = Parser::new();
        let yaml = b"steps:\n  - command: echo hello\n  - command: echo world";
        let steps = parser.parse(yaml).unwrap();
        assert_eq!(steps.len(), 2);
    }

    #[test]
    fn parse_pipeline_with_wait() {
        let parser = Parser::new();
        let yaml = b"steps:\n  - command: echo 1\n  - wait\n  - command: echo 2";
        let steps = parser.parse(yaml).unwrap();
        assert_eq!(steps.len(), 3);
    }

    #[test]
    fn parse_pipeline_with_depends_on() {
        let parser = Parser::new();
        let yaml = b"steps:\n  - key: build\n    command: cargo build\n  - key: test\n    depends_on: build\n    command: cargo test";
        let steps = parser.parse(yaml).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(
            steps[1].get("depends_on").unwrap().as_str().unwrap(),
            "build"
        );
    }

    #[test]
    fn parse_empty_steps() {
        let parser = Parser::new();
        let yaml = b"steps: []";
        let steps = parser.parse(yaml).unwrap();
        assert!(steps.is_empty());
    }

    #[test]
    fn parse_invalid_yaml_returns_error() {
        let parser = Parser::new();
        let yaml = b"{{invalid yaml:";
        assert!(parser.parse(yaml).is_err());
    }

    #[test]
    fn parse_pipeline_with_agents() {
        let parser = Parser::new();
        let yaml =
            b"steps:\n  - command: echo hello\n    agents:\n      queue: default\n      os: linux";
        let steps = parser.parse(yaml).unwrap();
        assert_eq!(steps.len(), 1);
        let agents = steps[0].get("agents").unwrap();
        assert_eq!(agents.get("queue").unwrap().as_str().unwrap(), "default");
        assert_eq!(agents.get("os").unwrap().as_str().unwrap(), "linux");
    }

    #[test]
    fn parse_pipeline_with_label_and_key() {
        let parser = Parser::new();
        let yaml = b"steps:\n  - label: \":rust: build\"\n    key: build\n    command: cargo build";
        let steps = parser.parse(yaml).unwrap();
        assert_eq!(
            steps[0].get("label").unwrap().as_str().unwrap(),
            ":rust: build"
        );
        assert_eq!(steps[0].get("key").unwrap().as_str().unwrap(), "build");
    }

    #[test]
    fn parse_pipeline_with_timeout() {
        let parser = Parser::new();
        let yaml = b"steps:\n  - command: cargo test\n    timeout_in_minutes: 10";
        let steps = parser.parse(yaml).unwrap();
        assert_eq!(
            steps[0]
                .get("timeout_in_minutes")
                .unwrap()
                .as_i64()
                .unwrap(),
            10
        );
    }

    #[test]
    fn parse_no_steps_key() {
        let parser = Parser::new();
        let yaml = b"something_else: true";
        let steps = parser.parse(yaml).unwrap();
        assert!(steps.is_empty());
    }
}

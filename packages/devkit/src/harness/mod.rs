pub mod scenarios;

use crate::harness::scenarios::{scenario_source, ScenarioFixture, SCENARIO_NAMES};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessError {
    UnknownScenario { requested: String },
    MalformedScenario {
        scenario: &'static str,
        message: String,
    },
}

impl std::fmt::Display for HarnessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownScenario { requested } => {
                write!(formatter, "unknown scenario: {requested}")
            }
            Self::MalformedScenario { scenario, message } => {
                write!(formatter, "scenario {scenario} could not be parsed: {message}")
            }
        }
    }
}

impl std::error::Error for HarnessError {}

pub fn load_scenario(name: &'static str) -> Result<ScenarioFixture, HarnessError> {
    let source = scenario_source(name).ok_or_else(|| HarnessError::UnknownScenario {
        requested: name.to_string(),
    })?;
    serde_json::from_str(source).map_err(|err| HarnessError::MalformedScenario {
        scenario: name,
        message: err.to_string(),
    })
}

pub fn load_all() -> Result<Vec<ScenarioFixture>, HarnessError> {
    SCENARIO_NAMES
        .iter()
        .map(|&name| load_scenario(name))
        .collect()
}

pub fn scenario_names() -> &'static [&'static str; 5] {
    &SCENARIO_NAMES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_a_known_scenario_succeeds() {
        let scenario = load_scenario("congestion").expect("scenario");
        assert_eq!(scenario.name, "congestion");
        assert_eq!(scenario.records.len(), 8);
    }

    #[test]
    fn loading_an_unknown_scenario_fails() {
        assert_eq!(
            load_scenario("meltdown").err(),
            Some(HarnessError::UnknownScenario {
                requested: "meltdown".to_string()
            })
        );
    }

    #[test]
    fn every_scenario_loads() {
        assert_eq!(load_all().expect("all scenarios load").len(), SCENARIO_NAMES.len());
    }

    #[test]
    fn loading_twice_returns_equal_fixtures() {
        assert_eq!(load_scenario("spike"), load_scenario("spike"));
    }

    #[test]
    fn the_error_messages_describe_the_failure() {
        assert_eq!(
            HarnessError::UnknownScenario {
                requested: "meltdown".to_string()
            }
            .to_string(),
            "unknown scenario: meltdown"
        );
        assert_eq!(
            HarnessError::MalformedScenario {
                scenario: "spike",
                message: "EOF while parsing".to_string(),
            }
            .to_string(),
            "scenario spike could not be parsed: EOF while parsing"
        );
    }
}

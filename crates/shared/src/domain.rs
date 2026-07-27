use serde::{Deserialize, Serialize};

/// Geographic jurisdiction for a tracked source or normalized policy record.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    UnitedStates,
    Canada,
    Global,
}

impl Region {
    /// Returns the stable code used in external contracts and future persistence.
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::UnitedStates => "us",
            Self::Canada => "ca",
            Self::Global => "global",
        }
    }

    /// Parses a persisted region code without accepting aliases.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "us" => Some(Self::UnitedStates),
            "ca" => Some(Self::Canada),
            "global" => Some(Self::Global),
            _ => None,
        }
    }
}

/// Distinguishes government-policy sources from AI-news sources.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceCategory {
    Policy,
    News,
}

impl SourceCategory {
    /// Returns the stable database and API code for this category.
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::Policy => "policy",
            Self::News => "news",
        }
    }

    /// Parses a persisted category code without accepting aliases.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "policy" => Some(Self::Policy),
            "news" => Some(Self::News),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Region, SourceCategory};

    #[test]
    fn region_codes_are_stable() {
        assert_eq!(Region::UnitedStates.as_code(), "us");
        assert_eq!(Region::Canada.as_code(), "ca");
        assert_eq!(Region::Global.as_code(), "global");
        assert_eq!(Region::from_code("global"), Some(Region::Global));
        assert_eq!(Region::from_code("unknown"), None);
    }

    #[test]
    fn source_category_codes_are_stable() {
        assert_eq!(SourceCategory::Policy.as_code(), "policy");
        assert_eq!(SourceCategory::News.as_code(), "news");
        assert_eq!(SourceCategory::from_code("news"), Some(SourceCategory::News));
        assert_eq!(SourceCategory::from_code("unknown"), None);
    }
}

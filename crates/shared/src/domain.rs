use serde::{Deserialize, Serialize};

/// Geographic jurisdiction for a tracked source or normalized policy record.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    UnitedStates,
    Canada,
}

impl Region {
    /// Returns the stable code used in external contracts and future persistence.
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::UnitedStates => "us",
            Self::Canada => "ca",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Region;

    #[test]
    fn region_codes_are_stable() {
        assert_eq!(Region::UnitedStates.as_code(), "us");
        assert_eq!(Region::Canada.as_code(), "ca");
    }
}

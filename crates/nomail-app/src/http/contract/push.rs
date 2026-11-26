use crate::push::Preferences;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct PushPreferences {
    pub low: bool,
    pub normal: bool,
    pub high: bool,
    pub urgent: bool,
}

impl From<Preferences> for PushPreferences {
    fn from(p: Preferences) -> Self {
        Self {
            low: p.low,
            normal: p.normal,
            high: p.high,
            urgent: p.urgent,
        }
    }
}

impl From<PushPreferences> for Preferences {
    fn from(p: PushPreferences) -> Self {
        Self {
            low: p.low,
            normal: p.normal,
            high: p.high,
            urgent: p.urgent,
        }
    }
}

//! Tenant usage: activity metrics and asset type counts.

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The spec types every count as a JSON number (double); they are whole
/// numbers in practice, and pcli2 prints them as integers.
fn to_count(value: f64) -> u64 {
    value.max(0.0).round() as u64
}

fn deserialize_count<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    f64::deserialize(deserializer).map(to_count)
}

/// Named counts, in the order the server sent them and under the names it
/// uses (`text`, `DUPLICATION`, `folder_browse`, `model`, ...).
///
/// A plain map would sort the names; a struct would drop a name Physna adds
/// later. The server also leaves out asset types the tenant has not enabled.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Counts(pub Vec<(String, u64)>);

impl Serialize for Counts {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (name, count) in &self.0 {
            map.serialize_entry(name, count)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Counts {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CountsVisitor;

        impl<'de> Visitor<'de> for CountsVisitor {
            type Value = Counts;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object of counts")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Counts, A::Error> {
                let mut counts = Vec::new();
                while let Some((name, value)) = map.next_entry::<String, f64>()? {
                    counts.push((name, to_count(value)));
                }
                Ok(Counts(counts))
            }
        }

        deserializer.deserialize_map(CountsVisitor)
    }
}

/// One UTC day of activity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyActivity {
    /// `YYYY-MM-DD`
    pub date: String,
    #[serde(deserialize_with = "deserialize_count")]
    pub searches: u64,
    #[serde(deserialize_with = "deserialize_count")]
    pub compares: u64,
    #[serde(deserialize_with = "deserialize_count")]
    pub downloads: u64,
    #[serde(deserialize_with = "deserialize_count")]
    pub uploads: u64,
    #[serde(deserialize_with = "deserialize_count")]
    pub reports: u64,
    #[serde(deserialize_with = "deserialize_count")]
    pub active_users: u64,
}

/// `GET /tenants/{tenantId}/activity-metrics?from&to`: how much the tenant
/// used Physna between two UTC days, inclusive. Tenant admins only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityMetrics {
    #[serde(deserialize_with = "deserialize_count")]
    pub searches: u64,
    #[serde(deserialize_with = "deserialize_count")]
    pub compares: u64,
    #[serde(deserialize_with = "deserialize_count")]
    pub downloads: u64,
    /// Uploaded by users; demo assets are not counted.
    #[serde(deserialize_with = "deserialize_count")]
    pub uploads: u64,
    #[serde(deserialize_with = "deserialize_count")]
    pub reports: u64,
    /// Distinct users with at least one authenticated request.
    #[serde(deserialize_with = "deserialize_count")]
    pub active_users: u64,
    /// How many of `active_users` have a Physna email address.
    #[serde(deserialize_with = "deserialize_count")]
    pub internal_active_users: u64,
    pub searches_by_type: Counts,
    pub reports_by_type: Counts,
    pub feature_usage: Counts,
    /// One entry per day, oldest first, zeros included.
    pub daily: Vec<DailyActivity>,
}

/// The `tenant usage` output: the period, the activity in it, and how many
/// assets of each type the tenant holds now.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TenantUsage {
    pub from: String,
    pub to: String,
    #[serde(flatten)]
    pub activity: ActivityMetrics,
    /// `GET /tenants/{tenantId}/assets/type-counts`; not tied to the period.
    pub asset_types: Counts,
}

/// The `tenant usage --daily` output: one row per day.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct DailyUsage(pub Vec<DailyActivity>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_keep_the_server_order_and_unknown_names() {
        let counts: Counts =
            serde_json::from_str(r#"{"text":49,"visual":98.0,"hologram":1,"part":-2}"#).unwrap();
        assert_eq!(
            counts.0,
            vec![
                ("text".to_string(), 49),
                ("visual".to_string(), 98),
                ("hologram".to_string(), 1),
                ("part".to_string(), 0),
            ]
        );
        assert_eq!(
            serde_json::to_string(&counts).unwrap(),
            r#"{"text":49,"visual":98,"hologram":1,"part":0}"#
        );
    }
}

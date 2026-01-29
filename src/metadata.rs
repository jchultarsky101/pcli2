use csv::Reader;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use tracing::{debug, trace};

/// Represents a metadata entry from the CSV file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEntry {
    /// The path of the asset to update
    #[serde(rename = "ASSET_PATH")]
    pub asset_path: String,
    /// The name of the metadata field
    #[serde(rename = "NAME")]
    pub name: String,
    /// The value of the metadata field
    #[serde(rename = "VALUE")]
    pub value: String,
}

impl MetadataEntry {
    /// Create a new MetadataEntry
    pub fn new(asset_path: String, name: String, value: String) -> Self {
        Self {
            asset_path,
            name,
            value,
        }
    }
}

/// Read metadata entries from a CSV file
///
/// # Arguments
/// * `file_path` - Path to the CSV file
///
/// # Returns
/// * `Ok(Vec<MetadataEntry>)` - Vector of metadata entries sorted by asset path
/// * `Err(csv::Error)` - Error reading or parsing the CSV file
pub fn read_metadata_from_csv(file_path: &str) -> Result<Vec<MetadataEntry>, csv::Error> {
    debug!("Reading metadata from CSV file: {}", file_path);

    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let mut csv_reader = Reader::from_reader(reader);

    let mut entries: Vec<MetadataEntry> = Vec::new();

    // Read each record from the CSV
    for result in csv_reader.deserialize() {
        let record: MetadataEntry = result?;
        trace!("Read metadata entry: {:?}", record);
        entries.push(record);
    }

    // Sort entries by asset path
    entries.sort_by(|a, b| a.asset_path.cmp(&b.asset_path));

    debug!(
        "Successfully read {} metadata entries from CSV",
        entries.len()
    );
    Ok(entries)
}

/// Group metadata entries by asset path
///
/// # Arguments
/// * `entries` - Vector of metadata entries
///
/// # Returns
/// * `HashMap<String, HashMap<String, String>>` - Map of asset paths to metadata key-value pairs
pub fn group_metadata_by_asset(
    entries: Vec<MetadataEntry>,
) -> HashMap<String, HashMap<String, String>> {
    let mut grouped: HashMap<String, HashMap<String, String>> = HashMap::new();

    for entry in entries {
        // Get or create the metadata map for this asset path
        let metadata_map = grouped.entry(entry.asset_path).or_default();

        // Add the metadata key-value pair
        metadata_map.insert(entry.name, entry.value);
    }

    grouped
}

/// Convert metadata string values to appropriate JSON types
///
/// This function converts string metadata values to appropriate JSON types (string, number, boolean)
/// based on the provided type information. For now, it treats all values as strings for compatibility.
///
/// # Arguments
/// * `metadata` - Map of metadata key-value pairs as strings
///
/// # Returns
/// * `HashMap<String, Value>` - Map of metadata key-value pairs as JSON values
pub fn convert_metadata_to_json_values(
    metadata: &HashMap<String, String>,
) -> HashMap<String, Value> {
    let mut json_metadata: HashMap<String, Value> = HashMap::new();

    for (key, value) in metadata {
        // Sanitize the value to remove or replace problematic characters
        let sanitized_value = sanitize_metadata_value(value);

        // For now, treat all values as strings to avoid type mismatch issues
        // The API might be expecting string values for all metadata fields
        json_metadata.insert(key.clone(), Value::String(sanitized_value));
    }

    json_metadata
}

/// Convert a single metadata value to appropriate JSON type based on the specified meta-type
///
/// This function converts a single metadata value to the appropriate JSON type based on the
/// provided metadata type (text, number, boolean), with proper sanitization.
///
/// # Arguments
/// * `name` - The metadata property name
/// * `value` - The metadata property value as string
/// * `metadata_type` - The expected type ("text", "number", "boolean")
///
/// # Returns
/// * `Value` - The converted JSON value
pub fn convert_single_metadata_to_json_value(
    _name: &str,
    value: &str,
    metadata_type: &str,
) -> Value {
    match metadata_type {
        "number" => {
            // Try to parse as number (integer or float)
            if let Ok(int_val) = value.parse::<i64>() {
                serde_json::Value::Number(serde_json::Number::from(int_val))
            } else if let Ok(float_val) = value.parse::<f64>() {
                if float_val.fract() == 0.0 {
                    serde_json::Value::Number(serde_json::Number::from(float_val as i64))
                } else {
                    serde_json::Number::from_f64(float_val)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::String(value.to_string()))
                }
            } else {
                serde_json::Value::String(value.to_string())
            }
        }
        "boolean" => {
            let bool_val = match value.to_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => true,
                "false" | "0" | "no" | "off" => false,
                _ => {
                    // Try to parse as boolean string
                    value.parse::<bool>().unwrap_or(false)
                }
            };
            serde_json::Value::Bool(bool_val)
        }
        "text" | _ => {
            // Default to text/string type, with sanitization
            let sanitized_value = sanitize_metadata_value(value);
            serde_json::Value::String(sanitized_value)
        }
    }
}

/// Sanitize metadata values to handle special characters that might cause API issues
///
/// This function replaces or removes special characters that are known to cause issues
/// with the Physna API, such as Unicode symbols that might not be properly encoded.
///
/// # Arguments
/// * `value` - The metadata value to sanitize
///
/// # Returns
/// * `String` - The sanitized metadata value
fn sanitize_metadata_value(value: &str) -> String {
    value
        // Replace special Unicode characters with ASCII equivalents
        .replace('Ø', "O") // Diameter symbol
        .replace('°', " deg") // Degree symbol
        .replace('″', "\"") // Double prime (inch symbol)
        .replace("…", "...") // Ellipsis
        // Keep other characters as they are
        .to_string()
}

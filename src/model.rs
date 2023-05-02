use crate::format::{
    CsvRecordProducer, FormattingError, JsonProducer, OutputFormat, OutputFormatter,
};
use csv::Writer;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::BufWriter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("missing property value {name:?}")]
    MissingPropertyValue { name: String },
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Folder {
    id: u32,
    name: String,
}

impl Folder {
    pub fn new(id: u32, name: String) -> Folder {
        Folder { id, name }
    }

    #[allow(dead_code)]
    pub fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    #[allow(dead_code)]
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn builder() -> FolderBuilder {
        FolderBuilder::new()
    }
}

impl CsvRecordProducer for Folder {
    fn csv_header() -> Vec<String> {
        vec!["ID".to_string(), "NAME".to_string()]
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        vec![vec![self.id().to_string(), self.name()]]
    }
}

impl JsonProducer for Folder {}

impl OutputFormatter for Folder {
    type Item = Folder;

    fn format(&self, format: OutputFormat) -> Result<String, FormattingError> {
        match format {
            OutputFormat::Json => Ok(self.to_json()?),
            OutputFormat::Csv => Ok(self.to_csv_with_header()?),
        }
    }
}

pub struct FolderBuilder {
    id: Option<u32>,
    name: Option<String>,
}

impl FolderBuilder {
    fn new() -> FolderBuilder {
        FolderBuilder {
            id: None,
            name: None,
        }
    }

    pub fn id(&mut self, id: u32) -> &mut FolderBuilder {
        self.id = Some(id);
        self
    }

    pub fn name(&mut self, name: &String) -> &mut FolderBuilder {
        self.name = Some(name.clone());
        self
    }

    pub fn build(&self) -> Result<Folder, ModelError> {
        let id = match &self.id {
            Some(id) => id.clone(),
            None => {
                return Err(ModelError::MissingPropertyValue {
                    name: "id".to_string(),
                })
            }
        };

        let name = match &self.name {
            Some(name) => name.clone(),
            None => {
                return Err(ModelError::MissingPropertyValue {
                    name: "name".to_string(),
                })
            }
        };

        Ok(Folder::new(id, name.clone()))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderList {
    folders: HashMap<u32, Folder>,
}

impl FolderList {
    pub fn empty() -> FolderList {
        FolderList {
            folders: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.folders.is_empty()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.folders.len()
    }

    pub fn insert(&mut self, folder: Folder) {
        self.folders.insert(folder.id, folder.clone());
    }

    #[allow(dead_code)]
    pub fn remove(&mut self, id: &u32) {
        self.folders.remove(id);
    }

    #[allow(dead_code)]
    pub fn get(&self, id: &u32) -> Option<&Folder> {
        self.folders.get(id)
    }

    #[allow(dead_code)]
    pub fn find_by_name(&self, name: &String) -> Option<&Folder> {
        let result = self.folders.iter().find(|(_, f)| f.name.eq(name));

        match result {
            Some((_key, folder)) => Some(folder),
            None => None,
        }
    }
}

impl Default for FolderList {
    fn default() -> Self {
        FolderList::empty()
    }
}

impl FromIterator<Folder> for FolderList {
    fn from_iter<I: IntoIterator<Item = Folder>>(iter: I) -> FolderList {
        let mut folders = FolderList::empty();
        for f in iter {
            folders.insert(f);
        }

        folders
    }
}

impl CsvRecordProducer for FolderList {
    fn csv_header() -> Vec<String> {
        Folder::csv_header()
    }

    fn as_csv_records(&self) -> Vec<Vec<String>> {
        let mut records: Vec<Vec<String>> = Vec::new();

        for (_, folder) in &self.folders {
            records.push(folder.as_csv_records()[0].clone());
        }

        records
    }
}

impl OutputFormatter for FolderList {
    type Item = FolderList;

    fn format(&self, format: OutputFormat) -> Result<String, FormattingError> {
        match format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(self);
                match json {
                    Ok(json) => Ok(json),
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
            OutputFormat::Csv => {
                let buf = BufWriter::new(Vec::new());
                let mut wtr = Writer::from_writer(buf);
                wtr.write_record(&Self::csv_header()).unwrap();
                for record in self.as_csv_records() {
                    wtr.write_record(&record).unwrap();
                }
                match wtr.flush() {
                    Ok(_) => {
                        let bytes = wtr.into_inner().unwrap().into_inner().unwrap();
                        let csv = String::from_utf8(bytes).unwrap();
                        Ok(csv.clone())
                    }
                    Err(e) => Err(FormattingError::FormatFailure { cause: Box::new(e) }),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_folder_creation() {
        let id: u32 = 100;
        let name: String = "some_folder_name".to_string();

        let folder = Folder::new(id, name.clone());
        assert_eq!(id, folder.id());
        assert_eq!(name, folder.name());
    }

    #[test]
    fn test_folder_builder() {
        let id: u32 = 110;
        let name: String = "some_other_name".to_string();

        let folder = Folder::builder().id(id).name(&name).build().unwrap();
        assert_eq!(id, folder.id());
        assert_eq!(name, folder.name());
    }

    #[test]
    fn test_output_format() {
        let id: u32 = 120;
        let name: String = "folder_name".to_string();

        let folder = Folder::builder().id(id).name(&name).build().unwrap();
        let json = folder.format(OutputFormat::Json).unwrap();
        let json_expected = r#"{
  "id": 120,
  "name": "folder_name"
}"#;
        assert_eq!(json_expected, json);

        let csv = folder.format(OutputFormat::Csv).unwrap();
        let csv_expected = r#"ID,NAME
120,folder_name
"#;
        assert_eq!(csv_expected, csv);
    }
}

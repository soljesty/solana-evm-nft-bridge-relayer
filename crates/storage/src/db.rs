use log::trace;
use rocksdb::{Options, DB};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Arc};

use crate::errors::DbError;

#[derive(Clone, Debug)]
pub struct Database {
    db: Arc<DB>,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let path_str = path
            .as_ref()
            .to_str()
            .ok_or_else(|| DbError::InvalidPath(format!("{:?}", path.as_ref())))?;

        let mut opts = Options::default();
        opts.create_if_missing(true);

        let db = DB::open(&opts, path_str).map_err(|e| DbError::RocksDb(e.to_string()))?;
        Ok(Self { db: Arc::new(db) })
    }

    pub fn write_value<K: AsRef<[u8]>, V: Serialize>(
        &self,
        key: K,
        value: &V,
    ) -> Result<(), DbError> {
        let serialized =
            serde_json::to_string(value).map_err(|e| DbError::Serialization(e.to_string()))?;

        trace!("Value to write {}", serialized);

        self.db
            .put(key, serialized)
            .map_err(|e| DbError::WriteDb(e.to_string()))?;
        Ok(())
    }

    pub fn read<K: AsRef<[u8]>, V: for<'a> Deserialize<'a>>(
        &self,
        key: K,
    ) -> Result<Option<V>, DbError> {
        if let Some(bytes) = self
            .db
            .get(key)
            .map_err(|e| DbError::WriteDb(e.to_string()))?
        {
            let value: V =
                serde_json::from_slice(&bytes).map_err(|e| DbError::ReadDb(e.to_string()))?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod db_tests {
    use crate::{db::Database, errors::DbError};
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestStruct {
        field1: String,
        field2: i32,
    }

    #[test]
    fn test_database_open() {
        let temp_dir = tempdir().unwrap();
        let db = Database::open(temp_dir.path());
        assert!(db.is_ok());
    }

    #[test]
    fn test_write_and_read_value() {
        let temp_dir = tempdir().unwrap();
        let db = Database::open(temp_dir.path()).unwrap();

        let test_data = TestStruct {
            field1: "test".to_string(),
            field2: 42,
        };

        // Write value
        db.write_value(b"test_key", &test_data).unwrap();

        // Read value
        let read_data: TestStruct = db.read(b"test_key").unwrap().unwrap();
        assert_eq!(read_data, test_data);
    }

    #[test]
    fn test_read_nonexistent_key() {
        let temp_dir = tempdir().unwrap();
        let db = Database::open(temp_dir.path()).unwrap();

        let result: Option<TestStruct> = db.read(b"nonexistent_key").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_invalid_path() {
        let result = Database::open("/nonexistent/path/that/should/fail");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DbError::RocksDb(_)));
    }

    #[test]
    fn test_write_multiple_values() {
        let temp_dir = tempdir().unwrap();
        let db = Database::open(temp_dir.path()).unwrap();

        let test_data1 = TestStruct {
            field1: "test1".to_string(),
            field2: 42,
        };
        let test_data2 = TestStruct {
            field1: "test2".to_string(),
            field2: 84,
        };

        // Write values
        db.write_value(b"test_key1", &test_data1).unwrap();
        db.write_value(b"test_key2", &test_data2).unwrap();

        // Read values
        let read_data1: TestStruct = db.read(b"test_key1").unwrap().unwrap();
        let read_data2: TestStruct = db.read(b"test_key2").unwrap().unwrap();

        assert_eq!(read_data1, test_data1);
        assert_eq!(read_data2, test_data2);
    }

    #[test]
    fn test_overwrite_value() {
        let temp_dir = tempdir().unwrap();
        let db = Database::open(temp_dir.path()).unwrap();

        let test_data1 = TestStruct {
            field1: "test1".to_string(),
            field2: 42,
        };
        let test_data2 = TestStruct {
            field1: "test2".to_string(),
            field2: 84,
        };

        // Write initial value
        db.write_value(b"test_key", &test_data1).unwrap();
        
        // Overwrite with new value
        db.write_value(b"test_key", &test_data2).unwrap();

        // Read value
        let read_data: TestStruct = db.read(b"test_key").unwrap().unwrap();
        assert_eq!(read_data, test_data2);
    }

    #[test]
    fn test_invalid_deserialization() {
        let temp_dir = tempdir().unwrap();
        let db = Database::open(temp_dir.path()).unwrap();

        // Write a string value
        db.write_value(b"test_key", &"invalid_data").unwrap();

        // Try to read it as TestStruct
        let result: Result<Option<TestStruct>, _> = db.read(b"test_key");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DbError::ReadDb(_)));
    }
}

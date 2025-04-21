use std::collections::HashMap;

use eyre::Result;
use storage::{
    db::Database,
    keys::{COMPLETED_REQUESTS, PENDING_REQUESTS},
};

use crate::BRequest;

pub fn request_data(request_id: &str, db: &Database) -> Result<Option<BRequest>> {
    let request = db.read::<_, BRequest>(request_id)?;
    Ok(request)
}

pub fn pending_requests(db: &Database) -> Option<Vec<String>> {
    db.read(PENDING_REQUESTS).unwrap()
}

pub fn completed_requests(db: &Database) -> Option<Vec<String>> {
    db.read(COMPLETED_REQUESTS).unwrap()
}

pub fn add_completed_request(request_id: &str, db: &Database) -> Result<()> {
    if let Ok(Some(mut completed)) = db.read::<_, Vec<String>>(COMPLETED_REQUESTS) {
        completed.push(request_id.to_owned());
        update_vector(db, COMPLETED_REQUESTS, completed)?;
    } else {
        let completed = vec![request_id.to_owned()];
        update_vector(db, COMPLETED_REQUESTS, completed)?;
    }
    Ok(())
}

pub fn update_vector(db: &Database, key: &str, requests: Vec<String>) -> Result<()> {
    _ = db.write_value(key, &requests)?;
    Ok(())
}

pub fn update_hashmap(db: &Database, key: &str, indexes: HashMap<String, i128>) -> Result<()> {
    _ = db.write_value(key, &indexes)?;
    Ok(())
}

#[cfg(test)]
mod types_test {
    use crate::{
        add_completed_request, completed_requests, pending_requests, update_hashmap, update_vector,
    };
    use std::collections::HashMap;
    use storage::db::Database;
    use storage::keys::{COMPLETED_REQUESTS, PENDING_REQUESTS};
    use tempfile::tempdir;

    // Helper function to create a test database
    fn setup_test_db() -> Database {
        let dir = tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        Database::open(path).unwrap()
    }

    #[test]
    fn test_pending_and_completed_requests() {
        let db = setup_test_db();

        // Initially there should be no pending or completed requests
        assert!(pending_requests(&db).is_none());
        assert!(completed_requests(&db).is_none());

        // Add a pending request
        let pending = vec!["request1".to_string()];
        update_vector(&db, PENDING_REQUESTS, pending.clone()).unwrap();

        // Check that the pending request was added
        let retrieved_pending = pending_requests(&db).unwrap();
        assert_eq!(retrieved_pending, pending);

        // Add a completed request
        let completed = vec!["request2".to_string()];
        update_vector(&db, COMPLETED_REQUESTS, completed.clone()).unwrap();

        // Check that the completed request was added
        let retrieved_completed = completed_requests(&db).unwrap();
        assert_eq!(retrieved_completed, completed);
    }

    #[test]
    fn test_add_completed_request() {
        let db = setup_test_db();

        // Initially there should be no completed requests
        assert!(completed_requests(&db).is_none());

        // Add a completed request
        add_completed_request("request1", &db).unwrap();

        // Check that the completed request was added
        let completed = completed_requests(&db).unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0], "request1");

        // Add another completed request
        add_completed_request("request2", &db).unwrap();

        // Check that both completed requests are there
        let completed = completed_requests(&db).unwrap();
        assert_eq!(completed.len(), 2);
        assert!(completed.contains(&"request1".to_string()));
        assert!(completed.contains(&"request2".to_string()));
    }

    #[test]
    fn test_update_vector() {
        let db = setup_test_db();
        let key = "test_vector";

        // Update with an initial vector
        let initial = vec!["item1".to_string(), "item2".to_string()];
        update_vector(&db, key, initial.clone()).unwrap();

        // Check that the vector was saved
        let retrieved: Vec<String> = db.read(key).unwrap().unwrap();
        assert_eq!(retrieved, initial);

        // Update with a new vector
        let updated = vec!["item3".to_string(), "item4".to_string()];
        update_vector(&db, key, updated.clone()).unwrap();

        // Check that the vector was updated
        let retrieved: Vec<String> = db.read(key).unwrap().unwrap();
        assert_eq!(retrieved, updated);
    }

    #[test]
    fn test_update_hashmap() {
        let db = setup_test_db();
        let key = "test_hashmap";

        // Update with an initial hashmap
        let mut initial = HashMap::new();
        initial.insert("key1".to_string(), 100);
        initial.insert("key2".to_string(), 200);
        update_hashmap(&db, key, initial.clone()).unwrap();

        // Check that the hashmap was saved
        let retrieved: HashMap<String, i128> = db.read(key).unwrap().unwrap();
        assert_eq!(retrieved, initial);

        // Update with a new hashmap
        let mut updated = HashMap::new();
        updated.insert("key3".to_string(), 300);
        updated.insert("key4".to_string(), 400);
        update_hashmap(&db, key, updated.clone()).unwrap();

        // Check that the hashmap was updated
        let retrieved: HashMap<String, i128> = db.read(key).unwrap().unwrap();
        assert_eq!(retrieved, updated);
    }
}

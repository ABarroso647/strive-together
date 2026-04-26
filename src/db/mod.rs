// Database modules - one per tracker type
pub mod gym;

use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Open or create the database at the given path
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;

        // Enable foreign keys
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Initialize all tracker schemas
    pub fn init_schema(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        // Initialize gym tracker schema
        conn.execute_batch(gym::schema::SCHEMA)?;

        // Future: Initialize other tracker schemas here
        // conn.execute_batch(calories::schema::SCHEMA)?;

        Ok(())
    }

    /// Get a lock on the connection for queries
    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }
}

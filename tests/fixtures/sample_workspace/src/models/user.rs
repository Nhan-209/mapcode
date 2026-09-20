// tests/fixtures/sample_workspace/src/models/user.rs

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub email: String,
}

impl Default for User {
    fn default() -> Self {
        User {
            id: 0,
            username: String::from("guest"),
            email: String::from("guest@example.com"),
        }
    }
}

impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "User({}, {})", self.id, self.username)
    }
}

impl User {
    pub fn new(id: u64, username: &str, email: &str) -> Self {
        User {
            id,
            username: username.to_string(),
            email: email.to_string(),
        }
    }

    pub fn display_name(&self) -> String {
        format!("{} <{}>", self.username, self.email)
    }
}

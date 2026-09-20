// tests/fixtures/sample_workspace/src/service/user_service.rs

use crate::models::user::User;

pub struct UserService {
    users: Vec<User>,
}

impl UserService {
    pub fn new() -> Self {
        UserService {
            users: vec![
                User::new(1, "alice", "alice@example.com"),
                User::new(2, "bob", "bob@example.com"),
            ],
        }
    }

    pub fn get_all(&self) -> Vec<User> {
        self.users.clone()
    }

    pub fn find_by_id(&self, id: u64) -> Option<User> {
        self.users.iter().find(|u| u.id == id).cloned()
    }

    pub fn create_user(&mut self, username: &str, email: &str) -> User {
        let next_id = (self.users.len() + 1) as u64;
        let new_user = User::new(next_id, username, email);
        self.users.push(new_user.clone());
        new_user
    }
}

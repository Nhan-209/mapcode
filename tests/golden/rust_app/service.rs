pub struct Service {
    pub name: String,
}

impl Service {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }

    pub fn process(&self) -> String {
        format!("Processed: {}", self.name)
    }

    pub fn close(&self) {
        // cleanup service
    }
}

pub fn standalone_process() -> String {
    "standalone".to_string()
}

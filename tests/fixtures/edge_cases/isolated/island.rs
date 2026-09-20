// tests/fixtures/edge_cases/isolated/island.rs

// This file has NO incoming imports and NO outgoing imports.
// Ca = 0, Ce = 0, Instability = 0.0.

pub struct SolitaryIsland {
    pub name: String,
}

impl SolitaryIsland {
    pub fn new() -> Self {
        SolitaryIsland {
            name: "Atlantis".to_string(),
        }
    }

    pub fn compute_secret(&self) -> u64 {
        42
    }
}

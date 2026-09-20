// tests/fixtures/edge_cases/deep_call_chain/chains.rs

pub fn leaf_step1() -> i32 {
    1
}

pub fn step2() -> i32 {
    leaf_step1() + 1
}

pub fn step3() -> i32 {
    step2() + 1
}

pub fn step4() -> i32 {
    step3() + 1
}

pub fn step5() -> i32 {
    step4() + 1
}

pub fn chain_main() -> i32 {
    step5() + 1
}

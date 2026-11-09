pub struct State {
    pub value: u8,
}

pub fn strong() -> State {
    let mut state = State { value: 0 };
    state.value = 7;
    state
}

pub fn weak(mut items: [u8; 1]) -> [u8; 1] {
    items[0] = 9;
    items
}

pub fn inferred() -> usize {
    let mut items = [Default::default()];
    items[0] = 3_u8;
    items.len()
}

pub fn restored_move() -> String {
    struct Owned {
        value: String,
    }
    let mut state = Owned {
        value: String::new(),
    };
    let previous = state.value;
    state.value = String::from("new");
    drop(state);
    previous
}

#[cfg(test)]
fn excluded_test_only() {
    let mut state = State { value: 0 };
    state.value = 99;
    assert_eq!(state.value, 99);
}

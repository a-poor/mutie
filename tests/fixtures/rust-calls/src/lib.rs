pub fn strong() -> Vec<u8> {
    let mut values: Vec<u8> = Vec::new();
    values.push(7);
    values
}

pub fn weak(include: bool) -> Vec<u8> {
    let mut values: Vec<u8> = Vec::new();
    if include {
        values.push(9);
    }
    values
}

pub fn inferred() -> usize {
    let mut values = Vec::new();
    values.push(3_u8);
    values.len()
}

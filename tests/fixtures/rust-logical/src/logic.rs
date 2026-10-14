pub fn all(a: bool, b: bool, trace: &mut String) -> bool {
    ({
        trace.push('L');
        a
    }) && {
        trace.push('R');
        b
    }
}
pub fn any(a: bool, b: bool, trace: &mut String) -> bool {
    ({
        trace.push('L');
        a
    }) || {
        trace.push('R');
        b
    }
}
pub fn weak_any(a: bool, b: bool) -> bool {
    a || b
}
pub fn all_three(a: bool, b: bool, c: bool) -> bool {
    a && b && c
}
pub fn any_three(a: bool, b: bool, c: bool) -> bool {
    a || b || c
}
pub fn chain(a: bool, b: bool, c: bool) -> bool {
    if a && b
        && let Some(value) = Some(c)
    {
        value
    } else {
        false
    }
}

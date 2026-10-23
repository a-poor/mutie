pub fn choose(value: bool) -> u8 { if value { 1 } else { 2 } }
pub fn weak(value: bool) -> u8 { if value { 1 } else { 2 } }
pub fn initialized(flag: bool) -> u8 { let value; if { value = 7; flag } {} value }

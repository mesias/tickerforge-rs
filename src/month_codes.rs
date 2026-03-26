//! Futures month codes (CME-style).

pub fn month_to_code(month: u32) -> Result<char, String> {
    let c = match month {
        1 => 'F',
        2 => 'G',
        3 => 'H',
        4 => 'J',
        5 => 'K',
        6 => 'M',
        7 => 'N',
        8 => 'Q',
        9 => 'U',
        10 => 'V',
        11 => 'X',
        12 => 'Z',
        _ => return Err(format!("Invalid month: {month}")),
    };
    Ok(c)
}

pub fn code_to_month(code: char) -> Result<u32, String> {
    match code.to_ascii_uppercase() {
        'F' => Ok(1),
        'G' => Ok(2),
        'H' => Ok(3),
        'J' => Ok(4),
        'K' => Ok(5),
        'M' => Ok(6),
        'N' => Ok(7),
        'Q' => Ok(8),
        'U' => Ok(9),
        'V' => Ok(10),
        'X' => Ok(11),
        'Z' => Ok(12),
        _ => Err(format!("Invalid month code: {code}")),
    }
}

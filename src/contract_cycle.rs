//! Contract month resolution from cycle definitions.

use crate::models::ContractCycle;
use crate::month_codes::code_to_month;

fn builtin_month_codes(name: &str) -> Option<Vec<&'static str>> {
    match name {
        "monthly" => Some(vec![
            "F", "G", "H", "J", "K", "M", "N", "Q", "U", "V", "X", "Z",
        ]),
        "bimonthly_even" => Some(vec!["G", "J", "M", "Q", "V", "Z"]),
        "quarterly" => Some(vec!["H", "M", "U", "Z"]),
        _ => None,
    }
}

/// Resolve list of calendar months (1–12) for a contract cycle and year.
pub fn resolve_contract_months(cycle: &ContractCycle, _year: i32) -> Result<Vec<u32>, String> {
    let month_codes: Vec<String> = if !cycle.months.is_empty() {
        cycle.months.clone()
    } else if let Some(codes) = builtin_month_codes(cycle.name.as_str()) {
        codes.iter().map(|s| (*s).to_string()).collect()
    } else {
        return Err(format!("Unknown contract cycle: {}", cycle.name));
    };

    let mut months: Vec<u32> = month_codes
        .iter()
        .map(|code| {
            let ch = code
                .chars()
                .next()
                .ok_or_else(|| format!("empty month code in cycle {}", cycle.name))?;
            code_to_month(ch)
        })
        .collect::<Result<Vec<_>, _>>()?;
    months.sort_unstable();
    months.dedup();
    Ok(months)
}

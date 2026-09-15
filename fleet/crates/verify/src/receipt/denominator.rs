pub(super) fn valid_input_denominator(value: &str, checked: u64, total: u64) -> bool {
    let Some((input_checked, input_total)) = value
        .strip_prefix("denominator:")
        .and_then(parse_denominator)
    else {
        return false;
    };
    input_checked == checked && input_total == total
}

fn parse_denominator(value: &str) -> Option<(u64, u64)> {
    let (numerator, total) = value.split_once('/')?;
    Some((numerator.parse().ok()?, total.parse().ok()?))
}

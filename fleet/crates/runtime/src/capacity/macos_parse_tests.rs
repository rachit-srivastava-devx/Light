use super::*;

#[test]
fn parses_a_real_vm_stat_sample() {
    let sample = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\n\
                   Pages free:                              10000.\n\
                   Pages inactive:                          20000.\n\
                   Pages speculative:                         500.\n\
                   Pages wired down:                         9999.\n";
    assert_eq!(parse_vm_stat_available(sample).unwrap(), 30500 * 16384);
}

#[test]
fn parses_loadavg_braces() {
    assert!((parse_loadavg("{ 1.23 4.56 7.89 }\n").unwrap() - 1.23).abs() < 1e-9);
}

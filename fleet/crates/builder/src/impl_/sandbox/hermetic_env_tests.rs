use super::*;

#[test]
fn build_never_reuses_the_real_home() {
    let dir = tempfile::tempdir().unwrap();
    let env = build(dir.path());
    assert!(env.home.starts_with(dir.path()));
    assert_ne!(
        env.home,
        PathBuf::from(env::var("HOME").unwrap_or_default())
    );
}

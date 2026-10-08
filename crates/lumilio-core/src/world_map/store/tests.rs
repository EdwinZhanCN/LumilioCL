use super::*;
#[test]
fn manual_seeds_survive_restart_and_remain_instance_scoped() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("launcher.db");
    let db = Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE instances(id TEXT); INSERT INTO instances VALUES('one'),('two'); CREATE TABLE world_map_seeds(instance TEXT,seed INTEGER,version TEXT,PRIMARY KEY(instance,seed,version));").unwrap();
    drop(db);
    save(&path, "one", i64::MIN, "1.21.4").unwrap();
    save(&path, "one", i64::MIN, "1.21.4").unwrap();
    assert_eq!(load(&path, "one").unwrap().len(), 1);
    assert!(load(&path, "two").unwrap().is_empty());
    assert_eq!(load(&path, "one").unwrap()[0].context.seed, Some(i64::MIN));
}
#[test]
fn textual_seeds_use_java_utf16_hash() {
    assert_eq!(parse_seed(" 262 "), Some(262));
    assert_eq!(parse_seed("hello"), Some(99162322));
    assert_eq!(parse_seed("😀"), Some(1772899));
    assert_eq!(parse_seed(" "), None);
}

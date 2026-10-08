use sled::Db;
fn main() {
    let db = sled::open("/app/data/tusm_indexer.db").unwrap();
    for row in db.iter() {
        let (k, v) = row.unwrap();
        println!("Key: {:?}", String::from_utf8_lossy(&k));
    }
}

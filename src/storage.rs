use rkv::{Manager, Rkv, StoreOptions, Value};

use std::sync::{Arc, RwLock};

pub struct SingleKvStorage {
    pub env: Arc<RwLock<Rkv>>,
    pub single: rkv::store::single::SingleStore,
}

impl SingleKvStorage {
    pub fn new(path: &str, db: &str) -> SingleKvStorage {
        let path = std::path::Path::new(path);
        std::fs::create_dir_all(path).unwrap();
        let created_arc = Manager::singleton()
            .write()
            .unwrap()
            .get_or_create(path, Rkv::new)
            .unwrap();
        let created_arc2 = Manager::singleton()
            .write()
            .unwrap()
            .get_or_create(path, Rkv::new)
            .unwrap();
        let k = created_arc2.read().unwrap();
        let store = k.open_single(db, StoreOptions::create()).unwrap();
        SingleKvStorage {
            env: created_arc,
            single: store,
        }
    }

    pub fn put_single(&self, key: &str, value: &Value<'_>) {
        let env = self.env.read().unwrap();
        let mut writer = env.write().unwrap();
        self.single.put(&mut writer, key, &value).unwrap();
        writer.commit().unwrap();
    }

    pub fn get_single(&self, key: &str) -> Option<String> {
        let env = self.env.read().unwrap();
        let reader = env.read().unwrap();
        let access_token = self.single.get(&reader, key).unwrap();
        match access_token {
            Some(rkv::value::Value::Json(str_token)) => Some(str_token.to_string()),
            _ => None,
        }
    }

    pub fn del_single(&self, key: &str) {
        let env = self.env.read().unwrap();
        let mut writer = env.write().unwrap();
        self.single.delete(&mut writer, key).unwrap();
        writer.commit().unwrap();
    }
}

use chrono;
use serde_json;
use uuid;

use std::sync::{Arc, RwLock};

const STORE: &str = "content";
const STORE_INDEX: &str = "content_index";

lazy_static! {
    pub static ref INTERFACE: ContentInterface = ContentInterface::new();
}

pub struct ContentInterface {
    // 实际内容 id/body
    storage: super::storage::SingleKvStorage,
    // 按照日期存储的 date/vec(id) 索引
    storage_index: super::storage::SingleKvStorage,
    // 上次检查过期时间
    last_expire_check: Arc<RwLock<chrono::Date<chrono::Local>>>,
}

impl ContentInterface {
    pub fn new() -> ContentInterface {
        ContentInterface {
            storage: super::storage::SingleKvStorage::new(&super::CONFIG.db_path, STORE),
            storage_index: super::storage::SingleKvStorage::new(
                &super::CONFIG.db_path,
                STORE_INDEX,
            ),
            last_expire_check: Arc::new(RwLock::new(chrono::Local::today().pred())),
        }
    }
    // 返回内容id
    pub fn add_content(&self, body: &str) -> String {
        let id = uuid::Uuid::new_v4().to_simple().to_string();
        let today = chrono::Local::today();
        debug!("new content id:{},body:{}", id, body);
        let date: String = today.format("%Y%m%d").to_string();
        // 添加内容
        self.storage.put_single(&id, &rkv::Value::Json(&body));
        // 添加到索引
        let ids = self.storage_index.get_single(&date);
        let mut new_ids = Vec::new();
        if let Some(ids_string) = ids {
            new_ids = serde_json::from_str(&ids_string).unwrap();
        }
        new_ids.push(id.clone());
        let new_json = serde_json::to_string(&new_ids).unwrap();
        self.storage_index
            .put_single(&date, &rkv::Value::Json(&new_json));

        let content = self.storage.get_single(&id);
        debug!("get content:{}", id);
        match content {
            Some(content_string) => {
                debug!("content:{}", content_string);
                ()
            }
            None => {
                debug!("没找到对应内容");
                ()
            }
        }

        id
    }

    pub fn get_content(&self, id: &str) -> Result<String, &str> {
        let content = self.storage.get_single(id);
        debug!("get content:{}", id);
        match content {
            Some(content_string) => Ok(content_string),
            None => Err("没找到对应内容"),
        }
    }

    pub fn clean_contents(&self) {
        let expire = super::CONFIG.content_expire;
        debug!("expire:{}", expire);
        if expire == 0 {
            return;
        }
        let today = chrono::Local::today();
        {
            let last_check = self.last_expire_check.read().unwrap();
            debug!("last_check:{:?},today:{:?}", *last_check, today);
            if *last_check >= today {
                return;
            }
        }
        {
            let mut last_check = self.last_expire_check.write().unwrap();
            *last_check = today;
        }
        let dur = chrono::Duration::days(expire as i64);
        let cmp_day = today.checked_sub_signed(dur).unwrap();
        let cmp_num = cmp_day.format("%Y%m%d").to_string().parse::<u32>().unwrap();
        // 遍历storage_index查过期内容
        let env = self.storage_index.env.read().unwrap();
        let reader = env.read().unwrap();
        let mut iter = self.storage_index.single.iter_start(&reader).unwrap();
        let mut index_to_delete = Vec::new();
        while let Some(Ok((date, id))) = iter.next() {
            let date_string = std::str::from_utf8(&date).unwrap();
            let date_num = date_string.parse::<u32>().unwrap();
            debug!("date_num:{},cmp_num:{}", date_num, cmp_num);
            if date_num >= cmp_num {
                continue;
            }
            if let Some(rkv::Value::Json(ids)) = id {
                let _ids: Vec<String> = serde_json::from_str(ids).unwrap();
                for _id in _ids {
                    // 从storage删除数据
                    debug!("del date:{}, id:{}", date_string, _id);
                    self.storage.del_single(&_id);
                }
            }
            // 删除自己
            index_to_delete.push(date_string);
        }
        for _date in index_to_delete {
            debug!("del index date:{}", _date);
            self.storage_index.del_single(&_date);
        }
    }
}

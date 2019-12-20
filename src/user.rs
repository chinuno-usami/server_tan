extern crate serde_json;

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub owns: Vec<String>,
    pub subscribes: Vec<String>,
}

pub struct UserInterface {
    storage: super::storage::SingleKvStorage,
}

lazy_static! {
    pub static ref INTERFACE: UserInterface = UserInterface::new();
}

const STORE: &str = "user";

impl UserInterface {
    pub fn new() -> UserInterface {
        UserInterface { storage: super::storage::SingleKvStorage::new(&super::CONFIG.db_path, STORE) }
    }

    fn get_user_name_internal(&self, id: &str) -> String {
        let token = super::wx_interface::INTERFACE.get_access_token();
        let client = reqwest::Client::new();
        let res: serde_json::Value = client.get("https://api.weixin.qq.com/cgi-bin/user/info")
            .query(&[("access_token", token.access_token.as_str())])
            .query(&[("openid", id)])
            .query(&[("lang", "zh_CN")])
            .send().unwrap()
            .json().unwrap();
        res["nickname"].as_str().unwrap().to_string()
    }


    fn update_user_name(&self, id: &str) -> User {
        let new_name = self.get_user_name_internal(id);
        match self.get_user(id) {
            Ok(mut user) => {
                user.name = new_name;
                let json_string = serde_json::to_string(&user).unwrap();
                self.storage.put_single(id, &rkv::Value::Json(&json_string));
                user
            }
            _ => {
                let new_user = User { id: id.to_string(), name: new_name, owns:Vec::<String>::new(), subscribes:Vec::<String>::new() };
                let json_string = serde_json::to_string(&new_user).unwrap();
                self.storage.put_single(id, &rkv::Value::Json(&json_string));
                new_user
            }
        }
    }

    pub fn get_user(&self, id: &str) -> Result<User, &str> {
        // 尝试从数据库获取user
        let user = self.storage.get_single(id);
        match user {
            Some(user_string) => {
                let _user: User = serde_json::from_str(&user_string).unwrap();
                Ok(_user)
            }
            None => Err("未找到用户")
        }
    }

    pub fn add_user(&self, id: &str) -> Result<bool, &str> {
        // 尝试从数据库获取user
        let user = self.storage.get_single(id);
        match user {
            Some(_) => Err("用户已存在"),
            None => {
                self.update_user_name(id);
                Ok(true)
            }
        }
    }

    pub fn user_subscribe(&self, user: &str, channel: &str) -> Result<bool, &str> {
        match self.get_user(user) {
            Ok(mut _user) => {
                if _user.subscribes.contains(&channel.to_string()){
                    Err("已经订阅过了")
                } else {
                    _user.subscribes.push(channel.to_string());
                    let json_string = serde_json::to_string(&_user).unwrap();
                    self.storage.put_single(user, &rkv::Value::Json(&json_string));
                    Ok(true)
                }
            }
            Err(err) => Err(err)
        }
    }

    pub fn user_unsubscribe(&self, user: &str, channel: &str) -> Result<bool, &str> {
        match self.get_user(user) {
            Ok(mut _user) => {
                for (i, chn) in _user.subscribes.iter().enumerate() {
                    if channel == chn.as_str() {
                        _user.subscribes.remove(i);
                        break;
                    }
                }
                let json_string = serde_json::to_string(&_user).unwrap();
                self.storage.put_single(user, &rkv::Value::Json(&json_string));
                Ok(true)
            }
            Err(err) => Err(err)
        }
    }

    pub fn user_new_channel(&self, user: &str, channel: &str) -> Result<bool, &str> {
        match self.get_user(user) {
            Ok(mut _user) => {
                _user.owns.push(channel.to_string());
                let json_string = serde_json::to_string(&_user).unwrap();
                self.storage.put_single(user, &rkv::Value::Json(&json_string));
                Ok(true)
            }
            Err(err) => Err(err)
        }
    }

    pub fn user_del_channel(&self, user: &str, channel: &str) -> Result<bool, &str> {
        match self.get_user(user) {
            Ok(mut _user) => {
                for (i, chn) in _user.owns.iter().enumerate() {
                    if channel == chn.as_str() {
                        _user.owns.remove(i);
                        break;
                    }
                }
                _user.owns.push(channel.to_string());
                let json_string = serde_json::to_string(&_user).unwrap();
                self.storage.put_single(user, &rkv::Value::Json(&json_string));
                Ok(true)
            }
            Err(err) => Err(err)
        }
    }
}

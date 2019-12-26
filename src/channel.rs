use rkv;
use serde_json;
use uuid;

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct Channel {
    pub id: String,
    pub sendkey: String,
    pub name: String,
    pub owner: String,
    pub subscribers: Vec<String>,
}

const STORE: &str = "channel";

lazy_static! {
    pub static ref INTERFACE: ChannelInterface = ChannelInterface::new();
}

pub struct ChannelInterface {
    storage: super::storage::SingleKvStorage,
}

impl ChannelInterface {
    pub fn new() -> ChannelInterface {
        ChannelInterface {
            storage: super::storage::SingleKvStorage::new(&super::CONFIG.db_path, STORE),
        }
    }

    // 返回id
    pub fn add_channel(&self, name: &str, owner: &str) -> Result<String, &str> {
        let id = uuid::Uuid::new_v4().to_simple().to_string();
        let sendkey = uuid::Uuid::new_v4().to_simple().to_string();
        let channel = Channel {
            id: id.clone(),
            sendkey: sendkey.clone(),
            name: name.to_string(),
            owner: owner.to_string(),
            subscribers: Vec::<String>::new(),
        };

        let json_string = serde_json::to_string(&channel).unwrap();
        self.storage
            .put_single(&id, &rkv::Value::Json(&json_string));
        match super::user::INTERFACE.user_new_channel(owner, &id) {
            Ok(_) => Ok(id),
            Err(err) => Err(err),
        }
    }

    pub fn delete_channel(&self, id: &str, owner: &str) -> Result<bool, &str> {
        match self.get_channel_by_id(id) {
            Ok(chn) => {
                // 先取消所有用户订阅
                for user in chn.subscribers {
                    self.unsubscribe(id, &user).unwrap();
                }
                match super::user::INTERFACE.user_del_channel(owner, id) {
                    Ok(_) => {
                        self.storage.del_single(id);
                        Ok(true)
                    }
                    err => err,
                }
            }
            Err(err) => Err(err),
        }
    }
    pub fn subscribe(&self, channel: &str, user: &str) -> Result<bool, &str> {
        match self.get_channel_by_id(channel) {
            Ok(mut chn) => match super::user::INTERFACE.user_subscribe(user, channel) {
                Ok(_) => {
                    chn.subscribers.push(user.to_string());
                    let json_string = serde_json::to_string(&chn).unwrap();
                    self.storage
                        .put_single(channel, &rkv::Value::Json(&json_string));
                    Ok(true)
                }
                err => err,
            },
            Err(err) => Err(err),
        }
    }
    pub fn unsubscribe(&self, channel: &str, user: &str) -> Result<bool, &str> {
        match self.get_channel_by_id(channel) {
            Ok(mut chn) => match super::user::INTERFACE.user_unsubscribe(user, channel) {
                Ok(_) => {
                    for (i, usr) in chn.subscribers.iter().enumerate() {
                        if user == usr.as_str() {
                            chn.subscribers.remove(i);
                            break;
                        }
                    }
                    let json_string = serde_json::to_string(&chn).unwrap();
                    self.storage
                        .put_single(channel, &rkv::Value::Json(&json_string));
                    Ok(true)
                }
                err => err,
            },
            Err(err) => Err(err),
        }
    }
    pub fn get_channel_by_id(&self, id: &str) -> Result<Channel, &str> {
        let channel = self.storage.get_single(id);
        match channel {
            Some(channel_string) => {
                let channel: Channel = serde_json::from_str(&channel_string).unwrap();
                Ok(channel)
            }
            None => Err("没找到对应频道"),
        }
    }
    pub fn get_channel_by_owner(&self, user: &str) -> Result<Vec<Channel>, &str> {
        let env = self.storage.env.read().unwrap();
        let reader = env.read().unwrap();
        let mut iter = self.storage.single.iter_start(&reader).unwrap();
        let mut ret = Vec::<Channel>::new();
        while let Some(Ok((id, channel))) = iter.next() {
            if let Some(rkv::Value::Json(_channel)) = channel {
                let chn: Channel = serde_json::from_str(_channel).unwrap();
                debug!("{}, {:?}", std::str::from_utf8(&id).unwrap(), chn);
                if chn.owner == user {
                    ret.push(chn);
                }
            }
        }
        Ok(ret)
    }

    pub fn get_channel_by_sendkey(&self, sendkey: &str) -> Result<Channel, &str> {
        let env = self.storage.env.read().unwrap();
        let reader = env.read().unwrap();
        let mut iter = self.storage.single.iter_start(&reader).unwrap();
        while let Some(Ok((id, channel))) = iter.next() {
            if let Some(rkv::Value::Json(_channel)) = channel {
                let chn: Channel = serde_json::from_str(_channel).unwrap();
                debug!("{}, {:?}", std::str::from_utf8(&id).unwrap(), chn);
                if chn.sendkey == sendkey {
                    return Ok(chn);
                }
            }
        }
        Err("没找到对应的频道")
    }

    pub fn get_subscribers(&self, id: &str) -> Result<Vec<super::user::User>, &str> {
        match self.get_channel_by_id(id) {
            Ok(channel) => {
                let mut ret = Vec::<super::user::User>::new();
                for uid in channel.subscribers {
                    match super::user::INTERFACE.get_user(&uid) {
                        Ok(user) => ret.push(user),
                        Err(err) => {
                            return Err(err);
                        }
                    }
                }
                Ok(ret)
            }
            Err(err) => Err(err),
        }
    }
}

extern crate chrono;
extern crate serde_derive;
extern crate serde_json;
extern crate crypto;


#[derive(Debug, Serialize, Deserialize)]
struct GetTokenResult {
    access_token: String,
    expires_in: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct TemplateResult {
    errcode: i32,
    errmsg: String,
    // msgid: i64,
}

lazy_static! {
    pub static ref INTERFACE: WxInterface = WxInterface::new();
}

use super::access_token::AccessToken;

pub struct WxInterface {
    storage: super::storage::SingleKvStorage,
}

const STORE: &str = "wx";

impl WxInterface {
    pub fn new() -> WxInterface {
        WxInterface { storage: super::storage::SingleKvStorage::new(&super::CONFIG.db_path, STORE) }
    }

    fn get_access_token_internal(&self) -> AccessToken {
        let config = super::CONFIG.clone();
        let appid = config.appid;
        let secret = config.secret;
        let client = reqwest::Client::new();
        let res: GetTokenResult = client.get("https://api.weixin.qq.com/cgi-bin/token")
            .query(&[("grant_type", "client_credential")])
            .query(&[("appid", &appid)])
            .query(&[("secret", &secret)])
            .send().unwrap()
            .json().unwrap();
        let expires = chrono::Utc::now()+chrono::Duration::seconds(res.expires_in);
        let token = res.access_token;
        AccessToken { access_token:token, expires:expires.timestamp()}
    }


    fn update_access_token(&self) -> AccessToken {
        let new_token = self.get_access_token_internal();
        // kv.put_access_token(&serde_json::to_string(&new_token).unwrap());
        let json_string = serde_json::to_string(&new_token).unwrap();
        self.storage.put_single("access_token", &rkv::Value::Json(&json_string));
        new_token
    }

    pub fn get_access_token(&self) -> AccessToken {
        // 尝试从数据库获取access token
        let token = self.storage.get_single("access_token");
        match token {
            Some(token_string) => {
                let access_token: AccessToken = serde_json::from_str(&token_string).unwrap();
                let now = chrono::Utc::now();
                // 过期重新获取
                if access_token.expires <= now.timestamp() {
                    self.update_access_token()
                } else {
                    access_token
                }
            }
            None => self.update_access_token()
        }
    }


    pub fn send_template(&self, template_id: &str, user: &str, channel_name: &str, title: &str, time: &str, body: &str, url: &str) {
        let post = json!({
            "touser": user,
            "template_id": template_id,
            "url": url,
            "data": {
                "title": {
                    "value": title,
                    "color": "#173177"
                },
                "name": {
                    "value": channel_name,
                    "color": "#173177"
                },
                "time": {
                    "value": time,
                    "color": "#173177"
                },
                "body": {
                    "value": body,
                    "color": "#173177"
                }
            }
        });

        debug!("template req:{:?}", post);
        debug!("template req:{}", &post.to_string());

        let mut result = reqwest::Client::new()
            .post("https://api.weixin.qq.com/cgi-bin/message/template/send")
            .query(&[("access_token", &self.get_access_token().access_token)])
            .json(&post)
            .send().unwrap();
        debug!("template res:{:?}", result);
        let tmpres: TemplateResult = result.json().unwrap();
        debug!("{:?}", tmpres);
        debug!("{:?}", result.text().unwrap());
    }
}


pub fn check_signature(signature: &str, timestamp: &str, nonce: &str) -> bool {
    debug!("signature:{}",signature);
    debug!("timestamp:{}",timestamp);
    debug!("nonce:{}",nonce);
    let token:String = super::CONFIG.token.clone();
    let mut v = [token, timestamp.to_string(), nonce.to_string()];
    v.sort();

    use self::crypto::digest::Digest;
    use self::crypto::sha1::Sha1;

    let mut hasher = Sha1::new();
    hasher.input_str(format!("{}{}{}",v[0],v[1],v[2]).as_str());

    let hex = hasher.result_str();
    debug!("calced signature:{}", hex);
    hex == signature
}

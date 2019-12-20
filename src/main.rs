extern crate actix_web;
extern crate url;

#[macro_use]
extern crate log;
extern crate chrono;
extern crate env_logger;

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate serde;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate clap;

mod storage;
mod config;
mod channel;
mod user;
mod content;

mod xml;


mod wx_interface;
mod access_token;

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use std::sync::Mutex;
use std::io::prelude::*;
use std::fs;

// 全局配置对象
lazy_static! {
    static ref CONFIG_FILE: Mutex<String> = Mutex::new("config.toml".to_string());
    static ref CONFIG: config::Config = match config::Config::new(&CONFIG_FILE.lock().unwrap()){
        Ok(cfg) => cfg,
        Err(err) => panic!("{:?}", err),
    };
    static ref DETAIL_TEMPLATE: String = {
        let mut file = fs::File::open(&CONFIG.detail_template).unwrap();
        let mut html = String::new();
        file.read_to_string(&mut html).unwrap();
        html
    };
}

// 初始化日志，自定义了日志格式
fn init_log() {
    use chrono::Local;

    let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "debug");
    env_logger::Builder::from_env(env)
        .format(|buf, record| {
            writeln!(
                buf,
                "{} {} [{}:{}:{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.module_path().unwrap_or("<unnamed>"),
                record.file().unwrap_or(""),
                record.line().unwrap_or(0),
                &record.args()
                )

        })
        .init();

    info!("env_logger initialized.");
}


#[derive(Deserialize,Debug)]
struct AuthEchoInfo {
    signature: String,
    timestamp: String,
    nonce: String,
    echostr: String,
}

fn wx_auth(query: web::Query<AuthEchoInfo>) -> impl Responder {
    debug!("get /wx");
    debug!("query:{:?}", query);
    let signature = &query.signature;
    let timestamp = &query.timestamp;
    let nonce = &query.nonce;
    let echostr = &query.echostr;
    debug!("echostr:{}",echostr);
    if wx_interface::check_signature(signature, timestamp, nonce) {
        debug!("auth pass!");
        HttpResponse::Ok().body(echostr)
    } else {
        debug!("auth failed!");
        HttpResponse::Forbidden().finish()
    }
}

#[derive(Deserialize,Debug)]
struct SubInfo {
    sendkey: String,
    text: String,
    desp: String,
}

fn wx_sub(query: web::Query<SubInfo>) -> impl Responder {
    debug!("get /sub");
    debug!("query:{:?}", query);
    // 先清理过期数据
    content::INTERFACE.clean_contents();
    // 通过sendkey获取channel
    let ch = channel::INTERFACE.get_channel_by_sendkey(&query.sendkey);
    match ch {
        Ok(_ch) => {
            // 添加content
            let id = content::INTERFACE.add_content(&query.desp);
            // 通过模板发送消息
            let subers = channel::INTERFACE.get_subscribers(&_ch.id).unwrap();
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            debug!("new wx_interface");
            let wx = wx_interface::WxInterface::new();
            debug!("wx get token");
            let at = wx.get_access_token();
            debug!("wx at:{:?}", at);
            for user in subers {
                wx.send_template(&CONFIG.template_id,
                                 &user.id,
                                 &_ch.name,
                                 &query.text,
                                 &now,
                                 &query.desp,
                                 &format!("{}/content/{}", CONFIG.host, id)
                                );
            }

            HttpResponse::Ok().body("success")
        }
        Err(err) => HttpResponse::BadRequest().body(err)
    }
}

fn show_content(path: web::Path<String>) -> impl Responder {
    debug!("get /content/{}", path);
    // 获取content
    match content::INTERFACE.get_content(&path.to_string()) {
       Ok(body) => {
           debug!("get content:{}", body);
           // 替换模板
           let output = DETAIL_TEMPLATE.replace("{::}", &body);
           HttpResponse::Ok().body(output)
       }
       Err(err) => {
           debug!("get content:{}", err);
           HttpResponse::NotFound().finish()
       }
    }
}

#[derive(Deserialize,Debug)]
struct AuthInfo {
    signature: String,
    timestamp: String,
    nonce: String,
}

fn show_channel(msg: xml::UniversMessage) -> String {
    let owner = msg.from.clone().unwrap();
    let mut channel_info: String = String::new();
    let channels = channel::INTERFACE.get_channel_by_owner(&owner).unwrap();
    if channels.is_empty() {
        channel_info.push_str("没有创建的频道");
    } else {
        for channel in channels {
            let mut subscribers = String::new();
            match channel::INTERFACE.get_subscribers(&channel.id) {
                Ok(users) => {
                    for user in users {
                        subscribers.push_str(&format!("{}({}) ", &user.name, &user.id));
                    }
                }
                _err => ()
            }
            channel_info.push_str(&format!(
r#"频道名:{}
频道ID:{}
SendKey:{}
订阅者:{}
"#,
&channel.name, &channel.id, &channel.sendkey, &subscribers));
            debug!("{}", &channel_info);
        }
    }
    xml::gen_message_reply(&msg.from.unwrap(), &msg.to.unwrap(), &channel_info)
}

fn show_subscribe(msg: xml::UniversMessage) -> String {
    let owner = msg.from.clone().unwrap();
    let user = user::INTERFACE.get_user(&owner).unwrap();
    let mut channel_infos: String = String::new();
    if user.subscribes.is_empty() {
        channel_infos.push_str("没有订阅的频道");
    } else {
        for channel in user.subscribes {
            match channel::INTERFACE.get_channel_by_id(&channel) {
                Ok(chn) => {
                    channel_infos.push_str(&format!(
r#"频道名:{}
频道ID:{}
"#,
&chn.name, &chn.id));
                }
                _err => ()
            }
            debug!("{}", &channel_infos);
        }
    }
    xml::gen_message_reply(&msg.from.unwrap(), &msg.to.unwrap(), &channel_infos)
}

fn del_channel(msg: xml::UniversMessage) -> String {
    let content = msg.content.unwrap();
    let v: Vec<&str> = content.as_str().splitn(3, ' ').collect();
    if v.len() != 3 {
        xml::gen_message_reply(&msg.from.unwrap(), &msg.to.unwrap(), "格式不对")
    } else {
        let owner = msg.from.clone().unwrap();
        match channel::INTERFACE.delete_channel(v[2], &owner) {
            Ok(_) => xml::gen_message_reply(&owner, &msg.to.unwrap(), "操作成功"),
            Err(err) => xml::gen_message_reply(&msg.from.unwrap(), &msg.to.unwrap(), err)
        }
    }
}
fn add_channel(msg: xml::UniversMessage) -> String {
    let content = msg.content.unwrap();
    let v: Vec<&str> = content.as_str().splitn(3, ' ').collect();
    if v.len() != 3 {
        xml::gen_message_reply(&msg.from.unwrap(), &msg.to.unwrap(), "格式不对")
    } else {
        let owner = msg.from.clone().unwrap();
        match channel::INTERFACE.add_channel(v[2], &owner) {
            Ok(cid) => xml::gen_message_reply(&owner, &msg.to.unwrap(), &format!("操作成功,id:{}", &cid)),
            Err(err) => xml::gen_message_reply(&msg.from.unwrap(), &msg.to.unwrap(), err)
        }
    }
}

fn do_subscribe(msg: xml::UniversMessage) -> String {
    let content = msg.content.unwrap();
    let v: Vec<&str> = content.as_str().splitn(2, ' ').collect();
    if v.len() != 2 {
        xml::gen_message_reply(&msg.from.unwrap(), &msg.to.unwrap(), "格式不对")
    } else {
        let owner = msg.from.clone().unwrap();
        match channel::INTERFACE.subscribe(v[1], &owner) {
            Ok(_) => xml::gen_message_reply(&owner, &msg.to.unwrap(), "操作成功"),
            Err(err) => xml::gen_message_reply(&msg.from.unwrap(), &msg.to.unwrap(), err)
        }
    }
}

fn do_unsubscribe(msg: xml::UniversMessage) -> String {
    let content = msg.content.unwrap();
    let v: Vec<&str> = content.as_str().splitn(2, ' ').collect();
    if v.len() != 2 {
        xml::gen_message_reply(&msg.from.unwrap(), &msg.to.unwrap(), "格式不对")
    } else {
        let owner = msg.from.clone().unwrap();
        match channel::INTERFACE.unsubscribe(v[1], &owner) {
            Ok(_) => xml::gen_message_reply(&owner, &msg.to.unwrap(), "操作成功"),
            Err(err) => xml::gen_message_reply(&msg.from.unwrap(), &msg.to.unwrap(), err)
        }
    }
}

fn wx_post(query: web::Query<AuthInfo>, message: String) -> impl Responder {
    debug!("POST /wx");
    let signature = &query.signature;
    let timestamp = &query.timestamp;
    let nonce = &query.nonce;
    if !wx_interface::check_signature(signature, timestamp, nonce) {
        debug!("auth failed!");
        return HttpResponse::Forbidden().finish();
    }
    debug!("auth pass!");
    debug!("msg:{}", message);
    let msg = xml::parse_message(&message);
    let msg2 = msg.clone();
    match msg2.msg_type {
        Some(msg_type) => {
            debug!("msg_type:{}", msg_type);
            match msg_type.as_str() {
                "text" => {
                    let content = msg2.content.unwrap();
                    if content == "help" {
                        HttpResponse::Ok()
                            .body(xml::gen_message_reply(
                                    &msg.from.unwrap(),
                                    &msg.to.unwrap(),
                                    &CONFIG.help))
                    } else if content.as_str().starts_with("show channel") {
                        HttpResponse::Ok().body(show_channel(msg))
                    } else if content.as_str().starts_with("show subscribe") {
                        HttpResponse::Ok().body(show_subscribe(msg))
                    } else if content.as_str().starts_with("del channel") {
                        HttpResponse::Ok().body(del_channel(msg))
                    } else if content.as_str().starts_with("create channel") {
                        HttpResponse::Ok().body(add_channel(msg))
                    } else if content.as_str().starts_with("subscribe") {
                        HttpResponse::Ok().body(do_subscribe(msg))
                    } else if content.as_str().starts_with("unsubscribe") {
                        HttpResponse::Ok().body(do_unsubscribe(msg))
                    } else {
                        HttpResponse::Ok()
                            .body(xml::gen_message_reply(
                                    &msg.from.unwrap(),
                                    &msg.to.unwrap(),
                                    &content))
                    }
                }
                "event" => {
                    match msg.event.unwrap().as_str() {
                        "subscribe" => {
                            let uid = msg.from.unwrap().clone();
                            match user::INTERFACE.get_user(&uid) {
                                Ok(_user) => (),
                                Err(_err) => {
                                    let _ = user::INTERFACE.add_user(&uid);
                                }
                            }
                            HttpResponse::Ok()
                                .body(xml::gen_message_reply(
                                        &uid,
                                        &msg.to.unwrap(), &CONFIG.welcome))
                        }
                        _ => HttpResponse::Ok().finish()
                    }
                }
                _ => HttpResponse::Ok().finish()
            }
        }
        _ => HttpResponse::Ok().finish()
    }
}



fn main() {
    // 参数处理
    let matches = clap::App::new("Server Tan")
        .version(crate_version!())
        .author("Chinuno Usami. <usami@chinuno.com>")
        .about("Wechat notify service")
        .args_from_usage("-c, --config=[FILE] 'Sets a custom config file'")
        .get_matches();

    if let Some(c) = matches.value_of("config") {
        debug!("Value for config: {}", c);
        *CONFIG_FILE.lock().unwrap() = c.to_string();
    }

    // 初始化日志
    init_log();

    info!("Listening on http://{}", CONFIG.listen);


    HttpServer::new(|| {
        App::new()
            .route( "/wx", web::get().to(wx_auth))
            .route( "/wx", web::post().to(wx_post))
            .route( "/sub", web::get().to(wx_sub))
            .route( "/content/{id}", web::get().to(show_content))
    })
    .bind(&CONFIG.listen)
    .unwrap()
    .run()
    .unwrap();
}

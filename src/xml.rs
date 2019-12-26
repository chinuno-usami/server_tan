use chrono;

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::io::Cursor;

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct UniversMessage {
    pub from: Option<String>,
    pub to: Option<String>,
    pub content: Option<String>,
    pub msg_type: Option<String>,
    pub event: Option<String>,
    pub event_key: Option<String>,
}

impl UniversMessage {
    pub fn new() -> UniversMessage {
        UniversMessage {
            from: None,
            to: None,
            content: None,
            msg_type: None,
            event: None,
            event_key: None,
        }
    }
}

pub fn parse_message(xml: &str) -> UniversMessage {
    let mut ret = UniversMessage::new();
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();

    let mut tag: String = String::new();
    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => tag = reader.decode(e.name()).to_string(),
            Ok(Event::CData(data)) => {
                let value = reader.decode(&data);
                debug!("{}:{:?}", tag, value);
                if tag == "MsgType" {
                    ret.msg_type = Some(value.to_string());
                } else if tag == "Content" {
                    ret.content = Some(value.to_string());
                } else if tag == "ToUserName" {
                    ret.to = Some(value.to_string());
                } else if tag == "FromUserName" {
                    ret.from = Some(value.to_string());
                } else if tag == "Event" {
                    ret.event = Some(value.to_string());
                } else if tag == "EventKey" {
                    ret.event_key = Some(value.to_string());
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
            _ => (),
        }
        buf.clear();
    }
    ret
}

pub fn gen_message_reply(to: &str, from: &str, content: &str) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    let tag_xml_start = BytesStart::owned(b"xml".to_vec(), "xml".len());
    writer.write_event(Event::Start(tag_xml_start)).unwrap();

    let tag_to_start = BytesStart::owned(b"ToUserName".to_vec(), "ToUserName".len());
    let tag_to_end = BytesEnd::borrowed(b"ToUserName");
    let text_to = BytesText::from_escaped_str(to);
    writer.write_event(Event::Start(tag_to_start)).unwrap();
    writer.write_event(Event::CData(text_to)).unwrap();
    writer.write_event(Event::End(tag_to_end)).unwrap();

    let tag_from_start = BytesStart::owned(b"FromUserName".to_vec(), "FromUserName".len());
    let tag_from_end = BytesEnd::borrowed(b"FromUserName");
    let text_from = BytesText::from_escaped_str(from);
    writer.write_event(Event::Start(tag_from_start)).unwrap();
    writer.write_event(Event::CData(text_from)).unwrap();
    writer.write_event(Event::End(tag_from_end)).unwrap();

    let tag_time_start = BytesStart::owned(b"CreateTime".to_vec(), "CreateTime".len());
    let tag_time_end = BytesEnd::borrowed(b"CreateTime");
    let time = chrono::Utc::now().timestamp().to_string();
    let text_time = BytesText::from_escaped_str(&time);
    writer.write_event(Event::Start(tag_time_start)).unwrap();
    writer.write_event(Event::Text(text_time)).unwrap();
    writer.write_event(Event::End(tag_time_end)).unwrap();

    let tag_type_start = BytesStart::owned(b"MsgType".to_vec(), "MsgType".len());
    let tag_type_end = BytesEnd::borrowed(b"MsgType");
    let text_type = BytesText::from_escaped_str("text");
    writer.write_event(Event::Start(tag_type_start)).unwrap();
    writer.write_event(Event::CData(text_type)).unwrap();
    writer.write_event(Event::End(tag_type_end)).unwrap();

    let tag_content_start = BytesStart::owned(b"Content".to_vec(), "Content".len());
    let tag_content_end = BytesEnd::borrowed(b"Content");
    let text_content = BytesText::from_escaped_str(content);
    writer.write_event(Event::Start(tag_content_start)).unwrap();
    writer.write_event(Event::CData(text_content)).unwrap();
    writer.write_event(Event::End(tag_content_end)).unwrap();

    let tag_xml_end = BytesEnd::borrowed(b"xml");
    writer.write_event(Event::End(tag_xml_end)).unwrap();

    let result = writer.into_inner().into_inner();
    String::from_utf8(result).unwrap()
}

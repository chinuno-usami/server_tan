use config::ConfigError;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub appid: String,
    pub secret: String,
    pub token: String,
    pub db_path: String,
    pub welcome: String,
    pub help: String,
    pub template_id: String,
    pub host: String,
    pub detail_template: String,
    pub content_expire: u32,
    pub listen: String,
}

impl Config {
    pub fn new(path: &str) -> Result<Self, ConfigError> {
        let mut settings = config::Config::default();
        match settings.merge(config::File::with_name(path)) {
            Ok(_) => settings.try_into(),
            Err(err) => Err(err),
        }
    }
}

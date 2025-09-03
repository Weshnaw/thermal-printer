use alloc::sync::Arc;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, rwlock::RwLock};

static CONFIG: RwLock<CriticalSectionRawMutex, Config> = RwLock::new(Config::new());

pub struct Config {
    mqtt_user: Option<Arc<str>>,
    mqtt_password: Option<Arc<str>>,
    wifi_ssid: Option<Arc<str>>,
    wifi_password: Option<Arc<str>>,
}

pub struct ConfigUpdater {
    mqtt_user: Option<Arc<str>>,
    mqtt_password: Option<Arc<str>>,
    wifi_ssid: Option<Arc<str>>,
    wifi_password: Option<Arc<str>>,
}

impl ConfigUpdater {
    const fn new() -> Self {
        Self {
            mqtt_user: Option::None,
            mqtt_password: Option::None,
            wifi_ssid: Option::None,
            wifi_password: Option::None,
        }
    }

    pub fn mqtt_user(mut self, new: impl Into<Arc<str>>) -> Self {
        self.mqtt_user = Option::Some(new.into());
        self
    }

    pub fn mqtt_password(mut self, new: impl Into<Arc<str>>) -> Self {
        self.mqtt_password = Option::Some(new.into());
        self
    }

    pub fn wifi_ssid(mut self, new: impl Into<Arc<str>>) -> Self {
        self.wifi_ssid = Option::Some(new.into());
        self
    }

    pub fn wifi_password(mut self, new: impl Into<Arc<str>>) -> Self {
        self.wifi_password = Option::Some(new.into());
        self
    }

    pub async fn update(self) {
        let mut config = CONFIG.write().await;
        if self.mqtt_user.is_some() {
            config.mqtt_user = self.mqtt_user;
        }
        if self.mqtt_password.is_some() {
            config.mqtt_password = self.mqtt_password;
        }
        if self.wifi_ssid.is_some() {
            config.wifi_ssid = self.wifi_ssid;
        }
        if self.wifi_password.is_some() {
            config.wifi_password = self.wifi_password;
        }
    }
}

impl Config {
    const fn new() -> Self {
        Self {
            mqtt_user: Option::None,
            mqtt_password: Option::None,
            wifi_ssid: Option::None,
            wifi_password: Option::None,
        }
    }

    pub fn update() -> ConfigUpdater {
        ConfigUpdater::new()
    }

    pub async fn mqtt_user() -> Arc<str> {
        CONFIG.read().await.mqtt_user.clone().unwrap_or("".into())
    }

    pub async fn mqtt_password() -> Arc<str> {
        CONFIG
            .read()
            .await
            .mqtt_password
            .clone()
            .unwrap_or("".into())
    }

    pub async fn wifi_ssid() -> Arc<str> {
        CONFIG.read().await.wifi_ssid.clone().unwrap_or("".into())
    }

    pub async fn wifi_password() -> Arc<str> {
        CONFIG
            .read()
            .await
            .wifi_password
            .clone()
            .unwrap_or("".into())
    }
}

const MQTT_USER: &str = env!("MQTT_USER");
const MQTT_PASSWORD: &str = env!("MQTT_PASSWORD");
const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");

pub async fn setup_config_from_env() {
    Config::update()
        .mqtt_user(MQTT_USER)
        .mqtt_password(MQTT_PASSWORD)
        .wifi_ssid(WIFI_SSID)
        .wifi_password(WIFI_PASSWORD)
        .update()
        .await;
}

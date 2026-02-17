use core::convert::TryInto;

use embedded_svc::wifi::{
    AccessPointConfiguration, AuthMethod, ClientConfiguration, Configuration,
};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::modem::Modem;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};

const WIFI_SETTINGS_PATH: &str = "/sd/.xteink/wifi.tsv";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiMode {
    AccessPoint,
    Station,
}

impl WifiMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AccessPoint => "ap",
            Self::Station => "sta",
        }
    }

    pub fn from_str(raw: &str) -> Option<Self> {
        match raw {
            "ap" | "access-point" => Some(Self::AccessPoint),
            "sta" | "station" => Some(Self::Station),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::AccessPoint => "Hotspot",
            Self::Station => "Wi-Fi",
        }
    }
}

#[derive(Debug, Clone)]
pub struct WifiTransferInfo {
    pub mode: String,
    pub ssid: String,
    pub password_hint: String,
    pub url: String,
    pub message: String,
}

impl Default for WifiTransferInfo {
    fn default() -> Self {
        Self {
            mode: String::from("Hotspot"),
            ssid: String::new(),
            password_hint: String::new(),
            url: String::new(),
            message: String::from("Configure via CLI: wifi ap <ssid> [password]"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WifiSettings {
    pub mode: WifiMode,
    pub ap_ssid: String,
    pub ap_password: String,
    pub sta_ssid: String,
    pub sta_password: String,
}

impl Default for WifiSettings {
    fn default() -> Self {
        Self {
            mode: WifiMode::AccessPoint,
            ap_ssid: String::from("Xteink-X4"),
            ap_password: String::from("xteink2026"),
            sta_ssid: String::new(),
            sta_password: String::new(),
        }
    }
}

pub struct WifiManager {
    modem: Option<Modem>,
    sys_loop: EspSystemEventLoop,
    nvs: Option<EspDefaultNvsPartition>,
    wifi: Option<BlockingWifi<EspWifi<'static>>>,
    settings: WifiSettings,
    transfer_info: WifiTransferInfo,
    network_active: bool,
}

impl WifiManager {
    pub fn new(modem: Modem, sys_loop: EspSystemEventLoop) -> Self {
        let mut manager = Self {
            modem: Some(modem),
            sys_loop,
            nvs: EspDefaultNvsPartition::take().ok(),
            wifi: None,
            settings: WifiSettings::default(),
            transfer_info: WifiTransferInfo::default(),
            network_active: false,
        };
        let _ = manager.load_settings_from_disk();
        manager
    }

    pub fn settings(&self) -> &WifiSettings {
        &self.settings
    }

    pub fn is_network_active(&self) -> bool {
        self.network_active
    }

    pub fn transfer_info(&self) -> WifiTransferInfo {
        self.transfer_info.clone()
    }

    pub fn set_mode(&mut self, mode: WifiMode) -> Result<(), String> {
        self.settings.mode = mode;
        self.save_settings_to_disk()
    }

    pub fn configure_ap(&mut self, ssid: String, password: String) -> Result<(), String> {
        if ssid.trim().is_empty() {
            return Err(String::from("AP SSID is empty"));
        }
        if !password.is_empty() && password.len() < 8 {
            return Err(String::from("AP password must be 8+ chars or empty"));
        }
        self.settings.ap_ssid = ssid;
        self.settings.ap_password = password;
        self.settings.mode = WifiMode::AccessPoint;
        self.save_settings_to_disk()
    }

    pub fn configure_sta(&mut self, ssid: String, password: String) -> Result<(), String> {
        if ssid.trim().is_empty() {
            return Err(String::from("STA SSID is empty"));
        }
        self.settings.sta_ssid = ssid;
        self.settings.sta_password = password;
        self.settings.mode = WifiMode::Station;
        self.save_settings_to_disk()
    }

    pub fn clear_sta(&mut self) -> Result<(), String> {
        self.settings.sta_ssid.clear();
        self.settings.sta_password.clear();
        self.save_settings_to_disk()
    }

    pub fn start_transfer_network(&mut self) -> Result<(), String> {
        match self.settings.mode {
            WifiMode::AccessPoint => self.start_access_point(),
            WifiMode::Station => self.start_station(),
        }
    }

    pub fn stop_transfer_network(&mut self) {
        if let Some(wifi) = self.wifi.as_mut() {
            let _ = wifi.disconnect();
            let _ = wifi.stop();
        }
        self.network_active = false;
        self.transfer_info = WifiTransferInfo {
            mode: self.settings.mode.label().to_string(),
            ssid: String::new(),
            password_hint: String::new(),
            url: String::new(),
            message: String::from("Network stopped"),
        };
    }

    fn ensure_wifi(&mut self) -> Result<&mut BlockingWifi<EspWifi<'static>>, String> {
        if self.wifi.is_none() {
            let Some(modem) = self.modem.take() else {
                return Err(String::from("Wi-Fi modem unavailable"));
            };
            let esp_wifi = EspWifi::new(modem, self.sys_loop.clone(), self.nvs.take())
                .map_err(|err| format!("wifi init failed: {}", err))?;
            let blocking = BlockingWifi::wrap(esp_wifi, self.sys_loop.clone())
                .map_err(|err| format!("wifi wrapper init failed: {}", err))?;
            self.wifi = Some(blocking);
        }
        self.wifi
            .as_mut()
            .ok_or_else(|| String::from("wifi init failed"))
    }

    fn start_access_point(&mut self) -> Result<(), String> {
        let ssid = self.settings.ap_ssid.trim().to_string();
        if ssid.is_empty() {
            return Err(String::from("AP SSID is empty"));
        }

        let ssid_h = ssid
            .as_str()
            .try_into()
            .map_err(|_| String::from("AP SSID too long (max 32)"))?;

        let mut password_hint = String::from("Open network");
        let mut auth_method = AuthMethod::None;
        let mut password_h = Default::default();
        let password = self.settings.ap_password.trim().to_string();
        if !password.is_empty() {
            if password.len() < 8 {
                return Err(String::from("AP password must be 8+ chars or empty"));
            }
            auth_method = AuthMethod::WPA2Personal;
            password_h = password
                .as_str()
                .try_into()
                .map_err(|_| String::from("AP password too long (max 64)"))?;
            password_hint = format!("Password: {}", password);
        }

        let wifi = self.ensure_wifi()?;
        let conf = Configuration::AccessPoint(AccessPointConfiguration {
            ssid: ssid_h,
            ssid_hidden: false,
            channel: 6,
            secondary_channel: None,
            auth_method,
            password: password_h,
            max_connections: 4,
            ..Default::default()
        });

        wifi.set_configuration(&conf)
            .map_err(|err| format!("wifi ap config failed: {}", err))?;
        wifi.start()
            .map_err(|err| format!("wifi ap start failed: {}", err))?;
        wifi.wait_netif_up()
            .map_err(|err| format!("wifi ap netif up failed: {}", err))?;

        let ip = wifi
            .wifi()
            .ap_netif()
            .get_ip_info()
            .map_err(|err| format!("wifi ap ip failed: {}", err))?
            .ip;
        let ip_str = ip.to_string();
        self.network_active = true;
        self.transfer_info = WifiTransferInfo {
            mode: String::from("Hotspot"),
            ssid,
            password_hint,
            url: format!("http://{}/", ip_str),
            message: String::from("Connect your phone/PC to this hotspot"),
        };
        Ok(())
    }

    fn start_station(&mut self) -> Result<(), String> {
        let ssid = self.settings.sta_ssid.trim().to_string();
        if ssid.is_empty() {
            return Err(String::from("STA SSID is empty"));
        }

        let ssid_h = ssid
            .as_str()
            .try_into()
            .map_err(|_| String::from("STA SSID too long (max 32)"))?;

        let password = self.settings.sta_password.trim().to_string();
        let (auth_method, password_h) = if password.is_empty() {
            (AuthMethod::None, Default::default())
        } else {
            (
                AuthMethod::WPA2Personal,
                password
                    .as_str()
                    .try_into()
                    .map_err(|_| String::from("STA password too long (max 64)"))?,
            )
        };

        let wifi = self.ensure_wifi()?;
        let conf = Configuration::Client(ClientConfiguration {
            ssid: ssid_h,
            bssid: None,
            auth_method,
            password: password_h,
            channel: None,
            ..Default::default()
        });

        wifi.set_configuration(&conf)
            .map_err(|err| format!("wifi sta config failed: {}", err))?;
        wifi.start()
            .map_err(|err| format!("wifi sta start failed: {}", err))?;
        wifi.connect()
            .map_err(|err| format!("wifi sta connect failed: {}", err))?;
        wifi.wait_netif_up()
            .map_err(|err| format!("wifi sta netif up failed: {}", err))?;

        let ip = wifi
            .wifi()
            .sta_netif()
            .get_ip_info()
            .map_err(|err| format!("wifi sta ip failed: {}", err))?
            .ip;
        let ip_str = ip.to_string();
        self.network_active = true;
        self.transfer_info = WifiTransferInfo {
            mode: String::from("Wi-Fi"),
            ssid,
            password_hint: String::new(),
            url: format!("http://{}/", ip_str),
            message: String::from("Connected to network"),
        };

        Ok(())
    }

    fn escape_field(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for ch in input.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '\t' => out.push_str("\\t"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                _ => out.push(ch),
            }
        }
        out
    }

    fn unescape_field(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        let mut chars = input.chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                match chars.next() {
                    Some('t') => out.push('\t'),
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('\\') => out.push('\\'),
                    Some(other) => {
                        out.push('\\');
                        out.push(other);
                    }
                    None => out.push('\\'),
                }
            } else {
                out.push(ch);
            }
        }
        out
    }

    fn load_settings_from_disk(&mut self) -> Result<(), String> {
        let Ok(raw) = std::fs::read_to_string(WIFI_SETTINGS_PATH) else {
            return Ok(());
        };
        let mut lines = raw.lines();
        if lines.next() != Some("v1") {
            return Ok(());
        }
        let Some(line) = lines.next() else {
            return Ok(());
        };
        let mut parts = line.split('\t');
        let Some(mode_raw) = parts.next() else {
            return Ok(());
        };
        let Some(ap_ssid) = parts.next() else {
            return Ok(());
        };
        let Some(ap_password) = parts.next() else {
            return Ok(());
        };
        let Some(sta_ssid) = parts.next() else {
            return Ok(());
        };
        let Some(sta_password) = parts.next() else {
            return Ok(());
        };
        self.settings.mode = WifiMode::from_str(mode_raw).unwrap_or(WifiMode::AccessPoint);
        self.settings.ap_ssid = Self::unescape_field(ap_ssid);
        self.settings.ap_password = Self::unescape_field(ap_password);
        self.settings.sta_ssid = Self::unescape_field(sta_ssid);
        self.settings.sta_password = Self::unescape_field(sta_password);
        Ok(())
    }

    fn save_settings_to_disk(&self) -> Result<(), String> {
        if let Some(parent) = std::path::Path::new(WIFI_SETTINGS_PATH).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("wifi settings dir create failed: {}", err))?;
        }
        let line = format!(
            "{}\t{}\t{}\t{}\t{}\n",
            self.settings.mode.as_str(),
            Self::escape_field(&self.settings.ap_ssid),
            Self::escape_field(&self.settings.ap_password),
            Self::escape_field(&self.settings.sta_ssid),
            Self::escape_field(&self.settings.sta_password),
        );
        let mut out = String::from("v1\n");
        out.push_str(&line);
        std::fs::write(WIFI_SETTINGS_PATH, out)
            .map_err(|err| format!("wifi settings write failed: {}", err))
    }

    pub fn masked_password(input: &str) -> String {
        if input.is_empty() {
            return String::from("(none)");
        }
        let visible = input.chars().count().min(2);
        let mut out = String::new();
        for _ in 0..visible {
            out.push('*');
        }
        out.push_str("...");
        out
    }
}

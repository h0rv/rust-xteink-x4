use core::convert::TryInto;

use embedded_svc::wifi::{
    AccessPointConfiguration, AuthMethod, ClientConfiguration, Configuration,
};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::modem::Modem;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiMode {
    AccessPoint,
    Station,
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
}

impl WifiManager {
    pub fn new(modem: Modem, sys_loop: EspSystemEventLoop) -> Self {
        Self {
            modem: Some(modem),
            sys_loop,
            nvs: EspDefaultNvsPartition::take().ok(),
            wifi: None,
            settings: WifiSettings::default(),
            transfer_info: WifiTransferInfo::default(),
        }
    }

    pub fn settings(&self) -> &WifiSettings {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut WifiSettings {
        &mut self.settings
    }

    pub fn transfer_info(&self) -> WifiTransferInfo {
        self.transfer_info.clone()
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
        self.transfer_info = WifiTransferInfo {
            mode: match self.settings.mode {
                WifiMode::AccessPoint => String::from("Hotspot"),
                WifiMode::Station => String::from("Wi-Fi"),
            },
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
        let ssid = self.settings.ap_ssid.trim();
        if ssid.is_empty() {
            return Err(String::from("AP SSID is empty"));
        }

        let ssid_h: heapless::String<32> = ssid
            .try_into()
            .map_err(|_| String::from("AP SSID too long (max 32)"))?;

        let mut password_hint = String::from("Open network");
        let mut auth_method = AuthMethod::None;
        let mut password_h: heapless::String<64> = heapless::String::new();
        let password = self.settings.ap_password.trim();
        if !password.is_empty() {
            if password.len() < 8 {
                return Err(String::from("AP password must be 8+ chars or empty"));
            }
            auth_method = AuthMethod::WPA2Personal;
            password_h = password
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
        self.transfer_info = WifiTransferInfo {
            mode: String::from("Hotspot"),
            ssid: ssid.to_string(),
            password_hint,
            url: format!("http://{}/", ip_str),
            message: String::from("Connect your phone/PC to this hotspot"),
        };
        Ok(())
    }

    fn start_station(&mut self) -> Result<(), String> {
        let ssid = self.settings.sta_ssid.trim();
        if ssid.is_empty() {
            return Err(String::from("STA SSID is empty"));
        }

        let ssid_h: heapless::String<32> = ssid
            .try_into()
            .map_err(|_| String::from("STA SSID too long (max 32)"))?;

        let password = self.settings.sta_password.trim();
        let (auth_method, password_h): (AuthMethod, heapless::String<64>) = if password.is_empty() {
            (AuthMethod::None, heapless::String::new())
        } else {
            (
                AuthMethod::WPA2Personal,
                password
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
        self.transfer_info = WifiTransferInfo {
            mode: String::from("Wi-Fi"),
            ssid: ssid.to_string(),
            password_hint: String::new(),
            url: format!("http://{}/", ip_str),
            message: String::from("Connected to network"),
        };

        Ok(())
    }
}

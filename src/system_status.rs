//! Read-only system Wi-Fi/power snapshots and event subscriptions. No polling.
use crate::native::{self, Obj};
use objc2::{class, msg_send, runtime::AnyObject, sel};
use std::ffi::c_void;

type Ptr = *const c_void;
#[link(name = "CoreWLAN", kind = "framework")]
unsafe extern "C" {}
#[link(name = "SystemConfiguration", kind = "framework")]
unsafe extern "C" {
    fn SCDynamicStoreCopyValue(store: Ptr, key: *const AnyObject) -> *mut AnyObject;
}
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPSCopyPowerSourcesInfo() -> *mut AnyObject;
    fn IOPSCopyPowerSourcesList(info: *const AnyObject) -> *mut AnyObject;
    fn IOPSGetPowerSourceDescription(
        info: *const AnyObject,
        source: *const AnyObject,
    ) -> *mut AnyObject;
    fn IOPSNotificationCreateRunLoopSource(
        callback: extern "C" fn(*mut c_void),
        context: *mut c_void,
    ) -> Ptr;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRunLoopGetMain() -> Ptr;
    fn CFRunLoopAddSource(run_loop: Ptr, source: Ptr, mode: Ptr);
    fn CFRunLoopRemoveSource(run_loop: Ptr, source: Ptr, mode: Ptr);
    fn CFRunLoopSourceInvalidate(source: Ptr);
    fn CFRelease(value: Ptr);
    static kCFRunLoopCommonModes: Ptr;
}

#[derive(Clone, Debug, PartialEq)]
pub enum Battery {
    None,
    Unavailable,
    Internal {
        percent: Option<f64>,
        charging: bool,
        external: bool,
        full: bool,
        minutes: Option<i64>,
    },
}
impl Battery {
    pub fn progress(&self) -> Option<f64> {
        match self {
            Self::None => Some(1.),
            Self::Unavailable => None,
            Self::Internal { percent, .. } => percent.map(|v| v / 100.),
        }
    }
    pub fn percent_label(&self) -> Option<String> {
        match self {
            Self::None => None,
            Self::Internal {
                percent: Some(p), ..
            } => Some(format!("{p:.0}%")),
            _ => Some("—".into()),
        }
    }
    pub fn charging(&self) -> bool {
        matches!(self, Self::Internal { charging: true, .. })
    }
    pub fn show_bolt(&self) -> bool {
        matches!(
            self,
            Self::Internal { external: true, .. } | Self::Internal { charging: true, .. }
        )
    }
    pub fn low(&self) -> bool {
        matches!(self, Self::Internal { percent: Some(p), external: false, charging: false, .. } if *p <= 20.)
    }
    pub fn summary(&self) -> String {
        match self {
            Self::None => "外接电源 · 无内置电池".into(),
            Self::Unavailable => "电源信息暂不可用".into(),
            Self::Internal {
                percent,
                charging,
                external,
                full,
                ..
            } => {
                let amount = percent
                    .map(|p| format!("{p:.0}%"))
                    .unwrap_or_else(|| "电量未知".into());
                let state = if *charging {
                    "正在充电"
                } else if *full {
                    "已充满"
                } else if *external {
                    "已接电源 · 未充电"
                } else {
                    "使用电池"
                };
                format!("{amount} · {state}")
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum WifiState {
    Unavailable,
    Unknown,
    Off,
    Disconnected,
    Connected,
    Sharing,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Wifi {
    pub state: WifiState,
    pub ssid: Option<String>,
    pub rssi: Option<i64>,
    pub noise: Option<i64>,
    pub rate: Option<f64>,
    pub channel: Option<i64>,
}
impl Wifi {
    pub fn summary(&self) -> &str {
        match self.state {
            WifiState::Unavailable => "不可用",
            WifiState::Unknown => "状态未知",
            WifiState::Off => "已关闭",
            WifiState::Disconnected => "未连接",
            WifiState::Connected => "已连接",
            WifiState::Sharing => "共享网络",
        }
    }
    pub fn strength(&self) -> f64 {
        match self.state {
            WifiState::Connected | WifiState::Sharing => match self.rssi {
                Some(r) if r < -80 => 0.33,
                Some(r) if r < -65 => 0.66,
                _ => 1.,
            },
            _ => 0.,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub wifi: Wifi,
    pub battery: Battery,
    pub bluetooth: crate::bluetooth::State,
}
impl Snapshot {
    pub fn read() -> Self {
        Self {
            wifi: read_wifi(),
            battery: read_battery(),
            bluetooth: crate::bluetooth::read(),
        }
    }
    pub fn accessibility(&self) -> String {
        format!(
            "Ashmactool，Wi-Fi {}，{}，蓝牙{}。点击查看网络、电源与工具。",
            self.wifi.summary(),
            self.battery.summary(),
            self.bluetooth.summary()
        )
    }
    pub fn fixture(battery: Battery, state: WifiState) -> Self {
        Self {
            wifi: Wifi {
                state,
                ssid: None,
                rssi: Some(-52),
                noise: None,
                rate: None,
                channel: None,
            },
            battery,
            bluetooth: crate::bluetooth::State::NotConnected,
        }
    }
}
fn value(d: &AnyObject, key: &str) -> Option<String> {
    native::get(d, key)
        .filter(|v| native::kind(v, c"NSString"))
        .map(|v| native::text(&v))
}
fn flag(d: &AnyObject, key: &str) -> bool {
    native::get(d, key)
        .filter(|v| native::kind(v, c"NSNumber"))
        .is_some_and(|v| unsafe { msg_send![&v, boolValue] })
}
fn battery_from_sources(sources: &[Obj]) -> Battery {
    for d in sources {
        if value(d, "Type").as_deref() != Some("InternalBattery") {
            continue;
        }
        if native::get(d, "Is Present").is_some() && !flag(d, "Is Present") {
            continue;
        }
        let current = native::numeric(d, "Current Capacity").ok();
        let maximum = native::numeric(d, "Max Capacity").ok();
        let percent = current
            .zip(maximum)
            .filter(|(c, m)| c.is_finite() && *c >= 0. && m.is_finite() && *m > 0.)
            .map(|(c, m)| (100. * (c / m)).clamp(0., 100.));
        let charging = flag(d, "Is Charging");
        let minutes = native::numeric(
            d,
            if charging {
                "Time to Full Charge"
            } else {
                "Time to Empty"
            },
        )
        .ok()
        .filter(|v| v.is_finite() && *v > 0. && *v < 24. * 60.)
        .map(|v| v as i64);
        return Battery::Internal {
            percent,
            charging,
            external: value(d, "Power Source State").as_deref() == Some("AC Power"),
            full: flag(d, "Is Charged"),
            minutes,
        };
    }
    Battery::None
}
fn read_battery() -> Battery {
    unsafe {
        let Some(info) = Obj::from_raw(IOPSCopyPowerSourcesInfo()) else {
            return Battery::Unavailable;
        };
        let Some(list) = Obj::from_raw(IOPSCopyPowerSourcesList(&*info)) else {
            return Battery::Unavailable;
        };
        let mut descriptions = Vec::new();
        for i in 0..native::count(&list) {
            let source = native::at(&list, i);
            let p = IOPSGetPowerSourceDescription(&*info, &*source);
            if p.is_null() {
                return Battery::Unavailable;
            }
            // Get returns a borrowed dictionary. Retain it while the snapshot is parsed.
            descriptions.push(Obj::retain(p).expect("non-null power source"));
        }
        battery_from_sources(&descriptions)
    }
}
fn wifi_state(mode: isize, link: Option<bool>, association_evidence: bool) -> WifiState {
    if mode == 3 {
        WifiState::Sharing
    } else {
        match link {
            Some(true) => WifiState::Connected,
            Some(false) => WifiState::Disconnected,
            None if association_evidence => WifiState::Connected,
            None if mode == 0 => WifiState::Disconnected,
            None => WifiState::Unknown,
        }
    }
}
fn read_wifi() -> Wifi {
    let mut wifi = Wifi {
        state: WifiState::Unavailable,
        ssid: None,
        rssi: None,
        noise: None,
        rate: None,
        channel: None,
    };
    unsafe {
        let client: Obj = msg_send![class!(CWWiFiClient), sharedWiFiClient];
        let interface: Option<Obj> = msg_send![&client, interface];
        let Some(interface) = interface else {
            return wifi;
        };
        let on: bool = msg_send![&interface, powerOn];
        if !on {
            wifi.state = WifiState::Off;
            return wifi;
        }
        let mode: isize = msg_send![&interface, interfaceMode];
        let channel: Option<Obj> = msg_send![&interface, wlanChannel];
        let ssid: Option<Obj> = msg_send![&interface, ssid];
        let rssi: isize = msg_send![&interface, rssiValue];
        // A redacted SSID is not evidence of disconnection. Mode/channel/RSSI
        // remain useful without requesting or bypassing Location authorization.
        let interface_name: Option<Obj> = msg_send![&interface, interfaceName];
        let link = interface_name.and_then(|name| {
            let key = native::string(&format!(
                "State:/Network/Interface/{}/Link",
                native::text(&name)
            ));
            let description = Obj::from_raw(SCDynamicStoreCopyValue(
                std::ptr::null(),
                objc2::rc::Retained::as_ptr(&key).cast(),
            ))?;
            if native::get(&description, "Active").is_some() {
                Some(flag(&description, "Active"))
            } else {
                None
            }
        });
        wifi.state = wifi_state(
            mode,
            link,
            ssid.is_some() || (channel.is_some() && rssi < 0),
        );
        if matches!(wifi.state, WifiState::Connected | WifiState::Sharing) {
            wifi.ssid = ssid.map(|s| native::text(&s)).filter(|s| !s.is_empty());
            wifi.rssi = (-127..0).contains(&rssi).then_some(rssi as i64);
            let noise: isize = msg_send![&interface, noiseMeasurement];
            wifi.noise = (-127..0).contains(&noise).then_some(noise as i64);
            let rate: f64 = msg_send![&interface, transmitRate];
            wifi.rate = (rate.is_finite() && rate > 0.).then_some(rate);
            wifi.channel = channel.map(|c| {
                let n: isize = msg_send![&c, channelNumber];
                n as i64
            });
        }
    }
    wifi
}

pub struct Monitor {
    client: Obj,
    events: Vec<isize>,
    power_source: Ptr,
    pub warnings: Vec<String>,
    pub bluetooth: crate::bluetooth::Monitor,
}
impl Monitor {
    pub fn new(target: &AnyObject) -> Self {
        unsafe {
            let client: Obj = msg_send![class!(CWWiFiClient), sharedWiFiClient];
            let _: () = msg_send![&client, setDelegate: target];
            let mut events = Vec::new();
            let mut warnings = Vec::new();
            for event in [1isize, 2, 3, 5, 6, 7] {
                let ok: bool = msg_send![&client, startMonitoringEventWithType: event, error: std::ptr::null_mut::<*mut AnyObject>()];
                if ok {
                    events.push(event);
                } else {
                    warnings.push(format!("Wi-Fi 事件 {event} 未订阅，打开菜单时刷新"));
                }
            }
            let power_source =
                IOPSNotificationCreateRunLoopSource(power_changed, std::ptr::null_mut());
            if power_source.is_null() {
                warnings.push("电源事件未订阅，打开菜单时刷新".into());
            } else {
                CFRunLoopAddSource(CFRunLoopGetMain(), power_source, kCFRunLoopCommonModes);
            }
            let bluetooth = crate::bluetooth::Monitor::new(target);
            warnings.extend(bluetooth.warnings.iter().cloned());
            Self {
                bluetooth,
                client,
                events,
                power_source,
                warnings,
            }
        }
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        unsafe {
            for event in &self.events {
                let _: bool = msg_send![&self.client, stopMonitoringEventWithType: *event, error: std::ptr::null_mut::<*mut AnyObject>()];
            }
            let _: () = msg_send![&self.client, setDelegate: std::ptr::null::<AnyObject>()];
            if !self.power_source.is_null() {
                CFRunLoopRemoveSource(CFRunLoopGetMain(), self.power_source, kCFRunLoopCommonModes);
                CFRunLoopSourceInvalidate(self.power_source);
                CFRelease(self.power_source);
            }
        }
    }
}
extern "C" fn power_changed(_: *mut c_void) {
    crate::app::refresh_system_status();
}
/// CoreWLAN invokes delegates off the main thread; only enqueue a main-thread selector here.
pub extern "C-unwind" fn wifi_changed(
    target: &AnyObject,
    _: objc2::runtime::Sel,
    _: *mut AnyObject,
) {
    objc2::rc::autoreleasepool(|_| unsafe {
        let _: () = msg_send![target, performSelectorOnMainThread: sel!(systemChanged:), withObject: std::ptr::null::<AnyObject>(), waitUntilDone: false];
    });
}
pub extern "C-unwind" fn wifi_quality(
    target: &AnyObject,
    selector: objc2::runtime::Sel,
    name: *mut AnyObject,
    _: isize,
    _: f64,
) {
    wifi_changed(target, selector, name);
}

#[cfg(test)]
mod tests {
    #[test]
    fn percentage_distinguishes_missing_battery_unknown_and_zero() {
        assert_eq!(super::Battery::None.percent_label(), None);
        assert_eq!(
            super::Battery::Unavailable.percent_label().as_deref(),
            Some("—")
        );
        for (percent, expected) in [
            (Some(0.), "0%"),
            (Some(79.6), "80%"),
            (Some(100.), "100%"),
            (None, "—"),
        ] {
            let battery = super::Battery::Internal {
                percent,
                charging: false,
                external: false,
                full: false,
                minutes: None,
            };
            assert_eq!(battery.percent_label().as_deref(), Some(expected));
        }
    }

    use super::*;
    use objc2::rc::autoreleasepool;
    #[test]
    fn distinguishes_internal_battery_ups_and_unknown_capacity() {
        autoreleasepool(|_| {
            let ups = native::dictionary();
            native::insert(&ups, "Type", &native::string("UPS"));
            assert_eq!(battery_from_sources(&[ups]), Battery::None);
            let d = native::dictionary();
            native::insert(&d, "Type", &native::string("InternalBattery"));
            native::insert(&d, "Current Capacity", &native::number(82.));
            native::insert(&d, "Max Capacity", &native::number(100.));
            native::insert(&d, "Is Charging", &native::number(1.));
            native::insert(&d, "Power Source State", &native::string("AC Power"));
            let b = battery_from_sources(std::slice::from_ref(&d));
            assert_eq!(b.progress(), Some(0.82));
            assert!(b.charging());
            assert!(!b.low());
            native::insert(&d, "Max Capacity", &native::number(0.));
            assert_eq!(battery_from_sources(&[d]).progress(), None);
            assert_eq!(Battery::Unavailable.progress(), None);
            assert_eq!(Battery::None.progress(), Some(1.));
        });
    }
    #[test]
    fn station_mode_and_redacted_ssid_do_not_determine_connection_alone() {
        assert_eq!(wifi_state(1, Some(true), false), WifiState::Connected);
        assert_eq!(wifi_state(1, Some(false), true), WifiState::Disconnected);
        assert_eq!(wifi_state(1, None, false), WifiState::Unknown);
        assert_eq!(wifi_state(1, None, true), WifiState::Connected);
        assert_eq!(wifi_state(0, None, false), WifiState::Disconnected);
        assert_eq!(wifi_state(3, Some(false), false), WifiState::Sharing);
    }
    #[test]
    fn plugged_in_is_not_always_charging_and_empty_is_not_full() {
        let b = Battery::Internal {
            percent: Some(0.),
            charging: false,
            external: false,
            full: false,
            minutes: None,
        };
        assert_eq!(b.progress(), Some(0.));
        assert!(!b.show_bolt());
        assert!(!Battery::None.show_bolt());
        assert!(b.low());
        assert!(!b.charging());
        let b = Battery::Internal {
            percent: Some(80.),
            charging: false,
            external: true,
            full: false,
            minutes: None,
        };
        assert!(!b.charging());
        assert!(b.show_bolt());
        assert!(b.summary().contains("未充电"));
    }
}

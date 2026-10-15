//! Read connection metadata only: no inquiry, scanning, pairing or input access.
use crate::native::{self, Obj};
use objc2::{
    class, msg_send,
    runtime::{AnyObject, Sel},
    sel,
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    ffi::{c_char, c_void},
};
type Ptr = *const c_void;
#[link(name = "IOBluetooth", kind = "framework")]
unsafe extern "C" {}
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const c_char) -> Ptr;
    fn IOServiceGetMatchingServices(port: u32, matching: Ptr, iterator: *mut u32) -> i32;
    fn IOIteratorNext(iterator: u32) -> u32;
    fn IOObjectRelease(object: u32) -> i32;
    fn IORegistryEntryCreateCFProperty(
        entry: u32,
        key: *const AnyObject,
        allocator: Ptr,
        options: u32,
    ) -> *mut AnyObject;
    fn IONotificationPortCreate(port: u32) -> *mut c_void;
    fn IONotificationPortGetRunLoopSource(port: *mut c_void) -> Ptr;
    fn IONotificationPortDestroy(port: *mut c_void);
    fn IOServiceAddMatchingNotification(
        port: *mut c_void,
        kind: *const c_char,
        matching: Ptr,
        callback: extern "C" fn(*mut c_void, u32),
        context: *mut c_void,
        iterator: *mut u32,
    ) -> i32;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRunLoopGetMain() -> Ptr;
    fn CFRunLoopAddSource(run_loop: Ptr, source: Ptr, mode: Ptr);
    fn CFRunLoopRemoveSource(run_loop: Ptr, source: Ptr, mode: Ptr);
    static kCFRunLoopCommonModes: Ptr;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Connected,
    NotConnected,
    Off,
    Unavailable,
}
impl State {
    pub fn connected(&self) -> bool {
        *self == Self::Connected
    }
    pub fn summary(&self) -> &str {
        match self {
            Self::Connected => "有设备连接",
            Self::NotConnected => "未连接设备",
            Self::Off => "已关闭",
            Self::Unavailable => "状态暂不可用",
        }
    }
}
thread_local! {static EVENT_DEVICES:RefCell<Vec<Obj>>=const{RefCell::new(Vec::new())};}
fn connected(device: &AnyObject) -> bool {
    unsafe { msg_send![device, isConnected] }
}
fn paired() -> Option<Obj> {
    unsafe { msg_send![class!(IOBluetoothDevice), pairedDevices] }
}
fn candidates() -> Vec<Obj> {
    let mut devices = Vec::new();
    if let Some(list) = paired() {
        for i in 0..native::count(&list) {
            devices.push(native::at(&list, i));
        }
    }
    EVENT_DEVICES.with(|s| devices.extend(s.borrow().iter().cloned()));
    devices
}
fn hid_connected() -> Option<bool> {
    unsafe {
        let mut iterator = 0;
        let matching = IOServiceMatching(c"IOHIDDevice".as_ptr());
        if matching.is_null() || IOServiceGetMatchingServices(0, matching, &mut iterator) != 0 {
            return None;
        }
        let mut found = false;
        loop {
            let entry = IOIteratorNext(iterator);
            if entry == 0 {
                break;
            }
            let key = native::string("Transport");
            let value = Obj::from_raw(IORegistryEntryCreateCFProperty(
                entry,
                objc2::rc::Retained::as_ptr(&key).cast(),
                std::ptr::null(),
                0,
            ));
            if let Some(v) = value.filter(|v| native::kind(v, c"NSString")) {
                found |= native::text(&v).starts_with("Bluetooth");
            }
            IOObjectRelease(entry);
        }
        IOObjectRelease(iterator);
        Some(found)
    }
}
pub fn read() -> State {
    unsafe {
        let host: Option<Obj> = msg_send![class!(IOBluetoothHostController), defaultController];
        if let Some(host) = host {
            let power: i32 = msg_send![&host, powerState];
            if power == 0 {
                return State::Off;
            }
        }
        let list = paired();
        let classic = list
            .as_ref()
            .map(|list| (0..native::count(list)).any(|i| connected(&native::at(list, i))));
        let event = EVENT_DEVICES.with(|s| s.borrow().iter().any(|d| connected(d)));
        let hid = hid_connected();
        if classic == Some(true) || event || hid == Some(true) {
            State::Connected
        } else if classic.is_some() && hid.is_some() {
            State::NotConnected
        } else {
            State::Unavailable
        }
    }
}
/// Runs on the main thread, after a delegate queued the device object.
pub fn remember(device: *mut AnyObject) {
    if device.is_null() {
        return;
    }
    EVENT_DEVICES.with(|s| unsafe {
        let mut devices = s.borrow_mut();
        devices.retain(|d| connected(d));
        if connected(&*device) && !devices.iter().any(|d| Obj::as_ptr(d) == device) {
            if let Some(device) = Obj::retain(device) {
                devices.push(device);
            }
        }
    });
}
pub extern "C-unwind" fn device_event(
    target: &AnyObject,
    _: Sel,
    _: *mut AnyObject,
    device: *mut AnyObject,
) {
    objc2::rc::autoreleasepool(|_| unsafe {
        let _: () = msg_send![target,performSelectorOnMainThread:sel!(bluetoothChanged:),withObject:device,waitUntilDone:false];
    });
}
fn drain(iterator: u32) {
    unsafe {
        loop {
            let entry = IOIteratorNext(iterator);
            if entry == 0 {
                break;
            }
            IOObjectRelease(entry);
        }
    }
}
extern "C" fn hid_event(_: *mut c_void, iterator: u32) {
    drain(iterator);
    crate::app::refresh_system_status();
}
pub struct Monitor {
    target: Obj,
    connect: Option<Obj>,
    disconnect: BTreeMap<usize, (Obj, Obj)>,
    port: *mut c_void,
    source: Ptr,
    iterators: Vec<u32>,
    pub warnings: Vec<String>,
}
impl Monitor {
    pub fn new(target: &AnyObject) -> Self {
        unsafe {
            let connect: Option<Obj> = msg_send![class!(IOBluetoothDevice),registerForConnectNotifications:target,selector:sel!(bluetoothEvent:device:)];
            let mut warnings = Vec::new();
            if connect.is_none() {
                warnings.push("蓝牙连接事件不可用，打开菜单时刷新".into());
            }
            let port = IONotificationPortCreate(0);
            let mut source = std::ptr::null();
            let mut iterators = Vec::new();
            if !port.is_null() {
                source = IONotificationPortGetRunLoopSource(port);
                if !source.is_null() {
                    CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopCommonModes);
                }
                for kind in [c"IOServiceMatched", c"IOServiceTerminate"] {
                    let mut iterator = 0;
                    let matching = IOServiceMatching(c"IOHIDDevice".as_ptr());
                    if !matching.is_null()
                        && IOServiceAddMatchingNotification(
                            port,
                            kind.as_ptr(),
                            matching,
                            hid_event,
                            std::ptr::null_mut(),
                            &mut iterator,
                        ) == 0
                    {
                        drain(iterator);
                        iterators.push(iterator);
                    }
                }
            }
            if source.is_null() || iterators.len() != 2 {
                warnings.push("蓝牙 HID 事件不可用，打开菜单时刷新".into());
            }
            let mut monitor = Self {
                target: Obj::retain(target as *const AnyObject as *mut AnyObject).unwrap(),
                connect,
                disconnect: BTreeMap::new(),
                port,
                source,
                iterators,
                warnings,
            };
            monitor.sync();
            monitor
        }
    }
    pub fn subscriptions(&self) -> (bool, usize, usize) {
        (
            self.connect.is_some(),
            self.disconnect.len(),
            self.iterators.len(),
        )
    }
    pub fn sync(&mut self) {
        self.disconnect.retain(|_, (device, token)| {
            if connected(device) {
                true
            } else {
                unsafe {
                    let _: () = msg_send![&**token, unregister];
                }
                false
            }
        });
        for device in candidates().into_iter().filter(|d| connected(d)) {
            let key = Obj::as_ptr(&device) as usize;
            if let std::collections::btree_map::Entry::Vacant(entry) = self.disconnect.entry(key) {
                let token: Option<Obj> = unsafe {
                    msg_send![&device,registerForDisconnectNotification:&*self.target,selector:sel!(bluetoothEvent:device:)]
                };
                if let Some(token) = token {
                    entry.insert((device, token));
                }
            }
        }
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        unsafe {
            for (_, token) in self.disconnect.values() {
                let _: () = msg_send![&**token, unregister];
            }
            if let Some(token) = &self.connect {
                let _: () = msg_send![&**token, unregister];
            }
            for iterator in &self.iterators {
                IOObjectRelease(*iterator);
            }
            if !self.source.is_null() {
                CFRunLoopRemoveSource(CFRunLoopGetMain(), self.source, kCFRunLoopCommonModes);
            }
            if !self.port.is_null() {
                IONotificationPortDestroy(self.port);
            }
        }
    }
}

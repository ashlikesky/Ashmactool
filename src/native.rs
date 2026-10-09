//! AppKit and Foundation bridge. All UI calls must run on the main thread.
use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
use objc2_foundation::NSString;
use std::{ffi::CStr, path::Path};

pub type Obj = Retained<AnyObject>;
pub type Result<T> = std::result::Result<T, String>;

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}
#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {}

pub fn string(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}
pub fn text(object: &AnyObject) -> String {
    unsafe {
        let p: *const std::ffi::c_char = msg_send![object, UTF8String];
        if p.is_null() {
            String::new()
        } else {
            CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    }
}
pub fn kind(object: &AnyObject, name: &CStr) -> bool {
    unsafe {
        msg_send![object, isKindOfClass: objc2::runtime::AnyClass::get(name).expect("system class")]
    }
}
pub fn dictionary() -> Obj {
    unsafe { msg_send![class!(NSMutableDictionary), new] }
}
pub fn array() -> Obj {
    unsafe { msg_send![class!(NSMutableArray), new] }
}
pub fn insert(d: &AnyObject, key: &str, value: &AnyObject) {
    unsafe {
        let _: () = msg_send![d, setObject: value, forKey: &*string(key)];
    }
}
pub fn get(d: &AnyObject, key: &str) -> Option<Obj> {
    unsafe { msg_send![d, objectForKey: &*string(key)] }
}
pub fn append(a: &AnyObject, value: &AnyObject) {
    unsafe {
        let _: () = msg_send![a, addObject: value];
    }
}
pub fn count(a: &AnyObject) -> usize {
    unsafe { msg_send![a, count] }
}
pub fn at(a: &AnyObject, i: usize) -> Obj {
    unsafe { msg_send![a, objectAtIndex: i] }
}
pub fn number(n: f64) -> Obj {
    unsafe { msg_send![class!(NSNumber), numberWithDouble: n] }
}
pub fn numeric(d: &AnyObject, key: &str) -> Result<f64> {
    let n = get(d, key).ok_or_else(|| format!("缺少 {key}"))?;
    if !kind(&n, c"NSNumber") {
        return Err(format!("{key} 必须是数字"));
    }
    Ok(unsafe { msg_send![&n, doubleValue] })
}
pub fn data(bytes: &[u8]) -> Obj {
    unsafe {
        msg_send![class!(NSData), dataWithBytes: bytes.as_ptr().cast::<std::ffi::c_void>(), length: bytes.len()]
    }
}
pub fn bytes(d: &AnyObject) -> &[u8] {
    unsafe {
        let n: usize = msg_send![d, length];
        let p: *const std::ffi::c_void = msg_send![d, bytes];
        if n == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(p.cast::<u8>(), n)
        }
    }
}
pub fn read_plist(path: &Path) -> Result<Obj> {
    let raw = std::fs::read(path).map_err(|e| e.to_string())?;
    if raw.len() > 32 * 1024 * 1024 {
        return Err("主题文件不能超过 32 MB".into());
    }
    unsafe {
        let value: Option<Obj> = msg_send![class!(NSPropertyListSerialization), propertyListWithData: &*data(&raw), options: 0usize, format: std::ptr::null_mut::<usize>(), error: std::ptr::null_mut::<*mut AnyObject>()];
        value.ok_or_else(|| "无法读取 plist 主题".into())
    }
}
pub fn write_plist(path: &Path, value: &AnyObject) -> Result<()> {
    let raw: Option<Obj> = unsafe {
        msg_send![class!(NSPropertyListSerialization), dataWithPropertyList: value, format: 200usize, options: 0usize, error: std::ptr::null_mut::<*mut AnyObject>()]
    };
    let raw = raw.ok_or("无法保存 plist")?;
    if bytes(&raw).len() > 32 * 1024 * 1024 {
        return Err("备份或主题超过 32 MB，未修改指针".into());
    }
    let parent = path.parent().ok_or("无效路径")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temp, bytes(&raw)).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, path).map_err(|e| e.to_string())
}
pub fn support() -> Result<std::path::PathBuf> {
    let home = std::env::var_os("HOME").ok_or("无法找到用户目录")?;
    Ok(std::path::PathBuf::from(home).join("Library/Application Support/Ashmactool"))
}
pub fn alert(title: &str, message: &str) {
    unsafe {
        let alert: Obj = msg_send![class!(NSAlert), new];
        let _: () = msg_send![&alert, setMessageText: &*string(title)];
        let _: () = msg_send![&alert, setInformativeText: &*string(message)];
        let _: *mut AnyObject = msg_send![&alert, addButtonWithTitle: &*string("好")];
        let app: Obj = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![&app, activateIgnoringOtherApps: true];
        let _: isize = msg_send![&alert, runModal];
    }
}

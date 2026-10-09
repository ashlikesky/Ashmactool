//! Native cursor registration. API declarations traced to Mousecape/CGSInternal.
//! See THIRD_PARTY_NOTICES.md for origins and original license notices.
use crate::{
    graphics::{self, Image},
    native::{self, Obj, Result},
};
use objc2::{msg_send, runtime::AnyObject};
use objc2_core_foundation::{CGPoint as Point, CGSize as Size};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{CStr, CString, c_char, c_void},
    path::{Path, PathBuf},
};

type Ptr = *const c_void;
type Register = unsafe extern "C" fn(
    i32,
    *const c_char,
    bool,
    bool,
    Size,
    Point,
    usize,
    f64,
    Ptr,
    *mut i32,
) -> i32;
type CopyImages = unsafe extern "C" fn(
    i32,
    *const c_char,
    *mut Size,
    *mut Point,
    *mut usize,
    *mut f64,
    *mut *mut AnyObject,
) -> i32;
type CoreCopy = unsafe extern "C" fn(
    i32,
    i32,
    *mut *mut AnyObject,
    *mut Size,
    *mut Point,
    *mut usize,
    *mut f64,
) -> i32;
type Main = unsafe extern "C" fn() -> i32;
type Name = unsafe extern "C" fn(i32) -> *const c_char;
type Set = unsafe extern "C" fn(i32, *const c_char, *mut i32) -> i32;
type Dock = unsafe extern "C" fn(i32, bool);

#[link(name = "System")]
unsafe extern "C" {
    fn dlopen(path: *const c_char, mode: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
}

pub struct Api {
    pub cid: i32,
    register: Register,
    copy: CopyImages,
    name: Name,
    set: Set,
    _handles: Vec<*mut c_void>,
    dock: Option<Dock>,
    core_copy: Option<CoreCopy>,
}
impl Api {
    pub fn load() -> Result<Self> {
        let mut handles = vec![];
        for path in [c"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight",c"/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices",c"/System/Library/Frameworks/ApplicationServices.framework/Frameworks/HIServices.framework/HIServices"] {
            let h=unsafe{dlopen(path.as_ptr(),1)};
            if !h.is_null(){handles.push(h);}
        }
        let resolve = |name: &CStr| -> Result<*mut c_void> {
            for h in &handles {
                let p = unsafe { dlsym(*h, name.as_ptr()) };
                if !p.is_null() {
                    return Ok(p);
                }
            }
            Err(format!(
                "当前 macOS 不提供 {}，未修改指针",
                name.to_string_lossy()
            ))
        };
        // Function pointer ABIs match the upstream C declarations; symbols are checked first.
        unsafe {
            let main: Main = std::mem::transmute(resolve(c"CGSMainConnectionID")?);
            let cid = main();
            if cid == 0 {
                return Err("无法连接到图形会话".into());
            }
            Ok(Self {
                cid,
                register: std::mem::transmute::<*mut c_void, Register>(resolve(
                    c"CGSRegisterCursorWithImages",
                )?),
                copy: std::mem::transmute::<*mut c_void, CopyImages>(resolve(
                    c"CGSCopyRegisteredCursorImages",
                )?),
                name: std::mem::transmute::<*mut c_void, Name>(resolve(
                    c"CGSCursorNameForSystemCursor",
                )?),
                set: std::mem::transmute::<*mut c_void, Set>(resolve(c"CGSSetRegisteredCursor")?),
                dock: resolve(c"CGSSetDockCursorOverride")
                    .ok()
                    .map(|p| std::mem::transmute::<*mut c_void, Dock>(p)),
                core_copy: resolve(c"CoreCursorCopyImages")
                    .ok()
                    .map(|p| std::mem::transmute::<*mut c_void, CoreCopy>(p)),
                _handles: handles,
            })
        }
    }
    pub fn names(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::from([
            "com.apple.coregraphics.Arrow".into(),
            "com.apple.coregraphics.ArrowCtx".into(),
        ]);
        for id in 0..128 {
            let p = unsafe { (self.name)(id) };
            if !p.is_null() {
                let name = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
                // macOS 26+ may expose S variants under additional cursor IDs.
                if name.rsplit('.').next().is_some_and(|suffix| {
                    ["Arrow", "ArrowCtx", "ArrowS", "ArrowCtxS"].contains(&suffix)
                }) {
                    names.insert(name);
                }
            }
        }
        names
    }
    pub fn snapshot(&self, name: &str) -> Result<Cursor> {
        let n = CString::new(name).map_err(|_| "无效指针名称")?;
        let (mut size, mut hot, mut frames, mut duration, mut images) = (
            Size::new(0., 0.),
            Point::new(0., 0.),
            0,
            0.,
            std::ptr::null_mut(),
        );
        let core_id = name
            .strip_prefix("com.apple.cursor.")
            .and_then(|s| s.parse::<i32>().ok());
        let error = if let (Some(id), Some(copy)) = (core_id, self.core_copy) {
            unsafe {
                copy(
                    self.cid,
                    id,
                    &mut images,
                    &mut size,
                    &mut hot,
                    &mut frames,
                    &mut duration,
                )
            }
        } else {
            unsafe {
                (self.copy)(
                    self.cid,
                    n.as_ptr(),
                    &mut size,
                    &mut hot,
                    &mut frames,
                    &mut duration,
                    &mut images,
                )
            }
        };
        if error != 0 || images.is_null() {
            return Err(format!("无法备份 {name}（CGError {error}）"));
        }
        let images = unsafe { objc2::rc::Retained::from_raw(images) }.ok_or("无图片")?;
        // Never replace a cursor whose native form cannot be registered back.
        // Some system wait cursors have 30 frames; preserve those rather than
        // downsampling the user's original during restoration.
        graphics::validate(size, hot, frames, duration)
            .map_err(|e| format!("{name} 的原始指针无法完整恢复：{e}"))?;
        if native::count(&images) == 0 || native::count(&images) > 4 {
            return Err(format!("{name} 的原始图片表示超出备份范围"));
        }
        for i in 0..native::count(&images) {
            let image = native::at(&images, i);
            let ptr = (&*image as *const AnyObject).cast();
            let (w, h) = unsafe {
                (
                    graphics::CGImageGetWidth(ptr),
                    graphics::CGImageGetHeight(ptr),
                )
            };
            if w == 0 || w > 512 || h == 0 || h % frames != 0 || h / frames > 512 {
                return Err(format!("{name} 的原始图片超出可恢复范围，保持原样"));
            }
        }
        Ok(Cursor {
            size,
            hot,
            frames,
            duration,
            images,
        })
    }
    pub fn register(&self, name: &str, c: &Cursor) -> Result<()> {
        let name = CString::new(name).map_err(|_| "无效指针名称")?;
        let mut seed = 0;
        let error = unsafe {
            (self.register)(
                self.cid,
                name.as_ptr(),
                true,
                true,
                c.size,
                c.hot,
                c.frames,
                c.duration,
                (&*c.images as *const AnyObject).cast(),
                &mut seed,
            )
        };
        if error == 0 {
            Ok(())
        } else {
            Err(format!(
                "注册 {} 失败（CGError {error}）",
                name.to_string_lossy()
            ))
        }
    }
    pub fn refresh(&self, name: &str) {
        if let Ok(n) = CString::new(name) {
            let mut seed = 0;
            unsafe {
                (self.set)(self.cid, n.as_ptr(), &mut seed);
            }
        }
    }
}

pub struct Cursor {
    pub size: Size,
    pub hot: Point,
    pub frames: usize,
    pub duration: f64,
    pub images: Obj,
}
impl Cursor {
    pub fn preset(style: &str, points: f64) -> Result<Self> {
        let images = native::array();
        for scale in [1, 2] {
            let image = graphics::preset(style, scale)?;
            unsafe {
                native::append(&images, &*(image.0.cast::<AnyObject>()));
            }
        }
        let c = Self {
            size: Size::new(points, points),
            hot: Point::new(13. * points / 64., 10. * points / 64.),
            frames: 1,
            duration: 0.,
            images,
        };
        graphics::validate(c.size, c.hot, c.frames, c.duration)?;
        Ok(c)
    }
    pub fn picture(path: &Path, points: f64, hot: Point) -> Result<Self> {
        let image = Image::from_file(path)?;
        let (w, h) = unsafe {
            (
                graphics::CGImageGetWidth(image.0),
                graphics::CGImageGetHeight(image.0),
            )
        };
        if h > 512 {
            return Err("静态图片高度不能超过 512 px".into());
        }
        let longest = w.max(h) as f64;
        let size = Size::new(points * w as f64 / longest, points * h as f64 / longest);
        graphics::validate(size, hot, 1, 0.)?;
        let images = native::array();
        for scale in [1, 2] {
            let img = image.resized(
                (size.width * scale as f64).ceil() as usize,
                (size.height * scale as f64).ceil() as usize,
            )?;
            unsafe {
                native::append(&images, &*img.0.cast::<AnyObject>());
            }
        }
        Ok(Self {
            size,
            hot,
            frames: 1,
            duration: 0.,
            images,
        })
    }
    fn from_plist(d: &AnyObject) -> Result<Self> {
        if !native::kind(d, c"NSDictionary") {
            return Err("指针条目必须是字典".into());
        }
        let size = Size::new(
            native::numeric(d, "PointsWide")?,
            native::numeric(d, "PointsHigh")?,
        );
        let hot = Point::new(
            native::numeric(d, "HotSpotX")?,
            native::numeric(d, "HotSpotY")?,
        );
        let frame_value = native::numeric(d, "FrameCount")?;
        if frame_value.fract() != 0. || !(1.0..=24.0).contains(&frame_value) {
            return Err("动画帧数必须是 1–24 的整数".into());
        }
        let frames = frame_value as usize;
        let duration = native::numeric(d, "FrameDuration")?;
        graphics::validate(size, hot, frames, duration)?;
        let reps = native::get(d, "Representations").ok_or("没有指针图片")?;
        if !native::kind(&reps, c"NSArray") || native::count(&reps) == 0 || native::count(&reps) > 4
        {
            return Err("图片表示数量须为 1–4".into());
        }
        let images = native::array();
        for i in 0..native::count(&reps) {
            let image = Image::from_data(&native::at(&reps, i))?;
            let h = unsafe { graphics::CGImageGetHeight(image.0) };
            if h % frames != 0 || h / frames > 512 {
                return Err("动画图片高度必须能被帧数整除，每帧不超过 512 px".into());
            }
            unsafe {
                native::append(&images, &*image.0.cast::<AnyObject>());
            }
        }
        Ok(Self {
            size,
            hot,
            frames,
            duration,
            images,
        })
    }
    pub fn plist(&self) -> Result<Obj> {
        let d = native::dictionary();
        for (k, v) in [
            ("PointsWide", self.size.width),
            ("PointsHigh", self.size.height),
            ("HotSpotX", self.hot.x),
            ("HotSpotY", self.hot.y),
            ("FrameCount", self.frames as f64),
            ("FrameDuration", self.duration),
        ] {
            native::insert(&d, k, &native::number(v));
        }
        let reps = native::array();
        for i in 0..native::count(&self.images) {
            let image = native::at(&self.images, i);
            unsafe extern "C" {
                fn CGImageRetain(p: Ptr) -> Ptr;
            }
            let image = Image(unsafe { CGImageRetain((&*image as *const AnyObject).cast()) });
            native::append(&reps, &*image.png()?);
        }
        native::insert(&d, "Representations", &reps);
        Ok(d)
    }
    pub fn pixels(&self) -> Result<Vec<Vec<u8>>> {
        let mut result = Vec::new();
        unsafe extern "C" {
            fn CGImageRetain(p: Ptr) -> Ptr;
        }
        for i in 0..native::count(&self.images) {
            let native_image = native::at(&self.images, i);
            let image =
                Image(unsafe { CGImageRetain((&*native_image as *const AnyObject).cast()) });
            result.push(image.pixels()?);
        }
        Ok(result)
    }
}

pub fn theme(path: &Path) -> Result<BTreeMap<String, Cursor>> {
    let root = native::read_plist(path)?;
    if !native::kind(&root, c"NSDictionary") {
        return Err("主题根节点必须是字典".into());
    }
    let entries = native::get(&root, "Cursors").ok_or("主题中没有 Cursors")?;
    if !native::kind(&entries, c"NSDictionary") {
        return Err("Cursors 必须是字典".into());
    }
    let keys: Obj = unsafe { msg_send![&entries, allKeys] };
    if native::count(&keys) > 128 {
        return Err("主题最多包含 128 种指针".into());
    }
    let mut result = BTreeMap::new();
    for i in 0..native::count(&keys) {
        let key = native::at(&keys, i);
        if !native::kind(&key, c"NSString") {
            return Err("指针名称必须是字符串".into());
        }
        let name = native::text(&key);
        if !name.starts_with("com.apple.") || name.len() > 256 || name.contains('\0') {
            return Err("主题包含无效的系统指针名称".into());
        }
        let value = native::get(&entries, &name).ok_or("缺失指针")?;
        result.insert(name, Cursor::from_plist(&value)?);
    }
    if result.is_empty() {
        return Err("主题中没有指针".into());
    }
    Ok(result)
}

pub struct Engine {
    pub api: Api,
    backups: BTreeMap<String, Cursor>,
    active: BTreeMap<String, Cursor>,
    backup_path: PathBuf,
    pub error: Option<String>,
    pub skipped: Vec<String>,
}
impl Engine {
    pub fn new(backup_path: PathBuf) -> Result<Self> {
        let api = Api::load()?;
        let backups = if backup_path.exists() {
            theme(&backup_path)?
        } else {
            BTreeMap::new()
        };
        Ok(Self {
            api,
            backups,
            active: BTreeMap::new(),
            backup_path,
            error: None,
            skipped: Vec::new(),
        })
    }
    fn persist_backup(&self) -> Result<()> {
        let root = native::dictionary();
        let entries = native::dictionary();
        for (name, c) in &self.backups {
            native::insert(&entries, name, &*c.plist()?);
        }
        native::insert(&root, "Cursors", &entries);
        native::write_plist(&self.backup_path, &root)
    }
    pub fn apply(&mut self, mut desired: BTreeMap<String, Cursor>) -> Result<()> {
        // Fully prepare and validate before restoring or touching global state.
        for c in desired.values() {
            graphics::validate(c.size, c.hot, c.frames, c.duration)?;
        }
        self.restore()?;
        let mut missing = vec![];
        for name in desired.keys() {
            match self.api.snapshot(name) {
                Ok(c) => {
                    self.backups.insert(name.clone(), c);
                }
                Err(e) => missing.push((name.clone(), e)),
            }
        }
        // Unknown legacy aliases are skipped, never changed without a restorable backup.
        for (name, _) in &missing {
            desired.remove(name);
        }
        self.skipped = missing.iter().map(|(_, e)| e.clone()).collect();
        if desired.is_empty() {
            return Err(format!(
                "没有可安全替换的指针：{}",
                missing
                    .iter()
                    .map(|(_, e)| e.as_str())
                    .collect::<Vec<_>>()
                    .join("；")
            ));
        }
        self.persist_backup()?;
        self.active = desired;
        if let Err(e) = self.reapply() {
            let rollback = self.restore();
            return Err(format!(
                "{e}；回滚：{}",
                rollback.err().unwrap_or_else(|| "完成".into())
            ));
        }
        if let Some(dock) = self.api.dock {
            unsafe {
                dock(self.api.cid, false);
            }
        }
        if let Some(name) = self.active.keys().find(|n| n.contains("Arrow")) {
            self.api.refresh(name);
        }
        self.error = None;
        Ok(())
    }
    pub fn arrow(&mut self, style: &str, size: f64) -> Result<()> {
        let mut entries = BTreeMap::new();
        for n in self.api.names() {
            entries.insert(n, Cursor::preset(style, size)?);
        }
        self.apply(entries)
    }
    pub fn picture(&mut self, path: &Path, size: f64, hot: Point) -> Result<()> {
        let mut entries = BTreeMap::new();
        for n in self.api.names() {
            entries.insert(n, Cursor::picture(path, size, hot)?);
        }
        self.apply(entries)
    }
    pub fn reapply(&mut self) -> Result<()> {
        for (name, c) in &self.active {
            if let Err(e) = self.api.register(name, c) {
                self.error = Some(e.clone());
                return Err(e);
            }
        }
        if !self.active.is_empty() {
            self.error = None;
        }
        Ok(())
    }
    pub fn restore(&mut self) -> Result<()> {
        // Stop event-driven reapplication even if restoration itself needs a retry.
        self.active.clear();
        let mut failures = vec![];
        for (name, c) in &self.backups {
            if let Err(e) = self.api.register(name, c) {
                failures.push(e);
            }
        }
        if !failures.is_empty() {
            let e = failures.join("；");
            self.error = Some(e.clone());
            return Err(e);
        }
        if let Some(name) = self.backups.keys().find(|n| n.contains("Arrow")) {
            self.api.refresh(name);
        }
        if self.backup_path.exists() {
            std::fs::remove_file(&self.backup_path)
                .map_err(|e| format!("已恢复，但备份清理失败：{e}"))?;
        }
        self.backups.clear();
        self.error = None;
        Ok(())
    }
    pub fn active_count(&self) -> usize {
        self.active.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2::rc::autoreleasepool;
    #[test]
    fn snapshot_plist_roundtrip_preserves_pixels_and_hotspot() {
        autoreleasepool(|_| {
            let original = Cursor::preset("glow", 52.).unwrap();
            let encoded = original.plist().unwrap();
            let decoded = Cursor::from_plist(&encoded).unwrap();
            assert_eq!(original.size, decoded.size);
            assert_eq!(original.hot, decoded.hot);
            assert!(
                original.pixels().unwrap() == decoded.pixels().unwrap(),
                "normalized raster pixels differ"
            );
        });
    }
    #[test]
    fn rejects_wrong_types_and_fractional_frames() {
        autoreleasepool(|_| {
            let d = Cursor::preset("dark", 40.).unwrap().plist().unwrap();
            native::insert(&d, "FrameCount", &native::string("1"));
            assert!(Cursor::from_plist(&d).is_err());
            native::insert(&d, "FrameCount", &native::number(1.5));
            assert!(Cursor::from_plist(&d).is_err());
            native::insert(&d, "FrameCount", &native::number(1.));
            native::insert(&d, "HotSpotX", &native::number(40.));
            assert!(Cursor::from_plist(&d).is_err());
        });
    }
    #[test]
    fn rejects_oversized_representation_arrays() {
        autoreleasepool(|_| {
            let d = Cursor::preset("light", 64.).unwrap().plist().unwrap();
            let reps = native::get(&d, "Representations").unwrap();
            let oversized = native::array();
            for _ in 0..5 {
                native::append(&oversized, &native::at(&reps, 0));
            }
            native::insert(&d, "Representations", &oversized);
            assert!(Cursor::from_plist(&d).is_err());
        });
    }
}

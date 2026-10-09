//! Native cursor registration. API declarations traced to Mousecape/CGSInternal.
//! See THIRD_PARTY_NOTICES.md for origins and original license notices.
use crate::{
    graphics::{self, Image},
    native::{self, Obj, Result},
};
use objc2::{class, msg_send, runtime::AnyObject, sel};
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
    _standard_cursors: Vec<Obj>,
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
            let mut cursors = Vec::new();
            // New AppKit resize cursors are created lazily. Warm the public
            // factories without setting the visible cursor, then discover IDs.
            let modern: bool = msg_send![class!(NSCursor),respondsToSelector:sel!(frameResizeCursorFromPosition:inDirections:)];
            if modern {
                for position in [1usize, 2, 4, 8, 3, 9, 6, 12] {
                    for direction in [1usize, 2, 3] {
                        let c: Obj = msg_send![class!(NSCursor),frameResizeCursorFromPosition:position,inDirections:direction];
                        cursors.push(c);
                    }
                }
                for directions in [1usize, 2, 3] {
                    let column: Obj =
                        msg_send![class!(NSCursor),columnResizeCursorInDirections:directions];
                    let row: Obj =
                        msg_send![class!(NSCursor),rowResizeCursorInDirections:directions];
                    cursors.extend([column, row]);
                }
            }
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
                _standard_cursors: cursors,
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
    pub fn discover(&self) -> BTreeSet<String> {
        let mut names = self.names();
        for id in 0..256 {
            let p = unsafe { (self.name)(id) };
            if !p.is_null() {
                let name = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
                if name.starts_with("com.apple.") && name.len() <= 256 {
                    names.insert(name);
                }
            }
        }
        // CoreCursor has its own identifier space, distinct from CGS system IDs.
        // Probe images instead of guessing which numeric names this OS supports.
        for id in 0..128 {
            let name = format!("com.apple.cursor.{id}");
            if self.snapshot(&name).is_ok() {
                names.insert(name);
            }
        }
        names
    }
    pub fn snapshot(&self, name: &str) -> Result<Cursor> {
        self.snapshot_with_limit(name, 64)
    }
    pub fn snapshot_with_limit(&self, name: &str, max_frames: usize) -> Result<Cursor> {
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
        let named_error = unsafe {
            (self.copy)(
                self.cid,
                n.as_ptr(),
                &mut size,
                &mut hot,
                &mut frames,
                &mut duration,
                &mut images,
            )
        };
        let error = if named_error == 0 && !images.is_null() {
            named_error
        } else if let (Some(id), Some(copy)) = (core_id, self.core_copy) {
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
            named_error
        };
        if error != 0 || images.is_null() {
            return Err(format!("无法备份 {name}（CGError {error}）"));
        }
        let images = unsafe { objc2::rc::Retained::from_raw(images) }.ok_or("无图片")?;
        // Never replace a cursor whose native form cannot be registered back.
        // Some system wait cursors have 30 frames; preserve those rather than
        // downsampling the user's original during restoration.
        graphics::validate_with_limit(size, hot, frames, duration, max_frames)
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
    pub fn preset_theme(&self, style: &str, size: f64) -> Result<BTreeMap<String, Cursor>> {
        let mut entries = BTreeMap::new();
        let arrows = self.names();
        for n in self.discover() {
            if n.ends_with(".Empty") {
                continue;
            } // Hidden stays hidden.
            if arrows.contains(&n) || n == "com.apple.cursor.0" {
                entries.insert(n, Cursor::preset(style, size)?);
            } else if n == "com.apple.cursor.7" || n == "com.apple.cursor.8" {
                let images = native::array();
                for scale in [1, 2] {
                    let img = graphics::crosshair(style, scale, n.ends_with(".8"))?;
                    unsafe {
                        native::append(&images, &*img.0.cast::<AnyObject>());
                    }
                }
                entries.insert(
                    n,
                    Cursor {
                        size: Size::new(size, size),
                        hot: Point::new(size / 2., size / 2.),
                        frames: 1,
                        duration: 0.,
                        images,
                    },
                );
            } else if let Ok(original) = self.snapshot(&n) {
                if original.frames > 24 && !native_wait(&n) {
                    continue;
                }
                // Legacy badge aliases may report placeholder images. Use the
                // matching full CoreCursor silhouette rather than a placeholder.
                let source = match n.as_str() {
                    "com.apple.coregraphics.Alias" => self.snapshot("com.apple.cursor.2")?,
                    "com.apple.coregraphics.Copy" => self.snapshot("com.apple.cursor.5")?,
                    "com.apple.coregraphics.Move" => self.snapshot("com.apple.cursor.39")?,
                    "com.apple.coregraphics.IBeamXOR" => self.snapshot("com.apple.cursor.1")?,
                    _ => original,
                };
                entries.insert(n, Cursor::themed(&source, style, size)?);
            }
        }
        Ok(entries)
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
    pub fn resampled(&self, frames: usize) -> Result<Self> {
        let images = native::array();
        unsafe extern "C" {
            fn CGImageRetain(p: Ptr) -> Ptr;
        }
        for i in 0..native::count(&self.images) {
            let source = native::at(&self.images, i);
            let source = Image(unsafe { CGImageRetain((&*source as *const AnyObject).cast()) });
            let image = graphics::resample_sprite(&source, self.frames, frames)?;
            unsafe {
                native::append(&images, &*image.0.cast::<AnyObject>());
            }
        }
        Ok(Self {
            size: self.size,
            hot: self.hot,
            frames,
            duration: self.duration * self.frames as f64 / frames as f64,
            images,
        })
    }
    pub fn themed(original: &Self, style: &str, points: f64) -> Result<Self> {
        let last = native::at(&original.images, native::count(&original.images) - 1);
        unsafe extern "C" {
            fn CGImageRetain(p: Ptr) -> Ptr;
        }
        let source = Image(unsafe { CGImageRetain((&*last as *const AnyObject).cast()) });
        let frames = original.frames.min(24);
        let images = native::array();
        for scale in [1, 2] {
            let img = graphics::themed_sprite(
                &source,
                original.size,
                original.frames,
                frames,
                style,
                scale,
            )?;
            unsafe {
                native::append(&images, &*img.0.cast::<AnyObject>());
            }
        }
        let ratio = 40. / original.size.width.max(original.size.height);
        let hot = Point::new(
            ((64. - original.size.width * ratio) / 2. + original.hot.x * ratio) * points / 64.,
            ((64. - original.size.height * ratio) / 2. + original.hot.y * ratio) * points / 64.,
        );
        let out = Self {
            size: Size::new(points, points),
            hot,
            frames,
            duration: if frames > 1 {
                original.duration * original.frames as f64 / frames as f64
            } else {
                0.
            },
            images,
        };
        graphics::validate(out.size, out.hot, out.frames, out.duration)?;
        Ok(out)
    }
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
        Self::from_plist_with_limit(d, 24)
    }
    fn from_plist_with_limit(d: &AnyObject, max_frames: usize) -> Result<Self> {
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
        if frame_value.fract() != 0. || !(1.0..=max_frames as f64).contains(&frame_value) {
            return Err(format!("动画帧数必须是 1–{max_frames} 的整数"));
        }
        let frames = frame_value as usize;
        let duration = native::numeric(d, "FrameDuration")?;
        graphics::validate_with_limit(size, hot, frames, duration, max_frames)?;
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
    read_theme(path, false)
}
fn read_theme(path: &Path, backup: bool) -> Result<BTreeMap<String, Cursor>> {
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
        let max = if backup && native_wait(&name) { 64 } else { 24 };
        let value = if max == 24 {
            Cursor::from_plist(&value)?
        } else {
            Cursor::from_plist_with_limit(&value, max)?
        };
        result.insert(name, value);
    }
    if result.is_empty() {
        return Err("主题中没有指针".into());
    }
    Ok(result)
}

fn native_wait(name: &str) -> bool {
    matches!(name, "com.apple.coregraphics.Wait" | "com.apple.cursor.6")
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
            read_theme(&backup_path, true)?
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
                Ok(c) if c.frames <= 24 || native_wait(name) => {
                    self.backups.insert(name.clone(), c);
                }
                Ok(_) => missing.push((
                    name.clone(),
                    format!("{name} 原生动画无法完整恢复，保持原样"),
                )),
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
    pub fn preset(&mut self, style: &str, size: f64) -> Result<()> {
        self.restore()?;
        self.apply(self.api.preset_theme(style, size)?)
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
        if self.backups.values().any(|c| c.frames > 24) {
            let archive = self.backup_path.with_file_name("native-original.cape");
            if !archive.exists() {
                // Retain all original frames, even though this OS only accepts
                // 24 on registration. Never overwrite this lossless archive.
                std::fs::copy(&self.backup_path, &archive).map_err(|e| e.to_string())?;
            }
        }
        for (name, c) in &self.backups {
            let reduced;
            let restore = if c.frames > 24 {
                reduced = c.resampled(24)?;
                &reduced
            } else {
                c
            };
            let result = self.api.register(name, restore);
            if let Err(e) = result {
                failures.push(e);
            }
        }
        // The native wait animation has 30 frames; this API accepts only 24.
        // Restore its original appearance and total cycle time, verify every
        // selected frame, and retain the full 30-frame source in the archive.
        if failures.is_empty() {
            for (name, before) in &self.backups {
                if before.frames <= 24 {
                    continue;
                }
                let expected = before.resampled(24)?;
                match self.api.snapshot(name) {
                    Ok(after)
                        if expected.size == after.size
                            && expected.hot == after.hot
                            && expected.frames == after.frames
                            && expected.pixels()? == after.pixels()? => {}
                    _ => failures.push(format!("{name} 的原生动画恢复回读不一致，保留备份")),
                }
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
    pub fn verify_active(&self) -> Result<()> {
        for (name, expected) in &self.active {
            let actual = self.api.snapshot(name)?;
            if (actual.size.width - expected.size.width).abs() > 0.0001
                || (actual.size.height - expected.size.height).abs() > 0.0001
                || (actual.hot.x - expected.hot.x).abs() > 0.0001
                || (actual.hot.y - expected.hot.y).abs() > 0.0001
                || actual.frames != expected.frames
                || (actual.duration - expected.duration).abs() > 0.00001
                || actual.pixels()? != expected.pixels()?
            {
                let a = actual.pixels()?;
                let e = expected.pixels()?;
                let dimensions = |p: &Vec<Vec<u8>>| {
                    p.iter()
                        .map(|x| {
                            format!(
                                "{}x{}",
                                u64::from_le_bytes(x[..8].try_into().unwrap()),
                                u64::from_le_bytes(x[8..16].try_into().unwrap())
                            )
                        })
                        .collect::<Vec<_>>()
                };
                let delta = a
                    .iter()
                    .zip(&e)
                    .flat_map(|(a, e)| a[16..].iter().zip(&e[16..]).map(|(a, e)| a.abs_diff(*e)))
                    .max()
                    .unwrap_or(0);
                return Err(format!(
                    "{name} 的主题注册回读不一致：size {:?}/{:?}, hot {:?}/{:?}, frames {}/{}, images {:?}/{:?}, max_delta={delta}",
                    actual.size,
                    expected.size,
                    actual.hot,
                    expected.hot,
                    actual.frames,
                    expected.frames,
                    dimensions(&a),
                    dimensions(&e)
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_animation_keeps_cycle_time_and_lossless_archive_data() {
        objc2::rc::autoreleasepool(|_| {
            let one = graphics::preset("dark", 2).unwrap();
            let strip = graphics::resample_sprite(&one, 1, 30).unwrap();
            let images = native::array();
            unsafe {
                native::append(&images, &*strip.0.cast::<AnyObject>());
            }
            let original = Cursor {
                size: Size::new(64., 64.),
                hot: Point::new(13., 10.),
                frames: 30,
                duration: 1. / 60.,
                images,
            };
            let d = original.plist().unwrap();
            assert!(Cursor::from_plist(&d).is_err());
            let archived = Cursor::from_plist_with_limit(&d, 64).unwrap();
            assert_eq!(archived.frames, 30);
            assert_eq!(archived.pixels().unwrap(), original.pixels().unwrap());
            let restored = archived.resampled(24).unwrap();
            assert_eq!(restored.frames, 24);
            assert!((restored.duration * 24. - original.duration * 30.).abs() < 1e-12);
        });
    }
    #[test]
    fn complete_themes_preserve_animation_and_valid_hotspots_at_all_sizes() {
        objc2::rc::autoreleasepool(|_| {
            let original = Cursor::preset("light", 64.).unwrap();
            for style in ["glow", "dark", "light"] {
                for size in [28., 35., 45.5, 56.] {
                    let themed = Cursor::themed(&original, style, size).unwrap();
                    assert_eq!(themed.size, Size::new(size, size));
                    assert!(
                        themed.hot.x >= 0.
                            && themed.hot.x < size
                            && themed.hot.y >= 0.
                            && themed.hot.y < size
                    );
                    for pixels in themed.pixels().unwrap() {
                        assert!(pixels[16..].chunks_exact(4).any(|p| p[3] > 240));
                        let body: Vec<_> = pixels[16..]
                            .chunks_exact(4)
                            .filter(|p| p[3] > 240)
                            .collect();
                        let light = body
                            .iter()
                            .filter(|p| {
                                p[0] as u32 + p[1] as u32 + p[2] as u32 > p[3] as u32 * 3 / 2
                            })
                            .count();
                        assert_eq!(light > body.len() / 2, style == "light");
                    }
                }
            }
        });
    }
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

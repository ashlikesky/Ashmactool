use crate::{
    cursor::{self, Engine},
    native::{self, Obj, Result},
    power::KeepAwake,
};
use objc2::{
    class, msg_send,
    rc::autoreleasepool,
    runtime::{AnyObject, ClassBuilder, Sel},
    sel,
};
use objc2_core_foundation::{CGPoint as Point, CGRect as Rect, CGSize as Size};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
};

#[link(name = "ServiceManagement", kind = "framework")]
unsafe extern "C" {}

#[derive(Clone)]
struct Preferences {
    style: String,
    size: f64,
    path: String,
    hot: Point,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            style: "none".into(),
            size: 28.,
            path: String::new(),
            hot: Point::new(0., 0.),
        }
    }
}
impl Preferences {
    fn load() -> Result<Self> {
        let path = native::support()?.join("settings.plist");
        if !path.exists() {
            return Ok(Self::default());
        }
        let d = native::read_plist(&path)?;
        if !native::kind(&d, c"NSDictionary") {
            return Err("设置文件格式错误".into());
        }
        let s = |key| {
            native::get(&d, key)
                .filter(|v| native::kind(v, c"NSString"))
                .map(|v| native::text(&v))
                .unwrap_or_default()
        };
        let mut p = Self {
            style: s("Style"),
            size: native::numeric(&d, "Size")?,
            path: s("Path"),
            hot: Point::new(native::numeric(&d, "HotX")?, native::numeric(&d, "HotY")?),
        };
        if native::get(&d, "SizeSchema").is_none() {
            p.size *= 0.875;
            if p.style == "image" {
                p.hot.x *= 0.875;
                p.hot.y *= 0.875;
            }
        }
        if !["none", "glow", "dark", "light", "image", "cape"].contains(&p.style.as_str())
            || !p.size.is_finite()
            || !(16.0..=64.0).contains(&p.size)
        {
            return Err("保存的指针设置无效".into());
        }
        Ok(p)
    }
    fn save(&self) -> Result<()> {
        let d = native::dictionary();
        native::insert(&d, "SizeSchema", &native::number(1.));
        native::insert(&d, "Style", &native::string(&self.style));
        native::insert(&d, "Path", &native::string(&self.path));
        for (k, v) in [
            ("Size", self.size),
            ("HotX", self.hot.x),
            ("HotY", self.hot.y),
        ] {
            native::insert(&d, k, &native::number(v));
        }
        native::write_plist(&native::support()?.join("settings.plist"), &d)
    }
    fn label(&self) -> &str {
        match self.style.as_str() {
            "glow" => "柔光主题",
            "dark" => "黑色主题",
            "light" => "白色主题",
            "image" => "自定义图片",
            "cape" => "导入主题",
            _ => "系统默认",
        }
    }
}
struct App {
    engine: Engine,
    prefs: Preferences,
    menu: Obj,
    cursor_menu: Obj,
    size_menu: Obj,
    awake: KeepAwake,
    target: Obj,
    status: Obj,
    last_error: Option<String>,
}
thread_local! {static APP:RefCell<Option<App>>=const{RefCell::new(None)};static BUSY:Cell<bool>=const{Cell::new(false)};static PENDING_EVENT:Cell<bool>=const{Cell::new(false)};}

fn action(f: impl FnOnce(&mut App) -> Result<()>) {
    // NSOpenPanel/NSAlert run nested event loops. Notifications during a modal
    // interaction must not recursively borrow the Rust state.
    if BUSY.with(|b| b.replace(true)) {
        return;
    }
    autoreleasepool(|_| {
        let result = APP.with(|slot| {
            let mut slot = slot.borrow_mut();
            let app = slot.as_mut().ok_or("应用尚未启动")?;
            let result = f(app);
            app.last_error = result.as_ref().err().cloned();
            app.rebuild();
            result
        });
        if let Err(e) = result {
            native::alert("Ashmactool", &e);
        }
    });
    BUSY.with(|b| b.set(false));
    if PENDING_EVENT.with(|p| p.replace(false)) {
        background_refresh();
    }
}
impl App {
    fn activate(&mut self, new: Preferences) -> Result<()> {
        match new.style.as_str() {
            "none" => self.engine.restore()?,
            "image" => self
                .engine
                .picture(&PathBuf::from(&new.path), new.size, new.hot)?,
            "cape" => self
                .engine
                .apply(cursor::theme(&PathBuf::from(&new.path))?)?,
            s => self.engine.preset(s, new.size)?,
        }
        if let Err(e) = new.save() {
            self.engine.restore()?;
            self.prefs.style = "none".into();
            return Err(format!("无法保存设置，已恢复系统指针：{e}"));
        }
        self.prefs = new;
        Ok(())
    }
    fn add(
        &self,
        menu: &AnyObject,
        title: &str,
        selector: Option<Sel>,
        tag: isize,
        checked: bool,
        enabled: bool,
    ) {
        unsafe {
            let item: objc2::rc::Allocated<AnyObject> = msg_send![class!(NSMenuItem), alloc];
            let item: Obj = msg_send![item,initWithTitle:&*native::string(title),action:selector,keyEquivalent:&*native::string("")];
            let _: () = msg_send![&item,setTarget:&*self.target];
            let _: () = msg_send![&item,setTag:tag];
            let _: () = msg_send![&item,setState:if checked{1isize}else{0isize}];
            let _: () = msg_send![&item,setEnabled:enabled];
            let _: () = msg_send![menu,addItem:&*item];
        }
    }
    fn separator(&self, menu: &AnyObject) {
        unsafe {
            let item: Obj = msg_send![class!(NSMenuItem), separatorItem];
            let _: () = msg_send![menu,addItem:&*item];
        }
    }
    fn submenu(&self, menu: &AnyObject, title: &str, submenu: &AnyObject, tag: isize) {
        unsafe {
            self.add(menu, title, None, tag, false, true);
            let item: Obj = msg_send![menu,itemWithTag:tag];
            let _: () = msg_send![&item,setSubmenu:submenu];
        }
    }
    fn rebuild(&self) {
        unsafe {
            for menu in [&self.menu, &self.cursor_menu, &self.size_menu] {
                let _: () = msg_send![menu, removeAllItems];
            }
            let cursor_status = if self.engine.error.is_some() {
                "需要重试"
            } else if self.engine.active_count() > 0 {
                self.prefs.label()
            } else {
                "系统默认"
            };
            self.add(
                &self.menu,
                if self.last_error.is_some() {
                    "Ashmactool · 需要重试"
                } else {
                    "Ashmactool"
                },
                None,
                0,
                false,
                false,
            );
            self.separator(&self.menu);
            for (tag, key, label) in [
                (1, "glow", "柔光主题"),
                (2, "dark", "黑色主题"),
                (3, "light", "白色主题"),
            ] {
                self.add(
                    &self.cursor_menu,
                    label,
                    Some(sel!(choose:)),
                    tag,
                    self.engine.active_count() > 0 && self.prefs.style == key,
                    true,
                );
            }
            self.separator(&self.cursor_menu);
            self.add(
                &self.cursor_menu,
                &format!("{} 项系统指针已应用", self.engine.active_count()),
                None,
                9,
                false,
                false,
            );
            self.add(
                &self.cursor_menu,
                "导入图片或主题…",
                Some(sel!(import:)),
                4,
                false,
                true,
            );
            self.add(
                &self.cursor_menu,
                "调整图片点击位置…",
                Some(sel!(hotspot:)),
                5,
                false,
                self.prefs.style == "image",
            );
            for (tag, n, label) in [
                (100, 28., "更小"),
                (101, 35., "小"),
                (102, 45.5, "中"),
                (103, 56., "大"),
            ] {
                self.add(
                    &self.size_menu,
                    label,
                    Some(sel!(resize:)),
                    tag,
                    (self.prefs.size - n).abs() < 0.1,
                    self.prefs.style != "cape",
                );
            }
            self.submenu(&self.cursor_menu, "指针尺寸", &self.size_menu, 6);
            self.separator(&self.cursor_menu);
            self.add(
                &self.cursor_menu,
                "恢复系统指针",
                Some(sel!(reset:)),
                7,
                self.engine.active_count() == 0,
                true,
            );
            self.add(
                &self.cursor_menu,
                "重新应用",
                Some(sel!(reapply:)),
                8,
                false,
                self.engine.active_count() > 0,
            );
            self.submenu(
                &self.menu,
                &format!("指针样式 · {cursor_status}"),
                &self.cursor_menu,
                10,
            );
            self.add(
                &self.menu,
                "保持屏幕唤醒",
                Some(sel!(toggleAwake:)),
                20,
                self.awake.enabled(),
                true,
            );
            self.add(
                &self.menu,
                if self.awake.enabled() {
                    "已开启 · 直到关闭或退出"
                } else {
                    "已关闭 · 按系统设置休眠"
                },
                None,
                21,
                false,
                false,
            );
            self.separator(&self.menu);
            self.add(
                &self.menu,
                "登录时启动",
                Some(sel!(login:)),
                30,
                login_status() == 1,
                in_bundle(),
            );
            self.add(
                &self.menu,
                "关于 Ashmactool…",
                Some(sel!(about:)),
                31,
                false,
                true,
            );
            self.add(&self.menu, "退出", Some(sel!(quit:)), 32, false, true);
            let button: Obj = msg_send![&self.status, button];
            let _: () = msg_send![&button,setToolTip:&*native::string(&format!("Ashmactool：{cursor_status}；屏幕唤醒{}", if self.awake.enabled() { "已开启" } else { "已关闭" }))];
        }
    }
}
fn in_bundle() -> bool {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false)
}
fn login_status() -> isize {
    unsafe {
        let s: Obj = msg_send![class!(SMAppService), mainAppService];
        msg_send![&s, status]
    }
}

extern "C-unwind" fn choose(_: &AnyObject, _: Sel, sender: *mut AnyObject) {
    let tag: isize = unsafe { msg_send![sender, tag] };
    action(|a| {
        let mut p = a.prefs.clone();
        p.style = match tag {
            1 => "glow",
            2 => "dark",
            _ => "light",
        }
        .into();
        a.activate(p)
    });
}
extern "C-unwind" fn resize(_: &AnyObject, _: Sel, sender: *mut AnyObject) {
    let tag: isize = unsafe { msg_send![sender, tag] };
    action(|a| {
        let mut p = a.prefs.clone();
        p.size = match tag {
            100 => 28.,
            101 => 35.,
            102 => 45.5,
            103 => 56.,
            _ => return Err("无效的指针尺寸".into()),
        };
        if p.style == "image" {
            p.hot.x *= p.size / a.prefs.size;
            p.hot.y *= p.size / a.prefs.size;
        }
        a.activate(p)
    });
}
extern "C-unwind" fn reset(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    action(|a| {
        let mut p = a.prefs.clone();
        p.style = "none".into();
        a.activate(p)
    });
}
extern "C-unwind" fn reapply(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    action(|a| a.engine.reapply());
}
extern "C-unwind" fn toggle_awake(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    action(|a| {
        if a.awake.enabled() {
            a.awake.disable()
        } else {
            a.awake.enable()
        }
    });
}
extern "C-unwind" fn wake(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    background_refresh();
}
fn background_refresh() {
    if BUSY.with(|b| b.replace(true)) {
        PENDING_EVENT.with(|p| p.set(true));
        return;
    }
    autoreleasepool(|_| {
        APP.with(|slot| {
            if let Some(a) = slot.borrow_mut().as_mut() {
                a.last_error = a.engine.reapply().err();
                a.rebuild();
            }
        });
    });
    BUSY.with(|b| b.set(false));
}
extern "C-unwind" fn import(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    action(|a| unsafe {
        let panel: Obj = msg_send![class!(NSOpenPanel), openPanel];
        let _: () = msg_send![&panel,setTitle:&*native::string("选择 PNG 图片或 .cape 主题")];
        let _: () = msg_send![&panel,setCanChooseDirectories:false];
        let _: () = msg_send![&panel,setAllowsMultipleSelection:false];
        let types = native::array();
        for ext in ["png", "cape"] {
            native::append(&types, &native::string(ext));
        }
        let _: () = msg_send![&panel,setAllowedFileTypes:&*types];
        let app: Obj = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![&app,activateIgnoringOtherApps:true];
        let result: isize = msg_send![&panel, runModal];
        if result != 1 {
            return Ok(());
        }
        let url: Obj = msg_send![&panel, URL];
        let path: Obj = msg_send![&url, path];
        let path = PathBuf::from(native::text(&path));
        let cape = path
            .extension()
            .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("cape"));
        // Validate the entire import before writing its managed copy or touching cursors.
        if cape {
            cursor::theme(&path)?;
        } else {
            cursor::Cursor::picture(&path, a.prefs.size, Point::new(0., 0.))?;
        }
        let dest = native::support()?.join(if cape {
            "imported.cape"
        } else {
            "imported.png"
        });
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let pending = dest.with_file_name(format!(
            "import-{stamp}.{}",
            if cape { "cape" } else { "png" }
        ));
        std::fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::copy(&path, &pending).map_err(|e| e.to_string())?;
        let mut p = a.prefs.clone();
        p.path = pending.to_string_lossy().into_owned();
        p.style = if cape { "cape" } else { "image" }.into();
        p.hot = Point::new(0., 0.);
        // Keep imports at a fresh path until application and preferences succeed.
        // If a user imports while an earlier theme is active, its file remains intact.
        a.activate(p)?;
        // A unique managed filename avoids corrupting an earlier import, and
        // stays at the same location that was atomically saved in preferences.
        if cape && !a.engine.skipped.is_empty() {
            native::alert(
                "主题已部分应用",
                &format!(
                    "已应用 {} 项；以下原始指针无法安全备份，保持原样：\n\n{}",
                    a.engine.active_count(),
                    a.engine.skipped.join("\n")
                ),
            );
        }
        Ok(())
    });
}
fn input_field(rect: Rect, value: &str) -> Obj {
    unsafe {
        let f: objc2::rc::Allocated<AnyObject> = msg_send![class!(NSTextField), alloc];
        let f: Obj = msg_send![f,initWithFrame:rect];
        let _: () = msg_send![&f,setStringValue:&*native::string(value)];
        f
    }
}
extern "C-unwind" fn hotspot(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    action(|a| unsafe {
        let alert: Obj = msg_send![class!(NSAlert), new];
        let _: () = msg_send![&alert,setMessageText:&*native::string("点击位置")];
        let _: () = msg_send![&alert,setInformativeText:&*native::string("从图片左上角计算，单位为点。X 向右、Y 向下。请将位置放在箭头尖端。")];
        let _: *mut AnyObject = msg_send![&alert,addButtonWithTitle:&*native::string("应用")];
        let _: *mut AnyObject = msg_send![&alert,addButtonWithTitle:&*native::string("取消")];
        let view: objc2::rc::Allocated<AnyObject> = msg_send![class!(NSView), alloc];
        let view: Obj =
            msg_send![view,initWithFrame:Rect::new(Point::new(0.,0.),Size::new(240.,64.))];
        let x = input_field(
            Rect::new(Point::new(24., 16.), Size::new(84., 24.)),
            &a.prefs.hot.x.to_string(),
        );
        let y = input_field(
            Rect::new(Point::new(150., 16.), Size::new(84., 24.)),
            &a.prefs.hot.y.to_string(),
        );
        for (field, label) in [(&x, "X"), (&y, "Y")] {
            let _: () = msg_send![field,setPlaceholderString:&*native::string(label)];
            let _: () = msg_send![field,setAccessibilityLabel:&*native::string(label)];
            let _: () = msg_send![&view,addSubview:&**field];
            let label_x = if label == "X" { 4. } else { 130. };
            let visible = input_field(
                Rect::new(Point::new(label_x, 16.), Size::new(20., 24.)),
                label,
            );
            let _: () = msg_send![&visible,setEditable:false];
            let _: () = msg_send![&visible,setSelectable:false];
            let _: () = msg_send![&visible,setBordered:false];
            let _: () = msg_send![&visible,setDrawsBackground:false];
            let _: () = msg_send![&view,addSubview:&*visible];
        }
        let _: () = msg_send![&alert,setAccessoryView:&*view];
        let result: isize = msg_send![&alert, runModal];
        if result != 1000 {
            return Ok(());
        }
        let xv: Obj = msg_send![&x, stringValue];
        let yv: Obj = msg_send![&y, stringValue];
        let mut p = a.prefs.clone();
        p.hot = Point::new(
            native::text(&xv).parse().map_err(|_| "X 请输入数字")?,
            native::text(&yv).parse().map_err(|_| "Y 请输入数字")?,
        );
        a.activate(p)
    });
}
extern "C-unwind" fn login(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    action(|_| unsafe {
        let service: Obj = msg_send![class!(SMAppService), mainAppService];
        let mut error: *mut AnyObject = std::ptr::null_mut();
        let success: bool = if login_status() == 1 {
            msg_send![&service,unregisterAndReturnError:&mut error]
        } else {
            msg_send![&service,registerAndReturnError:&mut error]
        };
        if !success {
            let detail = if error.is_null() {
                "请在系统设置 → 通用 → 登录项中检查权限".into()
            } else {
                let d: Obj = msg_send![error, localizedDescription];
                native::text(&d)
            };
            return Err(detail);
        }
        if login_status() == 2 {
            native::alert(
                "登录时启动",
                "请在系统设置 → 通用 → 登录项中允许Ashmactool。",
            )
        }
        Ok(())
    });
}
extern "C-unwind" fn about(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    action(|_| {
        native::alert(
            "Ashmactool 0.3.1",
            "Rust 编写的原生菜单栏工具箱。\n\n指针样式 · PNG / .cape 导入 · 保持屏幕唤醒\n\n屏幕唤醒防止闲置熄屏；合盖、手动睡眠和锁屏仍由系统处理。关闭开关或退出时释放。重启应用默认关闭。\n\n退出时恢复原始指针。系统沙滩球受接口限制以 24 帧恢复原生外观和周期，完整原始帧单独保留。\n\n私有光标接口思路来自 Alex Zielenski 的 Mousecape；参考 sdmj76 的现代版本。独立 Rust 实现采用 MIT 许可，第三方声明随包保留。\n\n部分应用自行绘制指针，可能覆盖系统主题。系统升级后可能需要兼容性更新。\n\n导入主题和原始指针备份保存在用户的 Application Support/Ashmactool 中。",
        );
        Ok(())
    });
}
extern "C-unwind" fn quit(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    action(|a| {
        a.awake.disable()?;
        a.engine.restore()?;
        unsafe {
            let app: Obj = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![&app,terminate:std::ptr::null::<AnyObject>()];
        }
        Ok(())
    });
}
extern "C-unwind" fn will_terminate(_: &AnyObject, _: Sel, _: *mut AnyObject) {
    if BUSY.with(|b| b.get()) {
        return;
    }
    APP.with(|s| {
        if let Some(a) = s.borrow_mut().as_mut() {
            if let Err(e) = a.awake.disable() {
                eprintln!("退出时释放电源请求失败：{e}");
            }
            if let Err(e) = a.engine.restore() {
                eprintln!("退出时恢复失败；下次启动可重试：{e}");
            }
        }
    });
}

pub fn run(smoke_test: bool) -> Result<()> {
    autoreleasepool(|_| unsafe {
        let settings_path = native::support()?.join("settings.plist");
        let saved_settings = if smoke_test && settings_path.exists() {
            Some(std::fs::read(&settings_path).map_err(|e| e.to_string())?)
        } else {
            None
        };
        let mut builder =
            ClassBuilder::new(c"AshmactoolActions", class!(NSObject)).ok_or("无法注册菜单动作")?;
        for (selector, callback) in [
            (
                sel!(choose:),
                choose as extern "C-unwind" fn(&AnyObject, Sel, *mut AnyObject),
            ),
            (sel!(resize:), resize),
            (sel!(reset:), reset),
            (sel!(reapply:), reapply),
            (sel!(import:), import),
            (sel!(hotspot:), hotspot),
            (sel!(login:), login),
            (sel!(toggleAwake:), toggle_awake),
            (sel!(about:), about),
            (sel!(quit:), quit),
            (sel!(wake:), wake),
            (sel!(applicationWillTerminate:), will_terminate),
        ] {
            builder.add_method(selector, callback as extern "C-unwind" fn(_, _, _));
        }
        let target: Obj = msg_send![builder.register(), new];
        let app: Obj = msg_send![class!(NSApplication), sharedApplication];
        let _: bool = msg_send![&app,setActivationPolicy:1isize];
        let _: () = msg_send![&app,setDelegate:&*target];
        let statusbar: Obj = msg_send![class!(NSStatusBar), systemStatusBar];
        let status: Obj = msg_send![&statusbar,statusItemWithLength:-1f64];
        let button: Obj = msg_send![&status, button];
        let image: Option<Obj> = msg_send![class!(NSImage),imageWithSystemSymbolName:&*native::string("wrench.and.screwdriver"),accessibilityDescription:&*native::string("Ashmactool")];
        if let Some(image) = image {
            let _: () = msg_send![&image,setTemplate:true];
            let _: () = msg_send![&image,setSize:Size::new(18.,18.)];
            let _: () = msg_send![&button,setImage:&*image];
        } else {
            let _: () = msg_send![&button,setTitle:&*native::string("A")];
        }
        let menu: Obj = msg_send![class!(NSMenu), new];
        let _: () = msg_send![&menu,setAutoenablesItems:false];
        let _: () = msg_send![&status,setMenu:&*menu];
        let mut engine = Engine::new(native::support()?.join("original.cape"))?;
        engine.restore()?;
        let prefs = Preferences::load()?;
        let cursor_menu: Obj = msg_send![class!(NSMenu), new];
        let size_menu: Obj = msg_send![class!(NSMenu), new];
        for submenu in [&cursor_menu, &size_menu] {
            let _: () = msg_send![submenu,setAutoenablesItems:false];
        }
        let mut state = App {
            engine,
            prefs: prefs.clone(),
            menu,
            cursor_menu,
            size_menu,
            awake: KeepAwake::default(),
            target: target.clone(),
            status,
            last_error: None,
        };
        if prefs.style != "none" {
            if let Err(e) = state.activate(prefs) {
                state.last_error = Some(e.clone());
                native::alert("无法应用保存的指针", &e);
            }
        }
        state.rebuild();
        APP.with(|slot| *slot.borrow_mut() = Some(state));
        let workspace: Obj = msg_send![class!(NSWorkspace), sharedWorkspace];
        let center: Obj = msg_send![&workspace, notificationCenter];
        for n in [
            "NSWorkspaceDidWakeNotification",
            "NSWorkspaceSessionDidBecomeActiveNotification",
            "NSWorkspaceActiveSpaceDidChangeNotification",
        ] {
            let _: () = msg_send![&center,addObserver:&*target,selector:sel!(wake:),name:&*native::string(n),object:std::ptr::null::<AnyObject>()];
        }
        let center: Obj = msg_send![class!(NSNotificationCenter), defaultCenter];
        let _: () = msg_send![&center,addObserver:&*target,selector:sel!(wake:),name:&*native::string("NSApplicationDidChangeScreenParametersNotification"),object:std::ptr::null::<AnyObject>()];
        if smoke_test {
            // Exercise the same native menu selector dispatch as a user click,
            // without driving other applications or changing login settings.
            let checked = (|| -> Result<()> {
                let menu = APP.with(|s| s.borrow().as_ref().unwrap().menu.clone());
                let cursors = APP.with(|s| s.borrow().as_ref().unwrap().cursor_menu.clone());
                let sizes = APP.with(|s| s.borrow().as_ref().unwrap().size_menu.clone());
                let item: Obj = msg_send![&cursors,itemWithTag:1isize];
                let dispatched: bool =
                    msg_send![&app,sendAction:sel!(choose:),to:&*target,from:&*item];
                if !dispatched
                    || APP.with(|s| s.borrow().as_ref().unwrap().engine.active_count() == 0)
                {
                    return Err("菜单样式动作未生效".into());
                }
                let smaller: Obj = msg_send![&sizes,itemWithTag:100isize];
                let dispatched: bool =
                    msg_send![&app,sendAction:sel!(resize:),to:&*target,from:&*smaller];
                if !dispatched || APP.with(|s| s.borrow().as_ref().unwrap().prefs.size != 28.) {
                    return Err("菜单更小尺寸动作未生效".into());
                }
                APP.with(|s| -> Result<()> {
                    let slot = s.borrow();
                    let a = slot.as_ref().unwrap();
                    for name in a.engine.api.names() {
                        let c = a.engine.api.snapshot(&name)?;
                        if c.size.width != 28. || c.hot != Point::new(5.6875, 4.375) {
                            return Err(format!("{name}: 更小尺寸系统回读失败"));
                        }
                    }
                    Ok(())
                })?;
                let awake: Obj = msg_send![&menu,itemWithTag:20isize];
                let dispatched: bool =
                    msg_send![&app,sendAction:sel!(toggleAwake:),to:&*target,from:&*awake];
                if !dispatched {
                    return Err("屏幕唤醒菜单未分发".into());
                }
                APP.with(|s| s.borrow().as_ref().unwrap().awake.verify())?;
                // Rebuild creates new menu items; look up the current one again.
                let awake: Obj = msg_send![&menu,itemWithTag:20isize];
                let dispatched: bool =
                    msg_send![&app,sendAction:sel!(toggleAwake:),to:&*target,from:&*awake];
                if !dispatched || APP.with(|s| s.borrow().as_ref().unwrap().awake.enabled()) {
                    return Err("关闭屏幕唤醒菜单未生效".into());
                }
                let reset_item: Obj = msg_send![&cursors,itemWithTag:7isize];
                let dispatched: bool =
                    msg_send![&app,sendAction:sel!(reset:),to:&*target,from:&*reset_item];
                if !dispatched
                    || APP.with(|s| s.borrow().as_ref().unwrap().engine.active_count() != 0)
                {
                    return Err("菜单恢复动作未生效".into());
                }
                println!(
                    "native_menu_dispatch=passed style_switch=passed smaller_28pt_readback=passed awake_toggle=passed reset=passed"
                );
                Ok(())
            })();
            let power_clean = APP.with(|s| s.borrow_mut().as_mut().unwrap().awake.disable());
            let restored = APP.with(|s| s.borrow_mut().as_mut().unwrap().engine.restore());
            let clean = match saved_settings {
                Some(bytes) => std::fs::write(&settings_path, bytes),
                None => {
                    if settings_path.exists() {
                        std::fs::remove_file(&settings_path)
                    } else {
                        Ok(())
                    }
                }
            }
            .map_err(|e| e.to_string());
            checked?;
            power_clean?;
            restored?;
            clean?;
            let _: () = msg_send![&statusbar,removeStatusItem:&*APP.with(|s|s.borrow().as_ref().unwrap().status.clone())];
            return Ok(());
        }
        let _: () = msg_send![&app, run];
        Ok(())
    })
}

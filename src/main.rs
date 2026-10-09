#![cfg(target_os = "macos")]
mod app;
mod cursor;
mod graphics;
mod native;
mod power;
use native::Result;
use objc2::{class, msg_send, rc::autoreleasepool};
use std::{fs::File, os::fd::AsRawFd, path::PathBuf};

unsafe extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}
fn singleton() -> Result<File> {
    let dir = native::support()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join("app.lock"))
        .map_err(|e| e.to_string())?;
    if unsafe { flock(f.as_raw_fd(), 2 | 4) } != 0 {
        return Err("Ashmactool已经在运行，请使用菜单栏图标操作。".into());
    }
    Ok(f)
}
fn main() {
    if let Err(e) = autoreleasepool(|_| entry()) {
        eprintln!("{e}");
        if std::env::args().len() == 1 {
            native::alert("Ashmactool", &e);
        }
        std::process::exit(1);
    }
}
fn entry() -> Result<()> {
    let args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|s| !s.starts_with("-psn_"))
        .collect();
    // Initialize AppKit before opening a WindowServer cursor connection.
    let _app: native::Obj = unsafe { msg_send![class!(NSApplication), sharedApplication] };
    match args.first().map(String::as_str) {
        Some("--help") => {
            println!(
                "Ashmactool 0.3.1\n默认：运行菜单栏应用\n--doctor：检查接口与系统箭头，不修改指针\n--inspect FILE.cape：验证主题，不修改指针\n--preview DIRECTORY：导出箭头 PNG\n--preview-all DIRECTORY：导出整套指针预览\n--restore：恢复本工具保存的原始指针\n--self-test：短暂注册内置指针，检查后立即恢复\n--power-self-test：回读屏幕唤醒请求，再释放并回读\n--ui-smoke-test：验证原生菜单动作并恢复测试前配置"
            );
            Ok(())
        }
        Some("--doctor") => {
            let api = cursor::Api::load()?;
            println!("private_api=available connection={}", api.cid);
            for name in api.discover() {
                match api.snapshot(&name) {
                    Ok(c) => println!(
                        "{name}: {}×{}pt hotspot=({}, {}) frames={} representations={}",
                        c.size.width,
                        c.size.height,
                        c.hot.x,
                        c.hot.y,
                        c.frames,
                        native::count(&c.images)
                    ),
                    Err(e) => println!("{name}: {e}"),
                }
            }
            Ok(())
        }
        Some("--inspect") => {
            let path = PathBuf::from(args.get(1).ok_or("缺少主题文件路径")?);
            let theme = cursor::theme(&path)?;
            println!("valid_cursors={}", theme.len());
            for (name, c) in theme {
                println!(
                    "{name} {}×{}pt frames={}",
                    c.size.width, c.size.height, c.frames
                );
            }
            Ok(())
        }
        Some("--preview") => {
            let dir = PathBuf::from(args.get(1).ok_or("缺少输出目录")?);
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            for style in ["glow", "dark", "light"] {
                let image = graphics::preset(style, 2)?;
                std::fs::write(
                    dir.join(format!("{style}.png")),
                    native::bytes(&*image.png()?),
                )
                .map_err(|e| e.to_string())?;
            }
            println!("preview={}", dir.display());
            Ok(())
        }
        Some("--preview-all") => {
            let dir = PathBuf::from(args.get(1).ok_or("缺少输出目录")?);
            let api = cursor::Api::load()?;
            for style in ["glow", "dark", "light"] {
                let folder = dir.join(style);
                std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                for (name, cursor) in api.preset_theme(style, 56.)? {
                    let rep = native::at(&cursor.images, 1);
                    unsafe extern "C" {
                        fn CGImageRetain(p: *const std::ffi::c_void) -> *const std::ffi::c_void;
                    }
                    let image = graphics::Image(unsafe {
                        CGImageRetain((&*rep as *const objc2::runtime::AnyObject).cast())
                    });
                    let frame = graphics::first_frame(&image, cursor.frames)?;
                    let path = folder.join(format!("{name}.png"));
                    std::fs::write(path, native::bytes(&*frame.png()?))
                        .map_err(|e| e.to_string())?;
                }
            }
            println!("full_theme_preview={}", dir.display());
            Ok(())
        }
        Some("--restore") => {
            let _lock = singleton()?;
            let mut engine = cursor::Engine::new(native::support()?.join("original.cape"))?;
            engine.restore()?;
            println!("restored=true");
            Ok(())
        }
        Some("--power-self-test") => power::KeepAwake::self_test(),
        Some("--self-test") => {
            let _lock = singleton()?;
            let mut engine = cursor::Engine::new(native::support()?.join("original.cape"))?;
            engine.restore()?;
            let names = engine.api.discover();
            let baseline: Vec<_> = names
                .iter()
                .filter_map(|n| engine.api.snapshot(n).ok().map(|c| (n.clone(), c)))
                .collect();
            let style = args.get(1).map(String::as_str).unwrap_or("glow");
            if !["glow", "dark", "light"].contains(&style) {
                return Err("未知主题样式".into());
            }
            let size: f64 = args
                .get(2)
                .map(|s| s.parse().map_err(|_| "无效尺寸"))
                .transpose()?
                .unwrap_or(28.);
            engine.preset(style, size)?;
            let checked = engine.verify_active();
            if let Err(e) = &checked {
                eprintln!("theme_readback_error={e}");
            }
            println!(
                "theme_states={} full_theme_registration_readback={}",
                engine.active_count(),
                if checked.is_ok() { "passed" } else { "failed" }
            );
            // Re-open the disk snapshot as a restarted app would. Keep the
            // original in-memory backup until the on-disk restoration succeeds.
            let restored = (|| -> Result<()> {
                let mut restarted = cursor::Engine::new(native::support()?.join("original.cape"))?;
                restarted.restore()
            })();
            if restored.is_err() {
                engine.restore()?;
            }
            restored?;
            checked?;
            for (name, before) in baseline {
                let after = engine.api.snapshot(&name)?;
                let before = if before.frames > 24 {
                    before.resampled(24)?
                } else {
                    before
                };
                if before.size != after.size
                    || before.hot != after.hot
                    || before.frames != after.frames
                    || (before.duration - after.duration).abs() > 0.00001
                    || before.pixels()? != after.pixels()?
                {
                    return Err(format!("{name}: restore readback mismatch"));
                }
            }
            println!(
                "registration_readback=passed disk_restore_readback=passed pixels_restored=passed backup_removed=true"
            );
            Ok(())
        }
        Some("--ui-smoke-test") => {
            let _lock = singleton()?;
            app::run(true)
        }
        Some(s) => Err(format!("未知参数 {s}；使用 --help")),
        None => {
            let _lock = singleton()?;
            app::run(false)
        }
    }
}

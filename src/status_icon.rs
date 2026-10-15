//! A single circular Wi-Fi/battery mark with shared geometry, rendered at display scale.
use crate::{
    native::{self, Obj, Result},
    system_status::{Battery, Snapshot, WifiState},
};
use objc2::{
    class, msg_send,
    runtime::{AnyObject, ClassBuilder, Sel},
    sel,
};
use objc2_core_foundation::{CGPoint as Point, CGRect as Rect, CGSize as Size};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    sync::OnceLock,
};
const OUTER_RADIUS: f64 = 8.;
const STROKE: f64 = 1.7;
const WIFI_START: f64 = 35.;
const WIFI_END: f64 = 145.;
const BATTERY_START: f64 = 167.;
const BATTERY_END: f64 = 373.;
const BOLT_GAP_START: f64 = 242.;
const BOLT_GAP_END: f64 = 298.;
thread_local! { static SHOW_PERCENT:Cell<bool>=const{Cell::new(false)}; static STATE: RefCell<Snapshot> = RefCell::new(Snapshot::fixture(Battery::Unavailable, WifiState::Unavailable)); }

pub fn set_state(snapshot: &Snapshot, show_percent: bool) {
    SHOW_PERCENT.with(|s| s.set(show_percent));
    STATE.with(|s| *s.borrow_mut() = snapshot.clone());
}
pub fn width(snapshot: &Snapshot, show_percent: bool) -> f64 {
    if show_percent && snapshot.battery.percent_label().is_some() {
        64.
    } else {
        28.
    }
}
pub fn install(button: &AnyObject) -> Result<Obj> {
    static VIEW_CLASS: OnceLock<&'static objc2::runtime::AnyClass> = OnceLock::new();
    let cls = VIEW_CLASS.get_or_init(|| {
        let mut b =
            ClassBuilder::new(c"AshmactoolStatusIcon", class!(NSView)).expect("unique icon class");
        unsafe {
            b.add_method(sel!(drawRect:), draw_rect as extern "C-unwind" fn(_, _, _));
            b.add_method(
                sel!(viewDidChangeEffectiveAppearance),
                appearance_changed as extern "C-unwind" fn(_, _),
            );
            b.add_method(
                sel!(hitTest:),
                hit_test as extern "C-unwind" fn(_, _, _) -> *mut AnyObject,
            );
            b.add_method(
                sel!(isAccessibilityElement),
                not_accessible as extern "C-unwind" fn(_, _) -> objc2::runtime::Bool,
            );
        }
        b.register()
    });
    unsafe {
        let bounds: Rect = msg_send![button, bounds];
        let view: objc2::rc::Allocated<AnyObject> = msg_send![*cls, alloc];
        let view: Obj = msg_send![view, initWithFrame: bounds];
        let _: () = msg_send![&view, setAutoresizingMask: 18usize];
        let _: () = msg_send![button, addSubview: &*view];
        Ok(view)
    }
}
extern "C-unwind" fn not_accessible(_: &AnyObject, _: Sel) -> objc2::runtime::Bool {
    objc2::runtime::Bool::NO
}
extern "C-unwind" fn hit_test(_: &AnyObject, _: Sel, _: Point) -> *mut AnyObject {
    std::ptr::null_mut()
}
extern "C-unwind" fn appearance_changed(view: &AnyObject, _: Sel) {
    unsafe {
        let _: () = msg_send![
            super(view, class!(NSView)),
            viewDidChangeEffectiveAppearance
        ];
        let _: () = msg_send![view, setNeedsDisplay: true];
    }
}
extern "C-unwind" fn draw_rect(view: &AnyObject, _: Sel, _: Rect) {
    objc2::rc::autoreleasepool(|_| unsafe {
        let bounds: Rect = msg_send![view, bounds];
        let button: Option<Obj> = msg_send![view, superview];
        let selected = button
            .map(|b| {
                let cell: Obj = msg_send![&b, cell];
                let h: bool = msg_send![&cell, isHighlighted];
                h
            })
            .unwrap_or(false);
        let color: Obj = if selected {
            msg_send![class!(NSColor), selectedMenuItemTextColor]
        } else {
            msg_send![class!(NSColor), labelColor]
        };
        let snapshot = STATE.with(|s| s.borrow().clone());
        let _ = draw_status(
            &snapshot,
            SHOW_PERCENT.with(|s| s.get()),
            &color,
            bounds.size,
        );
    });
}
fn draw_status(s: &Snapshot, show_percent: bool, color: &AnyObject, size: Size) -> Result<()> {
    unsafe {
        let text = if show_percent {
            s.battery.percent_label()
        } else {
            None
        };
        let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
        let t: Obj = msg_send![class!(NSAffineTransform), transform];
        let x = if text.is_some() {
            3.
        } else {
            (size.width - 22.) / 2.
        };
        let _: () = msg_send![&t,translateXBy:x,yBy:(size.height-22.)/2.];
        let _: () = msg_send![&t, concat];
        let result = draw(s, color);
        let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
        result?;
        if let Some(text) = text {
            let attrs = native::dictionary();
            let font: Obj =
                msg_send![class!(NSFont),monospacedDigitSystemFontOfSize:11f64,weight:0.23f64];
            native::insert(&attrs, "NSFont", &font);
            native::insert(&attrs, "NSColor", color);
            let text = native::string(&text);
            let measured: Size = msg_send![&*text, sizeWithAttributes:&*attrs];
            let _: () = msg_send![&*text, drawAtPoint:Point::new(27., (size.height-measured.height)/2.), withAttributes:&*attrs];
        }
        Ok(())
    }
}
fn rgb(r: f64, g: f64, b: f64, a: f64) -> Obj {
    unsafe { msg_send![class!(NSColor), colorWithSRGBRed:r, green:g, blue:b, alpha:a] }
}
fn arc_segment(color: &AnyObject, start: f64, end: f64, radius: f64, width: f64) {
    unsafe {
        let path: Obj = msg_send![class!(NSBezierPath), bezierPath];
        let _: () = msg_send![&path, setLineWidth:width];
        let _: () = msg_send![&path, setLineCapStyle:1usize];
        let _: () = msg_send![&path, appendBezierPathWithArcWithCenter:Point::new(11.,11.), radius:radius, startAngle:start, endAngle:end, clockwise:false];
        let _: () = msg_send![color, setStroke];
        let _: () = msg_send![&path, stroke];
    }
}
fn battery_arc(color: &AnyObject, end: f64, show_bolt: bool) {
    if show_bolt {
        // The charge bolt is part of the circumference, centered at 270°.
        if end > BATTERY_START {
            arc_segment(
                color,
                BATTERY_START,
                end.min(BOLT_GAP_START),
                OUTER_RADIUS,
                STROKE,
            );
        }
        if end > BOLT_GAP_END {
            arc_segment(color, BOLT_GAP_END, end, OUTER_RADIUS, STROKE);
        }
    } else {
        arc_segment(color, BATTERY_START, end, OUTER_RADIUS, STROKE);
    }
}
fn symbol(name: &str, value: f64, rect: Rect, color: &AnyObject) -> Result<()> {
    unsafe {
        let image: Option<Obj> = msg_send![class!(NSImage), imageWithSystemSymbolName:&*native::string(name), variableValue:value, accessibilityDescription:std::ptr::null::<AnyObject>()];
        let image = image.ok_or_else(|| format!("无法绘制系统符号 {name}"))?;
        let weight: Obj = msg_send![class!(NSImageSymbolConfiguration), configurationWithPointSize:17f64, weight:0.23f64];
        let image: Obj = msg_send![&image, imageWithSymbolConfiguration:&*weight];
        let palette: Obj =
            msg_send![class!(NSImageSymbolConfiguration), configurationWithHierarchicalColor:color];
        let image: Obj = msg_send![&image, imageWithSymbolConfiguration:&*palette];
        let _: () = msg_send![&image, drawInRect:rect, fromRect:Rect::new(Point::new(0.,0.),Size::new(0.,0.)), operation:2usize, fraction:1f64, respectFlipped:false, hints:std::ptr::null::<AnyObject>()];
        Ok(())
    }
}
fn draw(s: &Snapshot, foreground: &AnyObject) -> Result<()> {
    unsafe {
        let track: Obj = msg_send![foreground, colorWithAlphaComponent:0.22f64];
        battery_arc(&track, BATTERY_END, s.battery.show_bolt());
        let battery_color: Obj = if s.battery.charging() {
            msg_send![class!(NSColor), systemGreenColor]
        } else if s.battery.low() {
            msg_send![class!(NSColor), systemRedColor]
        } else {
            Obj::retain(foreground as *const AnyObject as *mut AnyObject).unwrap()
        };
        if let Some(progress) = s.battery.progress() {
            if progress > 0. {
                battery_arc(
                    &battery_color,
                    BATTERY_START + (BATTERY_END - BATTERY_START) * progress,
                    s.battery.show_bolt(),
                );
            }
        }
        // Unknown charge leaves only the continuous shaded track, without
        // bright segments. Wi-Fi and battery still share one circular edge.
        let connected = matches!(s.wifi.state, WifiState::Connected | WifiState::Sharing);
        let strength = s.wifi.strength();
        for (radius, threshold) in [(OUTER_RADIUS, 0.9), (5., 0.5)] {
            let broken = matches!(s.wifi.state, WifiState::Off | WifiState::Unavailable);
            let color: Obj = if broken {
                msg_send![foreground,colorWithAlphaComponent:0.45f64]
            } else if connected && strength >= threshold {
                Obj::retain(foreground as *const AnyObject as *mut AnyObject).unwrap()
            } else {
                msg_send![foreground,colorWithAlphaComponent:0.27f64]
            };
            if broken && radius == OUTER_RADIUS {
                for (start, end) in [(35., 76.), (104., 145.)] {
                    arc_segment(&color, start, end, radius, STROKE);
                }
            } else {
                arc_segment(&color, WIFI_START, WIFI_END, radius, STROKE);
            }
        }
        let dot: Obj = msg_send![class!(NSBezierPath),bezierPathWithOvalInRect:Rect::new(Point::new(9.3,9.3),Size::new(3.4,3.4))];
        let dot_color: Obj = if s.bluetooth.connected() {
            msg_send![class!(NSColor), systemBlueColor]
        } else {
            msg_send![foreground,colorWithAlphaComponent:if connected {1f64}else{0.27f64}]
        };
        let _: () = msg_send![&dot_color, setFill];
        let _: () = msg_send![&dot, fill];
        if s.battery.show_bolt() {
            symbol(
                "bolt.fill",
                1.,
                Rect::new(Point::new(8.2, 0.0), Size::new(5.6, 7.7)),
                &battery_color,
            )?;
        }
    }
    Ok(())
}
fn bitmap(
    width: usize,
    height: usize,
    scale: usize,
    draw: impl FnOnce() -> Result<()>,
) -> Result<Obj> {
    unsafe {
        let rep: objc2::rc::Allocated<AnyObject> = msg_send![class!(NSBitmapImageRep), alloc];
        let rep: Obj = msg_send![rep,initWithBitmapDataPlanes:std::ptr::null_mut::<*mut u8>(),pixelsWide:(width*scale) as isize,pixelsHigh:(height*scale) as isize,bitsPerSample:8isize,samplesPerPixel:4isize,hasAlpha:true,isPlanar:false,colorSpaceName:&*native::string("NSCalibratedRGBColorSpace"),bytesPerRow:(width*scale*4) as isize,bitsPerPixel:32isize];
        let context: Option<Obj> =
            msg_send![class!(NSGraphicsContext),graphicsContextWithBitmapImageRep:&*rep];
        let context = context.ok_or("无法创建图标预览上下文")?;
        let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
        let _: () = msg_send![class!(NSGraphicsContext),setCurrentContext:&*context];
        let t: Obj = msg_send![class!(NSAffineTransform), transform];
        let _: () = msg_send![&t,scaleXBy:scale as f64,yBy:scale as f64];
        let _: () = msg_send![&t, concat];
        let result = draw();
        let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
        result?;
        Ok(rep)
    }
}
fn write_png(path: &Path, rep: &AnyObject) -> Result<()> {
    let data: Option<Obj> =
        unsafe { msg_send![rep,representationUsingType:4usize,properties:&*native::dictionary()] };
    std::fs::write(path, native::bytes(&*data.ok_or("PNG 编码失败")?)).map_err(|e| e.to_string())
}
fn label(text: &str, point: Point, size: f64, color: &AnyObject) {
    unsafe {
        let attrs = native::dictionary();
        let font: Obj = msg_send![class!(NSFont),systemFontOfSize:size];
        native::insert(&attrs, "NSFont", &font);
        native::insert(&attrs, "NSColor", color);
        let _: () = msg_send![&*native::string(text),drawAtPoint:point,withAttributes:&*attrs];
    }
}
pub fn export(directory: &Path) -> Result<()> {
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let mut scenarios = vec![
        (
            "82% · 使用电池",
            Battery::Internal {
                percent: Some(82.),
                charging: false,
                external: false,
                full: false,
                minutes: None,
            },
            WifiState::Connected,
        ),
        (
            "48% · 正在充电",
            Battery::Internal {
                percent: Some(48.),
                charging: true,
                external: true,
                full: false,
                minutes: None,
            },
            WifiState::Connected,
        ),
        (
            "15% · 低电量",
            Battery::Internal {
                percent: Some(15.),
                charging: false,
                external: false,
                full: false,
                minutes: None,
            },
            WifiState::Connected,
        ),
        ("桌面 Mac · 无电池", Battery::None, WifiState::Connected),
        (
            "Wi-Fi 已关闭",
            Battery::Internal {
                percent: Some(64.),
                charging: false,
                external: false,
                full: false,
                minutes: None,
            },
            WifiState::Off,
        ),
        (
            "未连接 · 电量未知",
            Battery::Unavailable,
            WifiState::Disconnected,
        ),
        (
            "80% · 接电未充电",
            Battery::Internal {
                percent: Some(80.),
                charging: false,
                external: true,
                full: false,
                minutes: None,
            },
            WifiState::Connected,
        ),
    ];
    scenarios.extend([
        (
            "Wi-Fi · 蓝牙已连接",
            Battery::Internal {
                percent: Some(80.),
                charging: false,
                external: true,
                full: false,
                minutes: None,
            },
            WifiState::Connected,
        ),
        (
            "Wi-Fi 关闭 · 蓝牙已连接",
            Battery::Internal {
                percent: Some(64.),
                charging: false,
                external: false,
                full: false,
                minutes: None,
            },
            WifiState::Off,
        ),
    ]);
    let snapshot_for = |index: usize, battery: &Battery, wifi: &WifiState| {
        let mut s = Snapshot::fixture(battery.clone(), wifi.clone());
        if index >= 7 {
            s.bluetooth = crate::bluetooth::State::Connected;
        }
        s
    };
    for (dark, name) in [(false, "light"), (true, "dark")] {
        let color = if dark {
            rgb(1., 1., 1., 1.)
        } else {
            rgb(0.08, 0.08, 0.09, 1.)
        };
        for (i, (_, battery, wifi)) in scenarios.iter().enumerate() {
            let snapshot = snapshot_for(i, battery, wifi);
            for scale in [1, 2] {
                let rep = bitmap(22, 22, scale, || draw(&snapshot, &color))?;
                write_png(&directory.join(format!("{name}-{i}@{scale}x.png")), &rep)?;
            }
        }
    }
    let height = 96 + scenarios.len() * 84;
    let board = bitmap(760, height, 2, || unsafe {
        for (column, dark) in [false, true].into_iter().enumerate() {
            let bg = if dark {
                rgb(0.09, 0.10, 0.12, 1.)
            } else {
                rgb(0.96, 0.96, 0.97, 1.)
            };
            let fg = if dark {
                rgb(0.96, 0.97, 1., 1.)
            } else {
                rgb(0.10, 0.11, 0.13, 1.)
            };
            let x = column as f64 * 380.;
            let path: Obj = msg_send![class!(NSBezierPath),bezierPathWithRect:Rect::new(Point::new(x,0.),Size::new(380.,height as f64))];
            let _: () = msg_send![&bg, setFill];
            let _: () = msg_send![&path, fill];
            label(
                if dark {
                    "深色菜单栏"
                } else {
                    "浅色菜单栏"
                },
                Point::new(x + 28., height as f64 - 50.),
                18.,
                &fg,
            );
            for (i, (title, battery, wifi)) in scenarios.iter().enumerate() {
                let y = height as f64 - 135. - i as f64 * 84.;
                let s = snapshot_for(i, battery, wifi);
                let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
                let t: Obj = msg_send![class!(NSAffineTransform), transform];
                let _: () = msg_send![&t,translateXBy:x+32.,yBy:y+14.];
                let _: () = msg_send![&t, concat];
                draw(&s, &fg)?;
                let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
                let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
                let t: Obj = msg_send![class!(NSAffineTransform), transform];
                let _: () = msg_send![&t,translateXBy:x+94.,yBy:y];
                let _: () = msg_send![&t,scaleBy:2.5f64];
                let _: () = msg_send![&t, concat];
                draw(&s, &fg)?;
                let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
                label(title, Point::new(x + 176., y + 18.), 13., &fg);
            }
        }
        Ok(())
    })?;
    write_png(&directory.join("menu-bar.png"), &board)?;
    let percent_board = bitmap(420, 156, 2, || unsafe {
        for (col, dark) in [false, true].into_iter().enumerate() {
            let bg = if dark {
                rgb(0.09, 0.10, 0.12, 1.)
            } else {
                rgb(0.96, 0.96, 0.97, 1.)
            };
            let fg = if dark {
                rgb(0.96, 0.97, 1., 1.)
            } else {
                rgb(0.10, 0.11, 0.13, 1.)
            };
            let x = col as f64 * 210.;
            let path: Obj = msg_send![class!(NSBezierPath),bezierPathWithRect:Rect::new(Point::new(x,0.),Size::new(210.,156.))];
            let _: () = msg_send![&bg, setFill];
            let _: () = msg_send![&path, fill];
            for (row, index) in [7usize, 1, 4].into_iter().enumerate() {
                let (_, battery, wifi) = &scenarios[index];
                let s = snapshot_for(index, battery, wifi);
                let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
                let t: Obj = msg_send![class!(NSAffineTransform), transform];
                let _: () = msg_send![&t,translateXBy:x+36.,yBy:112.-row as f64*42.];
                let _: () = msg_send![&t,scaleBy:2f64];
                let _: () = msg_send![&t, concat];
                draw_status(&s, true, &fg, Size::new(64., 22.))?;
                let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
            }
        }
        Ok(())
    })?;
    write_png(&directory.join("percentage.png"), &percent_board)
}

/// Read-only raster verification using the same drawing path as the live icon.
pub fn self_test() -> Result<()> {
    let color = rgb(1., 1., 1., 1.);
    let pixels_for = |s: &Snapshot| -> Result<Vec<u8>> {
        let rep = bitmap(22, 22, 2, || draw(s, &color))?;
        unsafe {
            let row: isize = msg_send![&rep, bytesPerRow];
            let p: *const u8 = msg_send![&rep, bitmapData];
            if row != 44 * 4 || p.is_null() {
                return Err("图标像素格式无效".into());
            }
            Ok(std::slice::from_raw_parts(p, (row * 44) as usize).to_vec())
        }
    };
    let pixels = |battery| pixels_for(&Snapshot::fixture(battery, WifiState::Connected));
    let battery = |percent, charging| Battery::Internal {
        percent: Some(percent),
        charging,
        external: charging,
        full: percent == 100.,
        minutes: None,
    };
    // Use non-low charge levels so the alpha test measures progress, not red tint.
    let empty = pixels(battery(0., false))?;
    let half = pixels(battery(50., false))?;
    let full = pixels(battery(100., false))?;
    let desktop = pixels(Battery::None)?;
    let unknown = pixels(Battery::Unavailable)?;
    let solid = |p: &[u8]| p.chunks_exact(4).filter(|v| v[3] > 230).count();
    if !(solid(&empty) < solid(&half) && solid(&half) < solid(&full))
        || full != desktop
        || unknown == desktop
        || unknown != empty
    {
        return Err("电量进度或无电池/未知状态绘制验证失败".into());
    }
    let charging = pixels(battery(50., true))?;
    let green =
        |v: &[u8]| v[1] > v[0].saturating_add(30) && v[1] > v[2].saturating_add(20) && v[3] > 128;
    if !charging.chunks_exact(4).any(green) || half.chunks_exact(4).any(green) {
        return Err("充电绿色状态验证失败".into());
    }
    let plugged = pixels(Battery::Internal {
        percent: Some(50.),
        charging: false,
        external: true,
        full: false,
        minutes: None,
    })?;
    if plugged == half
        || plugged.chunks_exact(4).any(green)
        || !plugged
            .chunks_exact(4)
            .zip(charging.chunks_exact(4))
            .all(|(a, b)| a[3] == b[3])
    {
        return Err("接电未充电的闪电形状或配色验证失败".into());
    }
    let mut bt = Snapshot::fixture(battery(50., false), WifiState::Connected);
    bt.bluetooth = crate::bluetooth::State::Connected;
    let bt_pixels = pixels_for(&bt)?;
    let blue =
        |v: &[u8]| v[2] > v[0].saturating_add(40) && v[2] > v[1].saturating_add(20) && v[3] > 128;
    if !bt_pixels.chunks_exact(4).any(blue) || half.chunks_exact(4).any(blue) {
        return Err("蓝牙圆点配色验证失败".into());
    }
    for (index, (a, b)) in bt_pixels
        .chunks_exact(4)
        .zip(half.chunks_exact(4))
        .enumerate()
    {
        if a != b {
            let (x, y) = (index % 44, index / 44);
            if !(17..=27).contains(&x) || !(17..=27).contains(&y) {
                return Err("蓝牙颜色影响了圆点以外的区域".into());
            }
        }
    }
    bt.wifi.state = WifiState::Off;
    if !pixels_for(&bt)?.chunks_exact(4).any(blue) {
        return Err("Wi-Fi 关闭时蓝牙蓝点丢失".into());
    }
    println!(
        "battery_arc_progress=passed desktop_full_arc=passed unknown_shaded_track=passed charge_color=passed adapter_bolt=passed bluetooth_dot=passed independent_wifi_state=passed"
    );
    Ok(())
}

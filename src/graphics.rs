//! Small vector presets drawn by CoreGraphics; no external artwork or renderer.
use crate::native::{self, Obj, Result};
use objc2::{class, msg_send, runtime::AnyObject};
use objc2_core_foundation::{CGPoint as Point, CGRect as Rect, CGSize as Size};
use std::{ffi::c_void, path::Path};

type Ptr = *const c_void;
// Give Objective-C the exact native pointer encoding rather than an untyped
// void pointer. objc2 validates this in debug builds at the message boundary.
#[repr(C)]
struct CGImageOpaque {
    _private: [u8; 0],
}
unsafe impl objc2::encode::RefEncode for CGImageOpaque {
    const ENCODING_REF: objc2::encode::Encoding =
        objc2::encode::Encoding::Pointer(&objc2::encode::Encoding::Struct("CGImage", &[]));
}
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGColorSpaceCreateDeviceRGB() -> Ptr;
    fn CGColorSpaceRelease(p: Ptr);
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits: usize,
        row: usize,
        space: Ptr,
        info: u32,
    ) -> Ptr;
    fn CGBitmapContextCreateImage(ctx: Ptr) -> Ptr;
    fn CGContextRelease(ctx: Ptr);
    pub fn CGImageRelease(image: Ptr);
    pub fn CGImageGetWidth(image: Ptr) -> usize;
    pub fn CGImageGetHeight(image: Ptr) -> usize;
    fn CGContextScaleCTM(ctx: Ptr, x: f64, y: f64);
    fn CGContextTranslateCTM(ctx: Ptr, x: f64, y: f64);
    fn CGContextRotateCTM(ctx: Ptr, angle: f64);
    fn CGContextSaveGState(ctx: Ptr);
    fn CGContextRestoreGState(ctx: Ptr);
    fn CGContextClipToRect(ctx: Ptr, rect: Rect);
    fn CGContextSetRGBFillColor(ctx: Ptr, r: f64, g: f64, b: f64, a: f64);
    fn CGContextSetRGBStrokeColor(ctx: Ptr, r: f64, g: f64, b: f64, a: f64);
    fn CGContextSetLineWidth(ctx: Ptr, width: f64);
    fn CGContextSetLineJoin(ctx: Ptr, join: i32);
    fn CGContextSetLineCap(ctx: Ptr, cap: i32);
    fn CGContextMoveToPoint(ctx: Ptr, x: f64, y: f64);
    fn CGContextAddLineToPoint(ctx: Ptr, x: f64, y: f64);
    fn CGContextAddCurveToPoint(ctx: Ptr, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64);
    fn CGContextClosePath(ctx: Ptr);
    fn CGContextDrawPath(ctx: Ptr, mode: i32);
    fn CGColorCreate(space: Ptr, components: *const f64) -> Ptr;
    fn CGColorRelease(color: Ptr);
    fn CGContextSetShadowWithColor(ctx: Ptr, offset: Size, blur: f64, color: Ptr);
    fn CGContextDrawImage(ctx: Ptr, rect: Rect, image: Ptr);
    fn CGImageCreateWithImageInRect(image: Ptr, rect: Rect) -> Ptr;
    fn CGBitmapContextGetData(ctx: Ptr) -> *const c_void;
}

pub struct Image(pub Ptr);
impl Drop for Image {
    fn drop(&mut self) {
        unsafe { CGImageRelease(self.0) }
    }
}
impl Image {
    pub fn pixels(&self) -> Result<Vec<u8>> {
        unsafe {
            let (w, h) = (CGImageGetWidth(self.0), CGImageGetHeight(self.0));
            let space = CGColorSpaceCreateDeviceRGB();
            let ctx = CGBitmapContextCreate(std::ptr::null_mut(), w, h, 8, w * 4, space, 1);
            CGColorSpaceRelease(space);
            if ctx.is_null() {
                return Err("无法读取像素".into());
            }
            CGContextDrawImage(
                ctx,
                Rect::new(Point::new(0., 0.), Size::new(w as f64, h as f64)),
                self.0,
            );
            let mut out = Vec::with_capacity(16 + w * h * 4);
            out.extend_from_slice(&(w as u64).to_le_bytes());
            out.extend_from_slice(&(h as u64).to_le_bytes());
            out.extend_from_slice(std::slice::from_raw_parts(
                CGBitmapContextGetData(ctx).cast::<u8>(),
                w * h * 4,
            ));
            CGContextRelease(ctx);
            Ok(out)
        }
    }
    pub fn from_file(path: &Path) -> Result<Self> {
        let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if meta.len() > 8 * 1024 * 1024 {
            return Err("图片不能超过 8 MB".into());
        }
        Self::from_data(&native::data(
            &std::fs::read(path).map_err(|e| e.to_string())?,
        ))
    }
    pub fn from_data(data: &AnyObject) -> Result<Self> {
        if !native::kind(data, c"NSData") {
            return Err("主题中的图片必须是 NSData".into());
        }
        let rep: Option<Obj> =
            unsafe { msg_send![class!(NSBitmapImageRep), imageRepWithData: data] };
        let rep = rep.ok_or("图片解码失败")?;
        let p: *const CGImageOpaque = unsafe { msg_send![&rep, CGImage] };
        let p = p.cast::<c_void>();
        if p.is_null() {
            return Err("图片没有有效像素".into());
        }
        let (w, h) = unsafe { (CGImageGetWidth(p), CGImageGetHeight(p)) };
        if w == 0 || h == 0 || w > 512 || h > 32768 {
            return Err("图片宽度限 512 px；动画条带高度限 32768 px".into());
        }
        // Retain the native image independently of the autoreleased bitmap representation.
        unsafe extern "C" {
            fn CGImageRetain(p: Ptr) -> Ptr;
        }
        Ok(Image(unsafe { CGImageRetain(p) }))
    }
    pub fn png(&self) -> Result<Obj> {
        unsafe {
            let rep: objc2::rc::Allocated<AnyObject> = msg_send![class!(NSBitmapImageRep), alloc];
            let rep: Obj = msg_send![rep, initWithCGImage: self.0.cast::<CGImageOpaque>()];
            let out: Option<Obj> = msg_send![&rep, representationUsingType: 4usize, properties: &*native::dictionary()];
            out.ok_or_else(|| "PNG 编码失败".into())
        }
    }
    pub fn resized(&self, width: usize, height: usize) -> Result<Self> {
        bitmap(width, height, |ctx| unsafe {
            CGContextDrawImage(
                ctx,
                Rect::new(Point::new(0., 0.), Size::new(width as f64, height as f64)),
                self.0,
            );
        })
    }
}
fn bitmap(width: usize, height: usize, draw: impl FnOnce(Ptr)) -> Result<Image> {
    unsafe {
        let space = CGColorSpaceCreateDeviceRGB();
        let ctx =
            CGBitmapContextCreate(std::ptr::null_mut(), width, height, 8, width * 4, space, 1);
        CGColorSpaceRelease(space);
        if ctx.is_null() {
            return Err("创建像素缓冲失败".into());
        }
        draw(ctx);
        let image = CGBitmapContextCreateImage(ctx);
        CGContextRelease(ctx);
        if image.is_null() {
            Err("创建指针图片失败".into())
        } else {
            Ok(Image(image))
        }
    }
}
pub fn preset(style: &str, scale: usize) -> Result<Image> {
    bitmap(64 * scale, 64 * scale, |ctx| unsafe {
        CGContextTranslateCTM(ctx, 0., (64 * scale) as f64);
        CGContextScaleCTM(ctx, scale as f64, -(scale as f64));
        if style == "glow" {
            let space = CGColorSpaceCreateDeviceRGB();
            let color = CGColorCreate(space, [0.62, 0.40, 0.96, 0.75].as_ptr());
            CGContextSetShadowWithColor(ctx, Size::new(0., 0.), 7., color);
            CGColorRelease(color);
            CGColorSpaceRelease(space);
        }
        let white = style == "light";
        let fill = if white { 1. } else { 0.02 };
        let stroke = if white { 0.08 } else { 1. };
        CGContextSetRGBFillColor(ctx, fill, fill, fill, 1.);
        CGContextSetRGBStrokeColor(ctx, stroke, stroke, stroke, 1.);
        CGContextSetLineWidth(ctx, 2.5);
        CGContextSetLineJoin(ctx, 1);
        CGContextMoveToPoint(ctx, 13., 12.);
        CGContextAddCurveToPoint(ctx, 12., 9., 14., 8., 17., 9.);
        CGContextAddLineToPoint(ctx, 42., 18.);
        CGContextAddCurveToPoint(ctx, 46., 20., 46., 22., 42., 23.);
        CGContextAddLineToPoint(ctx, 31., 26.);
        CGContextAddLineToPoint(ctx, 26., 39.);
        CGContextAddCurveToPoint(ctx, 25., 43., 22., 42., 21., 38.);
        CGContextClosePath(ctx);
        CGContextDrawPath(ctx, 3);
    })
}
pub fn crosshair(style: &str, scale: usize, thin: bool) -> Result<Image> {
    bitmap(64 * scale, 64 * scale, |ctx| unsafe {
        CGContextScaleCTM(ctx, scale as f64, scale as f64);
        CGContextSetLineCap(ctx, 1);
        if style == "glow" {
            let space = CGColorSpaceCreateDeviceRGB();
            let color = CGColorCreate(space, [0.62, 0.40, 0.96, 0.75].as_ptr());
            CGContextSetShadowWithColor(ctx, Size::new(0., 0.), 7., color);
            CGColorRelease(color);
            CGColorSpaceRelease(space);
        }
        for outline in [true, false] {
            let white = outline != (style == "light");
            let tone = if white { 1. } else { 0.02 };
            CGContextSetRGBStrokeColor(ctx, tone, tone, tone, 1.);
            let width = if thin { 1.75 } else { 2.5 };
            CGContextSetLineWidth(ctx, if outline { width + 2.5 } else { width });
            CGContextMoveToPoint(ctx, 16., 32.);
            CGContextAddLineToPoint(ctx, 48., 32.);
            CGContextMoveToPoint(ctx, 32., 16.);
            CGContextAddLineToPoint(ctx, 32., 48.);
            CGContextDrawPath(ctx, 2);
            CGContextSetShadowWithColor(ctx, Size::new(0., 0.), 0., std::ptr::null());
        }
    })
}

include!("geometric_paths.rs");

fn geometric_path(ctx: Ptr, segments: &[Segment]) {
    unsafe {
        for segment in segments {
            match *segment {
                Segment::Move(x, y) => CGContextMoveToPoint(ctx, x, y),
                Segment::Line(x, y) => CGContextAddLineToPoint(ctx, x, y),
                Segment::Curve(a, b, c, d, x, y) => CGContextAddCurveToPoint(ctx, a, b, c, d, x, y),
                Segment::Close => CGContextClosePath(ctx),
            }
        }
    }
}

/// Flat paths; contrast under-strokes keep thin symbols legible on both backgrounds.
pub fn geometric_cursor(style: &str, scale: usize, kind: GeometricCursor) -> Result<Image> {
    bitmap(64 * scale, 64 * scale, |ctx| unsafe {
        CGContextTranslateCTM(ctx, 0., (64 * scale) as f64);
        CGContextScaleCTM(ctx, scale as f64, -(scale as f64));
        if matches!(kind, GeometricCursor::VerticalText) {
            CGContextTranslateCTM(ctx, 32., 32.);
            CGContextRotateCTM(ctx, std::f64::consts::FRAC_PI_2);
            CGContextTranslateCTM(ctx, -32., -32.);
        }
        CGContextSetLineJoin(ctx, 1);
        CGContextSetLineCap(ctx, 1);
        let light = style == "light";
        let outline = if light { 0.08 } else { 1. };
        let fill = if light { 1. } else { 0.02 };
        let ink = if style == "glow" { 1. } else { 0.02 };
        let contrast = if style == "glow" { 0.02 } else { 1. };
        let draw = |alpha: f64| {
            if kind.filled() {
                CGContextSetRGBFillColor(ctx, fill, fill, fill, alpha);
                CGContextSetRGBStrokeColor(ctx, outline, outline, outline, alpha);
                CGContextSetLineWidth(ctx, 2.);
                geometric_path(ctx, kind.path());
                CGContextDrawPath(ctx, 3);
            } else {
                for (tone, width) in [(contrast, 4.), (ink, 2.)] {
                    CGContextSetRGBStrokeColor(ctx, tone, tone, tone, alpha);
                    CGContextSetLineWidth(ctx, width);
                    geometric_path(ctx, kind.path());
                    CGContextDrawPath(ctx, 2);
                }
            }
        };
        let inactive = match kind {
            GeometricCursor::HorizontalLeft => Some(32.),
            GeometricCursor::HorizontalRight => Some(0.),
            _ => None,
        };
        if let Some(inactive) = inactive {
            for x in [0., 32.] {
                CGContextSaveGState(ctx);
                CGContextClipToRect(ctx, Rect::new(Point::new(x, 0.), Size::new(32., 64.)));
                draw(if x == inactive { 0.28 } else { 1. });
                CGContextRestoreGState(ctx);
            }
        } else {
            draw(1.);
        }
        if matches!(kind, GeometricCursor::Grip) {
            CGContextSetRGBStrokeColor(ctx, outline, outline, outline, 1.);
            CGContextSetLineWidth(ctx, 1.25);
            geometric_path(ctx, GRIP_DETAILS);
            CGContextDrawPath(ctx, 2);
        }
    })
}

/// Fit opaque ink, rather than transparent source padding, to the ordinary
/// arrow's 35.5-point silhouette on the shared 64-point canvas. All animation
/// frames share one transform. The hotspot follows that exact same transform.
pub fn cursor_layout(
    pixels: &[u8],
    logical: Size,
    hot: Point,
    frames: usize,
) -> Result<(Rect, Point)> {
    if pixels.len() < 16
        || frames == 0
        || ![logical.width, logical.height, hot.x, hot.y]
            .iter()
            .all(|v| v.is_finite())
        || logical.width <= 0.
        || logical.height <= 0.
    {
        return Err("指针布局缺少像素".into());
    }
    let width = u64::from_le_bytes(pixels[..8].try_into().unwrap()) as usize;
    let height = u64::from_le_bytes(pixels[8..16].try_into().unwrap()) as usize;
    if width == 0
        || height == 0
        || height % frames != 0
        || width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(4))
            .and_then(|n| n.checked_add(16))
            != Some(pixels.len())
    {
        return Err("指针布局像素尺寸无效".into());
    }
    let frame_height = height / frames;
    let mut bounds = (width, frame_height, 0, 0);
    for (index, pixel) in pixels[16..].chunks_exact(4).enumerate() {
        // Ignore native shadows so transparent padding and glow cannot decide size.
        if pixel[3] >= 128 {
            let x = index % width;
            let y = (index / width) % frame_height;
            bounds.0 = bounds.0.min(x);
            bounds.1 = bounds.1.min(y);
            bounds.2 = bounds.2.max(x + 1);
            bounds.3 = bounds.3.max(y + 1);
        }
    }
    if bounds.2 == 0 || bounds.3 == 0 {
        bounds = (0, 0, width, frame_height);
    }
    let center = Point::new(
        (bounds.0 + bounds.2) as f64 / 2.,
        (bounds.1 + bounds.3) as f64 / 2.,
    );
    let source_hot = Point::new(
        hot.x * width as f64 / logical.width,
        hot.y * frame_height as f64 / logical.height,
    );
    let mut factor = 35.5 / (bounds.2 - bounds.0).max(bounds.3 - bounds.1) as f64;
    // Preserve unusual native hotspots outside the ink without clamping them.
    for distance in [
        (source_hot.x - center.x).abs(),
        (source_hot.y - center.y).abs(),
    ] {
        if distance > 0. {
            factor = factor.min(31. / distance);
        }
    }
    let top = Point::new(32. - center.x * factor, 32. - center.y * factor);
    let size = Size::new(width as f64 * factor, frame_height as f64 * factor);
    let rect = Rect::new(Point::new(top.x, 64. - top.y - size.height), size);
    let transformed_hot = Point::new(top.x + source_hot.x * factor, top.y + source_hot.y * factor);
    Ok((rect, transformed_hot))
}

/// Keep each native silhouette and its internal detail, with a common palette
/// and canvas. Sprite frames are cropped separately so shadows cannot bleed
/// between frames. CoreGraphics, rather than a polling timer, animates them.
pub fn themed_sprite(
    original: &Image,
    layout: Rect,
    native_frames: usize,
    frames: usize,
    style: &str,
    scale: usize,
) -> Result<Image> {
    let mut pixels = original.pixels()?;
    let width = unsafe { CGImageGetWidth(original.0) };
    let height = unsafe { CGImageGetHeight(original.0) };
    let mut bright = 0usize;
    let mut dark = 0usize;
    for p in pixels[16..].chunks_exact(4).filter(|p| p[3] > 240) {
        if p[0] as u32 + p[1] as u32 + p[2] as u32 > p[3] as u32 * 3 / 2 {
            bright += 1;
        } else {
            dark += 1;
        }
    }
    let invert = (bright > dark) != (style == "light");
    for p in pixels[16..].chunks_exact_mut(4) {
        let gray = ((p[0] as u32 * 54 + p[1] as u32 * 183 + p[2] as u32 * 19) / 256) as u8;
        // Components are premultiplied by alpha; subtract from alpha, not 255.
        let tone = if invert {
            p[3].saturating_sub(gray)
        } else {
            gray
        };
        p[0] = tone;
        p[1] = tone;
        p[2] = tone;
    }
    let mono = bitmap(width, height, |ctx| unsafe {
        std::ptr::copy_nonoverlapping(
            pixels[16..].as_ptr(),
            CGBitmapContextGetData(ctx).cast_mut().cast(),
            width * height * 4,
        );
    })?;
    let side = 64 * scale;
    bitmap(side, side * frames, |ctx| unsafe {
        CGContextScaleCTM(ctx, scale as f64, scale as f64);
        if style == "glow" {
            let space = CGColorSpaceCreateDeviceRGB();
            let color = CGColorCreate(space, [0.62, 0.40, 0.96, 0.75].as_ptr());
            CGContextSetShadowWithColor(ctx, Size::new(0., 0.), 7., color);
            CGColorRelease(color);
            CGColorSpaceRelease(space);
        }
        let frame_height = height / native_frames;
        for i in 0..frames {
            let index = i * native_frames / frames;
            let crop = CGImageCreateWithImageInRect(
                mono.0,
                Rect::new(
                    Point::new(0., (index * frame_height) as f64),
                    Size::new(width as f64, frame_height as f64),
                ),
            );
            if !crop.is_null() {
                let image = Image(crop);
                CGContextDrawImage(
                    ctx,
                    Rect::new(
                        Point::new(
                            layout.origin.x,
                            (frames - i - 1) as f64 * 64. + layout.origin.y,
                        ),
                        layout.size,
                    ),
                    image.0,
                );
            }
        }
    })
}

pub fn first_frame(image: &Image, frames: usize) -> Result<Image> {
    let (w, h) = unsafe { (CGImageGetWidth(image.0), CGImageGetHeight(image.0)) };
    let crop = unsafe {
        CGImageCreateWithImageInRect(
            image.0,
            Rect::new(Point::new(0., 0.), Size::new(w as f64, (h / frames) as f64)),
        )
    };
    if crop.is_null() {
        Err("无法提取预览帧".into())
    } else {
        Ok(Image(crop))
    }
}
pub fn resample_sprite(original: &Image, native_frames: usize, frames: usize) -> Result<Image> {
    let (w, h) = unsafe { (CGImageGetWidth(original.0), CGImageGetHeight(original.0)) };
    let fh = h / native_frames;
    bitmap(w, fh * frames, |ctx| unsafe {
        for i in 0..frames {
            let index = i * native_frames / frames;
            let crop = CGImageCreateWithImageInRect(
                original.0,
                Rect::new(
                    Point::new(0., (index * fh) as f64),
                    Size::new(w as f64, fh as f64),
                ),
            );
            if !crop.is_null() {
                let image = Image(crop);
                CGContextDrawImage(
                    ctx,
                    Rect::new(
                        Point::new(0., ((frames - i - 1) * fh) as f64),
                        Size::new(w as f64, fh as f64),
                    ),
                    image.0,
                );
            }
        }
    })
}

/// Bounds are in logical points, never pixels. Reject NaN and invalid hotspots.
pub fn validate(size: Size, hotspot: Point, frames: usize, duration: f64) -> Result<()> {
    validate_with_limit(size, hotspot, frames, duration, 24)
}
pub fn validate_with_limit(
    size: Size,
    hotspot: Point,
    frames: usize,
    duration: f64,
    max_frames: usize,
) -> Result<()> {
    if ![size.width, size.height, hotspot.x, hotspot.y, duration]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err("指针参数必须是有限数字".into());
    }
    if size.width <= 0. || size.height <= 0. || size.width > 64. || size.height > 64. {
        return Err("指针逻辑尺寸必须在 0–64 点之间".into());
    }
    if hotspot.x < 0. || hotspot.y < 0. || hotspot.x >= size.width || hotspot.y >= size.height {
        return Err("点击热点必须位于指针图片内".into());
    }
    if !(1..=max_frames).contains(&frames) || (frames > 1 && !(0.01..=10.).contains(&duration)) {
        return Err(format!("动画限 1–{max_frames} 帧，帧间隔 0.01–10 秒"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(
        width: usize,
        frame_height: usize,
        frames: usize,
        shapes: &[(usize, usize, usize, usize, usize)],
    ) -> Vec<u8> {
        let mut pixels = vec![0; 16 + width * frame_height * frames * 4];
        pixels[..8].copy_from_slice(&(width as u64).to_le_bytes());
        pixels[8..16].copy_from_slice(&((frame_height * frames) as u64).to_le_bytes());
        for &(frame, x0, y0, x1, y1) in shapes {
            for y in y0..y1 {
                for x in x0..x1 {
                    let offset = 16 + ((frame * frame_height + y) * width + x) * 4;
                    pixels[offset..offset + 4].fill(255);
                }
            }
        }
        pixels
    }
    #[test]
    fn visible_size_and_hotspot_are_independent_of_transparent_padding() {
        let a = fixture(80, 80, 1, &[(0, 20, 12, 40, 52)]);
        let b = fixture(120, 120, 1, &[(0, 50, 42, 70, 82)]);
        let (ar, ah) = cursor_layout(&a, Size::new(80., 80.), Point::new(22., 14.), 1).unwrap();
        let (br, bh) = cursor_layout(&b, Size::new(120., 120.), Point::new(52., 44.), 1).unwrap();
        assert!((ah.x - bh.x).abs() < 1e-9 && (ah.y - bh.y).abs() < 1e-9);
        let af = ar.size.width / 80.;
        let bf = br.size.width / 120.;
        assert!((ar.origin.x + 20. * af - br.origin.x - 50. * bf).abs() < 1e-9);
        assert!((40. * af - 35.5).abs() < 1e-9);
        assert!((ah.x - (ar.origin.x + 22. * af)).abs() < 1e-9);
        assert!((ah.y - (64. - ar.origin.y - ar.size.height + 14. * af)).abs() < 1e-9);
    }
    #[test]
    fn animation_uses_union_of_frames_and_keeps_distant_hotspots_inside_canvas() {
        let pixels = fixture(64, 64, 2, &[(0, 10, 20, 20, 40), (1, 30, 10, 45, 40)]);
        let (rect, hot) =
            cursor_layout(&pixels, Size::new(64., 64.), Point::new(12., 20.), 2).unwrap();
        assert!((rect.size.width / 64. * 35. - 35.5).abs() < 1e-9);
        validate(Size::new(64., 64.), hot, 2, 0.1).unwrap();
        let padded = fixture(100, 100, 1, &[(0, 10, 10, 20, 20)]);
        let (rect, hot) =
            cursor_layout(&padded, Size::new(100., 100.), Point::new(99., 99.), 1).unwrap();
        assert!((hot.x - 63.).abs() < 1e-9 && (hot.y - 63.).abs() < 1e-9);
        assert!((hot.x - (rect.origin.x + 99. * rect.size.width / 100.)).abs() < 1e-9);
        validate(Size::new(64., 64.), hot, 1, 0.).unwrap();
    }
    #[test]
    fn rejects_invalid_hotspots_and_nonfinite_values() {
        let size = Size::new(32., 32.);
        assert!(validate(size, Point::new(32., 0.), 1, 0.).is_err());
        assert!(validate(size, Point::new(-1., 0.), 1, 0.).is_err());
        assert!(validate(size, Point::new(f64::NAN, 0.), 1, 0.).is_err());
        assert!(validate(size, Point::new(0., 0.), 25, 0.1).is_err());
        assert!(validate(size, Point::new(12., 9.), 1, 0.).is_ok());
    }
}

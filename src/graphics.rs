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
    fn CGContextSetRGBFillColor(ctx: Ptr, r: f64, g: f64, b: f64, a: f64);
    fn CGContextSetRGBStrokeColor(ctx: Ptr, r: f64, g: f64, b: f64, a: f64);
    fn CGContextSetLineWidth(ctx: Ptr, width: f64);
    fn CGContextSetLineJoin(ctx: Ptr, join: i32);
    fn CGContextMoveToPoint(ctx: Ptr, x: f64, y: f64);
    fn CGContextAddLineToPoint(ctx: Ptr, x: f64, y: f64);
    fn CGContextAddCurveToPoint(ctx: Ptr, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64);
    fn CGContextClosePath(ctx: Ptr);
    fn CGContextDrawPath(ctx: Ptr, mode: i32);
    fn CGColorCreate(space: Ptr, components: *const f64) -> Ptr;
    fn CGColorRelease(color: Ptr);
    fn CGContextSetShadowWithColor(ctx: Ptr, offset: Size, blur: f64, color: Ptr);
    fn CGContextDrawImage(ctx: Ptr, rect: Rect, image: Ptr);
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
        if w == 0 || h == 0 || w > 512 || h > 12288 {
            return Err("图片宽度限 512 px；动画条带高度限 12288 px".into());
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

/// Bounds are in logical points, never pixels. Reject NaN and invalid hotspots.
pub fn validate(size: Size, hotspot: Point, frames: usize, duration: f64) -> Result<()> {
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
    if !(1..=24).contains(&frames) || (frames > 1 && !(0.01..=10.).contains(&duration)) {
        return Err("动画限 1–24 帧，帧间隔 0.01–10 秒".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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

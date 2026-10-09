use crate::native::{self, Obj, Result};
use objc2::runtime::AnyObject;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPMAssertionCreateWithName(
        kind: *const AnyObject,
        level: u32,
        name: *const AnyObject,
        id: *mut u32,
    ) -> i32;
    fn IOPMAssertionRelease(id: u32) -> i32;
    fn IOPMAssertionCopyProperties(id: u32) -> *mut AnyObject;
}

/// A process-owned display assertion; no timer, subprocess or global settings.
#[derive(Default)]
pub struct KeepAwake {
    id: Option<u32>,
}
impl KeepAwake {
    pub fn enabled(&self) -> bool {
        self.id.is_some()
    }
    pub fn enable(&mut self) -> Result<()> {
        if self.enabled() {
            return Ok(());
        }
        let kind = native::string("PreventUserIdleDisplaySleep");
        let name = native::string("Ashmactool: keep display awake");
        let mut id = 0;
        let code = unsafe {
            IOPMAssertionCreateWithName(
                objc2::rc::Retained::as_ptr(&kind).cast(),
                255,
                objc2::rc::Retained::as_ptr(&name).cast(),
                &mut id,
            )
        };
        if code != 0 {
            return Err(format!("无法保持屏幕唤醒（IOKit 0x{:08x}）", code as u32));
        }
        self.id = Some(id);
        if let Err(e) = self.verify() {
            self.disable()?;
            return Err(e);
        }
        Ok(())
    }
    pub fn disable(&mut self) -> Result<()> {
        if let Some(id) = self.id {
            let code = unsafe { IOPMAssertionRelease(id) };
            if code != 0 {
                return Err(format!("无法关闭屏幕唤醒（IOKit 0x{:08x}）", code as u32));
            }
            self.id = None;
        }
        Ok(())
    }
    pub fn verify(&self) -> Result<()> {
        let id = self.id.ok_or("屏幕唤醒尚未启用")?;
        // The Copy function transfers one retained CFDictionary, toll-free
        // bridged to NSDictionary. Obj releases exactly that ownership.
        let props = unsafe { Obj::from_raw(IOPMAssertionCopyProperties(id)) }
            .ok_or("无法回读屏幕唤醒状态")?;
        let kind = native::get(&props, "AssertType").ok_or("缺少电源请求类型")?;
        if native::text(&kind) != "PreventUserIdleDisplaySleep"
            || native::numeric(&props, "AssertLevel")? != 255.
        {
            return Err("系统未启用屏幕唤醒请求".into());
        }
        Ok(())
    }
    pub fn self_test() -> Result<()> {
        let mut awake = Self::default();
        awake.enable()?;
        awake.verify()?;
        let id = awake.id.unwrap();
        // Disabling twice must be harmless; the released assertion must vanish.
        awake.disable()?;
        awake.disable()?;
        let leftover = unsafe { Obj::from_raw(IOPMAssertionCopyProperties(id)) };
        if leftover.is_some() || awake.enabled() {
            return Err("关闭后电源请求仍然存在".into());
        }
        println!("display_assertion_readback=passed release_readback=passed");
        Ok(())
    }
}
impl Drop for KeepAwake {
    fn drop(&mut self) {
        if let Err(e) = self.disable() {
            eprintln!("释放电源请求失败：{e}");
        }
    }
}

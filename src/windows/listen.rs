use crate::{
    rdev::{Event, ListenError},
    windows::common::{
        convert, get_scan_code, is_kb_injected, is_mouse_injected, set_key_hook, set_mouse_hook,
        HookError,
    },
};
use std::{os::raw::c_int, ptr::null_mut, time::SystemTime};
use winapi::{
    shared::{
        basetsd::ULONG_PTR,
        minwindef::{LPARAM, LRESULT, WPARAM},
    },
    um::winuser::{CallNextHookEx, GetMessageA, HC_ACTION, PKBDLLHOOKSTRUCT, PMOUSEHOOKSTRUCT},
};

static mut GLOBAL_CALLBACK: Option<Box<dyn FnMut(Event)>> = None;

impl From<HookError> for ListenError {
    fn from(error: HookError) -> Self {
        match error {
            HookError::Mouse(code) => ListenError::MouseHookError(code),
            HookError::Key(code) => ListenError::KeyHookError(code),
        }
    }
}

unsafe fn raw_callback(
    code: c_int,
    param: WPARAM,
    lpdata: LPARAM,
    f_get_extra_data: impl FnOnce(isize) -> ULONG_PTR,
    f_is_injected: impl FnOnce(isize) -> bool,
) -> LRESULT {
    if code == HC_ACTION {
        let (opt, code) = convert(param, lpdata);
        if let Some(event_type) = opt {
            let event = Event {
                event_type,
                time: SystemTime::now(),
                unicode: None,
                platform_code: code as _,
                position_code: get_scan_code(lpdata),
                usb_hid: 0,
                is_injected: f_is_injected(lpdata),
                extra_data: f_get_extra_data(lpdata),
            };
            if let Some(callback) = &mut GLOBAL_CALLBACK {
                callback(event);
            }
        }
    }
    CallNextHookEx(null_mut(), code, param, lpdata)
}

unsafe extern "system" fn raw_callback_mouse(code: i32, param: usize, lpdata: isize) -> isize {
    raw_callback(
        code,
        param,
        lpdata,
        |data: isize| unsafe { (*(data as PMOUSEHOOKSTRUCT)).dwExtraInfo },
        |data: isize| unsafe { is_mouse_injected(data) },
    )
}

unsafe extern "system" fn raw_callback_keyboard(code: i32, param: usize, lpdata: isize) -> isize {
    raw_callback(
        code,
        param,
        lpdata,
        |data: isize| unsafe { (*(data as PKBDLLHOOKSTRUCT)).dwExtraInfo },
        |data: isize| unsafe { is_kb_injected(data) },
    )
}

pub fn listen<T>(callback: T) -> Result<(), ListenError>
where
    T: FnMut(Event) + 'static,
{
    unsafe {
        GLOBAL_CALLBACK = Some(Box::new(callback));
        set_key_hook(raw_callback_keyboard)?;
        if !crate::keyboard_only() {
            set_mouse_hook(raw_callback_mouse)?;
        }

        GetMessageA(null_mut(), null_mut(), 0, 0);
    }
    Ok(())
}

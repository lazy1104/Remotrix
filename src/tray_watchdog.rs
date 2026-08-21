#![cfg(target_os = "windows")]
#![allow(non_snake_case)]
#![allow(clippy::upper_case_acronyms)]

use std::os::raw::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::JoinHandle;

use libloading::{Library, Symbol};

use crate::message::{Message, TrayMsg};

type HWND = *mut c_void;
type HINSTANCE = *mut c_void;
type HCURSOR = *mut c_void;
type HBRUSH = *mut c_void;
type WPARAM = usize;
type LPARAM = isize;
type LRESULT = isize;

type WndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

#[repr(C)]
struct WndClassExW {
    cb_size: u32,
    style: u32,
    lpfn_wnd_proc: Option<WndProc>,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: HINSTANCE,
    h_icon: *mut c_void,
    h_cursor: HCURSOR,
    hbr_background: HBRUSH,
    lpsz_menu_name: *const u16,
    lpsz_class_name: *const u16,
    h_icon_sm: *mut c_void,
}

#[repr(C)]
struct Msg {
    hwnd: HWND,
    message: u32,
    w_param: WPARAM,
    l_param: LPARAM,
    time: u32,
    pt_x: i32,
    pt_y: i32,
}

const WM_QUIT: u32 = 0x0012;
const WM_DESTROY: u32 = 0x0002;
const GWLP_USERDATA: i32 = -21;
const HWND_MESSAGE: isize = -3;
const IDC_ARROW: *const u16 = 32512usize as *const u16;

const TASKBAR_CREATED_NAME: &[u16] = &[
    b'T' as u16,
    b'a' as u16,
    b's' as u16,
    b'k' as u16,
    b'b' as u16,
    b'a' as u16,
    b'r' as u16,
    b'C' as u16,
    b'r' as u16,
    b'e' as u16,
    b'a' as u16,
    b't' as u16,
    b'e' as u16,
    b'd' as u16,
    0,
];

static CLASS_NAME: [u16; 64] = wide_const("RemotrixTrayWatchdog");

const fn wide_const(s: &str) -> [u16; 64] {
    let bytes = s.as_bytes();
    let mut out = [0u16; 64];
    let mut i = 0;
    while i < bytes.len() && i < 63 {
        out[i] = bytes[i] as u16;
        i += 1;
    }
    out[i] = 0;
    out
}

fn wide_cstr(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}

struct Win {
    _user32: Library,
    register_class_ex_w: unsafe extern "system" fn(*const WndClassExW) -> u16,
    unregister_class_w: unsafe extern "system" fn(*const u16, HINSTANCE) -> i32,
    create_window_ex_w: unsafe extern "system" fn(
        u32,
        *const u16,
        *const u16,
        u32,
        i32,
        i32,
        i32,
        i32,
        HWND,
        *mut c_void,
        HINSTANCE,
        *mut c_void,
    ) -> HWND,
    destroy_window: unsafe extern "system" fn(HWND) -> i32,
    def_window_proc_w: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
    load_cursor_w: unsafe extern "system" fn(HINSTANCE, *const u16) -> HCURSOR,
    get_module_handle_w: unsafe extern "system" fn(*const u16) -> HINSTANCE,
    set_window_long_ptr_w: unsafe extern "system" fn(HWND, i32, isize) -> isize,
    get_message_w: unsafe extern "system" fn(*mut Msg, HWND, u32, u32, u32) -> i32,
    dispatch_message_w: unsafe extern "system" fn(*const Msg) -> LRESULT,
    translate_message: unsafe extern "system" fn(*const Msg) -> i32,
    post_message_w: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> i32,
    register_window_message_w: unsafe extern "system" fn(*const u16) -> u32,
}

unsafe fn load_fn<T: Copy>(lib: &Library, name: &[u8]) -> Result<T, libloading::Error> {
    let sym: Symbol<T> = lib.get(name)?;
    Ok(*sym)
}

impl Win {
    unsafe fn load() -> Result<Self, libloading::Error> {
        let user32 = Library::new("user32.dll")?;
        Ok(Win {
            register_class_ex_w: load_fn(&user32, b"RegisterClassExW\0")?,
            unregister_class_w: load_fn(&user32, b"UnregisterClassW\0")?,
            create_window_ex_w: load_fn(&user32, b"CreateWindowExW\0")?,
            destroy_window: load_fn(&user32, b"DestroyWindow\0")?,
            def_window_proc_w: load_fn(&user32, b"DefWindowProcW\0")?,
            load_cursor_w: load_fn(&user32, b"LoadCursorW\0")?,
            get_module_handle_w: load_fn(&user32, b"GetModuleHandleW\0")?,
            set_window_long_ptr_w: load_fn(&user32, b"SetWindowLongPtrW\0")?,
            get_message_w: load_fn(&user32, b"GetMessageW\0")?,
            dispatch_message_w: load_fn(&user32, b"DispatchMessageW\0")?,
            translate_message: load_fn(&user32, b"TranslateMessage\0")?,
            post_message_w: load_fn(&user32, b"PostMessageW\0")?,
            register_window_message_w: load_fn(&user32, b"RegisterWindowMessageW\0")?,
            _user32: user32,
        })
    }
}

static WIN_SLOT: AtomicUsize = AtomicUsize::new(0);
static TASKBAR_CREATED_MSG: AtomicUsize = AtomicUsize::new(0);

pub struct TrayWatchdog {
    hwnd: HWND,
    thread: Option<JoinHandle<()>>,
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let taskbar_msg = TASKBAR_CREATED_MSG.load(Ordering::SeqCst) as u32;
    if taskbar_msg != 0 && msg == taskbar_msg {
        let tx_ptr = WIN_SLOT.load(Ordering::SeqCst);
        if tx_ptr != 0 {
            let tx = &*(tx_ptr as *const tokio::sync::mpsc::UnboundedSender<Message>);
            tracing::info!("tray watchdog: TaskbarCreated received, requesting re-add");
            let _ = tx.send(Message::Tray(TrayMsg::WatchdogReaddRequested));
        }
        return 0;
    }
    if msg == WM_DESTROY {
        return 0;
    }
    match unsafe { Win::load() } {
        Ok(win) => unsafe { (win.def_window_proc_w)(hwnd, msg, wparam, lparam) },
        Err(_) => 0,
    }
}

impl TrayWatchdog {
    pub fn start(tx: tokio::sync::mpsc::UnboundedSender<Message>) -> Option<Self> {
        let win = match unsafe { Win::load() } {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!(error = %e, "tray watchdog: user32 load failed");
                return None;
            }
        };

        let class_name_ptr = CLASS_NAME.as_ptr();
        let hinstance = unsafe { (win.get_module_handle_w)(std::ptr::null()) };
        let cursor = unsafe { (win.load_cursor_w)(std::ptr::null_mut(), IDC_ARROW) };
        let wc = WndClassExW {
            cb_size: std::mem::size_of::<WndClassExW>() as u32,
            style: 0,
            lpfn_wnd_proc: Some(wndproc),
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance: hinstance,
            h_icon: std::ptr::null_mut(),
            h_cursor: cursor,
            hbr_background: std::ptr::null_mut(),
            lpsz_menu_name: std::ptr::null(),
            lpsz_class_name: class_name_ptr,
            h_icon_sm: std::ptr::null_mut(),
        };
        let atom = unsafe { (win.register_class_ex_w)(&wc) };
        if atom == 0 {
            tracing::warn!("tray watchdog: RegisterClassExW failed");
            return None;
        }

        let tx_box = Box::new(tx);
        let tx_ptr = Box::into_raw(tx_box) as isize;
        WIN_SLOT.store(tx_ptr as usize, Ordering::SeqCst);

        let title = wide_cstr("Remotrix Tray Watchdog");
        let hwnd = unsafe {
            (win.create_window_ex_w)(
                0,
                class_name_ptr,
                title.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE as HWND,
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null_mut(),
            )
        };
        if hwnd.is_null() {
            tracing::warn!("tray watchdog: CreateWindowExW failed");
            unsafe {
                let _ = (win.unregister_class_w)(class_name_ptr, hinstance);
            }
            drop(unsafe {
                Box::from_raw(tx_ptr as *mut tokio::sync::mpsc::UnboundedSender<Message>)
            });
            WIN_SLOT.store(0, Ordering::SeqCst);
            return None;
        }
        unsafe {
            (win.set_window_long_ptr_w)(hwnd, GWLP_USERDATA, tx_ptr);
        }

        let class_name_static: &'static [u16; 64] = &CLASS_NAME;
        let hwnd_isize = hwnd as isize;
        let hinstance_isize = hinstance as isize;
        let thread = std::thread::Builder::new()
            .name("remotrix-tray-watchdog".into())
            .spawn(move || {
                run_message_loop(
                    hwnd_isize as HWND,
                    class_name_static,
                    hinstance_isize as HINSTANCE,
                    tx_ptr,
                );
            })
            .ok()?;

        Some(TrayWatchdog {
            hwnd,
            thread: Some(thread),
        })
    }

    pub fn stop(&mut self) {
        if let Ok(win) = unsafe { Win::load() } {
            unsafe {
                let _ = (win.post_message_w)(self.hwnd, WM_QUIT, 0, 0);
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_message_loop(
    hwnd: HWND,
    class_name: &'static [u16; 64],
    hinstance: HINSTANCE,
    tx_ptr: isize,
) {
    let win = match unsafe { Win::load() } {
        Ok(w) => w,
        Err(_) => {
            let _ = unsafe {
                Box::from_raw(tx_ptr as *mut tokio::sync::mpsc::UnboundedSender<Message>)
            };
            return;
        }
    };

    let taskbar = unsafe { (win.register_window_message_w)(TASKBAR_CREATED_NAME.as_ptr()) };
    TASKBAR_CREATED_MSG.store(taskbar as usize, Ordering::SeqCst);

    let mut msg = Msg {
        hwnd: std::ptr::null_mut(),
        message: 0,
        w_param: 0,
        l_param: 0,
        time: 0,
        pt_x: 0,
        pt_y: 0,
    };
    loop {
        let r = unsafe { (win.get_message_w)(&mut msg, std::ptr::null_mut(), 0, 0, 0) };
        if r <= 0 {
            break;
        }
        unsafe {
            (win.translate_message)(&msg);
            (win.dispatch_message_w)(&msg);
        }
    }

    unsafe {
        let _ = (win.destroy_window)(hwnd);
        let _ = (win.unregister_class_w)(class_name.as_ptr(), hinstance);
    }
    let _ = unsafe { Box::from_raw(tx_ptr as *mut tokio::sync::mpsc::UnboundedSender<Message>) };
    WIN_SLOT.store(0, Ordering::SeqCst);
    TASKBAR_CREATED_MSG.store(0, Ordering::SeqCst);
}

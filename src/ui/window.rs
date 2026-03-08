use std::sync::Arc;
use parking_lot::RwLock;
use crossbeam_channel::Receiver;
use windows::{
    core::w,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::{UpdateWindow, InvalidateRect},
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::WindowsAndMessaging::*,
    Win32::UI::Input::KeyboardAndMouse::*,
};
use crate::index::search::{search, SearchResult};
use crate::index::store::IndexStore;
use crate::mft::types::IndexEvent;
use crate::ui::{hotkey, renderer::Renderer};

const WM_INDEX_READY: u32 = WM_USER + 1;
const MAX_RESULTS: usize  = 500;
const VISIBLE_ROWS: usize = 18;
const WINDOW_W: i32       = 760;
const WINDOW_H: i32       = 600;
const SEARCH_H: i32       = 52;

struct AppState {
    index: Arc<RwLock<IndexStore>>,
    renderer: Renderer,
    results: Vec<SearchResult>,
    scroll: usize,
    query: String,
    edit_hwnd: HWND,
}

pub fn run(index: Arc<RwLock<IndexStore>>, event_rx: Receiver<IndexEvent>) {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap();

        let class_name = w!("fastsearchWnd");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpszClassName: class_name,
            lpfnWndProc: Some(wnd_proc),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hbrBackground: std::mem::zeroed(),
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        let x = (sw - WINDOW_W) / 2;
        let y = (sh - WINDOW_H) / 3;

        let state = Box::new(AppState {
            index: Arc::clone(&index),
            renderer: Renderer::new(),
            results: Vec::new(),
            scroll: 0,
            query: String::new(),
            edit_hwnd: HWND(std::ptr::null_mut()),
        });
        let state_ptr = Box::into_raw(state);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            w!("fastsearch"),
            WS_POPUP | WS_BORDER,
            x, y, WINDOW_W, WINDOW_H,
            HWND(std::ptr::null_mut()),
            None,
            hinstance,
            Some(state_ptr as *const _),
        ).unwrap();

        let edit = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("EDIT"),
            w!(""),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            0, 0, WINDOW_W, SEARCH_H,
            hwnd,
            None,
            hinstance,
            None,
        ).unwrap();

        (*state_ptr).edit_hwnd = edit;

        hotkey::register(hwnd);
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

       // Cast HWND to usize — usize is Send, reconstruct on the other side
        let hwnd_raw = hwnd.0 as usize;
        let index_for_thread: Arc<RwLock<IndexStore>> = Arc::clone(&index);

        std::thread::spawn(move || {
            let hwnd = HWND(hwnd_raw as *mut _);
            for event in event_rx {
                let mut store = index_for_thread.write();
                match event {
                    IndexEvent::Created(r) => store.insert(r),
                    IndexEvent::Deleted(id) => store.remove(id),
                    IndexEvent::Renamed { old_ref, new_record } => {
                        store.rename(old_ref, new_record)
                    }
                }
                drop(store);
                unsafe { PostMessageW(hwnd, WM_INDEX_READY, WPARAM(0), LPARAM(0)).ok() };
            }
        });

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
            LRESULT(0)
        }

        WM_HOTKEY => {
            if IsWindowVisible(hwnd).as_bool() {
                ShowWindow(hwnd, SW_HIDE);
            } else {
                ShowWindow(hwnd, SW_SHOW);
                SetForegroundWindow(hwnd);
            }
            LRESULT(0)
        }

        WM_COMMAND => {
            let notif = (wparam.0 >> 16) as u16;
            if notif == 0x300 {
                let s = get_state(hwnd);
                if !s.is_null() {
                    let s = &mut *s;
                    let mut buf = vec![0u16; 512];
                    let len = GetWindowTextW(s.edit_hwnd, &mut buf) as usize;
                    s.query = String::from_utf16_lossy(&buf[..len]);
                    run_search(hwnd, s);
                }
            }
            LRESULT(0)
        }

        WM_KEYDOWN => {
            let s = get_state(hwnd);
            if s.is_null() { return DefWindowProcW(hwnd, msg, wparam, lparam); }
            let s = &mut *s;

            match VIRTUAL_KEY(wparam.0 as u16) {
                VK_DOWN => {
                    if s.renderer.selected_index + 1 < s.results.len() {
                        s.renderer.selected_index += 1;
                        if s.renderer.selected_index >= s.scroll + VISIBLE_ROWS {
                            s.scroll += 1;
                        }
                        InvalidateRect(hwnd, None, true);
                    }
                }
                VK_UP => {
                    if s.renderer.selected_index > 0 {
                        s.renderer.selected_index -= 1;
                        if s.renderer.selected_index < s.scroll {
                            s.scroll = s.renderer.selected_index;
                        }
                        InvalidateRect(hwnd, None, true);
                    }
                }
                VK_RETURN => {
                    if let Some(r) = s.results.get(s.renderer.selected_index) {
                        let _ = std::process::Command::new("explorer")
                            .arg(&r.full_path)
                            .spawn();
                    }
                }
                VK_ESCAPE => { ShowWindow(hwnd, SW_HIDE); }
                _ => {}
            }
            LRESULT(0)
        }

        WM_PAINT => {
            let s = get_state(hwnd);
            if !s.is_null() {
                let s = &*s;
                s.renderer.paint(hwnd, &s.results, s.scroll, VISIBLE_ROWS);
            }
            LRESULT(0)
        }

        WM_INDEX_READY => {
            let s = get_state(hwnd);
            if !s.is_null() {
                run_search(hwnd, &mut *s);
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            let s = get_state(hwnd);
            if !s.is_null() {
                drop(Box::from_raw(s));
            }
            PostQuitMessage(0);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn get_state(hwnd: HWND) -> *mut AppState {
    GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState
}

fn run_search(hwnd: HWND, state: &mut AppState) {
    let store = state.index.read();
    state.results = search(&store.entries, &state.query, MAX_RESULTS);
    state.renderer.selected_index = 0;
    state.scroll = 0;
    unsafe { InvalidateRect(hwnd, None, true); }
}
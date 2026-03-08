use windows::{
    Win32::Foundation::{HWND, RECT, COLORREF},
    Win32::Graphics::Gdi::{
        BeginPaint, DrawTextW, EndPaint, FillRect, SelectObject,
        SetBkColor, SetTextColor, PAINTSTRUCT,
        DT_LEFT, DT_SINGLELINE, DT_VCENTER,
        CreateSolidBrush, CreateFontW, DeleteObject,
    },
};
use crate::index::search::SearchResult;

const BG_COLOR: u32   = 0x0F0F11;
const ROW_COLOR: u32  = 0x131316;
const ROW_ALT: u32    = 0x1A1A1F;
const SEL_COLOR: u32  = 0x1E3A5F;
const TEXT_COLOR: u32 = 0xE8E8F0;
const DIM_COLOR: u32  = 0x8888A0;
const DIR_COLOR: u32  = 0x4F8EF7;
const ROW_HEIGHT: i32 = 32;

pub struct Renderer {
    pub selected_index: usize,
}

impl Renderer {
    pub fn new() -> Self {
        Self { selected_index: 0 }
    }

    pub fn paint(
        &self,
        hwnd: HWND,
        results: &[SearchResult],
        scroll_offset: usize,
        visible_rows: usize,
    ) {
        unsafe {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            let bg_brush = CreateSolidBrush(COLORREF(BG_COLOR));
            FillRect(hdc, &ps.rcPaint, bg_brush);
            DeleteObject(bg_brush);

            let font = CreateFontW(
                16, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 0, 0,
                windows::core::w!("Segoe UI"),
            );
            let old_font = SelectObject(hdc, font);
            SetBkColor(hdc, COLORREF(ROW_COLOR));

            for (i, result) in results
                .iter()
                .skip(scroll_offset)
                .take(visible_rows)
                .enumerate()
            {
                let y = i as i32 * ROW_HEIGHT;
                let global_i = scroll_offset + i;

                let row_bg = if global_i == self.selected_index {
                    SEL_COLOR
                } else if i % 2 == 0 {
                    ROW_COLOR
                } else {
                    ROW_ALT
                };

                let row_brush = CreateSolidBrush(COLORREF(row_bg));
                let row_rect = RECT { left: 0, top: y, right: 2000, bottom: y + ROW_HEIGHT };
                FillRect(hdc, &row_rect, row_brush);
                DeleteObject(row_brush);

                let icon = if result.is_dir { "D  " } else { "F  " };
                let color = if result.is_dir { DIR_COLOR } else { TEXT_COLOR };
                SetTextColor(hdc, COLORREF(color));

                let display = format!("{}{}", icon, result.full_path.to_string_lossy());
                let mut text_w: Vec<u16> = display.encode_utf16().collect();
                let mut text_rect = RECT { left: 12, top: y, right: 2000, bottom: y + ROW_HEIGHT };
                DrawTextW(hdc, &mut text_w, &mut text_rect, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
            }

            if results.is_empty() {
                SetTextColor(hdc, COLORREF(DIM_COLOR));
                let mut msg: Vec<u16> = "Type to search...".encode_utf16().collect();
                let mut r = RECT { left: 12, top: 8, right: 800, bottom: 40 };
                DrawTextW(hdc, &mut msg, &mut r, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
            }

            SelectObject(hdc, old_font);
            DeleteObject(font);
            EndPaint(hwnd, &ps);
        }
    }
}
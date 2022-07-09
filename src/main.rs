use std::mem::size_of;
use std::process::Command;
use std::ptr::null_mut;
use std::thread::sleep;
use std::time::Duration;
use anyhow::anyhow;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{BI_BITFIELDS, BitBlt, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, DIB_RGB_COLORS, GetDC, GetDIBits, ReleaseDC, SelectObject, SetDIBits, SRCCOPY};
use windows::Win32::UI::HiDpi::{PROCESS_PER_MONITOR_DPI_AWARE, SetProcessDpiAwareness};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetClientRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER};

pub struct Image {
    pub width: usize,
    pub height: usize,
    pub buf: Vec<u8>,
}

impl Image {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            buf: vec![0u8; width * height * 4],
        }
    }
    pub fn from_window(class_name: &str, window_name: &str, x: i32, y: i32) -> anyhow::Result<Self> {
        unsafe {
            let hwnd = FindWindowW(class_name, window_name);
            if hwnd.0 == 0 {
                return Err(anyhow!("Window not found"));
            }
            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);
            let width = (rect.right - rect.left) as usize;
            let height = (rect.bottom - rect.top) as usize;
            if width == 0 || height == 0 {
                return Err(anyhow!("Window not shown"));
            }
            let hdc = GetDC(hwnd);
            let hmemdc = CreateCompatibleDC(hdc);
            let hbm = CreateCompatibleBitmap(hdc, width as i32, height as i32);
            let hbm_old = SelectObject(hmemdc, hbm);
            BitBlt(hmemdc, 0, 0, width as i32, height as i32, hdc, x, y, SRCCOPY);
            let mut bmi_buf = [0u8; (size_of::<BITMAPINFO>() + 8)]; // 因为调色板 bmiColors 是个变长数组，RGB 三个颜色，数组实际长度是 3，比 1 个元素多出 8 字节
            let bmi = &mut *(bmi_buf.as_mut_ptr() as *mut BITMAPINFO);
            bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            GetDIBits(hmemdc, hbm, 0, 0, null_mut(), bmi, DIB_RGB_COLORS);
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_BITFIELDS as u32;
            bmi.bmiColors.get_unchecked_mut(0).rgbRed = 255;
            bmi.bmiColors.get_unchecked_mut(1).rgbGreen = 255;
            bmi.bmiColors.get_unchecked_mut(2).rgbBlue = 255;
            let mut buf = vec![0u8; width * height * 4];
            GetDIBits(hmemdc, hbm, 0, height as u32, buf.as_mut_ptr() as _, bmi, DIB_RGB_COLORS);
            let _ = SelectObject(hmemdc, hbm_old);
            DeleteObject(hbm);
            DeleteDC(hmemdc);
            ReleaseDC(hwnd, hdc);
            Ok(Self {
                width,
                height,
                buf,
            })
        }
    }
    pub fn paint_to_window(&self, class_name: &str, window_name: &str, x: i32, y: i32) -> anyhow::Result<()> {
        unsafe {
            let hwnd = FindWindowW(class_name, window_name);
            if hwnd.0 == 0 {
                return Err(anyhow!("Window not found"));
            }
            let hdc = GetDC(hwnd);
            let hmemdc = CreateCompatibleDC(hdc);
            let hbm = CreateCompatibleBitmap(hdc, self.width as i32, self.height as i32);
            let hbm_old = SelectObject(hmemdc, hbm);
            let mut bmi_buf = [0u8; (size_of::<BITMAPINFO>() + 8)];
            let bmi = &mut *(bmi_buf.as_mut_ptr() as *mut BITMAPINFO);
            bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            GetDIBits(hmemdc, hbm, 0, 0, null_mut(), bmi, DIB_RGB_COLORS);
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_BITFIELDS as u32;
            bmi.bmiColors.get_unchecked_mut(0).rgbRed = 255;
            bmi.bmiColors.get_unchecked_mut(1).rgbGreen = 255;
            bmi.bmiColors.get_unchecked_mut(2).rgbBlue = 255;
            SetDIBits(hmemdc, hbm, 0, self.height as u32, self.buf.as_ptr() as _, bmi, DIB_RGB_COLORS);
            BitBlt(hdc, x, y, self.width as i32, self.height as i32, hmemdc, 0, 0, SRCCOPY);
            let _ = SelectObject(hmemdc, hbm_old);
            DeleteObject(hbm);
            DeleteDC(hmemdc);
            ReleaseDC(hwnd, hdc);
            Ok(())
        }
    }
    pub fn from_fn<F: Fn(usize, usize) -> (u8, u8, u8)>(width: usize, height: usize, f: F) -> Self {
        let mut image = Self::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let (r, g, b) = f(x, y);
                image.set_color(x, y, r, g, b);
            }
        }
        image
    }
    pub fn get_offset(&self, x: usize, y: usize) -> usize {
        ((self.height - 1 - y) * self.width + x) * 4
    }
    pub fn get_color(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let offset = self.get_offset(x, y);
        (self.buf[offset + 2], self.buf[offset + 1], self.buf[offset + 0])
    }
    pub fn set_color(&mut self, x: usize, y: usize, r: u8, g: u8, b: u8) {
        let offset = self.get_offset(x, y);
        self.buf[offset + 2] = r;
        self.buf[offset + 1] = g;
        self.buf[offset + 0] = b;
    }
    pub fn get_grayscale_color(&self, x: usize, y: usize) -> u8 {
        let (r, g, b) = self.get_color(x, y);
        return r / 4 + g / 2 + b / 4;
    }
    pub fn is_white(&self, x: usize, y: usize) -> bool {
        self.get_grayscale_color(x, y) > 192
    }
    pub fn is_edge(&self, x: usize, y: usize) -> bool {
        let a = self.is_white(x, y);
        let b = self.is_white(x, y + 1);
        let c = self.is_white(x + 1, y);
        let d = self.is_white(x + 1, y + 1);
        !(a == b && a == c && a == d)
    }
    pub fn to_taskmgr_style(&self) -> Self {
        Self::from_fn(self.width - 1, self.height - 1, |x, y| {
            if x == 0 || y == 0 || x == self.width - 2 || y == self.height - 2 || self.is_edge(x, y) {
                (0x4c, 0x9d, 0xcb) // 边框
            } else if x % 50 == 0 || y % 50 == 0 {
                (0xd9, 0xea, 0xf4) // 网格
            } else if self.is_white(x, y) {
                (0xff, 0xff, 0xff) // 白色
            } else {
                (0xf1, 0xf6, 0xfa) // 黑色
            }
        })
    }
}

fn main() -> anyhow::Result<()> {
    unsafe {
        let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);

        let hwnd_taskmgr = FindWindowW("TaskManagerWindow", "任务管理器");
        if hwnd_taskmgr.0 == 0 {
            Command::new("taskmgr.exe")
                .spawn()?;
        }
        let hwnd_ffplay = FindWindowW("SDL_app", "ffplay");
        if hwnd_ffplay.0 == 0 {
            Command::new("ffplay.exe")
                .arg("-x")
                .arg("540")
                .arg("-volume")
                .arg("1")
                .arg("-window_title")
                .arg("ffplay")
                .arg(std::env::args().skip(1).next().unwrap())
                .spawn()?;
        }

        loop {
            if let Ok(img) = Image::from_window("SDL_app", "ffplay", 0, 0) {
                let hwnd_taskmgr = FindWindowW("TaskManagerWindow", "任务管理器");
                if hwnd_taskmgr.0 != 0 {
                    let img2 = img.to_taskmgr_style();
                    SetWindowPos(hwnd_taskmgr, None, 0, 0, (img2.width + 396) as i32, (img2.height + 508) as i32, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
                    let _ = img2.paint_to_window("TaskManagerWindow", "任务管理器", 350, 126);
                } else {
                    sleep(Duration::from_millis(16));
                }
            } else {
                sleep(Duration::from_millis(16));
            }
        }
    }
}
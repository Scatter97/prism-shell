//! Real application icon extraction.
//!
//! `SHGetFileInfoW` supplies the shell icon for an executable path. The HICON
//! is converted to straight RGBA and handed to Slint as an embedded image.

use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits,
    GetObjectW, HGDIOBJ, ReleaseDC,
};
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::UI::Shell::{
    SHFILEINFOW, SHGFI_FLAGS, SHGFI_ICON, SHGFI_LARGEICON, SHGetFileInfoW,
};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};
use windows::core::PCWSTR;

use crate::host::to_wide;

pub fn load_icon(path: &str) -> Option<slint::Image> {
    if path.trim().is_empty() {
        return None;
    }
    let wide = to_wide(path);
    unsafe {
        let mut info = SHFILEINFOW::default();
        let result = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_FLAGS(SHGFI_ICON.0 | SHGFI_LARGEICON.0),
        );
        if result == 0 || info.hIcon.0.is_null() {
            return None;
        }
        let image = icon_to_image(info.hIcon);
        let _ = DestroyIcon(info.hIcon);
        image
    }
}

unsafe fn icon_to_image(hicon: HICON) -> Option<slint::Image> {
    let mut icon_info = ICONINFO::default();
    if unsafe { GetIconInfo(hicon, &mut icon_info) }.is_err() {
        return None;
    }

    let color = icon_info.hbmColor;
    let mask = icon_info.hbmMask;

    let image = unsafe { bitmap_to_image(color, mask) };

    if !color.0.is_null() {
        let _ = unsafe { DeleteObject(HGDIOBJ(color.0)) };
    }
    if !mask.0.is_null() {
        let _ = unsafe { DeleteObject(HGDIOBJ(mask.0)) };
    }
    image
}

unsafe fn bitmap_to_image(
    color: windows::Win32::Graphics::Gdi::HBITMAP,
    mask: windows::Win32::Graphics::Gdi::HBITMAP,
) -> Option<slint::Image> {
    if color.0.is_null() {
        return None;
    }

    let mut bitmap = BITMAP::default();
    let got = unsafe {
        GetObjectW(
            HGDIOBJ(color.0),
            size_of::<BITMAP>() as i32,
            Some(&mut bitmap as *mut _ as *mut c_void),
        )
    };
    if got == 0 {
        return None;
    }

    let width = bitmap.bmWidth.clamp(1, 256);
    let height = bitmap.bmHeight.clamp(1, 256);
    if width == 0 || height == 0 {
        return None;
    }

    let mut color_info = BITMAPINFO::default();
    color_info.bmiHeader = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: -height,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    let hdc = unsafe { GetDC(None) };
    if hdc.0.is_null() {
        return None;
    }
    let scanned = unsafe {
        GetDIBits(
            hdc,
            color,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut c_void),
            &mut color_info,
            DIB_RGB_COLORS,
        )
    };
    if scanned == 0 {
        let _ = unsafe { ReleaseDC(None, hdc) };
        return None;
    }

    let mut mask_bits = vec![0u8; 0];
    let mut mask_stride = 0usize;
    if !mask.0.is_null() {
        mask_stride = (((width + 31) / 32) * 4) as usize;
        mask_bits.resize(mask_stride * height as usize, 0);
        let mut mask_info = BITMAPINFO::default();
        mask_info.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 1,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let _ = unsafe {
            GetDIBits(
                hdc,
                mask,
                0,
                height as u32,
                Some(mask_bits.as_mut_ptr() as *mut c_void),
                &mut mask_info,
                DIB_RGB_COLORS,
            )
        };
    }
    let _ = unsafe { ReleaseDC(None, hdc) };

    let has_alpha = pixels.chunks_exact(4).any(|pixel| pixel[3] != 0);
    let mut buffer =
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width as u32, height as u32);
    let target = buffer.make_mut_slice();

    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize;
            let b = pixels[index * 4];
            let g = pixels[index * 4 + 1];
            let r = pixels[index * 4 + 2];
            let mut a = pixels[index * 4 + 3];
            if !has_alpha {
                let transparent = if mask_stride != 0 {
                    let row = y as usize * mask_stride;
                    let col = x as usize;
                    let byte = mask_bits[row + col / 8];
                    (byte >> (7 - (col % 8))) & 1 != 0
                } else {
                    false
                };
                a = if transparent { 0 } else { 255 };
            }
            target[index] = slint::Rgba8Pixel { r, g, b, a };
        }
    }

    Some(slint::Image::from_rgba8(buffer))
}

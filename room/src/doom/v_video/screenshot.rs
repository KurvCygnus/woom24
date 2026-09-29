//! Screenshots and the table loaders: the PCX writer, the F-key screenshot
//! path, the mouse-speed HUD box, and the `TINTTAB`/`XLATAB` loaders.

use std::ffi::{c_char, c_int, c_void};
use std::ptr;

use crate::doom::i_video::{
    mouse_acceleration, mouse_threshold, usemouse, I_GetPaletteIndex, I_VideoBuffer, SCREENHEIGHT,
    SCREENWIDTH,
};
use crate::doom::m_misc::{M_FileExists, M_WriteFile};
use crate::doom::w_wad::W_CacheLumpName;
use crate::doom::z_zone::{PU_CACHE, PU_STATIC, Z_Free, Z_Malloc};
use crate::i_error;

use super::blit::{draw_box, draw_filled_box, draw_horiz_line, draw_vert_line};
use super::state::{tinttable, xlatab};

/// PCX file image header (`pcx_t`), laid out exactly as the C `#pragma pack`
/// original; the pixel payload follows the `data` field in the same malloc
/// block.
#[repr(C, packed)]
struct pcx_t {
    manufacturer: c_char,
    version: c_char,
    encoding: c_char,
    bits_per_pixel: c_char,
    xmin: u16,
    ymin: u16,
    xmax: u16,
    ymax: u16,
    hres: u16,
    vres: u16,
    palette: [u8; 48],
    reserved: c_char,
    color_planes: c_char,
    bytes_per_line: u16,
    palette_type: u16,
    filler: [u8; 58],
    data: u8,
}

/// Writes `width x height` bytes of indexed pixel data plus a 768-byte
/// palette as an uncompressed PCX file (RLE-encoded, though every literal
/// run uses the two-byte escape form).
#[doc(alias = "WritePCXfile")]
#[export_name = "WritePCXfile"]
pub extern "C" fn write_pcx_file(
    filename: *mut c_char,
    data: *mut u8,
    width: c_int,
    height: c_int,
    palette: *mut u8,
) {
    unsafe {
        let pcx = Z_Malloc(width * height * 2 + 1000, PU_STATIC, ptr::null_mut()) as *mut pcx_t;

        (*pcx).manufacturer = 0x0a;
        (*pcx).version = 5;
        (*pcx).encoding = 1;
        (*pcx).bits_per_pixel = 8;
        (*pcx).xmin = 0;
        (*pcx).ymin = 0;
        (*pcx).xmax = (width - 1) as u16;
        (*pcx).ymax = (height - 1) as u16;
        (*pcx).hres = width as u16;
        (*pcx).vres = height as u16;
        (*pcx).palette = [0; 48];
        (*pcx).color_planes = 1;
        (*pcx).bytes_per_line = width as u16;
        (*pcx).palette_type = 2;
        (*pcx).filler = [0; 58];

        let mut pack = (pcx as *mut u8).add(std::mem::offset_of!(pcx_t, data));

        let mut i = 0;
        let total = (width * height) as usize;
        let mut data_ptr = data;
        while i < total {
            let byte = *data_ptr;
            if (byte & 0xc0) != 0xc0 {
                *pack = byte;
                pack = pack.add(1);
            } else {
                *pack = 0xc1;
                pack = pack.add(1);
                *pack = byte;
                pack = pack.add(1);
            }
            data_ptr = data_ptr.add(1);
            i += 1;
        }

        // write palette
        *pack = 0x0c;
        pack = pack.add(1);

        let mut palette_ptr = palette;
        i = 0;
        while i < 768 {
            *pack = *palette_ptr;
            pack = pack.add(1);
            palette_ptr = palette_ptr.add(1);
            i += 1;
        }

        let length = pack.offset_from(pcx as *mut u8) as c_int;
        M_WriteFile(filename, pcx as *mut c_void, length);

        Z_Free(pcx as *mut c_void);
    }
}

/// Build a screenshot filename for index `i`: `"DOOM{i:02}.pcx"` in a 16-byte null-padded buffer.
/// The only real caller always passes `"DOOM%02i.%s"` as format, so the pattern is fixed.
fn screenshot_filename(i: i32) -> [u8; 16] {
    let s = format!("DOOM{:02}.pcx", i);
    let mut buf = [0u8; 16];
    buf[..s.len()].copy_from_slice(s.as_bytes());
    buf
}

/// Captures the primary framebuffer as the next free `DOOMnn.pcx`.
#[doc(alias = "V_ScreenShot")]
#[export_name = "V_ScreenShot"]
pub extern "C" fn screen_shot(_format: *mut c_char) {
    unsafe {
        let mut i = 0;
        while i <= 99 {
            let mut lbmname = screenshot_filename(i);
            if M_FileExists(lbmname.as_mut_ptr() as *mut c_char) == 0 {
                break;
            }
            i += 1;
        }

        if i == 100 {
            i_error!("V_ScreenShot: Couldn't create a PCX");
        }

        let mut lbmname = screenshot_filename(i);
        write_pcx_file(
            lbmname.as_mut_ptr() as *mut c_char,
            I_VideoBuffer,
            SCREENWIDTH,
            SCREENHEIGHT,
            W_CacheLumpName(c"PLAYPAL".as_ptr(), PU_CACHE) as *mut u8,
        );
    }
}

const MOUSE_SPEED_BOX_WIDTH: c_int = 120;
const MOUSE_SPEED_BOX_HEIGHT: c_int = 9;

/// Draws the mouse-sensitivity test HUD (only under `testcontrols`).
#[doc(alias = "V_DrawMouseSpeedBox")]
#[export_name = "V_DrawMouseSpeedBox"]
pub extern "C" fn draw_mouse_speed_box(speed: c_int) {
    unsafe {
        let bgcolor = I_GetPaletteIndex(0x77, 0x77, 0x77);
        let bordercolor = I_GetPaletteIndex(0x55, 0x55, 0x55);
        let red = I_GetPaletteIndex(0xff, 0x00, 0x00);
        let black = I_GetPaletteIndex(0x00, 0x00, 0x00);
        let yellow = I_GetPaletteIndex(0xff, 0xff, 0x00);
        let white = I_GetPaletteIndex(0xff, 0xff, 0xff);

        if usemouse == 0 || (mouse_acceleration - 1.0).abs() < 0.01 {
            return;
        }

        let box_x = SCREENWIDTH - MOUSE_SPEED_BOX_WIDTH - 10;
        let box_y = 15;

        draw_filled_box(
            box_x,
            box_y,
            MOUSE_SPEED_BOX_WIDTH,
            MOUSE_SPEED_BOX_HEIGHT,
            bgcolor,
        );
        draw_box(
            box_x,
            box_y,
            MOUSE_SPEED_BOX_WIDTH,
            MOUSE_SPEED_BOX_HEIGHT,
            bordercolor,
        );

        let redline_x = MOUSE_SPEED_BOX_WIDTH / 3;

        let original_speed = if speed < mouse_threshold {
            speed
        } else {
            let mut s = speed - mouse_threshold;
            s = (s as f32 / mouse_acceleration) as c_int;
            s + mouse_threshold
        };

        let mut linelen = (original_speed * redline_x) / mouse_threshold;
        if linelen > MOUSE_SPEED_BOX_WIDTH - 1 {
            linelen = MOUSE_SPEED_BOX_WIDTH - 1;
        }

        draw_horiz_line(box_x + 1, box_y + 4, MOUSE_SPEED_BOX_WIDTH - 2, black);

        if linelen < redline_x {
            draw_horiz_line(
                box_x + 1,
                box_y + MOUSE_SPEED_BOX_HEIGHT / 2,
                linelen,
                white,
            );
        } else {
            draw_horiz_line(
                box_x + 1,
                box_y + MOUSE_SPEED_BOX_HEIGHT / 2,
                redline_x,
                white,
            );
            draw_horiz_line(
                box_x + redline_x,
                box_y + MOUSE_SPEED_BOX_HEIGHT / 2,
                linelen - redline_x,
                yellow,
            );
        }

        draw_vert_line(
            box_x + redline_x,
            box_y + 1,
            MOUSE_SPEED_BOX_HEIGHT - 2,
            red,
        );
    }
}

/// Loads the translucent-draw lookup table from the `TINTTAB` lump.
///
/// Dead-but-exported (boot never calls it in-tree): kept for symbol-set
/// byte-identity with the dead `TL` patch family, retires with the freeze
/// zone.
#[doc(alias = "V_LoadTintTable")]
#[export_name = "V_LoadTintTable"]
pub extern "C" fn load_tint_table() {
    unsafe {
        tinttable = W_CacheLumpName(c"TINTTAB".as_ptr(), PU_STATIC) as *mut u8;
    }
}

/// Loads the `XLATAB` translucent-draw lookup table.
///
/// Dead-but-exported (boot never calls it in-tree): kept for symbol-set
/// byte-identity with the dead `XLA` patch family, retires with the freeze
/// zone.
#[doc(alias = "V_LoadXlaTable")]
#[export_name = "V_LoadXlaTable"]
pub extern "C" fn load_xla_table() {
    unsafe {
        xlatab = W_CacheLumpName(c"XLATAB".as_ptr(), PU_STATIC) as *mut u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_filename_zero_padded() {
        let name = screenshot_filename(0);
        assert_eq!(&name[..10], b"DOOM00.pcx");
        assert_eq!(name[10], 0);
    }

    #[test]
    fn screenshot_filename_two_digit() {
        let name = screenshot_filename(42);
        assert_eq!(&name[..10], b"DOOM42.pcx");
    }

    #[test]
    fn screenshot_filename_max() {
        let name = screenshot_filename(99);
        assert_eq!(&name[..10], b"DOOM99.pcx");
    }
}

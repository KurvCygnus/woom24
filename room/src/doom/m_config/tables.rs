//! The configuration variable tables: the entry/collection types and the
//! two static tables (`doom_defaults`, `extra_defaults`) whose order and
//! names are kept verbatim from the C source so on-disk `.cfg` files stay
//! compatible.

#![allow(non_upper_case_globals, static_mut_refs, clippy::manual_c_str_literals)]

use std::ffi::{c_char, c_int, c_void};
use std::ptr;

/// Type tag for a configuration variable.
///
/// Mirrors the C `default_type_t` enum (`DEFAULT_INT`, `DEFAULT_INT_HEX`,
/// `DEFAULT_STRING`, `DEFAULT_FLOAT`, `DEFAULT_KEY`). The order is
/// significant because the variant is serialised across the FFI boundary
/// and compared with `PartialEq` in the getters in `vars.rs`.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ConfigValueType {
    /// Decimal integer (also accepts `0x` hex when read from disk).
    Int = 0,
    /// Hexadecimal integer (display form `0x...`).
    IntHex,
    /// Heap-allocated null-terminated string.
    String,
    /// Single-precision float parsed via libc `atof`.
    Float,
    /// DOS scancode that maps through `SCANTOKEY` to an internal key code.
    Key,
}

/// Description of a single configuration variable.
///
/// Mirrors the C `default_t` struct, including the `untranslated` /
/// `original_translated` bookkeeping used to roundtrip DOS scancodes.
#[repr(C)]
pub(super) struct ConfigVarEntry {
    /// Variable name, e.g. `"key_fire\0"`; pointers into `'static` byte
    /// literals supplied by the `config_var_entry` helper.
    pub(super) name: *const c_char,
    /// Pointer to the in-memory location set by `bind_variable`.
    /// NULL until binding.
    pub(super) location: *mut c_void,
    /// Data type stored at `location`.
    pub(super) ty: ConfigValueType,
    /// For `Key` variables: original DOS scancode read from the config
    /// file before translation through `SCANTOKEY`. Zero if never loaded.
    pub(super) untranslated: c_int,
    /// For `Key` variables: the translated internal key code at the time
    /// the value was loaded; used to detect user edits when saving.
    pub(super) original_translated: c_int,
    /// `true` once `bind_variable` has attached this entry to memory.
    pub(super) bound: bool,
}

/// A named group of `ConfigVarEntry` entries shared by a single config file.
///
/// Mirrors the C `default_collection_t` struct.
pub(super) struct ConfigVarCollection {
    /// Pointer to the first entry of an array of `numdefaults`
    /// `ConfigVarEntry`s.
    pub(super) defaults: *mut ConfigVarEntry,
    /// Number of entries in the `defaults` array.
    pub(super) numdefaults: c_int,
    /// Filename used by `load_defaults` / `save_defaults`. NULL until
    /// set, owned by the caller's argv or by `M_StringJoinA`.
    pub(super) filename: *mut c_char,
}

/// Construct a `ConfigVarEntry` from a NUL-terminated name literal and a
/// type tag.
///
/// `const`-evaluated so the static variable tables below can be declared at
/// module level. Replaces the C `CONFIG_VARIABLE_*` macros.
pub(super) const fn config_var_entry(name: &'static [u8], ty: ConfigValueType) -> ConfigVarEntry {
    ConfigVarEntry {
        name: name.as_ptr() as *const c_char,
        location: ptr::null_mut(),
        ty,
        untranslated: 0,
        original_translated: 0,
        bound: false,
    }
}


/// Static table backing the vanilla `doom_defaults` collection.
///
/// Order and names are kept verbatim from the C source so that on-disk
/// `.cfg` files remain compatible. Entries become live once
/// `bind_variable` attaches a memory location.
pub(super) static mut DOOM_DEFAULTS_LIST: [ConfigVarEntry; 76] = [
    config_var_entry(b"mouse_sensitivity\0", ConfigValueType::Int),
    config_var_entry(b"sfx_volume\0", ConfigValueType::Int),
    config_var_entry(b"music_volume\0", ConfigValueType::Int),
    config_var_entry(b"show_talk\0", ConfigValueType::Int),
    config_var_entry(b"voice_volume\0", ConfigValueType::Int),
    config_var_entry(b"show_messages\0", ConfigValueType::Int),
    config_var_entry(b"key_right\0", ConfigValueType::Key),
    config_var_entry(b"key_left\0", ConfigValueType::Key),
    config_var_entry(b"key_up\0", ConfigValueType::Key),
    config_var_entry(b"key_down\0", ConfigValueType::Key),
    config_var_entry(b"key_strafeleft\0", ConfigValueType::Key),
    config_var_entry(b"key_straferight\0", ConfigValueType::Key),
    config_var_entry(b"key_useHealth\0", ConfigValueType::Key),
    config_var_entry(b"key_jump\0", ConfigValueType::Key),
    config_var_entry(b"key_flyup\0", ConfigValueType::Key),
    config_var_entry(b"key_flydown\0", ConfigValueType::Key),
    config_var_entry(b"key_flycenter\0", ConfigValueType::Key),
    config_var_entry(b"key_lookup\0", ConfigValueType::Key),
    config_var_entry(b"key_lookdown\0", ConfigValueType::Key),
    config_var_entry(b"key_lookcenter\0", ConfigValueType::Key),
    config_var_entry(b"key_invquery\0", ConfigValueType::Key),
    config_var_entry(b"key_mission\0", ConfigValueType::Key),
    config_var_entry(b"key_invPop\0", ConfigValueType::Key),
    config_var_entry(b"key_invKey\0", ConfigValueType::Key),
    config_var_entry(b"key_invHome\0", ConfigValueType::Key),
    config_var_entry(b"key_invEnd\0", ConfigValueType::Key),
    config_var_entry(b"key_invleft\0", ConfigValueType::Key),
    config_var_entry(b"key_invright\0", ConfigValueType::Key),
    config_var_entry(b"key_invLeft\0", ConfigValueType::Key),
    config_var_entry(b"key_invRight\0", ConfigValueType::Key),
    config_var_entry(b"key_useartifact\0", ConfigValueType::Key),
    config_var_entry(b"key_invUse\0", ConfigValueType::Key),
    config_var_entry(b"key_invDrop\0", ConfigValueType::Key),
    config_var_entry(b"key_lookUp\0", ConfigValueType::Key),
    config_var_entry(b"key_lookDown\0", ConfigValueType::Key),
    config_var_entry(b"key_fire\0", ConfigValueType::Key),
    config_var_entry(b"key_use\0", ConfigValueType::Key),
    config_var_entry(b"key_strafe\0", ConfigValueType::Key),
    config_var_entry(b"key_speed\0", ConfigValueType::Key),
    config_var_entry(b"use_mouse\0", ConfigValueType::Int),
    config_var_entry(b"mouseb_fire\0", ConfigValueType::Int),
    config_var_entry(b"mouseb_strafe\0", ConfigValueType::Int),
    config_var_entry(b"mouseb_forward\0", ConfigValueType::Int),
    config_var_entry(b"mouseb_jump\0", ConfigValueType::Int),
    config_var_entry(b"use_joystick\0", ConfigValueType::Int),
    config_var_entry(b"joyb_fire\0", ConfigValueType::Int),
    config_var_entry(b"joyb_strafe\0", ConfigValueType::Int),
    config_var_entry(b"joyb_use\0", ConfigValueType::Int),
    config_var_entry(b"joyb_speed\0", ConfigValueType::Int),
    config_var_entry(b"joyb_jump\0", ConfigValueType::Int),
    config_var_entry(b"screenblocks\0", ConfigValueType::Int),
    config_var_entry(b"screensize\0", ConfigValueType::Int),
    config_var_entry(b"detaillevel\0", ConfigValueType::Int),
    config_var_entry(b"snd_channels\0", ConfigValueType::Int),
    config_var_entry(b"snd_musicdevice\0", ConfigValueType::Int),
    config_var_entry(b"snd_sfxdevice\0", ConfigValueType::Int),
    config_var_entry(b"snd_sbport\0", ConfigValueType::Int),
    config_var_entry(b"snd_sbirq\0", ConfigValueType::Int),
    config_var_entry(b"snd_sbdma\0", ConfigValueType::Int),
    config_var_entry(b"snd_mport\0", ConfigValueType::Int),
    config_var_entry(b"usegamma\0", ConfigValueType::Int),
    config_var_entry(b"savedir\0", ConfigValueType::String),
    config_var_entry(b"messageson\0", ConfigValueType::Int),
    config_var_entry(b"back_flat\0", ConfigValueType::String),
    config_var_entry(b"nickname\0", ConfigValueType::String),
    config_var_entry(b"chatmacro0\0", ConfigValueType::String),
    config_var_entry(b"chatmacro1\0", ConfigValueType::String),
    config_var_entry(b"chatmacro2\0", ConfigValueType::String),
    config_var_entry(b"chatmacro3\0", ConfigValueType::String),
    config_var_entry(b"chatmacro4\0", ConfigValueType::String),
    config_var_entry(b"chatmacro5\0", ConfigValueType::String),
    config_var_entry(b"chatmacro6\0", ConfigValueType::String),
    config_var_entry(b"chatmacro7\0", ConfigValueType::String),
    config_var_entry(b"chatmacro8\0", ConfigValueType::String),
    config_var_entry(b"chatmacro9\0", ConfigValueType::String),
    config_var_entry(b"comport\0", ConfigValueType::Int),
];

/// Collection wrapper around [`DOOM_DEFAULTS_LIST`] for the vanilla config
/// file. Mirrors C `static default_collection_t doom_defaults`.
pub(super) static mut doom_defaults: ConfigVarCollection = ConfigVarCollection {
    defaults: unsafe { &mut DOOM_DEFAULTS_LIST as *mut _ },
    numdefaults: 76,
    filename: ptr::null_mut(),
};

/// Static table backing the chocolate-doom `extra_defaults` collection.
///
/// Holds the extended (non-vanilla) settings - video, joystick, mouse and
/// extra key bindings. Order matches the C source.
pub(super) static mut EXTRA_DEFAULTS_LIST: [ConfigVarEntry; 119] = [
    config_var_entry(b"graphical_startup\0", ConfigValueType::Int),
    config_var_entry(b"autoadjust_video_settings\0", ConfigValueType::Int),
    config_var_entry(b"fullscreen\0", ConfigValueType::Int),
    config_var_entry(b"aspect_ratio_correct\0", ConfigValueType::Int),
    config_var_entry(b"startup_delay\0", ConfigValueType::Int),
    config_var_entry(b"screen_width\0", ConfigValueType::Int),
    config_var_entry(b"screen_height\0", ConfigValueType::Int),
    config_var_entry(b"screen_bpp\0", ConfigValueType::Int),
    config_var_entry(b"grabmouse\0", ConfigValueType::Int),
    config_var_entry(b"novert\0", ConfigValueType::Int),
    config_var_entry(b"mouse_acceleration\0", ConfigValueType::Float),
    config_var_entry(b"mouse_threshold\0", ConfigValueType::Int),
    config_var_entry(b"snd_samplerate\0", ConfigValueType::Int),
    config_var_entry(b"snd_cachesize\0", ConfigValueType::Int),
    config_var_entry(b"snd_maxslicetime_ms\0", ConfigValueType::Int),
    config_var_entry(b"snd_musiccmd\0", ConfigValueType::String),
    config_var_entry(b"opl_io_port\0", ConfigValueType::IntHex),
    config_var_entry(b"show_endoom\0", ConfigValueType::Int),
    config_var_entry(b"png_screenshots\0", ConfigValueType::Int),
    config_var_entry(b"vanilla_savegame_limit\0", ConfigValueType::Int),
    config_var_entry(b"vanilla_demo_limit\0", ConfigValueType::Int),
    config_var_entry(b"vanilla_keyboard_mapping\0", ConfigValueType::Int),
    config_var_entry(b"video_driver\0", ConfigValueType::String),
    config_var_entry(b"window_position\0", ConfigValueType::String),
    config_var_entry(b"joystick_index\0", ConfigValueType::Int),
    config_var_entry(b"joystick_x_axis\0", ConfigValueType::Int),
    config_var_entry(b"joystick_x_invert\0", ConfigValueType::Int),
    config_var_entry(b"joystick_y_axis\0", ConfigValueType::Int),
    config_var_entry(b"joystick_y_invert\0", ConfigValueType::Int),
    config_var_entry(b"joystick_strafe_axis\0", ConfigValueType::Int),
    config_var_entry(b"joystick_strafe_invert\0", ConfigValueType::Int),
    config_var_entry(b"joystick_physical_button0\0", ConfigValueType::Int),
    config_var_entry(b"joystick_physical_button1\0", ConfigValueType::Int),
    config_var_entry(b"joystick_physical_button2\0", ConfigValueType::Int),
    config_var_entry(b"joystick_physical_button3\0", ConfigValueType::Int),
    config_var_entry(b"joystick_physical_button4\0", ConfigValueType::Int),
    config_var_entry(b"joystick_physical_button5\0", ConfigValueType::Int),
    config_var_entry(b"joystick_physical_button6\0", ConfigValueType::Int),
    config_var_entry(b"joystick_physical_button7\0", ConfigValueType::Int),
    config_var_entry(b"joystick_physical_button8\0", ConfigValueType::Int),
    config_var_entry(b"joystick_physical_button9\0", ConfigValueType::Int),
    config_var_entry(b"joyb_strafeleft\0", ConfigValueType::Int),
    config_var_entry(b"joyb_straferight\0", ConfigValueType::Int),
    config_var_entry(b"joyb_menu_activate\0", ConfigValueType::Int),
    config_var_entry(b"joyb_prevweapon\0", ConfigValueType::Int),
    config_var_entry(b"joyb_nextweapon\0", ConfigValueType::Int),
    config_var_entry(b"mouseb_strafeleft\0", ConfigValueType::Int),
    config_var_entry(b"mouseb_straferight\0", ConfigValueType::Int),
    config_var_entry(b"mouseb_use\0", ConfigValueType::Int),
    config_var_entry(b"mouseb_backward\0", ConfigValueType::Int),
    config_var_entry(b"mouseb_prevweapon\0", ConfigValueType::Int),
    config_var_entry(b"mouseb_nextweapon\0", ConfigValueType::Int),
    config_var_entry(b"dclick_use\0", ConfigValueType::Int),
    config_var_entry(b"key_pause\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_activate\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_up\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_down\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_left\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_right\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_back\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_forward\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_confirm\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_abort\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_help\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_save\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_load\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_volume\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_detail\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_qsave\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_endgame\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_messages\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_qload\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_quit\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_gamma\0", ConfigValueType::Key),
    config_var_entry(b"key_spy\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_incscreen\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_decscreen\0", ConfigValueType::Key),
    config_var_entry(b"key_menu_screenshot\0", ConfigValueType::Key),
    config_var_entry(b"key_map_toggle\0", ConfigValueType::Key),
    config_var_entry(b"key_map_north\0", ConfigValueType::Key),
    config_var_entry(b"key_map_south\0", ConfigValueType::Key),
    config_var_entry(b"key_map_east\0", ConfigValueType::Key),
    config_var_entry(b"key_map_west\0", ConfigValueType::Key),
    config_var_entry(b"key_map_zoomin\0", ConfigValueType::Key),
    config_var_entry(b"key_map_zoomout\0", ConfigValueType::Key),
    config_var_entry(b"key_map_maxzoom\0", ConfigValueType::Key),
    config_var_entry(b"key_map_follow\0", ConfigValueType::Key),
    config_var_entry(b"key_map_grid\0", ConfigValueType::Key),
    config_var_entry(b"key_map_mark\0", ConfigValueType::Key),
    config_var_entry(b"key_map_clearmark\0", ConfigValueType::Key),
    config_var_entry(b"key_weapon1\0", ConfigValueType::Key),
    config_var_entry(b"key_weapon2\0", ConfigValueType::Key),
    config_var_entry(b"key_weapon3\0", ConfigValueType::Key),
    config_var_entry(b"key_weapon4\0", ConfigValueType::Key),
    config_var_entry(b"key_weapon5\0", ConfigValueType::Key),
    config_var_entry(b"key_weapon6\0", ConfigValueType::Key),
    config_var_entry(b"key_weapon7\0", ConfigValueType::Key),
    config_var_entry(b"key_weapon8\0", ConfigValueType::Key),
    config_var_entry(b"key_prevweapon\0", ConfigValueType::Key),
    config_var_entry(b"key_nextweapon\0", ConfigValueType::Key),
    config_var_entry(b"key_arti_all\0", ConfigValueType::Key),
    config_var_entry(b"key_arti_health\0", ConfigValueType::Key),
    config_var_entry(b"key_arti_poisonbag\0", ConfigValueType::Key),
    config_var_entry(b"key_arti_blastradius\0", ConfigValueType::Key),
    config_var_entry(b"key_arti_teleport\0", ConfigValueType::Key),
    config_var_entry(b"key_arti_teleportother\0", ConfigValueType::Key),
    config_var_entry(b"key_arti_egg\0", ConfigValueType::Key),
    config_var_entry(b"key_arti_invulnerability\0", ConfigValueType::Key),
    config_var_entry(b"key_message_refresh\0", ConfigValueType::Key),
    config_var_entry(b"key_demo_quit\0", ConfigValueType::Key),
    config_var_entry(b"key_multi_msg\0", ConfigValueType::Key),
    config_var_entry(b"key_multi_msgplayer1\0", ConfigValueType::Key),
    config_var_entry(b"key_multi_msgplayer2\0", ConfigValueType::Key),
    config_var_entry(b"key_multi_msgplayer3\0", ConfigValueType::Key),
    config_var_entry(b"key_multi_msgplayer4\0", ConfigValueType::Key),
    config_var_entry(b"key_multi_msgplayer5\0", ConfigValueType::Key),
    config_var_entry(b"key_multi_msgplayer6\0", ConfigValueType::Key),
    config_var_entry(b"key_multi_msgplayer7\0", ConfigValueType::Key),
    config_var_entry(b"key_multi_msgplayer8\0", ConfigValueType::Key),
];

/// Collection wrapper around [`EXTRA_DEFAULTS_LIST`] for the extended
/// chocolate-doom config file. Mirrors C `static default_collection_t
/// extra_defaults`.
pub(super) static mut extra_defaults: ConfigVarCollection = ConfigVarCollection {
    defaults: unsafe { &mut EXTRA_DEFAULTS_LIST as *mut _ },
    numdefaults: 119,
    filename: ptr::null_mut(),
};

/// Baseline pins written before the graduation move (F10 wave F2-b): the
/// table shapes are part of the `.cfg` compat surface and must be
/// identical before and after the split.
#[cfg(test)]
mod tests {
    use super::*;

    /// The two collections keep their upstream entry counts, and the
    /// collection headers agree with the table lengths (`.cfg` compat
    /// surface: order and names are verbatim from `m_config.c`).
    #[test]
    fn defaults_table_counts_match_c() {
        unsafe {
            assert_eq!(DOOM_DEFAULTS_LIST.len(), 76);
            assert_eq!(EXTRA_DEFAULTS_LIST.len(), 119);
            assert_eq!(doom_defaults.numdefaults, 76);
            assert_eq!(extra_defaults.numdefaults, 119);
        }
    }

    /// `repr(C)` entry layout pin (LP64 host): two pointers, the type tag,
    /// the two scancode bookkeeping ints, and the bound flag.
    #[test]
    fn config_var_entry_layout_pin() {
        assert_eq!(std::mem::size_of::<ConfigVarEntry>(), 32);
    }
}

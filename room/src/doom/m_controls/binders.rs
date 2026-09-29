//! The `M_Bind*` registration helpers: each binder wires a subset of the
//! binding statics in `defaults` into the `m_config` variable table so the
//! values can be read or written through `default.cfg`. The Heretic /
//! Hexen / Strife binders are dead-but-exported in this Doom-only build.

#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_void};
use std::os::raw::c_uint;

use crate::doom::doomkeys::{KEY_DEL, KEY_INS, KEY_PGDN, KEY_PGUP};

//* Path call, not extern: m_config graduated in the same wave and its
//* `bind_variable` is called by path (the extern declaration this file
//* carried converted in the m_config lockstep commit; the upstream C
//* symbol stays pinned at the renamed definition).
use crate::doom::m_config::bind_variable;

use crate::c_write;

//
use super::defaults::*;

//
// Bind all of the common controls used by Doom and all other games.
//

/// Register the keyboard / mouse / joystick bindings shared by every game.
///
/// Called once during startup. Each `bind_variable` call associates a
/// `.cfg` variable name (matching the entry in `m_config`'s
/// `doom_defaults` / `extra_defaults` tables) with the address of the
/// corresponding `static mut` in this module, so loading a config file
/// updates the live binding.
#[doc(alias = "M_BindBaseControls")]
#[export_name = "M_BindBaseControls"]
pub extern "C" fn bind_base_controls() {
    bind_variable(
        cstr(b"key_right\0"),
        &raw mut key_right as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_left\0"),
        &raw mut key_left as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_up\0"),
        &raw mut key_up as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_down\0"),
        &raw mut key_down as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_strafeleft\0"),
        &raw mut key_strafeleft as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_straferight\0"),
        &raw mut key_straferight as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_fire\0"),
        &raw mut key_fire as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_use\0"),
        &raw mut key_use as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_strafe\0"),
        &raw mut key_strafe as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_speed\0"),
        &raw mut key_speed as *mut c_int as *mut c_void,
    );

    bind_variable(
        cstr(b"mouseb_fire\0"),
        &raw mut mousebfire as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"mouseb_strafe\0"),
        &raw mut mousebstrafe as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"mouseb_forward\0"),
        &raw mut mousebforward as *mut c_int as *mut c_void,
    );

    bind_variable(
        cstr(b"joyb_fire\0"),
        &raw mut joybfire as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"joyb_strafe\0"),
        &raw mut joybstrafe as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"joyb_use\0"),
        &raw mut joybuse as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"joyb_speed\0"),
        &raw mut joybspeed as *mut c_int as *mut c_void,
    );

    bind_variable(
        cstr(b"joyb_menu_activate\0"),
        &raw mut joybmenu as *mut c_int as *mut c_void,
    );

    // Extra controls that are not in the Vanilla versions:

    bind_variable(
        cstr(b"joyb_strafeleft\0"),
        &raw mut joybstrafeleft as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"joyb_straferight\0"),
        &raw mut joybstraferight as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"mouseb_strafeleft\0"),
        &raw mut mousebstrafeleft as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"mouseb_straferight\0"),
        &raw mut mousebstraferight as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"mouseb_use\0"),
        &raw mut mousebuse as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"mouseb_backward\0"),
        &raw mut mousebbackward as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"dclick_use\0"),
        &raw mut dclick_use as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_pause\0"),
        &raw mut key_pause as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_message_refresh\0"),
        &raw mut key_message_refresh as *mut c_int as *mut c_void,
    );
}

/// Register the Heretic-specific keyboard bindings (fly / look / inventory
/// keys).
#[doc(alias = "M_BindHereticControls")]
#[export_name = "M_BindHereticControls"]
pub extern "C" fn bind_heretic_controls() {
    bind_variable(
        cstr(b"key_flyup\0"),
        &raw mut key_flyup as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_flydown\0"),
        &raw mut key_flydown as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_flycenter\0"),
        &raw mut key_flycenter as *mut c_int as *mut c_void,
    );

    bind_variable(
        cstr(b"key_lookup\0"),
        &raw mut key_lookup as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_lookdown\0"),
        &raw mut key_lookdown as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_lookcenter\0"),
        &raw mut key_lookcenter as *mut c_int as *mut c_void,
    );

    bind_variable(
        cstr(b"key_invleft\0"),
        &raw mut key_invleft as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_invright\0"),
        &raw mut key_invright as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_useartifact\0"),
        &raw mut key_useartifact as *mut c_int as *mut c_void,
    );
}

/// Register the Hexen-specific bindings: jump on key / mouse / joystick
/// plus the eight artifact hotkeys.
#[doc(alias = "M_BindHexenControls")]
#[export_name = "M_BindHexenControls"]
pub extern "C" fn bind_hexen_controls() {
    bind_variable(
        cstr(b"key_jump\0"),
        &raw mut key_jump as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"mouseb_jump\0"),
        &raw mut mousebjump as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"joyb_jump\0"),
        &raw mut joybjump as *mut c_int as *mut c_void,
    );

    bind_variable(
        cstr(b"key_arti_all\0"),
        &raw mut key_arti_all as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_arti_health\0"),
        &raw mut key_arti_health as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_arti_poisonbag\0"),
        &raw mut key_arti_poisonbag as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_arti_blastradius\0"),
        &raw mut key_arti_blastradius as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_arti_teleport\0"),
        &raw mut key_arti_teleport as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_arti_teleportother\0"),
        &raw mut key_arti_teleportother as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_arti_egg\0"),
        &raw mut key_arti_egg as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_arti_invulnerability\0"),
        &raw mut key_arti_invulnerability as *mut c_int as *mut c_void,
    );
}

/// Register the Strife-specific bindings.
///
/// Mutates several shared bindings to their Strife defaults before
/// registration: `key_message_refresh = '/'`, `key_jump = 'a'`,
/// `key_lookup = PGUP`, `key_lookdown = PGDN`, `key_invleft = INS`,
/// `key_invright = DEL`. Mirrors C `M_BindStrifeControls`.
#[doc(alias = "M_BindStrifeControls")]
#[export_name = "M_BindStrifeControls"]
pub extern "C" fn bind_strife_controls() {
    unsafe {
        // These are shared with all games, but have different defaults:
        key_message_refresh = b'/' as c_int;

        // These keys are shared with Heretic/Hexen but have different defaults:
        key_jump = b'a' as c_int;
        key_lookup = KEY_PGUP as c_int;
        key_lookdown = KEY_PGDN as c_int;
        key_invleft = KEY_INS as c_int;
        key_invright = KEY_DEL as c_int;

        bind_variable(
            cstr(b"key_jump\0"),
            &raw mut key_jump as *mut c_int as *mut c_void,
        );
        bind_variable(
            cstr(b"key_lookUp\0"),
            &raw mut key_lookup as *mut c_int as *mut c_void,
        );
        bind_variable(
            cstr(b"key_lookDown\0"),
            &raw mut key_lookdown as *mut c_int as *mut c_void,
        );
        bind_variable(
            cstr(b"key_invLeft\0"),
            &raw mut key_invleft as *mut c_int as *mut c_void,
        );
        bind_variable(
            cstr(b"key_invRight\0"),
            &raw mut key_invright as *mut c_int as *mut c_void,
        );

        // Custom Strife-only Keys:
        bind_variable(
            cstr(b"key_useHealth\0"),
            &raw mut key_usehealth as *mut c_int as *mut c_void,
        );
        bind_variable(
            cstr(b"key_invquery\0"),
            &raw mut key_invquery as *mut c_int as *mut c_void,
        );
        bind_variable(
            cstr(b"key_mission\0"),
            &raw mut key_mission as *mut c_int as *mut c_void,
        );
        bind_variable(
            cstr(b"key_invPop\0"),
            &raw mut key_invpop as *mut c_int as *mut c_void,
        );
        bind_variable(
            cstr(b"key_invKey\0"),
            &raw mut key_invkey as *mut c_int as *mut c_void,
        );
        bind_variable(
            cstr(b"key_invHome\0"),
            &raw mut key_invhome as *mut c_int as *mut c_void,
        );
        bind_variable(
            cstr(b"key_invEnd\0"),
            &raw mut key_invend as *mut c_int as *mut c_void,
        );
        bind_variable(
            cstr(b"key_invUse\0"),
            &raw mut key_invuse as *mut c_int as *mut c_void,
        );
        bind_variable(
            cstr(b"key_invDrop\0"),
            &raw mut key_invdrop as *mut c_int as *mut c_void,
        );

        // Strife also supports jump on mouse and joystick, and in the exact same
        // manner as Hexen!
        bind_variable(
            cstr(b"mouseb_jump\0"),
            &raw mut mousebjump as *mut c_int as *mut c_void,
        );
        bind_variable(
            cstr(b"joyb_jump\0"),
            &raw mut joybjump as *mut c_int as *mut c_void,
        );
    }

}
/// Register the weapon-selection key bindings (`key_weapon1`..`8`,
/// `key_prevweapon`, `key_nextweapon`) plus the corresponding mouse and
/// joystick prev/next bindings.
#[doc(alias = "M_BindWeaponControls")]
#[export_name = "M_BindWeaponControls"]
pub extern "C" fn bind_weapon_controls() {
    bind_variable(
        cstr(b"key_weapon1\0"),
        &raw mut key_weapon1 as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_weapon2\0"),
        &raw mut key_weapon2 as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_weapon3\0"),
        &raw mut key_weapon3 as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_weapon4\0"),
        &raw mut key_weapon4 as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_weapon5\0"),
        &raw mut key_weapon5 as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_weapon6\0"),
        &raw mut key_weapon6 as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_weapon7\0"),
        &raw mut key_weapon7 as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_weapon8\0"),
        &raw mut key_weapon8 as *mut c_int as *mut c_void,
    );

    bind_variable(
        cstr(b"key_prevweapon\0"),
        &raw mut key_prevweapon as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_nextweapon\0"),
        &raw mut key_nextweapon as *mut c_int as *mut c_void,
    );

    bind_variable(
        cstr(b"joyb_prevweapon\0"),
        &raw mut joybprevweapon as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"joyb_nextweapon\0"),
        &raw mut joybnextweapon as *mut c_int as *mut c_void,
    );

    bind_variable(
        cstr(b"mouseb_prevweapon\0"),
        &raw mut mousebprevweapon as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"mouseb_nextweapon\0"),
        &raw mut mousebnextweapon as *mut c_int as *mut c_void,
    );
}

/// Register the automap movement / zoom / overlay bindings.
#[doc(alias = "M_BindMapControls")]
#[export_name = "M_BindMapControls"]
pub extern "C" fn bind_map_controls() {
    bind_variable(
        cstr(b"key_map_north\0"),
        &raw mut key_map_north as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_map_south\0"),
        &raw mut key_map_south as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_map_east\0"),
        &raw mut key_map_east as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_map_west\0"),
        &raw mut key_map_west as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_map_zoomin\0"),
        &raw mut key_map_zoomin as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_map_zoomout\0"),
        &raw mut key_map_zoomout as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_map_toggle\0"),
        &raw mut key_map_toggle as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_map_maxzoom\0"),
        &raw mut key_map_maxzoom as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_map_follow\0"),
        &raw mut key_map_follow as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_map_grid\0"),
        &raw mut key_map_grid as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_map_mark\0"),
        &raw mut key_map_mark as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_map_clearmark\0"),
        &raw mut key_map_clearmark as *mut c_int as *mut c_void,
    );
}

/// Register the in-game menu navigation keys plus the F1..F12 function
/// shortcuts (help / save / load / volume / detail / qsave / endgame /
/// messages / qload / quit / gamma) and the increment/decrement screen-
/// size and screenshot bindings.
#[doc(alias = "M_BindMenuControls")]
#[export_name = "M_BindMenuControls"]
pub extern "C" fn bind_menu_controls() {
    bind_variable(
        cstr(b"key_menu_activate\0"),
        &raw mut key_menu_activate as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_up\0"),
        &raw mut key_menu_up as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_down\0"),
        &raw mut key_menu_down as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_left\0"),
        &raw mut key_menu_left as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_right\0"),
        &raw mut key_menu_right as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_back\0"),
        &raw mut key_menu_back as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_forward\0"),
        &raw mut key_menu_forward as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_confirm\0"),
        &raw mut key_menu_confirm as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_abort\0"),
        &raw mut key_menu_abort as *mut c_int as *mut c_void,
    );

    bind_variable(
        cstr(b"key_menu_help\0"),
        &raw mut key_menu_help as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_save\0"),
        &raw mut key_menu_save as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_load\0"),
        &raw mut key_menu_load as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_volume\0"),
        &raw mut key_menu_volume as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_detail\0"),
        &raw mut key_menu_detail as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_qsave\0"),
        &raw mut key_menu_qsave as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_endgame\0"),
        &raw mut key_menu_endgame as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_messages\0"),
        &raw mut key_menu_messages as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_qload\0"),
        &raw mut key_menu_qload as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_quit\0"),
        &raw mut key_menu_quit as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_gamma\0"),
        &raw mut key_menu_gamma as *mut c_int as *mut c_void,
    );

    bind_variable(
        cstr(b"key_menu_incscreen\0"),
        &raw mut key_menu_incscreen as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_decscreen\0"),
        &raw mut key_menu_decscreen as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_menu_screenshot\0"),
        &raw mut key_menu_screenshot as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_demo_quit\0"),
        &raw mut key_demo_quit as *mut c_int as *mut c_void,
    );
    bind_variable(
        cstr(b"key_spy\0"),
        &raw mut key_spy as *mut c_int as *mut c_void,
    );
}

/// Register multiplayer chat bindings: the broadcast `key_multi_msg`
/// plus one `key_multi_msgplayerN` binding per active player.
///
/// `num_players` should be at most 8, matching the size of
/// [`key_multi_msgplayer`]. Each per-player name is formatted into a local
/// 32-byte buffer with [`c_write!`] before being passed to
/// `bind_variable`; this replaces the C `M_snprintf` call.
#[doc(alias = "M_BindChatControls")]
#[export_name = "M_BindChatControls"]
pub extern "C" fn bind_chat_controls(num_players: c_uint) {
    unsafe {
        let mut name: [c_char; 32] = [0; 32];
        let mut i: c_uint = 0;

        bind_variable(
            cstr(b"key_multi_msg\0"),
            &raw mut key_multi_msg as *mut c_int as *mut c_void,
        );

        while i < num_players {
            c_write!(name, "key_multi_msgplayer{}", i + 1);
            bind_variable(
                name.as_ptr() as *mut c_char,
                &mut key_multi_msgplayer[i as usize] as *mut c_int as *mut c_void,
            );
            i += 1;
        }
    }
}

//
// Apply custom patches to the default values depending on the
// platform we are running on.
//

/// Apply per-platform overrides to default bindings.
///
/// Currently a no-op on every platform - mirrors the C source. Exists as
/// an extension point for porters who want to retune defaults.
#[doc(alias = "M_ApplyPlatformDefaults")]
#[export_name = "M_ApplyPlatformDefaults"]
pub extern "C" fn apply_platform_defaults() {
    // no-op. Add your platform-specific patches here.
}

/// Cast a `'static` NUL-terminated byte slice to a `*mut c_char` for use
/// as a configuration variable name.
///
/// The slice is conceptually `const`, but `bind_variable` takes
/// `*mut c_char` (matching the C signature), so the cast strips the
/// const-ness. The pointee is never mutated by the binding code; callers
/// must continue to honour that invariant.
const fn cstr(bytes: &[u8]) -> *mut c_char {
    bytes.as_ptr() as *mut c_char
}

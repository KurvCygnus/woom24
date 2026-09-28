//! Configuration-variable binding: `bind_variables` (upstream
//! `D_BindVariables`) wires every cvar, control table and chat macro to
//! the config system before `M_LoadDefaults` reads the config file.
//! One-shot boot glue -- outputs are upstream inputs to the demo
//! surface, not the per-tic surface itself.

use std::ffi::{c_char, c_int, c_void};

use crate::c_write;
use crate::doom::g_game::{vanilla_demo_limit, vanilla_savegame_limit};
use crate::doom::hu_stuff::chat_macros;
use crate::doom::i_video::I_BindVideoVariables;
use crate::doom::i_sound::I_BindSoundVariables;
use crate::doom::i_joystick::I_BindJoystickVariables;
use crate::doom::m_config::M_BindVariable;
use crate::doom::m_controls::{
    key_multi_msgplayer, M_ApplyPlatformDefaults, M_BindBaseControls, M_BindChatControls,
    M_BindMapControls, M_BindMenuControls, M_BindWeaponControls,
};
use crate::doom::m_menu::{detailLevel, screenblocks, showMessages, mouseSensitivity};
use crate::doom::s_sound::{musicVolume, snd_channels, sfxVolume};

use super::consts::{HUSTR_KEYBROWN, HUSTR_KEYGREEN, HUSTR_KEYINDIGO, HUSTR_KEYRED};
use super::state::show_endoom;

/// Bind all configuration variables.
///
/// The pre-move export symbol is kept with `#[export_name]` below;
/// `boot::doom_main` calls the upstream name through the in-module
/// alias.
#[doc(alias = "D_BindVariables")]
#[export_name = "D_BindVariables"]
pub extern "C" fn bind_variables() {
    unsafe {
        M_ApplyPlatformDefaults();

        I_BindVideoVariables();
        I_BindJoystickVariables();
        I_BindSoundVariables();

        M_BindBaseControls();
        M_BindWeaponControls();
        M_BindMapControls();
        M_BindMenuControls();
        M_BindChatControls(4); // MAXPLAYERS = 4

        key_multi_msgplayer[0] = HUSTR_KEYGREEN as c_int;
        key_multi_msgplayer[1] = HUSTR_KEYINDIGO as c_int;
        key_multi_msgplayer[2] = HUSTR_KEYBROWN as c_int;
        key_multi_msgplayer[3] = HUSTR_KEYRED as c_int;

        M_BindVariable(
            c"mouse_sensitivity".as_ptr().cast_mut(),
            &raw mut mouseSensitivity as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            c"sfx_volume".as_ptr().cast_mut(),
            &raw mut sfxVolume as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            c"music_volume".as_ptr().cast_mut(),
            &raw mut musicVolume as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            c"show_messages".as_ptr().cast_mut(),
            &raw mut showMessages as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            c"screenblocks".as_ptr().cast_mut(),
            &raw mut screenblocks as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            c"detaillevel".as_ptr().cast_mut(),
            &raw mut detailLevel as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            c"snd_channels".as_ptr().cast_mut(),
            &raw mut snd_channels as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            c"vanilla_savegame_limit".as_ptr().cast_mut(),
            &raw mut vanilla_savegame_limit as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            c"vanilla_demo_limit".as_ptr().cast_mut(),
            &raw mut vanilla_demo_limit as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            c"show_endoom".as_ptr().cast_mut(),
            &raw mut show_endoom as *mut c_int as *mut c_void,
        );

        // Multiplayer chat macros
        for i in 0..10 {
            let mut buf: [c_char; 12] = [0; 12];
            c_write!(buf, "chatmacro{}", i);
            M_BindVariable(
                buf.as_mut_ptr(),
                &mut chat_macros[i] as *mut *mut c_char as *mut c_void,
            );
        }
    }
}

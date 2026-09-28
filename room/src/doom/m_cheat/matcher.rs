//! The cheat matcher: an incremental sequence state machine plus the
//! parameter-buffer accessor, vanilla-exact.
//!
//! Fed by PLATFORM KEY EVENTS (`ST_Responder` keydowns, automap
//! keys) -- never the ticcmd/demo stream: during demo playback no key
//! events are generated, so the matcher cannot perturb a replayed
//! sequence.

use std::ffi::{c_char, c_int};

use super::types::cheatseq_t;

/// Helper: strlen for a raw c_char array (no C call needed).
///
/// Already plain-English snake_case, so it keeps its name (m_random
/// precedent: no rename, so no shim).
fn raw_strlen(buf: &[c_char]) -> usize { buf.iter().position(|&c| c == 0).unwrap_or(buf.len()) }

/// `int cht_CheckCheat(cheatseq_t *cht, char key)` — feed one keystroke
/// into the matcher; returns 1 (true) if the cheat was successfully
/// entered, 0 otherwise.
///
/// Vanilla quirk carried verbatim (`m_cheat.c:39-43`): if the declared
/// `sequence_len` exceeds the real NUL-terminated sequence while the
/// cheat carries parameters, the matcher never fires. This is
/// upstream's documented intended behaviour, not a bug emulation --
/// never "fix".
///
/// # Safety
/// - `cht` must point to a valid, initialised `cheatseq_t` owned by the
///   caller for the duration of the call.
#[doc(alias = "cht_CheckCheat")]
pub extern "C" fn check_cheat(cht: *mut cheatseq_t, key: c_char) -> c_int
{
    let cht = unsafe { &mut *cht };

    let seq_len = raw_strlen(&cht.sequence);

    // If we make a short sequence on a cheat with parameters, this
    // will not work in vanilla doom. Behave the same.
    if cht.parameter_chars > 0 && seq_len < cht.sequence_len { return 0; /* false */ }

    if cht.chars_read < seq_len
    {
        // Still reading characters from the cheat code and verifying.
        // Reset back to the beginning if a key is wrong.
        if key == cht.sequence[cht.chars_read] { cht.chars_read += 1; }
        else { cht.chars_read = 0; }
        cht.param_chars_read = 0;
    }
    else if cht.param_chars_read < cht.parameter_chars
    {
        // We have passed the end of the cheat sequence and are
        // entering parameters now.
        cht.parameter_buf[cht.param_chars_read as usize] = key;
        cht.param_chars_read += 1;
    }

    if cht.chars_read >= seq_len && cht.param_chars_read >= cht.parameter_chars
    {
        cht.chars_read = 0;
        cht.param_chars_read = 0;
        return 1; // true
    }

    0 // false
}

/// `void cht_GetParam(cheatseq_t *cht, char *buffer)` — copy the
/// captured parameter characters into `buffer`.
///
/// Trusts the caller's `parameter_chars` cursor when slicing the
/// capture buffer -- unchanged behaviour; do not "fix" during a
/// refactor.
///
/// # Safety
/// - `cht` must point to a valid `cheatseq_t`.
/// - `buffer` must be writable for at least `cht.parameter_chars`
///   bytes.
#[doc(alias = "cht_GetParam")]
pub extern "C" fn get_param(cht: *mut cheatseq_t, buffer: *mut c_char)
{
    let cht = unsafe { &*cht };
    let buf = unsafe { std::slice::from_raw_parts_mut(buffer, cht.parameter_chars as usize) };
    let param_chars = cht.parameter_chars as usize;
    buf[..param_chars].copy_from_slice(&cht.parameter_buf[..param_chars]);
}

/// The pre-move baseline test module (six vectors plus the
/// short-sequence quirk vector), re-pointed to the graduated names
/// after the split -- same vectors, same results (F10 wave B5).
#[cfg(test)]
mod tests
{
    use crate::doom::m_cheat::types::{MAX_CHEAT_LEN, MAX_CHEAT_PARAMS};
    use super::*;

    /// Build a `cheatseq_t` from a Rust string for use in the tests below.
    /// `sequence_len` is set to the string length so the
    /// short-sequence-on-parameter-cheat guard does not fire.
    fn make_cheat(seq: &str, param_chars: c_int) -> cheatseq_t
    {
        let mut sequence = [0i8; MAX_CHEAT_LEN];
        let bytes = seq.as_bytes();
        for(i, &b) in bytes.iter().enumerate() { sequence[i] = b as c_char; }
        cheatseq_t
        {
            sequence,
            sequence_len: bytes.len(),
            parameter_chars: param_chars,
            chars_read: 0,
            param_chars_read: 0,
            parameter_buf: [0; MAX_CHEAT_PARAMS],
        }
    }

    /// Five correct keystrokes for a 5-character no-parameter cheat
    /// should report success only on the last keystroke.
    #[test]
    fn simple_cheat_five_chars()
    {
        let mut cheat = make_cheat("IDKFA", 0);
        assert_eq!(check_cheat(&mut cheat, b'I' as c_char), 0);
        assert_eq!(check_cheat(&mut cheat, b'D' as c_char), 0);
        assert_eq!(check_cheat(&mut cheat, b'K' as c_char), 0);
        assert_eq!(check_cheat(&mut cheat, b'F' as c_char), 0);
        assert_eq!(check_cheat(&mut cheat, b'A' as c_char), 1); // matched!
    }

    /// Same flow as `simple_cheat_five_chars` but with a 4-character
    /// cheat, confirming the implementation isn't length-specific.
    #[test]
    fn simple_cheat_four_chars()
    {
        let mut cheat = make_cheat("IDFA", 0);
        assert_eq!(check_cheat(&mut cheat, b'I' as c_char), 0);
        assert_eq!(check_cheat(&mut cheat, b'D' as c_char), 0);
        assert_eq!(check_cheat(&mut cheat, b'F' as c_char), 0);
        assert_eq!(check_cheat(&mut cheat, b'A' as c_char), 1); // matched!
    }

    /// A wrong keystroke must reset `chars_read` to 0 so the user has to
    /// restart the cheat from scratch.
    #[test]
    fn wrong_char_resets()
    {
        let mut cheat = make_cheat("IDFA", 0);
        assert_eq!(check_cheat(&mut cheat, b'I' as c_char), 0);
        assert_eq!(check_cheat(&mut cheat, b'X' as c_char), 0); // wrong, resets
        assert_eq!(check_cheat(&mut cheat, b'I' as c_char), 0); // restart
    }

    /// Cheats with `parameter_chars > 0` only fire once the parameter
    /// characters have also been entered.
    #[test]
    fn cheat_with_params()
    {
        let mut cheat = make_cheat("IDKFA", 1);
        for ch in b"IDKFA"
        {
            let result = check_cheat(&mut cheat, *ch as c_char);
            // The cheat has 1 param char, so after typing the sequence
            // we need to also type a param character
            if *ch == b'A' { assert_eq!(result, 0); /* need param still */ }
            else { assert_eq!(result, 0); }
        }
        // Now type a param char
        assert_eq!(check_cheat(&mut cheat, b'7' as c_char), 1); // matched!
    }

    /// After a successful cheat with parameters, `get_param` must
    /// copy the captured bytes into the caller-provided buffer.
    #[test]
    fn get_param_copies_buffer()
    {
        let mut cheat = make_cheat("IDKFA", 1);
        for ch in b"IDKFA" { check_cheat(&mut cheat, *ch as c_char); }
        check_cheat(&mut cheat, b'7' as c_char); // completes the cheat

        let mut param_buf = [0i8; MAX_CHEAT_PARAMS];
        get_param(&mut cheat, param_buf.as_mut_ptr());
        assert_eq!(param_buf[0], b'7' as c_char);
    }

    /// Runtime check that the 64-bit `cheatseq_t` layout still totals
    /// 72 bytes (matches the layout comment on the struct).
    #[test]
    #[cfg(target_pointer_width = "64")]
    fn struct_size_assertion() { assert_eq!(size_of::<cheatseq_t>(), 72); }

    /// Runtime check that the 32-bit `cheatseq_t` layout still totals
    /// 52 bytes (matches the layout comment on the struct).
    #[test]
    #[cfg(target_pointer_width = "32")]
    fn struct_size_assertion() { assert_eq!(std::mem::size_of::<cheatseq_t>(), 52); }

    /// Vanilla quirk (m_cheat.c:39-43, Rust guard at the
    /// `parameter_chars > 0` branch): a sequence SHORTER than the
    /// declared `sequence_len` on a parameter-bearing cheat NEVER
    /// fires -- the checker bails before matching anything. Upstream
    /// documents this as intended behaviour, not a bug emulation.
    /// Baseline vector written pre-move (F10 wave B5).
    #[test]
    fn short_sequence_on_parameter_cheat_never_fires()
    {
        // Declared length one past the real 5-char sequence: the
        // guard `seq_len < sequence_len` trips on every keystroke.
        let mut cheat = make_cheat("IDKFA", 2);
        cheat.sequence_len = 6;
        for ch in b"IDKFA" { assert_eq!(check_cheat(&mut cheat, *ch as c_char), 0); }
        // Even the trailing parameter characters cannot rescue it.
        assert_eq!(check_cheat(&mut cheat, b'0' as c_char), 0);
        assert_eq!(check_cheat(&mut cheat, b'1' as c_char), 0);

        // Positive control: declared length EQUAL to the real
        // sequence fires normally once the parameters are typed.
        let mut control = make_cheat("IDKFA", 2);
        for ch in b"IDKFA" { assert_eq!(check_cheat(&mut control, *ch as c_char), 0); }
        assert_eq!(check_cheat(&mut control, b'0' as c_char), 0);
        assert_eq!(check_cheat(&mut control, b'1' as c_char), 1);
    }
}

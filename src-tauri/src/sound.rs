//! Semnal sonor pentru alerte. Pe Windows folosește plăcuța de sunet (`Beep`), deci se aude
//! indiferent de schema de sunete aleasă de utilizator.

/// Redă un semnal de trei sunete, fără să blocheze apelantul.
pub fn alarm() {
    std::thread::spawn(play_blocking);
}

#[cfg(windows)]
fn play_blocking() {
    use windows_sys::Win32::System::Diagnostics::Debug::Beep;
    for _ in 0..3 {
        // SAFETY: `Beep` nu primește pointeri; doar frecvență (Hz) și durată (ms).
        unsafe {
            Beep(988, 220);
            Beep(784, 220);
            Beep(988, 320);
        }
        std::thread::sleep(std::time::Duration::from_millis(350));
    }
}

#[cfg(target_os = "macos")]
fn play_blocking() {
    for _ in 0..2 {
        let _ = std::process::Command::new("afplay").arg("/System/Library/Sounds/Sosumi.aiff").status();
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn play_blocking() {}

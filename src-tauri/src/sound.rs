//! Semnal sonor pentru alerte. Pe Windows folosește plăcuța de sunet (`Beep`), deci se aude
//! indiferent de schema de sunete aleasă de utilizator.

/// Redă semnalul sonor, fără să blocheze apelantul. Alerta `serious` sună mai lung, mai ascuțit și mai insistent.
pub fn alarm(serious: bool) {
    std::thread::spawn(move || play_blocking(serious));
}

#[cfg(windows)]
fn play_blocking(serious: bool) {
    use windows_sys::Win32::System::Diagnostics::Debug::Beep;
    let rounds = if serious { 8 } else { 3 };
    for _ in 0..rounds {
        // SAFETY: `Beep` nu primește pointeri; doar frecvență (Hz) și durată (ms).
        unsafe {
            if serious {
                Beep(1568, 180);
                Beep(988, 180);
                Beep(1568, 180);
                Beep(988, 180);
            } else {
                Beep(988, 220);
                Beep(784, 220);
                Beep(988, 320);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(if serious { 250 } else { 350 }));
    }
}

#[cfg(target_os = "macos")]
fn play_blocking(serious: bool) {
    for _ in 0..if serious { 5 } else { 2 } {
        let _ = std::process::Command::new("afplay").arg("/System/Library/Sounds/Sosumi.aiff").status();
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn play_blocking(_serious: bool) {}

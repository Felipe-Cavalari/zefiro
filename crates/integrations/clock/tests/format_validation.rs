use zefiro_integration_clock::format::{ClockText, PtBrFallback};

#[test]
fn date_formats_accept_system_default_and_reject_embedded_nul() {
    assert!(PtBrFallback.validate_format("").is_ok());
    assert!(PtBrFallback.validate_format("ddd d MMM").is_ok());
    assert!(PtBrFallback.validate_format("ddd\0MMMM").is_err());
    #[cfg(windows)]
    {
        let clock = zefiro_integration_clock::windows::WindowsClock;
        assert!(clock.validate_format("").is_ok());
        assert!(clock.validate_format("dddd, d 'de' MMMM").is_ok());
        assert!(clock.validate_format("ddd\0MMMM").is_err());
    }
}

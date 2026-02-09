use arboard::Clipboard;

pub struct ClipboardHandler {
    clipboard: Option<Clipboard>,
}

impl Default for ClipboardHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardHandler {
    pub fn new() -> Self {
        let clipboard = Clipboard::new().ok();
        Self { clipboard }
    }

    pub fn set_text(&mut self, text: String) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(clipboard) = &mut self.clipboard {
            clipboard.set_text(text)?;
            Ok(())
        } else {
            Err("Clipboard not available".into())
        }
    }
}

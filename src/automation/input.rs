// Mouse + keyboard simulation via the `enigo` crate.
//
// All operations are gated upstream by `Autonomy::allow_input_control`;
// this module assumes the caller has already confirmed permission.

use anyhow::{Context, Result};
use enigo::{Coordinate, Direction, Enigo, Keyboard, Mouse, Settings};

pub struct Input {
    enigo: Enigo,
}

impl Input {
    pub fn new() -> Result<Self> {
        let enigo = Enigo::new(&Settings::default()).context("init enigo")?;
        Ok(Self { enigo })
    }

    pub fn move_mouse(&mut self, x: i32, y: i32) -> Result<()> {
        self.enigo
            .move_mouse(x, y, Coordinate::Abs)
            .context("move_mouse")?;
        Ok(())
    }

    pub fn click_left(&mut self) -> Result<()> {
        self.enigo
            .button(enigo::Button::Left, Direction::Click)
            .context("left click")?;
        Ok(())
    }

    pub fn click_right(&mut self) -> Result<()> {
        self.enigo
            .button(enigo::Button::Right, Direction::Click)
            .context("right click")?;
        Ok(())
    }

    pub fn type_text(&mut self, s: &str) -> Result<()> {
        self.enigo.text(s).context("type_text")?;
        Ok(())
    }
}

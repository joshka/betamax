//! Stateful cell input translated through Ghostty's negotiated mouse encoder.

use libghostty_vt::key::Mods;
use libghostty_vt::{mouse, Terminal};
use miette::miette;

use super::engine::TerminalGrid;
use crate::tape::{KeyModifiers, MouseButton, MouseEvent, ScrollDirection};
use crate::Result;

pub(super) struct MouseInput {
    encoder: mouse::Encoder<'static>,
    grid: TerminalGrid,
    cell_width: u32,
    cell_height: u32,
    position: (u16, u16),
    pressed: Vec<MouseButton>,
}

impl MouseInput {
    pub(super) fn new(grid: TerminalGrid, cell_width: u32, cell_height: u32) -> Result<Self> {
        let mut encoder = mouse::Encoder::new().map_err(mouse_error)?;
        // Exclude presentation padding. Positions and geometry use the terminal grid itself.
        encoder.set_size(mouse::EncoderSize {
            screen_width: u32::from(grid.columns) * cell_width,
            screen_height: u32::from(grid.rows) * cell_height,
            cell_width,
            cell_height,
            padding_top: 0,
            padding_bottom: 0,
            padding_left: 0,
            padding_right: 0,
        });
        Ok(Self {
            encoder,
            grid,
            cell_width,
            cell_height,
            position: (0, 0),
            pressed: Vec::new(),
        })
    }

    pub(super) fn resize(&mut self, grid: TerminalGrid) {
        self.grid = grid;
        self.position.0 = self.position.0.min(grid.columns - 1);
        self.position.1 = self.position.1.min(grid.rows - 1);
        self.encoder.set_size(mouse::EncoderSize {
            screen_width: u32::from(grid.columns) * self.cell_width,
            screen_height: u32::from(grid.rows) * self.cell_height,
            cell_width: self.cell_width,
            cell_height: self.cell_height,
            padding_top: 0,
            padding_bottom: 0,
            padding_left: 0,
            padding_right: 0,
        });
    }

    pub(super) fn encode(
        &mut self,
        terminal: &Terminal<'_, '_>,
        event: MouseEvent,
        modifiers: KeyModifiers,
    ) -> Result<Vec<u8>> {
        let (action, button, count) = match event {
            MouseEvent::Move { column, row } => {
                if column >= self.grid.columns || row >= self.grid.rows {
                    return Err(miette!(
                        "mouse cell ({column}, {row}) is outside {} columns by {} rows",
                        self.grid.columns,
                        self.grid.rows
                    )
                    .into());
                }
                self.position = (column, row);
                (
                    mouse::Action::Motion,
                    self.pressed.last().copied().map(button),
                    1,
                )
            }
            MouseEvent::Down(value) => {
                if self.pressed.contains(&value) {
                    return Err(miette!("mouse button {value:?} is already pressed").into());
                }
                self.pressed.push(value);
                (mouse::Action::Press, Some(button(value)), 1)
            }
            MouseEvent::Up(value) => {
                let Some(index) = self.pressed.iter().position(|pressed| *pressed == value) else {
                    return Err(miette!("mouse button {value:?} is not pressed").into());
                };
                self.pressed.remove(index);
                (mouse::Action::Release, Some(button(value)), 1)
            }
            MouseEvent::Scroll { direction, count } => {
                if count == 0 {
                    return Err(miette!("mouse wheel tick count must be positive").into());
                }
                let button = match direction {
                    ScrollDirection::Up => mouse::Button::Four,
                    ScrollDirection::Down => mouse::Button::Five,
                    ScrollDirection::Left => mouse::Button::Six,
                    ScrollDirection::Right => mouse::Button::Seven,
                };
                (mouse::Action::Press, Some(button), count)
            }
        };
        let mut mods = Mods::empty();
        if modifiers.shift {
            mods |= Mods::SHIFT;
        }
        if modifiers.alt {
            mods |= Mods::ALT;
        }
        if modifiers.ctrl {
            mods |= Mods::CTRL;
        }
        let mut input = mouse::Event::new().map_err(mouse_error)?;
        input.set_action(action);
        input.set_button(button);
        input.set_mods(mods);
        input.set_position(mouse::Position {
            x: f32::from(self.position.0) * self.cell_width as f32 + self.cell_width as f32 / 2.0,
            y: f32::from(self.position.1) * self.cell_height as f32 + self.cell_height as f32 / 2.0,
        });
        self.encoder.set_options_from_terminal(terminal);
        self.encoder
            .set_any_button_pressed(!self.pressed.is_empty());
        let mut bytes = Vec::new();
        for _ in 0..count {
            self.encoder
                .encode_to_vec(&input, &mut bytes)
                .map_err(mouse_error)?;
        }
        Ok(bytes)
    }
}

fn button(value: MouseButton) -> mouse::Button {
    match value {
        MouseButton::Left => mouse::Button::Left,
        MouseButton::Middle => mouse::Button::Middle,
        MouseButton::Right => mouse::Button::Right,
    }
}

fn mouse_error(error: libghostty_vt::Error) -> miette::Report {
    miette!("failed to encode mouse input: {error}")
}

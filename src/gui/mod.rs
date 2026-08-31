//! Habillage de la fenetre : la coque dessinee, le papier pose dessous, et la
//! seule fenetre modale qui reste.

pub mod fond;
pub mod shell;
pub mod widgets;

pub use shell::{Palette, ShellColor};
pub use widgets::{ActiveModal, GuiWidgets};

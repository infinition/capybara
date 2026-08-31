//! Icone de zone de notification et son menu minimal.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionTray {
    Afficher,
    Quitter,
}

pub struct Tray {
    _icone: TrayIcon,
    action: Arc<AtomicU8>,
}

impl Tray {
    pub fn new(ctx: &egui::Context) -> Result<Self, String> {
        let image = image::load_from_memory(include_bytes!("../assets/icone.png"))
            .map_err(|e| e.to_string())?
            .into_rgba8();
        let (largeur, hauteur) = image.dimensions();
        let icone = Icon::from_rgba(image.into_raw(), largeur, hauteur)
            .map_err(|e| e.to_string())?;

        let menu = Menu::new();
        let afficher = MenuItem::new("Afficher", true, None);
        let quitter = MenuItem::new("Quitter", true, None);
        menu.append_items(&[&afficher, &quitter])
            .map_err(|e| e.to_string())?;

        // Un simple receveur consulte depuis update ne suffit pas quand la
        // fenetre est cachee : certains bureaux suspendent alors ses images.
        // Le gestionnaire reveille explicitement la boucle graphique.
        let action = Arc::new(AtomicU8::new(0));
        let action_evenement = Arc::clone(&action);
        let afficher_id = afficher.id().clone();
        let quitter_id = quitter.id().clone();
        let contexte = ctx.clone();
        MenuEvent::set_event_handler(Some(move |evenement: MenuEvent| {
            let valeur = if evenement.id == afficher_id {
                1
            } else if evenement.id == quitter_id {
                2
            } else {
                0
            };
            if valeur != 0 {
                action_evenement.store(valeur, Ordering::Release);
                contexte.request_repaint();
            }
        }));

        let icone = TrayIconBuilder::new()
            .with_tooltip("Capybara")
            .with_icon(icone)
            .with_menu(Box::new(menu))
            .build()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            _icone: icone,
            action,
        })
    }

    pub fn action(&self) -> Option<ActionTray> {
        match self.action.swap(0, Ordering::AcqRel) {
            1 => Some(ActionTray::Afficher),
            2 => Some(ActionTray::Quitter),
            _ => None,
        }
    }
}

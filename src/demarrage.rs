//! Lancement automatique a l'ouverture de la session.
//!
//! Chaque systeme a son mecanisme, et aucun n'a besoin de dependance : une
//! valeur dans la base de registres sous Windows, un fichier de service sous
//! Mac, un raccourci de demarrage sous Linux.
//!
//! L'etat n'est pas retenu dans les reglages du logiciel mais relu sur le
//! systeme a chaque affichage. C'est lui qui fait foi : l'utilisateur peut
//! avoir retire l'entree par ailleurs, et une case a cocher qui ment est pire
//! que pas de case du tout.

/// Nom sous lequel l'entree est posee.
const NOM: &str = "Capybara";

/// Chemin de l'executable, entre guillemets s'il contient une espace.
fn executable() -> Result<std::path::PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
mod systeme {
    use super::{executable, NOM};
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    /// Sans ce drapeau, chaque appel fait clignoter une fenetre de console.
    const SANS_FENETRE: u32 = 0x0800_0000;
    const CLE: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

    fn reg(args: &[&str]) -> Result<std::process::Output, String> {
        Command::new("reg")
            .args(args)
            .creation_flags(SANS_FENETRE)
            .output()
            .map_err(|e| e.to_string())
    }

    pub fn actif() -> bool {
        // L'outil du systeme plutot qu'une bibliotheque : il est present
        // partout, et une dependance de plus pour lire une seule valeur n'en
        // vaut pas la peine.
        reg(&["query", CLE, "/v", NOM]).map(|s| s.status.success()).unwrap_or(false)
    }

    pub fn regler(voulu: bool) -> Result<(), String> {
        if voulu {
            let chemin = executable()?;
            let valeur = format!("\"{}\"", chemin.display());
            let sortie = reg(&["add", CLE, "/v", NOM, "/t", "REG_SZ", "/d", &valeur, "/f"])?;
            if !sortie.status.success() {
                return Err(String::from_utf8_lossy(&sortie.stderr).trim().to_string());
            }
        } else {
            // Une entree absente n'est pas une erreur : on voulait qu'elle le
            // soit, elle l'est.
            let _ = reg(&["delete", CLE, "/v", NOM, "/f"]);
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod systeme {
    use super::{executable, NOM};
    use std::path::PathBuf;

    fn fichier() -> Option<PathBuf> {
        let maison = std::env::var_os("HOME")?;
        Some(
            PathBuf::from(maison)
                .join("Library")
                .join("LaunchAgents")
                .join("com.infinition.capybara.plist"),
        )
    }

    pub fn actif() -> bool {
        fichier().map(|f| f.is_file()).unwrap_or(false)
    }

    pub fn regler(voulu: bool) -> Result<(), String> {
        let Some(f) = fichier() else {
            return Err("dossier personnel introuvable".to_string());
        };
        if !voulu {
            let _ = std::fs::remove_file(&f);
            return Ok(());
        }
        let chemin = executable()?;
        if let Some(parent) = f.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let plist = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
             \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n<dict>\n\
             \t<key>Label</key>\n\t<string>com.infinition.capybara</string>\n\
             \t<key>ProgramArguments</key>\n\t<array>\n\t\t<string>{}</string>\n\t</array>\n\
             \t<key>RunAtLoad</key>\n\t<true/>\n\
             </dict>\n</plist>\n",
            chemin.display()
        );
        let _ = NOM;
        std::fs::write(&f, plist).map_err(|e| e.to_string())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod systeme {
    use super::{executable, NOM};
    use std::path::PathBuf;

    /// Emplacement decrit par la specification des entrees de bureau.
    fn fichier() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("autostart").join("capybara.desktop"))
    }

    pub fn actif() -> bool {
        fichier().map(|f| f.is_file()).unwrap_or(false)
    }

    pub fn regler(voulu: bool) -> Result<(), String> {
        let Some(f) = fichier() else {
            return Err("dossier de configuration introuvable".to_string());
        };
        if !voulu {
            let _ = std::fs::remove_file(&f);
            return Ok(());
        }
        let chemin = executable()?;
        if let Some(parent) = f.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let entree = format!(
            "[Desktop Entry]\nType=Application\nName={}\nExec={}\n\
             Icon=capybara\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
            NOM,
            chemin.display()
        );
        std::fs::write(&f, entree).map_err(|e| e.to_string())
    }
}

#[cfg(not(any(windows, unix)))]
mod systeme {
    pub fn actif() -> bool {
        false
    }
    pub fn regler(_voulu: bool) -> Result<(), String> {
        Err("systeme non pris en charge".to_string())
    }
}

/// Vrai si le logiciel est declare au demarrage de la session.
pub fn actif() -> bool {
    systeme::actif()
}

/// Declare ou retire le logiciel du demarrage de la session.
pub fn regler(voulu: bool) -> Result<(), String> {
    systeme::regler(voulu)
}

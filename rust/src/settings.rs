//! Réglages mémorisés d'une session à l'autre (fichier texte `clé=valeur`, local).

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub annexe: String,
    pub template: String,
    pub start: String,
    pub total: String,
    pub size: String,
    pub white_fill: bool,
    pub fx: f32,
    pub fy: f32,
    pub margin: f32,
    pub side_w: f32,
    /// 0 = système, 1 = clair, 2 = sombre
    pub theme: u8,
    pub ui_scale: f32,
    pub thumbs: bool,
    pub recents: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            annexe: "1".into(),
            template: "Annexe {annexe} Folio {n}/{total}".into(),
            start: "1".into(),
            total: String::new(),
            size: "14".into(),
            white_fill: true,
            fx: 0.55,
            fy: 0.03,
            margin: 20.0,
            side_w: 290.0,
            theme: 0,
            ui_scale: 1.0,
            thumbs: true,
            recents: Vec::new(),
        }
    }
}

impl Settings {
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let mut kv = |k: &str, v: String| {
            s.push_str(k);
            s.push('=');
            s.push_str(&v.replace(['\n', '\r'], " "));
            s.push('\n');
        };
        kv("annexe", self.annexe.clone());
        kv("template", self.template.clone());
        kv("start", self.start.clone());
        kv("total", self.total.clone());
        kv("size", self.size.clone());
        kv("white_fill", self.white_fill.to_string());
        kv("fx", self.fx.to_string());
        kv("fy", self.fy.to_string());
        kv("margin", self.margin.to_string());
        kv("side_w", self.side_w.to_string());
        kv("theme", self.theme.to_string());
        kv("ui_scale", self.ui_scale.to_string());
        kv("thumbs", self.thumbs.to_string());
        for r in &self.recents {
            kv("recent", r.clone());
        }
        s
    }

    pub fn from_text(text: &str) -> Settings {
        let mut s = Settings::default();
        let mut recents = Vec::new();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k {
                "annexe" => s.annexe = v.into(),
                "template" if !v.trim().is_empty() => s.template = v.into(),
                "start" => s.start = v.into(),
                "total" => s.total = v.into(),
                "size" => s.size = v.into(),
                "white_fill" => s.white_fill = v == "true",
                "fx" => s.fx = v.parse().unwrap_or(s.fx).clamp(0.0, 1.0),
                "fy" => s.fy = v.parse().unwrap_or(s.fy).clamp(0.0, 1.0),
                "margin" => s.margin = v.parse().unwrap_or(s.margin).clamp(0.0, 200.0),
                "side_w" => s.side_w = v.parse().unwrap_or(s.side_w).clamp(220.0, 640.0),
                "theme" => s.theme = v.parse().unwrap_or(0).min(2),
                "ui_scale" => s.ui_scale = v.parse::<f32>().unwrap_or(1.0).clamp(0.8, 2.0),
                "thumbs" => s.thumbs = v == "true",
                "recent" if !v.is_empty() => recents.push(v.to_string()),
                _ => {}
            }
        }
        recents.truncate(8);
        s.recents = recents;
        s
    }
}

fn config_file(dir: &str) -> Option<PathBuf> {
    let base = if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA")?)
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support")
    } else {
        match std::env::var_os("XDG_CONFIG_HOME") {
            Some(x) if !x.is_empty() => PathBuf::from(x),
            _ => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
        }
    };
    Some(base.join(dir).join("settings.txt"))
}

/// Reprend les réglages de l'ancien dossier « AnnexePDF » si « Annexio » n'existe pas encore.
pub fn load() -> Settings {
    ["Annexio", "AnnexePDF"]
        .iter()
        .filter_map(|d| config_file(d))
        .find_map(|p| std::fs::read_to_string(p).ok())
        .map(|t| Settings::from_text(&t))
        .unwrap_or_default()
}

pub fn save(s: &Settings) {
    if let Some(p) = config_file("Annexio") {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, s.to_text());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour() {
        let mut s = Settings::default();
        s.annexe = "B".into();
        s.template = "Annexe {annexe} = page {n}".into(); // « = » dans la valeur
        s.theme = 2;
        s.ui_scale = 1.25;
        s.recents = vec!["C:\\a b\\c.pdf".into(), "/tmp/d.pdf".into()];
        assert_eq!(Settings::from_text(&s.to_text()), s);
    }

    #[test]
    fn valeurs_invalides_ignorees() {
        let s = Settings::from_text("fx=abc\ntheme=9\nui_scale=50\nn'importe quoi\nsize=20\n");
        assert_eq!(s.fx, Settings::default().fx);
        assert_eq!(s.theme, 2);
        assert_eq!(s.ui_scale, 2.0);
        assert_eq!(s.size, "20");
    }
}

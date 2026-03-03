#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    ZhCn,
    En,
}

impl Language {
    pub fn detect_default() -> Self {
        let raw = std::env::var("LANG").unwrap_or_default();
        let normalized = raw.to_ascii_lowercase();
        if normalized.starts_with("zh") {
            Self::ZhCn
        } else {
            Self::En
        }
    }

    pub fn to_index(self) -> u32 {
        match self {
            Self::ZhCn => 0,
            Self::En => 1,
        }
    }

    pub fn from_index(index: u32) -> Self {
        match index {
            0 => Self::ZhCn,
            _ => Self::En,
        }
    }
}

pub fn pick(lang: Language, zh: &'static str, en: &'static str) -> &'static str {
    match lang {
        Language::ZhCn => zh,
        Language::En => en,
    }
}

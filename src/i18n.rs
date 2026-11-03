//! Fixed label strings written by the crate itself (DESIGN section 7). Never translates data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    Fr,
}

#[derive(Debug, Clone, Copy)]
pub enum Key {
    HeadlinePosition,
    HeadlinePersonalStatement,
    AchievementProjects,
    SkillsPrefix,
    Present,
    UrlPrefix,
    GenderM,
    GenderF,
    SourceComment,
    Omitted,
}

impl Locale {
    pub fn parse(s: &str) -> Option<Locale> {
        match s.trim().to_ascii_lowercase().as_str() {
            "en" => Some(Locale::En),
            "fr" => Some(Locale::Fr),
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Fr => "fr",
        }
    }

    pub fn t(self, k: Key) -> &'static str {
        let fr = self == Locale::Fr;
        match k {
            Key::HeadlinePosition => {
                if fr {
                    "Poste"
                } else {
                    "Position"
                }
            }
            Key::HeadlinePersonalStatement => {
                if fr {
                    "Profil"
                } else {
                    "Personal statement"
                }
            }
            Key::AchievementProjects => {
                if fr {
                    "Projets"
                } else {
                    "Projects"
                }
            }
            Key::SkillsPrefix => {
                if fr {
                    "Compétences :"
                } else {
                    "Skills:"
                }
            }
            Key::Present => {
                if fr {
                    "aujourd'hui"
                } else {
                    "present"
                }
            }
            Key::UrlPrefix => {
                if fr {
                    "URL :"
                } else {
                    "URL:"
                }
            }
            Key::GenderM => {
                if fr {
                    "Homme"
                } else {
                    "Male"
                }
            }
            Key::GenderF => {
                if fr {
                    "Femme"
                } else {
                    "Female"
                }
            }
            Key::Omitted => {
                if fr {
                    "[Suite omise : limite de longueur]"
                } else {
                    "[Remainder omitted: length limit]"
                }
            }
            Key::SourceComment => {
                if fr {
                    "Source :"
                } else {
                    "Source:"
                }
            }
        }
    }
}

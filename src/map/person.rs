//! person -> Identification and Headline (DESIGN section 4.2).
use super::{Report, CAP_WEBSITES, LIM_HEADLINE, LIM_LINE};
use crate::codesets::{COUNTRIES, NATIONALITIES, TELEPHONE_USES};
use crate::error::short;
use crate::i18n::Key;
use crate::input::Person;
use crate::model::headline::Headline;
use crate::model::identification::*;
use crate::model::label::Label;
use crate::options::DatePrecision;
use crate::options::SummaryPlacement;
use crate::text::{clean_uri, valid_email};

/// Split on the last space: "Jean de La Fontaine" gives FirstName "Jean de La", Surname "Fontaine".
pub fn split_name(name: &str) -> (Option<String>, Option<String>) {
    let n = name.split_whitespace().collect::<Vec<_>>().join(" ");
    match n.rsplit_once(' ') {
        Some((a, b)) => (Some(a.to_string()), Some(b.to_string())),
        None if n.is_empty() => (None, None),
        None => (Some(n), None),
    }
}

fn website_use(label: &str) -> Label {
    let l = label.to_lowercase();
    let code = match l.as_str() {
        "github" | "cv" => "portfolio",
        "blog" => "blog",
        _ => "personal", // includes linkedin and personal
    };
    Label::new(Some(code.to_string()), Some(label.to_string()))
}

pub fn identification(p: &Person, rep: &mut Report) -> Option<Identification> {
    let given = rep.opt_line("person.givenName", &p.given_name, LIM_LINE);
    let family = rep.opt_line("person.familyName", &p.family_name, LIM_LINE);
    let name = if given.is_some() || family.is_some() {
        Some(PersonName {
            first_name: given,
            surname: family,
        })
    } else {
        rep.opt_line("person.name", &p.name, LIM_LINE).map(|n| {
            let (f, s) = split_name(&n);
            PersonName {
                first_name: f,
                surname: s,
            }
        })
    };

    // Email
    let email = rep.opt_line("person.email", &p.email, 254).and_then(|e| {
        let e = if e.to_ascii_lowercase().starts_with("mailto:") {
            e.get(7..).unwrap_or("").trim().to_string()
        } else {
            e
        };
        if valid_email(&e) {
            Some(e)
        } else {
            rep.warn(
                "email_invalid",
                "person.email",
                format!("not a usable email address: \"{}\"", short(&e)),
            );
            None
        }
    });

    // Address
    let address = p.address.as_ref().map(|a| {
        let country_label = rep.opt_line("person.address.country", &a.country, LIM_LINE);
        let code = a
            .country_code
            .as_deref()
            .map(|c| c.trim().to_ascii_uppercase())
            .filter(|c| !c.is_empty());
        let code = match code {
            Some(c) if c.len() == 2 && COUNTRIES.binary_search(&c.as_str()).is_ok() => Some(c),
            Some(c) => {
                rep.warn(
                    "country_code_unknown",
                    "person.address.countryCode",
                    format!("not an Europass country code: \"{}\"", short(&c)),
                );
                None
            }
            None => None,
        };
        Address {
            line1: rep.opt_line("person.address.line1", &a.line1, LIM_LINE),
            line2: rep.opt_line("person.address.line2", &a.line2, LIM_LINE),
            postal_code: rep.opt_line("person.address.postalCode", &a.postal_code, LIM_LINE),
            municipality: rep.opt_line("person.address.municipality", &a.municipality, LIM_LINE),
            country: Some(Label::new(code, country_label)).filter(|l| !l.is_empty()),
        }
    });

    // Phones
    let mut phones = Vec::new();
    for (i, ph) in p.phones.iter().enumerate() {
        let path = format!("person.phones[{i}]");
        let Some(number) = rep.line(&format!("{path}.number"), &ph.number, LIM_LINE) else {
            rep.warn("empty_entry", &path, "phone without number skipped".into());
            continue;
        };
        let use_code = ph.use_.as_deref().and_then(|u| {
            let u = u.trim().to_ascii_lowercase();
            if TELEPHONE_USES.binary_search(&u.as_str()).is_ok() {
                Some(Label::new(Some(u), None))
            } else {
                rep.warn(
                    "enum_unknown",
                    &format!("{path}.use"),
                    format!("unknown telephone use: \"{}\"", short(&u)),
                );
                None
            }
        });
        phones.push(Telephone { number, use_code });
    }

    // Websites
    let mut websites = Vec::new();
    let n = rep.cap("person.websites", &p.websites, CAP_WEBSITES);
    for (i, w) in p.websites.iter().take(n).enumerate() {
        let path = format!("person.websites[{i}]");
        let Some(url) = rep.line(&format!("{path}.url"), &w.url, 2000) else {
            rep.warn("empty_entry", &path, "website without url skipped".into());
            continue;
        };
        let Some(url) = clean_uri(&url) else {
            rep.warn(
                "url_invalid",
                &format!("{path}.url"),
                format!("not an http, https or ftp URL: \"{}\"", short(&url)),
            );
            continue;
        };
        let use_ = rep
            .opt_line(&format!("{path}.label"), &w.label, LIM_LINE)
            .map(|l| website_use(&l));
        websites.push(Website { url, use_ });
    }

    let contact = ContactInfo {
        address,
        email,
        phones,
        websites,
    };
    let contact = Some(contact).filter(|c| c.to_node().is_some());

    // Demographics (full precision, a birth date is what was given)
    let birthdate =
        p.birth_date
            .as_ref()
            .and_then(|b| match rep.date("person.birthDate", &Some(b.clone())) {
                super::DateField::Ok(d) => Some(d.to_attrs(DatePrecision::Day)),
                _ => None,
            });
    let gender = p
        .gender
        .as_deref()
        .and_then(|g| match g.trim().to_ascii_uppercase().as_str() {
            "M" => Some(Label::new(
                Some("M".into()),
                Some(rep.locale.t(Key::GenderM).into()),
            )),
            "F" => Some(Label::new(
                Some("F".into()),
                Some(rep.locale.t(Key::GenderF).into()),
            )),
            "" => None,
            other => {
                rep.warn(
                    "enum_unknown",
                    "person.gender",
                    format!("gender must be M or F, got \"{}\"", short(other)),
                );
                None
            }
        });
    let mut nationalities = Vec::new();
    for (i, nat) in p.nationalities.iter().enumerate() {
        let path = format!("person.nationalities[{i}]");
        let code = nat.code.trim().to_ascii_uppercase();
        if NATIONALITIES.binary_search(&code.as_str()).is_err() {
            rep.warn(
                "country_code_unknown",
                &format!("{path}.code"),
                format!("not an Europass nationality code: \"{}\"", short(&code)),
            );
            continue;
        }
        let label = rep.opt_line(&format!("{path}.label"), &nat.label, LIM_LINE);
        nationalities.push(Label::new(Some(code), label));
    }
    let demographics = Demographics {
        birthdate,
        gender,
        nationalities,
    };
    let demographics = Some(demographics).filter(|d| d.to_node().is_some());

    let id = Identification {
        name,
        contact,
        demographics,
    };
    Some(id).filter(|i| i.to_node().is_some())
}

pub fn headline(p: &Person, rep: &mut Report) -> Option<Headline> {
    let headline = rep.opt_line("person.headline", &p.headline, LIM_LINE);
    let summary = if rep.opts.summary_placement == SummaryPlacement::Drop {
        None
    } else {
        rep.opt_block("person.summary", &p.summary, LIM_HEADLINE)
    };
    match (summary, headline) {
        (Some(s), h) => {
            let text = match h {
                Some(h) => format!("{h}\n\n{s}"),
                None => s,
            };
            let (text, orig) = crate::text::clamp(&text, LIM_HEADLINE, true);
            if let Some(n) = orig {
                rep.warn(
                    "text_truncated",
                    "person.summary",
                    format!("headline text of {n} characters cut to {LIM_HEADLINE}"),
                );
            }
            Some(Headline {
                type_: Label::new(
                    Some("personal_statement".into()),
                    Some(rep.locale.t(Key::HeadlinePersonalStatement).into()),
                ),
                description: text,
            })
        }
        (None, Some(h)) => Some(Headline {
            type_: Label::new(
                Some("position".into()),
                Some(rep.locale.t(Key::HeadlinePosition).into()),
            ),
            description: h,
        }),
        (None, None) => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn name_split() {
        assert_eq!(
            split_name("Alex Martin"),
            (Some("Alex".into()), Some("Martin".into()))
        );
        assert_eq!(
            split_name("Jean de La Fontaine"),
            (Some("Jean de La".into()), Some("Fontaine".into()))
        );
        assert_eq!(split_name("Madonna"), (Some("Madonna".into()), None));
        assert_eq!(
            split_name("  Ada   Lovelace "),
            (Some("Ada".into()), Some("Lovelace".into()))
        );
        assert_eq!(split_name("   "), (None, None));
    }

    #[test]
    fn website_uses() {
        assert_eq!(website_use("GitHub").code.as_deref(), Some("portfolio"));
        assert_eq!(website_use("CV").code.as_deref(), Some("portfolio"));
        assert_eq!(website_use("Blog").code.as_deref(), Some("blog"));
        assert_eq!(website_use("LinkedIn").code.as_deref(), Some("personal"));
        assert_eq!(website_use("Whatever").code.as_deref(), Some("personal"));
    }
}

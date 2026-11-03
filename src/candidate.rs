//! CV JSON to the Europass *Candidate* XML, the format the europass.europa.eu editor imports.
//!
//! The legacy `SkillsPassport` XML is refused by the editor ("obsolete version"). The Candidate format has no
//! official schema; its structure comes from the editor's own exports and from the reverse-engineered
//! reference at <https://github.com/J0ker98/europass-cv> (CC-BY-4.0), see `docs/candidate.md`.
use crate::date::IsoDate;
use crate::error::{short, ExportError};
use crate::export::Export;
use crate::i18n::{Key, Locale};
use crate::input::{self, CvInput, EducationEntry, Experience, LanguageEntry, Person, SkillEntry};
use crate::map::activities::{clip_project, exp_parts, finish, full_text, project_block, Known};
use crate::map::person::split_name;
use crate::map::{DateField, Report, CAP_EDU, CAP_LANGS, CAP_WORK, LIM_HEADLINE, LIM_LINE};
use crate::options::{DatePrecision, Options, SummaryPlacement};
use crate::text::{clean_uri, valid_email};
use crate::xml::node::{Kids, Node};
use crate::xml::write::serialize;

const NS_EUROPASS: &str = "http://www.europass.eu/1.0";
const NS_OA: &str = "http://www.openapplications.org/oagis/9";
const NS_EURES: &str = "http://www.europass_eures.eu/1.0";
const NS_HR: &str = "http://www.hr-xml.org/3";
const NS_XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
const SCHEME_ID: &str = "europass-xml";

fn t(name: &'static str, text: impl Into<String>) -> Node {
    Node::text(name, text)
}

fn attrs(n: Node, a: &[(&'static str, &str)]) -> Node {
    n.with_attrs(a.iter().map(|(k, v)| (*k, (*v).to_string())).collect())
}

fn group(name: &'static str, kids: Vec<Option<Node>>) -> Option<Node> {
    Node::parent(name, kids)
}

fn scheme_id(name: &'static str, scheme_name: &str, version: &str, text: Option<&str>) -> Node {
    let n = attrs(
        Node::empty(name, vec![]),
        &[
            ("schemeID", SCHEME_ID),
            ("schemeName", scheme_name),
            ("schemeAgencyName", "EUROPASS"),
            ("schemeVersionID", version),
        ],
    );
    match text {
        Some(x) => Node {
            kids: Kids::Text(x.to_string()),
            ..n
        },
        None => n,
    }
}

fn esc_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The crate's plain text (blank line = paragraph break, `- ` = bullet) as the HTML the editor stores.
/// The caller puts the result in a text node, so the XML writer escapes it once more, as the editor does.
pub fn html_from_text(text: &str) -> String {
    let mut out = String::new();
    let mut in_list = false;
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() {
            if in_list {
                out.push_str("</ul>");
                in_list = false;
            }
            continue;
        }
        if let Some(item) = l.strip_prefix("- ") {
            if !in_list {
                out.push_str("<ul>");
                in_list = true;
            }
            out.push_str("<li>");
            out.push_str(&esc_html(item.trim()));
            out.push_str("</li>");
        } else {
            if in_list {
                out.push_str("</ul>");
                in_list = false;
            }
            out.push_str("<p>");
            out.push_str(&esc_html(l));
            out.push_str("</p>");
        }
    }
    if in_list {
        out.push_str("</ul>");
    }
    out
}

/// ISO 3166-1 alpha-2, lower case. The legacy Europass codes `EL` and `UK` become `gr` and `gb`.
fn iso2(code: &str) -> Option<String> {
    let c = code.trim().to_ascii_lowercase();
    if c.len() != 2 || !c.bytes().all(|b| b.is_ascii_lowercase()) {
        return None;
    }
    Some(
        match c.as_str() {
            "el" => "gr",
            "uk" => "gb",
            other => other,
        }
        .to_string(),
    )
}

/// Calling code, country. Longest prefix wins; the first country of a shared code (1, 7) is used.
const DIAL: &[(&str, &str)] = &[
    ("1", "us"),
    ("7", "ru"),
    ("20", "eg"),
    ("27", "za"),
    ("30", "gr"),
    ("31", "nl"),
    ("32", "be"),
    ("33", "fr"),
    ("34", "es"),
    ("36", "hu"),
    ("39", "it"),
    ("40", "ro"),
    ("41", "ch"),
    ("43", "at"),
    ("44", "gb"),
    ("45", "dk"),
    ("46", "se"),
    ("47", "no"),
    ("48", "pl"),
    ("49", "de"),
    ("51", "pe"),
    ("52", "mx"),
    ("54", "ar"),
    ("55", "br"),
    ("56", "cl"),
    ("57", "co"),
    ("58", "ve"),
    ("60", "my"),
    ("61", "au"),
    ("62", "id"),
    ("63", "ph"),
    ("64", "nz"),
    ("65", "sg"),
    ("66", "th"),
    ("81", "jp"),
    ("82", "kr"),
    ("84", "vn"),
    ("86", "cn"),
    ("90", "tr"),
    ("91", "in"),
    ("92", "pk"),
    ("212", "ma"),
    ("213", "dz"),
    ("216", "tn"),
    ("351", "pt"),
    ("352", "lu"),
    ("353", "ie"),
    ("354", "is"),
    ("356", "mt"),
    ("357", "cy"),
    ("358", "fi"),
    ("359", "bg"),
    ("370", "lt"),
    ("371", "lv"),
    ("372", "ee"),
    ("380", "ua"),
    ("381", "rs"),
    ("385", "hr"),
    ("386", "si"),
    ("387", "ba"),
    ("389", "mk"),
    ("420", "cz"),
    ("421", "sk"),
    ("423", "li"),
    ("971", "ae"),
    ("972", "il"),
];

/// Countries whose national numbers are written with a leading trunk `0` that the dial prefix replaces.
const TRUNK_ZERO: &[&str] = &["fr", "gb", "de", "be", "nl", "at", "ch", "ie", "gr"];

/// (calling code, country, national number) from a phone number; `default_country` serves national numbers.
fn split_phone(raw: &str, default_country: Option<&str>) -> Option<(String, String, String)> {
    let cleaned = raw.replace("(0)", "");
    let intl = cleaned.trim_start().starts_with('+') || cleaned.trim_start().starts_with("00");
    let digits: String = cleaned.chars().filter(char::is_ascii_digit).collect();
    if intl {
        let d = digits.strip_prefix("00").unwrap_or(&digits);
        for len in [3usize, 2, 1] {
            let (Some(prefix), Some(rest)) = (d.get(..len), d.get(len..)) else {
                continue;
            };
            if let Some((_, c)) = DIAL.iter().find(|(p, _)| *p == prefix) {
                if !rest.is_empty() {
                    return Some((prefix.to_string(), (*c).to_string(), rest.to_string()));
                }
            }
        }
        return None;
    }
    let country = default_country?;
    let (prefix, _) = DIAL.iter().find(|(_, c)| *c == country)?;
    let national = if TRUNK_ZERO.contains(&country) {
        digits.strip_prefix('0').unwrap_or(&digits)
    } else {
        &digits
    };
    if national.is_empty() {
        return None;
    }
    Some((
        (*prefix).to_string(),
        country.to_string(),
        national.to_string(),
    ))
}

const SOCIAL: &[(&str, &str)] = &[
    ("linkedin.com", "linkedin"),
    ("facebook.com", "facebook"),
    ("twitter.com", "twitter"),
    ("x.com", "twitter"),
    ("instagram.com", "instagram"),
    ("youtube.com", "youtube"),
    ("pinterest.com", "pinterest"),
    ("tumblr.com", "tumblr"),
];

fn social_use(url: &str) -> Option<&'static str> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or(host).to_ascii_lowercase();
    SOCIAL
        .iter()
        .find(|(d, _)| host == *d || host.ends_with(&format!(".{d}")))
        .map(|(_, u)| *u)
}

/// ISO 639-1 (and the Europass extras) to the ISO 639-2/T codes the editor's language list uses.
const LANG3: &[(&str, &str)] = &[
    ("ar", "ara"),
    ("bg", "bul"),
    ("br", "bre"),
    ("ca", "cat"),
    ("co", "cos"),
    ("cs", "ces"),
    ("cy", "cym"),
    ("da", "dan"),
    ("de", "deu"),
    ("el", "ell"),
    ("en", "eng"),
    ("es", "spa"),
    ("et", "est"),
    ("eu", "eus"),
    ("fa", "fas"),
    ("fi", "fin"),
    ("fr", "fra"),
    ("ga", "gle"),
    ("gl", "glg"),
    ("he", "heb"),
    ("hi", "hin"),
    ("hr", "hrv"),
    ("hu", "hun"),
    ("hy", "hye"),
    ("is", "isl"),
    ("it", "ita"),
    ("ja", "jpn"),
    ("ka", "kat"),
    ("ko", "kor"),
    ("la", "lat"),
    ("lt", "lit"),
    ("lv", "lav"),
    ("mk", "mkd"),
    ("mt", "mlt"),
    ("nl", "nld"),
    ("no", "nor"),
    ("oc", "oci"),
    ("pl", "pol"),
    ("pt", "por"),
    ("ro", "ron"),
    ("ru", "rus"),
    ("sk", "slk"),
    ("sl", "slv"),
    ("sq", "sqi"),
    ("sr", "srp"),
    ("sv", "swe"),
    ("tr", "tur"),
    ("uk", "ukr"),
    ("vi", "vie"),
    ("zh", "zho"),
];

/// `(scheme, value)`: a known code is `NORMAL` with its 639-2 code, anything else `FREE_TEXT` with the label.
fn language_code(code: &Option<String>, label: &str) -> (&'static str, String) {
    if let Some(c) = code.as_deref().map(|c| c.trim().to_ascii_lowercase()) {
        if let Some((_, three)) = LANG3.iter().find(|(two, _)| *two == c) {
            return ("NORMAL", (*three).to_string());
        }
    }
    ("FREE_TEXT", label.to_string())
}

fn comm(kids: Vec<Option<Node>>) -> Option<Node> {
    group("Communication", kids)
}

fn person_communications(p: &Person, rep: &mut Report) -> Vec<Node> {
    let mut out = Vec::new();
    let country = p
        .address
        .as_ref()
        .and_then(|a| a.country_code.as_deref())
        .and_then(iso2);
    if let Some(e) = rep.opt_line("person.email", &p.email, 254) {
        let e = e
            .strip_prefix("mailto:")
            .map_or(e.clone(), |x| x.trim().to_string());
        if valid_email(&e) {
            out.extend(comm(vec![
                Some(t("ChannelCode", "Email")),
                Some(t("oa:URI", e)),
            ]));
        } else {
            rep.warn(
                "email_invalid",
                "person.email",
                format!("not a usable email address: \"{}\"", short(&e)),
            );
        }
    }
    for (i, w) in p.websites.iter().enumerate() {
        let path = format!("person.websites[{i}]");
        let Some(url) = rep.line(&format!("{path}.url"), &w.url, 2000) else {
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
        match social_use(&url) {
            Some(u) => out.extend(comm(vec![
                Some(t("ChannelCode", "Social Media")),
                Some(t("UseCode", u)),
                Some(t("oa:URI", url)),
            ])),
            None => out.extend(comm(vec![
                Some(t("ChannelCode", "Web")),
                Some(t("oa:URI", url)),
            ])),
        }
    }
    for (i, ph) in p.phones.iter().enumerate() {
        let path = format!("person.phones[{i}]");
        let Some(number) = rep.line(&format!("{path}.number"), &ph.number, LIM_LINE) else {
            continue;
        };
        let Some((dial, cc, national)) = split_phone(&number, country.as_deref()) else {
            rep.warn(
                "phone_prefix_unknown",
                &path,
                format!(
                    "no country calling code could be determined for \"{}\": the editor requires one, phone skipped",
                    short(&number)
                ),
            );
            continue;
        };
        let use_code = ph
            .use_
            .as_deref()
            .map(|u| u.trim().to_ascii_lowercase())
            .filter(|u| ["home", "work", "mobile", "other"].contains(&u.as_str()));
        out.extend(comm(vec![
            Some(t("ChannelCode", "Telephone")),
            use_code.map(|u| t("UseCode", u)),
            Some(t("CountryDialing", dial)),
            Some(t("oa:DialNumber", national)),
            Some(t("CountryCode", cc)),
        ]));
    }
    if let Some(a) = &p.address {
        let line = [
            rep.opt_line("person.address.line1", &a.line1, LIM_LINE),
            rep.opt_line("person.address.line2", &a.line2, LIM_LINE),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
        let address = group(
            "Address",
            vec![
                Some(line)
                    .filter(|l| !l.is_empty())
                    .map(|l| t("oa:AddressLine", l)),
                rep.opt_line("person.address.municipality", &a.municipality, LIM_LINE)
                    .map(|c| t("oa:CityName", c)),
                country.clone().map(|c| t("CountryCode", c)),
                rep.opt_line("person.address.postalCode", &a.postal_code, LIM_LINE)
                    .map(|c| t("oa:PostalCode", c)),
            ],
        )
        .map(|n| attrs(n, &[("type", "home")]));
        if address.is_some() {
            out.extend(comm(vec![Some(t("UseCode", "home")), address]));
        }
    }
    out
}

fn person_name(p: &Person, rep: &mut Report) -> Option<(Option<String>, Option<String>)> {
    let given = rep.opt_line("person.givenName", &p.given_name, LIM_LINE);
    let family = rep.opt_line("person.familyName", &p.family_name, LIM_LINE);
    let (given, family) = if given.is_some() || family.is_some() {
        (given, family)
    } else {
        rep.opt_line("person.name", &p.name, LIM_LINE)
            .map(|n| split_name(&n))?
    };
    // the schema requires a FamilyName: a lone name is used as one
    match (given, family) {
        (Some(g), None) => {
            rep.warn(
                "name_single",
                "person",
                "only one name given: used as the family name, given name left empty".into(),
            );
            Some((None, Some(g)))
        }
        other => Some(other),
    }
}

fn name_node(name: &(Option<String>, Option<String>)) -> Option<Node> {
    group(
        "PersonName",
        vec![
            // both elements are required by the schema; a missing part is written empty
            Some(t("oa:GivenName", name.0.clone().unwrap_or_default())),
            Some(t("hr:FamilyName", name.1.clone().unwrap_or_default())),
        ],
    )
}

fn date_node(
    name: &'static str,
    ns_child: &'static str,
    d: Option<IsoDate>,
    p: DatePrecision,
) -> Option<Node> {
    d.map(|d| Node {
        name,
        attrs: vec![],
        kids: Kids::Elems(vec![t(ns_child, d.to_text(p))]),
    })
}

fn employment(input: &CvInput, rep: &mut Report) -> Option<Node> {
    let mut items: Vec<&Experience> = input.experiences.iter().collect();
    if let Some(n) = rep.opts.max_experiences {
        items.truncate(n);
    }
    let n = rep.cap("experiences", &items, CAP_WORK);
    let p = rep.opts.date_precision;
    let mut entries: Vec<((u16, u8), Node)> = Vec::new();
    for (i, e) in items.iter().take(n).enumerate() {
        let path = format!("experiences[{i}]");
        let parts = exp_parts(rep, &path, e);
        let mut role = rep.opt_line(&format!("{path}.role"), &e.role, LIM_LINE);
        if role.is_none() {
            // sanitised like the role (the warning for this value is already reported by exp_parts)
            role = e
                .positions
                .first()
                .and_then(|x| x.title.as_deref())
                .map(crate::text::sanitize_line)
                .map(|s| s.text)
                .filter(|s| !s.is_empty());
        }
        let start = rep.date(&format!("{path}.start"), &e.start);
        let end = rep.date(&format!("{path}.end"), &e.end);
        rep.check_order(&path, start, end);
        let current =
            e.current == Some(true) || (end == DateField::Absent && e.current != Some(false));
        let text = full_text(rep, &parts, e);
        let desc = finish(rep, &path, text).map(|d| html_from_text(&d));
        let org = rep.opt_line(&format!("{path}.organization"), &e.organization, LIM_LINE);
        if role.is_none() && org.is_none() && start == DateField::Absent && end == DateField::Absent
        {
            rep.warn(
                "empty_entry",
                &path,
                "experience without role, organization or period skipped".into(),
            );
            continue;
        }
        let period = group(
            "eures:EmploymentPeriod",
            vec![
                date_node("eures:StartDate", "hr:FormattedDateTime", start.ok(), p),
                if current {
                    None
                } else {
                    date_node("eures:EndDate", "hr:FormattedDateTime", end.ok(), p)
                },
                Some(t(
                    "hr:CurrentIndicator",
                    if current { "true" } else { "false" },
                )),
            ],
        )
        .filter(|_| start != DateField::Absent || end != DateField::Absent || current);
        let link = rep
            .opt_line(&format!("{path}.url"), &e.url, 2000)
            .and_then(|u| clean_uri(&u));
        let node = group(
            "EmployerHistory",
            vec![
                org.map(|o| t("hr:OrganizationName", o)),
                link.map(|l| t("Link", l)),
                group(
                    "PositionHistory",
                    vec![
                        role.map(|r| attrs(t("PositionTitle", r), &[("typeCode", "FREETEXT")])),
                        period,
                        desc.map(|d| t("oa:Description", d)),
                    ],
                ),
            ],
        );
        if let Some(node) = node {
            let key = start
                .ok()
                .map_or((0, 0), |d| (d.year, d.month.unwrap_or(0)));
            entries.push((key, node));
        }
    }
    // newest first; stable, so equal starts keep the input order
    entries.sort_by_key(|e| std::cmp::Reverse(e.0));
    group(
        "EmploymentHistory",
        entries.into_iter().map(|(_, n)| Some(n)).collect(),
    )
}

fn education(input: &CvInput, rep: &mut Report) -> Option<Node> {
    let n = rep.cap("education", &input.education, CAP_EDU);
    let p = rep.opts.date_precision;
    let mut out = Vec::new();
    for (i, e) in input.education.iter().take(n).enumerate() {
        out.push(education_entry(rep, i, e, p));
    }
    group("EducationHistory", out)
}

fn education_entry(
    rep: &mut Report,
    i: usize,
    e: &EducationEntry,
    p: DatePrecision,
) -> Option<Node> {
    let path = format!("education[{i}]");
    let Some(title) = rep.line(&format!("{path}.title"), &e.title, LIM_LINE) else {
        rep.warn(
            "empty_entry",
            &path,
            "education entry without title skipped".into(),
        );
        return None;
    };
    let start = rep.date(&format!("{path}.start"), &e.start);
    let end = rep.date(&format!("{path}.end"), &e.end);
    rep.check_order(&path, start, end);
    let blocks: Vec<String> = e
        .projects
        .iter()
        .enumerate()
        .filter_map(|(j, pr)| {
            let pp = format!("{path}.projects[{j}]");
            let pr = clip_project(rep, &pp, pr, start.ok(), end.ok());
            project_block(rep, &pp, &pr, false, &Known::default())
        })
        .collect();
    let activities = finish(rep, &format!("{path}.projects"), blocks.join("\n\n"));
    let school = rep.opt_line(&format!("{path}.school"), &e.school, LIM_LINE);
    let city = rep.opt_line(&format!("{path}.city"), &e.city, LIM_LINE);
    let eqf = e.eqf.as_ref().and_then(|n| match n.as_i64() {
        Some(v) if (1..=8).contains(&v) => Some(v),
        _ => {
            rep.warn(
                "eqf_out_of_range",
                &format!("{path}.eqf"),
                format!(
                    "EQF level must be an integer 1..8, got {}",
                    short(&n.to_string())
                ),
            );
            None
        }
    });
    let level_label = rep.opt_line(&format!("{path}.levelLabel"), &e.level_label, LIM_LINE);
    if eqf.is_none() && level_label.is_some() {
        rep.warn(
            "level_label_dropped",
            &format!("{path}.levelLabel"),
            "the editor only reads a numeric EQF level, the label is not exported".into(),
        );
    }
    let attendance = group(
        "AttendancePeriod",
        vec![
            date_node("StartDate", "hr:FormattedDateTime", start.ok(), p),
            date_node("EndDate", "hr:FormattedDateTime", end.ok(), p),
            (start != DateField::Absent || end != DateField::Absent).then(|| t("Ongoing", "false")),
        ],
    );
    group(
        "EducationOrganizationAttendance",
        vec![
            school.map(|s| t("hr:OrganizationName", s)),
            group(
                "OrganizationContact",
                vec![comm(vec![group(
                    "Address",
                    vec![city.map(|c| t("oa:CityName", c))],
                )])],
            ),
            attendance,
            group(
                "EducationDegree",
                vec![
                    Some(t("hr:DegreeName", title)),
                    activities.map(|a| t("OccupationalSkillsCovered", html_from_text(&a))),
                ],
            ),
            eqf.map(|v| t("EducationLevelCode", v.to_string())),
        ],
    )
}

fn cef_dimension(
    code: &'static str,
    level: &Option<String>,
    rep: &mut Report,
    path: &str,
) -> Option<Node> {
    let raw = level.as_deref()?.trim();
    if raw.is_empty() {
        return None;
    }
    let up = raw.to_ascii_uppercase();
    if !["A1", "A2", "B1", "B2", "C1", "C2"].contains(&up.as_str()) {
        rep.warn(
            "cef_invalid",
            path,
            format!("CEFR level must be A1..C2, got \"{}\"", short(raw)),
        );
        return None;
    }
    group(
        "eures:CompetencyDimension",
        vec![
            Some(t("hr:CompetencyDimensionTypeCode", code)),
            group("eures:Score", vec![Some(t("hr:ScoreText", up))]),
        ],
    )
}

fn language_skill(rep: &mut Report, i: usize, l: &LanguageEntry) -> Option<Node> {
    let path = format!("languages[{i}]");
    let label = rep.line(&format!("{path}.label"), &l.label, LIM_LINE)?;
    let cef = l.cef.clone().unwrap_or_default();
    let dims: Vec<Option<Node>> = vec![
        cef_dimension(
            "CEF-Understanding-Listening",
            &cef.listening,
            rep,
            &format!("{path}.cef.listening"),
        ),
        cef_dimension(
            "CEF-Understanding-Reading",
            &cef.reading,
            rep,
            &format!("{path}.cef.reading"),
        ),
        cef_dimension(
            "CEF-Speaking-Interaction",
            &cef.spoken_interaction,
            rep,
            &format!("{path}.cef.spokenInteraction"),
        ),
        cef_dimension(
            "CEF-Speaking-Production",
            &cef.spoken_production,
            rep,
            &format!("{path}.cef.spokenProduction"),
        ),
        cef_dimension(
            "CEF-Writing-Production",
            &cef.writing,
            rep,
            &format!("{path}.cef.writing"),
        ),
    ];
    let has_levels = dims.iter().any(Option::is_some);
    let level_text = rep.opt_line(&format!("{path}.level"), &l.level, LIM_LINE);
    let (scheme, mut value) = language_code(&l.code, &label);
    if !has_levels {
        if let Some(lv) = level_text.filter(|x| !x.eq_ignore_ascii_case(&label)) {
            if scheme == "FREE_TEXT" {
                value = format!("{label} ({lv})");
            } else {
                rep.warn(
                    "language_level_text_dropped",
                    &format!("{path}.level"),
                    format!(
                        "\"{}\" has no CEFR level and the editor has no place for free text",
                        short(&lv)
                    ),
                );
            }
        }
    }
    let mut kids = vec![
        Some(attrs(t("CompetencyID", value), &[("schemeName", scheme)])),
        Some(t("hr:TaxonomyID", "language")),
    ];
    kids.extend(dims);
    group("PersonCompetency", kids)
}

fn languages(input: &CvInput, rep: &mut Report) -> (Vec<Node>, Option<Node>) {
    let n = rep.cap("languages", &input.languages, CAP_LANGS);
    let mut mothers = Vec::new();
    let mut skills = Vec::new();
    for (i, l) in input.languages.iter().take(n).enumerate() {
        if l.mother_tongue {
            let Some(label) = rep.line(&format!("languages[{i}].label"), &l.label, LIM_LINE) else {
                continue;
            };
            let (scheme, value) = language_code(&l.code, &label);
            mothers.push(attrs(t("PrimaryLanguageCode", value), &[("name", scheme)]));
        } else {
            skills.push(language_skill(rep, i, l));
        }
    }
    (mothers, group("PersonQualifications", skills))
}

fn skills_section(input: &CvInput, rep: &mut Report) -> Option<Node> {
    if !rep.opts.include_skills {
        return None;
    }
    let fr = rep.locale == Locale::Fr;
    let titles = [
        (
            "Computer",
            if fr {
                "Compétences informatiques"
            } else {
                "Computer skills"
            },
        ),
        (
            "Organisational",
            if fr {
                "Compétences organisationnelles"
            } else {
                "Organisational skills"
            },
        ),
        (
            "JobRelated",
            if fr {
                "Compétences liées à l'emploi"
            } else {
                "Job-related skills"
            },
        ),
    ];
    let mut seen: Vec<String> = Vec::new();
    let mut grouped: Vec<Vec<Option<Node>>> = vec![Vec::new(); titles.len()];
    let mut loose: Vec<Option<Node>> = Vec::new();
    let mut dups = 0usize;
    for (i, s) in input.skills.iter().enumerate() {
        let path = format!("skills[{i}]");
        let Some(label) = rep.line(&format!("{path}.label"), &s.label, LIM_LINE) else {
            rep.warn("empty_entry", &path, "skill without label skipped".into());
            continue;
        };
        if seen.iter().any(|x| x.eq_ignore_ascii_case(&label)) {
            dups += 1;
            continue;
        }
        seen.push(label.clone());
        let node = competency(rep, s, label);
        let cat = s
            .category
            .as_deref()
            .map(|c| c.rsplit(['#', '/']).next().unwrap_or(c).trim())
            .unwrap_or("");
        let slot = match cat {
            "TechnicalSkill" | "ToolSkill" => Some(0),
            "ManagementSkill" => Some(1),
            "MethodologicalSkill" | "ComplianceSkill" => Some(2),
            _ => None,
        };
        match slot.and_then(|k| grouped.get_mut(k)) {
            Some(g) => g.push(Some(node)),
            None => loose.push(Some(node)),
        }
    }
    if dups > 0 {
        rep.warn(
            "duplicate_skill",
            "skills",
            format!("{dups} duplicate skill label(s) dropped (the editor rejects duplicates)"),
        );
    }
    let mut kids = loose;
    for ((_, title), g) in titles.iter().zip(grouped) {
        if g.is_empty() {
            continue;
        }
        let mut gk = vec![Some(t("Title", *title))];
        gk.extend(g);
        kids.push(group("SkillsGroup", gk));
    }
    group("Skills", kids)
}

fn competency(rep: &Report, s: &SkillEntry, label: String) -> Node {
    let esco =
        s.esco.as_deref().map(str::trim).filter(|u| {
            rep.opts.include_esco && u.starts_with("http://data.europa.eu/esco/skill/")
        });
    match esco {
        Some(uri) => group(
            "PersonCompetency",
            vec![
                Some(t("CompetencyID", uri)),
                Some(t("hr:TaxonomyID", "ESCO_Skill")),
                Some(t("hr:CompetencyName", label)),
            ],
        ),
        None => group(
            "PersonCompetency",
            vec![
                Some(t("hr:TaxonomyID", "Digital_Skill")),
                Some(t("hr:CompetencyName", label)),
            ],
        ),
    }
    .unwrap_or_else(|| Node::empty("PersonCompetency", vec![]))
}

fn projects(input: &CvInput, rep: &mut Report) -> Vec<Node> {
    if !rep.opts.include_projects {
        return Vec::new();
    }
    let title = rep.locale.t(Key::AchievementProjects).to_string();
    let mut out = Vec::new();
    for (i, p) in input.projects.iter().enumerate() {
        let path = format!("projects[{i}]");
        let Some(name) = rep.line(&format!("{path}.name"), &p.name, LIM_LINE) else {
            rep.warn("empty_entry", &path, "project without name skipped".into());
            continue;
        };
        // the name is the entry title: project_block writes it as a header line, which is dropped below
        let shown = p.clone();
        let block = project_block(rep, &path, &shown, true, &Known::default())
            .map(|b| {
                b.split_once('\n')
                    .map_or(String::new(), |(_, rest)| rest.to_string())
            })
            .and_then(|b| finish(rep, &path, b));
        out.extend(group(
            "Others",
            vec![
                Some(t("Title", title.clone())),
                group(
                    "Other",
                    vec![
                        Some(t("Title", name)),
                        block.map(|b| t("Description", html_from_text(&b))),
                    ],
                ),
            ],
        ));
    }
    out
}

fn summary(p: &Person, rep: &mut Report) -> Option<Node> {
    if rep.opts.summary_placement == SummaryPlacement::Drop {
        return None;
    }
    let mut parts = Vec::new();
    if let Some(h) = rep.opt_line("person.headline", &p.headline, LIM_LINE) {
        parts.push(format!("<p><strong>{}</strong></p>", esc_html(&h)));
    }
    if let Some(s) = rep.opt_block("person.summary", &p.summary, LIM_HEADLINE) {
        parts.push(html_from_text(&s));
    }
    Some(parts.concat())
        .filter(|s| !s.is_empty())
        .map(|s| t("hr:ExecutiveSummary", s))
}

/// Build the Candidate document.
pub fn export(input: &CvInput, opts: &Options) -> Result<Export, ExportError> {
    let mut errs = input::hard_errors(input);
    if !errs.is_empty() {
        return Err(errs.remove(0));
    }
    let locale = Locale::parse(&input.lang).unwrap_or(Locale::En);
    let mut rep = Report::new(opts.clone(), locale);
    let p = input.person.clone().unwrap_or_default();

    let name = person_name(&p, &mut rep);
    if name.is_none() {
        rep.warn(
            "person_name_missing",
            "person",
            "no person name: the Europass editor rejects a CV without a name".into(),
        );
    }
    let name_node = name.as_ref().and_then(name_node);
    let comms = person_communications(&p, &mut rep);
    let email_comm = comms
        .iter()
        .find(|c| matches!(&c.kids, Kids::Elems(k) if k.iter().any(|x| x.name == "ChannelCode" && x.kids == Kids::Text("Email".into()))))
        .cloned();

    let birth = match rep.date("person.birthDate", &p.birth_date) {
        DateField::Ok(d) => Some(d.to_text(DatePrecision::Day)),
        _ => None,
    };
    let gender = p
        .gender
        .as_deref()
        .and_then(|g| match g.trim().to_ascii_lowercase().as_str() {
            "m" | "male" => Some("male"),
            "f" | "female" => Some("female"),
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
        match iso2(&nat.code) {
            Some(c) => nationalities.push(t("NationalityCode", c)),
            None => rep.warn(
                "country_code_unknown",
                &format!("person.nationalities[{i}].code"),
                format!("not a two-letter country code: \"{}\"", short(&nat.code)),
            ),
        }
    }
    let (mothers, qualifications) = languages(input, &mut rep);

    let mut person_kids: Vec<Option<Node>> = vec![name_node.clone()];
    person_kids.extend(comms.into_iter().map(Some));
    person_kids.extend(nationalities.into_iter().map(Some));
    person_kids.push(birth.map(|b| t("hr:BirthDate", b)));
    person_kids.push(gender.map(|g| t("GenderCode", g)));
    person_kids.extend(mothers.into_iter().map(Some));
    let candidate_person = Node::parent_always("CandidatePerson", person_kids);

    let supplier = Node::parent_always(
        "CandidateSupplier",
        vec![
            Some(scheme_id("hr:PartyID", "PartyID", "1.0", None)),
            Some(t("hr:PartyName", "Owner")),
            group("PersonContact", vec![name_node, email_comm]),
            Some(t("hr:PrecedenceCode", "1")),
        ],
    );

    let employment = employment(input, &mut rep);
    let education = education(input, &mut rep);
    let skills = skills_section(input, &mut rep);
    let has_lang = qualifications.is_some();
    let mut order: Vec<&'static str> = Vec::new();
    if education.is_some() {
        order.push("education-training");
    }
    if employment.is_some() {
        order.push("work-experience");
    }
    if has_lang {
        order.push("language");
    }
    if skills.is_some() {
        order.push("profile-skills");
    }

    let mut profile_kids = vec![
        Some(scheme_id("hr:ID", "CandidateProfileID", "1.0", None)),
        summary(&p, &mut rep),
        employment,
        education,
        qualifications,
        skills,
    ];
    profile_kids.extend(projects(input, &mut rep).into_iter().map(Some));
    let profile = attrs(
        Node::parent_always("CandidateProfile", profile_kids),
        &[("languageCode", locale.code())],
    );

    let rendering = group(
        "RenderingInformation",
        vec![group(
            "Design",
            vec![
                Some(t("Template", "Template1")),
                Some(t("Color", "Default")),
                Some(t("FontSize", "Medium")),
                Some(t("Logo", "FirstPage")),
                Some(t("PageNumbers", "false")),
                group(
                    "SectionsOrder",
                    order
                        .into_iter()
                        .map(|s| group("Section", vec![Some(t("Title", s))]))
                        .collect(),
                ),
            ],
        )],
    );

    let root = Node::parent_always(
        "Candidate",
        vec![
            Some(scheme_id(
                "hr:DocumentID",
                "DocumentIdentifier",
                "4.0",
                None,
            )),
            Some(supplier),
            Some(candidate_person),
            Some(profile),
            rendering,
        ],
    )
    .with_attrs(vec![
        ("xmlns", NS_EUROPASS.to_string()),
        ("xmlns:oa", NS_OA.to_string()),
        ("xmlns:eures", NS_EURES.to_string()),
        ("xmlns:hr", NS_HR.to_string()),
        ("xmlns:xsi", NS_XSI.to_string()),
        ("xsi:schemaLocation", format!("{NS_EUROPASS} Candidate.xsd")),
    ]);
    Ok(Export {
        xml: serialize(&root, opts.pretty),
        warnings: rep.issues,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn html_paragraphs_and_lists() {
        assert_eq!(
            html_from_text("Intro & more\n- a\n- b <c>\n\nEnd"),
            "<p>Intro &amp; more</p><ul><li>a</li><li>b &lt;c&gt;</li></ul><p>End</p>"
        );
    }

    #[test]
    fn phone_splitting() {
        assert_eq!(
            split_phone("+33 (0)6 12 34 56 78", None),
            Some(("33".into(), "fr".into(), "612345678".into()))
        );
        assert_eq!(
            split_phone("06 12 34 56 78", Some("fr")),
            Some(("33".into(), "fr".into(), "612345678".into()))
        );
        assert_eq!(
            split_phone("333 000 0000", Some("it")),
            Some(("39".into(), "it".into(), "3330000000".into()))
        );
        assert_eq!(split_phone("0612345678", None), None);
        assert_eq!(split_phone("+9999", None), None);
    }

    #[test]
    fn social_hosts() {
        assert_eq!(
            social_use("https://www.linkedin.com/in/x"),
            Some("linkedin")
        );
        assert_eq!(social_use("https://x.com/y"), Some("twitter"));
        assert_eq!(social_use("https://github.com/z"), None);
        assert_eq!(social_use("https://notlinkedin.com/z"), None);
    }

    #[test]
    fn language_codes() {
        assert_eq!(
            language_code(&Some("fr".into()), "French"),
            ("NORMAL", "fra".into())
        );
        assert_eq!(
            language_code(&Some("xx".into()), "Klingon"),
            ("FREE_TEXT", "Klingon".into())
        );
        assert_eq!(iso2("EL").as_deref(), Some("gr"));
        assert_eq!(iso2("FRA"), None);
    }
}

//! skills[] -> Skills groups (DESIGN section 4.6).
use super::Report;
use crate::input::CvInput;
use crate::model::skills::{Linguistic, SkillSet};
use crate::text::{clamp_list, dedup_ci};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    Computer,
    Organisational,
    JobRelated,
    Other,
}

fn group_of(category: Option<&str>) -> (Group, bool) {
    let local = category
        .map(|c| c.rsplit(['#', '/']).next().unwrap_or(c).trim())
        .unwrap_or("");
    match local {
        "TechnicalSkill" | "ToolSkill" => (Group::Computer, true),
        "ManagementSkill" => (Group::Organisational, true),
        "MethodologicalSkill" | "ComplianceSkill" => (Group::JobRelated, true),
        "" => (Group::Other, true),
        _ => (Group::Other, false),
    }
}

pub fn skill_set(input: &CvInput, rep: &mut Report, linguistic: Option<Linguistic>) -> SkillSet {
    let mut set = SkillSet {
        linguistic,
        ..Default::default()
    };
    if !rep.opts.include_skills {
        return set;
    }
    let mut groups: [Vec<String>; 4] = Default::default();
    for (i, s) in input.skills.iter().enumerate() {
        let path = format!("skills[{i}]");
        let Some(label) = rep.line(&format!("{path}.label"), &s.label, super::LIM_LINE) else {
            rep.warn("empty_entry", &path, "skill without label skipped".into());
            continue;
        };
        let (g, known) = group_of(s.category.as_deref());
        if !known {
            rep.warn(
                "unknown_category",
                &format!("{path}.category"),
                format!(
                    "unknown skill category \"{}\", filed under Other",
                    crate::error::short(s.category.as_deref().unwrap_or(""))
                ),
            );
        }
        if let Some(v) = groups.get_mut(g as usize) {
            v.push(label);
        }
    }
    let names = ["Computer", "Organisational", "JobRelated", "Other"];
    let mut out: [Option<String>; 4] = Default::default();
    for (gi, items) in groups.into_iter().enumerate() {
        let before = items.len();
        let items = dedup_ci(items);
        if items.len() < before {
            rep.warn(
                "duplicate_skill",
                &format!("skills ({})", names.get(gi).unwrap_or(&"")),
                format!("{} duplicate skill label(s) dropped", before - items.len()),
            );
        }
        if items.is_empty() {
            continue;
        }
        let max = rep.opts.max_activities_chars;
        let (text, orig) = clamp_list(&items.join(", "), max);
        if let Some(n) = orig {
            rep.warn(
                "text_truncated",
                &format!("skills ({})", names.get(gi).unwrap_or(&"")),
                format!("skill list of {n} characters cut to whole items within {max}"),
            );
        }
        if let Some(slot) = out.get_mut(gi) {
            *slot = Some(text);
        }
    }
    let [computer, organisational, job_related, other] = out;
    set.computer = computer;
    set.organisational = organisational;
    set.job_related = job_related;
    set.other = other;
    set
}

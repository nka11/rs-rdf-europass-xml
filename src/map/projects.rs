//! projects -> AchievementList, one Achievement per project (DESIGN section 4.7).
use super::activities::{finish, project_block, Known};
use super::{Report, CAP_ACHIEVEMENTS};
use crate::codesets::ACHIEVEMENT_TITLES;
use crate::i18n::Key;
use crate::input::CvInput;
use crate::model::achievement::Achievement;
use crate::model::label::Label;

pub fn achievements(input: &CvInput, rep: &mut Report) -> Vec<Achievement> {
    if !rep.opts.include_projects {
        return Vec::new();
    }
    let n = rep.cap("projects", &input.projects, CAP_ACHIEVEMENTS);
    let code = ACHIEVEMENT_TITLES
        .binary_search(&"projects")
        .is_ok()
        .then(|| "projects".to_string());
    let mut out = Vec::new();
    for (i, p) in input.projects.iter().take(n).enumerate() {
        let path = format!("projects[{i}]");
        let Some(block) = project_block(rep, &path, p, true, &Known::default()) else {
            continue;
        };
        out.push(Achievement {
            title: Label::new(
                code.clone(),
                Some(rep.locale.t(Key::AchievementProjects).to_string()),
            ),
            description: finish(rep, &path, block),
        });
    }
    out
}

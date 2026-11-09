use crate::refs;

pub(crate) struct ReedlineValidator<SE: brush_core::ShellExtensions> {
    pub shell: refs::ShellRef<SE>,
}

/// neosh: says whether a line bash would read as unfinished is complete all
/// the same (a line for navi, `?what's wrong`). It is given the shell, as a
/// `brush_core::Shell<SE>`, to downcast.
pub type Completes = fn(&str, &mut dyn std::any::Any) -> bool;

static COMPLETES: std::sync::OnceLock<Completes> = std::sync::OnceLock::new();

/// neosh: sets the check `Completes` describes, once.
pub fn set_completes(check: Completes) {
    let _ = COMPLETES.set(check);
}

impl<SE: brush_core::ShellExtensions + 'static> reedline::Validator for ReedlineValidator<SE> {
    fn validate(&self, line: &str) -> reedline::ValidationResult {
        let mut shell = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.shell.lock())
        });

        let incomplete = match shell.parse_string(line.to_owned()) {
            Err(brush_parser::ParseError::Tokenizing { inner, position: _ })
                if inner.is_incomplete() =>
            {
                true
            }
            Err(brush_parser::ParseError::ParsingAtEndOfInput) => true,
            _ => false,
        };
        if incomplete && !COMPLETES.get().is_some_and(|check| check(line, &mut *shell)) {
            reedline::ValidationResult::Incomplete
        } else {
            reedline::ValidationResult::Complete
        }
    }
}

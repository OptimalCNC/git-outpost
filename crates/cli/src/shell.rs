use crate::cli::ShellKind;

mod install;

#[allow(unused_imports)]
pub use install::managed_source_block;
pub use install::{
    InstallOptions, ShellInstallReport, default_rc_file, default_script_file, install, uninstall,
};

const BASH_ZSH_INIT_SCRIPT: &str = r#"# >>> git-outpost shell integration >>>
# Git Outpost shell integration for Bash and Zsh.
# Evaluate with:
#   eval "$(gop shell init bash)"
#   eval "$(gop shell init zsh)"
# Remove this marked block if you manually paste it into a shell startup file.
unalias gop 2>/dev/null || true
gop() {
    if [ "$#" -gt 0 ] && [ "$1" = "cd" ]; then
        shift
        if [ "$#" -eq 0 ]; then
            local _gop_target
            _gop_target="$(command gop path src)" || return
            cd "$_gop_target"
            return
        fi

        if [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
            printf '%s\n' 'Usage: gop cd [OUTPOST]'
            printf '%s\n' 'Run `eval "$(gop shell init bash)"` or `eval "$(gop shell init zsh)"` first.'
            printf '%s\n' 'With no OUTPOST, change to the associated source repository.'
            printf '%s\n' 'With OUTPOST, change to the path printed by: gop path OUTPOST'
            return 0
        fi

        local _gop_target
        _gop_target="$(command gop path "$@")" || return
        cd "$_gop_target"
        return
    fi

    command gop "$@"
}
"#;

const BASH_COMPLETION_LOADER: &str = r#"eval "$(COMPLETE=bash command gop)"
"#;

const ZSH_COMPLETION_LOADER: &str = r#"eval "$(COMPLETE=zsh command gop)"
"#;

const DETECTED_COMPLETION_LOADER: &str = r#"if [ -n "${BASH_VERSION:-}" ]; then
    eval "$(COMPLETE=bash command gop)"
elif [ -n "${ZSH_VERSION:-}" ]; then
    eval "$(COMPLETE=zsh command gop)"
else
    printf '%s\n' 'gop: cannot enable completion; source this integration from Bash or Zsh.' >&2
fi
"#;

const INTEGRATION_END: &str = "# <<< git-outpost shell integration <<<\n";

fn completion_loader(shell: Option<ShellKind>) -> &'static str {
    match shell {
        Some(ShellKind::Bash) => BASH_COMPLETION_LOADER,
        Some(ShellKind::Zsh) => ZSH_COMPLETION_LOADER,
        None => DETECTED_COMPLETION_LOADER,
    }
}

pub fn init_script(shell: Option<ShellKind>) -> String {
    let mut script = String::from(BASH_ZSH_INIT_SCRIPT);
    script.push_str(completion_loader(shell));
    script.push_str(INTEGRATION_END);
    script
}

#[cfg(test)]
mod tests {
    use crate::cli::ShellKind;

    use super::init_script;

    #[test]
    fn init_script_selects_completion_loader_without_caching_adapter_output() {
        let bash = init_script(Some(ShellKind::Bash));
        assert!(bash.contains(r#"eval "$(COMPLETE=bash command gop)""#));
        assert!(!bash.contains("BASH_VERSION"));
        assert!(!bash.contains("_clap_complete_gop"));

        let zsh = init_script(Some(ShellKind::Zsh));
        assert!(zsh.contains(r#"eval "$(COMPLETE=zsh command gop)""#));
        assert!(!zsh.contains("BASH_VERSION"));
        assert!(!zsh.contains("_clap_dynamic_completer_gop"));

        let detected = init_script(None);
        assert!(detected.contains("BASH_VERSION"));
        assert!(detected.contains("ZSH_VERSION"));
        assert!(!detected.contains("${SHELL"));
        assert!(detected.contains("cannot enable completion"));
    }

    #[test]
    fn gop_cd_help_mentions_shell_init_setup() {
        let script = init_script(None);
        assert!(
            script.contains("eval \"$(gop shell init bash)\"")
                || script.contains("eval \"$(gop shell init zsh)\""),
            "expected setup command in script:\n{script}"
        );
        assert!(
            script.contains(
                "Run `eval \"$(gop shell init bash)\"` or `eval \"$(gop shell init zsh)\"` first."
            ),
            "expected gop cd help to mention shell init setup:\n{script}"
        );
    }
}

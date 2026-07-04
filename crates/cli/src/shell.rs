use crate::cli::ShellKind;

pub fn init_script(shell: Option<ShellKind>) -> &'static str {
    match shell {
        Some(ShellKind::Bash) | Some(ShellKind::Zsh) | None => BASH_ZSH_INIT_SCRIPT,
    }
}

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
# <<< git-outpost shell integration <<<
"#;

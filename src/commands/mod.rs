pub mod devices;
pub mod exec;
pub mod port;
pub mod run;
pub mod sync;

pub fn wrap_in_interactive_shell(command: &str) -> String {
    let escaped = command.replace("'", "'\\''");
    format!("bash -ic '{escaped}'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_in_interactive_shell_simple_command() {
        let wrapped = wrap_in_interactive_shell("echo hello");
        assert_eq!(wrapped, "bash -ic 'echo hello'");
    }

    #[test]
    fn wrap_in_interactive_shell_escapes_single_quotes() {
        let wrapped = wrap_in_interactive_shell("echo 'hello world'");
        assert_eq!(wrapped, "bash -ic 'echo '\\''hello world'\\'''");
    }
}

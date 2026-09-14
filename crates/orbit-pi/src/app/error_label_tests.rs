use super::helpers::*;
use super::*;

#[test]
fn humanize_command_reads_as_a_label() {
    assert_eq!(humanize_command("set_model"), "Set model failed");
    assert_eq!(
        humanize_command("get_available_models"),
        "Get available models failed"
    );
    assert_eq!(humanize_command("follow_up"), "Follow up failed");
    assert_eq!(humanize_command("auth.login"), "Auth login failed");
    // Empty / malformed commands still produce something readable.
    assert_eq!(humanize_command(""), "Command failed");
}

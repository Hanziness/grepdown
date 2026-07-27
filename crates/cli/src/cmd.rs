pub mod init;
pub mod lint;
pub mod reach;
pub mod read;
pub mod search;

/// Print a serializable value as JSON (compact or pretty).
/// Returns Ok(()) after printing — callers should early-return.
pub fn print_json_output<T: serde::Serialize>(value: &T, pretty: bool) -> anyhow::Result<()> {
    let output = if pretty {
        serde_json::to_string_pretty(value)?
    } else {
        serde_json::to_string(value)?
    };
    println!("{}", output);
    Ok(())
}
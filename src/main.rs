use ccometixline::cli::Cli;
use ccometixline::config::{Config, InputData};
use ccometixline::core::{collect_all_segments, StatusLineGenerator};
use std::io::{self, IsTerminal};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse_args();

    // Handle configuration commands
    if cli.init {
        use ccometixline::config::InitResult;
        match Config::init()? {
            InitResult::Created(path) => println!("Created config at {}", path.display()),
            InitResult::AlreadyExists(path) => {
                println!("Config already exists at {}", path.display())
            }
        }
        return Ok(());
    }

    if cli.print {
        let mut config = Config::load().unwrap_or_else(|_| Config::default());

        // Apply theme override if provided
        if let Some(theme) = cli.theme {
            config = ccometixline::ui::themes::ThemePresets::get_theme(&theme);
        }

        config.print()?;
        return Ok(());
    }

    if cli.check {
        let config = Config::load()?;
        config.check()?;
        println!("✓ Configuration valid");
        return Ok(());
    }

    if cli.config {
        #[cfg(feature = "tui")]
        {
            ccometixline::ui::run_configurator()?;
        }
        #[cfg(not(feature = "tui"))]
        {
            eprintln!("TUI feature is not enabled. Please install with --features tui");
            std::process::exit(1);
        }
        return Ok(());
    }

    if cli.update {
        #[cfg(feature = "self-update")]
        {
            println!("Update feature not implemented in new architecture yet");
        }
        #[cfg(not(feature = "self-update"))]
        {
            println!("Update check not available (self-update feature disabled)");
        }
        return Ok(());
    }

    // Handle Claude Code patcher
    if let Some(claude_path) = cli.patch {
        use ccometixline::utils::ClaudeCodePatcher;

        println!("🔧 Claude Code Context Warning Disabler");
        println!("Target file: {}", claude_path);

        // Create backup in same directory
        let backup_path = format!("{}.backup", claude_path);
        std::fs::copy(&claude_path, &backup_path)?;
        println!("📦 Created backup: {}", backup_path);

        // Load and patch
        let mut patcher = ClaudeCodePatcher::new(&claude_path)?;

        println!("\n🔄 Applying patches...");
        let results = patcher.apply_all_patches();
        patcher.save()?;

        ClaudeCodePatcher::print_summary(&results);
        println!("💡 To restore warnings, replace your cli.js with the backup file:");
        println!("   cp {} {}", backup_path, claude_path);

        return Ok(());
    }

    // Load configuration
    let mut config = Config::load().unwrap_or_else(|_| Config::default());

    // Apply theme override if provided
    if let Some(theme) = cli.theme {
        config = ccometixline::ui::themes::ThemePresets::get_theme(&theme);
    }

    // Check if stdin has data
    if io::stdin().is_terminal() {
        // No input data available, show main menu
        #[cfg(feature = "tui")]
        {
            use ccometixline::ui::{MainMenu, MenuResult};

            if let Some(result) = MainMenu::run()? {
                match result {
                    MenuResult::LaunchConfigurator => {
                        ccometixline::ui::run_configurator()?;
                    }
                    MenuResult::InitConfig | MenuResult::CheckConfig => {
                        // These are now handled internally by the menu
                        // and should not be returned, but handle gracefully
                    }
                    MenuResult::Exit => {
                        // Exit gracefully
                    }
                }
            }
        }
        #[cfg(not(feature = "tui"))]
        {
            eprintln!("No input data provided and TUI feature is not enabled.");
            eprintln!("Usage: echo '{{...}}' | ccline");
            eprintln!("   or: ccline --help");
        }
        return Ok(());
    }

    // Read Claude Code data from stdin
    let stdin = io::stdin();
    let input: InputData = serde_json::from_reader(stdin.lock())?;

    // Collect segment data
    let segments_data = collect_all_segments(&config, &input);

    // Render statusline
    let generator = StatusLineGenerator::new(config);
    let statusline = generator.generate(segments_data);

    // Truncate statusline to fit terminal width (leave space for Claude Code's context indicator)
    let statusline = truncate_to_terminal_width(&statusline, 60);

    println!("{}", statusline);

    Ok(())
}

/// Calculate visible width of text (excluding ANSI escape sequences)
fn visible_width(text: &str) -> usize {
    let mut width = 0;
    let mut in_escape = false;

    for ch in text.chars() {
        if ch == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if ch.is_alphabetic() {
                in_escape = false;
            }
        } else {
            // Count visible characters (CJK characters count as 2)
            width += if ch > '\u{FF}' { 2 } else { 1 };
        }
    }

    width
}

/// Get terminal width using multiple fallback methods
fn get_terminal_width() -> Option<usize> {
    use std::io::IsTerminal;

    // Method 1: Try terminal_size on stderr (stderr is usually still connected to terminal)
    let stderr = std::io::stderr();
    if stderr.is_terminal() {
        if let Some((terminal_size::Width(w), _)) = terminal_size::terminal_size_of(&stderr) {
            return Some(w as usize);
        }
    }

    // Method 2: Try COLUMNS environment variable
    if let Ok(cols) = std::env::var("COLUMNS") {
        if let Ok(w) = cols.parse::<usize>() {
            return Some(w);
        }
    }

    // Method 3: Try terminal_size on stdout (fallback)
    if let Some((terminal_size::Width(w), _)) = terminal_size::terminal_size() {
        return Some(w as usize);
    }

    None
}

/// Truncate statusline to fit within a percentage of terminal width
fn truncate_to_terminal_width(text: &str, percent: usize) -> String {
    let max_width = if let Some(term_width) = get_terminal_width() {
        // Reserve space for Claude Code's context indicator (~40 chars)
        let reserved_for_context = 40;
        let available = term_width.saturating_sub(reserved_for_context);
        // Use the smaller of: percentage-based limit or available space
        std::cmp::min((term_width * percent) / 100, available)
    } else {
        // Fallback: assume 120 char terminal, use 60%
        72
    };

    let current_width = visible_width(text);
    if current_width <= max_width {
        return text.to_string();
    }

    // Need to truncate
    let mut result = String::new();
    let mut width = 0;
    let mut in_escape = false;

    for ch in text.chars() {
        if ch == '\x1b' {
            in_escape = true;
            result.push(ch);
        } else if in_escape {
            result.push(ch);
            if ch.is_alphabetic() {
                in_escape = false;
            }
        } else {
            let char_width = if ch > '\u{FF}' { 2 } else { 1 };
            if width + char_width > max_width.saturating_sub(3) {
                result.push_str("...\x1b[0m");
                break;
            }
            result.push(ch);
            width += char_width;
        }
    }

    result
}

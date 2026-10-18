fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "--version") {
        println!(
            "Plotium MCP connector {}\nUsage: plotium-mcp [--client claude-desktop]\nMCP over stdio; open Plotium and enable MCP to use project tools.",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }
    let desktop = args == ["--client", "claude-desktop"];
    if !args.is_empty() && !desktop {
        eprintln!("Unknown arguments; use plotium-mcp --help");
        std::process::exit(2);
    }
    if let Err(error) = plotium_agent::connector::run_stdio(desktop) {
        eprintln!("Plotium MCP connector failed: {error}");
        std::process::exit(1);
    }
}

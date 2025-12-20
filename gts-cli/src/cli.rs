use anyhow::Result;
use clap::{Parser, Subcommand};
use gts::GtsOps;
use std::io::Write;

use crate::gen_schemas::generate_schemas_from_rust;
use crate::server::GtsHttpServer;

#[derive(Parser)]
#[command(name = "gts")]
#[command(about = "GTS helpers CLI (demo)", long_about = None)]
struct Cli {
    /// Increase verbosity (can be used multiple times)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Path to optional GTS config JSON to override defaults
    #[arg(long)]
    config: Option<String>,

    /// Path to json and schema files or directories (global default)
    #[arg(long)]
    path: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate a GTS ID format
    ValidateId {
        #[arg(long)]
        gts_id: String,
    },
    /// Parse a GTS ID into its components
    ParseId {
        #[arg(long)]
        gts_id: String,
    },
    /// Match a GTS ID against a pattern
    MatchIdPattern {
        #[arg(long)]
        pattern: String,
        #[arg(long)]
        candidate: String,
    },
    /// Generate UUID from a GTS ID
    Uuid {
        #[arg(long)]
        gts_id: String,
        #[arg(long, default_value = "major")]
        scope: String,
    },
    /// Validate an instance against its schema
    ValidateInstance {
        #[arg(long)]
        gts_id: String,
    },
    /// Resolve relationships for an entity
    ResolveRelationships {
        #[arg(long)]
        gts_id: String,
    },
    /// Check compatibility between two schemas
    Compatibility {
        #[arg(long)]
        old_schema_id: String,
        #[arg(long)]
        new_schema_id: String,
    },
    /// Cast an instance or schema to a target schema
    Cast {
        #[arg(long)]
        from_id: String,
        #[arg(long)]
        to_schema_id: String,
    },
    /// Query entities using an expression
    Query {
        #[arg(long)]
        expr: String,
        #[arg(long, default_value = "100")]
        limit: usize,
    },
    /// Get attribute value from a GTS entity
    Attr {
        #[arg(long)]
        gts_with_path: String,
    },
    /// List all entities
    List {
        #[arg(long, default_value = "100")]
        limit: usize,
    },
    /// Start the GTS HTTP server
    Server {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value = "8000")]
        port: u16,
    },
    /// Generate `OpenAPI` specification
    OpenapiSpec {
        #[arg(long)]
        out: String,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value = "8000")]
        port: u16,
    },
    /// Generate GTS schemas from Rust source code with `#[struct_to_gts_schema]` annotations
    GenerateFromRust {
        /// Source directory or file to scan for annotated structs
        #[arg(long)]
        source: String,
        /// Output directory for generated schemas (optional: uses paths from macro if not specified)
        #[arg(long)]
        output: Option<String>,
        /// Exclude patterns (can be specified multiple times). Supports glob patterns.
        /// Example: --exclude "tests/*" --exclude "examples/*"
        #[arg(long, action = clap::ArgAction::Append)]
        exclude: Vec<String>,
    },
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Set up logging to match Python implementation
    // WARNING (no -v), INFO (-v), DEBUG (-vv)
    let log_level = match cli.verbose {
        0 => tracing::Level::WARN,
        1 => tracing::Level::INFO,
        _ => tracing::Level::DEBUG,
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();

    // Parse path into Vec<String>
    let path = cli.path.map(|p| vec![p]);

    // Create GtsOps
    let mut ops = GtsOps::new(path, cli.config, cli.verbose as usize);

    match cli.command {
        Commands::Server { host, port } => {
            println!("starting the server @ http://{host}:{port}");
            if cli.verbose == 0 {
                println!("use --verbose to see server logs");
            }
            let server = GtsHttpServer::new(ops, host.clone(), port, cli.verbose);
            server.run().await?;
        }
        Commands::OpenapiSpec { out, host, port } => {
            let server = GtsHttpServer::new(ops, host, port, cli.verbose);
            let spec = server.openapi_spec();
            std::fs::write(&out, serde_json::to_string_pretty(&spec)?)?;
            let result = serde_json::json!({
                "ok": true,
                "out": out
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::ValidateId { gts_id } => {
            let result = ops.validate_id(&gts_id);
            print_result(&result)?;
        }
        Commands::ParseId { gts_id } => {
            let result = ops.parse_id(&gts_id);
            print_result(&result)?;
        }
        Commands::MatchIdPattern { pattern, candidate } => {
            let result = ops.match_id_pattern(&candidate, &pattern);
            print_result(&result)?;
        }
        Commands::Uuid { gts_id, scope: _ } => {
            let result = ops.uuid(&gts_id);
            print_result(&result)?;
        }
        Commands::ValidateInstance { gts_id } => {
            let result = ops.validate_instance(&gts_id);
            print_result(&result)?;
        }
        Commands::ResolveRelationships { gts_id } => {
            let result = ops.schema_graph(&gts_id);
            print_result(&result)?;
        }
        Commands::Compatibility {
            old_schema_id,
            new_schema_id,
        } => {
            let result = ops.compatibility(&old_schema_id, &new_schema_id);
            print_result(&result)?;
        }
        Commands::Cast {
            from_id,
            to_schema_id,
        } => {
            let result = ops.cast(&from_id, &to_schema_id);
            print_result(&result)?;
        }
        Commands::Query { expr, limit } => {
            let result = ops.query(&expr, limit);
            print_result(&result)?;
        }
        Commands::Attr { gts_with_path } => {
            let result = ops.attr(&gts_with_path);
            print_result(&result)?;
        }
        Commands::List { limit } => {
            let result = ops.get_entities(limit);
            print_result(&result)?;
        }
        Commands::GenerateFromRust {
            source,
            output,
            exclude,
        } => {
            generate_schemas_from_rust(&source, output.as_deref(), &exclude, cli.verbose)?;
        }
    }

    Ok(())
}

fn print_result<T: serde::Serialize>(value: &T) -> Result<()> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer_pretty(&mut handle, value)?;
    writeln!(handle)?;
    Ok(())
}

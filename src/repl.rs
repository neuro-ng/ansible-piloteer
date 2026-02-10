use crate::session::Session;
use jmespath::Runtime;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

pub fn run(session: &Session) -> Result<(), Box<dyn std::error::Error>> {
    let mut rl = DefaultEditor::new()?;
    // Optional: Load history
    // if rl.load_history("history.txt").is_err() {
    //     println!("No previous history.");
    // }

    let mut runtime = Runtime::new();
    runtime.register_builtin_functions();
    // Register custom functions
    runtime.register_function("group_by", Box::new(crate::query::GroupBy::new()));
    runtime.register_function("unique", Box::new(crate::query::Unique::new()));
    runtime.register_function("count", Box::new(crate::query::Count::new()));
    runtime.register_function("sum", Box::new(crate::query::Sum::new()));
    runtime.register_function("avg", Box::new(crate::query::Avg::new()));
    runtime.register_function("min", Box::new(crate::query::Min::new()));
    runtime.register_function("max", Box::new(crate::query::Max::new()));

    // Serialize session once for querying
    // Note: This might be expensive for large sessions, but necessary for JMESPath
    println!("Preparing session data...");
    let json_val = serde_json::to_value(session)?;

    // Create a Variable from the Value.
    // jmespath-rs works best with its own Variable type.
    // We can convert serde_json::Value to string and back, but let's try direct if possible.
    // Actually jmespath::Variable::from_serializable is the way.
    let root_var = jmespath::Variable::from_serializable(&json_val)?;

    println!("Interactive Query Mode. Type '.help' for commands.");

    let mut format = "pretty-json";

    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                let line = line.trim();
                rl.add_history_entry(line)?;

                if line.is_empty() {
                    continue;
                }

                if line.starts_with('.') {
                    match line {
                        ".exit" | ".quit" => break,
                        ".json" => {
                            format = "json";
                            println!("Output format set to JSON");
                        }
                        ".yaml" => {
                            format = "yaml";
                            println!("Output format set to YAML");
                        }
                        ".pretty" | ".pretty-json" => {
                            format = "pretty-json";
                            println!("Output format set to Pretty JSON");
                        }
                        ".help" => {
                            println!("Commands:");
                            println!("  .exit, .quit    Exit REPL");
                            println!("  .json           Set output to compact JSON");
                            println!("  .pretty         Set output to pretty JSON (default)");
                            println!("  .yaml           Set output to YAML");
                            println!("  .templates      Show available query templates");
                            println!("  .help           Show this help");
                            println!("\nAvailable Functions:");
                            println!("  group_by(arr, expr)  Group array items by expression");
                            println!("  unique(arr)          Get unique items from array");
                            println!("  count(arr)           Count items in array");
                            println!("  sum(arr)             Sum numeric values in array");
                            println!("  avg(arr)             Average of numeric values");
                            println!("  min(arr)             Minimum value in array");
                            println!("  max(arr)             Maximum value in array");
                        }
                        ".templates" => {
                            println!("\nQuery Templates:");
                            println!("\n1. Failed Tasks:");
                            println!("   task_history[?failed == `true`]");
                            println!("\n2. Changed Hosts:");
                            println!("   task_history[?changed == `true`].host | unique(@)");
                            println!("\n3. Unreachable Hosts:");
                            println!("   hosts[?status == 'unreachable'].name");
                            println!("\n4. Task Execution Count:");
                            println!("   count(task_history[*])");
                            println!("\n5. Failed Tasks by Host:");
                            println!(r"   group_by(task_history[?failed == `true`], &host)");
                            println!("\n6. Tasks with Errors:");
                            println!(
                                "   task_history[?error != null].{{name: name, error: error}}"
                            );
                        }
                        _ => println!("Unknown command: {}", line),
                    }
                    continue;
                }

                // Execute Query
                match runtime.compile(line) {
                    Ok(expr) => match expr.search(&root_var) {
                        Ok(result) => match format {
                            "json" => println!("{}", serde_json::to_string(&result)?),
                            "pretty-json" => println!("{}", serde_json::to_string_pretty(&result)?),
                            "yaml" => println!("{}", serde_yaml::to_string(&result)?),
                            _ => println!("{:?}", result),
                        },
                        Err(e) => eprintln!("Evaluation error: {}", e),
                    },
                    Err(e) => eprintln!("Compilation error: {}", e),
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    // rl.save_history("history.txt")?;
    Ok(())
}

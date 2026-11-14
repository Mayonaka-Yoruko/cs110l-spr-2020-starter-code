use crate::debugger_command::DebuggerCommand;
use crate::dwarf_data::{DwarfData, Error as DwarfError};
use crate::inferior::{Inferior, Status};
use rustyline::error::ReadlineError;
use rustyline::Editor;

/// Parse a raw address literal like `0x401136`.
fn parse_address_literal(addr: &str) -> Option<usize> {
    let addr = addr.trim();
    let hex = addr
        .strip_prefix("0x")
        .or_else(|| addr.strip_prefix("0X"))?;
    usize::from_str_radix(hex, 16).ok()
}

pub struct Debugger {
    target: String,
    history_path: String,
    readline: Editor<()>,
    inferior: Option<Inferior>,
    dwarf_data: DwarfData,
    breakpoints: Vec<usize>,
}

impl Debugger {
    /// Initializes the debugger.
    pub fn new(target: &str) -> Debugger {
        // TODO (milestone 3): initialize the DwarfData

        let dwarf_data = match DwarfData::from_file(target) {
            Ok(val) => val,
            Err(DwarfError::ErrorOpeningFile) => {
                println!("Could not open file {}", target);
                std::process::exit(1);
            }
            Err(DwarfError::DwarfFormatError(err)) => {
                println!("Could not load debugging symbols from {}: {:?}", target, err);
                std::process::exit(1);
            }
        };
        let history_path = format!("{}/.deet_history", std::env::var("HOME").unwrap());
        let mut readline = Editor::<()>::new();
        // Attempt to load history from ~/.deet_history if it exists
        let _ = readline.load_history(&history_path);

        dwarf_data.print();

        Debugger {
            target: target.to_string(),
            history_path,
            readline,
            inferior: None,
            dwarf_data,
            breakpoints: Vec::new(),
        }
    }

    pub fn run(&mut self) {
        loop {
            match self.get_next_command() {
                DebuggerCommand::Run(args) => {
                    if let Some(mut inferior) = self.inferior.take() {
                        inferior.kill();
                    }
                    if let Some(inferior) =
                        Inferior::new(&self.target, &args, &self.breakpoints)
                    {
                        self.inferior = Some(inferior);
                        self.continue_inferior();
                    } else {
                        println!("Error starting subprocess");
                    }
                }
                DebuggerCommand::Continue => {
                    if self.inferior.is_some() {
                        self.continue_inferior();
                    } else {
                        println!("There is no process running");
                    }
                }
                DebuggerCommand::Backtrace => {
                    if let Some(inferior) = self.inferior.as_ref() {
                        if let Err(err) = inferior.print_backtrace(&self.dwarf_data) {
                            println!("Error printing backtrace: {}", err);
                        }
                    } else {
                        println!("There is no process running");
                    }
                }
                DebuggerCommand::Breakpoint(target) => {
                    match self.parse_breakpoint_target(&target) {
                        Some(addr) => {
                            println!("Set breakpoint {} at {:#x}", self.breakpoints.len(), addr);
                            if !self.breakpoints.contains(&addr) {
                                self.breakpoints.push(addr);
                            }
                            if let Some(inferior) = self.inferior.as_mut() {
                                if let Err(err) = inferior.set_breakpoint(addr) {
                                    println!("Error setting breakpoint: {}", err);
                                }
                            }
                        }
                        None => {
                            println!("Invalid breakpoint target: {}", target);
                        }
                    }
                }
                DebuggerCommand::Quit => {
                    return;
                }
            }
        }
    }

    /// Resolve b/break argument to an address.
    /// Supports: *0xADDR, 0xADDR, line number, function name, file:line.
    fn parse_breakpoint_target(&self, target: &str) -> Option<usize> {
        if target.starts_with('*') {
            return parse_address_literal(&target[1..]);
        }
        if let Some(addr) = parse_address_literal(target) {
            return Some(addr);
        }
        if let Ok(line) = target.parse::<usize>() {
            return self.dwarf_data.get_addr_for_line(None, line);
        }
        if let Some((file, line_str)) = target.rsplit_once(':') {
            if let Ok(line) = line_str.parse::<usize>() {
                return self.dwarf_data.get_addr_for_line(Some(file), line);
            }
        }
        self.dwarf_data.get_addr_for_function(None, target)
    }

    fn continue_inferior(&mut self) {
        match self.inferior.as_mut().unwrap().continue_exec() {
            Ok(Status::Exited(status)) => {
                println!("Child exited (status {})", status);
                self.inferior = None;
            }
            Ok(Status::Signaled(signal)) => {
                println!("Child exited due to signal {}", signal);
                self.inferior = None;
            }
            Ok(Status::Stopped(signal, rip)) => {
                // After int3, %rip points one byte past the breakpoint.
                let stop_addr = if self.breakpoints.contains(&(rip.wrapping_sub(1))) {
                    rip - 1
                } else {
                    rip
                };
                let location = match (
                    self.dwarf_data.get_function_from_addr(stop_addr),
                    self.dwarf_data.get_line_from_addr(stop_addr),
                ) {
                    (Some(func), Some(line)) => format!("{} ({})", func, line),
                    (Some(func), None) => func,
                    (None, Some(line)) => format!("{}", line),
                    (None, None) => format!("{:#x}", stop_addr),
                };
                println!("Child stopped (signal {}) at {}", signal, location);
            }
            Err(err) => {
                println!("Error continuing inferior: {}", err);
            }
        }
    }

    /// This function prompts the user to enter a command, and continues re-prompting until the user
    /// enters a valid command. It uses DebuggerCommand::from_tokens to do the command parsing.
    ///
    /// You don't need to read, understand, or modify this function.
    fn get_next_command(&mut self) -> DebuggerCommand {
        loop {
            // Print prompt and get next line of user input
            match self.readline.readline("(deet) ") {
                Err(ReadlineError::Interrupted) => {
                    // User pressed ctrl+c. We're going to ignore it
                    println!("Type \"quit\" to exit");
                }
                Err(ReadlineError::Eof) => {
                    // User pressed ctrl+d, which is the equivalent of "quit" for our purposes
                    return DebuggerCommand::Quit;
                }
                Err(err) => {
                    panic!("Unexpected I/O error: {:?}", err);
                }
                Ok(line) => {
                    if line.trim().len() == 0 {
                        continue;
                    }
                    self.readline.add_history_entry(line.as_str());
                    if let Err(err) = self.readline.save_history(&self.history_path) {
                        println!(
                            "Warning: failed to save history file at {}: {}",
                            self.history_path, err
                        );
                    }
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    if let Some(cmd) = DebuggerCommand::from_tokens(&tokens) {
                        return cmd;
                    } else {
                        println!("Unrecognized command.");
                    }
                }
            }
        }
    }
}

mod editor;
mod file_io;
mod model;

use std::io::{self, Write};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        // Interactive setup
        interactive_setup();
        return;
    }

    // Check for help flag
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        return;
    }

    // Parse flags vs positional filename
    let mut activity: Option<u32> = None;
    let mut chapter: Option<u32> = None;
    let mut lab: Option<u32> = None;
    let mut num_steps: usize = 6;
    let mut filename: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if let Some(rest) = arg.strip_prefix("-a") {
            activity = Some(parse_flag_value(rest, "-a"));
        } else if let Some(rest) = arg.strip_prefix("-c") {
            chapter = Some(parse_flag_value(rest, "-c"));
        } else if let Some(rest) = arg.strip_prefix("-l") {
            lab = Some(parse_flag_value(rest, "-l"));
        } else if let Some(rest) = arg.strip_prefix("-s") {
            num_steps = parse_flag_value(rest, "-s") as usize;
        } else if arg.starts_with('-') {
            eprintln!("Unknown flag: {}", arg);
            std::process::exit(1);
        } else {
            // Positional argument = filename
            filename = Some(arg.clone());
        }
        i += 1;
    }

    // Determine filename
    let filename = if let Some(f) = filename {
        f
    } else if activity.is_some() || chapter.is_some() || lab.is_some() {
        let a = activity.unwrap_or(1);
        let c = chapter.unwrap_or(1);
        let l = lab.unwrap_or(1);
        format!("activity-{:02}_ch_{:02}_lab_{:02}.txt", a, c, l)
    } else {
        eprintln!("No filename or -a/-c/-l flags given.");
        print_usage();
        std::process::exit(1);
    };

    run_editor(&filename, num_steps);
}

fn parse_flag_value(rest: &str, flag: &str) -> u32 {
    if rest.is_empty() {
        eprintln!("Flag {} requires a value (e.g. {}2)", flag, flag);
        std::process::exit(1);
    }
    match rest.parse::<u32>() {
        Ok(v) if v > 0 => v,
        _ => {
            eprintln!("Invalid value for {}: {}", flag, rest);
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    println!(
        "\
Usage: lab-editor [OPTIONS] [FILE]

Open or create a lab submission file for editing.

Arguments:
  [FILE]              Open/create this file directly (default 6 steps)

Options:
  -aN                 Activity number (e.g. -a1)
  -cN                 Chapter number  (e.g. -c2)
  -lN                 Lab number      (e.g. -l1)
  -sN                 Number of steps (default 6, e.g. -s8)
  -h, --help          Print this help and exit

Examples:
  lab-editor myfile.txt           Open/create myfile.txt with 6 steps
  lab-editor myfile.txt -s8       Open/create myfile.txt with 8 steps
  lab-editor -a1 -c2 -l1 -s6     Creates activity-01_ch_02_lab_01.txt (6 steps)
  lab-editor                      Interactive setup"
    );
}

fn interactive_setup() {
    println!("=== Lab Editor Setup ===");
    println!();

    let activity = prompt_required("  Activity number");
    let chapter = prompt_required("  Chapter number");
    let lab_num = prompt_required("  Lab number");
    let num_steps = prompt_required("  Number of steps");

    let filename = format!(
        "activity-{:02}_ch_{:02}_lab_{:02}.txt",
        activity, chapter, lab_num
    );
    println!();
    println!("  -> {}  ({} steps)", filename, num_steps);
    println!();

    run_editor(&filename, num_steps as usize);
}

fn prompt_required(label: &str) -> u32 {
    loop {
        print!("{}: ", label);
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let trimmed = input.trim();

        if trimmed.is_empty() {
            print!("Please enter a value: ");
            io::stdout().flush().unwrap();
            continue;
        }

        match trimmed.parse::<u32>() {
            Ok(v) if v > 0 => return v,
            _ => {
                eprintln!("  Error: Invalid input. Please enter a positive number.");
            }
        }
    }
}


fn run_editor(filename: &str, num_steps: usize) {
    let path = Path::new(filename);

    let mut steps = if path.exists() {
        file_io::load_file(path)
    } else {
        model::make_steps(num_steps)
    };

    // Pad to requested step count if file had fewer
    while steps.len() < num_steps {
        steps.push(model::new_step());
    }

    let mut ed = editor::Editor::new(filename.to_string(), steps);

    if let Err(e) = ed.run() {
        eprintln!("Editor error: {}", e);
        std::process::exit(1);
    }

    println!("Done. File: {}", filename);
}

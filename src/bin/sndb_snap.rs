
use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    program: Option<String>
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if let Some(prog) = args.program {
        println!("Program: {}", prog);
    }

    Ok(())
}

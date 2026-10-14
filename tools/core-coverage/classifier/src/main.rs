use std::path::Path;

fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 {
        eprintln!("usage: outpost-core-coverage-classifier REPO_ROOT SOURCE_DIR_REL");
        std::process::exit(2);
    }
    match outpost_core_coverage_classifier::classify_tree(Path::new(&args[1]), Path::new(&args[2]))
    {
        Ok(report) => println!("{report}"),
        Err(error) => {
            eprintln!("source classification failed: {error}");
            std::process::exit(1);
        }
    }
}

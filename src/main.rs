use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clang::{Clang, Entity, Index};
use structopt::StructOpt;

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Debug, StructOpt)]
#[structopt(name = PKG_NAME, about = "TODO")]
struct CpplumberOptions {
    #[structopt(parse(from_os_str))]
    file_paths: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let options = CpplumberOptions::from_args();

    let clang = Clang::new().map_err(|e| anyhow!(e))?;
    let index = Index::new(&clang, false, false);

    for file_path in options.file_paths {
        let translation_unit = index.parser(file_path).parse()?;
        print_tree(translation_unit.get_entity());
    }

    Ok(())
}

fn print_tree(root_entity: Entity) {
    print_rec(root_entity, 0)
}

fn print_rec(root_entity: Entity, current_depth: usize) {
    for _ in 0..current_depth {
        print!("  ");
    }
    println!("{:?}", root_entity);

    for child in root_entity.get_children() {
        // We're only interested in declarations made in the main files
        if child.is_in_main_file() {
            print_rec(child, current_depth + 1);
        }
    }
}

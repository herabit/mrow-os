use cargo_metadata::MetadataCommand;

fn main() -> anyhow::Result<()> {
    let command = MetadataCommand::new().no_deps().exec()?;

    let packages = command.workspace_packages();

    for package in &packages {
        println!("{}", package.name);
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    for source in [0, 1] {
        let config = tunerift::config::Config {
            source,
            ..Default::default()
        };
        let stations = tunerift::directory::fetch(&config, "")?;
        println!(
            "{}: {} stations",
            tunerift::directory::source_names(&config)[source],
            stations.len()
        );
        for station in stations.iter().take(2) {
            println!("  {}: {} routes", station.name, station.urls.len());
        }
    }
    Ok(())
}

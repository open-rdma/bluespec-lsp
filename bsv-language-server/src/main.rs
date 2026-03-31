use std::error::Error;
use log::info;
use env_logger::{Builder, Target};
use bsv_language_server::run;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 初始化日志
    Builder::new()
        .filter_level(log::LevelFilter::Info)
        .target(Target::Stderr)
        .format_timestamp_micros()
        .init();
    
    info!("Starting BSV Language Server...");
    
    // 创建并运行服务器
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    
    run(stdin, stdout).await?;
    
    Ok(())
}

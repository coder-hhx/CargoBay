use anyhow::Result;
use bollard::Docker;

use cratebay_core::container;
use cratebay_core::models::ImageSearchResult;

use super::{print_structured, OutputFormat};

pub fn print_search_results(results: &[ImageSearchResult], format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Table => {
            println!(
                "{:<40} {:>7} {:>8} {:<8} DESCRIPTION",
                "REFERENCE", "STARS", "SOURCE", "OFFICIAL"
            );
            for r in results {
                let stars = r.stars.unwrap_or(0);
                println!(
                    "{:<40} {:>7} {:>8} {:<8} {}",
                    r.reference,
                    stars,
                    r.source,
                    if r.official { "yes" } else { "no" },
                    r.description
                );
            }
            Ok(())
        }
        _ => print_structured(results, format),
    }
}

pub async fn list(docker: &Docker, format: &OutputFormat) -> Result<()> {
    let images = container::image_list(docker).await?;

    match format {
        OutputFormat::Table => {
            println!("{:<20} {:<50} {:>10}", "ID", "TAGS", "SIZE");
            for img in images {
                let id = img.id.trim_start_matches("sha256:");
                let short = id.chars().take(12).collect::<String>();
                let tags = if img.repo_tags.is_empty() {
                    "<none>".to_string()
                } else {
                    img.repo_tags.join(",")
                };
                println!("{:<20} {:<50} {:>10}", short, tags, img.size_human);
            }
            Ok(())
        }
        _ => print_structured(&images, format),
    }
}

pub async fn search(
    docker: &Docker,
    query: &str,
    limit: Option<u32>,
    format: &OutputFormat,
) -> Result<()> {
    let results = container::image_search(docker, query, limit.map(u64::from)).await?;
    print_search_results(&results, format)
}

pub async fn pull(docker: &Docker, image: &str) -> Result<()> {
    eprintln!("Pulling image: {}", image);

    let cb: container::PullProgressCallback = Box::new(|progress| {
        if progress.total_bytes > 0 {
            let pct = (progress.current_bytes as f64 / progress.total_bytes as f64) * 100.0;
            eprintln!("{:>6.1}% {}", pct, progress.status);
        } else {
            eprintln!("{}", progress.status);
        }
    });

    container::image_pull(docker, image, None, Some(cb)).await?;
    println!("Pulled {}", image);
    Ok(())
}

pub async fn delete(docker: &Docker, id: &str) -> Result<()> {
    container::image_remove(docker, id, false).await?;
    println!("Deleted {}", id);
    Ok(())
}

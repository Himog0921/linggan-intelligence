//! Operator tool for opportunity diagnostics, platform-time projection and mention projection.
//! Mention writes are gated and reuse the existing comment-study memberships.

use linggan_intelligence::opportunity::{
    CommenterDigest, check_invariants, current_as_of, load_current_metric_definition,
    project_comment_platform_times, read_platform_time_report, sync_comment_mentions,
};
use linggan_storage_postgres::Database;
use std::process::ExitCode;
use uuid::Uuid;

const USAGE: &str = "usage:
  linggan-opportunity check-invariants [--schema SCHEMA]
  linggan-opportunity project-platform-times [--schema SCHEMA]
  linggan-opportunity migrate-comment-mentions [--schema SCHEMA]
  linggan-opportunity sync-comment-mentions [--schema SCHEMA]
  linggan-opportunity platform-time-report --domain UUID [--as-of RFC3339] [--tz TZ]
                                            [--periods N] [--schema SCHEMA]";

/// C3 acceptance reports the most recent 12 weeks. This is a reporting window, not a metric
/// threshold, so it stays out of `metric_definition`.
const DEFAULT_COVERAGE_PERIODS: i32 = 12;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(success) => {
            if success {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("opportunity: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Default)]
struct Options {
    schema: Option<String>,
    domain: Option<String>,
    as_of: Option<String>,
    time_zone: Option<String>,
    periods: Option<i32>,
}

fn parse_options(args: &[String]) -> Result<(&str, Options), String> {
    let Some((command, flags)) = args.split_first() else {
        return Err(USAGE.to_owned());
    };
    let mut options = Options::default();
    let mut index = 0;
    while index < flags.len() {
        let flag = flags[index].as_str();
        let value = flags
            .get(index + 1)
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match flag {
            "--schema" => options.schema = Some(value.clone()),
            "--domain" => options.domain = Some(value.clone()),
            "--as-of" => options.as_of = Some(value.clone()),
            "--tz" => options.time_zone = Some(value.clone()),
            "--periods" => {
                let periods: i32 = value
                    .parse()
                    .map_err(|_| "--periods must be a positive integer".to_owned())?;
                if periods < 1 {
                    return Err("--periods must be a positive integer".to_owned());
                }
                options.periods = Some(periods);
            }
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
        index += 2;
    }
    Ok((command.as_str(), options))
}

async fn connect(options: &Options) -> Result<Database, Box<dyn std::error::Error>> {
    let url = std::env::var("LINGGAN_LOCAL_DATABASE_URL")?;
    Ok(match options.schema.as_deref() {
        Some(schema) => Database::connect_within_schema(&url, schema).await?,
        None => Database::connect(&url).await?,
    })
}

async fn run() -> Result<bool, Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, options) = parse_options(&args)?;
    let opportunity_enabled =
        opportunity_card_enabled(std::env::var("LINGGAN_OPPORTUNITY_CARD").ok().as_deref());
    match command {
        "check-invariants" => {
            let database = connect(&options).await?;
            let report = check_invariants(&database).await?;
            print_json(&report)?;
            Ok(!report.failed())
        }
        "project-platform-times" => {
            let database = connect(&options).await?;
            let projection = project_comment_platform_times(&database).await?;
            print_json(&projection)?;
            Ok(true)
        }
        "migrate-comment-mentions" | "sync-comment-mentions" => {
            if !opportunity_enabled {
                return Err("opportunity_card_disabled".into());
            }
            let digest = CommenterDigest::load()?;
            let database = connect(&options).await?;
            let report = sync_comment_mentions(&database, &digest).await?;
            let matches = report
                .reconciliations
                .iter()
                .all(|item| item.reasons.is_empty());
            print_json(&report)?;
            Ok(matches)
        }
        "platform-time-report" => {
            let domain = options
                .domain
                .as_deref()
                .ok_or("platform-time-report requires --domain")?;
            let domain = Uuid::parse_str(domain).map_err(|_| "--domain must be a UUID")?;
            let database = connect(&options).await?;
            let as_of: String = match options.as_of {
                Some(ref as_of) => as_of.clone(),
                None => current_as_of(&database).await?,
            };
            let time_zone = match options.time_zone {
                Some(ref time_zone) => time_zone.clone(),
                None => {
                    load_current_metric_definition(&database)
                        .await
                        .map_err(|error| error.to_string())?
                        .params
                        .period_tz
                }
            };
            let report = read_platform_time_report(
                &database,
                domain,
                &as_of,
                &time_zone,
                options.periods.unwrap_or(DEFAULT_COVERAGE_PERIODS),
            )
            .await?;
            print_json(&report)?;
            Ok(report.timing_violations == 0)
        }
        other => Err(format!("unknown command {other}\n{USAGE}").into()),
    }
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<(), serde_json::Error> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn opportunity_card_enabled(value: Option<&str>) -> bool {
    value == Some("enabled")
}

#[cfg(test)]
mod tests {
    use super::opportunity_card_enabled;

    #[test]
    fn opportunity_switch_is_closed_unless_exactly_enabled() {
        assert!(opportunity_card_enabled(Some("enabled")));
        for value in [
            None,
            Some(""),
            Some("true"),
            Some("ENABLED"),
            Some(" enabled"),
        ] {
            assert!(!opportunity_card_enabled(value));
        }
    }
}

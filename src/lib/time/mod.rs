use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, DurationRound, NaiveDate, NaiveDateTime, SecondsFormat, Utc};
use chrono_tz::America::New_York;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref REPLACE_TIMEZONE_RGX: Regex = Regex::new("[+-][0-9]{2}:[0-9]{2}$").unwrap();
}

// parse_date expects a date formatted as "2023-08-31"
pub fn parse_date(d: &str) -> Result<NaiveDate> {
    if let Some(d) = d.get(..10) {
        NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .map_err(|e| anyhow!("Invalid date format {}: {:?}", d, e))
    } else {
        Err(anyhow!(
            "Invalid date format (too short) - expected '2006-12-31', got '{}'",
            d
        ))
    }
}

// parse_date_time expects a date formatted as "2023-08-31T09:30:00"
pub fn parse_date_time(d: &str) -> Result<NaiveDateTime> {
    NaiveDateTime::parse_from_str(d, "%Y-%m-%dT%H:%M:%S")
        .map_err(|e| anyhow!("Invalid date format {}: {:?}", d, e))
}

pub fn utc_until_tomorrow() -> Result<Duration> {
    until_tomorrow(chrono::offset::Utc::now())
}
pub fn until_tomorrow(d: DateTime<Utc>) -> Result<Duration> {
    let utc_tomorrow = d.duration_round(Duration::days(1)).unwrap();
    Ok(utc_tomorrow - d)
}
pub fn until_nyse_trading_hours_end(utc_time: DateTime<Utc>) -> Result<Duration> {
    let nyse_hours_end = Duration::hours(16); // nyse trading hours end at 4pm eastern time (11:00 utc)
    let ny_time = utc_time.with_timezone(&New_York);
    let ny_end = ny_time.duration_trunc(Duration::days(1)).unwrap() + nyse_hours_end;
    let ny_end = match ny_end - ny_time {
        d if d < Duration::zero() => {
            ny_time.duration_trunc(Duration::days(1)).unwrap() + Duration::days(1) + nyse_hours_end
        }
        _ => ny_end,
    };
    Ok(ny_end - ny_time)
}
pub fn until_nyse_trading_hours_start(utc_time: DateTime<Utc>) -> Result<Duration> {
    let nyse_hours_start = Duration::hours(9) + Duration::minutes(30); // nyse trading hours start at 9:30am eastern time (14:30 utc (utc = eastern time + 5 Hours))
    let ny_time = utc_time.with_timezone(&New_York);
    let ny_start = ny_time.duration_trunc(Duration::days(1)).unwrap() + nyse_hours_start;
    let ny_start = match ny_start - ny_time {
        d if d < Duration::zero() => {
            ny_time.duration_trunc(Duration::days(1)).unwrap()
                + Duration::days(1)
                + nyse_hours_start
        }
        _ => ny_start,
    };
    Ok(ny_start - ny_time)
}
pub fn new_york_now() -> DateTime<chrono_tz::Tz> {
    chrono::offset::Local::now().with_timezone(&New_York)
}
pub fn utc_now() -> DateTime<Utc> {
    chrono::offset::Utc::now()
}

pub async fn wait_until_trading_hours_started(lbl: &str) -> Result<()> {
    let now = utc_now();
    let until_nyse_start = until_nyse_trading_hours_start(now)?;
    let until_nyse_end = until_nyse_trading_hours_end(now)?;
    if until_nyse_start < until_nyse_end {
        // trading hours havent started yet, wait until they do:
        println!(
            "{}: waiting until trading hours start: {:?}",
            lbl, until_nyse_start
        );
        tokio::time::sleep(until_nyse_start.to_std()?).await;
    } else {
        // we war in the middle of the trading day, so don't wait at all and start right away
        println!("{}: waiting until trading hours start: 0", lbl);
    }
    Ok(())
}

pub fn format_naive_daytime(dt: &NaiveDateTime) -> String {
    REPLACE_TIMEZONE_RGX
        .replace(
            &dt.and_utc().to_rfc3339_opts(SecondsFormat::Secs, false)[..],
            "",
        )
        .into_owned()
}
pub fn format_db_time(dt: &DateTime<chrono_tz::Tz>) -> String {
    REPLACE_TIMEZONE_RGX
        .replace(&dt.to_rfc3339_opts(SecondsFormat::Secs, false)[..], "")
        .into_owned()
}
pub fn formatted_ny_db_time() -> String {
    REPLACE_TIMEZONE_RGX
        .replace(
            &new_york_now().to_rfc3339_opts(SecondsFormat::Secs, false)[..],
            "",
        )
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, SubsecRound, TimeZone};

    fn pseudo_date(hours_offset: u32) -> DateTime<Utc> {
        NaiveDate::from_ymd_opt(2022, 1, 23)
            .unwrap()
            .and_hms_milli_opt(hours_offset, 0, 0, 0)
            .unwrap()
            .and_local_timezone(Utc)
            .unwrap()
    }
    #[test]
    fn until_tomorrow_test() {
        let hours = 18;
        let dt = pseudo_date(hours);
        let ut = until_tomorrow(dt).unwrap();

        assert_eq!(ut, chrono::Duration::hours(24i64 - hours as i64));
    }
    #[test]
    fn summer_time() {
        let hours = 4;
        let dt = NaiveDate::from_ymd_opt(2023, 8, 28)
            .unwrap()
            .and_hms_milli_opt(hours, 30, 0, 0)
            .unwrap()
            .and_local_timezone(Utc)
            .unwrap();
        let until_start = until_nyse_trading_hours_start(dt).unwrap();
        // utc in daylight saving time: -4 hours -> 9:30 eastern time is 13:30 utc:
        assert_eq!(until_start, Duration::hours(13i64 - hours as i64));
    }
    #[test]
    fn until_nyse_end() {
        // before end of nyse trading hours:
        let hours = 14;
        let naive_dt = NaiveDate::from_ymd_opt(2022, 1, 19)
            .unwrap()
            .and_hms_opt(hours, 0, 0)
            .unwrap();
        let ny_aware = New_York.from_local_datetime(&naive_dt).unwrap();
        let dt: DateTime<Utc> = ny_aware.with_timezone(&Utc);
        let until_end = until_nyse_trading_hours_end(dt).unwrap();
        assert_eq!(until_end, Duration::hours(16i64 - hours as i64));

        // after end of nyse trading hours:
        let hours = 20;
        let naive_dt = NaiveDate::from_ymd_opt(2038, 1, 19)
            .unwrap()
            .and_hms_opt(hours, 0, 0)
            .unwrap();
        let ny_aware = New_York.from_local_datetime(&naive_dt).unwrap();
        let dt = ny_aware.with_timezone(&Utc);
        let until_end = until_nyse_trading_hours_end(dt).unwrap();
        assert_eq!(until_end, Duration::hours(24 - hours as i64 + 16));
    }
    #[test]
    fn until_nyse_start() {
        // before start of nyse trading hours:
        let hours = 8;
        let naive_dt = NaiveDate::from_ymd_opt(2022, 1, 19)
            .unwrap()
            .and_hms_opt(hours, 0, 0)
            .unwrap();
        let ny_aware = New_York.from_local_datetime(&naive_dt).unwrap();
        let dt = ny_aware.with_timezone(&Utc);
        let until_end = until_nyse_trading_hours_start(dt).unwrap();
        assert_eq!(
            until_end,
            Duration::hours(9 - hours as i64) + Duration::minutes(30)
        );

        // after start of nyse trading hours:
        let hours = 10;
        let naive_dt = NaiveDate::from_ymd_opt(2038, 1, 19)
            .unwrap()
            .and_hms_opt(hours, 0, 0)
            .unwrap();
        let ny_aware = New_York.from_local_datetime(&naive_dt).unwrap();
        let dt = ny_aware.with_timezone(&Utc);
        let until_end = until_nyse_trading_hours_start(dt).unwrap();
        assert_eq!(
            until_end,
            Duration::hours(24 - hours as i64 + 9) + Duration::minutes(30)
        );
    }
    #[test]
    fn current_datetime() {
        let now = utc_now().trunc_subsecs(1);
        assert_eq!(
            now,
            chrono::offset::Local::now()
                .with_timezone(&Utc)
                .trunc_subsecs(1)
        );
    }
    #[test]
    fn date_formatting() {
        // without daylight saving time:
        let hours = 14;
        let months = 1;
        let hours_offset = 5;
        let ny_hours = hours - hours_offset;
        let dt = NaiveDate::from_ymd_opt(2023, months, 28)
            .unwrap()
            .and_hms_milli_opt(hours, 8, 1, 0)
            .unwrap()
            .and_local_timezone(Utc)
            .unwrap()
            .with_timezone(&New_York);

        let db_time_rgx =
            Regex::new("^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}$").unwrap();

        let frmtd = format_db_time(&dt);

        assert!(db_time_rgx.is_match(&frmtd[..]));
        assert_eq!(
            &frmtd[..],
            &format!("2023-{:02}-28T{:02}:08:01", months, ny_hours)[..]
        );

        let months = 8;
        let hours_offset = 4;
        let ny_hours = hours - hours_offset;

        let dt = NaiveDate::from_ymd_opt(2023, months, 28)
            .unwrap()
            .and_hms_milli_opt(hours, 8, 1, 0)
            .unwrap()
            .and_local_timezone(Utc)
            .unwrap()
            .with_timezone(&New_York);

        let frmtd = format_db_time(&dt);

        assert!(db_time_rgx.is_match(&frmtd[..]));
        assert_eq!(
            &frmtd[..],
            &format!("2023-{:02}-28T{:02}:08:01", months, ny_hours)[..]
        );

        let ny_time = formatted_ny_db_time();
        assert!(db_time_rgx.is_match(&ny_time[..]));

        let from = "2024-03-28T00:00:00";
        let dt = parse_date_time(from).unwrap();
        let frmtd = format_naive_daytime(&dt);

        assert!(db_time_rgx.is_match(&frmtd[..]));

        let from = "2024-04-16";
        let dt = parse_date(from).unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 4);
        assert_eq!(dt.day(), 16);
        assert_eq!(&dt.to_string()[..], from);
    }
}

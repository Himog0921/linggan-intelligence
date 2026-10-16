//! Shared local display-time helpers retained after the Targets HTML renderer was retired.
pub(super) use super::collection_target_presentation::keyword_archive_read;

pub(super) fn moment_without_year(value: &str) -> &str {
    // 只认 `YYYY-MM-DD ...` 这一种形状；认不出就原样返回（"尚未巡查" 这类文案）。
    let bytes = value.as_bytes();
    if bytes.len() >= 5 && bytes[0..4].iter().all(u8::is_ascii_digit) && bytes[4] == b'-' {
        &value[5..]
    } else {
        value
    }
}

/// 「下次巡查」写成还有多久，而不是一个绝对时刻。
///
/// 人在这一列要判断的是「还要等多久」，而不是「那一刻的钟点是几点」——后者他还得自己
/// 跟当前时间做一次减法。上次巡查保留绝对时刻：那是一个已经发生的事实，可能要拿去跟
/// 别的记录对时间。
///
/// 逾期单独说。把逾期显示成「0 分钟后」会把一个真的出问题的状态说成正常。
pub(super) fn beijing_now_minutes() -> i64 {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    i64::try_from(seconds / 60).unwrap_or(0) + 8 * 60
}

pub(super) fn minutes_since_epoch(value: &str) -> Option<i64> {
    let bytes = value.as_bytes();
    if bytes.len() < 16 {
        return None;
    }
    let number = |from: usize, to: usize| value.get(from..to)?.parse::<i64>().ok();
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute) = (number(11, 13)?, number(14, 16)?);
    // civil-from-days（Howard Hinnant）：不引日期库也能把年月日折成天数。
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some(days * 1440 + hour * 60 + minute)
}

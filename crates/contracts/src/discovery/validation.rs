use std::collections::HashSet;

use super::package::DiscoveryPackageWire;
use super::query::{DiscoverySort, DiscoveryUnit};
use super::{DiscoveryContractError, DiscoveryPackage, DiscoveryStopReason};

const FIRST_DISCOVERY_CONTRACT_VERSION: &str = "xhs.discovery.visible-card.v1";

pub fn parse_discovery_package(input: &str) -> Result<DiscoveryPackage, DiscoveryContractError> {
    let wire: DiscoveryPackageWire = serde_json::from_str(input)
        .map_err(|error| DiscoveryContractError::SchemaInvalid(error.to_string()))?;

    if wire.contract_version != FIRST_DISCOVERY_CONTRACT_VERSION {
        return Err(DiscoveryContractError::UnsupportedContractVersion(
            wire.contract_version,
        ));
    }
    if !wire.acquisition_spec.is_first_canary() {
        return Err(DiscoveryContractError::FirstCanaryChanged);
    }
    if !is_rfc3339_timestamp(&wire.observed_at) {
        return Err(DiscoveryContractError::InvalidObservedAt);
    }
    if wire.coverage.visible_cards != wire.cards.len() as u16
        || wire.coverage.visible_cards > wire.acquisition_spec.maximum_quota()
        || !matches!(wire.coverage.unit, DiscoveryUnit::VisibleSearchCard)
    {
        return Err(DiscoveryContractError::CoverageDoesNotMatchCards);
    }
    if matches!(
        wire.coverage.stopped_reason,
        DiscoveryStopReason::QuotaReached
    ) && wire.coverage.visible_cards != wire.acquisition_spec.maximum_quota()
    {
        return Err(DiscoveryContractError::QuotaReachedBeforeMaximumQuota);
    }
    if wire
        .cards
        .iter()
        .any(|card| card.content.platform_content_id.trim().is_empty())
    {
        return Err(DiscoveryContractError::MissingPlatformContentIdentity);
    }
    if wire.cards.iter().any(|card| {
        card.occurrence.query != wire.acquisition_spec.query
            || !matches!(card.occurrence.sort, DiscoverySort::Comprehensive)
            || card.occurrence.observed_at != wire.observed_at
            || !is_rfc3339_timestamp(&card.occurrence.observed_at)
            || card.occurrence.result_position == 0
            || card.occurrence.result_position > wire.acquisition_spec.maximum_quota()
    }) {
        return Err(DiscoveryContractError::OccurrenceContextMismatch);
    }
    let positions = wire
        .cards
        .iter()
        .map(|card| card.occurrence.result_position)
        .collect::<HashSet<_>>();
    if positions.len() != wire.cards.len() {
        return Err(DiscoveryContractError::DuplicateResultPosition);
    }

    Ok(DiscoveryPackage {
        acquisition_spec: wire.acquisition_spec,
        observed_at: wire.observed_at,
        coverage: wire.coverage,
        cards: wire.cards,
    })
}

/// This narrow parser keeps the discovery contract explicit without introducing a time library
/// or converting source times into an inferred publication timestamp. It accepts a complete
/// RFC-3339 calendar timestamp with an explicit `Z` or numeric UTC offset.
pub fn is_rfc3339_timestamp(value: &str) -> bool {
    if value.is_empty() || value.trim() != value {
        return false;
    }

    let Some((date, time_with_offset)) = value.split_once('T') else {
        return false;
    };
    let Some((year, month, day)) = parse_date(date) else {
        return false;
    };
    let Some((time, offset)) = split_time_and_offset(time_with_offset) else {
        return false;
    };
    is_valid_date(year, month, day) && is_valid_time(time) && is_valid_offset(offset)
}

fn parse_date(date: &str) -> Option<(u16, u8, u8)> {
    let bytes = date.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    Some((
        parse_decimal_u16(&bytes[0..4])?,
        u8::try_from(parse_decimal_u16(&bytes[5..7])?).ok()?,
        u8::try_from(parse_decimal_u16(&bytes[8..10])?).ok()?,
    ))
}

fn split_time_and_offset(value: &str) -> Option<(&str, &str)> {
    if let Some(time) = value.strip_suffix('Z') {
        return Some((time, "Z"));
    }
    let offset_index = value
        .as_bytes()
        .iter()
        .enumerate()
        .skip(8)
        .find_map(|(index, byte)| matches!(byte, b'+' | b'-').then_some(index))?;
    Some((&value[..offset_index], &value[offset_index..]))
}

fn is_valid_time(time: &str) -> bool {
    let (seconds, fractional) = match time.split_once('.') {
        Some((seconds, fractional)) => (seconds, Some(fractional)),
        None => (time, None),
    };
    let bytes = seconds.as_bytes();
    if bytes.len() != 8 || bytes[2] != b':' || bytes[5] != b':' {
        return false;
    }
    let Some(hour) = parse_decimal_u16(&bytes[0..2]) else {
        return false;
    };
    let Some(minute) = parse_decimal_u16(&bytes[3..5]) else {
        return false;
    };
    let Some(second) = parse_decimal_u16(&bytes[6..8]) else {
        return false;
    };
    hour < 24
        && minute < 60
        && second < 60
        && fractional.is_none_or(|fractional| {
            !fractional.is_empty() && fractional.as_bytes().iter().all(u8::is_ascii_digit)
        })
}

fn is_valid_offset(offset: &str) -> bool {
    if offset == "Z" {
        return true;
    }
    let bytes = offset.as_bytes();
    if bytes.len() != 6 || !matches!(bytes[0], b'+' | b'-') || bytes[3] != b':' {
        return false;
    }
    let Some(hour) = parse_decimal_u16(&bytes[1..3]) else {
        return false;
    };
    let Some(minute) = parse_decimal_u16(&bytes[4..6]) else {
        return false;
    };
    hour < 24 && minute < 60
}

fn is_valid_date(year: u16, month: u8, day: u8) -> bool {
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => {
            29
        }
        2 => 28,
        _ => return false,
    };
    day > 0 && day <= days_in_month
}

fn parse_decimal_u16(bytes: &[u8]) -> Option<u16> {
    bytes.iter().try_fold(0_u16, |value, byte| {
        byte.is_ascii_digit()
            .then(|| value.checked_mul(10)?.checked_add(u16::from(byte - b'0')))
            .flatten()
    })
}

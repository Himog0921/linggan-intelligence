export const FIRST_DISCOVERY_CONTRACT_VERSION = "xhs.discovery.visible-card.v1";

const FIRST_DISCOVERY_SPEC = Object.freeze({
  platform: "xhs",
  query: "ADHD",
  sort: "comprehensive",
  target: Object.freeze({
    basis: "maximum_quota",
    unit: "visible_search_card",
    maximumQuota: 20
  })
});

const STOP_REASONS = new Set([
  "quota_reached",
  "surface_ended",
  "risk_control",
  "manual_stop",
  "unknown"
]);

const CARD_FIELDS = new Set([
  "platformContentId",
  "title",
  "creatorDisplayName",
  "publishedAtSourceText",
  "coverCandidate",
  "resultPosition"
]);

const COVER_FIELDS = new Set(["observedExternalUri"]);

function assertObject(value, label) {
  if (!value || Array.isArray(value) || typeof value !== "object") {
    throw new TypeError(`${label} must be an object.`);
  }
}

function assertOnlyFields(value, allowed, label) {
  for (const field of Object.keys(value)) {
    if (!allowed.has(field)) {
      throw new TypeError(`${label} may not contain ${field}.`);
    }
  }
}

function assertRfc3339(value, label) {
  if (typeof value !== "string" || value.trim() !== value) {
    throw new TypeError(`${label} must be an RFC 3339 timestamp.`);
  }

  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(Z|[+-]\d{2}:\d{2})$/.exec(
    value
  );
  if (!match) {
    throw new TypeError(`${label} must be an RFC 3339 timestamp.`);
  }

  const [, yearText, monthText, dayText, hourText, minuteText, secondText, offset] = match;
  const year = Number(yearText);
  const month = Number(monthText);
  const day = Number(dayText);
  const hour = Number(hourText);
  const minute = Number(minuteText);
  const second = Number(secondText);
  const offsetHour = offset === "Z" ? 0 : Number(offset.slice(1, 3));
  const offsetMinute = offset === "Z" ? 0 : Number(offset.slice(4, 6));
  const februaryDays = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0) ? 29 : 28;
  const daysInMonth = [31, februaryDays, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

  if (
    month < 1 ||
    month > 12 ||
    day < 1 ||
    day > daysInMonth[month - 1] ||
    hour > 23 ||
    minute > 59 ||
    second > 59 ||
    offsetHour > 23 ||
    offsetMinute > 59
  ) {
    throw new TypeError(`${label} must be an RFC 3339 timestamp.`);
  }
}

function optionalText(value, label) {
  if (value === undefined || value === null) {
    return undefined;
  }
  if (typeof value !== "string") {
    throw new TypeError(`${label} must be text when present.`);
  }
  return value;
}

function visibleCard(card, observedAt) {
  assertObject(card, "Discovery card");
  assertOnlyFields(card, CARD_FIELDS, "Discovery card");

  if (typeof card.platformContentId !== "string" || card.platformContentId.trim() === "") {
    throw new TypeError("Discovery card requires a non-empty platformContentId.");
  }
  if (!Number.isInteger(card.resultPosition) || card.resultPosition < 1 || card.resultPosition > 20) {
    throw new TypeError("Discovery card resultPosition must be between 1 and 20.");
  }

  let coverCandidate;
  if (card.coverCandidate !== undefined && card.coverCandidate !== null) {
    assertObject(card.coverCandidate, "MediaCandidate");
    assertOnlyFields(card.coverCandidate, COVER_FIELDS, "MediaCandidate");
    if (
      typeof card.coverCandidate.observedExternalUri !== "string" ||
      card.coverCandidate.observedExternalUri.trim() === ""
    ) {
      throw new TypeError("MediaCandidate requires observedExternalUri.");
    }
    coverCandidate = { observedExternalUri: card.coverCandidate.observedExternalUri };
  }

  return {
    content: {
      platformContentId: card.platformContentId,
      ...(optionalText(card.title, "title") === undefined ? {} : { title: card.title }),
      ...(optionalText(card.creatorDisplayName, "creatorDisplayName") === undefined
        ? {}
        : { creatorDisplayName: card.creatorDisplayName }),
      ...(optionalText(card.publishedAtSourceText, "publishedAtSourceText") === undefined
        ? {}
        : { publishedAtSourceText: card.publishedAtSourceText }),
      ...(coverCandidate === undefined ? {} : { coverCandidate })
    },
    occurrence: {
      query: FIRST_DISCOVERY_SPEC.query,
      sort: FIRST_DISCOVERY_SPEC.sort,
      observedAt,
      resultPosition: card.resultPosition
    }
  };
}

/**
 * Shapes an already-observed, discovery-only package. It does not inspect a page, send a
 * network request, request permission, or create an Evidence record.
 */
export function buildFirstDiscoveryPackage({ observedAt, stoppedReason, cards }) {
  assertRfc3339(observedAt, "observedAt");
  if (!STOP_REASONS.has(stoppedReason)) {
    throw new TypeError("Discovery coverage requires a known stoppedReason.");
  }
  if (!Array.isArray(cards) || cards.length > FIRST_DISCOVERY_SPEC.target.maximumQuota) {
    throw new TypeError("Discovery cards must contain at most 20 actually visible cards.");
  }

  const visibleCards = cards.map((card) => visibleCard(card, observedAt));
  if (
    stoppedReason === "quota_reached" &&
    visibleCards.length !== FIRST_DISCOVERY_SPEC.target.maximumQuota
  ) {
    throw new TypeError("quota_reached requires all 20 currently visible cards to be present.");
  }
  const positions = new Set(visibleCards.map((card) => card.occurrence.resultPosition));
  if (positions.size !== visibleCards.length) {
    throw new TypeError("Discovery result positions must be unique in one package.");
  }

  return {
    contractVersion: FIRST_DISCOVERY_CONTRACT_VERSION,
    acquisitionSpec: {
      platform: FIRST_DISCOVERY_SPEC.platform,
      query: FIRST_DISCOVERY_SPEC.query,
      sort: FIRST_DISCOVERY_SPEC.sort,
      target: { ...FIRST_DISCOVERY_SPEC.target }
    },
    observedAt,
    coverage: {
      unit: FIRST_DISCOVERY_SPEC.target.unit,
      visibleCards: visibleCards.length,
      stoppedReason
    },
    cards: visibleCards
  };
}

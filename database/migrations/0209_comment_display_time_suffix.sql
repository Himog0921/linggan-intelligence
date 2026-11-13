-- C3b follow-up: the platform prints "昨天 14:35" as well as the bare "昨天".
-- The relative label still decides the date; the time of day does not change the
-- day-precision value, so it is stripped before parsing. 0207 is already applied
-- and must stay byte-identical, hence a separate replacement migration.

CREATE OR REPLACE FUNCTION linggan_opportunity_parse_comment_display_date(
 p_text text,
 p_collected_at timestamptz
)
RETURNS date LANGUAGE plpgsql STABLE AS $$
DECLARE
 v_text text := regexp_replace(
  btrim(coalesce(p_text, '')), ' [0-9]{1,2}:[0-9]{2}$', ''
 );
 v_local date;
 v_candidate date;
 v_year integer;
 v_month integer;
 v_day integer;
 v_count integer;
BEGIN
 IF v_text = '' OR p_collected_at IS NULL THEN
  RETURN NULL;
 END IF;
 v_local := (p_collected_at AT TIME ZONE 'Asia/Shanghai')::date;

 IF v_text ~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}$' THEN
  BEGIN
   v_candidate := to_date(v_text, 'YYYY-MM-DD');
  EXCEPTION WHEN others THEN
   RETURN NULL;
  END;
  IF to_char(v_candidate, 'YYYY-MM-DD') <> v_text THEN
   RETURN NULL;
  END IF;
  RETURN v_candidate;
 END IF;

 IF v_text ~ '^[0-9]{2}-[0-9]{2}$' THEN
  v_month := substring(v_text, 1, 2)::integer;
  v_day := substring(v_text, 4, 2)::integer;
  IF v_month < 1 OR v_month > 12 OR v_day < 1 OR v_day > 31 THEN
   RETURN NULL;
  END IF;
  v_year := extract(year FROM v_local)::integer;
  BEGIN
   v_candidate := make_date(v_year, v_month, v_day);
  EXCEPTION WHEN others THEN
   RETURN NULL;
  END;
  IF to_char(v_candidate, 'MM-DD') <> v_text THEN
   RETURN NULL;
  END IF;
  -- XHS prints MM-DD for the current year only. A label later than the collection date must
  -- belong to the previous year (for example, 12-31 collected on 2026-01-05).
  IF v_candidate > v_local THEN
   BEGIN
    v_candidate := make_date(v_year - 1, v_month, v_day);
   EXCEPTION WHEN others THEN
    RETURN NULL;
   END;
   IF to_char(v_candidate, 'MM-DD') <> v_text THEN
    RETURN NULL;
   END IF;
  END IF;
  RETURN v_candidate;
 END IF;

 IF v_text = '昨天' THEN
  RETURN v_local - 1;
 END IF;
 IF v_text = '刚刚' THEN
  RETURN v_local;
 END IF;

 IF v_text ~ '^[0-9]+天前$' THEN
  v_count := substring(v_text from '^([0-9]+)天前$')::integer;
  IF v_count > 3650 THEN
   RETURN NULL;
  END IF;
  RETURN v_local - v_count;
 END IF;

 IF v_text ~ '^[0-9]+小时前$' THEN
  v_count := substring(v_text from '^([0-9]+)小时前$')::integer;
  IF v_count > 87600 THEN
   RETURN NULL;
  END IF;
  RETURN ((p_collected_at - make_interval(hours => v_count)) AT TIME ZONE 'Asia/Shanghai')::date;
 END IF;

 IF v_text ~ '^[0-9]+分钟前$' THEN
  v_count := substring(v_text from '^([0-9]+)分钟前$')::integer;
  IF v_count > 5256000 THEN
   RETURN NULL;
  END IF;
  RETURN ((p_collected_at - make_interval(mins => v_count)) AT TIME ZONE 'Asia/Shanghai')::date;
 END IF;

 RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION linggan_opportunity_comment_display_source_kind(p_text text)
RETURNS text LANGUAGE sql IMMUTABLE AS $$
 SELECT CASE
  WHEN normalized.text ~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}$' THEN 'display_full_date'
  WHEN normalized.text ~ '^[0-9]{2}-[0-9]{2}$' THEN 'display_month_day'
  WHEN normalized.text ~ '(天前|小时前|分钟前|昨天|刚刚)$' THEN 'display_relative'
  ELSE NULL
 END
 FROM (
  SELECT regexp_replace(
   btrim(coalesce(p_text, '')), ' [0-9]{1,2}:[0-9]{2}$', ''
  ) AS text
 ) normalized
$$;

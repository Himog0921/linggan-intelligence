-- Creative AI shares one Beijing-calendar-month USD cap. Reservations contain no prompt text.
INSERT INTO linggan_ai_failure_code VALUES
 ('ai_creative_month_budget_exhausted',false,false,'创作月度额度不足，未派发','executor_v1'),
 ('ai_creative_price_unavailable',false,false,'创作模型价格不完整，未派发','executor_v1'),
 ('ai_creative_historic_spend_unknown',false,false,'创作历史费用未知，未派发','executor_v1');
CREATE TABLE linggan_creation_ai_budget (
 singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
 cap_pico_usd numeric(40,0) NOT NULL CHECK(cap_pico_usd>=0),
 revision integer NOT NULL DEFAULT 1 CHECK(revision>0),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);
INSERT INTO linggan_creation_ai_budget(singleton,cap_pico_usd) VALUES(true,100000000000000);
CREATE TABLE linggan_creation_ai_budget_change (
 request_ref uuid PRIMARY KEY,
 expected_revision integer NOT NULL CHECK(expected_revision>0),
 cap_cents bigint NOT NULL CHECK(cap_cents BETWEEN 0 AND 1000000000),
 result_revision integer NOT NULL CHECK(result_revision=expected_revision+1),
 changed_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TRIGGER creation_ai_budget_change_immutable BEFORE UPDATE OR DELETE ON linggan_creation_ai_budget_change
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TABLE linggan_creation_ai_budget_reservation (
 invocation_ref uuid PRIMARY KEY REFERENCES linggan_model_invocation(invocation_ref) DEFERRABLE INITIALLY DEFERRED,
 month_start date NOT NULL CHECK(EXTRACT(DAY FROM month_start)=1),
 reserved_pico_usd numeric(40,0) NOT NULL CHECK(reserved_pico_usd>=0),
 price_ref uuid NOT NULL REFERENCES linggan_model_price,
 created_at timestamptz NOT NULL
);
CREATE INDEX creation_ai_budget_reservation_month ON linggan_creation_ai_budget_reservation(month_start);
CREATE TRIGGER creation_ai_budget_reservation_immutable BEFORE UPDATE OR DELETE ON linggan_creation_ai_budget_reservation
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

-- Call in the dispatch transaction before the invocation row is inserted; the setting row lock
-- serializes reservations with each other and with changes to the cap. Unknown historical spend
-- cannot silently become zero. Settled known spend replaces the reservation; ambiguous spend keeps it.
CREATE FUNCTION linggan_creation_ai_reserve(invocation uuid, frozen_price uuid, tokens bigint, instant timestamptz)
 RETURNS text LANGUAGE plpgsql AS $$
DECLARE cap numeric; existing numeric; reserve numeric; missing bigint; month date; p linggan_model_price%ROWTYPE;
BEGIN
 IF tokens<0 OR instant IS NULL THEN RAISE EXCEPTION 'creation_ai_budget_invalid'; END IF;
 SELECT cap_pico_usd INTO cap FROM linggan_creation_ai_budget WHERE singleton FOR UPDATE;
 month := date_trunc('month', instant AT TIME ZONE 'Asia/Shanghai')::date;
 SELECT * INTO p FROM linggan_model_price WHERE price_ref=frozen_price;
 IF NOT FOUND OR p.input_micro_usd_per_million IS NULL OR p.output_micro_usd_per_million IS NULL
   OR p.cached_read_micro_usd_per_million IS NULL OR p.cached_write_micro_usd_per_million IS NULL THEN
   RETURN 'price_unavailable';
 END IF;
 reserve := tokens::numeric * GREATEST(p.input_micro_usd_per_million,p.output_micro_usd_per_million,
   p.cached_read_micro_usd_per_million,p.cached_write_micro_usd_per_million)::numeric;
 SELECT COALESCE(sum(COALESCE(i.cost_pico_usd,r.reserved_pico_usd)),0),
   count(*) FILTER (WHERE i.cost_pico_usd IS NULL AND r.invocation_ref IS NULL)
 INTO existing,missing FROM linggan_model_invocation i
 LEFT JOIN linggan_creation_ai_budget_reservation r ON r.invocation_ref=i.invocation_ref
 WHERE i.task_key IN ('writing_legacy','writing_assistant','creation_title','creation_template')
   AND date_trunc('month',i.created_at AT TIME ZONE 'Asia/Shanghai')::date=month;
 IF missing>0 THEN RETURN 'historic_spend_unknown'; END IF;
 IF existing+reserve>cap THEN RETURN 'monthly_budget_exhausted'; END IF;
 INSERT INTO linggan_creation_ai_budget_reservation(invocation_ref,month_start,reserved_pico_usd,price_ref,created_at)
 VALUES(invocation,month,reserve,frozen_price,instant);
 RETURN 'reserved';
END $$;

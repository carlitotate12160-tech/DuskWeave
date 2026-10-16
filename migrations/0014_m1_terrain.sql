DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'dw_m1_terrain') THEN
        CREATE ROLE dw_m1_terrain NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOBYPASSRLS;
    END IF;
END $$;
CREATE SCHEMA terrain;

CREATE TABLE terrain.observations (
    engagement_id UUID NOT NULL, campaign_id UUID NOT NULL, operation_id UUID NOT NULL,
    observation_id UUID NOT NULL UNIQUE, correlation_id UUID NOT NULL,
    source_kind TEXT NOT NULL CHECK (source_kind = 'operator_reviewed_dns'),
    vantage_ref UUID NOT NULL, resolver_ipv4 TEXT NOT NULL, subject_name TEXT NOT NULL, asset_ref UUID,
    perceived_at BIGINT NOT NULL CHECK (perceived_at > 0),
    recorded_at BIGINT NOT NULL CHECK (recorded_at >= perceived_at),
    request_hash TEXT NOT NULL, payload JSONB NOT NULL,
    PRIMARY KEY (engagement_id, campaign_id, operation_id));

CREATE TABLE terrain.materials (
    engagement_id UUID NOT NULL, campaign_id UUID NOT NULL, operation_id UUID NOT NULL,
    material_id UUID NOT NULL UNIQUE, observation_id UUID NOT NULL UNIQUE,
    transport TEXT NOT NULL CHECK (transport IN ('udp', 'tcp')),
    qtype TEXT NOT NULL CHECK (qtype = 'A'),
    rcode TEXT NOT NULL CHECK (rcode IN ('noerror', 'nxdomain', 'servfail', 'refused', 'formerr', 'notimp')),
    truncated BOOLEAN NOT NULL,
    completeness TEXT NOT NULL CHECK (completeness IN ('complete', 'incomplete', 'truncated', 'timeout')),
    answers JSONB NOT NULL CHECK (jsonb_typeof(answers) = 'array' AND jsonb_array_length(answers) <= 4),
    payload JSONB NOT NULL,
    PRIMARY KEY (engagement_id, campaign_id, operation_id),
    FOREIGN KEY (engagement_id, campaign_id, operation_id)
        REFERENCES terrain.observations (engagement_id, campaign_id, operation_id));

CREATE TABLE terrain.evaluations (
    engagement_id UUID NOT NULL, campaign_id UUID NOT NULL, operation_id UUID NOT NULL,
    evaluation_id UUID NOT NULL UNIQUE, observation_id UUID NOT NULL UNIQUE,
    material_id UUID NOT NULL UNIQUE, claim_id UUID NOT NULL UNIQUE,
    outcome TEXT NOT NULL CHECK (outcome IN (
        'qualified_dns_claim', 'incomplete_material', 'truncated_material',
        'negative_or_error_rcode', 'insufficient_empty_answers', 'ttl_zero_not_usable', 'applicability_conflict')),
    usable_routing_premise BOOLEAN NOT NULL, lowest_ttl_seconds INTEGER, valid_until BIGINT, payload JSONB NOT NULL,
    PRIMARY KEY (engagement_id, campaign_id, operation_id),
    FOREIGN KEY (engagement_id, campaign_id, operation_id)
        REFERENCES terrain.materials (engagement_id, campaign_id, operation_id));

CREATE TABLE terrain.claim_revisions (
    engagement_id UUID NOT NULL, campaign_id UUID NOT NULL, operation_id UUID NOT NULL,
    claim_id UUID NOT NULL UNIQUE, evaluation_id UUID NOT NULL UNIQUE,
    observation_id UUID NOT NULL UNIQUE, material_id UUID NOT NULL UNIQUE,
    subject_name TEXT NOT NULL, claim_kind TEXT NOT NULL CHECK (claim_kind = 'dns_hostname_ipv4_resolution'),
    vantage_ref UUID NOT NULL, resolver_ipv4 TEXT NOT NULL, asset_ref UUID,
    claim_revision INTEGER NOT NULL CHECK (claim_revision > 0),
    previous_claim_revision INTEGER NOT NULL CHECK (previous_claim_revision >= 0 AND previous_claim_revision < claim_revision),
    trust_tier TEXT NOT NULL CHECK (trust_tier = 'tier_1'),
    claim_status TEXT NOT NULL CHECK (claim_status IN ('observed', 'withheld', 'conflicted')),
    corroborated BOOLEAN NOT NULL CHECK (corroborated = FALSE), conflicted BOOLEAN NOT NULL,
    perceived_at BIGINT NOT NULL, valid_until BIGINT, answers JSONB NOT NULL, payload JSONB NOT NULL,
    PRIMARY KEY (engagement_id, campaign_id, operation_id),
    UNIQUE (engagement_id, campaign_id, subject_name, claim_kind, vantage_ref, resolver_ipv4, claim_revision),
    FOREIGN KEY (engagement_id, campaign_id, operation_id)
        REFERENCES terrain.evaluations (engagement_id, campaign_id, operation_id));

CREATE TABLE terrain.current_claims (
    engagement_id UUID NOT NULL, campaign_id UUID NOT NULL,
    subject_name TEXT NOT NULL, claim_kind TEXT NOT NULL CHECK (claim_kind = 'dns_hostname_ipv4_resolution'),
    vantage_ref UUID NOT NULL, resolver_ipv4 TEXT NOT NULL, asset_ref UUID,
    claim_revision INTEGER NOT NULL CHECK (claim_revision > 0),
    previous_claim_revision INTEGER NOT NULL CHECK (previous_claim_revision >= 0 AND previous_claim_revision < claim_revision),
    claim_id UUID NOT NULL UNIQUE, evaluation_id UUID NOT NULL UNIQUE,
    observation_id UUID NOT NULL UNIQUE, material_id UUID NOT NULL UNIQUE,
    operation_id UUID NOT NULL UNIQUE, event_id UUID NOT NULL UNIQUE,
    trust_tier TEXT NOT NULL CHECK (trust_tier = 'tier_1'),
    claim_status TEXT NOT NULL CHECK (claim_status IN ('observed', 'withheld', 'conflicted')),
    corroborated BOOLEAN NOT NULL CHECK (corroborated = FALSE), conflicted BOOLEAN NOT NULL,
    perceived_at BIGINT NOT NULL, valid_until BIGINT, answers JSONB NOT NULL, payload JSONB NOT NULL,
    PRIMARY KEY (engagement_id, campaign_id, subject_name, claim_kind, vantage_ref, resolver_ipv4));

CREATE TABLE terrain.outbox (
    engagement_id UUID NOT NULL, campaign_id UUID NOT NULL, operation_id UUID NOT NULL,
    event_id UUID NOT NULL UNIQUE, observation_id UUID NOT NULL UNIQUE,
    material_id UUID NOT NULL UNIQUE, evaluation_id UUID NOT NULL UNIQUE, claim_id UUID NOT NULL UNIQUE,
    claim_revision INTEGER NOT NULL CHECK (claim_revision > 0),
    request_hash TEXT NOT NULL, contract JSONB NOT NULL,
    PRIMARY KEY (engagement_id, campaign_id, operation_id),
    FOREIGN KEY (engagement_id, campaign_id, operation_id)
        REFERENCES terrain.claim_revisions (engagement_id, campaign_id, operation_id));

CREATE TABLE trajectory.terrain_history (
    history_seq BIGSERIAL PRIMARY KEY,
    engagement_id UUID NOT NULL, campaign_id UUID NOT NULL, operation_id UUID NOT NULL,
    event_id UUID NOT NULL, observation_id UUID NOT NULL, material_id UUID NOT NULL,
    evaluation_id UUID NOT NULL, claim_id UUID NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('accepted', 'anomaly')),
    reason TEXT, contract JSONB NOT NULL, recorded_at BIGINT NOT NULL);
CREATE UNIQUE INDEX terrain_history_accepted_idx
    ON trajectory.terrain_history (engagement_id, campaign_id, operation_id) WHERE status = 'accepted';
CREATE UNIQUE INDEX terrain_history_event_accepted_idx
    ON trajectory.terrain_history (event_id) WHERE status = 'accepted';

-- Only admitted request conflicts enter here; rejected bytes are never retained.
CREATE TABLE terrain.integrity_conflicts (
    engagement_id UUID NOT NULL, campaign_id UUID NOT NULL, operation_id UUID NOT NULL,
    claim_id UUID NOT NULL REFERENCES terrain.claim_revisions(claim_id),
    temporal_frontier BIGINT NOT NULL,
    blocks_use BOOLEAN NOT NULL,
    reason TEXT NOT NULL CHECK (reason = 'input_mismatch'),
    PRIMARY KEY (engagement_id, campaign_id, operation_id, temporal_frontier, blocks_use));

-- Derive affected applicability from immutable source references, never caller labels.
CREATE VIEW terrain.claim_integrity_holds AS
    SELECT cr.engagement_id, cr.campaign_id, cr.subject_name, cr.claim_kind,
           cr.vantage_ref, cr.resolver_ipv4, i.temporal_frontier
    FROM terrain.integrity_conflicts i JOIN terrain.claim_revisions cr ON cr.claim_id=i.claim_id
    WHERE i.blocks_use
    UNION ALL
    SELECT cr.engagement_id, cr.campaign_id, cr.subject_name, cr.claim_kind,
           cr.vantage_ref, cr.resolver_ipv4, (cr.payload->>'temporal_frontier')::bigint
    FROM trajectory.terrain_history h JOIN terrain.claim_revisions cr
      ON cr.engagement_id=h.engagement_id AND cr.campaign_id=h.campaign_id AND cr.operation_id=h.operation_id
    WHERE h.status='anomaly';

-- Private admission shared by the two intent-specific writers. Only the migration
-- owner can execute it; a runtime login never supplies accepted Mission metadata.
DROP FUNCTION IF EXISTS terrain.accept_dns_observation(jsonb);
CREATE FUNCTION terrain.admit_dns_request(p_request jsonb)
RETURNS jsonb LANGUAGE plpgsql SET search_path = pg_catalog AS $$
DECLARE
    r jsonb; k text; a jsonb; label text; rule jsonb; rules jsonb;
    m mission.missions%ROWTYPE; reg mission.registration_outbox%ROWTYPE;
    f jsonb; policy jsonb; matched boolean := false;
    now_at bigint := floor(extract(epoch FROM statement_timestamp()))::bigint;
    serialized text;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_terrain', 'MEMBER')
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['dw_runtime','dw_m1_broker','dw_m1_cognition','dw_m1_trajectory']) g WHERE pg_has_role(session_user,g,'MEMBER'))
       OR EXISTS (SELECT 1 FROM pg_roles p WHERE pg_has_role(session_user,p.oid,'SET') AND (p.rolsuper OR p.rolbypassrls OR p.rolcreatedb OR p.rolcreaterole)) THEN
        RAISE EXCEPTION 'unqualified_terrain_writer' USING ERRCODE='42501';
    END IF;
    IF current_setting('transaction_isolation')<>'serializable'
       OR current_setting('synchronous_commit')<>'on' THEN
        RAISE EXCEPTION 'terrain_transaction_refused';
    END IF;
    IF jsonb_typeof(p_request) IS DISTINCT FROM 'object' OR octet_length(p_request::text)>16384 THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    r := '{"asset_ref":null,"expected_claim_revision":0,"uncertainty_seconds":0,"qclass":"IN","limitations":[]}'::jsonb || p_request;
    IF (r - ARRAY['version','engagement_id','campaign_id','operator_ref','expected_mission_revision','goal_ref','exercise_mode',
        'correlation_id','source_kind','vantage_ref','resolver_ipv4','subject_name','asset_ref','expected_claim_revision',
        'perceived_at','uncertainty_seconds','transport','qtype','qclass','rcode','truncated','completeness','limitations','answers']) <> '{}'::jsonb THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    FOREACH k IN ARRAY ARRAY['version','engagement_id','campaign_id','operator_ref','goal_ref','exercise_mode','correlation_id',
        'source_kind','vantage_ref','resolver_ipv4','subject_name','transport','qtype','qclass','rcode','completeness'] LOOP
        IF jsonb_typeof(r->k) IS DISTINCT FROM 'string' THEN RAISE EXCEPTION 'terrain_request_refused'; END IF;
    END LOOP;
    FOREACH k IN ARRAY ARRAY['engagement_id','campaign_id','operator_ref','goal_ref','correlation_id','vantage_ref','asset_ref'] LOOP
        IF k='asset_ref' AND r->k='null'::jsonb THEN CONTINUE; END IF;
        IF jsonb_typeof(r->k) IS DISTINCT FROM 'string'
           OR (r->>k) !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
           OR (r->>k)::uuid='00000000-0000-0000-0000-000000000000'::uuid THEN
            RAISE EXCEPTION 'terrain_request_refused';
        END IF;
    END LOOP;
    FOREACH k IN ARRAY ARRAY['expected_mission_revision','expected_claim_revision','perceived_at','uncertainty_seconds'] LOOP
        IF jsonb_typeof(r->k) IS DISTINCT FROM 'number' OR (r->>k) !~ '^(0|[1-9][0-9]*)$' THEN
            RAISE EXCEPTION 'terrain_request_refused';
        END IF;
    END LOOP;
    IF r->>'version'<>'m1_terrain_dns_observation_v1' OR r->>'source_kind'<>'operator_reviewed_dns'
       OR r->>'expected_mission_revision'<>'1' OR (r->>'expected_claim_revision')::bigint>10000
       OR (r->>'uncertainty_seconds')::bigint>300 OR (r->>'perceived_at')::bigint<=0
       OR (r->>'perceived_at')::bigint>now_at OR r->>'exercise_mode' NOT IN ('blind','defender_informed')
       OR r->>'transport' NOT IN ('udp','tcp') OR r->>'qtype'<>'A' OR r->>'qclass'<>'IN'
       OR r->>'rcode' NOT IN ('noerror','nxdomain','servfail','refused','formerr','notimp')
       OR r->>'completeness' NOT IN ('complete','incomplete','truncated','timeout')
       OR jsonb_typeof(r->'truncated') IS DISTINCT FROM 'boolean' THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    IF length(r->>'subject_name')>253 OR position('.' in r->>'subject_name')=0
       OR r->>'subject_name' ~ '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$' THEN RAISE EXCEPTION 'terrain_request_refused'; END IF;
    FOREACH label IN ARRAY string_to_array(r->>'subject_name','.') LOOP
        IF length(label)>63 OR label !~ '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$' OR label LIKE 'xn--%' THEN
            RAISE EXCEPTION 'terrain_request_refused';
        END IF;
    END LOOP;
    IF family((r->>'resolver_ipv4')::inet)<>4 OR host((r->>'resolver_ipv4')::inet)<>r->>'resolver_ipv4' THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    IF jsonb_typeof(r->'answers') IS DISTINCT FROM 'array' OR jsonb_typeof(r->'limitations') IS DISTINCT FROM 'array' THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    IF jsonb_array_length(r->'answers')>4 OR jsonb_array_length(r->'limitations')>4 THEN RAISE EXCEPTION 'terrain_request_refused'; END IF;
    FOR a IN SELECT value FROM jsonb_array_elements(r->'answers') LOOP
        IF jsonb_typeof(a) IS DISTINCT FROM 'object' OR (a-ARRAY['ipv4','ttl_seconds'])<>'{}'::jsonb
           OR jsonb_typeof(a->'ipv4') IS DISTINCT FROM 'string' OR jsonb_typeof(a->'ttl_seconds') IS DISTINCT FROM 'number'
           OR (a->>'ttl_seconds') !~ '^(0|[1-9][0-9]*)$' THEN RAISE EXCEPTION 'terrain_request_refused'; END IF;
        IF (a->>'ttl_seconds')::bigint>86400 OR family((a->>'ipv4')::inet)<>4 OR host((a->>'ipv4')::inet)<>a->>'ipv4' THEN
            RAISE EXCEPTION 'terrain_request_refused';
        END IF;
    END LOOP;
    IF (SELECT count(DISTINCT value->>'ipv4') FROM jsonb_array_elements(r->'answers'))<>jsonb_array_length(r->'answers')
       OR (SELECT count(DISTINCT value) FROM jsonb_array_elements(r->'limitations'))<>jsonb_array_length(r->'limitations')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements(r->'limitations') x WHERE jsonb_typeof(x.value)<>'string'
          OR x.value#>>'{}' NOT IN ('cache_remaining_ttl','single_vantage_only','partial_alias_chain','resolver_recursion_variance')) THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    -- Lock inside the privileged boundary, including the no-current-claim case.
    SELECT * INTO m FROM mission.missions WHERE engagement_id=(r->>'engagement_id')::uuid AND campaign_id=(r->>'campaign_id')::uuid FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'terrain_authority_unavailable'; END IF;
    SELECT * INTO reg FROM mission.registration_outbox WHERE engagement_id=m.engagement_id AND campaign_id=m.campaign_id AND operation_id=m.operation_id;
    IF NOT FOUND THEN RAISE EXCEPTION 'terrain_authority_unavailable'; END IF;
    f := reg.contract->'fields'; policy := f->'m1_permission';
    IF ((reg.contract @> jsonb_build_object('engagement_id',m.engagement_id,'campaign_id',m.campaign_id,'operation_id',m.operation_id,
          'event_id',reg.event_id,'producer','mission','kind','mission_registered','owner_revision',m.revision)) IS NOT TRUE
       OR (f @> jsonb_build_object('operator_ref',m.operator_ref,'goal_ref',m.goal_ref,'authority_ref',m.authority_ref,
          'authority_revision',m.authority_revision,'included_assets',m.included_assets,'excluded_assets',m.excluded_assets,
          'exercise_mode',m.exercise_mode,'starts_at',m.starts_at,'ends_at',m.ends_at)) IS NOT TRUE
       OR m.revision<>1 OR r->>'operator_ref'<>m.operator_ref::text OR r->>'goal_ref'<>m.goal_ref::text
       OR r->>'exercise_mode'<>m.exercise_mode OR policy->>'policy_version' IS DISTINCT FROM '1'
       OR policy->>'vantage_ref' IS DISTINCT FROM r->>'vantage_ref' OR policy->>'resolver_ipv4' IS DISTINCT FROM r->>'resolver_ipv4'
       OR NOT (m.starts_at<=now_at AND now_at<m.ends_at AND m.starts_at<=(r->>'perceived_at')::bigint AND (r->>'perceived_at')::bigint<m.ends_at)
       OR NOT ((policy->>'starts_at')::bigint<=now_at AND now_at<(policy->>'ends_at')::bigint
          AND (policy->>'starts_at')::bigint<=(r->>'perceived_at')::bigint AND (r->>'perceived_at')::bigint<(policy->>'ends_at')::bigint)
       OR EXISTS (SELECT 1 FROM mission.withdrawals WHERE engagement_id=m.engagement_id AND campaign_id=m.campaign_id)) IS NOT FALSE THEN
        RAISE EXCEPTION 'terrain_authority_unavailable';
    END IF;
    IF r->'asset_ref'='null'::jsonb THEN rules:=policy->'discovery_rules';
    ELSE
        IF NOT (m.included_assets @> jsonb_build_array(r->'asset_ref')) OR m.excluded_assets @> jsonb_build_array(r->'asset_ref') THEN
            RAISE EXCEPTION 'terrain_authority_unavailable';
        END IF;
        SELECT coalesce(jsonb_agg(value->'rule'),'[]'::jsonb) INTO rules FROM jsonb_array_elements(policy->'contact_rules') WHERE value->'asset_ref'=r->'asset_ref';
    END IF;
    FOR rule IN SELECT value FROM jsonb_array_elements(rules) LOOP
        matched := matched OR rule->>'exact'=r->>'subject_name' OR rule->>'label_suffix'=r->>'subject_name'
            OR right(r->>'subject_name',length(rule->>'label_suffix')+1)='.'||(rule->>'label_suffix');
    END LOOP;
    IF matched IS NOT TRUE OR EXISTS (SELECT 1 FROM jsonb_array_elements(policy->'excluded_names') x
        WHERE x.value->>'exact'=r->>'subject_name' OR x.value->>'label_suffix'=r->>'subject_name'
           OR right(r->>'subject_name',length(x.value->>'label_suffix')+1)='.'||(x.value->>'label_suffix')) THEN
        RAISE EXCEPTION 'terrain_authority_unavailable';
    END IF;
    -- All strings/keys are now closed and safe. Match the existing typed serde
    -- representation byte-for-byte; never hash a rejected request as a substitute.
    serialized := replace(json_build_object('version',r->'version','engagement_id',r->'engagement_id','campaign_id',r->'campaign_id',
        'operator_ref',r->'operator_ref','expected_mission_revision',r->'expected_mission_revision','goal_ref',r->'goal_ref',
        'exercise_mode',r->'exercise_mode','correlation_id',r->'correlation_id','source_kind',r->'source_kind','vantage_ref',r->'vantage_ref',
        'resolver_ipv4',r->'resolver_ipv4','subject_name',r->'subject_name','asset_ref',r->'asset_ref','expected_claim_revision',r->'expected_claim_revision',
        'perceived_at',r->'perceived_at','uncertainty_seconds',r->'uncertainty_seconds','transport',r->'transport','qtype',r->'qtype','qclass',r->'qclass',
        'rcode',r->'rcode','truncated',r->'truncated','completeness',r->'completeness','limitations',r->'limitations','answers',r->'answers')::text,' ','');
    RETURN jsonb_build_object('request',r,'registration',reg.contract,'request_hash',encode(sha256(convert_to(serialized,'UTF8')),'hex'),'now',now_at);
EXCEPTION WHEN data_exception THEN RAISE EXCEPTION 'terrain_request_refused';
END; $$;

CREATE FUNCTION terrain.accept_dns_observation(p_request jsonb, p_event jsonb)
RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE
    admitted jsonb; r jsonb; reg jsonb; expected jsonb; ids jsonb := '{}'::jsonb;
    tag text; digest bytea; perceived bigint; recorded bigint; frontier bigint; held bigint;
    prior jsonb; prior_obs jsonb; prior_mat jsonb; duplicate boolean; conflict boolean := false;
    outcome text; status text := 'withheld'; ttl integer; until_at bigint;
    v_e uuid; v_c uuid; v_op uuid; v_ev uuid; v_hash text;
    v_obs jsonb; v_mat jsonb; v_eval jsonb; v_clm jsonb;
    v_obs_id uuid; v_mat_id uuid; v_eval_id uuid; v_clm_id uuid;
    v_rev integer; v_prev integer; v_cur_rev integer;
    existing terrain.outbox%ROWTYPE;
BEGIN
    admitted := terrain.admit_dns_request(p_request); r:=admitted->'request'; reg:=admitted->'registration';
    v_e:=(r->>'engagement_id')::uuid; v_c:=(r->>'campaign_id')::uuid; v_hash:=admitted->>'request_hash';
    IF jsonb_typeof(p_event) IS DISTINCT FROM 'object' OR octet_length(p_event::text)>16384
       OR jsonb_typeof(p_event->'operation_id') IS DISTINCT FROM 'string'
       OR (p_event->>'operation_id') !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' THEN
        RAISE EXCEPTION 'terrain_event_refused';
    END IF;
    v_op:=(p_event->>'operation_id')::uuid;
    IF v_op = '00000000-0000-0000-0000-000000000000'::uuid
       OR EXISTS (SELECT 1 FROM mission.missions WHERE operation_id = v_op)
       OR EXISTS (SELECT 1 FROM mission.planning_assessments WHERE operation_id = v_op)
       OR EXISTS (SELECT 1 FROM mission.withdrawals WHERE operation_id = v_op) THEN
        RAISE EXCEPTION 'operation_conflict' USING ERRCODE = 'P0001';
    END IF;
    SELECT * INTO existing FROM terrain.outbox WHERE engagement_id=v_e AND campaign_id=v_c AND operation_id=v_op FOR UPDATE;
    IF FOUND THEN
        IF existing.request_hash=v_hash AND existing.contract=p_event THEN RETURN existing.contract; END IF;
        RAISE EXCEPTION 'terrain_identity_conflict';
    END IF;
    FOREACH tag IN ARRAY ARRAY['observation','material','evaluation','claim','event'] LOOP
        digest:=substring(sha256(convert_to('dw-terrain-id-v1','UTF8')||decode('00','hex')||uuid_send(v_e)||uuid_send(v_c)||uuid_send(v_op)||convert_to(tag,'UTF8')) FROM 1 FOR 16);
        digest:=set_byte(digest,6,(get_byte(digest,6)&15)|64);
        digest:=set_byte(digest,8,(get_byte(digest,8)&63)|128);
        ids:=ids||jsonb_build_object(tag,encode(digest,'hex')::uuid);
    END LOOP;
    v_obs_id:=(ids->>'observation')::uuid; v_mat_id:=(ids->>'material')::uuid;
    v_eval_id:=(ids->>'evaluation')::uuid; v_clm_id:=(ids->>'claim')::uuid; v_ev:=(ids->>'event')::uuid;
    perceived:=(r->>'perceived_at')::bigint;
    IF jsonb_typeof(p_event#>'{observation,recorded_at}') IS DISTINCT FROM 'number'
       OR (p_event#>>'{observation,recorded_at}') !~ '^[1-9][0-9]*$' THEN RAISE EXCEPTION 'terrain_event_refused'; END IF;
    recorded:=(p_event#>>'{observation,recorded_at}')::bigint;
    IF recorded<perceived OR recorded>(admitted->>'now')::bigint THEN RAISE EXCEPTION 'terrain_event_refused'; END IF;
    SELECT cc.claim_revision, cc.payload, ob.payload, m.payload INTO v_cur_rev, prior, prior_obs, prior_mat
    FROM terrain.current_claims cc JOIN terrain.observations ob USING(engagement_id,campaign_id,operation_id)
    JOIN terrain.materials m USING(engagement_id,campaign_id,operation_id)
    WHERE cc.engagement_id=v_e AND cc.campaign_id=v_c AND cc.subject_name=r->>'subject_name'
      AND cc.claim_kind='dns_hostname_ipv4_resolution' AND cc.vantage_ref=(r->>'vantage_ref')::uuid
      AND cc.resolver_ipv4=r->>'resolver_ipv4' FOR UPDATE OF cc;
    v_prev:=(r->>'expected_claim_revision')::integer; v_rev:=v_prev+1;
    IF COALESCE(v_cur_rev, 0) <> v_prev THEN
        RAISE EXCEPTION 'claim_revision_mismatch' USING ERRCODE = 'P0001';
    END IF;
    SELECT max(h.temporal_frontier) INTO held FROM terrain.claim_integrity_holds h
    WHERE h.engagement_id=v_e AND h.campaign_id=v_c AND h.subject_name=r->>'subject_name'
      AND h.claim_kind='dns_hostname_ipv4_resolution' AND h.vantage_ref=(r->>'vantage_ref')::uuid AND h.resolver_ipv4=r->>'resolver_ipv4';
    frontier:=greatest((prior->>'temporal_frontier')::bigint,held,perceived);
    IF prior IS NOT NULL THEN
        duplicate := (prior->'conflicted'='false'::jsonb AND (held IS NULL OR held<(prior->>'temporal_frontier')::bigint)
            AND perceived=(prior->>'temporal_frontier')::bigint AND perceived=(prior->>'perceived_at')::bigint
            AND r->'uncertainty_seconds'=prior_obs->'uncertainty_seconds' AND r->'asset_ref'=prior->'asset_ref'
            AND (prior_mat-ARRAY['material_id','observation_id','answers','limitations'])=(r-ARRAY['version','engagement_id','campaign_id','operator_ref',
                'expected_mission_revision','goal_ref','exercise_mode','correlation_id','source_kind','vantage_ref','resolver_ipv4','subject_name',
                'asset_ref','expected_claim_revision','perceived_at','uncertainty_seconds','answers','limitations'])
            AND (prior_mat->'answers') @> (r->'answers') AND (r->'answers') @> (prior_mat->'answers')
            AND (prior_mat->'limitations') @> (r->'limitations') AND (r->'limitations') @> (prior_mat->'limitations'));
        conflict := NOT (perceived>greatest((prior->>'temporal_frontier')::bigint,held) OR duplicate);
    END IF;
    SELECT min((value->>'ttl_seconds')::integer) INTO ttl FROM jsonb_array_elements(r->'answers');
    IF conflict THEN outcome:='applicability_conflict'; status:='conflicted'; ttl:=NULL;
    ELSIF r->'truncated'='true'::jsonb OR r->>'completeness'='truncated' THEN outcome:='truncated_material'; ttl:=NULL;
    ELSIF r->>'completeness'<>'complete' OR r->'limitations' @> '["partial_alias_chain"]'::jsonb THEN outcome:='incomplete_material'; ttl:=NULL;
    ELSIF r->>'rcode'<>'noerror' THEN outcome:='negative_or_error_rcode'; ttl:=NULL;
    ELSIF ttl IS NULL THEN outcome:='insufficient_empty_answers';
    ELSIF ttl=0 THEN outcome:='ttl_zero_not_usable'; until_at:=perceived;
    ELSE outcome:='qualified_dns_claim'; status:='observed'; until_at:=perceived+least(ttl,60); END IF;
    v_obs:=jsonb_build_object('observation_id',v_obs_id,'operation_id',v_op,'engagement_id',v_e,'campaign_id',v_c,
        'correlation_id',r->'correlation_id','source_kind',r->'source_kind','vantage_ref',r->'vantage_ref','resolver_ipv4',r->'resolver_ipv4',
        'subject_name',r->'subject_name','asset_ref',r->'asset_ref','perceived_at',perceived,'uncertainty_seconds',r->'uncertainty_seconds','recorded_at',recorded);
    v_mat:=jsonb_build_object('material_id',v_mat_id,'observation_id',v_obs_id,'transport',r->'transport','qtype',r->'qtype','qclass',r->'qclass',
        'rcode',r->'rcode','truncated',r->'truncated','completeness',r->'completeness','limitations',r->'limitations','answers',r->'answers');
    v_eval:=jsonb_build_object('evaluation_id',v_eval_id,'observation_id',v_obs_id,'material_id',v_mat_id,'claim_id',v_clm_id,
        'outcome',outcome,'usable_routing_premise',outcome='qualified_dns_claim','lowest_ttl_seconds',ttl,'valid_until',until_at);
    v_clm:=jsonb_build_object('claim_id',v_clm_id,'claim_revision',v_rev,'previous_claim_revision',v_prev,'evaluation_id',v_eval_id,
        'observation_id',v_obs_id,'material_id',v_mat_id,'subject_name',r->'subject_name','claim_kind','dns_hostname_ipv4_resolution',
        'vantage_ref',r->'vantage_ref','resolver_ipv4',r->'resolver_ipv4','asset_ref',r->'asset_ref','trust_tier','tier_1','claim_status',status,
        'corroborated',false,'conflicted',conflict,'temporal_frontier',frontier,'perceived_at',perceived,'valid_until',until_at,'answers',r->'answers');
    expected:=jsonb_build_object('kind','terrain_changed','version',1,'producer','terrain','publication_obligation','trajectory.terrain_history.v1',
        'engagement_id',v_e,'campaign_id',v_c,'operation_id',v_op,'event_id',v_ev,'correlation_id',r->'correlation_id',
        'registration_operation_id',reg->'operation_id','registration_event_id',reg->'event_id','mission_revision',1,'request_hash',v_hash,
        'observation',v_obs,'material',v_mat,'evaluation',v_eval,'claim',v_clm);
    IF p_event IS DISTINCT FROM expected THEN RAISE EXCEPTION 'terrain_event_refused'; END IF;
    INSERT INTO terrain.observations VALUES (
        v_e, v_c, v_op, v_obs_id, (p_event->>'correlation_id')::uuid, v_obs->>'source_kind',
        (v_obs->>'vantage_ref')::uuid, v_obs->>'resolver_ipv4', v_obs->>'subject_name',
        NULLIF(v_obs->>'asset_ref', '')::uuid, (v_obs->>'perceived_at')::bigint,
        (v_obs->>'recorded_at')::bigint, v_hash, v_obs);
    INSERT INTO terrain.materials VALUES (
        v_e, v_c, v_op, v_mat_id, v_obs_id, v_mat->>'transport', v_mat->>'qtype',
        v_mat->>'rcode', (v_mat->>'truncated')::boolean, v_mat->>'completeness', v_mat->'answers', v_mat);
    INSERT INTO terrain.evaluations VALUES (
        v_e, v_c, v_op, v_eval_id, v_obs_id, v_mat_id, v_clm_id, v_eval->>'outcome',
        (v_eval->>'usable_routing_premise')::boolean, NULLIF(v_eval->>'lowest_ttl_seconds', '')::integer,
        NULLIF(v_eval->>'valid_until', '')::bigint, v_eval);
    INSERT INTO terrain.claim_revisions VALUES (
        v_e, v_c, v_op, v_clm_id, v_eval_id, v_obs_id, v_mat_id, v_clm->>'subject_name',
        v_clm->>'claim_kind', (v_clm->>'vantage_ref')::uuid, v_clm->>'resolver_ipv4',
        NULLIF(v_clm->>'asset_ref', '')::uuid, v_rev, v_prev, v_clm->>'trust_tier',
        v_clm->>'claim_status', (v_clm->>'corroborated')::boolean, (v_clm->>'conflicted')::boolean,
        (v_clm->>'perceived_at')::bigint, NULLIF(v_clm->>'valid_until', '')::bigint, v_clm->'answers', v_clm);
    INSERT INTO terrain.current_claims VALUES (
        v_e, v_c, v_clm->>'subject_name', v_clm->>'claim_kind', (v_clm->>'vantage_ref')::uuid,
        v_clm->>'resolver_ipv4', NULLIF(v_clm->>'asset_ref', '')::uuid, v_rev, v_prev,
        v_clm_id, v_eval_id, v_obs_id, v_mat_id, v_op, v_ev, v_clm->>'trust_tier',
        v_clm->>'claim_status', (v_clm->>'corroborated')::boolean, (v_clm->>'conflicted')::boolean,
        (v_clm->>'perceived_at')::bigint, NULLIF(v_clm->>'valid_until', '')::bigint, v_clm->'answers', v_clm
    ) ON CONFLICT (engagement_id, campaign_id, subject_name, claim_kind, vantage_ref, resolver_ipv4) DO UPDATE SET
        asset_ref = EXCLUDED.asset_ref, claim_revision = EXCLUDED.claim_revision,
        previous_claim_revision = EXCLUDED.previous_claim_revision, claim_id = EXCLUDED.claim_id,
        evaluation_id = EXCLUDED.evaluation_id, observation_id = EXCLUDED.observation_id,
        material_id = EXCLUDED.material_id, operation_id = EXCLUDED.operation_id,
        event_id = EXCLUDED.event_id, trust_tier = EXCLUDED.trust_tier,
        claim_status = EXCLUDED.claim_status, corroborated = EXCLUDED.corroborated,
        conflicted = EXCLUDED.conflicted, perceived_at = EXCLUDED.perceived_at,
        valid_until = EXCLUDED.valid_until, answers = EXCLUDED.answers, payload = EXCLUDED.payload;
    INSERT INTO terrain.outbox VALUES (v_e, v_c, v_op, v_ev, v_obs_id, v_mat_id, v_eval_id, v_clm_id, v_rev, v_hash, p_event);
    RETURN p_event;
EXCEPTION WHEN data_exception THEN RAISE EXCEPTION 'terrain_event_refused';
END; $$;

-- The only conflict command accepts intent, never affected references or status.
CREATE FUNCTION terrain.record_dns_integrity_conflict(p_request jsonb, p_operation uuid)
RETURNS boolean LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE admitted jsonb; r jsonb; source terrain.outbox%ROWTYPE; blocks boolean; frontier bigint;
BEGIN
    admitted:=terrain.admit_dns_request(p_request); r:=admitted->'request';
    SELECT * INTO source FROM terrain.outbox WHERE engagement_id=(r->>'engagement_id')::uuid
        AND campaign_id=(r->>'campaign_id')::uuid AND operation_id=p_operation FOR UPDATE;
    IF NOT FOUND OR source.request_hash=admitted->>'request_hash' THEN RAISE EXCEPTION 'terrain_conflict_refused'; END IF;
    -- Lock the affected projection as well; accepted source/history is never updated.
    PERFORM 1 FROM terrain.current_claims cc JOIN terrain.claim_revisions cr ON cr.claim_id=source.claim_id
        WHERE cc.engagement_id=cr.engagement_id AND cc.campaign_id=cr.campaign_id AND cc.subject_name=cr.subject_name
          AND cc.claim_kind=cr.claim_kind AND cc.vantage_ref=cr.vantage_ref AND cc.resolver_ipv4=cr.resolver_ipv4 FOR UPDATE OF cc;
    blocks:=r->'subject_name'=source.contract#>'{observation,subject_name}'
        AND r->'vantage_ref'=source.contract#>'{observation,vantage_ref}' AND r->'resolver_ipv4'=source.contract#>'{observation,resolver_ipv4}'
        AND r->'asset_ref'=source.contract#>'{observation,asset_ref}' AND r->'truncated'='false'::jsonb
        AND r->>'completeness'='complete' AND r->>'rcode'='noerror' AND jsonb_array_length(r->'answers')>0
        AND NOT (r->'limitations' @> '["partial_alias_chain"]'::jsonb)
        AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements(r->'answers') WHERE (value->>'ttl_seconds')::integer=0);
    frontier:=greatest((source.contract#>>'{claim,temporal_frontier}')::bigint,(r->>'perceived_at')::bigint);
    INSERT INTO terrain.integrity_conflicts VALUES(source.engagement_id,source.campaign_id,source.operation_id,source.claim_id,frontier,blocks,'input_mismatch') ON CONFLICT DO NOTHING;
    RETURN blocks;
END; $$;

CREATE FUNCTION trajectory.record_terrain_history(p_event jsonb)
RETURNS text LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE
    v_e uuid; v_c uuid; v_op uuid;
    v_source terrain.outbox%ROWTYPE; v_hist trajectory.terrain_history%ROWTYPE;
    v_source_ok boolean; v_reason text;
    v_now bigint := floor(extract(epoch FROM statement_timestamp()))::bigint;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_trajectory', 'MEMBER')
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['dw_runtime','dw_m1_broker','dw_m1_cognition','dw_m1_terrain']) g WHERE pg_has_role(session_user, g, 'MEMBER')) THEN
        RAISE EXCEPTION 'unqualified_trajectory_writer' USING ERRCODE = '42501';
    END IF;
    -- Parse only lookup references. Never expose cast errors containing rejected values.
    BEGIN
        v_e := (p_event->>'engagement_id')::uuid;
        v_c := (p_event->>'campaign_id')::uuid;
        v_op := (p_event->>'operation_id')::uuid;
    EXCEPTION WHEN invalid_text_representation THEN
        RETURN 'uncommitted';
    END;
    SELECT * INTO v_source FROM terrain.outbox
    WHERE engagement_id=v_e AND campaign_id=v_c AND operation_id=v_op;
    IF NOT FOUND THEN RETURN 'uncommitted'; END IF;
    IF EXISTS (SELECT 1 FROM trajectory.terrain_history
               WHERE engagement_id=v_e AND campaign_id=v_c AND operation_id=v_op AND status='anomaly')
       OR EXISTS (SELECT 1 FROM terrain.integrity_conflicts
                  WHERE engagement_id=v_e AND campaign_id=v_c AND operation_id=v_op AND blocks_use) THEN
        RETURN 'anomaly';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM mission.registration_outbox r JOIN trajectory.registration_history h
          ON h.engagement_id=r.engagement_id AND h.campaign_id=r.campaign_id
         AND h.operation_id=r.operation_id AND h.event_id=r.event_id AND h.status='accepted'
        WHERE r.engagement_id=v_e AND r.campaign_id=v_c
          AND r.operation_id=(v_source.contract->>'registration_operation_id')::uuid
          AND r.event_id=(v_source.contract->>'registration_event_id')::uuid
    ) OR EXISTS (SELECT 1 FROM trajectory.registration_history
                 WHERE engagement_id=v_e AND campaign_id=v_c AND status='anomaly') THEN
        RETURN 'pending';
    END IF;
    SELECT EXISTS (
        SELECT 1 FROM terrain.observations ob
        JOIN terrain.materials m USING (engagement_id,campaign_id,operation_id)
        JOIN terrain.evaluations ev USING (engagement_id,campaign_id,operation_id)
        JOIN terrain.claim_revisions cr USING (engagement_id,campaign_id,operation_id)
        WHERE ob.engagement_id=v_e AND ob.campaign_id=v_c AND ob.operation_id=v_op
          AND v_source.contract=p_event
          AND ob.observation_id=v_source.observation_id AND ob.request_hash=v_source.request_hash
          AND ob.payload=(v_source.contract->'observation')
          AND m.material_id=v_source.material_id AND m.observation_id=ob.observation_id
          AND m.payload=(v_source.contract->'material')
          AND ev.evaluation_id=v_source.evaluation_id AND ev.claim_id=v_source.claim_id
          AND ev.payload=(v_source.contract->'evaluation')
          AND cr.claim_id=v_source.claim_id AND cr.claim_revision=v_source.claim_revision
          AND cr.payload=(v_source.contract->'claim')
          AND v_source.event_id=(v_source.contract->>'event_id')::uuid
    ) INTO v_source_ok;
    SELECT * INTO v_hist FROM trajectory.terrain_history
    WHERE ((engagement_id=v_e AND campaign_id=v_c AND operation_id=v_op)
           OR event_id=v_source.event_id) AND status='accepted' LIMIT 1;
    IF FOUND THEN
        IF v_source_ok AND v_hist.contract=v_source.contract
           AND v_hist.engagement_id=v_e AND v_hist.campaign_id=v_c AND v_hist.operation_id=v_op
           AND v_hist.event_id=v_source.event_id AND v_hist.observation_id=v_source.observation_id
           AND v_hist.material_id=v_source.material_id AND v_hist.evaluation_id=v_source.evaluation_id
           AND v_hist.claim_id=v_source.claim_id THEN
            RETURN 'completed';
        END IF;
        v_reason := 'contract_conflict';
    ELSIF NOT v_source_ok THEN
        v_reason := 'source_conflict';
    END IF;
    IF v_reason IS NOT NULL THEN
        INSERT INTO trajectory.terrain_history
            (engagement_id,campaign_id,operation_id,event_id,observation_id,material_id,evaluation_id,claim_id,status,reason,contract,recorded_at)
        VALUES (v_e,v_c,v_op,v_source.event_id,v_source.observation_id,v_source.material_id,
                v_source.evaluation_id,v_source.claim_id,'anomaly',v_reason,
                jsonb_build_object('category',v_reason,'affected_claim_id',v_source.claim_id),v_now);
        RETURN 'anomaly';
    END IF;
    INSERT INTO trajectory.terrain_history
        (engagement_id,campaign_id,operation_id,event_id,observation_id,material_id,evaluation_id,claim_id,status,reason,contract,recorded_at)
    VALUES (v_e,v_c,v_op,v_source.event_id,v_source.observation_id,v_source.material_id,
            v_source.evaluation_id,v_source.claim_id,'accepted',NULL,v_source.contract,v_now);
    RETURN 'completed';
END; $$;

REVOKE ALL ON SCHEMA terrain FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;
REVOKE ALL ON ALL TABLES IN SCHEMA terrain FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;
REVOKE ALL ON ALL FUNCTIONS IN SCHEMA terrain FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;
REVOKE ALL ON TABLE trajectory.terrain_history FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;
REVOKE ALL ON SEQUENCE trajectory.terrain_history_history_seq_seq FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;
REVOKE ALL ON FUNCTION trajectory.record_terrain_history(jsonb) FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;

GRANT USAGE ON SCHEMA terrain TO dw_m1_terrain, dw_m1_trajectory;
-- Table-level REVOKE does not remove independently granted column privileges.
DO $$ DECLARE t record; cols text; BEGIN
    FOR t IN SELECT c.oid,c.relname FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
        WHERE n.nspname='terrain' AND c.relkind='r' LOOP
        SELECT string_agg(quote_ident(attname),',') INTO cols FROM pg_catalog.pg_attribute WHERE attrelid=t.oid AND attnum>0 AND NOT attisdropped;
        EXECUTE format('REVOKE INSERT (%s), UPDATE (%s), REFERENCES (%s) ON terrain.%I FROM PUBLIC,dw_runtime,dw_m1_broker,dw_m1_cognition,dw_m1_trajectory,dw_m1_terrain',cols,cols,cols,t.relname);
    END LOOP;
END $$;
GRANT SELECT ON terrain.observations, terrain.materials, terrain.evaluations, terrain.claim_revisions, terrain.outbox, terrain.integrity_conflicts, terrain.current_claims TO dw_m1_terrain;
GRANT EXECUTE ON FUNCTION terrain.accept_dns_observation(jsonb,jsonb), terrain.record_dns_integrity_conflict(jsonb,uuid) TO dw_m1_terrain;
GRANT SELECT ON terrain.observations, terrain.materials, terrain.evaluations, terrain.claim_revisions, terrain.current_claims, terrain.outbox TO dw_m1_trajectory;
GRANT SELECT ON TABLE trajectory.terrain_history TO dw_m1_trajectory, dw_m1_terrain, dw_runtime;
GRANT EXECUTE ON FUNCTION trajectory.record_terrain_history(jsonb) TO dw_m1_trajectory;
GRANT USAGE ON SCHEMA mission, trajectory TO dw_m1_terrain;
GRANT SELECT ON mission.missions, mission.planning_assessments, mission.registration_outbox, mission.withdrawals TO dw_m1_terrain;

GRANT SELECT ON terrain.integrity_conflicts, terrain.claim_integrity_holds TO dw_m1_terrain, dw_m1_trajectory;

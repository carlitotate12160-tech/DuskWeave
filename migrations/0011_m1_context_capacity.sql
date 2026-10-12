-- Broker-owned read projection only: no reservation, epoch write or send grant.
CREATE OR REPLACE FUNCTION execution.inference_capacity_snapshot(e uuid,c uuid)
RETURNS jsonb LANGUAGE sql STABLE SET search_path=pg_catalog AS $$
    SELECT jsonb_build_object('version','m1_inference_capacity_snapshot_v1',
        'epoch',f.epoch,'session_claimed',f.operation_id IS NOT NULL,
        'active_holds',count(h.operation_id),'charged',jsonb_build_object(
            'calls',coalesce(sum((h.capacity->>'calls')::bigint),0),
            'request_bytes',coalesce(sum((h.capacity->>'request_bytes')::bigint),0),
            'response_bytes',coalesce(sum((h.capacity->>'response_bytes')::bigint),0),
            'output_token_capacity',coalesce(sum((h.capacity->>'output_token_capacity')::bigint),0),
            'wall_time_ms',coalesce(sum((h.capacity->>'wall_time_ms')::bigint),0)))
    FROM execution.session_fences f LEFT JOIN execution.inference_reservations h
    ON (h.engagement_id,h.campaign_id)=(f.engagement_id,f.campaign_id)
    AND h.record_revision=1 AND NOT EXISTS (SELECT 1 FROM execution.inference_reservations x
        WHERE (x.engagement_id,x.campaign_id,x.operation_id)=
              (h.engagement_id,h.campaign_id,h.operation_id) AND x.record_revision=2)
    WHERE f.engagement_id=e AND f.campaign_id=c GROUP BY f.epoch,f.operation_id
$$;
REVOKE ALL ON FUNCTION execution.inference_capacity_snapshot(uuid,uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.inference_capacity_snapshot(uuid,uuid) TO dw_m1_broker;

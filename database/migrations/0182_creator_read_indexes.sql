-- CREATOR-READ-PERFORMANCE-001: existing qualification remains authoritative.
-- Dossier media hydration probes download attempts per observation, not by attempt PK.
CREATE INDEX linggan_media_download_attempt_observation_idx
    ON linggan_media_download_attempt (media_observation_ref);

-- The source eligibility function resolves a finite work's latest media-slot manifest.
CREATE INDEX linggan_material_lane_observation_work_lane_idx
    ON linggan_material_lane_observation (content_public_ref, lane, created_at DESC);

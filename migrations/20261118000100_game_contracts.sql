ALTER TABLE worldwright_content_artifacts
    DROP CONSTRAINT worldwright_artifact_game_v1;

ALTER TABLE worldwright_content_artifacts
    ADD CONSTRAINT worldwright_artifact_game_v1 CHECK (
        (game_id = 'worldwright-reference' AND game_contract_version = 1)
        OR (game_id = 'worldwright-expedition' AND game_contract_version = 1)
    );

ALTER TABLE worldwright_publication_requests
    ADD COLUMN studio_review_context JSONB;

ALTER TABLE worldwright_publication_requests
    ADD CONSTRAINT worldwright_publication_studio_context_valid CHECK (
        studio_review_context IS NULL OR (
            jsonb_typeof(studio_review_context) = 'object'
            AND octet_length(studio_review_context::TEXT) <= 4096
            AND studio_review_context @> '{"schema_version":1,"validation_diagnostic_count":0}'::JSONB
        )
    );

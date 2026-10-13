CREATE TABLE accounts (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name           TEXT NOT NULL,
    currency       CHAR(3) NOT NULL,
    allow_negative BOOLEAN NOT NULL DEFAULT false,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE journal_entries (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    description TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE postings (
    id               BIGSERIAL PRIMARY KEY,
    journal_entry_id UUID NOT NULL REFERENCES journal_entries(id),
    account_id       UUID NOT NULL REFERENCES accounts(id),
    amount           BIGINT NOT NULL CHECK (amount <> 0),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_postings_entry ON postings(journal_entry_id);
CREATE INDEX idx_postings_account_id ON postings(account_id, id DESC);

CREATE FUNCTION check_entry_balanced() RETURNS trigger AS $$
BEGIN
    IF (SELECT COALESCE(SUM(amount), 0) FROM postings
        WHERE journal_entry_id = NEW.journal_entry_id) <> 0 THEN
        RAISE EXCEPTION 'journal entry % is not balanced', NEW.journal_entry_id;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER postings_balanced
    AFTER INSERT ON postings
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION check_entry_balanced();
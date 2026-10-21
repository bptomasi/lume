CREATE TRIGGER test_reject_configuration_ack
BEFORE UPDATE OF snapshot ON configuration_state
BEGIN
    SELECT RAISE(ABORT, 'injected configuration acknowledgement failure');
END;

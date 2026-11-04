# Retained first native attempt

Candidate2f52fde0 failed while waiting for an active socket after canonical UI login; Map resources loaded and no page error was reported. The original HTTP ticket fixture used separate `Date.now()` reads for `server_time` and expiry, so a millisecond boundary could exceed the contract's exact30-second maximum. The owner changed only the new fixture to derive both values from one timestamp and added safe timing metadata. Candidate9523b0b5 then passed all ten contexts under the same original clock. The failed response timestamps were not captured, so this is a repaired fixture hazard and a plausible explanation, not proof of the exact first-attempt cause or a production transport defect.

The FAIL report/logs and full cleanup remain byte-exact. No application code, contract guard or ART source was weakened, no lease extension/new slot was taken, and no historical ART05 capture was replayed.

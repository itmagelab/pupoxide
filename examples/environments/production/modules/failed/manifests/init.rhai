exec("/bin/sleep 2", #{ });

// --- Graceful Failure Handling Example ---
// This resource is guaranteed to fail
exec("false", #{ id: "failing_task" })
// This resource depends on the failing one and will be SKIPPED
-> exec("echo 'You will never see this' > /tmp/should_be_skipped", #{ id: "dependent_task" });

// This resource is independent and will still SUCCEED despite the failure above
exec("echo 'Pupoxide handles failures gracefully!'", #{ });

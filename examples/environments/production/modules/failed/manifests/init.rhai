exec("/bin/sleep 2", #{ });

// This resource is guaranteed to fail with a very large output
exec("curl -v https://nonexistent-domain-123456789.com; exit 1", #{ id: "failing_task" })
// This resource depends on the failing one and will be SKIPPED
-> exec("echo 'You will never see this' > /tmp/should_be_skipped", #{ id: "dependent_task" });

// This resource is independent and will still SUCCEED despite the failure above
exec("echo 'Pupoxide handles failures gracefully!'", #{ });

// This test manifest verifies that resources with the same mutex group run serially.
// We use 'exec' with sleep and file creation to observe the order.

exec("task-1", #{
    command: "echo 'start 1' >> /tmp/mutex_test.log; sleep 2; echo 'end 1' >> /tmp/mutex_test.log",
    mutex: "serial_tasks"
});

exec("task-2", #{
    command: "echo 'start 2' >> /tmp/mutex_test.log; sleep 1; echo 'end 2' >> /tmp/mutex_test.log",
    mutex: "serial_tasks",
    require: "Exec[task-1]" // Dependencies still work, but mutex ensures absolute serialization if they were parallel
});

exec("task-3", #{
    command: "echo 'independent' >> /tmp/mutex_test.log",
    // No mutex, should run in parallel if ready
});

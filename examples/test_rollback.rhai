file("/tmp/rollback_test.txt", #{
    ensure: "present",
    content: "Modified Content",
    backup: true
});

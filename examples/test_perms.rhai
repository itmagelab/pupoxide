file("/tmp/test_perms.txt", #{
    ensure: "present",
    content: "permission test",
    mode: "600"
});

directory("/tmp/test_dir_perms", #{
    ensure: "present",
    mode: "750"
});

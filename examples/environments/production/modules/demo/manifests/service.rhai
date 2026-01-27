let pid = "/tmp/pupoxide/demo/.demo.pid";

exec(`touch ${pid}`, #{ creates: pid });

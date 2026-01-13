// Демонстрация новых возможностей импорта и зависимостей модулей

// 1. Относительный include() возвращает ModuleHandle
// Это позволяет выстраивать цепочки зависимостей
let setup = include("./setup");
let app = include("./app");

// setup должен выполниться полностью ДО начала app
setup -> app;

// 2. Нативный import также поддерживает относительные пути
import "./utils" as u;

// 3. Зависимость между ресурсом и модулем
file("/tmp/demo_ready", #{
    ensure: "present",
    content: u.get_banner("Pupoxide Demo")
}) -> setup;

// 4. Зависимость между модулем и ресурсом
app -> file("/tmp/demo_finished", #{
    ensure: "present"
});

// 5. Использование нативного модуля в require
file("/tmp/app_config", #{
    ensure: "present",
    require: setup // Ресурс требует завершения всего модуля setup
});
